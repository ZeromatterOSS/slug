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
        self.activations.lock().unwrap().push(HostSourceActivation {
            key: key.to_string(),
            kind: activation.kind(),
            event_free: activation.evaluation_data().is_none(),
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
        "bazel_dep(name='dep',version='1')\nlocal_path_override(module_name='dep',path='{local_path}')\n"
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
                    slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                        root_source.clone(),
                    )),
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
    let has = |prefix: &str| activations.iter().any(|entry| entry.key.starts_with(prefix));
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
        .filter(|entry| entry.kind == ActivationKind::Evaluated && !entry.event_free)
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
    assert!(tracker
        .take()
        .iter()
        .all(|entry| entry.event_free));

    let legacy = cold.compute(&direct_local_file_key("dep")).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy direct-local file must complete")
    };
    assert_eq!(legacy.as_ref(), cold_observed.result.as_ref());
    assert!(!tracker
        .take()
        .iter()
        .any(|entry| entry.key.contains("observed-")));

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
        .changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())])
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

    let (mut wrong_kind, _) = direct_local_file_transaction(
        &dice,
        "dep",
        None,
        Some(PathNodeKind::Directory),
        13,
        None,
    )
    .await;
    let wrong_kind = wrong_kind.compute(&key).await.unwrap();
    let wrong_kind = complete_direct_local_file(&wrong_kind);
    assert!(matches!(
        wrong_kind.result.as_ref(),
        Err(DirectLocalModuleFileError::Source(
            RepositorySourceFileError::WrongKind { .. }
        ))
    ));
    assert!(wrong_kind.observations.observations().len() > route_error.observations.observations().len());

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
    let merged = merge_direct_local_file_observations(&left, &right).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
    let conflict = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(b"different".as_slice())),
        )),
    )])
    .unwrap();
    assert!(matches!(
        merge_direct_local_file_observations(&left, &conflict),
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
    assert!(complete_direct_local_file(&route_compute).observations.observations().is_empty());
    assert!(Arc::ptr_eq(
        complete_direct_local_file(&source_compute).observations.get(&demand).unwrap(),
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
        assert!(matches!(&value, SourcePreparationOutcome::Complete(Err(error)) if error == &outer));
        assert!(DirectLocalModuleFileObservationKey::validity(&value));
        assert!(DirectLocalModuleFileObservationKey::equality(&value, &value));
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
        .compute(&DirectLocalModuleFileObservationKey(
            direct_local_file_key("dep"),
        ))
        .await
        .unwrap();
    let child = complete_direct_local_file(&child_value);
    assert_eq!(cold_observed.observations, child.observations);
    for (demand, result) in cold_observed.observations.observations() {
        assert!(Arc::ptr_eq(child.observations.get(demand).unwrap(), result));
    }
    let activations = tracker.take();
    let has = |prefix: &str| activations.iter().any(|entry| entry.key.starts_with(prefix));
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
        .filter(|entry| entry.kind == ActivationKind::Evaluated && !entry.event_free)
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 1);
    assert!(owners[0]
        .key
        .starts_with("bzlmod-observed-host-root-module-file:"));

    let warm = cold.compute(&key).await.unwrap();
    assert!(DirectLocalModuleInspectionObservationKey::equality(
        &cold_value,
        &warm
    ));
    assert!(Arc::ptr_eq(
        &cold_observed.result,
        &complete_observed_direct_local_inspection(&warm).result
    ));
    assert!(tracker.take().iter().all(|entry| entry.event_free));
    let legacy = cold.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy direct-local inspection must complete")
    };
    assert_eq!(legacy.as_ref(), cold_observed.result.as_ref());
    assert!(!tracker
        .take()
        .iter()
        .any(|entry| entry.key.contains("observed-")));

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
        let actual = match complete_observed_direct_local_inspection(&value).result.as_ref() {
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
        .changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())])
        .unwrap();
    let mut route_need = updater.commit().await;
    let route_need = route_need.compute(&key).await.unwrap();
    assert!(matches!(route_need, SourcePreparationOutcome::Need(_)));
    assert!(!DirectLocalModuleInspectionObservationKey::validity(&route_need));
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
        .compute(&DirectLocalModuleFileObservationKey(
            direct_local_file_key("missing"),
        ))
        .await
        .unwrap();
    assert_eq!(
        semantic.observations,
        complete_direct_local_file(&semantic_child).observations
    );

    let (mut invalid, _) = direct_local_file_transaction(
        &dice,
        "dep",
        Some(b"include(\n"),
        None,
        42,
        None,
    )
    .await;
    let invalid_value = invalid.compute(&key).await.unwrap();
    let invalid_observed = complete_observed_direct_local_inspection(&invalid_value);
    assert!(matches!(
        invalid_observed.result.as_ref(),
        Err(DirectLocalModuleInspectionError::Inspection(_, _))
    ));
    let invalid_child = invalid
        .compute(&DirectLocalModuleFileObservationKey(
            direct_local_file_key("dep"),
        ))
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
    assert!(complete_observed_direct_local_inspection(&carrier)
        .observations
        .observations()
        .is_empty());
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
    assert!(matches!(&outer_value, SourcePreparationOutcome::Complete(Err(error)) if error == &outer));
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
