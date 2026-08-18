#[derive(Debug, Clone)]
struct HostSourceActivation {
    key: String,
    kind: ActivationKind,
    event_free: bool,
}

#[derive(Debug, Default)]
struct HostSourceFamilyTracker {
    activations: Mutex<Vec<HostSourceActivation>>,
}

impl HostSourceFamilyTracker {
    fn take(&self) -> Vec<HostSourceActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for HostSourceFamilyTracker {
    fn key_activated(
        &self,
        _: &DynKey,
        _: &mut dyn Iterator<Item = &DynKey>,
        _: ActivationData,
    ) {
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        if key
            .downcast_ref::<HostRepositoryPathObservationKey>()
            .is_some()
            || key
                .downcast_ref::<HostRepositorySourceFileObservationKey>()
                .is_some()
            || key.downcast_ref::<HostRepositoryPathKey>().is_some()
            || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
        {
            self.activations.lock().unwrap().push(HostSourceActivation {
                key: key.to_string(),
                kind: activation.kind(),
                event_free: activation.evaluation_data().is_none(),
            });
        }
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
    for (demand, result) in expected.observations() {
        assert!(
            Arc::ptr_eq(actual.get(demand).unwrap(), result),
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
    let key =
        HostRepositorySourceFileObservationKey::new(local_route(), PathBuf::from("link"));
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
    assert!(HostRepositorySourceFileObservationKey::equality(&cold, &warm));
    let warm_observed = complete_observed_source(&warm);
    assert!(Arc::ptr_eq(observed.result(), warm_observed.result()));
    assert_exact_epoch(&epoch, warm_observed.observations());

    let activations = tracker.take();
    assert!(activations.iter().all(|entry| entry.event_free));
    assert!(activations
        .iter()
        .any(|entry| matches!(entry.kind, ActivationKind::Evaluated | ActivationKind::Reused)));
    assert!(activations
        .iter()
        .any(|entry| entry.key.starts_with("observed-host-repository-path:")));
    assert!(activations.iter().any(|entry| entry
        .key
        .starts_with("observed-host-repository-source-file:")));
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
    let mut pending = observed_source_transaction(
        &dice,
        material("dep"),
        PathObservationEpoch::empty(),
        None,
    )
    .await;
    let pending = pending
        .compute(&HostRepositoryPathObservationKey(HostRepositoryPathKey::new(
            local_route(),
            PathBuf::from("BUILD.bazel"),
        )))
        .await
        .unwrap();
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));

    let builtin = RootRepositoryRoute::builtin_for_test(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let mut invalid = observed_source_transaction(
        &dice,
        material("dep"),
        PathObservationEpoch::empty(),
        None,
    )
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
    assert!(activations.iter().any(|entry| entry
        .key
        .starts_with("host-repository-source-file:")));
    assert!(!activations
        .iter()
        .any(|entry| entry.key.starts_with("observed-host-repository-")));
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
    let merged =
        append_host_repository_source_observation(&base, demand.dupe(), equal).unwrap();
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
    let wrong_operation =
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
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
    let key = HostRepositorySourceFileObservationKey::new(
        local_route(),
        PathBuf::from("BUILD.bazel"),
    );
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
    assert!(tracker.take().iter().all(|entry| entry.event_free));

    let mut changed =
        observed_source_transaction(&dice, material("dep"), epoch(b"b"), None).await;
    let changed = changed.compute(&key).await.unwrap();
    assert!(matches!(
        complete_observed_source(&changed).result().as_ref(),
        Ok(HostRepositorySourceFileValue::Present { bytes, .. }) if bytes.as_ref() == b"b"
    ));

    let mut restored =
        observed_source_transaction(&dice, material("dep"), epoch(b"a"), None).await;
    let restored = restored.compute(&key).await.unwrap();
    assert_eq!(
        first_observed.result().as_ref(),
        complete_observed_source(&restored).result().as_ref()
    );
}
