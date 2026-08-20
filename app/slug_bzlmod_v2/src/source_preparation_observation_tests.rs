#[derive(Debug, Clone)]
struct HostSourceActivation {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Debug, Default)]
struct HostSourceFamilyTracker {
    activations: Mutex<Vec<HostSourceActivation>>,
    rows: Mutex<Vec<(String, Vec<String>)>>,
}

impl HostSourceFamilyTracker {
    fn take(&self) -> Vec<HostSourceActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }

    fn take_rows(&self) -> Vec<(String, Vec<String>)> {
        std::mem::take(&mut *self.rows.lock().unwrap())
    }
}

fn assert_repository_source_eventless(activations: &[HostSourceActivation]) {
    assert!(
        activations
            .iter()
            .filter(|entry| {
                entry.key.contains("repository-source-file:")
                    || entry.key.contains("resolved-path:")
                    || entry.key.starts_with("path-observation:")
            })
            .all(|entry| entry.batch.is_none())
    );
}

impl ActivationTracker for HostSourceFamilyTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        _: ActivationData,
    ) {
        self.rows
            .lock()
            .unwrap()
            .push((key.to_string(), deps.map(ToString::to_string).collect()));
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.activations.lock().unwrap().push(HostSourceActivation {
            key: key.to_string(),
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

async fn observed_source_transaction(
    dice: &Arc<Dice>,
    materialization: RepositoryMaterializationResultEpoch,
    observations: PathObservationEpoch,
    tracker: Option<Arc<HostSourceFamilyTracker>>,
) -> dice::DiceTransaction {
    let data = UserComputationData {
        activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    let mut updater = dice.updater_with_data(data);
    updater
        .changed_to(vec![(PathObservationEpochKey, observations)])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            materialization,
        )])
        .unwrap();
    updater.commit().await
}

fn complete_observed_source(
    value: &<HostRepositorySourceFileObservationKey as Key>::Value,
) -> &ObservedHostRepositorySourceFile {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed Host repository source must complete");
    };
    observed
}

fn assert_exact_epoch(expected: &PathObservationEpoch, actual: &PathObservationEpoch) {
    assert_eq!(actual.observations().len(), expected.observations().len());
    for ((demand, result), (actual_demand, actual_result)) in
        expected.observations().iter().zip(actual.observations())
    {
        assert_eq!(actual_demand, demand, "epoch iteration order changed");
        assert!(
            Arc::ptr_eq(actual_result, result),
            "result Arc changed for {demand:?}"
        );
    }
}

async fn assert_selected_epoch(
    transaction: &mut dice::DiceTransaction,
    expected: &PathObservationEpoch,
    actual: &PathObservationEpoch,
) {
    assert_eq!(actual.observations().len(), expected.observations().len());
    for demand in expected.observations().keys() {
        let selected = transaction
            .compute(&PathObservationKey::new(demand.dupe()))
            .await
            .unwrap();
        let PathOutcome::Complete(selected) = selected else {
            panic!("selected observation must be complete");
        };
        assert!(Arc::ptr_eq(actual.get(demand).unwrap(), &selected));
    }
}

fn source_epoch(
    path: PathObservationEpoch,
    namespace: PathObservationNamespace,
    real_path: &str,
    result: PathObservationResult,
) -> PathObservationEpoch {
    append_host_repository_source_observation(
        &path,
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(real_path).unwrap(),
            PathObservationOperation::FileBytes,
        ),
        Arc::new(result),
    )
    .unwrap()
}

#[tokio::test]
async fn observed_host_source_preserves_exact_symlink_epoch_and_isolates_families() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let path = symlink_path_epoch("/workspace/dep/link", "/physical/source");
    let epoch = source_epoch(
        path,
        PathObservationNamespace::Host,
        "/physical/source",
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
            b"source-a".as_slice(),
        ))),
    );
    let mut transaction =
        observed_source_transaction(&dice, material("dep"), epoch.dupe(), Some(tracker.dupe()))
            .await;
    let key = HostRepositorySourceFileObservationKey::new(local_route(), PathBuf::from("link"));
    assert_ne!(
        key.to_string(),
        HostRepositorySourceFileKey::new(local_route(), PathBuf::from("link")).to_string()
    );
    let cold = transaction.compute(&key).await.unwrap();
    assert!(HostRepositorySourceFileObservationKey::validity(&cold));
    let observed = complete_observed_source(&cold);
    assert!(matches!(
        observed.result().as_ref(),
        Ok(HostRepositorySourceFileValue::Present { bytes, logical_path })
            if bytes.as_ref() == b"source-a"
                && logical_path.as_path() == Path::new("/workspace/dep/link")
    ));
    assert_exact_epoch(&epoch, observed.observations());

    let warm = transaction.compute(&key).await.unwrap();
    assert!(HostRepositorySourceFileObservationKey::equality(
        &cold, &warm
    ));
    let warm_observed = complete_observed_source(&warm);
    assert!(Arc::ptr_eq(observed.result(), warm_observed.result()));
    assert_exact_epoch(&epoch, warm_observed.observations());

    let activations = tracker.take();
    assert!(activations.iter().all(|entry| entry.batch.is_none()));
    assert!(activations.iter().any(|entry| matches!(
        entry.kind,
        ActivationKind::Evaluated | ActivationKind::Reused
    )));
    assert!(
        activations
            .iter()
            .any(|entry| entry.key.starts_with("observed-host-repository-path:"))
    );
    assert!(activations.iter().any(|entry| {
        entry
            .key
            .starts_with("observed-host-repository-source-file:")
    }));
    assert!(!activations.iter().any(|entry| {
        entry.key.starts_with("host-repository-path:")
            || entry.key.starts_with("host-repository-source-file:")
    }));

    let mut legacy = host_path_transaction(&dice, material("dep"), epoch).await;
    let legacy = legacy
        .compute(&HostRepositorySourceFileKey::new(
            local_route(),
            PathBuf::from("link"),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy source must complete");
    };
    assert_eq!(&legacy, observed.result().as_ref());
}

#[tokio::test]
async fn observed_host_source_retains_semantic_prefixes_and_complete_only_validity() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let invalid = HostRepositoryPathObservationKey(HostRepositoryPathKey::new(
        local_route(),
        PathBuf::from("../BUILD"),
    ));
    let mut invalid_tx =
        observed_source_transaction(&dice, material("dep"), PathObservationEpoch::empty(), None)
            .await;
    let invalid = invalid_tx.compute(&invalid).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(invalid)) = invalid else {
        panic!("invalid relative path is a semantic terminal");
    };
    assert!(matches!(
        invalid.result.as_ref(),
        Err(RepositorySourceFileError::InvalidRepoRelativePath { .. })
    ));
    assert!(invalid.observations.observations().is_empty());

    for (kind, expected_absent) in [(None, true), (Some(PathNodeKind::Directory), false)] {
        let epoch = host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep/BUILD.bazel",
            kind,
            None,
        );
        let mut transaction =
            observed_source_transaction(&dice, material("dep"), epoch.dupe(), None).await;
        let value = transaction
            .compute(&HostRepositorySourceFileObservationKey::new(
                local_route(),
                PathBuf::from("BUILD.bazel"),
            ))
            .await
            .unwrap();
        let observed = complete_observed_source(&value);
        if expected_absent {
            assert!(matches!(
                observed.result().as_ref(),
                Ok(HostRepositorySourceFileValue::Absent)
            ));
        } else {
            assert!(matches!(
                observed.result().as_ref(),
                Err(RepositorySourceFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                })
            ));
        }
        let observations = observed.observations().dupe();
        assert_selected_epoch(&mut transaction, &epoch, &observations).await;
    }

    let pending_epoch = host_path_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep/BUILD.bazel",
        Some(PathNodeKind::RegularFile),
        None,
    );
    let mut pending =
        observed_source_transaction(&dice, material("dep"), pending_epoch, None).await;
    let pending = pending
        .compute(&HostRepositorySourceFileObservationKey::new(
            local_route(),
            PathBuf::from("BUILD.bazel"),
        ))
        .await
        .unwrap();
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));
    assert!(!HostRepositorySourceFileObservationKey::validity(&pending));
    assert!(!HostRepositorySourceFileObservationKey::equality(
        &pending, &pending
    ));
}

#[tokio::test]
async fn observed_host_source_covers_path_need_route_error_and_reverse_isolation() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut pending =
        observed_source_transaction(&dice, material("dep"), PathObservationEpoch::empty(), None)
            .await;
    let pending = pending
        .compute(&HostRepositoryPathObservationKey(
            HostRepositoryPathKey::new(local_route(), PathBuf::from("BUILD.bazel")),
        ))
        .await
        .unwrap();
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));

    let builtin =
        RootRepositoryRoute::builtin_for_test(NormalizedAbsolutePath::new("/workspace").unwrap());
    let mut invalid =
        observed_source_transaction(&dice, material("dep"), PathObservationEpoch::empty(), None)
            .await;
    let invalid = invalid
        .compute(&HostRepositorySourceFileObservationKey::new(
            builtin,
            PathBuf::from("BUILD.bazel"),
        ))
        .await
        .unwrap();
    let observed = complete_observed_source(&invalid);
    assert!(matches!(
        observed.result().as_ref(),
        Err(RepositorySourceFileError::Materialization { .. })
    ));
    assert!(observed.observations().observations().is_empty());

    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let epoch = source_epoch(
        host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep/BUILD.bazel",
            Some(PathNodeKind::RegularFile),
            None,
        ),
        PathObservationNamespace::Host,
        "/workspace/dep/BUILD.bazel",
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
            b"legacy".as_slice(),
        ))),
    );
    let mut legacy =
        observed_source_transaction(&dice, material("dep"), epoch, Some(tracker.dupe())).await;
    legacy
        .compute(&HostRepositorySourceFileKey::new(
            local_route(),
            PathBuf::from("BUILD.bazel"),
        ))
        .await
        .unwrap();
    let activations = tracker.take();
    assert!(
        activations
            .iter()
            .any(|entry| entry.key.starts_with("host-repository-source-file:"))
    );
    assert!(
        !activations
            .iter()
            .any(|entry| entry.key.starts_with("observed-host-repository-"))
    );
}

#[tokio::test]
async fn observed_host_source_preserves_immutable_and_file_error_prefixes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let instance = PathObservationInstanceId::new(73);
    let namespace = PathObservationNamespace::Materialization(instance);
    for result in [
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
            b"immutable".as_slice(),
        ))),
        PathObservationResult::FileBytes(PathOperationResult::Missing),
        PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::NotALink,
        )),
    ] {
        let path = host_path_epoch(
            namespace,
            "/generation/73/BUILD.bazel",
            Some(PathNodeKind::SpecialFile),
            None,
        );
        let epoch = source_epoch(path, namespace, "/generation/73/BUILD.bazel", result);
        let mut transaction = observed_source_transaction(
            &dice,
            immutable_material("/generation/73", instance),
            epoch.dupe(),
            None,
        )
        .await;
        let value = transaction
            .compute(&HostRepositorySourceFileObservationKey::new(
                immutable_route(),
                PathBuf::from("BUILD.bazel"),
            ))
            .await
            .unwrap();
        let observed = complete_observed_source(&value);
        let observations = observed.observations().dupe();
        assert_selected_epoch(&mut transaction, &epoch, &observations).await;
        match epoch
            .observations()
            .values()
            .find(|value| value.operation() == PathObservationOperation::FileBytes)
            .unwrap()
            .as_ref()
        {
            PathObservationResult::FileBytes(PathOperationResult::Present(_)) => assert!(matches!(
                observed.result().as_ref(),
                Ok(HostRepositorySourceFileValue::Present { bytes, .. })
                    if bytes.as_ref() == b"immutable"
            )),
            PathObservationResult::FileBytes(PathOperationResult::Missing) => assert!(matches!(
                observed.result().as_ref(),
                Err(RepositorySourceFileError::InconsistentState { .. })
            )),
            PathObservationResult::FileBytes(PathOperationResult::Error(_)) => assert!(matches!(
                observed.result().as_ref(),
                Err(RepositorySourceFileError::Observation { .. })
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn observed_host_source_union_is_left_first_and_fail_closed() {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/dep/BUILD.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let first = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"same".as_slice())),
    ));
    let equal = Arc::new(first.as_ref().clone());
    let base = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
    let merged = append_host_repository_source_observation(&base, demand.dupe(), equal).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));

    let conflict = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"different".as_slice())),
    ));
    assert!(matches!(
        append_host_repository_source_observation(&base, demand.dupe(), conflict),
        Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found)
        )) if found == demand
    ));
    let wrong_operation = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
    assert!(matches!(
        append_host_repository_source_observation(
            &PathObservationEpoch::empty(),
            demand,
            wrong_operation
        ),
        Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. }
        ))
    ));
}

#[tokio::test]
async fn observed_host_source_cancellation_publishes_nothing_and_recovers_aba() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let key =
        HostRepositorySourceFileObservationKey::new(local_route(), PathBuf::from("BUILD.bazel"));
    let epoch = |bytes: &'static [u8]| {
        source_epoch(
            host_path_epoch(
                PathObservationNamespace::Host,
                "/workspace/dep/BUILD.bazel",
                Some(PathNodeKind::RegularFile),
                None,
            ),
            PathObservationNamespace::Host,
            "/workspace/dep/BUILD.bazel",
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
        )
    };
    let a = epoch(b"a");
    let mut cancelled =
        observed_source_transaction(&dice, material("dep"), a.dupe(), Some(tracker.dupe())).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().is_empty());

    let mut first =
        observed_source_transaction(&dice, material("dep"), a, Some(tracker.dupe())).await;
    let first = first.compute(&key).await.unwrap();
    let first_observed = complete_observed_source(&first);
    assert!(matches!(
        first_observed.result().as_ref(),
        Ok(HostRepositorySourceFileValue::Present { bytes, .. }) if bytes.as_ref() == b"a"
    ));
    tracker.take();

    let mut changed = observed_source_transaction(&dice, material("dep"), epoch(b"b"), None).await;
    let changed = changed.compute(&key).await.unwrap();
    assert!(matches!(
        complete_observed_source(&changed).result().as_ref(),
        Ok(HostRepositorySourceFileValue::Present { bytes, .. }) if bytes.as_ref() == b"b"
    ));

    let mut restored = observed_source_transaction(&dice, material("dep"), epoch(b"a"), None).await;
    let restored = restored.compute(&key).await.unwrap();
    assert_eq!(
        first_observed.result().as_ref(),
        complete_observed_source(&restored).result().as_ref()
    );
}

fn complete_repository_source(
    value: &<RepositorySourceFileObservationKey as Key>::Value,
) -> &ObservedRepositorySourceFileValue {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed repository source must complete: {value:?}");
    };
    observed
}

fn repository_source_key(path: &str) -> RepositorySourceFileKey {
    RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from(path),
    }
}
fn extend_repository_source_epoch(
    prefix: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                incoming
                    .observations()
                    .iter()
                    .filter(|(demand, _)| prefix.observations().get(*demand).is_none())
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .unwrap()
}
async fn repository_source_transaction(
    dice: &Arc<Dice>,
    source: &str,
    variant: i64,
    result: RepositoryMaterializationResult,
    source_epoch: PathObservationEpoch,
    tracker: Arc<HostSourceFamilyTracker>,
) -> (dice::DiceTransaction, PathObservationEpoch) {
    let mut data = UserComputationData {
        activation_tracker: Some(tracker.dupe()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    inject_materialization_request_inputs(&mut updater, source, variant, &[], true);
    let mut transaction = updater.commit().await;
    let request = transaction
        .compute(&RepositoryMaterializationRequestObservationKey::new(
            PathBuf::from("/workspace"),
            "dep".into(),
        ))
        .await
        .unwrap();
    let request = observed_materialization_value(&request);
    let prefix = request.observations().dupe();
    let request = request.result().as_ref().as_ref().unwrap().clone();
    let epoch = extend_repository_source_epoch(&prefix, &source_epoch);
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            RepositoryMaterializationResultEpoch::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [RepositoryMaterializationEpochEntry {
                    request: Arc::new(request),
                    result,
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let materialization = transaction
        .compute(&RepositoryMaterializationObservationKey::new(
            PathBuf::from("/workspace"),
            "dep".into(),
        ))
        .await
        .unwrap();
    let epoch = extend_repository_source_epoch(
        observed_repository_materialization(&materialization).observations(),
        &source_epoch,
    );
    (transaction, epoch)
}
async fn repository_source_case(
    dice: &Arc<Dice>,
    source: &str,
    variant: i64,
    result: RepositoryMaterializationResult,
    source_epoch: PathObservationEpoch,
) -> (
    <RepositorySourceFileObservationKey as Key>::Value,
    <RepositorySourceFileObservationKey as Key>::Value,
    <RepositorySourceFileKey as Key>::Value,
    PathObservationEpoch,
    Vec<Vec<HostSourceActivation>>,
    Vec<(String, Vec<String>)>,
) {
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut transaction, epoch) =
        repository_source_transaction(dice, source, variant, result, source_epoch, tracker.dupe())
            .await;
    let child = tracker.take();
    tracker.take_rows();
    let observed_key = RepositorySourceFileObservationKey(repository_source_key("file"));
    let cold = transaction.compute(&observed_key).await.unwrap();
    let cold_activations = tracker.take();
    let mut rows = tracker.take_rows();
    let warm = transaction.compute(&observed_key).await.unwrap();
    let warm_activations = tracker.take();
    rows.extend(tracker.take_rows());
    let legacy = transaction
        .compute(&repository_source_key("file"))
        .await
        .unwrap();
    let legacy_activations = tracker.take();
    rows.extend(tracker.take_rows());
    if let SourcePreparationOutcome::Complete(Ok(observed)) = &cold {
        assert_selected_epoch(&mut transaction, &epoch, observed.observations()).await;
    }
    (
        cold,
        warm,
        legacy,
        epoch,
        vec![
            child,
            cold_activations,
            warm_activations,
            legacy_activations,
        ],
        rows,
    )
}
#[tokio::test]
async fn observed_repository_source_projection_and_algebra_are_exact() {
    let key = RepositorySourceFileObservationKey(repository_source_key("file"));
    assert_eq!(key.to_string(), "observed-repository-source-file:dep:file");
    let other = RepositorySourceFileObservationKey(repository_source_key("other"));
    assert_ne!(key, other);
    assert_ne!(test_hash(&key), test_hash(&other));
    let mut tx = Dice::builder()
        .build(DetectCycles::Enabled)
        .updater()
        .commit()
        .await;
    let invalid = tx
        .compute(&RepositorySourceFileObservationKey(repository_source_key(
            "../file",
        )))
        .await
        .unwrap();
    let invalid = complete_repository_source(&invalid);
    assert!(matches!(
        invalid.result().as_ref(),
        Err(RepositorySourceFileError::InvalidRepoRelativePath { .. })
    ));
    assert!(invalid.observations().observations().is_empty());
    let bytes = Arc::<[u8]>::from(b"bytes".as_slice());
    let carrier = repository_source_file_complete(
        Ok(RepositorySourceFileValue::Present(bytes.dupe())),
        PathObservationEpoch::empty(),
    );
    let held = complete_repository_source(&carrier).result().dupe();
    assert!(matches!(project_legacy_repository_source_file(carrier),
        SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Present(found)))
            if Arc::ptr_eq(&bytes, &found)));
    assert!(
        matches!(held.as_ref(), Ok(RepositorySourceFileValue::Present(found))
        if Arc::ptr_eq(&bytes, found))
    );
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/dep/file").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let first = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"same".as_slice())),
    ));
    let prefix = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
    let equal =
        PathObservationEpoch::from_shared([(demand.dupe(), Arc::new(first.as_ref().clone()))])
            .unwrap();
    assert!(Arc::ptr_eq(
        merge_path_observations(&prefix, &equal)
            .unwrap()
            .get(&demand)
            .unwrap(),
        &first
    ));
    let conflict = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(b"changed".as_slice())),
        )),
    )])
    .unwrap();
    assert!(matches!(
        merge_path_observations(&prefix, &conflict),
        Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(_)
        ))
    ));
    assert!(matches!(
        append_host_repository_source_observation(
            &prefix,
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        ),
        Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. }
        ))
    ));
    let path = Arc::new(PathBuf::from("file"));
    let materialization =
        repository_source_materialization_compute_error(path.dupe(), "materialization".into());
    let materialization = complete_repository_source(&materialization);
    assert!(matches!(materialization.result().as_ref(),
        Err(RepositorySourceFileError::MaterializationCompute { message, .. })
            if message.as_ref() == "materialization"));
    assert!(materialization.observations().observations().is_empty());
    let resolution =
        repository_source_resolution_compute_error(path.dupe(), "resolution".into(), prefix.dupe());
    let resolution = complete_repository_source(&resolution);
    assert!(
        matches!(resolution.result().as_ref(), Err(RepositorySourceFileError::ResolutionCompute { message, .. }) if message.as_ref() == "resolution")
    );
    assert_exact_epoch(&prefix, resolution.observations());
    let SourcePreparationOutcome::Complete(Ok((
        Err(RepositorySourceFileError::FileCompute { message, .. }),
        file_prefix,
    ))) = host_repository_source_file_compute_error(path.dupe(), "file".into(), &prefix)
    else {
        panic!("FileBytes compute failure must retain its prior prefix")
    };
    assert_exact_epoch(&prefix, &file_prefix);
    assert_eq!(message.as_ref(), "file");
    let semantic = finish_observed_repository_source_materialization(
        SourcePreparationOutcome::Complete(Ok(ObservedRepositoryMaterialization {
            result: Arc::new(Err(RepositoryMaterializationError::Spec("spec".into()))),
            observations: prefix.dupe(),
        })),
        path.dupe(),
    );
    let ControlFlow::Break(semantic) = semantic else {
        panic!("materialization semantic must stop")
    };
    assert_exact_epoch(
        &prefix,
        complete_repository_source(&semantic).observations(),
    );
    let need = NeedPathObservations::singleton(demand.dupe());
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::Lstat,
        },
    );
    for value in [
        finish_observed_repository_source_materialization(
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need.dupe())),
            path.dupe(),
        )
        .break_value()
        .unwrap(),
        finish_observed_repository_source_resolution(PathOutcome::Need(need), &prefix, path.dupe())
            .break_value()
            .unwrap(),
        finish_observed_repository_source_materialization(
            SourcePreparationOutcome::Complete(Err(outer.dupe())),
            path.dupe(),
        )
        .break_value()
        .unwrap(),
        finish_observed_repository_source_resolution(
            PathOutcome::Complete(Err(outer)),
            &prefix,
            path,
        )
        .break_value()
        .unwrap(),
    ] {
        assert_eq!(
            RepositorySourceFileObservationKey::validity(&value),
            value.is_complete()
        );
        assert_eq!(
            RepositorySourceFileObservationKey::equality(&value, &value),
            value.is_complete()
        );
    }
}
#[tokio::test]
async fn observed_repository_source_prefixes_families_and_events_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let local = "module(name='root')\nprint('root-source')\nlocal_path_override(module_name='dep',path='dep')\n";
    let local_success =
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local);
    let present_epoch = host_path_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep/file",
        Some(PathNodeKind::RegularFile),
        Some(b"local-a"),
    );
    let (cold, warm, legacy, expected, phases, rows) =
        repository_source_case(&dice, local, 301, local_success.clone(), present_epoch).await;
    let observed = complete_repository_source(&cold);
    assert!(RepositorySourceFileObservationKey::equality(&cold, &warm));
    assert!(Arc::ptr_eq(
        observed.result(),
        complete_repository_source(&warm).result()
    ));
    assert_exact_epoch(&expected, observed.observations());
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy source must complete");
    };
    assert_eq!(observed.result().as_ref(), &legacy);
    let (
        Ok(RepositorySourceFileValue::Present(observed_bytes)),
        Ok(RepositorySourceFileValue::Present(legacy_bytes)),
    ) = (observed.result().as_ref(), &legacy)
    else {
        panic!("both source families must retain bytes");
    };
    assert!(Arc::ptr_eq(observed_bytes, legacy_bytes));
    for (root, expected) in [
        (
            "observed-repository-source-file:dep:file",
            [
                "observed-repository-materialization:dep",
                "observed-resolved-path:Host:\"/workspace/dep/file\"",
                "path-observation:Host:\"/workspace/dep/file\":FileBytes",
            ],
        ),
        (
            "repository-source-file:dep:file",
            [
                "repository-materialization:dep",
                "resolved-path:Host:\"/workspace/dep/file\"",
                "path-observation:Host:\"/workspace/dep/file\":FileBytes",
            ],
        ),
    ] {
        let deps = &rows.iter().find(|(name, _)| name == root).unwrap().1;
        assert_eq!(deps, &expected.map(str::to_owned));
    }
    assert!(phases[0].iter().any(|entry| {
        entry.key.contains("root-module-file:")
            && entry
                .batch
                .as_ref()
                .is_some_and(|batch| !batch.events().is_empty())
    }));
    assert_repository_source_eventless(&phases[1]);
    assert!(
        phases[2]
            .iter()
            .all(|entry| entry.kind == ActivationKind::Reused && entry.batch.is_none())
    );
    let upper = [
        "host-nonregistry-package-preflight:",
        "host-nonregistry-repo-file:",
        "host-nonregistry-repository-ignore:",
        "module-source-preparation:",
        "direct-local-module-preparation:",
        "observed-direct-local-module-preparation:",
        "host-nonregistry-module-closure:",
        "host-discovered-module:",
        "host-selected-module-graph:",
        "registry-file:",
    ];
    assert!(
        !phases
            .iter()
            .flatten()
            .any(|entry| upper.iter().any(|p| entry.key.starts_with(p)))
    );
    let path = |kind| {
        host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep/file",
            kind,
            None,
        )
    };
    let regular = path(Some(PathNodeKind::RegularFile));
    let bytes = |result| {
        source_epoch(
            regular.dupe(),
            PathObservationNamespace::Host,
            "/workspace/dep/file",
            PathObservationResult::FileBytes(result),
        )
    };
    let resolution_error = repository_source_readlink_error_epoch();
    let cases = [
        (path(None), "absent"),
        (resolution_error, "resolution"),
        (path(Some(PathNodeKind::Directory)), "wrong"),
        (bytes(PathOperationResult::Missing), "missing"),
        (
            bytes(PathOperationResult::Error(PathObservationError::NotALink)),
            "error",
        ),
    ];
    for (index, (tail, expected_kind)) in cases.into_iter().enumerate() {
        let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let (value, _, _, epoch, _, _) = repository_source_case(
            &case_dice,
            local,
            310 + index as i64,
            local_success.clone(),
            tail,
        )
        .await;
        let value = complete_repository_source(&value);
        assert_exact_epoch(&epoch, value.observations());
        assert!(match (expected_kind, value.result().as_ref()) {
            ("absent", Ok(RepositorySourceFileValue::Absent)) => true,
            (
                "wrong",
                Err(RepositorySourceFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                }),
            ) => true,
            (
                "resolution",
                Err(RepositorySourceFileError::Observation {
                    operation: PathObservationOperation::ReadLink,
                    ..
                }),
            ) => true,
            ("missing", Err(RepositorySourceFileError::InconsistentState { .. })) => true,
            ("error", Err(RepositorySourceFileError::Observation { .. })) => true,
            _ => false,
        });
    }

    let spec_case = repository_source_case(
        &dice,
        local,
        320,
        RepositoryMaterializationResult::SpecError("spec".into()),
        PathObservationEpoch::empty(),
    )
    .await;
    let spec = complete_repository_source(&spec_case.0);
    assert!(matches!(
        spec.result().as_ref(),
        Err(RepositorySourceFileError::Materialization { .. })
    ));
    assert_exact_epoch(&spec_case.3, spec.observations());
    let archive = "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n";
    let instance = PathObservationInstanceId::new(321);
    let namespace = PathObservationNamespace::Materialization(instance);
    let immutable =
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("sha256-immutable"),
            generation_root: PathBuf::from("/immutable/321"),
            observation_instance: instance,
        });
    let immutable_tail = host_path_epoch(
        namespace,
        "/immutable/321/file",
        Some(PathNodeKind::SpecialFile),
        Some(b"immutable"),
    );
    let (immutable_value, _, _, immutable_epoch, _, _) =
        repository_source_case(&dice, archive, 321, immutable, immutable_tail).await;
    let immutable_value = complete_repository_source(&immutable_value);
    assert_exact_epoch(&immutable_epoch, immutable_value.observations());
    assert!(matches!(
        immutable_value.result().as_ref(),
        Ok(RepositorySourceFileValue::Present(bytes))
            if bytes.as_ref() == b"immutable"
    ));
}

#[tokio::test]
async fn observed_repository_source_need_cancel_and_lifecycle_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let local = "module(name='root')\nlocal_path_override(module_name='dep',path='dep')\n";
    let key = RepositorySourceFileObservationKey(repository_source_key("file"));

    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let mut data = UserComputationData {
        activation_tracker: Some(tracker.dupe()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    inject_materialization_request_inputs(&mut updater, local, 401, &[], false);
    let mut transaction = updater.commit().await;
    let materialization_need = transaction.compute(&key).await.unwrap();
    assert!(matches!(
        materialization_need,
        SourcePreparationOutcome::Need(_)
    ));
    let deps = &tracker
        .take_rows()
        .into_iter()
        .find(|(name, _)| name == &key.to_string())
        .unwrap()
        .1;
    assert!(
        deps.iter()
            .any(|dep| dep.starts_with("observed-repository-materialization:"))
    );
    assert!(
        !deps
            .iter()
            .any(|dep| dep.starts_with("observed-resolved-path:"))
    );

    assert_repository_source_eventless(&tracker.take());
    let result = RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local);
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut resolution_need, _) = repository_source_transaction(
        &dice,
        local,
        402,
        result.clone(),
        PathObservationEpoch::empty(),
        tracker.dupe(),
    )
    .await;
    assert!(matches!(
        resolution_need.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    let deps = &tracker
        .take_rows()
        .into_iter()
        .find(|(name, _)| name == &key.to_string())
        .unwrap()
        .1;
    assert!(
        deps.iter()
            .any(|dep| dep.starts_with("observed-resolved-path:"))
    );
    assert!(!deps.iter().any(|dep| dep.contains("FileBytes")));

    assert_repository_source_eventless(&tracker.take());
    let lstat_only = host_path_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep/file",
        Some(PathNodeKind::RegularFile),
        None,
    );
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut file_need, _) = repository_source_transaction(
        &dice,
        local,
        403,
        result.clone(),
        lstat_only,
        tracker.dupe(),
    )
    .await;
    assert!(matches!(
        file_need.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));
    assert_repository_source_eventless(&tracker.take());

    let full = host_path_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep/file",
        Some(PathNodeKind::RegularFile),
        Some(b"a"),
    );
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut cancelled, _) =
        repository_source_transaction(&dice, local, 404, result.clone(), full, tracker.dupe())
            .await;
    tracker.take();
    tracker.take_rows();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().is_empty());

    let recovery_tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut recovered, _) = repository_source_transaction(
        &dice,
        local,
        404,
        result.clone(),
        host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep/file",
            Some(PathNodeKind::RegularFile),
            Some(b"a"),
        ),
        recovery_tracker.dupe(),
    )
    .await;
    recovery_tracker.take();
    let recovered = recovered.compute(&key).await.unwrap();
    assert!(RepositorySourceFileObservationKey::validity(&recovered));
    assert!(
        recovery_tracker
            .take()
            .iter()
            .all(|entry| { entry.key != key.to_string() || entry.batch.is_none() })
    );

    let epoch_for = |kind, bytes| {
        host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep/file",
            kind,
            bytes,
        )
    };
    let lifecycle = [
        (Some(PathNodeKind::RegularFile), Some(b"a".as_slice())),
        (Some(PathNodeKind::RegularFile), Some(b"b".as_slice())),
        (None, None),
        (Some(PathNodeKind::Directory), None),
        (Some(PathNodeKind::RegularFile), Some(b"a".as_slice())),
    ];
    let mut local_values = Vec::new();
    for (kind, bytes) in lifecycle {
        let case =
            repository_source_case(&dice, local, 405, result.clone(), epoch_for(kind, bytes)).await;
        local_values.push((case.0, case.3));
    }
    let a_value = complete_repository_source(&local_values[0].0);
    let held_result = a_value.result().dupe();
    let held_epoch = a_value.observations().dupe();
    assert!(local_values[1..4].iter().all(|value| {
        !RepositorySourceFileObservationKey::equality(&local_values[0].0, &value.0)
    }));
    let (restored_value, _) = local_values.pop().unwrap();
    assert!(RepositorySourceFileObservationKey::equality(
        &local_values[0].0,
        &restored_value
    ));
    let restored = complete_repository_source(&restored_value);
    assert_eq!(held_result.as_ref(), restored.result().as_ref());
    assert_eq!(held_epoch, *restored.observations());

    let archive = "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n";
    let instance = PathObservationInstanceId::new(406);
    let invalid_root = repository_source_case(
        &dice,
        archive,
        407,
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("sha256-invalid-root"),
            generation_root: PathBuf::from("relative"),
            observation_instance: instance,
        }),
        PathObservationEpoch::empty(),
    )
    .await;
    let invalid = complete_repository_source(&invalid_root.0);
    assert!(matches!(
        invalid.result().as_ref(),
        Err(RepositorySourceFileError::InvalidMaterializedPath { .. })
    ));
    assert_exact_epoch(&invalid_root.3, invalid.observations());

    let immutable =
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("sha256-lifecycle"),
            generation_root: PathBuf::from("/immutable/406"),
            observation_instance: instance,
        });
    let mut values = Vec::new();
    for (kind, bytes) in lifecycle {
        let case = repository_source_case(
            &dice,
            archive,
            406,
            immutable.clone(),
            host_path_epoch(
                PathObservationNamespace::Materialization(instance),
                "/immutable/406/file",
                kind,
                bytes,
            ),
        )
        .await;
        values.push((case.0, case.3));
    }
    let immutable_a = complete_repository_source(&values[0].0);
    let held_immutable_result = immutable_a.result().dupe();
    let held_immutable_epoch = immutable_a.observations().dupe();
    assert!(
        values[1..4]
            .iter()
            .all(|value| { !RepositorySourceFileObservationKey::equality(&values[0].0, &value.0) })
    );
    let (restored_value, _) = values.pop().unwrap();
    assert!(RepositorySourceFileObservationKey::equality(
        &values[0].0,
        &restored_value
    ));
    let immutable_restored = complete_repository_source(&restored_value);
    assert_eq!(
        held_immutable_result.as_ref(),
        immutable_restored.result().as_ref()
    );
    assert_eq!(held_immutable_epoch, *immutable_restored.observations());
}

fn direct_local_file_key(apparent: &str) -> DirectLocalModuleFileKey {
    DirectLocalModuleFileKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new(apparent).unwrap(),
    )
    .unwrap()
}

fn complete_direct_local_file(
    value: &<DirectLocalModuleFileObservationKey as Key>::Value,
) -> &ObservedDirectLocalModuleFile {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed direct-local file must complete")
    };
    observed
}

async fn direct_local_file_transaction(
    dice: &Arc<Dice>,
    local_path: &str,
    module: Option<&[u8]>,
    module_kind: Option<PathNodeKind>,
    variant: i64,
    tracker: Option<Arc<HostSourceFamilyTracker>>,
) -> (dice::DiceTransaction, PathObservationEpoch) {
    let root_source = format!(
        "print('ROOT')\nbazel_dep(name='dep',version='1')\nlocal_path_override(module_name='dep',path='{local_path}')\n"
    );
    let route_path = format!("/workspace/{local_path}");
    let epoch = horizon_epoch(
        &root_source,
        PathObservationNamespace::Host,
        &route_path,
        module,
        None,
        module_kind,
        None,
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        variant,
    );
    let mut data = UserComputationData {
        activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    updater
        .changed_to(vec![(
            slug_workspace_v2::WorkspaceSnapshotKey {
                workspace: PathBuf::from("/workspace"),
            },
            Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel"),
                    slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(root_source.clone())),
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            slug_workspace_v2::WorkspaceRawSnapshotKey {
                workspace: PathBuf::from("/workspace"),
            },
            Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel.lock"),
                    slug_workspace_v2::WorkspaceRawFileValue::Absent,
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            material(local_path),
        )])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        crate::LockfileMode::Update,
    )
    .unwrap();
    (updater.commit().await, epoch)
}

#[tokio::test]
async fn observed_direct_local_file_preserves_arcs_events_families_and_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let key = DirectLocalModuleFileObservationKey(direct_local_file_key("dep"));
    assert_eq!(
        key.to_string(),
        "observed-direct-local-module-file:\"/workspace\":@dep"
    );

    let (mut cold, epoch) =
        direct_local_file_transaction(&dice, "dep", Some(b"a"), None, 1, Some(tracker.dupe()))
            .await;
    let cold_value = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModuleFileObservationKey::validity(&cold_value));
    let cold_observed = complete_direct_local_file(&cold_value);
    assert!(matches!(
        cold_observed.result.as_ref(),
        Ok(DirectLocalModuleFile(_, HostRepositorySourceFileValue::Present { bytes, .. }))
            if bytes.as_ref() == b"a"
    ));
    for (demand, result) in cold_observed.observations.observations() {
        assert!(Arc::ptr_eq(epoch.get(demand).unwrap(), result));
        let PathOutcome::Complete(selected) = cold
            .compute(&PathObservationKey::new(demand.dupe()))
            .await
            .unwrap()
        else {
            panic!("retained direct-local demand must be selected")
        };
        assert!(Arc::ptr_eq(&selected, result));
    }

    let activations = tracker.take();
    let has = |prefix: &str| {
        activations
            .iter()
            .any(|entry| entry.key.starts_with(prefix))
    };
    assert!(has("observed-direct-local-module-file:"));
    assert!(has("observed-root-repository-route:"));
    assert!(has("observed-host-repository-source-file:"));
    assert!(!has("direct-local-module-file:"));
    assert!(!has("root-repository-route:"));
    assert!(!has("host-repository-source-file:"));
    for forbidden in [
        "direct-local-module-inspection:",
        "direct-local-include-package-horizon:",
        "direct-local-module-preparation:",
        "direct-local-module-evaluation:",
        "repository-package-source:",
        "repository-package-load:",
        "root-query",
    ] {
        assert!(!has(forbidden), "unexpected activation {forbidden}");
    }
    let event_owners = activations
        .iter()
        .filter(|entry| entry.kind == ActivationKind::Evaluated && entry.batch.is_some())
        .map(|entry| entry.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(event_owners.len(), 1);
    assert!(event_owners[0].starts_with("bzlmod-observed-host-root-module-file:"));

    let warm = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModuleFileObservationKey::equality(
        &cold_value,
        &warm
    ));
    assert!(Arc::ptr_eq(
        &cold_observed.result,
        &complete_direct_local_file(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));

    let legacy = cold.compute(&direct_local_file_key("dep")).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy direct-local file must complete")
    };
    assert_eq!(legacy.as_ref(), cold_observed.result.as_ref());
    assert!(
        !tracker
            .take()
            .iter()
            .any(|entry| entry.key.contains("observed-"))
    );

    for (path, module, expected, variant) in [
        ("other", Some(&b"b"[..]), Some(&b"b"[..]), 2),
        ("other", None, None, 3),
        ("dep", Some(&b"a"[..]), Some(&b"a"[..]), 4),
    ] {
        let (mut transaction, _) =
            direct_local_file_transaction(&dice, path, module, None, variant, None).await;
        let value = transaction.compute(&key).await.unwrap();
        let result = complete_direct_local_file(&value).result.as_ref();
        let actual = match result.as_ref().unwrap() {
            DirectLocalModuleFile(_, HostRepositorySourceFileValue::Present { bytes, .. }) => {
                Some(bytes.as_ref())
            }
            DirectLocalModuleFile(_, HostRepositorySourceFileValue::Absent) => None,
        };
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn observed_direct_local_file_covers_needs_prefixes_suppression_and_cancellation() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = DirectLocalModuleFileObservationKey(direct_local_file_key("dep"));

    let (mut route_need, _) =
        direct_local_file_transaction(&dice, "dep", Some(b"a"), None, 10, None).await;
    let mut updater = route_need.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    route_need = updater.commit().await;
    let route_need = route_need.compute(&key).await.unwrap();
    assert!(matches!(route_need, SourcePreparationOutcome::Need(_)));
    assert!(!DirectLocalModuleFileObservationKey::validity(&route_need));
    assert!(!DirectLocalModuleFileObservationKey::equality(
        &route_need,
        &route_need
    ));

    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let missing = DirectLocalModuleFileObservationKey(direct_local_file_key("missing"));
    let (mut route_error, _) =
        direct_local_file_transaction(&dice, "dep", Some(b"a"), None, 11, Some(tracker.dupe()))
            .await;
    let route_error = route_error.compute(&missing).await.unwrap();
    let route_error = complete_direct_local_file(&route_error);
    assert!(matches!(
        route_error.result.as_ref(),
        Err(DirectLocalModuleFileError::Route(_))
    ));
    assert!(!route_error.observations.observations().is_empty());
    assert!(!tracker.take().iter().any(|entry| {
        entry
            .key
            .starts_with("observed-host-repository-source-file:")
    }));

    let (mut source_need, _) = direct_local_file_transaction(
        &dice,
        "dep",
        None,
        Some(PathNodeKind::RegularFile),
        12,
        None,
    )
    .await;
    assert!(matches!(
        source_need.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Need(_)
    ));

    let (mut wrong_kind, _) =
        direct_local_file_transaction(&dice, "dep", None, Some(PathNodeKind::Directory), 13, None)
            .await;
    let wrong_kind = wrong_kind.compute(&key).await.unwrap();
    let wrong_kind = complete_direct_local_file(&wrong_kind);
    assert!(matches!(
        wrong_kind.result.as_ref(),
        Err(DirectLocalModuleFileError::Source(
            RepositorySourceFileError::WrongKind { .. }
        ))
    ));
    assert!(
        wrong_kind.observations.observations().len()
            > route_error.observations.observations().len()
    );

    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut cancelled, _) =
        direct_local_file_transaction(&dice, "dep", Some(b"c"), None, 14, Some(tracker.dupe()))
            .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().is_empty());
    let (mut recovered, _) =
        direct_local_file_transaction(&dice, "dep", Some(b"c"), None, 14, Some(tracker.dupe()))
            .await;
    assert!(matches!(
        complete_direct_local_file(&recovered.compute(&key).await.unwrap())
            .result
            .as_ref(),
        Ok(DirectLocalModuleFile(_, HostRepositorySourceFileValue::Present { bytes, .. }))
            if bytes.as_ref() == b"c"
    ));
}

#[test]
fn observed_direct_local_file_union_and_complete_algebra_are_fail_closed() {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let first = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"same".as_slice())),
    ));
    let equal = Arc::new(first.as_ref().clone());
    let left = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
    let right = PathObservationEpoch::from_shared([(demand.dupe(), equal)]).unwrap();
    let merged = merge_path_observations(&left, &right).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
    let conflict = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(b"different".as_slice())),
        )),
    )])
    .unwrap();
    assert!(matches!(
        merge_path_observations(&left, &conflict),
        Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found)
        )) if found == demand
    ));

    let route_compute = direct_local_file_complete(
        Err(DirectLocalModuleFileError::RouteCompute("route".into())),
        PathObservationEpoch::empty(),
    );
    let source_compute = direct_local_file_complete(
        Err(DirectLocalModuleFileError::SourceCompute("source".into())),
        left.dupe(),
    );
    assert!(
        complete_direct_local_file(&route_compute)
            .observations
            .observations()
            .is_empty()
    );
    assert!(Arc::ptr_eq(
        complete_direct_local_file(&source_compute)
            .observations
            .get(&demand)
            .unwrap(),
        &first
    ));
    for value in [&route_compute, &source_compute] {
        assert!(DirectLocalModuleFileObservationKey::validity(value));
        assert!(DirectLocalModuleFileObservationKey::equality(value, value));
    }
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: demand.dupe(),
            result_operation: PathObservationOperation::Lstat,
        },
    );
    for child in [
        direct_local_observed_child::<()>(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
        direct_local_observed_child::<()>(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
    ] {
        let ControlFlow::Break(value) = child else {
            panic!("typed outer must suppress its next child")
        };
        assert!(
            matches!(&value, SourcePreparationOutcome::Complete(Err(error)) if error == &outer)
        );
        assert!(DirectLocalModuleFileObservationKey::validity(&value));
        assert!(DirectLocalModuleFileObservationKey::equality(
            &value, &value
        ));
    }
}

fn direct_local_inspection_key(apparent: &str) -> DirectLocalModuleInspectionKey {
    DirectLocalModuleInspectionKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new(apparent).unwrap(),
    )
    .unwrap()
}

fn complete_observed_direct_local_inspection(
    value: &<DirectLocalModuleInspectionObservationKey as Key>::Value,
) -> &ObservedDirectLocalModuleInspection {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed direct-local inspection must complete")
    };
    observed
}

#[tokio::test]
async fn observed_direct_local_inspection_preserves_arcs_events_families_and_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let legacy_key = direct_local_inspection_key("dep");
    let key = DirectLocalModuleInspectionObservationKey(legacy_key.clone());
    assert_eq!(
        key.to_string(),
        "observed-direct-local-module-inspection:\"/workspace\":@dep"
    );
    let module = b"include(\"//p:a.MODULE.bazel\")\n";
    let (mut cold, _) =
        direct_local_file_transaction(&dice, "dep", Some(module), None, 31, Some(tracker.dupe()))
            .await;
    let cold_value = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModuleInspectionObservationKey::validity(
        &cold_value
    ));
    let cold_observed = complete_observed_direct_local_inspection(&cold_value);
    assert!(matches!(
        cold_observed.result.as_ref(),
        Ok(DirectLocalModuleInspection(_, Some(inspection)))
            if inspection.includes.len() == 1
                && inspection.includes[0].path.as_str() == "//p:a.MODULE.bazel"
    ));

    let child_value = cold
        .compute(&DirectLocalModuleFileObservationKey(direct_local_file_key(
            "dep",
        )))
        .await
        .unwrap();
    let child = complete_direct_local_file(&child_value);
    assert_eq!(cold_observed.observations, child.observations);
    for (demand, result) in cold_observed.observations.observations() {
        assert!(Arc::ptr_eq(child.observations.get(demand).unwrap(), result));
    }
    let activations = tracker.take();
    let has = |prefix: &str| {
        activations
            .iter()
            .any(|entry| entry.key.starts_with(prefix))
    };
    for expected in [
        "observed-direct-local-module-inspection:",
        "observed-direct-local-module-file:",
        "observed-root-repository-route:",
        "observed-host-repository-source-file:",
    ] {
        assert!(has(expected), "missing activation {expected}");
    }
    for forbidden in [
        "direct-local-module-inspection:",
        "direct-local-module-file:",
        "direct-local-include-package-horizon:",
        "direct-local-module-preparation:",
        "direct-local-module-evaluation:",
        "repository-package-source:",
        "repository-package-load:",
        "root-query",
    ] {
        assert!(!has(forbidden), "unexpected activation {forbidden}");
    }
    let owners = activations
        .iter()
        .filter(|entry| entry.kind == ActivationKind::Evaluated && entry.batch.is_some())
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 1);
    assert!(
        owners[0]
            .key
            .starts_with("bzlmod-observed-host-root-module-file:")
    );

    let warm = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModuleInspectionObservationKey::equality(
        &cold_value,
        &warm
    ));
    assert!(Arc::ptr_eq(
        &cold_observed.result,
        &complete_observed_direct_local_inspection(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    let legacy = cold.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy direct-local inspection must complete")
    };
    assert_eq!(legacy.as_ref(), cold_observed.result.as_ref());
    assert!(
        !tracker
            .take()
            .iter()
            .any(|entry| entry.key.contains("observed-"))
    );

    for (bytes, expected, variant) in [
        (
            Some(b"include(\"//q:b.MODULE.bazel\")\n".as_slice()),
            Some("//q:b.MODULE.bazel"),
            32,
        ),
        (None, None, 33),
        (Some(module.as_slice()), Some("//p:a.MODULE.bazel"), 34),
    ] {
        let (mut transaction, _) =
            direct_local_file_transaction(&dice, "dep", bytes, None, variant, None).await;
        let value = transaction.compute(&key).await.unwrap();
        let actual = match complete_observed_direct_local_inspection(&value)
            .result
            .as_ref()
        {
            Ok(DirectLocalModuleInspection(_, inspection)) => inspection
                .as_ref()
                .and_then(|inspection| inspection.includes.first())
                .map(|request| request.path.as_str()),
            Err(error) => panic!("unexpected inspection error: {error}"),
        };
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn observed_direct_local_inspection_covers_terminals_projection_and_cancellation() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = DirectLocalModuleInspectionObservationKey(direct_local_inspection_key("dep"));
    let (route_need, _) =
        direct_local_file_transaction(&dice, "dep", Some(b""), None, 40, None).await;
    let mut updater = route_need.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut route_need = updater.commit().await;
    let route_need = route_need.compute(&key).await.unwrap();
    assert!(matches!(route_need, SourcePreparationOutcome::Need(_)));
    assert!(!DirectLocalModuleInspectionObservationKey::validity(
        &route_need
    ));
    assert!(!DirectLocalModuleInspectionObservationKey::equality(
        &route_need,
        &route_need
    ));

    let (mut semantic_transaction, _) =
        direct_local_file_transaction(&dice, "dep", Some(b""), None, 41, None).await;
    let missing = DirectLocalModuleInspectionObservationKey(direct_local_inspection_key("missing"));
    let semantic_value = semantic_transaction.compute(&missing).await.unwrap();
    let semantic = complete_observed_direct_local_inspection(&semantic_value);
    assert!(matches!(
        semantic.result.as_ref(),
        Err(DirectLocalModuleInspectionError::Input(_))
    ));
    assert!(!semantic.observations.observations().is_empty());
    let semantic_child = semantic_transaction
        .compute(&DirectLocalModuleFileObservationKey(direct_local_file_key(
            "missing",
        )))
        .await
        .unwrap();
    assert_eq!(
        semantic.observations,
        complete_direct_local_file(&semantic_child).observations
    );

    let (mut invalid, _) =
        direct_local_file_transaction(&dice, "dep", Some(b"include(\n"), None, 42, None).await;
    let invalid_value = invalid.compute(&key).await.unwrap();
    let invalid_observed = complete_observed_direct_local_inspection(&invalid_value);
    assert!(matches!(
        invalid_observed.result.as_ref(),
        Err(DirectLocalModuleInspectionError::Inspection(_, _))
    ));
    let invalid_child = invalid
        .compute(&DirectLocalModuleFileObservationKey(direct_local_file_key(
            "dep",
        )))
        .await
        .unwrap();
    assert_eq!(
        invalid_observed.observations,
        complete_direct_local_file(&invalid_child).observations
    );

    let semantic_arc = Arc::new(Err(DirectLocalModuleInspectionError::InputCompute(
        "held".into(),
    )));
    let carrier = SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleInspection {
        result: semantic_arc.dupe(),
        observations: PathObservationEpoch::empty(),
    }));
    assert!(
        complete_observed_direct_local_inspection(&carrier)
            .observations
            .observations()
            .is_empty()
    );
    let SourcePreparationOutcome::Complete(projected) =
        project_legacy_direct_local_inspection(carrier)
    else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&semantic_arc, &projected));

    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::Lstat,
        },
    );
    let ControlFlow::Break(outer_value) = direct_local_inspection_observed_child(
        SourcePreparationOutcome::Complete(Err(outer.dupe())),
    ) else {
        panic!("file typed outer must suppress inspection")
    };
    assert!(
        matches!(&outer_value, SourcePreparationOutcome::Complete(Err(error)) if error == &outer)
    );
    assert!(DirectLocalModuleInspectionObservationKey::validity(
        &outer_value
    ));
    assert!(DirectLocalModuleInspectionObservationKey::equality(
        &outer_value,
        &outer_value
    ));

    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut cancelled, _) = direct_local_file_transaction(
        &dice,
        "dep",
        Some(b"include(\"//p:a.MODULE.bazel\")\n"),
        None,
        43,
        Some(tracker.dupe()),
    )
    .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().is_empty());
    let (mut recovered, _) = direct_local_file_transaction(
        &dice,
        "dep",
        Some(b"include(\"//p:a.MODULE.bazel\")\n"),
        None,
        43,
        Some(tracker.dupe()),
    )
    .await;
    assert!(matches!(
        complete_observed_direct_local_inspection(&recovered.compute(&key).await.unwrap())
            .result
            .as_ref(),
        Ok(DirectLocalModuleInspection(_, Some(_)))
    ));
}

fn direct_local_horizon_key(apparent: &str) -> DirectLocalIncludePackageHorizonKey {
    DirectLocalIncludePackageHorizonKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new(apparent).unwrap(),
    )
    .unwrap()
}

fn complete_observed_direct_local_horizon(
    value: &<DirectLocalIncludePackageHorizonObservationKey as Key>::Value,
) -> &ObservedDirectLocalIncludePackageHorizon {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed direct-local horizon must complete")
    };
    observed
}

async fn direct_local_horizon_transaction(
    dice: &Arc<Dice>,
    module: &[u8],
    packages: &[(&str, bool)],
    variant: i64,
    tracker: Option<Arc<HostSourceFamilyTracker>>,
) -> (dice::DiceTransaction, PathObservationEpoch) {
    let (transaction, _) =
        direct_local_file_transaction(dice, "dep", Some(module), None, variant, tracker).await;
    let root_source = "print('ROOT')\nbazel_dep(name='dep',version='1')\nlocal_path_override(module_name='dep',path='dep')\n";
    let epoch = horizon_epoch(
        root_source,
        PathObservationNamespace::Host,
        "/workspace/dep",
        Some(module),
        Some(b"print('REPO')\n"),
        None,
        None,
        packages,
        &[],
        &[],
        &[],
        &[],
        &[],
        variant,
    );
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            [NormalizedAbsolutePath::new("/workspace").unwrap()],
            std::iter::empty::<&str>(),
            None,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    (updater.commit().await, epoch)
}

fn observed_horizon_epoch(
    name: &str,
) -> (
    PathObservationDemand,
    Arc<PathObservationResult>,
    PathObservationEpoch,
) {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(format!("/workspace/dep/{name}/BUILD.bazel")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
    let epoch = PathObservationEpoch::from_shared([(demand.dupe(), result.dupe())]).unwrap();
    (demand, result, epoch)
}

fn observed_horizon_lookup(
    value: ExternalRepositoryPackageLookup,
    observations: PathObservationEpoch,
) -> DirectLocalIncludePackageLookupOutcome {
    Ok(SourcePreparationOutcome::Complete(Ok((
        Arc::new(Ok(value)),
        observations,
    ))))
}

#[test]
fn observed_horizon_reducer_is_prefix_bounded_left_first_and_complete_only() {
    let p = horizon_occurrence("p", 1);
    let q = horizon_occurrence("q", 2);
    let r = horizon_occurrence("r", 3);
    let packages = vec![p.package.clone(), q.package.clone(), r.package.clone()];
    let (p_demand, p_result, p_epoch) = observed_horizon_epoch("p");
    let (_, _, q_epoch) = observed_horizon_epoch("q");
    let path_need =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/dep/p/pending").unwrap(),
            PathObservationOperation::Lstat,
        )));
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/dep/r/BUILD.bazel").unwrap(),
                PathObservationOperation::Lstat,
            ),
            result_operation: PathObservationOperation::FileBytes,
        },
    );
    let reduce = |outcomes, initial| {
        finish_direct_local_include_package_horizon_observed(
            local_route(),
            vec![p.clone(), q.clone(), r.clone()],
            &packages,
            outcomes,
            initial,
        )
    };
    let success = || {
        observed_horizon_lookup(
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            PathObservationEpoch::empty(),
        )
    };
    let batch = |p_value, q_value, r_value| {
        SmallMap::from_iter([
            (p.package.clone(), p_value),
            (q.package.clone(), q_value),
            (r.package.clone(), r_value),
        ])
    };
    let outcomes = batch(
        observed_horizon_lookup(ExternalRepositoryPackageLookup::NoBuildFile, p_epoch.dupe()),
        Ok(SourcePreparationOutcome::Need(path_need.dupe())),
        Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
    );
    let first_semantic = reduce(outcomes, PathObservationEpoch::empty());
    let observed = complete_observed_direct_local_horizon(&first_semantic);
    assert!(matches!(
        observed.result.as_ref(),
        Err(DirectLocalIncludePackageHorizonError::Package {
            raw_label,
            failure: DirectLocalIncludePackageFailure::NoBuildFile,
            ..
        }) if raw_label == &p.raw_label
    ));
    assert_eq!(observed.observations.observations().len(), 1);
    assert!(Arc::ptr_eq(
        observed.observations.get(&p_demand).unwrap(),
        &p_result
    ));
    let first_of_two = reduce(
        batch(
            observed_horizon_lookup(ExternalRepositoryPackageLookup::NoBuildFile, p_epoch.dupe()),
            observed_horizon_lookup(
                ExternalRepositoryPackageLookup::Deleted,
                PathObservationEpoch::empty(),
            ),
            success(),
        ),
        PathObservationEpoch::empty(),
    );
    assert_eq!(
        complete_observed_direct_local_horizon(&first_of_two)
            .result
            .as_ref(),
        observed.result.as_ref()
    );
    let outcomes = batch(
        Ok(SourcePreparationOutcome::Need(path_need.dupe())),
        observed_horizon_lookup(ExternalRepositoryPackageLookup::Deleted, q_epoch),
        Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
    );
    assert!(matches!(
        reduce(outcomes, PathObservationEpoch::empty()),
        SourcePreparationOutcome::Need(need) if need == path_need
    ));
    let outcomes = batch(
        Ok(SourcePreparationOutcome::Need(path_need.dupe())),
        Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
        success(),
    );
    let no_semantic_outer = reduce(outcomes, PathObservationEpoch::empty());
    assert!(matches!(
        &no_semantic_outer,
        SourcePreparationOutcome::Complete(Err(error)) if error == &outer
    ));
    assert!(
        DirectLocalIncludePackageHorizonObservationKey::validity(&no_semantic_outer)
            && DirectLocalIncludePackageHorizonObservationKey::equality(
                &no_semantic_outer,
                &no_semantic_outer
            )
    );
    let equal = Arc::new(p_result.as_ref().clone());
    let equal_epoch = PathObservationEpoch::from_shared([(p_demand.dupe(), equal)]).unwrap();
    let successes = batch(
        observed_horizon_lookup(
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            equal_epoch,
        ),
        success(),
        success(),
    );
    let stable = reduce(successes, p_epoch.dupe());
    assert!(Arc::ptr_eq(
        complete_observed_direct_local_horizon(&stable)
            .observations
            .get(&p_demand)
            .unwrap(),
        &p_result
    ));
    let different = Arc::new(PathObservationResult::Lstat(PathOperationResult::Error(
        PathObservationError::NotALink,
    )));
    let conflict_epoch = PathObservationEpoch::from_shared([(p_demand.dupe(), different)]).unwrap();
    let conflict = batch(
        observed_horizon_lookup(
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
            conflict_epoch,
        ),
        success(),
        success(),
    );
    assert!(matches!(
        reduce(conflict, p_epoch),
        SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found)
        ))) if found == p_demand
    ));
    let bootstrap_need =
        SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        });
    let needs = batch(
        Ok(SourcePreparationOutcome::Need(path_need)),
        Ok(SourcePreparationOutcome::Need(bootstrap_need)),
        success(),
    );
    let SourcePreparationOutcome::Need(union) = reduce(needs, PathObservationEpoch::empty()) else {
        panic!("full batch Needs must be unioned")
    };
    assert!(union.path_observations().is_some());
    assert!(union.root_module_bootstrap_request().is_some());
    let need = SourcePreparationOutcome::Need(union);
    assert!(
        !DirectLocalIncludePackageHorizonObservationKey::validity(&need)
            && !DirectLocalIncludePackageHorizonObservationKey::equality(&need, &need)
    );
}
#[tokio::test]
async fn observed_horizon_preserves_exact_children_events_families_and_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let module = b"include(\"//p:first.MODULE.bazel\")\ninclude(\"//p:second.MODULE.bazel\")\ninclude(\"//q:third.MODULE.bazel\")\n";
    let key = DirectLocalIncludePackageHorizonObservationKey(direct_local_horizon_key("dep"));
    assert_eq!(
        key.to_string(),
        "observed-direct-local-include-package-horizon:\"/workspace\":@dep"
    );
    let (mut cold, _) = direct_local_horizon_transaction(
        &dice,
        module,
        &[("p", true), ("q", true)],
        51,
        Some(tracker.dupe()),
    )
    .await;
    let cold_value = cold.compute(&key).await.unwrap();
    let observed = complete_observed_direct_local_horizon(&cold_value);
    let horizon = observed.result.as_ref().as_ref().unwrap();
    assert_eq!(
        horizon
            .occurrences
            .iter()
            .map(|occurrence| occurrence.package.package().as_str())
            .collect::<Vec<_>>(),
        ["p", "p", "q"]
    );
    let inspection = cold
        .compute(&DirectLocalModuleInspectionObservationKey(
            direct_local_inspection_key("dep"),
        ))
        .await
        .unwrap();
    let mut expected = complete_observed_direct_local_inspection(&inspection)
        .observations
        .dupe();
    for package in ["p", "q"] {
        let package = PackageIdentifier::new(
            local_route().canonical_repo().clone(),
            PackagePath::parse(package).unwrap(),
        );
        let lookup = cold
            .compute(
                &ExternalRepositoryPackageLookupObservationKey::new(local_route(), package)
                    .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(lookup)) = lookup else {
            panic!("observed package lookup must complete")
        };
        expected = merge_path_observations(&expected, lookup.observations()).unwrap();
    }
    assert_exact_epoch(&expected, &observed.observations);
    let activations = tracker.take();
    let has = |prefix: &str| {
        activations
            .iter()
            .any(|entry| entry.key.starts_with(prefix))
    };
    for prefix in [
        "observed-direct-local-include-package-horizon:",
        "observed-direct-local-module-inspection:",
        "observed-external-repository-package-lookup:",
    ] {
        assert!(has(prefix), "missing {prefix}");
    }
    for prefix in [
        "direct-local-include-package-horizon:",
        "direct-local-module-inspection:",
        "external-repository-package-lookup:",
        "direct-local-module-preparation:",
        "direct-local-module-evaluation:",
        "repository-package-source:",
        "repository-package-load:",
        "root-query",
    ] {
        assert!(!has(prefix), "unexpected {prefix}");
    }
    let event_owners = activations
        .iter()
        .filter(|entry| entry.batch.is_some())
        .map(|entry| (entry.key.as_str(), entry.batch.as_ref().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(event_owners.len(), 3, "{event_owners:?}");
    assert!(
        event_owners[0]
            .0
            .starts_with("bzlmod-observed-host-root-module-file:")
    );
    assert!(
        event_owners[1..]
            .iter()
            .all(|(key, _)| key.starts_with("observed-host-route-repo-file:"))
    );
    assert!(
        matches!(event_owners[0].1.events(), [EvaluationEvent::StarlarkPrint { text, .. }] if text == "ROOT")
    );
    assert!(event_owners[1..].iter().all(|(_, batch)| matches!(batch.events(), [EvaluationEvent::StarlarkPrint { text, .. }] if text == "REPO")));
    let warm = cold.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        &observed.result,
        &complete_observed_direct_local_horizon(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    let legacy = cold
        .compute(&direct_local_horizon_key("dep"))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy horizon must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result.as_ref());
    assert!(
        !tracker
            .take()
            .iter()
            .any(|entry| entry.key.contains("observed-"))
    );
    let (mut changed, _) =
        direct_local_horizon_transaction(&dice, module, &[("p", true), ("q", false)], 52, None)
            .await;
    assert!(matches!(
        complete_observed_direct_local_horizon(&changed.compute(&key).await.unwrap())
            .result
            .as_ref(),
        Err(DirectLocalIncludePackageHorizonError::Package {
            failure: DirectLocalIncludePackageFailure::NoBuildFile,
            ..
        })
    ));
    let (mut restored, _) =
        direct_local_horizon_transaction(&dice, module, &[("p", true), ("q", true)], 53, None)
            .await;
    assert_eq!(
        observed.result.as_ref(),
        complete_observed_direct_local_horizon(&restored.compute(&key).await.unwrap())
            .result
            .as_ref()
    );

    let cancelled_tracker = Arc::new(HostSourceFamilyTracker::default());
    let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let (mut cancelled, _) = direct_local_horizon_transaction(
        &cancelled_dice,
        module,
        &[("p", true), ("q", true)],
        54,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(cancelled_tracker.take().is_empty());
    let (mut recovered, _) = direct_local_horizon_transaction(
        &cancelled_dice,
        module,
        &[("p", true), ("q", true)],
        54,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    assert!(
        complete_observed_direct_local_horizon(&recovered.compute(&key).await.unwrap())
            .result
            .is_ok()
    );
}

fn direct_local_preparation_key(apparent: &str) -> DirectLocalModulePreparationKey {
    DirectLocalModulePreparationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new(apparent).unwrap(),
    )
    .unwrap()
}

fn complete_observed_direct_local_preparation(
    value: &<DirectLocalModulePreparationObservationKey as Key>::Value,
) -> &ObservedDirectLocalModulePreparation {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed direct-local preparation must complete")
    };
    observed
}

async fn direct_local_preparation_transaction(
    dice: &Arc<Dice>,
    module: &[u8],
    fragments: &[(&str, Option<&[u8]>)],
    variant: i64,
    tracker: Option<Arc<HostSourceFamilyTracker>>,
) -> (dice::DiceTransaction, PathObservationEpoch) {
    let (transaction, _) =
        direct_local_file_transaction(dice, "dep", Some(module), None, variant, tracker).await;
    let epoch = horizon_epoch(
        "print('ROOT')\nbazel_dep(name='dep',version='1')\nlocal_path_override(module_name='dep',path='dep')\n",
        PathObservationNamespace::Host,
        "/workspace/dep",
        Some(module),
        Some(b"print('REPO')\n"),
        None,
        None,
        &[("p", true), ("q", true)],
        &[],
        &[],
        fragments,
        &[],
        &[],
        variant,
    );
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            [NormalizedAbsolutePath::new("/workspace").unwrap()],
            std::iter::empty::<&str>(),
            None,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    (updater.commit().await, epoch)
}

#[tokio::test]
async fn observed_preparation_preserves_recursive_arcs_families_events_and_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let module = b"include(\"//p:a.MODULE.bazel\")\n";
    let p = b"include(\"//q:b.MODULE.bazel\")\n";
    let q = b"bazel_dep(name='leaf',version='1')\n";
    let fragments = [
        ("p/a.MODULE.bazel", Some(p.as_slice())),
        ("q/b.MODULE.bazel", Some(q.as_slice())),
    ];
    let legacy_key = direct_local_preparation_key("dep");
    let key = DirectLocalModulePreparationObservationKey(legacy_key.clone());
    assert_eq!(
        key.to_string(),
        "observed-direct-local-module-preparation:\"/workspace\":@dep"
    );
    let (mut cold, _) =
        direct_local_preparation_transaction(&dice, module, &fragments, 61, Some(tracker.dupe()))
            .await;
    let cold_value = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModulePreparationObservationKey::validity(
        &cold_value
    ));
    let observed = complete_observed_direct_local_preparation(&cold_value);
    assert!(matches!(
        observed.result.as_ref(),
        Ok(DirectLocalModulePreparation::Supported(DirectLocalModuleClosure {
            fragments,
            ..
        })) if fragments.len() == 2
    ));
    let activations = tracker.take();
    let inspection = cold
        .compute(&DirectLocalModuleInspectionObservationKey(
            direct_local_inspection_key("dep"),
        ))
        .await
        .unwrap();
    let mut expected = complete_observed_direct_local_inspection(&inspection)
        .observations
        .dupe();
    for (package, fragment) in [("p", "p/a.MODULE.bazel"), ("q", "q/b.MODULE.bazel")] {
        let package = PackageIdentifier::new(
            local_route().canonical_repo().clone(),
            PackagePath::parse(package).unwrap(),
        );
        let lookup = cold
            .compute(
                &ExternalRepositoryPackageLookupObservationKey::new(local_route(), package)
                    .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(lookup)) = lookup else {
            panic!("observed package lookup must complete")
        };
        expected = merge_path_observations(&expected, lookup.observations()).unwrap();
        let source = cold
            .compute(&HostRepositorySourceFileObservationKey::new(
                local_route(),
                PathBuf::from(fragment),
            ))
            .await
            .unwrap();
        expected =
            merge_path_observations(&expected, complete_observed_source(&source).observations())
                .unwrap();
    }
    assert_exact_epoch(&expected, &observed.observations);
    tracker.take();
    let has = |prefix: &str| {
        activations
            .iter()
            .any(|entry| entry.key.starts_with(prefix))
    };
    for expected in [
        "observed-direct-local-module-preparation:",
        "observed-direct-local-module-inspection:",
        "observed-host-repository-source-file:",
        "observed-external-repository-package-lookup:",
    ] {
        assert!(has(expected), "missing {expected}");
    }
    for forbidden in [
        "direct-local-module-preparation:",
        "direct-local-module-inspection:",
        "host-repository-source-file:",
        "external-repository-package-lookup:",
        "direct-local-module-evaluation:",
        "repository-package-source:",
        "repository-package-load:",
        "root-query",
    ] {
        assert!(!has(forbidden), "unexpected {forbidden}");
    }
    assert_eq!(
        activations
            .iter()
            .filter(|entry| entry
                .key
                .starts_with("observed-external-repository-package-lookup:")
                && entry.kind == ActivationKind::Evaluated)
            .count(),
        2
    );
    assert!(!has("observed-direct-local-include-package-horizon:"));
    let event_owners = activations
        .iter()
        .filter(|entry| entry.batch.is_some())
        .collect::<Vec<_>>();
    assert_eq!(event_owners.len(), 2, "{event_owners:?}");
    assert!(
        event_owners[0]
            .key
            .starts_with("bzlmod-observed-host-root-module-file:")
    );
    assert!(
        matches!(event_owners[0].batch.as_ref().unwrap().events(), [EvaluationEvent::StarlarkPrint { text, .. }] if text == "ROOT")
    );
    assert!(
        event_owners[1..]
            .iter()
            .all(|entry| entry.key.starts_with("observed-host-route-repo-file:"))
    );
    assert!(event_owners[1..].iter().all(|entry| matches!(
        entry.batch.as_ref().unwrap().events(),
        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "REPO"
    )));

    let warm = cold.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        &observed.result,
        &complete_observed_direct_local_preparation(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    let legacy = cold.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy preparation must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result.as_ref());
    assert!(
        !tracker
            .take()
            .iter()
            .any(|entry| entry.key.contains("observed-"))
    );

    let changed_fragments = [
        ("p/a.MODULE.bazel", Some(p.as_slice())),
        ("q/b.MODULE.bazel", None),
    ];
    let (mut changed, _) =
        direct_local_preparation_transaction(&dice, module, &changed_fragments, 62, None).await;
    assert!(matches!(
        complete_observed_direct_local_preparation(&changed.compute(&key).await.unwrap())
            .result
            .as_ref(),
        Err(DirectLocalModulePreparationError::Fragment {
            failure: DirectLocalIncludeFragmentFailure::Absent,
            ..
        })
    ));
    let (mut restored, _) =
        direct_local_preparation_transaction(&dice, module, &fragments, 63, None).await;
    assert_eq!(
        observed.result.as_ref(),
        complete_observed_direct_local_preparation(&restored.compute(&key).await.unwrap())
            .result
            .as_ref()
    );

    let cycle_fragments = [
        ("p/a.MODULE.bazel", Some(p.as_slice())),
        (
            "q/b.MODULE.bazel",
            Some(b"include(\"//p:a.MODULE.bazel\")\n".as_slice()),
        ),
    ];
    let (mut cycle, _) =
        direct_local_preparation_transaction(&dice, module, &cycle_fragments, 64, None).await;
    assert!(matches!(
        complete_observed_direct_local_preparation(&cycle.compute(&key).await.unwrap())
            .result
            .as_ref(),
        Ok(DirectLocalModulePreparation::Unsupported(_))
    ));

    let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let cancelled_tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut cancelled, _) = direct_local_preparation_transaction(
        &cancelled_dice,
        module,
        &fragments,
        65,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(cancelled_tracker.take().is_empty());
    let (mut recovered, _) = direct_local_preparation_transaction(
        &cancelled_dice,
        module,
        &fragments,
        65,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    assert!(
        complete_observed_direct_local_preparation(&recovered.compute(&key).await.unwrap())
            .result
            .is_ok()
    );
}

#[test]
fn observed_preparation_fragment_reducer_is_prefix_bounded_at_every_slot() {
    let slots = [("p", 1), ("q", 2), ("r", 3)].map(|(package, line)| {
        let direct = horizon_occurrence(package, line);
        let occurrence = NonregistryIncludeOccurrence {
            package: direct.package.package().clone(),
            target: direct.target.clone(),
            raw_label: direct.raw_label.clone(),
            location: direct.location.clone(),
        };
        let entry = NonregistryIncludeFrontierEntry {
            request: NonrootIncludeRequest {
                path: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
            },
            ancestry: Arc::from([]),
        };
        let path = nonregistry_fragment_relative_path(&occurrence);
        (entry, occurrence, path)
    });
    let frontier = slots
        .iter()
        .map(|(entry, _, _)| entry.clone())
        .collect::<Vec<_>>();
    let occurrences = slots
        .iter()
        .map(|(_, occurrence, _)| occurrence.clone())
        .collect::<Vec<_>>();
    let paths = slots
        .iter()
        .map(|(_, _, path)| path.clone())
        .collect::<Vec<_>>();
    let (initial_demand, initial_result, initial) = observed_horizon_epoch("initial");
    let child = [
        observed_horizon_epoch("p"),
        observed_horizon_epoch("q"),
        observed_horizon_epoch("r"),
    ];
    let success = |slot: usize| {
        Ok(SourcePreparationOutcome::Complete(Ok((
            Ok(Some((
                Arc::from(b"".as_slice()),
                NormalizedAbsolutePath::new(format!(
                    "/workspace/dep/{}/nested.MODULE.bazel",
                    slots[slot].1.package
                ))
                .unwrap(),
            ))),
            child[slot].2.dupe(),
        ))))
    };
    let batch = |slot: usize, terminal: NonregistryFragmentSourceOutcome| {
        paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    path.clone(),
                    if index == slot {
                        terminal.clone()
                    } else {
                        success(index)
                    },
                )
            })
            .collect::<SmallMap<_, _>>()
    };
    let reduce = |outcomes, all_need| {
        finish_nonregistry_fragment_batch(
            &NonregistryPreparationOwner::Direct(local_route()),
            &frontier,
            &occurrences,
            &paths,
            &outcomes,
            all_need,
            initial.dupe(),
            &mut Vec::new(),
            &mut None,
        )
    };
    let semantic = |value| match value {
        ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
        _ => panic!("expected preparation semantic"),
    };
    for slot in 0..3 {
        let observed = semantic(reduce(batch(slot, Err("source compute".into())), None));
        assert!(matches!(
            observed.result,
            Err(NonregistryPreparationError::Direct(
                DirectLocalModulePreparationError::Fragment {
                    failure: DirectLocalIncludeFragmentFailure::SourceCompute { .. },
                    ..
                }
            ))
        ));
        assert_eq!(observed.observations.observations().len(), 1 + slot);

        let source_error = RepositorySourceFileError::Cycle {
            repo_relative_path: Arc::new(paths[slot].clone()),
        };
        let observed = semantic(reduce(
            batch(
                slot,
                Ok(SourcePreparationOutcome::Complete(Ok((
                    Err(source_error),
                    child[slot].2.dupe(),
                )))),
            ),
            None,
        ));
        assert!(matches!(
            observed.result,
            Err(NonregistryPreparationError::Direct(
                DirectLocalModulePreparationError::Fragment {
                    failure: DirectLocalIncludeFragmentFailure::Source(_),
                    ..
                }
            ))
        ));
        assert_eq!(observed.observations.observations().len(), 2 + slot);
        assert!(Arc::ptr_eq(
            observed.observations.get(&child[slot].0).unwrap(),
            &child[slot].1
        ));
    }
    assert!(Arc::ptr_eq(
        initial.get(&initial_demand).unwrap(),
        &initial_result
    ));

    let path_need =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/dep/p/pending").unwrap(),
            PathObservationOperation::Lstat,
        )));
    let bootstrap_need =
        SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        });
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: child[2].0.dupe(),
            result_operation: PathObservationOperation::FileBytes,
        },
    );
    let absent = |slot: usize| {
        Ok(SourcePreparationOutcome::Complete(Ok((
            Ok(None),
            child[slot].2.dupe(),
        ))))
    };
    let need = Ok(SourcePreparationOutcome::Need(path_need.dupe()));
    let typed_outer = Ok(SourcePreparationOutcome::Complete(Err(outer.dupe())));

    for slot in 0..3 {
        assert!(matches!(
            reduce(batch(slot, need.clone()), Some(path_need.dupe())),
            ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == path_need
        ));
        assert!(matches!(
            reduce(batch(slot, typed_outer.clone()), None),
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(found)))
                if found == NonregistryPreparationFrontierError::Path(outer.dupe())
        ));
        let validation = success(slot).map(|outcome| {
            outcome.map(|result| {
                result.map(|(_, epoch)| {
                    (
                        Ok(Some((
                            Arc::from(b"unknown_identifier\n".as_slice()),
                            NormalizedAbsolutePath::new(format!(
                                "/workspace/dep/{}/nested.MODULE.bazel",
                                slots[slot].1.package
                            ))
                            .unwrap(),
                        ))),
                        epoch,
                    )
                })
            })
        });
        let observed = semantic(reduce(batch(slot, validation), None));
        assert!(matches!(
            observed.result,
            Err(NonregistryPreparationError::Direct(
                DirectLocalModulePreparationError::Fragment {
                    failure: DirectLocalIncludeFragmentFailure::Validation { .. },
                    ..
                }
            ))
        ));
        assert_eq!(observed.observations.observations().len(), 2 + slot);

        let conflict_result = Arc::new(PathObservationResult::Lstat(PathOperationResult::Error(
            PathObservationError::NotALink,
        )));
        let conflict_epoch =
            PathObservationEpoch::from_shared([(initial_demand.dupe(), conflict_result)]).unwrap();
        let conflict = success(slot).map(|outcome| {
            outcome.map(|result| result.map(|(source, _)| (source, conflict_epoch)))
        });
        assert!(matches!(
            reduce(batch(slot, conflict), None),
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
                NonregistryPreparationFrontierError::Path(
                    ObservedPathFrontierError::Epoch(
                        slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found)
                    )
                )
            ))) if found == initial_demand
        ));
    }

    let mut semantic_then_later = batch(0, absent(0));
    semantic_then_later.insert(paths[1].clone(), need.clone());
    semantic_then_later.insert(paths[2].clone(), typed_outer.clone());
    let observed = semantic(reduce(semantic_then_later, Some(path_need.dupe())));
    assert!(matches!(
        observed.result,
        Err(NonregistryPreparationError::Direct(
            DirectLocalModulePreparationError::Fragment {
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            }
        ))
    ));
    assert_eq!(observed.observations.observations().len(), 2);

    let mut need_then_semantic = batch(1, absent(1));
    need_then_semantic.insert(paths[0].clone(), need.clone());
    need_then_semantic.insert(paths[2].clone(), typed_outer.clone());
    assert!(matches!(
        reduce(need_then_semantic, Some(path_need.dupe())),
        ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == path_need
    ));

    let mut no_semantic = batch(0, need);
    no_semantic.insert(paths[2].clone(), typed_outer);
    assert!(matches!(
        reduce(no_semantic, Some(path_need.dupe())),
        ControlFlow::Break(SourcePreparationOutcome::Complete(Err(found)))
            if found == NonregistryPreparationFrontierError::Path(outer)
    ));

    let union = path_need.try_union(&bootstrap_need).unwrap();
    let mut needs = batch(0, Ok(SourcePreparationOutcome::Need(path_need.dupe())));
    needs.insert(
        paths[1].clone(),
        Ok(SourcePreparationOutcome::Need(bootstrap_need)),
    );
    let ControlFlow::Break(SourcePreparationOutcome::Need(found)) = reduce(needs, Some(union))
    else {
        panic!("full source Need union")
    };
    assert!(found.path_observations().is_some());
    assert!(found.root_module_bootstrap_request().is_some());
}

fn observed_evaluation_key(apparent: &str) -> DirectLocalModuleEvaluationObservationKey {
    DirectLocalModuleEvaluationObservationKey(
        DirectLocalModuleEvaluationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new(apparent).unwrap(),
        )
        .unwrap(),
    )
}

fn complete_observed_direct_local_evaluation(
    value: &<DirectLocalModuleEvaluationObservationKey as Key>::Value,
) -> &ObservedDirectLocalModuleEvaluation {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed direct-local evaluation must complete")
    };
    observed
}

#[tokio::test]
async fn observed_evaluation_forwards_exact_preparation_events_families_and_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(HostSourceFamilyTracker::default());
    let module = b"module(name='dep')\nprint('eval-root')\ninclude('//p:a.MODULE.bazel')\n";
    let fragment = b"print('eval-fragment')\n";
    let fragments = [("p/a.MODULE.bazel", Some(fragment.as_slice()))];
    let key = observed_evaluation_key("dep");
    assert_eq!(
        key.to_string(),
        "observed-direct-local-module-evaluation:\"/workspace\":@dep"
    );
    let (mut cold, _) =
        direct_local_preparation_transaction(&dice, module, &fragments, 71, Some(tracker.dupe()))
            .await;
    let cold_value = cold.compute(&key).await.unwrap();
    let observed = complete_observed_direct_local_evaluation(&cold_value);
    let DirectLocalModuleEvaluation::Supported(evaluated) =
        observed.result.as_ref().as_ref().unwrap()
    else {
        panic!("direct-local evaluation must be supported")
    };
    assert_eq!(evaluated.module.base.expected_key.name.as_str(), "dep");

    let activations = tracker.take();
    for expected in [
        "observed-direct-local-module-evaluation:",
        "observed-direct-local-module-preparation:",
        "observed-direct-local-module-inspection:",
        "observed-host-repository-source-file:",
    ] {
        assert!(
            activations
                .iter()
                .any(|entry| entry.key.starts_with(expected)),
            "missing {expected}"
        );
    }
    for forbidden in [
        "direct-local-module-evaluation:",
        "direct-local-module-preparation:",
        "repository-package-source:",
        "external-bzl-module:",
        "repository-package-load:",
        "root-query",
    ] {
        assert!(
            !activations
                .iter()
                .any(|entry| entry.key.starts_with(forbidden)),
            "unexpected {forbidden}"
        );
    }
    let event_owners = activations
        .iter()
        .filter(|entry| entry.batch.is_some())
        .collect::<Vec<_>>();
    assert_eq!(event_owners.len(), 3, "{event_owners:?}");
    assert!(
        event_owners[0]
            .key
            .starts_with("bzlmod-observed-host-root-module-file:")
    );
    assert!(
        event_owners[1]
            .key
            .starts_with("observed-host-route-repo-file:")
    );
    assert!(
        event_owners[2]
            .key
            .starts_with("observed-direct-local-module-evaluation:")
    );
    let texts = event_owners[2]
        .batch
        .as_ref()
        .unwrap()
        .events()
        .iter()
        .filter_map(|event| match event {
            EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(texts, ["eval-root", "eval-fragment"]);

    let preparation = cold
        .compute(&DirectLocalModulePreparationObservationKey(
            direct_local_preparation_key("dep"),
        ))
        .await
        .unwrap();
    assert_exact_epoch(
        &complete_observed_direct_local_preparation(&preparation).observations,
        &observed.observations,
    );
    tracker.take();
    let warm = cold.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        &observed.result,
        &complete_observed_direct_local_evaluation(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));

    let support = direct_local_module_support_observed(&mut cold, &evaluated.route).await;
    let SourcePreparationOutcome::Complete(Ok(support)) = support else {
        panic!("observed support must complete")
    };
    assert!(matches!(
        support.result().as_ref(),
        Ok(DirectLocalModuleSupport::Supported)
    ));
    assert_exact_epoch(&observed.observations, support.observations());
    tracker.take();

    let legacy_key = DirectLocalModuleEvaluationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new("dep").unwrap(),
    )
    .unwrap();
    let legacy = cold.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy evaluation must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result.as_ref());
    assert!(
        !tracker
            .take()
            .iter()
            .any(|entry| entry.key.contains("observed-"))
    );

    let changed_fragments = [(
        "p/a.MODULE.bazel",
        Some(b"bazel_dep(name='leaf',version='2')\n".as_slice()),
    )];
    let (mut changed, _) =
        direct_local_preparation_transaction(&dice, module, &changed_fragments, 72, None).await;
    assert_ne!(
        observed.result.as_ref(),
        complete_observed_direct_local_evaluation(&changed.compute(&key).await.unwrap())
            .result
            .as_ref()
    );
    let (mut absent, _) = direct_local_file_transaction(&dice, "dep", None, None, 73, None).await;
    let absent_value = absent.compute(&key).await.unwrap();
    assert!(matches!(
        complete_observed_direct_local_evaluation(&absent_value)
            .result
            .as_ref(),
        Err(DirectLocalModuleEvaluationError::RootAbsent { .. })
    ));
    let absent_preparation = absent
        .compute(&DirectLocalModulePreparationObservationKey(
            direct_local_preparation_key("dep"),
        ))
        .await
        .unwrap();
    assert_exact_epoch(
        &complete_observed_direct_local_preparation(&absent_preparation).observations,
        &complete_observed_direct_local_evaluation(&absent_value).observations,
    );
    let (mut restored, _) =
        direct_local_preparation_transaction(&dice, module, &fragments, 74, None).await;
    assert_eq!(
        observed.result.as_ref(),
        complete_observed_direct_local_evaluation(&restored.compute(&key).await.unwrap())
            .result
            .as_ref()
    );

    for (variant, source, expected) in [
        (76, b"unknown_identifier\n".as_slice(), "preparation"),
        (
            77,
            b"module(name='dep')\nfail('evaluation')\n".as_slice(),
            "evaluation",
        ),
    ] {
        let (mut terminal, _) =
            direct_local_preparation_transaction(&dice, source, &[], variant, None).await;
        let value = terminal.compute(&key).await.unwrap();
        let preparation = terminal
            .compute(&DirectLocalModulePreparationObservationKey(
                direct_local_preparation_key("dep"),
            ))
            .await
            .unwrap();
        let found = complete_observed_direct_local_evaluation(&value);
        assert!(match found.result.as_ref() {
            Err(DirectLocalModuleEvaluationError::Preparation(_)) => expected == "preparation",
            Err(DirectLocalModuleEvaluationError::Evaluation(_)) => expected == "evaluation",
            _ => false,
        });
        assert_exact_epoch(
            &complete_observed_direct_local_preparation(&preparation).observations,
            &found.observations,
        );
    }
    let cycle_fragment = b"include('//p:a.MODULE.bazel')\n";
    let (mut cycle, _) = direct_local_preparation_transaction(
        &dice,
        cycle_fragment,
        &[("p/a.MODULE.bazel", Some(cycle_fragment.as_slice()))],
        78,
        None,
    )
    .await;
    let cycle_value = cycle.compute(&key).await.unwrap();
    let cycle_preparation = cycle
        .compute(&DirectLocalModulePreparationObservationKey(
            direct_local_preparation_key("dep"),
        ))
        .await
        .unwrap();
    assert!(matches!(
        complete_observed_direct_local_evaluation(&cycle_value)
            .result
            .as_ref(),
        Ok(DirectLocalModuleEvaluation::Unsupported(_))
    ));
    assert_exact_epoch(
        &complete_observed_direct_local_preparation(&cycle_preparation).observations,
        &complete_observed_direct_local_evaluation(&cycle_value).observations,
    );

    let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let cancelled_tracker = Arc::new(HostSourceFamilyTracker::default());
    let (mut cancelled, _) = direct_local_preparation_transaction(
        &cancelled_dice,
        module,
        &fragments,
        75,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(cancelled_tracker.take().is_empty());
    let (mut recovered, _) = direct_local_preparation_transaction(
        &cancelled_dice,
        module,
        &fragments,
        75,
        Some(cancelled_tracker.dupe()),
    )
    .await;
    assert!(
        complete_observed_direct_local_evaluation(&recovered.compute(&key).await.unwrap())
            .result
            .is_ok()
    );

    let need_tracker = Arc::new(HostSourceFamilyTracker::default());
    let need_dice = Dice::builder().build(DetectCycles::Enabled);
    let (mut pending, _) = direct_local_preparation_transaction(
        &need_dice,
        b"include('//missing:a.MODULE.bazel')\n",
        &[],
        79,
        Some(need_tracker.dupe()),
    )
    .await;
    let need_value = pending.compute(&key).await.unwrap();
    assert!(matches!(need_value, SourcePreparationOutcome::Need(_)));
    assert!(need_tracker.take().iter().all(|entry| {
        !entry
            .key
            .starts_with("observed-direct-local-module-evaluation:")
            || entry.batch.is_none()
    }));
}

#[test]
fn observed_evaluation_projection_prefix_and_outer_algebra_are_exact() {
    let semantic = Arc::new(Err(DirectLocalModuleEvaluationError::PreparationCompute {
        message: "compute".into(),
    }));
    let carrier = SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleEvaluation {
        result: semantic.dupe(),
        observations: PathObservationEpoch::empty(),
    }));
    let SourcePreparationOutcome::Complete(projected) =
        project_legacy_direct_local_evaluation(carrier)
    else {
        panic!("legacy evaluation projection must complete")
    };
    assert!(Arc::ptr_eq(&semantic, &projected));

    let (demand, result, epoch) = observed_horizon_epoch("evaluation");
    let full = direct_local_evaluation_complete(
        Err(DirectLocalModuleEvaluationError::RootAbsent {
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
        }),
        epoch.dupe(),
    );
    assert!(Arc::ptr_eq(
        complete_observed_direct_local_evaluation(&full)
            .observations
            .get(&demand)
            .unwrap(),
        &result
    ));
    assert!(direct_local_evaluation_publishes_batch(&full));
    let need = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand.dupe()));
    let need_child =
        direct_local_evaluation_observed_child(SourcePreparationOutcome::Need(need.dupe()));
    assert!(matches!(
        need_child,
        ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == need
    ));
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::FileBytes,
        },
    );
    let ControlFlow::Break(outer_value) = direct_local_evaluation_observed_child(
        SourcePreparationOutcome::Complete(Err(outer.dupe())),
    ) else {
        panic!("typed outer must stop evaluation")
    };
    assert!(DirectLocalModuleEvaluationObservationKey::validity(
        &outer_value
    ));
    assert!(DirectLocalModuleEvaluationObservationKey::equality(
        &outer_value,
        &outer_value
    ));
    assert!(!direct_local_evaluation_publishes_batch(&outer_value));
    let need_value = SourcePreparationOutcome::Need(need);
    assert!(!DirectLocalModuleEvaluationObservationKey::validity(
        &need_value
    ));
    assert!(!DirectLocalModuleEvaluationObservationKey::equality(
        &need_value,
        &need_value
    ));
    assert!(!direct_local_evaluation_publishes_batch(&need_value));

    let occurrence = horizon_occurrence("p", 1);
    let capability = DirectLocalIncludeCycleCapability {
        package: occurrence.package,
        target: occurrence.target,
        repeated_raw_label: occurrence.raw_label.clone(),
        repeated_location: occurrence.location.clone(),
        ancestor_raw_label: occurrence.raw_label,
        ancestor_location: occurrence.location,
    };
    let unsupported = direct_local_module_support_result(
        &local_route(),
        &Ok(DirectLocalModuleEvaluation::Unsupported(capability)),
    );
    assert!(matches!(
        unsupported,
        Ok(DirectLocalModuleSupport::Unsupported(_))
    ));
    let ordinary = direct_local_module_support_result(
        &local_route(),
        &Err(DirectLocalModuleEvaluationError::PreparationCompute {
            message: "ordinary".into(),
        }),
    );
    assert!(matches!(
        ordinary,
        Err(DirectLocalModuleSupportError { .. })
    ));
}

fn observed_preflight_key(package: &str) -> HostNonregistryPackagePreflightObservationKey {
    HostNonregistryPackagePreflightObservationKey(nonregistry_preflight(package))
}

fn complete_observed_preflight(
    value: &<HostNonregistryPackagePreflightObservationKey as Key>::Value,
) -> &ObservedHostNonregistryPackagePreflight {
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("observed package preflight must complete semantically: {value:?}")
    };
    value
}

fn preflight_marker_key(marker: &str) -> RepositorySourceFileObservationKey {
    RepositorySourceFileObservationKey(RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from("pkg").join(marker),
    })
}

fn preflight_need(name: &str) -> SourcePreparationNeeds {
    let (demand, _, _) = observed_horizon_epoch(name);
    SourcePreparationNeeds::path(NeedPathObservations::singleton(demand))
}

async fn complete_preflight_transaction(
    mut transaction: dice::DiceTransaction,
    immutable: bool,
) -> dice::DiceTransaction {
    let mut epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
    if immutable {
        let root =
            "bazel_dep(name = 'dep', version = '1')\narchive_override(module_name = 'dep')\n";
        let host = horizon_epoch(
            root,
            PathObservationNamespace::Host,
            "/workspace/dep",
            None,
            None,
            None,
            None,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            1,
        );
        epoch = merge_path_observations(&epoch, &host).unwrap();
    }
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/MODULE.bazel.lock").unwrap(),
        PathObservationOperation::Lstat,
    );
    let lockfile = PathObservationEpoch::from_shared([(
        demand,
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
    )])
    .unwrap();
    let epoch = merge_path_observations(&epoch, &lockfile).unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater.commit().await
}

#[test]
fn observed_preflight_identity_projection_and_outer_algebra_are_exact() {
    use std::hash::Hash;
    use std::hash::Hasher;

    let key = observed_preflight_key("pkg");
    let other = observed_preflight_key("other");
    assert_ne!(key, other);
    assert_eq!(
        key.to_string(),
        "observed-host-nonregistry-package-preflight:dep@1//pkg"
    );
    let hash = |key: &HostNonregistryPackagePreflightObservationKey| {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut state);
        state.finish()
    };
    assert_ne!(hash(&key), hash(&other));

    let (_, _, observations) = observed_horizon_epoch("carrier");
    let semantic = Arc::new(Ok(HostNonregistryPackagePreflight::NoBuildFile));
    let carrier = ObservedHostNonregistryPackagePreflight {
        result: semantic.dupe(),
        observations: observations.dupe(),
    };
    let carrier_dupe = carrier.dupe();
    assert!(Arc::ptr_eq(carrier.result(), carrier_dupe.result()));
    assert_exact_epoch(carrier.observations(), carrier_dupe.observations());
    let legacy = project_preflight_legacy(SourcePreparationOutcome::Complete(Ok((
        semantic.dupe(),
        observations.dupe(),
    ))));
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&semantic, &legacy));

    let need = SourcePreparationOutcome::Need(preflight_need("need"));
    assert!(!HostNonregistryPackagePreflightObservationKey::validity(
        &need
    ));
    assert!(!HostNonregistryPackagePreflightObservationKey::equality(
        &need, &need
    ));
    let (demand, _, _) = observed_horizon_epoch("outer");
    let frontier = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::FileBytes,
        },
    );
    let compute = [
        (
            HostNonregistryPackagePreflightObservationError::effective_compute(
                "effective".into(),
            ),
            HostNonregistryPackagePreflightObservationError::EffectiveCompute(
                "effective".into(),
            ),
        ),
        (
            HostNonregistryPackagePreflightObservationError::policy_compute("policy".into()),
            HostNonregistryPackagePreflightObservationError::PolicyCompute("policy".into()),
        ),
        (
            HostNonregistryPackagePreflightObservationError::ignore_compute("ignore".into()),
            HostNonregistryPackagePreflightObservationError::IgnoreCompute("ignore".into()),
        ),
        (
            HostNonregistryPackagePreflightObservationError::marker_compute(
                HostBuildFileName::Build,
                "marker".into(),
            ),
            HostNonregistryPackagePreflightObservationError::MarkerCompute {
                marker: HostBuildFileName::Build,
                message: "marker".into(),
            },
        ),
    ];
    let frontiers = [
        HostNonregistryPackagePreflightObservationError::EffectiveFrontier(frontier.dupe()),
        HostNonregistryPackagePreflightObservationError::IgnoreFrontier(frontier.dupe()),
        HostNonregistryPackagePreflightObservationError::MarkerFrontier {
            marker: HostBuildFileName::BuildDotBazel,
            error: frontier.dupe(),
        },
    ];
    for outer in frontiers.into_iter().chain(compute.into_iter().map(
        |(actual, expected)| {
            assert_eq!(actual, expected);
            actual
        },
    )) {
        let value = preflight_outer(outer).map(|result| {
            result.map(
                |(result, observations)| ObservedHostNonregistryPackagePreflight {
                    result,
                    observations,
                },
            )
        });
        assert!(HostNonregistryPackagePreflightObservationKey::validity(
            &value
        ));
        assert!(HostNonregistryPackagePreflightObservationKey::equality(
            &value, &value
        ));
    }

    let (_, _, semantic_prefix) = observed_horizon_epoch("semantic-prefix");
    for result in [
        Err(HostNonregistryPackagePreflightError::RootModuleFiles(
            "root".into(),
        )),
        Err(HostNonregistryPackagePreflightError::NonregistryOverrideRequired("dep".into())),
        Err(HostNonregistryPackagePreflightError::PolicyInput(
            crate::RootPackagePolicyProjectionError::MissingInput {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        )),
        Err(HostNonregistryPackagePreflightError::UnsupportedDeletedPackages),
        Ok(HostNonregistryPackagePreflight::InvalidPackageName {
            message: "invalid".into(),
        }),
    ] {
        let SourcePreparationOutcome::Complete(Ok((projected, observations))) =
            preflight_complete(result.clone(), semantic_prefix.dupe())
        else {
            panic!("semantic terminal must retain a carrier")
        };
        assert_eq!(projected.as_ref(), &result);
        assert_exact_epoch(&semantic_prefix, &observations);
    }

    let (duplicate, first, left) = observed_horizon_epoch("duplicate");
    let equal = Arc::new(first.as_ref().clone());
    let right = PathObservationEpoch::from_shared([(duplicate.dupe(), equal.dupe())]).unwrap();
    let merged = merge_path_observations(&left, &right).unwrap();
    assert!(Arc::ptr_eq(merged.get(&duplicate).unwrap(), &first));
    let conflict = PathObservationEpoch::from_shared([(
        duplicate.dupe(),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::Directory, 1, 1, 1, 1, 0o755),
        ))),
    )])
    .unwrap();
    assert!(merge_path_observations(&left, &conflict).is_err());
    assert!(matches!(
        PathObservationEpoch::from_shared([(
            duplicate,
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Missing,
            )),
        )]),
        Err(slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. })
    ));
}

#[tokio::test]
async fn observed_preflight_preserves_prefix_rows_events_and_legacy_parity() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let transaction = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &[],
        &[],
        501,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    let key = observed_preflight_key("pkg");
    let cold = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_preflight(&cold);
    assert!(matches!(
        observed.result().as_ref(),
        Ok(HostNonregistryPackagePreflight::BuildDotBazel)
    ));
    let cold_batches = std::mem::take(&mut *tracker.batches.lock().unwrap());
    let held_result = observed.result().dupe();
    let held_epoch = observed.observations().dupe();
    let warm = transaction.compute(&key).await.unwrap();
    assert!(HostNonregistryPackagePreflightObservationKey::equality(
        &cold, &warm
    ));
    assert!(Arc::ptr_eq(
        &held_result,
        complete_observed_preflight(&warm).result()
    ));
    assert!(
        std::mem::take(&mut *tracker.batches.lock().unwrap())
            .iter()
            .all(|(_, _, batch)| batch.is_none())
    );

    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );
    let effective = transaction.compute(&effective_key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(effective)) = effective else {
        panic!("effective override must complete")
    };
    let ignore_key =
        HostNonregistryRepositoryIgnoreObservationKey(HostNonregistryRepositoryIgnoreKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", "1"),
        ));
    let ignore = transaction.compute(&ignore_key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(ignore)) = ignore else {
        panic!("ignore must complete")
    };
    let marker_key = preflight_marker_key("BUILD.bazel");
    let marker = transaction.compute(&marker_key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(marker)) = marker else {
        panic!("marker must complete")
    };
    let expected =
        merge_path_observations(effective.observations(), ignore.observations()).unwrap();
    let expected = merge_path_observations(&expected, marker.observations()).unwrap();
    assert_exact_epoch(&expected, &held_epoch);
    assert_selected_epoch(&mut transaction, &expected, &held_epoch).await;

    let policy_key = CanonicalDeletedPackagesProjectionKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let expected_row = vec![
        effective_key.to_string(),
        policy_key.to_string(),
        ignore_key.to_string(),
        marker_key.to_string(),
    ];
    let rows = tracker.rows.lock().unwrap();
    let observed_row = rows
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .expect("observed preflight dependency row");
    assert_eq!(observed_row.1, expected_row);
    drop(rows);

    let preflight = tracker.preflight.lock().unwrap();
    assert!(
        preflight
            .iter()
            .any(|(kind, eventless)| *kind == ActivationKind::Evaluated && *eventless)
    );
    assert!(preflight.iter().all(|(_, eventless)| *eventless));
    drop(preflight);
    assert!(
        tracker
            .repo
            .lock()
            .unwrap()
            .iter()
            .any(|(kind, eventless)| *kind == ActivationKind::Evaluated && !*eventless)
    );
    let eventful = cold_batches
        .iter()
        .filter_map(|(owner, kind, batch)| {
            (*kind == ActivationKind::Evaluated)
                .then_some(batch.as_ref().map(|batch| (owner.as_str(), batch.events())))
                .flatten()
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        eventful.as_slice(),
        [(root, []), (repo, [])]
            if root.starts_with("bzlmod-observed-host-root-module-file:")
                && repo.starts_with("observed-host-nonregistry-repo-file:")
    ));
    for forbidden in [
        "host-nonregistry-module-closure:",
        "host-discovered-module:",
        "host-selected-module-graph:",
        "module-source-preparation:",
        "registry-file:",
        "host-selected-extension-",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, deps)| {
            !owner.starts_with(forbidden) && deps.iter().all(|dep| !dep.starts_with(forbidden))
        }));
    }
    tracker.batches.lock().unwrap().clear();
    let mut legacy_tx = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &[],
        &[],
        501,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let legacy = legacy_tx
        .compute(&nonregistry_preflight("pkg"))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy preflight must complete")
    };
    assert_eq!(legacy.as_ref(), held_result.as_ref());
    let legacy_key = nonregistry_preflight("pkg");
    let legacy_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &legacy_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        legacy_row,
        vec![
            effective_key
                .to_string()
                .trim_start_matches("observed-")
                .to_owned(),
            policy_key.to_string(),
            ignore_key.to_string().trim_start_matches("observed-").to_owned(),
            marker_key.to_string().trim_start_matches("observed-").to_owned(),
        ]
    );
    let legacy_batches = std::mem::take(&mut *tracker.batches.lock().unwrap());
    let legacy_eventful = legacy_batches
        .iter()
        .filter_map(|(owner, kind, batch)| {
            (*kind == ActivationKind::Evaluated)
                .then_some(batch.as_ref().map(|batch| (owner.as_str(), batch.events())))
                .flatten()
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        legacy_eventful.as_slice(),
        [(root, []), (repo, [])]
            if root.starts_with("root-module-evaluation:")
                && repo.starts_with("host-nonregistry-repo-file:")
    ), "{legacy_eventful:?}");
}

#[derive(Clone, Copy, Debug)]
enum PreflightStopStage {
    Effective,
    Ignore,
    BuildDotBazel,
    Build,
}

fn project_preflight_frontier(
    stage: PreflightStopStage,
    error: ObservedPathFrontierError,
) -> HostNonregistryPackagePreflightObservationError {
    match stage {
        PreflightStopStage::Effective => {
            HostNonregistryPackagePreflightObservationError::EffectiveFrontier(error)
        }
        PreflightStopStage::Ignore => {
            HostNonregistryPackagePreflightObservationError::IgnoreFrontier(error)
        }
        PreflightStopStage::BuildDotBazel => {
            HostNonregistryPackagePreflightObservationError::MarkerFrontier {
                marker: HostBuildFileName::BuildDotBazel,
                error,
            }
        }
        PreflightStopStage::Build => {
            HostNonregistryPackagePreflightObservationError::MarkerFrontier {
                marker: HostBuildFileName::Build,
                error,
            }
        }
    }
}

#[test]
fn observed_preflight_child_reducer_stops_need_and_outer_at_every_position() {
    for stage in [
        PreflightStopStage::Effective,
        PreflightStopStage::Ignore,
        PreflightStopStage::BuildDotBazel,
        PreflightStopStage::Build,
    ] {
        let need = preflight_need(&format!("{stage:?}-need"));
        let stopped = finish_preflight_child::<(), ()>(
            SourcePreparationOutcome::Need(need.dupe()),
            |_| panic!("Need must not continue"),
            |error| project_preflight_frontier(stage, error),
        );
        assert!(matches!(
            stopped,
            ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == need
        ));
        let (demand, _, _) = observed_horizon_epoch(&format!("{stage:?}-outer"));
        let frontier = ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        let stopped = finish_preflight_child::<(), ()>(
            SourcePreparationOutcome::Complete(Err(frontier)),
            |_| panic!("outer must not continue"),
            |error| project_preflight_frontier(stage, error),
        );
        let ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))) = stopped else {
            panic!("{stage:?} outer must be carrierless")
        };
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&project_preflight_frontier(
                stage,
                ObservedPathFrontierError::Epoch(
                    slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                        demand: observed_horizon_epoch("comparison").0,
                        result_operation: PathObservationOperation::FileBytes,
                    },
                ),
            ))
        );
    }
}

async fn observed_preflight_case(
    dice: &Arc<Dice>,
    fragments: &[(&str, Option<&[u8]>)],
    needs: &[&str],
    variant: i64,
    immutable: Option<(&str, u64, &str)>,
    wrong_kind: Option<(&str, PathNodeKind)>,
    tracker: Option<Arc<NonregistryPreflightTracker>>,
) -> (
    dice::DiceTransaction,
    <HostNonregistryPackagePreflightObservationKey as Key>::Value,
) {
    let transaction = host_nonregistry_transaction(
        dice, None, None, fragments, needs, variant, immutable, tracker, wrong_kind, true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, immutable.is_some()).await;
    let value = transaction
        .compute(&observed_preflight_key("pkg"))
        .await
        .unwrap();
    (transaction, value)
}

fn assert_preflight_value(
    value: &<HostNonregistryPackagePreflightObservationKey as Key>::Value,
    expected: HostNonregistryPackagePreflight,
) -> &ObservedHostNonregistryPackagePreflight {
    let observed = complete_observed_preflight(value);
    assert_eq!(observed.result().as_ref(), &Ok(expected));
    observed
}

async fn preflight_child_epoch(
    transaction: &mut dice::DiceTransaction,
    markers: &[&str],
) -> PathObservationEpoch {
    let effective = transaction
        .compute(&HostEffectiveModuleOverrideObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            "dep".into(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(effective)) = effective else {
        panic!("effective override must complete")
    };
    let ignore = transaction
        .compute(&HostNonregistryRepositoryIgnoreObservationKey(
            HostNonregistryRepositoryIgnoreKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                NonrootModuleKey::new("dep", "1"),
            ),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(ignore)) = ignore else {
        panic!("repository ignore must complete")
    };
    let mut epoch =
        merge_path_observations(effective.observations(), ignore.observations()).unwrap();
    for marker in markers {
        let marker = transaction.compute(&preflight_marker_key(marker)).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(marker)) = marker else {
            panic!("marker source must complete")
        };
        epoch = merge_path_observations(&epoch, marker.observations()).unwrap();
    }
    epoch
}

#[tokio::test]
async fn observed_preflight_marker_lifecycle_preference_and_cancellation_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let marker = |bytes: &'static [u8]| vec![("pkg/BUILD.bazel", Some(bytes)), ("pkg/BUILD", None)];
    let (_, a) = observed_preflight_case(&dice, &marker(b"a"), &[], 520, None, None, None).await;
    let a_observed = assert_preflight_value(&a, HostNonregistryPackagePreflight::BuildDotBazel);
    let held_result = a_observed.result().dupe();
    let held_epoch = a_observed.observations().dupe();
    let (_, b) = observed_preflight_case(&dice, &marker(b"b"), &[], 521, None, None, None).await;
    let absent = [("pkg/BUILD.bazel", None), ("pkg/BUILD", None)];
    let (mut absent_tx, absent_value) =
        observed_preflight_case(&dice, &absent, &[], 522, None, None, None).await;
    let absent_observed =
        assert_preflight_value(&absent_value, HostNonregistryPackagePreflight::NoBuildFile);
    assert_exact_epoch(
        &preflight_child_epoch(&mut absent_tx, &["BUILD.bazel", "BUILD"]).await,
        absent_observed.observations(),
    );
    let (_, directory) = observed_preflight_case(
        &dice,
        &absent,
        &[],
        523,
        None,
        Some(("pkg/BUILD.bazel", PathNodeKind::Directory)),
        None,
    )

    .await;
    assert_preflight_value(&directory, HostNonregistryPackagePreflight::NoBuildFile);
    let fallback = [
        ("pkg/BUILD.bazel", None),
        ("pkg/BUILD", Some(&b"fallback"[..])),
    ];
    let fallback_tracker = Arc::new(NonregistryPreflightTracker::default());
    let (mut fallback_tx, fallback_value) = observed_preflight_case(
        &dice,
        &fallback,
        &[],
        524,
        None,
        None,
        Some(fallback_tracker.dupe()),
    )
    .await;
    let fallback_observed =
        assert_preflight_value(&fallback_value, HostNonregistryPackagePreflight::Build);
    assert_exact_epoch(
        &preflight_child_epoch(&mut fallback_tx, &["BUILD.bazel", "BUILD"]).await,
        fallback_observed.observations(),
    );
    let rows = fallback_tracker.rows.lock().unwrap();
    let row = rows
        .iter()
        .find(|(owner, _)| owner == &observed_preflight_key("pkg").to_string())
        .unwrap();
    assert_eq!(row.1.len(), 5);
    assert_eq!(
        row.1.last().unwrap(),
        &preflight_marker_key("BUILD").to_string()
    );
    let (mut restored_tx, restored) =
        observed_preflight_case(&dice, &marker(b"a"), &[], 520, None, None, None).await;
    assert!(!HostNonregistryPackagePreflightObservationKey::equality(
        &a, &b
    ));
    assert!(HostNonregistryPackagePreflightObservationKey::equality(
        &a, &restored
    ));
    assert_eq!(
        held_result.as_ref(),
        complete_observed_preflight(&restored).result().as_ref()
    );
    assert_eq!(
        &held_epoch,
        complete_observed_preflight(&restored).observations(),
    );
    let marker_value = restored_tx
        .compute(&preflight_marker_key("BUILD.bazel"))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(marker_value)) = marker_value else {
        panic!("restored marker must complete")
    };
    for demand in marker_value.observations().observations().keys() {
        assert!(Arc::ptr_eq(
            complete_observed_preflight(&restored)
                .observations()
                .get(demand)
                .unwrap(),
            marker_value.observations().get(demand).unwrap(),
        ));
    }

    let immutable = |generation, instance, fragments, wrong_kind| {
        observed_preflight_case(
            &dice,
            fragments,
            &[],
            instance as i64,
            Some((generation, instance, "stable-source")),
            wrong_kind,
            None,
        )
    };
    let immutable_a_fragments = [("pkg/BUILD.bazel", Some(&b"a"[..]))];
    let (_, immutable_a) = immutable("/generation/a", 530, &immutable_a_fragments, None).await;
    let held_immutable = complete_observed_preflight(&immutable_a).dupe();
    let immutable_b_fragments = [("pkg/BUILD.bazel", Some(&b"b"[..]))];
    let (_, immutable_b) = immutable("/generation/b", 531, &immutable_b_fragments, None).await;
    let immutable_absent_fragments = [("pkg/BUILD.bazel", None)];
    let (_, immutable_absent) =
        immutable("/generation/c", 532, &immutable_absent_fragments, None).await;
    assert_preflight_value(
        &immutable_absent,
        HostNonregistryPackagePreflight::NoBuildFile,
    );
    let (_, immutable_directory) = immutable(
        "/generation/d",
        533,
        &immutable_absent_fragments,
        Some(("pkg/BUILD.bazel", PathNodeKind::Directory)),
    )
    .await;
    assert_preflight_value(
        &immutable_directory,
        HostNonregistryPackagePreflight::NoBuildFile,
    );
    let (_, immutable_restored) =
        immutable("/generation/a", 530, &immutable_a_fragments, None).await;
    assert!(!HostNonregistryPackagePreflightObservationKey::equality(
        &immutable_a,
        &immutable_b,
    ));
    assert!(HostNonregistryPackagePreflightObservationKey::equality(
        &immutable_a,
        &immutable_restored,
    ));
    assert_eq!(
        held_immutable,
        complete_observed_preflight(&immutable_restored).dupe(),
    );

    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let cancelled = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &marker(b"cancel"),
        &[],
        540,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut cancelled = complete_preflight_transaction(cancelled, false).await;
    tracker.preflight.lock().unwrap().clear();
    tracker.repo.lock().unwrap().clear();
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let key = observed_preflight_key("pkg");
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.preflight.lock().unwrap().is_empty());
    assert!(tracker.repo.lock().unwrap().is_empty());
    assert!(tracker.rows.lock().unwrap().is_empty());
    assert!(tracker.batches.lock().unwrap().is_empty());
    let (_, recovered) = observed_preflight_case(
        &dice,
        &marker(b"cancel"),
        &[],
        540,
        None,
        None,
        Some(tracker.dupe()),
    )
    .await;
    assert_preflight_value(&recovered, HostNonregistryPackagePreflight::BuildDotBazel);
}

#[tokio::test]
async fn observed_preflight_semantic_terminals_keep_decisive_prefixes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let transaction = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &[],
        &[],
        550,
        None,
        Some(tracker.dupe()),
        None,
        false,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    let invalid_key = observed_preflight_key("bad:name");
    let invalid = transaction.compute(&invalid_key).await.unwrap();
    let invalid = complete_observed_preflight(&invalid);
    assert!(matches!(
        invalid.result().as_ref(),
        Ok(HostNonregistryPackagePreflight::InvalidPackageName { message })
            if message.contains("Invalid package name")
    ));
    let invalid_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &invalid_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(invalid_row.len(), 1);
    assert!(invalid_row[0].starts_with("observed-host-effective-module-override:"));

    tracker.rows.lock().unwrap().clear();
    let mut updater = transaction.into_updater();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            [NormalizedAbsolutePath::new("/workspace").unwrap()],
            ["@dep+//pkg"],
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut deleted_tx = updater.commit().await;
    let deleted = deleted_tx
        .compute(&observed_preflight_key("pkg"))
        .await
        .unwrap();
    let deleted = complete_observed_preflight(&deleted);
    assert!(matches!(
        deleted.result().as_ref(),
        Err(HostNonregistryPackagePreflightError::UnsupportedDeletedPackages)
    ));
    assert_exact_epoch(invalid.observations(), deleted.observations());

    let deleted_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_preflight_key("pkg").to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(deleted_row.len(), 2);
    assert_eq!(
        deleted_row[1],
        CanonicalDeletedPackagesProjectionKey::new(NormalizedAbsolutePath::new("/workspace").unwrap()).to_string()
    );
    let cases: [(&[(&str, Option<&[u8]>)], Option<(&str, PathNodeKind)>); 2] = [
        (
            &[
                ("REPO.bazel", Some(&b"ignore_directories(['pkg'])\n"[..])),
                ("pkg/BUILD.bazel", Some(&b"unused"[..])),
            ],
            None,
        ),
        (
            &[
                ("REPO.bazel", Some(&b"this is invalid("[..])),
                ("pkg/BUILD.bazel", Some(&b"unused"[..])),
            ],
            None,
        ),
    ];
    for (index, (fragments, wrong_kind)) in cases.into_iter().enumerate() {
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let (mut transaction, value) = observed_preflight_case(
            &dice,
            fragments,
            &[],
            560 + index as i64,
            None,
            wrong_kind,
            Some(tracker.dupe()),
        )
        .await;
        let observed = complete_observed_preflight(&value);
        assert_exact_epoch(
            &preflight_child_epoch(&mut transaction, &[]).await,
            observed.observations(),
        );
        let expected = match (index, observed.result().as_ref()) {
            (0, Ok(HostNonregistryPackagePreflight::Ignored)) => true,
            (1, Err(HostNonregistryPackagePreflightError::RepositoryIgnore(_))) => true,
            _ => false,
        };
        assert!(expected, "{index}: {:?}", observed.result());
        let row = tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &observed_preflight_key("pkg").to_string())
            .unwrap()
            .1
            .clone();
        assert_eq!(row.len(), 3);
    }
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let mut transaction = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &[],
        &[],
        570,
        None,
        Some(tracker.dupe()),
        None,
        false,
    )
    .await;
    let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
    let file_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/dep/pkg/BUILD.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let epoch =
        PathObservationEpoch::from_shared(epoch.observations().iter().map(|(demand, result)| {
            if demand == &file_demand {
                (
                    demand.dupe(),
                    Arc::new(PathObservationResult::FileBytes(
                        PathOperationResult::Error(PathObservationError::NotALink),
                    )),
                )
            } else {
                (demand.dupe(), result.dupe())
            }
        }))
        .unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut transaction = complete_preflight_transaction(updater.commit().await, false).await;
    let value = transaction
        .compute(&observed_preflight_key("pkg"))
        .await
        .unwrap();
    assert!(matches!(
        complete_observed_preflight(&value).result().as_ref(),
        Err(HostNonregistryPackagePreflightError::RepositorySource {
            marker: HostBuildFileName::BuildDotBazel,
            ..
        })
    ));
    assert_exact_epoch(
        &preflight_child_epoch(&mut transaction, &["BUILD.bazel"]).await,
        complete_observed_preflight(&value).observations(),
    );
    let row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_preflight_key("pkg").to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(row.len(), 4);
}

fn observed_host_closure_key() -> HostNonregistryModuleClosureObservationKey {
    HostNonregistryModuleClosureObservationKey(HostNonregistryModuleClosureKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        NonrootModuleKey::new("dep", "1"),
    ))
}

fn complete_observed_host_closure(
    value: &<HostNonregistryModuleClosureObservationKey as Key>::Value,
) -> &ObservedHostNonregistryModuleClosure {
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("observed Host closure must complete semantically: {value:?}")
    };
    value
}

async fn observed_host_closure_case(
    dice: &Arc<Dice>,
    root_module: Option<&[u8]>,
    root_wrong_kind: Option<PathNodeKind>,
    fragments: &[(&str, Option<&[u8]>)],
    fragment_needs: &[&str],
    variant: i64,
    immutable: Option<(&str, u64, &str)>,
    tracker: Option<Arc<NonregistryPreflightTracker>>,
    fragment_wrong_kind: Option<(&str, PathNodeKind)>,
    capture_events: bool,
) -> (
    dice::DiceTransaction,
    <HostNonregistryModuleClosureObservationKey as Key>::Value,
) {
    let transaction = host_nonregistry_transaction(
        dice,
        root_module,
        root_wrong_kind,
        fragments,
        fragment_needs,
        variant,
        immutable,
        tracker,
        fragment_wrong_kind,
        capture_events,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, immutable.is_some()).await;
    let value = transaction
        .compute(&observed_host_closure_key())
        .await
        .unwrap();
    (transaction, value)
}

#[test]
fn observed_host_closure_identity_projection_and_outer_algebra_are_exact() {
    use std::hash::Hash;
    use std::hash::Hasher;

    let key = observed_host_closure_key();
    let other = HostNonregistryModuleClosureObservationKey(
        HostNonregistryModuleClosureKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("other", "1"),
        ),
    );
    assert_ne!(key, other);
    assert_eq!(
        key.to_string(),
        "observed-host-nonregistry-module-closure:dep@1"
    );
    let hash = |value: &HostNonregistryModuleClosureObservationKey| {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    };
    assert_ne!(hash(&key), hash(&other));

    let semantic = Arc::new(Err(HostNonregistryModuleClosureError::RootAbsent));
    let complete = SourcePreparationOutcome::Complete(Ok(ObservedHostNonregistryModuleClosure {
        result: semantic.dupe(),
        observations: PathObservationEpoch::empty(),
    }));
    let SourcePreparationOutcome::Complete(projected) =
        project_legacy_host_nonregistry_closure(complete.dupe())
    else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&semantic, &projected));
    assert!(HostNonregistryModuleClosureObservationKey::validity(
        &complete
    ));
    assert!(HostNonregistryModuleClosureObservationKey::equality(
        &complete, &complete
    ));

    let need = SourcePreparationOutcome::Need(preflight_need("closure"));
    assert!(!HostNonregistryModuleClosureObservationKey::validity(&need));
    assert!(!HostNonregistryModuleClosureObservationKey::equality(
        &need, &need
    ));
    let outer = observed_host_nonregistry_closure_outer(
        HostNonregistryModuleClosureObservationError::EffectiveCompute("compute".into()),
    );
    assert!(HostNonregistryModuleClosureObservationKey::validity(&outer));

    assert!(HostNonregistryModuleClosureObservationKey::equality(
        &outer, &outer
    ));
}

#[test]
fn observed_host_closure_initial_reducers_preserve_terminal_prefixes() {
    let key = observed_host_closure_key();
    let (prefix_demand, prefix_result, prefix) = observed_horizon_epoch("closure-prefix");
    let need = preflight_need("closure-initial-need");
    for stage in 0..3 {
        let stopped = forward_host_nonregistry_closure_observation::<()>(
            SourcePreparationOutcome::Need(need.dupe()),
            |error| match stage {
                0 => HostNonregistryModuleClosureObservationError::EffectiveFrontier(error),
                1 => HostNonregistryModuleClosureObservationError::MaterializationFrontier(error),
                _ => HostNonregistryModuleClosureObservationError::RootSourceFrontier(error),
            },
        );
        assert!(matches!(
            stopped,
            ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == need
        ));
        let outer = ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: prefix_demand.dupe(),
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        let stopped = forward_host_nonregistry_closure_observation::<()>(
            SourcePreparationOutcome::Complete(Err(outer)),
            |error| match stage {
                0 => HostNonregistryModuleClosureObservationError::EffectiveFrontier(error),
                1 => HostNonregistryModuleClosureObservationError::MaterializationFrontier(error),
                _ => HostNonregistryModuleClosureObservationError::RootSourceFrontier(error),
            },
        );
        let ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))) = stopped else {
            panic!("stage {stage} outer must be carrierless")
        };
        assert!(matches!(
            (stage, error),
            (
                0,
                HostNonregistryModuleClosureObservationError::EffectiveFrontier(_)
            ) | (
                1,
                HostNonregistryModuleClosureObservationError::MaterializationFrontier(_)
            ) | (
                2,
                HostNonregistryModuleClosureObservationError::RootSourceFrontier(_)
            )
        ));
    }

    assert!(matches!(
        observed_host_nonregistry_closure_outer(
            HostNonregistryModuleClosureObservationError::EffectiveCompute("effective compute".into())
        ),
        SourcePreparationOutcome::Complete(Err(
            HostNonregistryModuleClosureObservationError::EffectiveCompute(message)
        )) if message.as_ref() == "effective compute"
    ));
    for outcome in [
        host_nonregistry_closure_compute_error(
            HostNonregistryModuleClosureError::MaterializationCompute(
                "materialization compute".into(),
            ),
            prefix.dupe(),
        ),
        host_nonregistry_closure_compute_error(
            HostNonregistryModuleClosureError::RootSourceCompute("root source compute".into()),
            prefix.dupe(),
        ),
    ] {
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("compute failures remain semantic")
        };
        assert_exact_epoch(&prefix, observed.observations());
        assert!(Arc::ptr_eq(
            observed.observations().get(&prefix_demand).unwrap(),
            &prefix_result
        ));
    }

    let effective_error = Err(HostEffectiveModuleOverrideError::CommandPolicy(
        "effective semantic".into(),
    ));
    let ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(observed))) =
        finish_host_nonregistry_effective(&key.0, &effective_error, prefix.dupe())
    else {
        panic!("effective semantic failure")
    };
    assert!(matches!(
        observed.result().as_ref(),
        Err(HostNonregistryModuleClosureError::RootModuleFiles(message))
            if message.as_str().contains("effective semantic")
    ));
    assert_exact_epoch(&prefix, observed.observations());

    let materialization_incoming = observed_horizon_epoch("materialization-semantic").2;
    let materialization_prefix =
        merge_path_observations(&prefix, &materialization_incoming).unwrap();
    let ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(observed))) =
        finish_host_nonregistry_materialization(
            &Err(RepositoryMaterializationError::Spec(
                "materialization semantic".into(),
            )),
            &materialization_incoming,
            prefix.dupe(),
            HostRepositoryLocalPathPolicy::WorkspaceRelative,
        )
    else {
        panic!("materialization semantic failure")
    };
    assert!(matches!(
        observed.result().as_ref(),
        Err(HostNonregistryModuleClosureError::Materialization(
            RepositoryMaterializationError::Spec(message)
        )) if message.as_str() == "materialization semantic"
    ));
    assert_exact_epoch(&materialization_prefix, observed.observations());

    let input = HostNonregistryModuleClosureInput {
        source_identity: HostNonregistryModuleSourceIdentity::Local {
            repo_spec: local_route()
                .source_capability()
                .repo_spec()
                .unwrap()
                .clone(),
        },
        local_path_policy: HostRepositoryLocalPathPolicy::WorkspaceRelative,
        observations: prefix.dupe(),
    };
    let root_incoming = observed_horizon_epoch("root-semantic").2;
    let root_prefix = merge_path_observations(&prefix, &root_incoming).unwrap();
    let ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(observed))) =
        finish_host_nonregistry_root_source(
            Ok(RepositorySourceFileValue::Absent),
            &root_incoming,
            input,
        )
    else {
        panic!("root semantic failure")
    };
    assert!(matches!(
        observed.result().as_ref(),
        Err(HostNonregistryModuleClosureError::RootAbsent)
    ));
    assert_exact_epoch(&root_prefix, observed.observations());
}


#[test]
fn observed_host_horizon_reducer_is_occurrence_ordered_and_prefix_bounded() {
    let occurrences = ["p", "q", "r"]
        .into_iter()
        .enumerate()
        .map(|(slot, package)| {
            let occurrence = horizon_occurrence(package, u32::try_from(slot + 1).unwrap());
            NonregistryIncludeOccurrence {
                package: occurrence.package.package().clone(),
                target: occurrence.target,
                raw_label: occurrence.raw_label,
                location: occurrence.location,
            }
        })
        .collect::<Vec<_>>();
    let (initial_demand, initial_result, initial) = observed_horizon_epoch("host-horizon-initial");
    let children = ["p", "q", "r"].map(|name| observed_horizon_epoch(&format!("host-{name}")));
    let successful = || {
        SmallMap::from_iter(occurrences.iter().enumerate().map(|(slot, occurrence)| {
            (
                occurrence.package.clone(),
                Ok(SourcePreparationOutcome::Complete(Ok(
                    ObservedHostNonregistryPackagePreflight {
                        result: Arc::new(Ok(HostNonregistryPackagePreflight::Build)),
                        observations: children[slot].2.dupe(),
                    },
                ))),
            )
        }))
    };
    let expected_prefix = |completed: usize| {
        children
            .iter()
            .take(completed)
            .fold(initial.dupe(), |prefix, child| {
                merge_path_observations(&prefix, &child.2).unwrap()
            })
    };

    for slot in 0..3 {
        let mut compute = successful();
        compute.insert(occurrences[slot].package.clone(), Err("package compute".into()));
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            finish_observed_host_nonregistry_include_horizon(
                occurrences.clone(),
                &compute,
                None,
                initial.dupe(),
            )
        else {
            panic!("package compute remains semantic")
        };
        assert!(matches!(
            observed.result,
            Err(NonregistryPreparationError::Host(
                HostNonregistryModuleClosureError::Package {
                    failure: HostNonregistryIncludePackageFailure::Compute(ref message),
                    ..
                }
            )) if message.as_ref() == "package compute"
        ));
        assert_exact_epoch(&expected_prefix(slot), &observed.observations);
        for later in children.iter().skip(slot) {
            assert!(observed.observations.get(&later.0).is_none());
        }

        let need = preflight_need(&format!("host-horizon-{slot}-need"));
        let mut needs = successful();
        needs.insert(
            occurrences[slot].package.clone(),
            Ok(SourcePreparationOutcome::Need(need.dupe())),
        );
        assert!(matches!(
            finish_observed_host_nonregistry_include_horizon(
                occurrences.clone(),
                &needs,
                Some(need.dupe()),
                initial.dupe(),
            ),
            SourcePreparationOutcome::Need(found) if found == need
        ));

        let outer =
            HostNonregistryPackagePreflightObservationError::EffectiveCompute("outer".into());
        let mut outers = successful();
        outers.insert(
            occurrences[slot].package.clone(),
            Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))),
        );
        assert!(matches!(
            finish_observed_host_nonregistry_include_horizon(
                occurrences.clone(),
                &outers,
                None,
                initial.dupe(),
            ),
            SourcePreparationOutcome::Complete(Err(
                NonregistryPreparationFrontierError::Package(found)
            )) if found == outer
        ));

        let mut semantic = successful();
        semantic.insert(
            occurrences[slot].package.clone(),
            Ok(SourcePreparationOutcome::Complete(Ok(
                ObservedHostNonregistryPackagePreflight {
                    result: Arc::new(Ok(HostNonregistryPackagePreflight::NoBuildFile)),
                    observations: children[slot].2.dupe(),
                },
            ))),
        );
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            finish_observed_host_nonregistry_include_horizon(
                occurrences.clone(),
                &semantic,
                None,
                initial.dupe(),
            )
        else {
            panic!("package semantic remains semantic")
        };
        assert!(matches!(
            observed.result,
            Err(NonregistryPreparationError::Host(
                HostNonregistryModuleClosureError::Package {
                    failure: HostNonregistryIncludePackageFailure::NoBuildFile,
                    ..
                }
            ))
        ));
        assert_exact_epoch(&expected_prefix(slot + 1), &observed.observations);
        for later in children.iter().skip(slot + 1) {
            assert!(observed.observations.get(&later.0).is_none());
        }
    }

    let path_need = preflight_need("host-horizon-union-path");
    let bootstrap_need =
        SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        });
    let union = path_need.try_union(&bootstrap_need).unwrap();
    let mut needs = successful();
    needs.insert(
        occurrences[0].package.clone(),
        Ok(SourcePreparationOutcome::Need(path_need)),
    );
    needs.insert(
        occurrences[2].package.clone(),
        Ok(SourcePreparationOutcome::Need(bootstrap_need)),
    );
    let SourcePreparationOutcome::Need(found) =
        finish_observed_host_nonregistry_include_horizon(
            occurrences.clone(),
            &needs,
            Some(union),
            initial.dupe(),
        )
    else {
        panic!("full compatible Need union")
    };
    assert!(found.path_observations().is_some());
    assert!(found.root_module_bootstrap_request().is_some());

    let duplicate = PathObservationEpoch::from_shared([(
        initial_demand.dupe(),
        initial_result.dupe(),
    )])
    .unwrap();
    let mut equal = successful();
    equal.insert(
        occurrences[0].package.clone(),
        Ok(SourcePreparationOutcome::Complete(Ok(
            ObservedHostNonregistryPackagePreflight {
                result: Arc::new(Ok(HostNonregistryPackagePreflight::Build)),
                observations: duplicate,
            },
        ))),
    );
    let SourcePreparationOutcome::Complete(Ok(observed)) =
        finish_observed_host_nonregistry_include_horizon(
            occurrences.clone(),
            &equal,
            None,
            initial.dupe(),
        )
    else {
        panic!("equal duplicate")
    };
    assert!(Arc::ptr_eq(
        observed.observations.get(&initial_demand).unwrap(),
        &initial_result
    ));

    let conflict = PathObservationEpoch::from_shared([(
        initial_demand.dupe(),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Error(
            PathObservationError::NotALink,
        ))),
    )])
    .unwrap();
    let mut conflicting = successful();
    conflicting.insert(
        occurrences[0].package.clone(),
        Ok(SourcePreparationOutcome::Complete(Ok(
            ObservedHostNonregistryPackagePreflight {
                result: Arc::new(Ok(HostNonregistryPackagePreflight::Build)),
                observations: conflict,
            },
        ))),
    );
    assert!(matches!(
        finish_observed_host_nonregistry_include_horizon(
            occurrences,
            &conflicting,
            None,
            initial,
        ),
        SourcePreparationOutcome::Complete(Err(
            NonregistryPreparationFrontierError::Path(
                ObservedPathFrontierError::Epoch(
                    slug_workspace_v2::PathObservationEpochError::ConflictingDemand(found)
                )
            )
        )) if found == initial_demand
    ));
}
#[tokio::test]
async fn observed_host_closure_preserves_exact_epoch_rows_events_and_legacy_parity() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = b"module(name='dep',version='1')\ninclude('//pkg:a.MODULE.bazel')\ninclude('//pkg:a.MODULE.bazel')\n";
    let fragments = [
        (
            "pkg/a.MODULE.bazel",
            Some(&b"include('//other:b.MODULE.bazel')\n"[..]),
        ),
        (
            "other/b.MODULE.bazel",
            Some(&b"bazel_dep(name='b',version='1')\n"[..]),
        ),
    ];
    let (mut transaction, cold) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &fragments,
        &[],
        600,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let observed = complete_observed_host_closure(&cold);
    let HostNonregistryModuleClosure::Supported(closure) = observed.result().as_ref().as_ref().unwrap()
    else {
        panic!("expected supported observed closure")
    };
    assert_eq!(closure.fragments.len(), 4);
    assert_eq!(
        closure
            .fragments
            .iter()
            .map(|fragment| fragment.occurrence.target.as_str())
            .collect::<Vec<_>>(),
        [
            "a.MODULE.bazel",
            "a.MODULE.bazel",
            "b.MODULE.bazel",
            "b.MODULE.bazel"
        ]
    );
    let held_result = observed.result().dupe();
    let held_epoch = observed.observations().dupe();

    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    let materialization_key = RepositoryMaterializationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
    );
    let SourcePreparationOutcome::Complete(Ok(materialization)) = transaction
        .compute(&materialization_key)
        .await
        .unwrap()
    else {
        panic!("materialization carrier")
    };
    let root_source_key = RepositorySourceFileObservationKey(RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from("MODULE.bazel"),
    });
    let SourcePreparationOutcome::Complete(Ok(root_source)) =
        transaction.compute(&root_source_key).await.unwrap()
    else {
        panic!("root source carrier")
    };
    let package_key = observed_preflight_key("pkg");
    let package = transaction.compute(&package_key).await.unwrap();
    let package = complete_observed_preflight(&package);
    let package_source_key = RepositorySourceFileObservationKey(RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from("pkg/a.MODULE.bazel"),
    });
    let SourcePreparationOutcome::Complete(Ok(package_source)) =
        transaction.compute(&package_source_key).await.unwrap()
    else {
        panic!("package fragment carrier")
    };
    let other_key = observed_preflight_key("other");
    let other = transaction.compute(&other_key).await.unwrap();
    let other = complete_observed_preflight(&other);
    let other_source_key = RepositorySourceFileObservationKey(RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from("other/b.MODULE.bazel"),
    });
    let SourcePreparationOutcome::Complete(Ok(other_source)) =
        transaction.compute(&other_source_key).await.unwrap()
    else {
        panic!("other fragment carrier")
    };
    let mut expected = effective.observations().dupe();
    for incoming in [
        materialization.observations(),
        root_source.observations(),
        package.observations(),
        package_source.observations(),
        other.observations(),
        other_source.observations(),
    ] {
        expected = merge_path_observations(&expected, incoming).unwrap();
    }
    assert_exact_epoch(&expected, &held_epoch);
    assert_selected_epoch(&mut transaction, &expected, &held_epoch).await;

    let observed_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_host_closure_key().to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        observed_row,
        vec![
            effective_key.to_string(),
            materialization_key.to_string(),
            root_source_key.to_string(),
            package_key.to_string(),
            package_source_key.to_string(),
            other_key.to_string(),
            other_source_key.to_string(),
        ]
    );
    let closure_batches = tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .filter(|(owner, _, _)| owner == &observed_host_closure_key().to_string())
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        closure_batches
            .iter()
            .all(|(_, kind, batch)| *kind == ActivationKind::Evaluated && batch.is_none())
    );
    let child_batches = tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(owner, kind, batch)| {
            (*kind == ActivationKind::Evaluated)
                .then_some(batch.as_ref().map(|batch| (owner.clone(), batch.events().to_vec())))
                .flatten()
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        child_batches.as_slice(),
        [(root_owner, root_events), (repo_owner, repo_events)]
            if root_owner.starts_with("bzlmod-observed-host-root-module-file:")
                && repo_owner.starts_with("observed-host-nonregistry-repo-file:")
                && root_events.is_empty()
                && repo_events.is_empty()
    ));

    tracker.batches.lock().unwrap().clear();
    let warm = transaction
        .compute(&observed_host_closure_key())
        .await
        .unwrap();
    assert!(HostNonregistryModuleClosureObservationKey::equality(
        &cold, &warm
    ));
    assert!(Arc::ptr_eq(
        &held_result,
        complete_observed_host_closure(&warm).result()
    ));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(_, _, batch)| batch.is_none()));

    let legacy_key = HostNonregistryModuleClosureKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        NonrootModuleKey::new("dep", "1"),
    );

    let legacy = transaction.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy closure must complete")
    };
    assert_eq!(legacy.as_ref(), held_result.as_ref());
    let legacy_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &legacy_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        legacy_row,
        vec![
            HostEffectiveModuleOverrideKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                "dep".into(),
            )
            .to_string(),
            RepositoryMaterializationKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
            }
            .to_string(),
            RepositorySourceFileKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
                repo_relative_path: PathBuf::from("MODULE.bazel"),
            }
            .to_string(),
            nonregistry_preflight("pkg").to_string(),
            RepositorySourceFileKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
                repo_relative_path: PathBuf::from("pkg/a.MODULE.bazel"),
            }
            .to_string(),
            nonregistry_preflight("other").to_string(),
            RepositorySourceFileKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
                repo_relative_path: PathBuf::from("other/b.MODULE.bazel"),
            }
            .to_string(),
        ]
    );
    for forbidden in [
        "host-discovered-module:",
        "host-selected-module-graph:",
        "module-source-preparation:",
        "registry-file:",
        "host-selected-extension-",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, deps)| {
            !owner.starts_with(forbidden) && deps.iter().all(|dep| !dep.starts_with(forbidden))
        }));
    }
}
#[tokio::test]
async fn observed_host_closure_terminals_lifecycles_need_cycle_and_cancellation_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = b"module(name='dep',version='1')\ninclude('//pkg:a.MODULE.bazel')\n";
    let fragment_a = [("pkg/a.MODULE.bazel", Some(&b"bazel_dep(name='a',version='1')\n"[..]))];
    let fragment_b = [("pkg/a.MODULE.bazel", Some(&b"bazel_dep(name='b',version='1')\n"[..]))];
    let (mut local_a_tx, local_a) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &fragment_a,
        &[],
        610,
        None,
        None,
        None,
        false,
    )
    .await;
    let held_local_result = complete_observed_host_closure(&local_a).result().dupe();
    let held_local_epoch = complete_observed_host_closure(&local_a).observations().dupe();
    assert_selected_epoch(
        &mut local_a_tx,
        &held_local_epoch,
        complete_observed_host_closure(&local_a).observations(),
    )
    .await;
    let (_, local_b) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &fragment_b,
        &[],
        611,
        None,
        None,
        None,
        false,
    )
    .await;
    let (_, local_absent) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &[("pkg/a.MODULE.bazel", None)],
        &[],
        612,
        None,
        None,
        None,
        false,
    )
    .await;
    let (_, local_directory) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &[("pkg/a.MODULE.bazel", None)],
        &[],
        613,
        None,
        None,
        Some(("pkg/a.MODULE.bazel", PathNodeKind::Directory)),
        false,
    )
    .await;
    let (mut local_restored_tx, local_restored) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &fragment_a,
        &[],
        610,
        None,
        None,
        None,
        false,
    )
    .await;
    assert!(!HostNonregistryModuleClosureObservationKey::equality(
        &local_a, &local_b
    ));
    assert!(matches!(
        complete_observed_host_closure(&local_absent).result().as_ref(),
        Err(HostNonregistryModuleClosureError::Fragment {
            failure: DirectLocalIncludeFragmentFailure::Absent,
            ..
        })
    ));
    assert!(matches!(
        complete_observed_host_closure(&local_directory).result().as_ref(),
        Err(HostNonregistryModuleClosureError::Fragment {
            failure: DirectLocalIncludeFragmentFailure::Source(
                RepositorySourceFileError::WrongKind { .. }
            ),
            ..
        })
    ));
    assert!(HostNonregistryModuleClosureObservationKey::equality(
        &local_a,
        &local_restored
    ));
    assert_eq!(held_local_result.as_ref(), complete_observed_host_closure(&local_restored).result().as_ref());
    assert!(!held_local_epoch.observations().is_empty());
    assert_selected_epoch(
        &mut local_restored_tx,
        complete_observed_host_closure(&local_restored).observations(),
        complete_observed_host_closure(&local_restored).observations(),
    )
    .await;

    let immutable = |generation, instance, fragments, wrong_kind| {
        observed_host_closure_case(
            &dice,
            Some(root),
            None,
            fragments,
            &[],
            620 + instance as i64,
            Some((generation, instance, "closure-content")),
            None,
            wrong_kind,
            false,
        )
    };
    let (_, immutable_a) = immutable("/generation/closure-a", 620, &fragment_a, None).await;
    let held_immutable = complete_observed_host_closure(&immutable_a).dupe();
    let (_, immutable_b) = immutable("/generation/closure-a", 620, &fragment_b, None).await;
    let (_, immutable_absent) = immutable(
        "/generation/closure-a",
        620,
        &[("pkg/a.MODULE.bazel", None)],
        None,
    )
    .await;
    let (_, immutable_directory) = immutable(
        "/generation/closure-a",
        620,
        &[("pkg/a.MODULE.bazel", None)],
        Some(("pkg/a.MODULE.bazel", PathNodeKind::Directory)),
    )
    .await;
    let (mut immutable_restored_tx, immutable_restored) =
        immutable("/generation/closure-a", 620, &fragment_a, None).await;
    assert!(!HostNonregistryModuleClosureObservationKey::equality(
        &immutable_a,
        &immutable_b
    ));
    assert!(matches!(
        complete_observed_host_closure(&immutable_absent).result().as_ref(),
        Err(HostNonregistryModuleClosureError::Fragment {
            failure: DirectLocalIncludeFragmentFailure::Absent,
            ..
        })
    ));
    assert!(matches!(
        complete_observed_host_closure(&immutable_directory).result().as_ref(),
        Err(HostNonregistryModuleClosureError::Fragment {
            failure: DirectLocalIncludeFragmentFailure::Source(
                RepositorySourceFileError::WrongKind { .. }
            ),
            ..
        })
    ));
    assert!(HostNonregistryModuleClosureObservationKey::equality(
        &immutable_a,
        &immutable_restored
    ));
    assert_eq!(held_immutable, complete_observed_host_closure(&immutable_restored).dupe());
    assert_selected_epoch(
        &mut immutable_restored_tx,
        complete_observed_host_closure(&immutable_restored).observations(),
        complete_observed_host_closure(&immutable_restored).observations(),
    )
    .await;

    let need_root = b"module(name='dep',version='1')\ninclude('//pkg:a.MODULE.bazel')\ninclude('//other:b.MODULE.bazel')\n";
    let (_, need) = observed_host_closure_case(
        &dice,
        Some(need_root),
        None,
        &[],
        &["pkg/a.MODULE.bazel", "other/b.MODULE.bazel"],
        630,
        None,
        None,
        None,
        false,
    )
    .await;
    assert!(matches!(
        &need,
        SourcePreparationOutcome::Need(found)
            if found.path_observations().unwrap().demands().len() == 2
    ));
    assert!(!HostNonregistryModuleClosureObservationKey::validity(&need));
    assert!(!HostNonregistryModuleClosureObservationKey::equality(
        &need, &need
    ));
    let cycle_fragment = [("pkg/a.MODULE.bazel", Some(&b"include('//pkg:a.MODULE.bazel')\n"[..]))];
    let (_, cycle) = observed_host_closure_case(
        &dice,
        Some(root),
        None,
        &cycle_fragment,
        &[],
        631,
        None,
        None,
        None,
        false,
    )
    .await;
    assert!(matches!(
        complete_observed_host_closure(&cycle).result().as_ref(),
        Ok(HostNonregistryModuleClosure::UnsupportedCycle { closure, .. })
            if closure.fragments.len() == 2
    ));
    let (_, bad_label) = observed_host_closure_case(
        &dice,
        Some(b"module(name='dep',version='1')\ninclude('bad')\n"),
        None,
        &[],
        &[],
        632,
        None,
        None,
        None,
        false,
    )
    .await;
    assert!(matches!(
        complete_observed_host_closure(&bad_label).result().as_ref(),
        Err(HostNonregistryModuleClosureError::BadLabel { .. })
    ));

    let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let cancelled_tracker = Arc::new(NonregistryPreflightTracker::default());
    let cancelled = host_nonregistry_transaction(
        &cancelled_dice,
        Some(root),
        None,
        &fragment_a,
        &[],
        640,
        None,
        Some(cancelled_tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut cancelled = complete_preflight_transaction(cancelled, false).await;
    cancelled_tracker.rows.lock().unwrap().clear();
    cancelled_tracker.batches.lock().unwrap().clear();
    cancelled_tracker.closure.lock().unwrap().clear();
    let mut future = Box::pin(cancelled.compute(&observed_host_closure_key()));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(cancelled_tracker.rows.lock().unwrap().is_empty());
    assert!(cancelled_tracker.batches.lock().unwrap().is_empty());
    assert!(cancelled_tracker.closure.lock().unwrap().is_empty());
    let (_, recovered) = observed_host_closure_case(
        &cancelled_dice,
        Some(root),
        None,
        &fragment_a,
        &[],
        640,
        None,
        Some(cancelled_tracker),
        None,
        true,
    )
    .await;
    assert!(matches!(
        complete_observed_host_closure(&recovered).result().as_ref(),
        Ok(HostNonregistryModuleClosure::Supported(_))
    ));
}
