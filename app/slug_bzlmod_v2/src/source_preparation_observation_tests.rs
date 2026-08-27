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

fn directory_epoch(
    namespace: PathObservationNamespace,
    path: &str,
    entries: &[(&str, PathDirectoryEntryKind)],
) -> PathObservationEpoch {
    let prefix = host_path_epoch(namespace, path, Some(PathNodeKind::Directory), None);
    let entries = PathDirectoryEntries::new(entries.iter().map(|(name, kind)| {
        PathDirectoryEntry::new(PathDirectoryName::new(*name).unwrap(), *kind)
    }));
    PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    namespace,
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::DirectoryEntries,
                ),
                Arc::new(PathObservationResult::DirectoryEntries(
                    PathOperationResult::Present(entries),
                )),
            ))),
    )
    .unwrap()
}

fn symlink_directory_epoch(
    requested: &str,
    target: &str,
    entry: &str,
) -> PathObservationEpoch {
    let prefix = symlink_path_epoch(requested, target);
    let target = NormalizedAbsolutePath::new(target).unwrap();
    let namespace = PathObservationNamespace::Host;
    PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .filter(|(demand, _)| {
                !(demand.path() == &target
                    && demand.operation() == PathObservationOperation::Lstat)
            })
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain([
                (
                    PathObservationDemand::new(
                        namespace,
                        target.dupe(),
                        PathObservationOperation::Lstat,
                    ),
                    Arc::new(PathObservationResult::Lstat(
                        PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::Directory,
                            1,
                            2,
                            3,
                            4,
                            0o755,
                        )),
                    )),
                ),
                (
                    PathObservationDemand::new(
                        namespace,
                        target.dupe(),
                        PathObservationOperation::DirectoryEntries,
                    ),
                    Arc::new(PathObservationResult::DirectoryEntries(
                        PathOperationResult::Present(PathDirectoryEntries::new([
                            PathDirectoryEntry::new(
                                PathDirectoryName::new(entry).unwrap(),
                                PathDirectoryEntryKind::File,
                            ),
                        ])),
                    )),
                ),
            ]),
    )
    .unwrap()
}

fn generated_listing_route(bytes: &'static [u8]) -> RootRepositoryRoute {
    let plan = GeneratedRepositoryFileEffectPlan::build([(
        CompactString::new("BUILD.bazel"),
        Arc::<[u8]>::from(bytes),
        true,
    )])
    .unwrap();
    RootRepositoryRoute::for_generated_repo_spec(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ApparentRepoName::new("generated").unwrap(),
        CanonicalRepoName::new("extension+generated").unwrap(),
        RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@extension+repo//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        },
        HostRepositoryLocalPathPolicy::LocalUnsupported,
        plan,
    )
    .unwrap()
}

fn route_immutable_material(
    route: &RootRepositoryRoute,
    source_identity: &'static str,
    generation_root: &str,
    observation_instance: PathObservationInstanceId,
) -> RepositoryMaterializationResultEpoch {
    let HostRepositoryMaterializationDisposition::Request(request) =
        host_repository_materialization_request(&route.source_capability()).unwrap()
    else {
        panic!("materialized listing route")
    };
    RepositoryMaterializationResultEpoch::new(
        route.workspace().dupe(),
        [RepositoryMaterializationEpochEntry {
            request,
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    source_identity: Arc::from(source_identity),
                    generation_root: PathBuf::from(generation_root),
                    observation_instance,
                },
            ),
        }],
    )
    .unwrap()
}

fn complete_observed_listing(
    value: &<HostRepositoryDirectoryListingObservationKey as Key>::Value,
) -> &ObservedHostRepositoryDirectoryListing {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed repository directory listing must complete")
    };
    observed
}

fn listing_names(listing: &PathDirectoryListing) -> Vec<(String, PathDirectoryEntryKind)> {
    let PathDirectoryListing::Present(entries) = listing else {
        return Vec::new();
    };
    entries
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.name().as_os_str().to_str().unwrap().to_owned(),
                entry.kind(),
            )
        })
        .collect()
}

#[tokio::test]
async fn routed_directory_listing_covers_builtin_legacy_and_observed_root() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let route = RootRepositoryRoute::builtin_for_test(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let mut transaction = dice.updater().commit().await;
    let legacy = transaction
        .compute(&HostRepositoryDirectoryListingKey::new(
            route.clone(),
            PackagePath::root(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(legacy)) = legacy else {
        panic!("built-in legacy listing")
    };
    assert_eq!(
        listing_names(&legacy),
        [
            ("MODULE.bazel".to_owned(), PathDirectoryEntryKind::File),
            ("src".to_owned(), PathDirectoryEntryKind::Directory),
            ("tools".to_owned(), PathDirectoryEntryKind::Directory),
        ]
    );
    let observed = transaction
        .compute(&HostRepositoryDirectoryListingObservationKey::new(
            route,
            PackagePath::root(),
        ))
        .await
        .unwrap();
    assert!(HostRepositoryDirectoryListingObservationKey::validity(
        &observed
    ));
    let observed = complete_observed_listing(&observed);
    assert_eq!(observed.result().as_ref(), &Ok(legacy));
    assert!(observed.observations().observations().is_empty());
}

#[tokio::test]
async fn routed_directory_listing_observes_local_present_missing_wrong_kind_and_need() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = HostRepositoryDirectoryListingObservationKey::new(
        local_route(),
        PackagePath::root(),
    );
    let present = directory_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep",
        &[
            ("z", PathDirectoryEntryKind::File),
            ("a", PathDirectoryEntryKind::Directory),
        ],
    );
    let mut transaction =
        observed_source_transaction(&dice, material("dep"), present.dupe(), None).await;
    let value = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_listing(&value);
    assert_eq!(
        listing_names(observed.result().as_ref().as_ref().unwrap()),
        [
            ("a".to_owned(), PathDirectoryEntryKind::Directory),
            ("z".to_owned(), PathDirectoryEntryKind::File),
        ]
    );
    assert_selected_epoch(&mut transaction, &present, observed.observations()).await;

    for (kind, expected_missing) in [
        (None, true),
        (Some(PathNodeKind::RegularFile), false),
    ] {
        let epoch = host_path_epoch(
            PathObservationNamespace::Host,
            "/workspace/dep",
            kind,
            None,
        );
        let mut transaction =
            observed_source_transaction(&dice, material("dep"), epoch, None).await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_listing(&value);
        if expected_missing {
            assert_eq!(
                observed.result().as_ref(),
                &Ok(PathDirectoryListing::Missing)
            );
        } else {
            assert!(matches!(
                &observed.result().as_ref().as_ref().unwrap_err().kind,
                HostRepositoryDirectoryListingErrorKind::WrongKind {
                    actual: PathNodeKind::RegularFile,
                    ..
                }
            ));
        }
    }

    let mut pending = observed_source_transaction(
        &dice,
        material("dep"),
        PathObservationEpoch::empty(),
        None,
    )
    .await;
    let pending = pending.compute(&key).await.unwrap();
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));
    assert!(!HostRepositoryDirectoryListingObservationKey::validity(
        &pending
    ));
    assert!(!HostRepositoryDirectoryListingObservationKey::equality(
        &pending, &pending
    ));

    let absent_materialization = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = absent_materialization.updater().commit().await;
    let pending = transaction.compute(&key).await.unwrap();
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));
}

#[test]
fn routed_directory_listing_key_is_route_and_package_exact() {
    let hash = |key: &HostRepositoryDirectoryListingKey| {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    };
    let root = HostRepositoryDirectoryListingKey::new(local_route(), PackagePath::root());
    let nested = HostRepositoryDirectoryListingKey::new(
        local_route(),
        PackagePath::parse("pkg").unwrap(),
    );
    let restored = HostRepositoryDirectoryListingKey::new(local_route(), PackagePath::root());
    let other_route = HostRepositoryDirectoryListingKey::new(
        local_route_with_path("other"),
        PackagePath::root(),
    );
    assert_eq!(root, restored);
    assert_eq!(hash(&root), hash(&restored));
    assert_ne!(root, nested);
    assert_ne!(root, other_route);
}

#[tokio::test]
async fn routed_directory_listing_covers_immutable_and_generated_materializations() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (route, root, instance, package) in [
        (
            immutable_route(),
            "/generation/81",
            PathObservationInstanceId::new(81),
            PackagePath::parse("pkg").unwrap(),
        ),
        (
            generated_listing_route(b"exports_files([])\n"),
            "/generation/82",
            PathObservationInstanceId::new(82),
            PackagePath::root(),
        ),
    ] {
        let namespace = PathObservationNamespace::Materialization(instance);
        let path = if package.as_str().is_empty() {
            root.to_owned()
        } else {
            format!("{root}/{}", package.as_str())
        };
        let epoch = directory_epoch(
            namespace,
            &path,
            &[("child", PathDirectoryEntryKind::Directory)],
        );
        let materialization = route_immutable_material(&route, "listing-content", root, instance);
        let mut transaction =
            observed_source_transaction(&dice, materialization, epoch.dupe(), None).await;
        let value = transaction
            .compute(&HostRepositoryDirectoryListingObservationKey::new(
                route, package,
            ))
            .await
            .unwrap();
        let observed = complete_observed_listing(&value);
        assert_eq!(
            listing_names(observed.result().as_ref().as_ref().unwrap()),
            [("child".to_owned(), PathDirectoryEntryKind::Directory)]
        );
        assert_selected_epoch(&mut transaction, &epoch, observed.observations()).await;
    }

    let generated_a = generated_listing_route(b"exports_files(['a'])\n");
    let generated_b = generated_listing_route(b"exports_files(['b'])\n");
    let request = |route: &RootRepositoryRoute| {
        let HostRepositoryMaterializationDisposition::Request(request) =
            host_repository_materialization_request(&route.source_capability()).unwrap()
        else {
            panic!("generated route must materialize")
        };
        request
    };
    assert_ne!(request(&generated_a), request(&generated_b));
}

#[test]
fn routed_directory_listing_observed_outer_and_need_remain_transient() {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/physical/private").unwrap(),
        PathObservationOperation::DirectoryEntries,
    );
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: demand.dupe(),
            result_operation: PathObservationOperation::Lstat,
        },
    );
    let projected = finish_observed_host_repository_directory_listing(
        PathOutcome::Complete(Err(outer.dupe())),
        Arc::new(PackagePath::root()),
    );
    assert!(matches!(
        projected,
        SourcePreparationOutcome::Complete(Err(error)) if error == outer
    ));
    let pending = finish_observed_host_repository_directory_listing(
        PathOutcome::Need(NeedPathObservations::singleton(demand)),
        Arc::new(PackagePath::root()),
    );
    assert!(matches!(pending, SourcePreparationOutcome::Need(_)));
}

#[tokio::test]
async fn routed_directory_listing_tracks_materialization_identity_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let route = immutable_route();
    let key = HostRepositoryDirectoryListingObservationKey::new(
        route.clone(),
        PackagePath::root(),
    );
    let cases = [
        ("source-a", "/generation/a", 91, "a"),
        ("source-b", "/generation/b", 92, "b"),
        ("source-a", "/generation/a", 91, "a"),
    ];
    let mut results = Vec::new();
    for (source, root, instance, child) in cases {
        let instance = PathObservationInstanceId::new(instance);
        let materialization = route_immutable_material(&route, source, root, instance);
        let epoch = directory_epoch(
            PathObservationNamespace::Materialization(instance),
            root,
            &[(child, PathDirectoryEntryKind::File)],
        );
        let mut transaction =
            observed_source_transaction(&dice, materialization, epoch, None).await;
        let value = transaction.compute(&key).await.unwrap();
        results.push(
            complete_observed_listing(&value)
                .result()
                .as_ref()
                .clone(),
        );
    }
    assert_ne!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[tokio::test]
async fn routed_directory_listing_restores_local_create_delete_and_symlink_retarget() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root_key = HostRepositoryDirectoryListingObservationKey::new(
        local_route(),
        PackagePath::root(),
    );
    let present = directory_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep",
        &[("a", PathDirectoryEntryKind::File)],
    );
    let mut first =
        observed_source_transaction(&dice, material("dep"), present.dupe(), None).await;
    let first = first.compute(&root_key).await.unwrap();
    let first = complete_observed_listing(&first).result().dupe();
    let missing = host_path_epoch(
        PathObservationNamespace::Host,
        "/workspace/dep",
        None,
        None,
    );
    let mut deleted = observed_source_transaction(&dice, material("dep"), missing, None).await;
    let deleted = deleted.compute(&root_key).await.unwrap();
    assert_eq!(
        complete_observed_listing(&deleted).result().as_ref(),
        &Ok(PathDirectoryListing::Missing)
    );
    let mut restored =
        observed_source_transaction(&dice, material("dep"), present, None).await;
    let restored = restored.compute(&root_key).await.unwrap();
    assert_eq!(complete_observed_listing(&restored).result(), &first);

    let link_key = HostRepositoryDirectoryListingObservationKey::new(
        local_route(),
        PackagePath::parse("link").unwrap(),
    );
    for (target, entry) in [("/physical/a", "a"), ("/physical/b", "b")] {
        let epoch = symlink_directory_epoch("/workspace/dep/link", target, entry);
        let mut transaction =
            observed_source_transaction(&dice, material("dep"), epoch.dupe(), None).await;
        let value = transaction.compute(&link_key).await.unwrap();
        let observed = complete_observed_listing(&value);
        assert_eq!(
            listing_names(observed.result().as_ref().as_ref().unwrap()),
            [(entry.to_owned(), PathDirectoryEntryKind::File)]
        );
        assert_selected_epoch(&mut transaction, &epoch, observed.observations()).await;
    }
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
        observed_horizon_lookup(
            ExternalRepositoryPackageLookup::IgnoredDirectory,
            q_epoch,
        ),
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

fn module_source_complete(
    value: &<ModuleSourcePreparationObservationKey as Key>::Value,
) -> &ObservedModuleSourcePreparation {
    let SourcePreparationOutcome::Complete(Ok(observed)) = value else {
        panic!("observed module source must complete semantically")
    };
    observed
}

#[tokio::test]
async fn observed_module_source_identity_projection_and_empty_prefix_are_exact() {
    let observed_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("relative"),
        "dep".into(),
        "1.0".into(),
    );
    let legacy_key = ModuleSourcePreparationKey {
        workspace: PathBuf::from("relative"),
        module_name: "dep".into(),
        version: "1.0".into(),
    };
    let other = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("relative"),
        "other".into(),
        "1.0".into(),
    );
    assert_ne!(observed_key, other);
    assert_ne!(test_hash(&observed_key), test_hash(&other));
    assert_ne!(observed_key.to_string(), legacy_key.to_string());

    let need: <ModuleSourcePreparationObservationKey as Key>::Value =
        SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
                PathObservationOperation::FileBytes,
            )),
        ));
    assert!(!ModuleSourcePreparationObservationKey::validity(&need));
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &need, &need
    ));
    let outer: <ModuleSourcePreparationObservationKey as Key>::Value =
        SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::DuplicateDemand(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
                    PathObservationOperation::FileBytes,
                ),
            ),
        )));
    assert!(ModuleSourcePreparationObservationKey::validity(&outer));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &outer, &outer
    ));

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = dice.updater().commit().await;
    let outcome = transaction.compute(&observed_key).await.unwrap();
    let observed = module_source_complete(&outcome);
    assert!(matches!(
        observed.result().as_ref(),
        Err(ModuleSourcePreparationError::RootModuleFiles(_))
    ));
    assert!(observed.observations().observations().is_empty());

    let result = Arc::new(Err(ModuleSourcePreparationError::MissingVersion));
    let projected = project_legacy_module_source(SourcePreparationOutcome::Complete(Ok((
        result.dupe(),
        PathObservationEpoch::empty(),
    ))));
    let SourcePreparationOutcome::Complete(projected) = projected else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&result, &projected));
}

#[test]
fn module_source_merge_preserves_first_arcs_and_rejects_conflicts() {
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/patch.diff").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let first_result = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"patch".as_slice())),
    ));
    let equal_result = Arc::new(first_result.as_ref().clone());
    let first =
        PathObservationEpoch::from_shared([(demand.dupe(), first_result.dupe())]).unwrap();
    let equal = PathObservationEpoch::from_shared([(demand.dupe(), equal_result)]).unwrap();
    let (accepted, merged) = finish_module_source_observed_child(
        SourcePreparationOutcome::Complete(Ok(equal)),
        first.dupe(),
        |epoch| epoch,
    )
    .unwrap();
    assert_eq!(accepted, merged);
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first_result));

    let conflict = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(b"other".as_slice())),
        )),
    )])
    .unwrap();
    assert!(matches!(
        finish_module_source_observed_child(
            SourcePreparationOutcome::Complete(Ok(conflict)),
            first.dupe(),
            |epoch| epoch,
        ),
        Err(SourcePreparationOutcome::Complete(Err(
            ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::ConflictingDemand(_)
            )
        )))
    ));
    let mismatch = PathObservationEpoch::from_shared([(
        demand.dupe(),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
    )]);
    assert!(matches!(
        mismatch,
        Err(slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            ..
        })
    ));
    let need = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand.dupe()));
    assert!(matches!(
        finish_module_source_observed_child::<PathObservationEpoch>(
            SourcePreparationOutcome::Need(need),
            first.dupe(),
            |epoch| epoch,
        ),
        Err(SourcePreparationOutcome::Need(_))
    ));
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand.dupe()),
    );
    assert!(matches!(
        finish_module_source_observed_child::<PathObservationEpoch>(
            SourcePreparationOutcome::Complete(Err(outer)),
            first.dupe(),
            |epoch| epoch,
        ),
        Err(SourcePreparationOutcome::Complete(Err(
            ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::DuplicateDemand(_)
            )
        )))
    ));

    let a = SourcePreparationOutcome::Complete(Ok(ObservedModuleSourcePreparation {
        result: Arc::new(Err(ModuleSourcePreparationError::MissingVersion)),
        observations: first,
    }));
    assert!(ModuleSourcePreparationObservationKey::equality(&a, &a));
    let same_result_new_epoch = SourcePreparationOutcome::Complete(Ok(
        ObservedModuleSourcePreparation {
            result: module_source_complete(&a).result().dupe(),
            observations: PathObservationEpoch::empty(),
        },
    ));
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &a,
        &same_result_new_epoch
    ));
}

#[test]
fn module_source_stage_projectors_preserve_exact_prefixes_and_outer_classes() {
    let path = NormalizedAbsolutePath::new("/workspace/patch.diff").unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        path.dupe(),
        PathObservationOperation::FileBytes,
    );
    let result = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"patch".as_slice())),
    ));
    let prefix = PathObservationEpoch::from_shared([(demand.dupe(), result.dupe())]).unwrap();
    for error in [
        ModuleSourcePreparationError::RootModuleFiles("effective".into()),
        ModuleSourcePreparationError::SourceCompute(Arc::from("source")),
        ModuleSourcePreparationError::RegistryPolicyCompute("policy".into()),
        ModuleSourcePreparationError::RegistryFileCompute {
            url: RegistryFileUrl::new("https://registry/modules/dep/1/MODULE.bazel"),
            prior_not_found_attempts: Arc::from([]),
            message: "registry".into(),
        },
        ModuleSourcePreparationError::PatchResolutionCompute {
            logical_path: path.dupe(),
            message: "resolution".into(),
        },
        ModuleSourcePreparationError::PatchFileCompute {
            demand: demand.dupe(),
            message: "file".into(),
        },
    ] {
        let SourcePreparationOutcome::Complete(Ok((actual, observations))) =
            module_source_error(error.clone(), prefix.dupe())
        else {
            panic!("compute errors must complete semantically")
        };
        assert_eq!(actual.as_ref(), &Err(error));
        assert_exact_epoch(&prefix, &observations);
    }

    let mismatch = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: demand.dupe(),
            result_operation: PathObservationOperation::Lstat,
        },
    );
    assert!(matches!(
        finish_module_source_observed_path::<PathObservationEpoch>(
            PathOutcome::Complete(Err(mismatch)),
            prefix.dupe(),
            |epoch| epoch,
        ),
        Err(SourcePreparationOutcome::Complete(Err(
            ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. }
            )
        )))
    ));

    let before = Some(PathLstat::new(
        PathNodeKind::RegularFile,
        1,
        2,
        3,
        4,
        0o644,
    ));
    for (result, expected) in [
        (
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Missing)),
            "missing",
        ),
        (
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Error(
                PathObservationError::NotALink,
            ))),
            "error",
        ),
    ] {
        let Err(SourcePreparationOutcome::Complete(Ok((actual, observations)))) =
            finish_module_source_patch_file(
                demand.dupe(),
                result.dupe(),
                before,
                PathObservationEpoch::empty(),
                true,
            )
        else {
            panic!("patch terminal must complete semantically")
        };
        assert!(matches!(
            (expected, actual.as_ref()),
            ("missing", Err(ModuleSourcePreparationError::PatchFileInconsistentState { .. }))
                | ("error", Err(ModuleSourcePreparationError::PatchFileObservation { .. }))
        ));
        assert!(Arc::ptr_eq(observations.get(&demand).unwrap(), &result));
    }
    assert!(matches!(
        finish_module_source_patch_file(
            demand,
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
            before,
            PathObservationEpoch::empty(),
            true,
        ),
        Err(SourcePreparationOutcome::Complete(Err(
            ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch { .. }
            )
        )))
    ));
}

async fn observed_nonregistry_module_source(
    dice: &Arc<Dice>,
    tracker: Arc<NonregistryPreflightTracker>,
    source: &[u8],
) -> (
    dice::DiceTransaction,
    <ModuleSourcePreparationObservationKey as Key>::Value,
) {
    let transaction = host_nonregistry_transaction(
        dice,
        Some(source),
        None,
        &[],
        &[],
        701,
        None,
        Some(tracker),
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    let value = transaction
        .compute(&ModuleSourcePreparationObservationKey::new(
            PathBuf::from("/workspace"),
            "dep".into(),
            "1".into(),
        ))
        .await
        .unwrap();
    (transaction, value)
}

#[tokio::test]
async fn observed_module_source_nonregistry_rows_events_parity_and_lifecycle_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let source_a = b"module(name='dep',version='1')\n";
    let (mut transaction, cold) =
        observed_nonregistry_module_source(&dice, tracker.dupe(), source_a).await;
    let observed = module_source_complete(&cold);
    assert!(matches!(
        observed.result().as_ref(),
        Ok(ModuleSourcePreparation::NonRegistry { bytes }) if bytes.as_ref() == source_a
    ));
    let held_result = observed.result().dupe();
    let held_epoch = observed.observations().dupe();

    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );
    let source_key = RepositorySourceFileObservationKey(RepositorySourceFileKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        repo_relative_path: PathBuf::from("MODULE.bazel"),
    });
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("observed effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(source)) =
        transaction.compute(&source_key).await.unwrap()
    else {
        panic!("observed source carrier")
    };
    let expected =
        merge_path_observations(effective.observations(), source.observations()).unwrap();
    assert_exact_epoch(&expected, &held_epoch);
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner.starts_with("observed-module-source-preparation:"))
            .unwrap()
            .1,
        vec![effective_key.to_string(), source_key.to_string()]
    );
    let cold_batches = std::mem::take(&mut *tracker.batches.lock().unwrap());
    assert!(cold_batches.iter().any(|(owner, kind, batch)| {
        owner.starts_with("observed-module-source-preparation:")
            && *kind == ActivationKind::Evaluated
            && batch.is_none()
    }));
    assert!(cold_batches.iter().all(|(owner, _, batch)| {
        !owner.starts_with("observed-module-source-preparation:") || batch.is_none()
    }));
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
        [(owner, events)]
            if owner.starts_with("bzlmod-observed-host-root-module-file:")
                && events.is_empty()
    ));

    let warm = transaction
        .compute(&ModuleSourcePreparationObservationKey::new(
            PathBuf::from("/workspace"),
            "dep".into(),
            "1".into(),
        ))
        .await
        .unwrap();
    assert!(ModuleSourcePreparationObservationKey::equality(&cold, &warm));
    assert!(Arc::ptr_eq(
        &held_result,
        module_source_complete(&warm).result()
    ));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(_, _, batch)| batch.is_none()));

    let legacy_key = ModuleSourcePreparationKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        version: "1".into(),
    };
    let SourcePreparationOutcome::Complete(legacy) =
        transaction.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy preparation must complete")
    };
    assert_eq!(legacy.as_ref(), held_result.as_ref());
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &legacy_key.to_string())
            .unwrap()
            .1,
        vec![
            HostEffectiveModuleOverrideKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                "dep".into(),
            )
            .to_string(),
            RepositorySourceFileKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
                repo_relative_path: PathBuf::from("MODULE.bazel"),
            }
            .to_string(),
        ]
    );

    let (_, changed) =
        observed_nonregistry_module_source(&dice, tracker.dupe(), b"module(name='changed')\n")
            .await;
    let (mut restored_tx, restored) = observed_nonregistry_module_source(&dice, tracker.dupe(), source_a).await;
    assert!(!ModuleSourcePreparationObservationKey::equality(&cold, &changed));
    assert!(ModuleSourcePreparationObservationKey::equality(&cold, &restored));
    assert_eq!(held_result.as_ref(), module_source_complete(&restored).result().as_ref());
    let restored = module_source_complete(&restored);
    assert_eq!(&held_epoch, restored.observations());
    assert!(!held_epoch.observations().is_empty());
    for forbidden in [
        "host-discovered-module:",
        "host-selected-module-graph:",
        "host-selected-repo",
        "host-registry-",
        "host-selected-extension-",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, deps)| {
            !owner.starts_with(forbidden) && deps.iter().all(|dep| !dep.starts_with(forbidden))
        }));
    }
    assert_selected_epoch(&mut restored_tx, restored.observations(), restored.observations())
        .await;
}

#[tokio::test]
async fn observed_module_source_nonregistry_absent_and_error_retain_source_prefix() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let transaction = host_nonregistry_transaction(
        &dice,
        None,
        None,
        &[],
        &[],
        702,
        None,
        None,
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    let absent = transaction.compute(&key).await.unwrap();
    let absent = module_source_complete(&absent);
    assert!(matches!(
        absent.result().as_ref(),
        Err(ModuleSourcePreparationError::ModuleNotFound { module_file_attempts })
            if module_file_attempts.is_empty()
    ));
    assert!(!absent.observations().observations().is_empty());

    let transaction = host_nonregistry_transaction(
        &dice,
        Some(b"module(name='dep',version='1')\n"),
        None,
        &[],
        &[],
        703,
        None,
        None,
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    let error = Arc::new(PathObservationResult::FileBytes(PathOperationResult::Error(
        PathObservationError::NotALink,
    )));
    let mut entries = epoch
        .observations()
        .iter()
        .filter(|(candidate, _)| candidate != &&demand)
        .map(|(candidate, result)| (candidate.dupe(), result.dupe()))
        .collect::<Vec<_>>();
    entries.push((demand.dupe(), error.dupe()));
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::from_shared(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let failed = transaction.compute(&key).await.unwrap();
    let failed = module_source_complete(&failed);
    assert!(matches!(
        failed.result().as_ref(),
        Err(ModuleSourcePreparationError::Source(
            RepositorySourceFileError::Observation {
                operation: PathObservationOperation::FileBytes,
                error: PathObservationError::NotALink,
                ..
            }
        ))
    ));
    assert!(Arc::ptr_eq(failed.observations().get(&demand).unwrap(), &error));
}

#[derive(Clone)]
enum ModuleSourceRegistryResponse {
    Found(Arc<[u8]>),
    NotFound,
    Error(&'static str),
}

struct ModuleSourceRegistryIo {
    responses: Mutex<std::collections::BTreeMap<String, ModuleSourceRegistryResponse>>,
    calls: Mutex<Vec<String>>,
}

impl ModuleSourceRegistryIo {
    fn new(
        responses: impl IntoIterator<Item = (impl Into<String>, ModuleSourceRegistryResponse)>,
    ) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(url, response)| (url.into(), response))
                    .collect(),
            ),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl crate::RegistryIo for ModuleSourceRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
        self.calls.lock().unwrap().push(url.as_str().to_owned());
        match self.responses.lock().unwrap().get(url.as_str()).cloned() {
            Some(ModuleSourceRegistryResponse::Found(bytes)) => {
                Ok(crate::RegistryIoOutcome::Found(bytes))
            }
            Some(ModuleSourceRegistryResponse::NotFound) | None => {
                Ok(crate::RegistryIoOutcome::NotFound)
            }
            Some(ModuleSourceRegistryResponse::Error(message)) => {
                Err(crate::RegistryTransportError {
                    message: message.into(),
                })
            }
        }
    }
}

async fn module_source_registry_transaction(
    dice: &Arc<Dice>,
    tracker: Arc<NonregistryPreflightTracker>,
    root_source: &str,
    registries: &[&str],
    generation: u64,
    extra: PathObservationEpoch,
) -> dice::DiceTransaction {
    let transaction = host_nonregistry_transaction(
        dice,
        None,
        None,
        &[],
        &[],
        711,
        None,
        Some(tracker),
        None,
        true,
    )
    .await;
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            slug_workspace_v2::WorkspaceSnapshotKey {
                workspace: PathBuf::from("/workspace"),
            },
            Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel"),
                    slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                        root_source.to_owned(),
                    )),
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            merge_path_observations(&horizon_epoch(
                root_source,
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
                711,
            ), &extra).unwrap(),
        )])
        .unwrap();
    crate::inject_registry_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        crate::RegistryUrls::new(registries.iter().copied()),
        crate::RegistryRequestGeneration(generation),
    )
    .unwrap();
    complete_preflight_transaction(updater.commit().await, false).await
}

async fn module_source_lockfile_transaction(
    mut transaction: dice::DiceTransaction,
    mode: crate::LockfileMode,
    bytes: Option<&[u8]>,
) -> dice::DiceTransaction {
    let lockfile_path = NormalizedAbsolutePath::new("/workspace/MODULE.bazel.lock").unwrap();
    let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
    let mut entries = epoch
        .observations()
        .iter()
        .filter(|(demand, _)| demand.path() != &lockfile_path)
        .map(|(demand, result)| (demand.dupe(), result.dupe()))
        .collect::<Vec<_>>();
    entries.push((
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            lockfile_path.dupe(),
            PathObservationOperation::Lstat,
        ),
        Arc::new(PathObservationResult::Lstat(match bytes {
            Some(_) => PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                2,
                3,
                4,
                0o644,
            )),
            None => PathOperationResult::Missing,
        })),
    ));
    if let Some(bytes) = bytes {
        entries.push((
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                lockfile_path,
                PathObservationOperation::FileBytes,
            ),
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Present(
                Arc::from(bytes),
            ))),
        ));
    }
    let mut updater = transaction.into_updater();
    updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::from_shared(entries).unwrap())]).unwrap();
    updater.changed_to(vec![(crate::RootModuleLockfileModeKey { workspace: PathBuf::from("/workspace") }, crate::RootModuleLockfileMode::from(mode))]).unwrap();
    updater.changed_to(vec![(slug_workspace_v2::WorkspaceRawSnapshotKey { workspace: PathBuf::from("/workspace") }, Arc::new(slug_workspace_v2::WorkspaceRawSnapshot { files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(PathBuf::from("/workspace/MODULE.bazel.lock"), bytes.map(|bytes| slug_workspace_v2::WorkspaceRawFileValue::Present(Arc::from(bytes))).unwrap_or(slug_workspace_v2::WorkspaceRawFileValue::Absent))])) }))]).unwrap();
    updater.commit().await
}

async fn observed_registry_file_carrier(
    transaction: &mut dice::DiceTransaction,
    url: &str,
) -> crate::registry_dice::ObservedRegistryFile {
    let value = transaction
        .compute(&RegistryFileObservationKey::new(
            PathBuf::from("/workspace"),
            RegistryFileUrl::new(url),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("registry file must complete")
    };
    value
}

#[tokio::test]
async fn observed_module_source_registry_attempts_prefix_rows_and_errors_are_exact() {
    let first = "https://first.invalid/modules/dep/1/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1/MODULE.bazel";
    let bytes: Arc<[u8]> = Arc::from(b"module(name='dep',version='1')\n".as_slice());
    let io = Arc::new(ModuleSourceRegistryIo::new([
        (first, ModuleSourceRegistryResponse::NotFound),
        (second, ModuleSourceRegistryResponse::Found(bytes.dupe())),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://first.invalid", "https://second.invalid"],
        1,
        PathObservationEpoch::empty(),
    )
    .await;
    let observed_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let cold = transaction.compute(&observed_key).await.unwrap();
    let observed = module_source_complete(&cold);
    assert!(matches!(
        observed.result().as_ref(),
        Ok(ModuleSourcePreparation::Registry {
            bytes: actual,
            selected_registry,
            module_file_attempts,
        }) if actual == &bytes
            && selected_registry.as_str() == "https://second.invalid"
            && module_file_attempts.len() == 2
            && module_file_attempts[0].url.as_str() == first
            && module_file_attempts[0].sha256.is_none()
            && module_file_attempts[1].url.as_str() == second
            && module_file_attempts[1].sha256.is_some()
    ));
    assert_eq!(io.calls.lock().unwrap().as_slice(), [first, second]);
    let observed_events = std::mem::take(&mut *tracker.batches.lock().unwrap())
        .into_iter()
        .filter_map(|(_, kind, batch)| {
            (kind == ActivationKind::Evaluated).then_some(batch).flatten()
        })
        .collect::<Vec<_>>();
    assert_eq!(observed_events.len(), 1);
    assert!(observed_events[0].events().is_empty());
    let warm = transaction.compute(&observed_key).await.unwrap();
    assert!(ModuleSourcePreparationObservationKey::equality(&cold, &warm));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(_, _, batch)| batch.is_none()));
    tracker.batches.lock().unwrap().clear();

    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );
    let policy_key = RegistryPolicyObservationKey::new(PathBuf::from("/workspace"));
    let first_key = RegistryFileObservationKey::new(
        PathBuf::from("/workspace"),
        RegistryFileUrl::new(first),
    );
    let second_key = RegistryFileObservationKey::new(
        PathBuf::from("/workspace"),
        RegistryFileUrl::new(second),
    );
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &observed_key.to_string())
            .unwrap()
            .1,
        vec![
            effective_key.to_string(),
            policy_key.to_string(),
            first_key.to_string(),
            second_key.to_string(),
        ]
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(policy)) =
        transaction.compute(&policy_key).await.unwrap()
    else {
        panic!("policy carrier")
    };
    let first_file = observed_registry_file_carrier(&mut transaction, first).await;
    let second_file = observed_registry_file_carrier(&mut transaction, second).await;
    let expected = merge_path_observations(effective.observations(), policy.observations()).unwrap();
    let expected = merge_path_observations(&expected, first_file.observations()).unwrap();
    let expected = merge_path_observations(&expected, second_file.observations()).unwrap();
    assert_exact_epoch(&expected, observed.observations());
    let legacy_key = ModuleSourcePreparationKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        version: "1".into(),
    };
    let SourcePreparationOutcome::Complete(legacy) =
        transaction.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy registry preparation must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &legacy_key.to_string())
            .unwrap()
            .1,
        vec![
            HostEffectiveModuleOverrideKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                "dep".into(),
            )
            .to_string(),
            RegistryPolicyKey { workspace: PathBuf::from("/workspace") }.to_string(),
            RegistryFileKey { workspace: PathBuf::from("/workspace"), url: RegistryFileUrl::new(first) }.to_string(),
            RegistryFileKey { workspace: PathBuf::from("/workspace"), url: RegistryFileUrl::new(second) }.to_string(),
        ]
    );
    let legacy_events = std::mem::take(&mut *tracker.batches.lock().unwrap())
        .into_iter()
        .filter_map(|(_, kind, batch)| (kind == ActivationKind::Evaluated).then_some(batch).flatten())
        .collect::<Vec<_>>();
    assert_eq!(observed_events, legacy_events);
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        !owner.starts_with("observed-module-source-preparation:") || batch.is_none()
    }));
}

#[tokio::test]
async fn observed_module_source_registry_errors_exhaustion_and_override_are_exact() {
    let first = "https://first.invalid/modules/dep/1/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1/MODULE.bazel";
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let observed_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let error_io = Arc::new(ModuleSourceRegistryIo::new([
        (first, ModuleSourceRegistryResponse::NotFound),
        (second, ModuleSourceRegistryResponse::Error("transport")),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, error_io);
    let error_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let mut error_tx = module_source_registry_transaction(
        &error_dice,
        Arc::new(NonregistryPreflightTracker::default()),
        root,
        &["https://first.invalid", "https://second.invalid"],
        2,
        PathObservationEpoch::empty(),
    )
    .await;
    let error = error_tx.compute(&observed_key).await.unwrap();
    let error = module_source_complete(&error);
    assert!(matches!(
        error.result().as_ref(),
        Err(ModuleSourcePreparationError::RegistryFile {
            url,
            prior_not_found_attempts,
            error: RegistryFileError::Transport { message, .. },
        }) if url.as_str() == second
            && prior_not_found_attempts.len() == 1
            && message.as_str() == "transport"
    ));
    assert!(!error.observations().observations().is_empty());

    let missing_io = Arc::new(ModuleSourceRegistryIo::new([
        (first, ModuleSourceRegistryResponse::NotFound),
        (second, ModuleSourceRegistryResponse::NotFound),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, missing_io);
    let missing_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let mut missing_tx = module_source_registry_transaction(
        &missing_dice,
        Arc::new(NonregistryPreflightTracker::default()),
        root,
        &["https://first.invalid", "https://second.invalid"],
        3,
        PathObservationEpoch::empty(),
    )
    .await;
    let missing = missing_tx.compute(&observed_key).await.unwrap();
    let missing = module_source_complete(&missing);
    assert!(matches!(
        missing.result().as_ref(),
        Err(ModuleSourcePreparationError::ModuleNotFound { module_file_attempts })
            if module_file_attempts.len() == 2
                && module_file_attempts.iter().all(|attempt| attempt.sha256.is_none())
    ));

    let override_url = "https://override.invalid/modules/dep/9/MODULE.bazel";
    let default_url = "https://default.invalid/modules/dep/9/MODULE.bazel";
    let override_io = Arc::new(ModuleSourceRegistryIo::new([
        (
            override_url,
            ModuleSourceRegistryResponse::Found(Arc::from(b"override".as_slice())),
        ),
        (default_url, ModuleSourceRegistryResponse::Error("must not run")),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, override_io.dupe());
    let override_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let override_root = "module(name='root')\n\
        bazel_dep(name='dep',version='1')\n\
        single_version_override(module_name='dep',version='9',registry='https://override.invalid')\n";
    let mut override_tx = module_source_registry_transaction(
        &override_dice,
        Arc::new(NonregistryPreflightTracker::default()),
        override_root,
        &["https://default.invalid"],
        4,
        PathObservationEpoch::empty(),
    )
    .await;
    let override_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "9".into(),
    );
    let override_value = override_tx.compute(&override_key).await.unwrap();
    assert!(matches!(
        module_source_complete(&override_value).result().as_ref(),
        Ok(ModuleSourcePreparation::Registry {
            bytes,
            selected_registry,
            module_file_attempts,
        }) if bytes.as_ref() == b"override"
            && selected_registry.as_str() == "https://override.invalid"
            && module_file_attempts.len() == 1
            && module_file_attempts[0].url.as_str() == override_url
    ));
    assert_eq!(override_io.calls.lock().unwrap().as_slice(), [override_url]);
}

#[tokio::test]
async fn observed_module_source_registry_policy_url_restores_a_b_a_with_held_carriers() {
    let first_base = "https://first.invalid";
    let second_base = "https://second.invalid";
    let first = "https://first.invalid/modules/dep/1/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1/MODULE.bazel";
    let bytes: Arc<[u8]> = Arc::from(b"module(name='dep',version='1')\n".as_slice());
    let io = Arc::new(ModuleSourceRegistryIo::new([
        (first, ModuleSourceRegistryResponse::Found(bytes.dupe())),
        (second, ModuleSourceRegistryResponse::Found(bytes)),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let mut values = Vec::new();
    for (generation, registry) in [(41, first_base), (42, second_base), (43, first_base)] {
        let mut transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root,
            &[registry],
            generation,
            PathObservationEpoch::empty(),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = module_source_complete(&value);
        assert!(matches!(
            observed.result().as_ref(),
            Ok(ModuleSourcePreparation::Registry { selected_registry, .. })
                if selected_registry.as_str() == registry
        ));
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        values.push(value);
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(&values[0], &values[1]));
    assert!(ModuleSourcePreparationObservationKey::equality(&values[0], &values[2]));
    let held = module_source_complete(&values[0]);
    let restored = module_source_complete(&values[2]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());
    assert!(!held.observations().observations().is_empty());
}

#[tokio::test]
async fn observed_module_source_registry_bytes_and_module_epoch_restore_independently() {
    let module_url = "https://registry.invalid/modules/dep/1/MODULE.bazel";
    let bytes_a: Arc<[u8]> = Arc::from(b"module(name='dep',version='1')\n".as_slice());
    let bytes_b: Arc<[u8]> = Arc::from(b"module(name='dep_changed',version='1')\n".as_slice());
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        module_url,
        ModuleSourceRegistryResponse::Found(bytes_a.dupe()),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let root_a = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut values = Vec::new();
    for (generation, bytes) in [(61, bytes_a.dupe()), (62, bytes_b), (63, bytes_a.dupe())] {
        io.responses.lock().unwrap().insert(
            module_url.to_owned(),
            ModuleSourceRegistryResponse::Found(bytes),
        );
        let mut transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root_a,
            &["https://registry.invalid"],
            generation,
            PathObservationEpoch::empty(),
        )
        .await;
        values.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(&values[0], &values[1]));
    assert!(ModuleSourcePreparationObservationKey::equality(&values[0], &values[2]));
    let held = module_source_complete(&values[0]);
    let restored = module_source_complete(&values[2]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());

    io.responses.lock().unwrap().insert(
        module_url.to_owned(),
        ModuleSourceRegistryResponse::Found(bytes_a),
    );
    let root_b = "module(name='root')\n# changed\nbazel_dep(name='dep',version='1')\n";
    let mut roots = Vec::new();
    for (generation, root) in [(64, root_a), (65, root_b), (66, root_a)] {
        let mut transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root,
            &["https://registry.invalid"],
            generation,
            PathObservationEpoch::empty(),
        )
        .await;
        roots.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(&roots[0], &roots[1]));
    assert!(ModuleSourcePreparationObservationKey::equality(&roots[0], &roots[2]));
    let held = module_source_complete(&roots[0]);
    let restored = module_source_complete(&roots[2]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());

    let lockfile = br#"{"lockFileVersion":28}"#;
    let mut lockfiles = Vec::new();
    for (generation, mode, bytes) in [
        (67, crate::LockfileMode::Update, None),
        (68, crate::LockfileMode::Error, None),
        (69, crate::LockfileMode::Update, None),
        (70, crate::LockfileMode::Update, Some(lockfile.as_slice())),
        (71, crate::LockfileMode::Update, None),
    ] {
        let transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root_a,
            &["https://registry.invalid"],
            generation,
            PathObservationEpoch::empty(),
        )
        .await;
        let mut transaction = module_source_lockfile_transaction(transaction, mode, bytes).await;
        lockfiles.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &lockfiles[0], &lockfiles[1]
    ));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &lockfiles[0], &lockfiles[2]
    ));
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &lockfiles[2], &lockfiles[3]
    ));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &lockfiles[2], &lockfiles[4]
    ));
    let held = module_source_complete(&lockfiles[0]);
    let restored = module_source_complete(&lockfiles[4]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());
}

#[tokio::test]
async fn observed_module_source_resolves_all_patches_then_stops_after_decisive_apply_error() {
    let module_url = "file:///registry/modules/dep/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        module_url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='dep',version='1')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let first_path = "/workspace/first.patch";
    let second_path = "/workspace/second.patch";
    let patch_lstat = || {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644,
        )))
    };
    let patch_demand = |path, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let extra = PathObservationEpoch::from_shared([
        (patch_demand(first_path, PathObservationOperation::Lstat), Arc::new(patch_lstat())),
        (
            patch_demand(first_path, PathObservationOperation::FileBytes),
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Present(
                Arc::from(b"not a patch".as_slice()),
            ))),
        ),
        (patch_demand(second_path, PathObservationOperation::Lstat), Arc::new(patch_lstat())),
    ])
    .unwrap();
    let root = "module(name='root')\n\
        bazel_dep(name='dep',version='1')\n\
        single_version_override(module_name='dep',version='1',registry='file:///registry',patches=['//:first.patch','//:second.patch'],patch_strip=1)\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["file:///registry"],
        1,
        extra,
    )
    .await;
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let value = transaction.compute(&key).await.unwrap();
    let observed = module_source_complete(&value);
    assert!(matches!(
        observed.result().as_ref(),
        Err(ModuleSourcePreparationError::Patch(_))
    ));
    let first = NormalizedAbsolutePath::new(first_path).unwrap();
    let second = NormalizedAbsolutePath::new(second_path).unwrap();
    let first_resolution = ResolvedPathObservationKey::new(
        PathObservationNamespace::Host,
        first.dupe(),
    );
    let second_resolution = ResolvedPathObservationKey::new(
        PathObservationNamespace::Host,
        second.dupe(),
    );
    let first_bytes_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        first,
        PathObservationOperation::FileBytes,
    );
    let first_bytes = PathObservationKey::new(first_bytes_demand.dupe());
    let second_bytes = PathObservationKey::new(PathObservationDemand::new(
        PathObservationNamespace::Host,
        second,
        PathObservationOperation::FileBytes,
    ));
    let row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    let first_resolution_position = row
        .iter()
        .position(|dep| dep == &first_resolution.to_string())
        .unwrap();
    let second_resolution_position = row
        .iter()
        .position(|dep| dep == &second_resolution.to_string())
        .unwrap();
    let first_bytes_position = row
        .iter()
        .position(|dep| dep == &first_bytes.to_string())
        .unwrap();
    assert!(first_resolution_position < second_resolution_position);
    assert!(second_resolution_position < first_bytes_position);
    assert!(!row.contains(&second_bytes.to_string()));
    assert_eq!(
        row,
        vec![
            HostEffectiveModuleOverrideObservationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                "dep".into(),
            )
            .to_string(),
            RegistryPolicyObservationKey::new(PathBuf::from("/workspace")).to_string(),
            RegistryFileObservationKey::new(
                PathBuf::from("/workspace"),
                RegistryFileUrl::new(module_url),
            )
            .to_string(),
            first_resolution.to_string(),
            second_resolution.to_string(),
            first_bytes.to_string(),
        ]
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) = transaction
        .compute(&HostEffectiveModuleOverrideObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            "dep".into(),
        ))
        .await
        .unwrap()
    else {
        panic!("effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(policy)) = transaction
        .compute(&RegistryPolicyObservationKey::new(PathBuf::from("/workspace")))
        .await
        .unwrap()
    else {
        panic!("policy carrier")
    };
    let file = observed_registry_file_carrier(&mut transaction, module_url).await;
    let PathOutcome::Complete(Ok(first_resolved)) =
        transaction.compute(&first_resolution).await.unwrap()
    else {
        panic!("first resolution carrier")
    };
    let PathOutcome::Complete(Ok(second_resolved)) =
        transaction.compute(&second_resolution).await.unwrap()
    else {
        panic!("second resolution carrier")
    };
    let PathOutcome::Complete(first_bytes_result) = transaction.compute(&first_bytes).await.unwrap()
    else {
        panic!("first bytes must complete")
    };
    let expected = merge_path_observations(effective.observations(), policy.observations()).unwrap();
    let expected = merge_path_observations(&expected, file.observations()).unwrap();
    let expected = merge_path_observations(&expected, first_resolved.observations()).unwrap();
    let expected = merge_path_observations(&expected, second_resolved.observations()).unwrap();
    let expected = append_host_repository_source_observation(
        &expected, first_bytes_demand.dupe(), first_bytes_result,
    ).unwrap();
    assert_exact_epoch(&expected, observed.observations());
    let legacy_key = ModuleSourcePreparationKey {
        workspace: PathBuf::from("/workspace"),
        module_name: "dep".into(),
        version: "1".into(),
    };
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy patch preparation must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &legacy_key.to_string())
        .unwrap()
        .1
        .clone();
    assert!(legacy_row[0].starts_with("host-effective-module-override:"));
    assert!(legacy_row[1].starts_with("registry-policy:"));
    assert!(legacy_row[2].starts_with("registry-file:"));
    assert!(legacy_row[3].starts_with("resolved-path:"));
    assert!(legacy_row[4].starts_with("resolved-path:"));
    assert_eq!(legacy_row[5], first_bytes.to_string());
    assert!(observed.observations().get(&first_bytes_demand).is_some());
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        owner != &key.to_string() || batch.is_none()
    }));
}

#[tokio::test]
async fn observed_module_source_patch_terminals_stop_at_first_middle_and_last_positions() {
    let module_url = "file:///registry/modules/dep/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        module_url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='dep',version='1')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let paths = ["/workspace/a.patch", "/workspace/b.patch", "/workspace/c.patch"];
    let root = "module(name='root')\n\
        bazel_dep(name='dep',version='1')\n\
        single_version_override(module_name='dep',version='1',registry='file:///registry',patches=['//:a.patch','//:b.patch','//:c.patch'],patch_strip=1)\n";
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let demand = |path: &str, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let regular = || {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::RegularFile,
            1,
            2,
            3,
            4,
            0o644,
        )))
    };

    for (position, terminal) in [
        PathObservationResult::Lstat(PathOperationResult::Missing),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::Directory,
            1,
            2,
            3,
            4,
            0o755,
        ))),
        PathObservationResult::Lstat(PathOperationResult::Error(
            PathObservationError::NotALink,
        )),
    ]
    .into_iter()
    .enumerate()
    {
        let decisive = Arc::new(terminal);
        let observations = PathObservationEpoch::from_shared(
            paths[..=position]
                .iter()
                .enumerate()
                .map(|(index, path)| {
                    (
                        demand(path, PathObservationOperation::Lstat),
                        if index == position {
                            decisive.dupe()
                        } else {
                            Arc::new(regular())
                        },
                    )
                }),
        )
        .unwrap();
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["file:///registry"],
            40 + position as u64,
            observations,
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = module_source_complete(&value);
        assert!(matches!(
            (position, observed.result().as_ref()),
            (0, Err(ModuleSourcePreparationError::PatchMissing { .. }))
                | (1, Err(ModuleSourcePreparationError::PatchWrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                }))
                | (2, Err(ModuleSourcePreparationError::PatchResolution(_)))
        ));
        let decisive_demand = demand(paths[position], PathObservationOperation::Lstat);
        assert!(Arc::ptr_eq(
            observed.observations().get(&decisive_demand).unwrap(),
            &decisive
        ));
        let row = tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &key.to_string())
            .unwrap()
            .1
            .clone();
        assert_eq!(
            row.iter()
                .filter(|dep| dep.starts_with("observed-resolved-path:"))
                .count(),
            position + 1
        );
        assert!(row.iter().all(|dep| !dep.starts_with("path-observation:")));
    }

    for (position, terminal) in [
        PathObservationResult::FileBytes(PathOperationResult::Missing),
        PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::NotALink,
        )),
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
            b"not a patch".as_slice(),
        ))),
    ]
    .into_iter()
    .enumerate()
    {
        let decisive = Arc::new(terminal);
        let mut entries = paths
            .iter()
            .map(|path| {
                (
                    demand(path, PathObservationOperation::Lstat),
                    Arc::new(regular()),
                )
            })
            .collect::<Vec<_>>();
        entries.extend(paths[..=position].iter().enumerate().map(|(index, path)| {
            (
                demand(path, PathObservationOperation::FileBytes),
                if index == position {
                    decisive.dupe()
                } else {
                    Arc::new(PathObservationResult::FileBytes(
                        PathOperationResult::Present(Arc::from([])),
                    ))
                },
            )
        }));
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["file:///registry"],
            50 + position as u64,
            PathObservationEpoch::from_shared(entries).unwrap(),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = module_source_complete(&value);
        assert!(matches!(
            (position, observed.result().as_ref()),
            (0, Err(ModuleSourcePreparationError::PatchFileInconsistentState { .. }))
                | (1, Err(ModuleSourcePreparationError::PatchFileObservation { .. }))
                | (2, Err(ModuleSourcePreparationError::Patch(_)))
        ));
        let decisive_demand = demand(paths[position], PathObservationOperation::FileBytes);
        assert!(Arc::ptr_eq(
            observed.observations().get(&decisive_demand).unwrap(),
            &decisive
        ));
        let row = tracker.rows.lock().unwrap().last().unwrap().1.clone();
        assert_eq!(
            row.iter()
                .filter(|dep| dep.starts_with("path-observation:"))
                .count(),
            position + 1
        );
    }
}

fn module_source_patch_epoch(bytes: Arc<[u8]>, inode: i64) -> PathObservationEpoch {
    let path = NormalizedAbsolutePath::new("/workspace/route.patch").unwrap();
    PathObservationEpoch::from_shared([
        (
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                path.dupe(),
                PathObservationOperation::Lstat,
            ),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, inode, 0o644),
            ))),
        ),
        (
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                path,
                PathObservationOperation::FileBytes,
            ),
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Present(bytes))),
        ),
    ])
    .unwrap()
}

fn module_source_patch(leaf: &str) -> Arc<[u8]> {
    Arc::from(
        format!(
            concat!(
                "--- a/MODULE.bazel\n",
                "+++ b/MODULE.bazel\n",
                "@@ -1,2 +1,2 @@\n",
                " module(name = 'dep', version = '1')\n",
                "-bazel_dep(name = 'base', version = '1')\n",
                "+bazel_dep(name = '{}', version = '1')\n",
            ),
            leaf,
        )
        .into_bytes(),
    )
}

#[tokio::test]
async fn observed_module_source_patch_bytes_restore_a_b_a_with_held_carriers() {
    let module_url = "file:///registry/modules/dep/1/MODULE.bazel";
    let original: Arc<[u8]> = Arc::from(
        b"module(name = 'dep', version = '1')\nbazel_dep(name = 'base', version = '1')\n"
            .as_slice(),
    );
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        module_url,
        ModuleSourceRegistryResponse::Found(original),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name='root')\n\
        bazel_dep(name='dep',version='1')\n\
        single_version_override(module_name='dep',version='1',registry='file:///registry',patches=['//:route.patch'],patch_strip=1)\n";
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let mut values = Vec::new();
    for (generation, leaf, inode) in [
        (11, "leaf_a", 4),
        (12, "leaf_b", 4),
        (13, "leaf_a", 4),
        (14, "leaf_a", 9),
        (15, "leaf_a", 4),
    ] {
        let mut transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root,
            &["file:///registry"],
            generation,
            module_source_patch_epoch(module_source_patch(leaf), inode),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = module_source_complete(&value);
        assert!(matches!(
            observed.result().as_ref(),
            Ok(ModuleSourcePreparation::Registry { bytes, .. })
                if String::from_utf8_lossy(bytes).contains(leaf)
        ));
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        values.push(value);
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &values[0], &values[1]
    ));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &values[2], &values[3]
    ));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &values[2], &values[4]
    ));
    let held = module_source_complete(&values[0]);
    let restored = module_source_complete(&values[2]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());
    assert!(!held.observations().observations().is_empty());

    let mut symlinks = Vec::new();
    for (generation, target) in [
        (16, "/workspace/patch-a.diff"),
        (17, "/workspace/patch-b.diff"),
        (18, "/workspace/patch-a.diff"),
    ] {
        let logical = NormalizedAbsolutePath::new("/workspace/route.patch").unwrap();
        let target = NormalizedAbsolutePath::new(target).unwrap();
        let route = symlink_path_epoch(logical.as_path().to_str().unwrap(), target.as_path().to_str().unwrap());
        let route = PathObservationEpoch::from_shared(
            route.observations().iter()
                .filter(|(demand, _)| demand.path() == &logical || demand.path() == &target)
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap();
        let bytes = PathObservationEpoch::from_shared([(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                target,
                PathObservationOperation::FileBytes,
            ),
            Arc::new(PathObservationResult::FileBytes(PathOperationResult::Present(
                module_source_patch("leaf_a"),
            ))),
        )])
        .unwrap();
        let mut transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root,
            &["file:///registry"],
            generation,
            merge_path_observations(&route, &bytes).unwrap(),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            module_source_complete(&value).result().as_ref(),
            Ok(ModuleSourcePreparation::Registry { bytes, .. })
                if String::from_utf8_lossy(bytes).contains("leaf_a")
        ));
        symlinks.push(value);
    }
    assert!(!ModuleSourcePreparationObservationKey::equality(
        &symlinks[0], &symlinks[1]
    ));
    assert!(ModuleSourcePreparationObservationKey::equality(
        &symlinks[0], &symlinks[2]
    ));
    let held = module_source_complete(&symlinks[0]);
    let restored = module_source_complete(&symlinks[2]);
    assert_eq!(held.result().as_ref(), restored.result().as_ref());
    assert_eq!(held.observations(), restored.observations());
}

fn assert_module_source_need(value: &<ModuleSourcePreparationObservationKey as Key>::Value) {
    assert!(matches!(value, SourcePreparationOutcome::Need(_)));
    assert!(!ModuleSourcePreparationObservationKey::validity(value));
    assert!(!ModuleSourcePreparationObservationKey::equality(value, value));
}

fn epoch_without_path(
    epoch: &PathObservationEpoch,
    path: &str,
) -> PathObservationEpoch {
    let path = NormalizedAbsolutePath::new(path).unwrap();
    PathObservationEpoch::from_shared(
        epoch
            .observations()
            .iter()
            .filter(|(demand, _)| demand.path() != &path)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap()
}

#[tokio::test]
async fn observed_module_source_need_positions_are_carrierless_and_suppress_later_children() {
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let transaction = host_nonregistry_transaction(
        &dice,
        Some(b"module(name='dep',version='1')\n"),
        None,
        &[],
        &[],
        721,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())])
        .unwrap();
    let mut transaction = updater.commit().await;
    let effective_need = transaction.compute(&key).await.unwrap();
    assert_module_source_need(&effective_need);
    let effective_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(effective_row.len(), 1);
    assert!(effective_row[0].starts_with("observed-host-effective-module-override:"));

    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let transaction = host_nonregistry_transaction(
        &dice,
        Some(b"module(name='dep',version='1')\n"),
        None,
        &[],
        &[],
        722,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let transaction = complete_preflight_transaction(transaction, false).await;
    let mut transaction = transaction;
    let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            epoch_without_path(&epoch, "/workspace/dep/MODULE.bazel"),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let source_need = transaction.compute(&key).await.unwrap();
    assert_module_source_need(&source_need);
    let source_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(source_row.len(), 2);
    assert!(source_row[1].starts_with("observed-repository-source-file:"));


    let patch_io = Arc::new(ModuleSourceRegistryIo::new([(
        "file:///registry/modules/dep/1/MODULE.bazel",
        ModuleSourceRegistryResponse::Found(Arc::from(b"dep".as_slice())),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, patch_io);
    let patch_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='root')\n\
        bazel_dep(name='dep',version='1')\n\
        single_version_override(module_name='dep',version='1',registry='file:///registry',patches=['//:route.patch'],patch_strip=1)\n";
    let mut transaction = module_source_registry_transaction(
        &patch_dice,
        tracker.dupe(),
        root,
        &["file:///registry"],
        1,
        PathObservationEpoch::empty(),
    )
    .await;
    let resolution_need = transaction.compute(&key).await.unwrap();
    assert_module_source_need(&resolution_need);
    let row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert!(row.last().unwrap().starts_with("observed-resolved-path:"));
    assert!(row.iter().all(|dep| !dep.starts_with("path-observation:")));
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        owner != &key.to_string() || batch.is_none()
    }));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let patch_path = NormalizedAbsolutePath::new("/workspace/route.patch").unwrap();
    let lstat_only = PathObservationEpoch::from_shared([(
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            patch_path,
            PathObservationOperation::Lstat,
        ),
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
        ))),
    )])
    .unwrap();
    let mut transaction = module_source_registry_transaction(
        &patch_dice,
        tracker.dupe(),
        root,
        &["file:///registry"],
        2,
        lstat_only,
    )
    .await;
    let file_need = transaction.compute(&key).await.unwrap();
    assert_module_source_need(&file_need);
    let row = tracker.rows.lock().unwrap().last().unwrap().1.clone();
    assert!(row.last().unwrap().starts_with("path-observation:"));
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        owner != &key.to_string() || batch.is_none()
    }));
}

struct CancelOnceRegistryIo {
    calls: AtomicUsize,
    bytes: Arc<[u8]>,
}

#[async_trait]
impl crate::RegistryIo for CancelOnceRegistryIo {
    async fn read_exact(
        &self,
        _: &RegistryFileUrl,
    ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            std::future::pending::<()>().await;
        }
        Ok(crate::RegistryIoOutcome::Found(self.bytes.dupe()))
    }
}

#[tokio::test]
async fn observed_module_source_poll_drop_publishes_no_parent_and_recovers_same_dice() {
    let io = Arc::new(CancelOnceRegistryIo {
        calls: AtomicUsize::new(0),
        bytes: Arc::from(b"module(name='dep',version='1')\n".as_slice()),
    });
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut cancelled = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        31,
        PathObservationEpoch::empty(),
    )
    .await;
    let key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while io.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(io.calls.load(Ordering::SeqCst), 1);
    drop(future);
    drop(cancelled);
    assert!(tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _)| owner != &key.to_string()));
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        owner != &key.to_string() || batch.is_none()
    }));

    let mut recovered = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        31,
        PathObservationEpoch::empty(),
    )
    .await;
    let recovered = recovered.compute(&key).await.unwrap();
    assert!(matches!(
        module_source_complete(&recovered).result().as_ref(),
        Ok(ModuleSourcePreparation::Registry { bytes, .. })
            if bytes.as_ref() == b"module(name='dep',version='1')\n"
    ));
    assert_eq!(io.calls.load(Ordering::SeqCst), 2);
    assert!(tracker.batches.lock().unwrap().iter().all(|(owner, _, batch)| {
        owner != &key.to_string() || batch.is_none()
    }));
}

fn observed_discovered_key(name: &str, version: &str) -> HostDiscoveredModuleObservationKey {
    HostDiscoveredModuleObservationKey::try_new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        NonrootModuleKey::new(name, version),
    )
    .unwrap()
}

fn complete_observed_discovered(
    value: &<HostDiscoveredModuleObservationKey as Key>::Value,
) -> &ObservedHostDiscoveredModule {
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("observed discovered module must complete: {value:?}")
    };
    value
}

fn discovered_parent_batch(
    tracker: &NonregistryPreflightTracker,
    key: &HostDiscoveredModuleObservationKey,
) -> Option<EventBatch> {
    tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, kind, _)| owner == &key.to_string() && *kind == ActivationKind::Evaluated)
        .and_then(|(_, _, batch)| batch.dupe())
}

fn discovered_eventful(tracker: &NonregistryPreflightTracker) -> Vec<(String, EventBatch)> {
    tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, kind, batch)| *kind == ActivationKind::Evaluated && batch.is_some())
        .map(|(owner, _, batch)| (owner.clone(), batch.dupe().unwrap()))
        .collect()
}

fn discovered_event_values(found: &[(String, EventBatch)]) -> Vec<EventBatch> { found.iter().map(|(_, batch)| batch.dupe()).collect() }

fn legacy_dependency_order(observed: &[String]) -> Vec<String> {
    observed
        .iter()
        .map(|dependency| {
            let dependency = dependency
                .strip_prefix("observed-")
                .unwrap_or(dependency);
            if let Some(path) = dependency
                .strip_prefix("bzlmod-observed-host-root-module-file:\"")
                .and_then(|path| path.strip_suffix('"'))
            { return format!("root-module-evaluation:{path}"); }
            dependency
                .strip_prefix("root-module-files:\"")
                .and_then(|path| path.strip_suffix('"'))
                .map_or_else(
                    || dependency.to_owned(),
                    |path| format!("root-module-files:{path}"),
                )
        })
        .collect()
}
#[test]
fn observed_discovered_identity_projection_and_terminal_algebra_are_exact() {
    use std::hash::Hash;
    use std::hash::Hasher;

    let key = observed_discovered_key("dep", "1");
    let other = observed_discovered_key("other", "1");
    assert_ne!(key, other);
    assert_eq!(key.to_string(), "observed-host-discovered-module:\"/workspace\":dep@1");
    let hash = |key: &HostDiscoveredModuleObservationKey| {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    };
    assert_ne!(hash(&key), hash(&other));

    let (_, _, epoch) = observed_horizon_epoch("discovered");
    let result = Arc::new(Err(HostDiscoveredModuleError::MissingVersion {
        module_name: "dep".into(),
    }));
    let complete = SourcePreparationOutcome::Complete(Ok(ObservedHostDiscoveredModule {
        result: result.dupe(),
        observations: epoch.dupe(),
    }));
    let observed = complete_observed_discovered(&complete);
    assert!(Arc::ptr_eq(observed.result(), &result));
    assert_exact_epoch(&epoch, observed.observations());
    assert!(HostDiscoveredModuleObservationKey::validity(&complete));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &complete, &complete
    ));
    let SourcePreparationOutcome::Complete(projected) =
        project_legacy_discovered(result.dupe())
    else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&result, &projected));

    let need = SourcePreparationOutcome::Need(preflight_need("discovered-need"));
    assert!(!HostDiscoveredModuleObservationKey::validity(&need));
    assert!(!HostDiscoveredModuleObservationKey::equality(&need, &need));
    let demand = observed_horizon_epoch("discovered-outer").0;
    let outer = SourcePreparationOutcome::Complete(Err(
        HostDiscoveredModuleObservationError::EffectiveFrontier(
            ObservedPathFrontierError::Epoch(
                slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                    demand,
                    result_operation: PathObservationOperation::FileBytes,
                },
            ),
        ),
    ));
    assert!(HostDiscoveredModuleObservationKey::validity(&outer));
    assert!(HostDiscoveredModuleObservationKey::equality(&outer, &outer));

    let (demand, first, prefix) = observed_horizon_epoch("discovered-merge");
    let duplicate = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
    let merged = merge_discovered_prefix(&prefix, &duplicate).unwrap();
    assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
    let conflict = PathObservationEpoch::from_shared([(
        demand,
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644),
        ))),
    )])
    .unwrap();
    assert!(matches!(
        merge_discovered_prefix(&prefix, &conflict),
        Err(SourcePreparationOutcome::Complete(Err(
            HostDiscoveredModuleObservationError::MergeFrontier(
                ObservedPathFrontierError::Epoch(
                    slug_workspace_v2::PathObservationEpochError::ConflictingDemand(_)
                )
            )
        )))
    ));

    let need = preflight_need("discovered-finisher");
    assert!(matches!(
        finish_discovered_observed_child::<(), ObservedPathFrontierError>(
            SourcePreparationOutcome::Need(need.dupe()),
            HostDiscoveredModuleObservationError::EffectiveFrontier,
        ),
        ControlFlow::Break(SourcePreparationOutcome::Need(found)) if found == need
    ));
    let demand = observed_horizon_epoch("discovered-mismatch").0;
    let frontier = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::FileBytes,
        },
    );
    let effective = finish_discovered_observed_child::<(), _>(
        SourcePreparationOutcome::Complete(Err(frontier.dupe())),
        HostDiscoveredModuleObservationError::EffectiveFrontier,
    );
    assert!(matches!(
        effective,
        ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
            HostDiscoveredModuleObservationError::EffectiveFrontier(_)
        )))
    ));
    let closure = finish_discovered_observed_child::<(), _>(
        SourcePreparationOutcome::Complete(Err(
            HostNonregistryModuleClosureObservationError::EffectiveFrontier(frontier.dupe()),
        )),
        |error| {
            HostDiscoveredModuleObservationError::ClosureFrontier(
                HostDiscoveredModuleClosureFrontier(error),
            )
        },
    );
    assert!(matches!(
        closure,
        ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
            HostDiscoveredModuleObservationError::ClosureFrontier(_)
        )))
    ));
    let preparation = finish_discovered_observed_child::<(), _>(
        SourcePreparationOutcome::Complete(Err(frontier)),
        HostDiscoveredModuleObservationError::PreparationFrontier,
    );
    assert!(matches!(
        preparation,
        ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
            HostDiscoveredModuleObservationError::PreparationFrontier(_)
        )))
    ));

    for stage in [
        HostDiscoveredComputeStage::Effective,
        HostDiscoveredComputeStage::Closure,
        HostDiscoveredComputeStage::Preparation,
    ] {
        let observations = (stage != HostDiscoveredComputeStage::Effective)
            .then(|| prefix.dupe())
            .unwrap_or_else(PathObservationEpoch::empty);
        let SourcePreparationOutcome::Complete(Ok((result, found, events))) =
            discovered_compute_error(stage, Arc::from("compute"), observations.dupe())
        else {
            panic!("compute failures remain semantic")
        };
        assert_eq!(found, observations);
        assert!(events.is_none());
        assert!(matches!(
            (stage, result.as_ref()),
            (HostDiscoveredComputeStage::Effective, Err(HostDiscoveredModuleError::RootModuleFiles(_)))
                | (HostDiscoveredComputeStage::Closure, Err(HostDiscoveredModuleError::NonRegistryClosureCompute(_)))
                | (HostDiscoveredComputeStage::Preparation, Err(HostDiscoveredModuleError::SourcePreparationCompute(_)))
        ));
    }

    let SourcePreparationOutcome::Complete(Ok((unsupported, found, events))) = discovered_error(
        HostDiscoveredModuleError::NonRegistryUnsupported {
            module_name: "dep".into(),
        },
        prefix.dupe(),
    ) else { panic!("invariant failure remains semantic") };
    assert!(matches!(unsupported.as_ref(), Err(HostDiscoveredModuleError::NonRegistryUnsupported { .. })));
    assert_eq!(found, prefix);
    assert!(events.is_none());

}
#[tokio::test]
async fn observed_discovered_builtin_matches_legacy_family_and_neutral_events() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        "module(name='root')\n",
        &[],
        800,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let observed_key = observed_discovered_key("bazel_tools", "");
    let observed_value = transaction.compute(&observed_key).await.unwrap();
    let observed = complete_observed_discovered(&observed_value);
    let observed_row = tracker.rows.lock().unwrap().iter()
        .find(|(owner, _)| owner == &observed_key.to_string()).unwrap().1.clone();
    assert_eq!(observed_row.len(), 2);
    assert!(observed_row[0].starts_with("observed-host-effective-module-override:"));
    assert!(observed_row[1].starts_with("builtin-bazel-tools-module:"));
    let observed_events = discovered_eventful(&tracker);
    assert_eq!(observed_events.len(), 1);
    assert!(observed_events[0].0.starts_with("bzlmod-observed-host-root-module-file:"));
    assert!(observed_events[0].1.events().is_empty());

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let legacy_key = HostDiscoveredModuleKey::try_new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        NonrootModuleKey::new("bazel_tools", ""),
    ).unwrap();
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&legacy_key).await.unwrap()
    else { panic!("legacy builtin discovery") };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_row = tracker.rows.lock().unwrap().iter()
        .find(|(owner, _)| owner == &legacy_key.to_string()).unwrap().1.clone();
    assert_eq!(legacy_row, vec![
        HostEffectiveModuleOverrideKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            "bazel_tools".into(),
        ).to_string(),
        observed_row[1].clone(),
    ]);
    let legacy_events = discovered_eventful(&tracker);
    assert!(legacy_events[0].0.starts_with("root-module-evaluation:"));
    assert_eq!(discovered_event_values(&observed_events), discovered_event_values(&legacy_events));
}

#[tokio::test]
async fn observed_discovered_builtin_and_registry_own_exact_prefix_rows_and_events() {
    let url = "https://registry.invalid/modules/dep/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='dep',version='1')\nprint('registry')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        801,
        PathObservationEpoch::empty(),
    )
    .await;


    tracker.rows.lock().unwrap().clear();
    let key = observed_discovered_key("dep", "1");
    let cold = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_discovered(&cold);
    assert!(matches!(
        observed.result().as_ref(),
        Ok(HostDiscoveredModule {
            provenance: HostDiscoveredModuleProvenance::Registry { .. },
            ..
        })
    ));
    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );
    let preparation_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(preparation)) =
        transaction.compute(&preparation_key).await.unwrap()
    else {
        panic!("preparation carrier")
    };
    let expected =
        merge_path_observations(effective.observations(), preparation.observations()).unwrap();
    assert_exact_epoch(&expected, observed.observations());
    let row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        row,
        vec![effective_key.to_string(), preparation_key.to_string()]
    );
    let batch = discovered_parent_batch(&tracker, &key).unwrap();
    assert!(matches!(
        batch.events(),
        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "registry"
    ));
    let eventful = discovered_eventful(&tracker);
    assert_eq!(eventful.len(), 2);
    assert!(eventful[0].0.starts_with("bzlmod-observed-host-root-module-file:"));
    assert!(eventful[0].1.events().is_empty());
    assert_eq!(eventful[1], (key.to_string(), batch.dupe()));
    assert_eq!(eventful.last().unwrap(), &(key.to_string(), batch.dupe()));

    tracker.batches.lock().unwrap().clear();
    let warm = transaction.compute(&key).await.unwrap();
    assert!(HostDiscoveredModuleObservationKey::equality(&cold, &warm));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _, batch)| owner != &key.to_string() || batch.is_none()));

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let mut legacy_tx = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        801,
        PathObservationEpoch::empty(),
    )
    .await;
    let legacy_key = HostDiscoveredModuleKey::try_new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        NonrootModuleKey::new("dep", "1"),
    )
    .unwrap();
    let legacy = legacy_tx.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy discovered module")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_batch = tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, kind, _)| owner == &legacy_key.to_string() && *kind == ActivationKind::Evaluated)
        .and_then(|(_, _, batch)| batch.dupe())
        .unwrap();
    assert_eq!(legacy_batch, batch);
    let legacy_eventful = discovered_eventful(&tracker);
    assert_eq!(legacy_eventful.len(), 2);
    assert!(legacy_eventful[0].0.starts_with("root-module-evaluation:"));
    assert!(legacy_eventful[0].1.events().is_empty());
    assert_eq!(legacy_eventful[1], (legacy_key.to_string(), legacy_batch.dupe()));
    assert_eq!(discovered_event_values(&eventful), discovered_event_values(&legacy_eventful));
    let legacy_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &legacy_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(legacy_row, vec![
        HostEffectiveModuleOverrideKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            "dep".into(),
        ).to_string(),
        ModuleSourcePreparationKey {
            workspace: PathBuf::from("/workspace"),
            module_name: "dep".into(),
            version: "1".into(),
        }.to_string(),
    ]);
}


#[tokio::test]
async fn observed_discovered_validation_semantic_and_evaluation_prefixes_are_exact() {
    let wrong_url = "https://wrong.invalid/modules/dep/1/MODULE.bazel";
    let mut builder = Dice::builder();
    crate::install_registry_io(
        &mut builder,
        Arc::new(ModuleSourceRegistryIo::new([(
            wrong_url,
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='wrong',version='1')\n".as_slice(),
            )),
        )])),
    );
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let effective_key = HostEffectiveModuleOverrideObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        "dep".into(),
    );

    let transaction = host_nonregistry_transaction(
        &dice,
        Some(b"module(name='dep')\n"),
        None,
        &[],
        &[],
        821,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let invalid_key = observed_discovered_key("dep", "1");
    let invalid = transaction.compute(&invalid_key).await.unwrap();
    let invalid = complete_observed_discovered(&invalid);
    assert!(matches!(
        invalid.result().as_ref(),
        Err(HostDiscoveredModuleError::InvalidNonRegistryVersion { .. })
    ));
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    assert_exact_epoch(effective.observations(), invalid.observations());
    assert!(discovered_parent_batch(&tracker, &invalid_key).is_none());
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &invalid_key.to_string())
            .unwrap()
            .1,
        vec![effective_key.to_string()]
    );

    let transaction = host_nonregistry_transaction(
        &dice,
        Some(b"module(name='wrong')\n"),
        None,
        &[],
        &[],
        822,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    tracker.batches.lock().unwrap().clear();
    let key = observed_discovered_key("dep", "");
    let evaluation = transaction.compute(&key).await.unwrap();
    let evaluation = complete_observed_discovered(&evaluation);
    assert!(matches!(
        evaluation.result().as_ref(),
        Err(HostDiscoveredModuleError::Evaluation(_))
    ));
    let closure_key = HostNonregistryModuleClosureObservationKey(
        HostNonregistryModuleClosureKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", ""),
        ),
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(closure)) =
        transaction.compute(&closure_key).await.unwrap()
    else {
        panic!("closure carrier")
    };
    assert_exact_epoch(
        &merge_path_observations(effective.observations(), closure.observations()).unwrap(),
        evaluation.observations(),
    );
    assert!(discovered_parent_batch(&tracker, &key)
        .is_some_and(|batch| batch.events().is_empty()));

    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut missing_tx = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://missing.invalid"],
        823,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.batches.lock().unwrap().clear();
    let registry_key = observed_discovered_key("dep", "1");
    let missing = missing_tx.compute(&registry_key).await.unwrap();
    let missing = complete_observed_discovered(&missing);
    assert!(matches!(
        missing.result().as_ref(),
        Err(HostDiscoveredModuleError::SourcePreparation(_))
    ));
    let preparation_key = ModuleSourcePreparationObservationKey::new(
        PathBuf::from("/workspace"),
        "dep".into(),
        "1".into(),
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        missing_tx.compute(&effective_key).await.unwrap()
    else {
        panic!("effective carrier")
    };
    let SourcePreparationOutcome::Complete(Ok(preparation)) =
        missing_tx.compute(&preparation_key).await.unwrap()
    else {
        panic!("preparation carrier")
    };
    assert_exact_epoch(
        &merge_path_observations(effective.observations(), preparation.observations()).unwrap(),
        missing.observations(),
    );
    assert!(discovered_parent_batch(&tracker, &registry_key).is_none());

    let mut wrong_tx = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://wrong.invalid"],
        824,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.batches.lock().unwrap().clear();
    let wrong = wrong_tx.compute(&registry_key).await.unwrap();
    let wrong = complete_observed_discovered(&wrong);
    assert!(matches!(
        wrong.result().as_ref(),
        Err(HostDiscoveredModuleError::Evaluation(_))
    ));
    assert!(discovered_parent_batch(&tracker, &registry_key)
        .is_some_and(|batch| batch.events().is_empty()));

    tracker.batches.lock().unwrap().clear();
    let builtin_version = observed_discovered_key("bazel_tools", "1");
    let invalid_builtin = wrong_tx.compute(&builtin_version).await.unwrap();
    let invalid_builtin = complete_observed_discovered(&invalid_builtin);
    assert!(matches!(
        invalid_builtin.result().as_ref(),
        Err(HostDiscoveredModuleError::InvalidBuiltinVersion { .. })
    ));
    assert!(discovered_parent_batch(&tracker, &builtin_version).is_none());
}
#[tokio::test]
async fn observed_discovered_error_batches_match_legacy_families() {
    let wrong_url = "https://wrong.invalid/modules/dep/1/MODULE.bazel";
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, Arc::new(ModuleSourceRegistryIo::new([(
        wrong_url,
        ModuleSourceRegistryResponse::Found(Arc::from(b"module(name='wrong',version='1')\n".as_slice())),
    )])));
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let transaction = host_nonregistry_transaction(
        &dice, Some(b"module(name='wrong')\n"), None, &[], &[], 859, None,
        Some(tracker.dupe()), None, true,
    ).await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    tracker.rows.lock().unwrap().clear(); tracker.batches.lock().unwrap().clear();
    let observed_key = observed_discovered_key("dep", "");
    let observed_value = transaction.compute(&observed_key).await.unwrap();
    let observed = complete_observed_discovered(&observed_value);
    let observed_events = discovered_eventful(&tracker);
    assert!(observed_events.last().unwrap().1.events().is_empty());
    tracker.rows.lock().unwrap().clear(); tracker.batches.lock().unwrap().clear();
    let legacy_key = HostDiscoveredModuleKey::try_new(
        NormalizedAbsolutePath::new("/workspace").unwrap(), NonrootModuleKey::new("dep", ""),
    ).unwrap();
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&legacy_key).await.unwrap()
    else { panic!("legacy nonregistry error") };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_events = discovered_eventful(&tracker);
    assert_eq!(discovered_event_values(&observed_events), discovered_event_values(&legacy_events));
    assert_eq!(legacy_events.last().unwrap().0, legacy_key.to_string());

    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut transaction = module_source_registry_transaction(
        &dice, tracker.dupe(), root, &["https://wrong.invalid"], 860,
        PathObservationEpoch::empty(),
    ).await;
    tracker.rows.lock().unwrap().clear(); tracker.batches.lock().unwrap().clear();
    let observed_key = observed_discovered_key("dep", "1");
    let observed_value = transaction.compute(&observed_key).await.unwrap();
    let observed = complete_observed_discovered(&observed_value);
    let observed_events = discovered_eventful(&tracker);
    assert!(observed_events.last().unwrap().1.events().is_empty());
    tracker.rows.lock().unwrap().clear(); tracker.batches.lock().unwrap().clear();
    let legacy_key = HostDiscoveredModuleKey::try_new(
        NormalizedAbsolutePath::new("/workspace").unwrap(), NonrootModuleKey::new("dep", "1"),
    ).unwrap();
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&legacy_key).await.unwrap()
    else { panic!("legacy registry error") };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_events = discovered_eventful(&tracker);
    assert_eq!(discovered_event_values(&observed_events), discovered_event_values(&legacy_events));
    assert_eq!(legacy_events.last().unwrap().0, legacy_key.to_string());
}

#[tokio::test]
async fn observed_discovered_reachable_branch_terminals_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();

    let builtin_effective =
        HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "bazel_tools".into());
    let override_root =
        "module(name='root')\nlocal_path_override(module_name='bazel_tools',path='tools')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        override_root,
        &[],
        861,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let key = observed_discovered_key("bazel_tools", "");
    let value = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_discovered(&value);
    assert!(matches!(
        observed.result().as_ref(),
        Err(HostDiscoveredModuleError::ExplicitBuiltinOverride)
    ));
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&builtin_effective).await.unwrap()
    else {
        panic!("builtin effective carrier")
    };
    assert_exact_epoch(effective.observations(), observed.observations());
    assert_eq!(
        tracker.rows.lock().unwrap().iter()
            .find(|(owner, _)| owner == &key.to_string()).unwrap().1,
        vec![builtin_effective.to_string()]
    );
    assert!(discovered_parent_batch(&tracker, &key).is_none());

    let root = "module(name='root')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://unused.invalid"],
        862,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let key = observed_discovered_key("dep", "");
    let value = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_discovered(&value);
    assert!(matches!(
        observed.result().as_ref(),
        Err(HostDiscoveredModuleError::MissingVersion { module_name })
            if module_name == "dep"
    ));
    let effective_key =
        HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "dep".into());
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        transaction.compute(&effective_key).await.unwrap()
    else {
        panic!("missing-version effective carrier")
    };
    assert_exact_epoch(effective.observations(), observed.observations());
    assert_eq!(
        tracker.rows.lock().unwrap().iter()
            .find(|(owner, _)| owner == &key.to_string()).unwrap().1,
        vec![effective_key.to_string()]
    );
    assert!(discovered_parent_batch(&tracker, &key).is_none());

    for (source, fragments, variant, cycle) in [
        (
            &b"module(name='dep')\ninclude('bad')\n"[..],
            Vec::new(),
            863,
            false,
        ),
        (
            &b"module(name='dep')\ninclude('//pkg:a.MODULE.bazel')\n"[..],
            vec![(
                "pkg/a.MODULE.bazel",
                Some(&b"include('//pkg:a.MODULE.bazel')\n"[..]),
            )],
            864,
            true,
        ),
    ] {
        let transaction = host_nonregistry_transaction(
            &dice,
            Some(source),
            None,
            &fragments,
            &[],
            variant,
            None,
            Some(tracker.dupe()),
            None,
            true,
        )
        .await;
        let mut transaction = complete_preflight_transaction(transaction, false).await;
        tracker.rows.lock().unwrap().clear();
        tracker.batches.lock().unwrap().clear();
        let key = observed_discovered_key("dep", "");
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_discovered(&value);
        assert!(matches!(
            (cycle, observed.result().as_ref()),
            (false, Err(HostDiscoveredModuleError::NonRegistryClosure(_)))
                | (true, Err(HostDiscoveredModuleError::NonRegistryCycle { .. }))
        ));
        let closure_key = HostNonregistryModuleClosureObservationKey(
            HostNonregistryModuleClosureKey::new(
                workspace.dupe(),
                NonrootModuleKey::new("dep", ""),
            ),
        );
        let SourcePreparationOutcome::Complete(Ok(effective)) =
            transaction.compute(&effective_key).await.unwrap()
        else {
            panic!("nonregistry effective carrier")
        };
        let SourcePreparationOutcome::Complete(Ok(closure)) =
            transaction.compute(&closure_key).await.unwrap()
        else {
            panic!("nonregistry closure carrier")
        };
        let expected =
            merge_path_observations(effective.observations(), closure.observations()).unwrap();
        assert_exact_epoch(&expected, observed.observations());
        assert_eq!(
            tracker.rows.lock().unwrap().iter()
                .find(|(owner, _)| owner == &key.to_string()).unwrap().1,
            vec![effective_key.to_string(), closure_key.to_string()]
        );
        assert!(discovered_parent_batch(&tracker, &key).is_none());
    }
}


#[tokio::test]
async fn observed_discovered_nonregistry_prefix_need_and_restoration_are_exact() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let key = observed_discovered_key("dep", "");
    let source_a = b"module(name='dep')\nprint('a')\n";
    let source_b = b"module(name='dep')\nprint('b')\n";
    let mut values = Vec::new();
    for (source, variant) in [
        (source_a.as_slice(), 811),
        (source_b.as_slice(), 812),
        (source_a.as_slice(), 811),
    ] {
        let transaction = host_nonregistry_transaction(
            &dice,
            Some(source),
            None,
            &[],
            &[],
            variant,
            None,
            Some(tracker.dupe()),
            None,
            true,
        )
        .await;
        let mut transaction = complete_preflight_transaction(transaction, false).await;
        tracker.rows.lock().unwrap().clear();
        tracker.batches.lock().unwrap().clear();
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_discovered(&value);
        let effective_key = HostEffectiveModuleOverrideObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            "dep".into(),
        );
        let closure_key = HostNonregistryModuleClosureObservationKey(HostNonregistryModuleClosureKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", ""),
        ));
        let SourcePreparationOutcome::Complete(Ok(effective)) =
            transaction.compute(&effective_key).await.unwrap()
        else {
            panic!("effective carrier")
        };
        let SourcePreparationOutcome::Complete(Ok(closure)) =
            transaction.compute(&closure_key).await.unwrap()
        else {
            panic!("closure carrier")
        };
        assert_exact_epoch(
            &merge_path_observations(effective.observations(), closure.observations()).unwrap(),
            observed.observations(),
        );
        let row = tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &key.to_string())
            .unwrap()
            .1
            .clone();
        assert_eq!(
            row,
            vec![effective_key.to_string(), closure_key.to_string()]
        );
        let expected = if source == source_a { "a" } else { "b" };
        assert!(matches!(
            discovered_parent_batch(&tracker, &key).unwrap().events(),
            [EvaluationEvent::StarlarkPrint { text, .. }] if text == expected
        ));
        if values.is_empty() {
            let observed_eventful = discovered_eventful(&tracker);
            assert_eq!(observed_eventful.last().unwrap().0, key.to_string());
            tracker.rows.lock().unwrap().clear();
            tracker.batches.lock().unwrap().clear();
            let legacy_key = HostDiscoveredModuleKey::try_new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                NonrootModuleKey::new("dep", ""),
            )
            .unwrap();
            let legacy = transaction.compute(&legacy_key).await.unwrap();
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("legacy nonregistry discovery")
            };
            assert_eq!(legacy.as_ref(), observed.result().as_ref());
            let legacy_eventful = discovered_eventful(&tracker);
            assert_eq!(legacy_eventful.last().unwrap().0, legacy_key.to_string());
            assert_eq!(discovered_event_values(&observed_eventful), discovered_event_values(&legacy_eventful));
            assert_eq!(
                tracker
                    .rows
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(owner, _)| owner == &legacy_key.to_string())
                    .unwrap()
                    .1,
                vec![
                    HostEffectiveModuleOverrideKey::new(
                        NormalizedAbsolutePath::new("/workspace").unwrap(),
                        "dep".into(),
                    )
                    .to_string(),
                    HostNonregistryModuleClosureKey::new(
                        NormalizedAbsolutePath::new("/workspace").unwrap(),
                        NonrootModuleKey::new("dep", ""),
                    )
                    .to_string(),
                ]
            );
        }
        values.push(value);
    }
    assert!(!HostDiscoveredModuleObservationKey::equality(
        &values[0], &values[1]
    ));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &values[0], &values[2]
    ));
    let held = complete_observed_discovered(&values[0]);
    let restored = complete_observed_discovered(&values[2]);
    assert_eq!(held.observations(), restored.observations());
    assert!(matches!(
        held.result().as_ref(),
        Ok(HostDiscoveredModule { .. })
    ));

    let need_root = b"module(name='dep')\ninclude('//pkg:a.MODULE.bazel')\n";
    let transaction = host_nonregistry_transaction(
        &dice,
        Some(need_root),
        None,
        &[],
        &["pkg/a.MODULE.bazel"],
        813,
        None,
        Some(tracker.dupe()),
        None,
        true,
    )
    .await;
    let mut transaction = complete_preflight_transaction(transaction, false).await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let need = transaction.compute(&key).await.unwrap();
    assert!(!HostDiscoveredModuleObservationKey::validity(&need));
    assert!(!HostDiscoveredModuleObservationKey::equality(&need, &need));
    assert!(discovered_parent_batch(&tracker, &key).is_none());
    for forbidden in [
        "host-selected-module-graph:",
        "host-selected-repo",
        "host-selected-extension",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
            !owner.starts_with(forbidden)
                && dependencies
                    .iter()
                    .all(|dependency| !dependency.starts_with(forbidden))
        }));
    }
}

#[tokio::test]
async fn observed_discovered_effective_and_preparation_restore_independently() {
    let url = "https://lifecycle.invalid/modules/dep/1/MODULE.bazel";
    let source_a: Arc<[u8]> = Arc::from(
        b"module(name='dep',version='1')\nbazel_dep(name='a',version='1')\nprint('a')\n"
            .as_slice(),
    );
    let source_b: Arc<[u8]> = Arc::from(
        b"module(name='dep',version='1')\nbazel_dep(name='b',version='1')\nprint('b')\n"
            .as_slice(),
    );
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        ModuleSourceRegistryResponse::Found(source_a.dupe()),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let key = observed_discovered_key("dep", "1");
    let roots = [
        "module(name='root')\nbazel_dep(name='dep',version='1')\n",
        "module(name='root')\nbazel_dep(name='dep',version='1+changed')\n",
        "module(name='root')\nbazel_dep(name='dep',version='1')\n",
    ];
    let mut effective_values = Vec::new();
    for root in roots {
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["https://lifecycle.invalid"],
            831,
            PathObservationEpoch::empty(),
        )
        .await;
        effective_values.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!HostDiscoveredModuleObservationKey::equality(
        &effective_values[0],
        &effective_values[1],
    ));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &effective_values[0],
        &effective_values[2],
    ));
    let held_effective = complete_observed_discovered(&effective_values[0]);
    let restored_effective = complete_observed_discovered(&effective_values[2]);
    assert_eq!(held_effective, restored_effective);
    assert_eq!(held_effective.observations(), restored_effective.observations());
    assert!(matches!(
        held_effective.result().as_ref(),
        Ok(HostDiscoveredModule { .. })
    ));
    assert!(!held_effective.observations().observations().is_empty());

    let root = roots[0];
    let mut preparation_values = Vec::new();
    for (slot, source) in [source_a.dupe(), source_b, source_a].into_iter().enumerate() {
        io.responses.lock().unwrap().insert(
            url.to_owned(),
            ModuleSourceRegistryResponse::Found(source),
        );
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["https://lifecycle.invalid"],
            840 + u64::try_from(slot).unwrap(),
            PathObservationEpoch::empty(),
        )
        .await;
        preparation_values.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!HostDiscoveredModuleObservationKey::equality(
        &preparation_values[0],
        &preparation_values[1],
    ));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &preparation_values[0],
        &preparation_values[2],
    ));
    let held_preparation = complete_observed_discovered(&preparation_values[0]);
    let changed_preparation = complete_observed_discovered(&preparation_values[1]);
    assert_ne!(held_preparation.result(), changed_preparation.result());
    assert_eq!(
        held_preparation.observations(), changed_preparation.observations()
    );
    let restored = complete_observed_discovered(&preparation_values[2]);
    assert_eq!(held_preparation.result(), restored.result());
    assert_eq!(held_preparation.observations(), restored.observations());
}

#[tokio::test]
async fn observed_discovered_lockfile_preparation_epoch_restores_independently() {
    let url = "https://lock.invalid/modules/dep/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='dep',version='1')\nprint('same')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let key = observed_discovered_key("dep", "1");
    let lockfile = br#"{"lockFileVersion":28}"#;
    let mut values = Vec::new();
    for (generation, mode, bytes) in [
        (871, crate::LockfileMode::Update, None),
        (872, crate::LockfileMode::Error, None),
        (873, crate::LockfileMode::Update, None),
        (874, crate::LockfileMode::Update, Some(lockfile.as_slice())),
        (875, crate::LockfileMode::Update, None),
    ] {
        let transaction = module_source_registry_transaction(
            &dice,
            Arc::new(NonregistryPreflightTracker::default()),
            root,
            &["https://lock.invalid"],
            generation,
            PathObservationEpoch::empty(),
        )
        .await;
        let mut transaction = module_source_lockfile_transaction(transaction, mode, bytes).await;
        values.push(transaction.compute(&key).await.unwrap());
    }
    assert!(!HostDiscoveredModuleObservationKey::equality(
        &values[0], &values[1]
    ));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!HostDiscoveredModuleObservationKey::equality(
        &values[2], &values[3]
    ));
    assert!(HostDiscoveredModuleObservationKey::equality(
        &values[2], &values[4]
    ));
    let held = complete_observed_discovered(&values[0]);
    let restored = complete_observed_discovered(&values[4]);
    assert_eq!(held.result(), restored.result());
    assert_eq!(held.observations(), restored.observations());
}


#[tokio::test]
async fn observed_discovered_poll_drop_publishes_nothing_and_recovers_same_dice() {
    let io = Arc::new(CancelOnceRegistryIo {
        calls: AtomicUsize::new(0),
        bytes: Arc::from(
            b"module(name='dep',version='1')\nprint('recovered')\n".as_slice(),
        ),
    });
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='root')\nbazel_dep(name='dep',version='1')\n";
    let mut cancelled = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://cancel.invalid"],
        851,
        PathObservationEpoch::empty(),
    )
    .await;
    let key = observed_discovered_key("dep", "1");
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while io.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(future);
    drop(cancelled);
    assert!(tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _)| owner != &key.to_string()));
    assert!(discovered_parent_batch(&tracker, &key).is_none());

    let mut recovered = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://cancel.invalid"],
        851,
        PathObservationEpoch::empty(),
    )
    .await;
    let recovered = recovered.compute(&key).await.unwrap();
    assert!(matches!(
        complete_observed_discovered(&recovered).result().as_ref(),
        Ok(HostDiscoveredModule { .. })
    ));
    assert_eq!(io.calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        discovered_parent_batch(&tracker, &key).unwrap().events(),
        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "recovered"
    ));
    for forbidden in [
        "host-selected-module-graph:",
        "host-selected-repo",
        "host-selected-extension",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
            !owner.starts_with(forbidden)
                && dependencies
                    .iter()
                    .all(|dependency| !dependency.starts_with(forbidden))
        }));
    }
}
use crate::module_eval::RootModuleCommandPolicyKey;
use crate::module_eval::RootModuleFilesObservationKey;
use crate::selected_graph::HostGraphModuleKey;
use crate::module_eval::RootModuleFilesKey;
use crate::selected_graph::HostSelectedModuleGraph;
use crate::selected_graph::HostSelectedModuleGraphKey;
use crate::selected_graph::HostSelectedModuleGraphObservationKey;
use crate::selected_graph::ObservedHostSelectedModuleGraph;

fn complete_observed_selected_graph(
    value: &<HostSelectedModuleGraphObservationKey as Key>::Value,
) -> &ObservedHostSelectedModuleGraph {
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("observed selected graph must complete: {value:?}")
    };
    value
}

fn selected_graph_event_values(tracker: &NonregistryPreflightTracker) -> Vec<EventBatch> {
    tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, kind, batch)| *kind == ActivationKind::Evaluated && batch.is_some())
        .map(|(_, _, batch)| batch.dupe().unwrap())
        .collect()
}

#[tokio::test]
async fn observed_selected_graph_matches_legacy_families_events_and_warm_reuse() {
    let url = "https://registry.invalid/modules/dep/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='dep',version='1')\nprint('selected')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        901,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();

    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());
    let cold = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_selected_graph(&cold);
    let Ok(HostSelectedModuleGraph { resolved, unpruned }) = observed.result().as_ref() else {
        panic!("selected graph must succeed: {:?}", observed.result())
    };
    assert_eq!(resolved.len(), 2);
    assert_eq!(unpruned.len(), 2);
    assert!(!observed.observations().observations().is_empty());

    let observed_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        observed_row,
        vec![
            RootModuleFilesObservationKey::new(workspace.dupe()).to_string(),
            RootModuleCommandPolicyKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            HostEffectiveModuleOverrideObservationKey::new(
                workspace.dupe(),
                "dep".into(),
            )
            .to_string(),
            observed_discovered_key("dep", "1").to_string(),
            observed_discovered_key("dep", "1").to_string(),
        ]
    );
    assert!(tracker.batches.lock().unwrap().iter().all(
        |(owner, _, batch)| owner != &key.to_string() || batch.is_none()
    ));
    let observed_eventful = discovered_eventful(&tracker);
    assert_eq!(observed_eventful.len(), 2);
    assert!(observed_eventful[0]
        .0
        .starts_with("bzlmod-observed-host-root-module-file:"));
    assert_eq!(observed_eventful[1].0, observed_discovered_key("dep", "1").to_string());
    assert!(observed_eventful[0].1.events().is_empty());
    assert!(matches!(observed_eventful[1].1.events(), [EvaluationEvent::StarlarkPrint { text, .. }] if text == "selected"));
    let observed_events = selected_graph_event_values(&tracker);

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let warm = transaction.compute(&key).await.unwrap();
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &cold, &warm
    ));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(_, _, batch)| batch.is_none()));

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let mut legacy_transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        901,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let legacy_key = HostSelectedModuleGraphKey::new(workspace.dupe());
    let legacy = legacy_transaction.compute(&legacy_key).await.unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy selected graph must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
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
            RootModuleFilesKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            RootModuleCommandPolicyKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            HostEffectiveModuleOverrideKey::new(workspace.dupe(), "dep".into()).to_string(),
            HostDiscoveredModuleKey::try_new(
                workspace.dupe(),
                NonrootModuleKey::new("dep", "1"),
            )
            .unwrap()
            .to_string(),
            HostDiscoveredModuleKey::try_new(workspace, NonrootModuleKey::new("dep", "1"))
                .unwrap()
                .to_string(),
        ]
    );
    let legacy_eventful = discovered_eventful(&tracker);
    assert_eq!(legacy_eventful.len(), 2);
    assert!(legacy_eventful[0].0.starts_with("root-module-evaluation:"));
    assert!(legacy_eventful[1].0.starts_with("host-discovered-module:"));
    assert_eq!(discovered_event_values(&legacy_eventful), observed_events);
    assert_eq!(selected_graph_event_values(&tracker), observed_events);
    for forbidden in [
        "host-selected-registry-repo",
        "host-selected-module-routes",
        "host-selected-extension",
        "public-",
        "accepted-",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
            !owner.starts_with(forbidden)
                && dependencies
                    .iter()
                    .all(|dependency| !dependency.starts_with(forbidden))
        }));
    }
}

async fn selected_graph_nonregistry_transaction(
    dice: &Arc<Dice>,
    tracker: Arc<NonregistryPreflightTracker>,
    root: &str,
    source: &[u8],
    variant: i64,
    registries: &[&str],
) -> dice::DiceTransaction {
    let transaction = host_nonregistry_transaction(
        dice,
        Some(source),
        None,
        &[],
        &[],
        variant,
        None,
        Some(tracker),
        None,
        true,
    )
    .await;
    let mut updater = transaction.into_updater();
    updater.changed_to(vec![(
        slug_workspace_v2::WorkspaceSnapshotKey { workspace: PathBuf::from("/workspace") },
        Arc::new(slug_workspace_v2::WorkspaceSnapshot {
            files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                PathBuf::from("/workspace/MODULE.bazel"),
                slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(root.to_owned())),
            )])),
        }),
    )]).unwrap();
    updater.changed_to(vec![(
        PathObservationEpochKey,
        horizon_epoch(
            root,
            PathObservationNamespace::Host,
            "/workspace/dep",
            Some(source),
            None, None, None,
            &[("pkg", true), ("other", true)],
            &[], &[],
            &[("pkg/BUILD.bazel", Some(&b""[..])), ("other/BUILD.bazel", Some(&b""[..]))],
            &[], &[],
            variant,
        ),
    )]).unwrap();
    if !registries.is_empty() {
        crate::inject_registry_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::RegistryUrls::new(registries.iter().copied()),
            crate::RegistryRequestGeneration(variant as u64),
        )
        .unwrap();
    }
    complete_preflight_transaction(updater.commit().await, false).await
}

#[tokio::test]
async fn observed_selected_graph_mixed_horizon_fixed_point_matches_legacy() {
    let url = "https://registry.invalid/modules/reg/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        ModuleSourceRegistryResponse::Found(Arc::from(
            b"module(name='reg',version='1')\nprint('registry')\n".as_slice(),
        )),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let source = b"module(name='dep')\nprint('local')\n";
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep')\nlocal_path_override(module_name='dep',path='dep')\nbazel_dep(name='reg',version='1')\n";
    let mut transaction = selected_graph_nonregistry_transaction(
        &dice,
        tracker.dupe(),
        root,
        source,
        960,
        &["https://registry.invalid"],
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());
    let value = transaction.compute(&key).await.unwrap();
    let observed = complete_observed_selected_graph(&value);
    let graph = observed.result().as_ref().as_ref().unwrap();
    assert_eq!(graph.resolved.len(), 3);
    let dep = observed_discovered_key("dep", "").to_string();
    let reg = observed_discovered_key("reg", "1").to_string();
    let observed_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        observed_row,
        vec![
            RootModuleFilesObservationKey::new(workspace.dupe()).to_string(),
            RootModuleCommandPolicyKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            HostEffectiveModuleOverrideObservationKey::new(
                workspace.dupe(),
                "dep".into(),
            )
            .to_string(),
            HostEffectiveModuleOverrideObservationKey::new(
                workspace.dupe(),
                "reg".into(),
            )
            .to_string(),
            dep.clone(),
            reg.clone(),
            dep,
            reg,
        ]
    );
    let observed_events = discovered_eventful(&tracker);
    assert_eq!(observed_events.len(), 3);
    assert!(observed_events[0]
        .0
        .starts_with("bzlmod-observed-host-root-module-file:"));
    assert_eq!(
        observed_events[1].0,
        observed_discovered_key("reg", "1").to_string()
    );
    assert_eq!(
        observed_events[2].0,
        observed_discovered_key("dep", "").to_string()
    );
    assert!(observed_events[0].1.events().is_empty());
    assert!(matches!(
        observed_events[1].1.events(),
        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "registry"
    ));
    assert!(matches!(
        observed_events[2].1.events(),
        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "local"
    ));
    assert!(tracker.batches.lock().unwrap().iter().all(
        |(owner, _, batch)| owner != &key.to_string() || batch.is_none()
    ));

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let legacy_key = HostSelectedModuleGraphKey::new(workspace.dupe());
    let SourcePreparationOutcome::Complete(legacy) = transaction.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy graph must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
    let legacy_dep = HostDiscoveredModuleKey::try_new(
        workspace.dupe(),
        NonrootModuleKey::new("dep", ""),
    )
    .unwrap()
    .to_string();
    let legacy_reg = HostDiscoveredModuleKey::try_new(
        workspace.dupe(),
        NonrootModuleKey::new("reg", "1"),
    )
    .unwrap()
    .to_string();
    assert_eq!(
        tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &legacy_key.to_string())
            .unwrap()
            .1,
        vec![
            RootModuleFilesKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            RootModuleCommandPolicyKey {
                workspace: PathBuf::from("/workspace"),
            }
            .to_string(),
            HostEffectiveModuleOverrideKey::new(workspace.dupe(), "dep".into()).to_string(),
            HostEffectiveModuleOverrideKey::new(workspace.dupe(), "reg".into()).to_string(),
            legacy_dep.clone(),
            legacy_reg.clone(),
            legacy_dep.clone(),
            legacy_reg.clone(),
        ]
    );
    let legacy_events = discovered_eventful(&tracker);
    assert_eq!(legacy_events.len(), 3);
    assert!(legacy_events[0].0.starts_with("root-module-evaluation:"));
    assert_eq!(legacy_events[1].0, legacy_reg);
    assert_eq!(legacy_events[2].0, legacy_dep);
    assert_eq!(
        discovered_event_values(&legacy_events),
        discovered_event_values(&observed_events)
    );
}

#[tokio::test]
async fn observed_selected_graph_registry_and_root_epochs_restore_independently() {
    let url = "https://registry.invalid/modules/dep/1/MODULE.bazel";
    let bytes = |text: &'static [u8]| {
        ModuleSourceRegistryResponse::Found(Arc::from(text))
    };
    let io = Arc::new(ModuleSourceRegistryIo::new([(
        url,
        bytes(b"module(name='dep',version='1')\nprint('a')\n"),
    )]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n";
    let variants = [
        b"module(name='dep',version='1')\nprint('a')\n".as_slice(),
        b"module(name='dep',version='1')\nprint('b')\n".as_slice(),
        b"module(name='dep',version='1')\nprint('a')\n".as_slice(),
    ];
    let mut registry_values = Vec::new();
    for (index, variant) in variants.iter().enumerate() {
        io.responses.lock().unwrap().insert(
            url.to_owned(),
            ModuleSourceRegistryResponse::Found(Arc::from(*variant)),
        );
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["https://registry.invalid"],
            920 + index as u64,
            PathObservationEpoch::empty(),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        assert!(observed.result().is_ok());
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        registry_values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &registry_values[0],
        &registry_values[2],
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &registry_values[0],
        &registry_values[1],
    ));
    let held_result = complete_observed_selected_graph(&registry_values[0])
        .result()
        .dupe();
    let held_epoch = complete_observed_selected_graph(&registry_values[0])
        .observations()
        .dupe();
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());

    io.responses.lock().unwrap().insert(
        url.to_owned(),
        bytes(b"module(name='dep',version='1')\nprint('a')\n"),
    );
    let roots = [
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n# a\n",
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n# b\n",
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n# a\n",
    ];
    let mut root_values = Vec::new();
    for root in roots {
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["https://registry.invalid"],
            930,
            PathObservationEpoch::empty(),
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        assert!(observed.result().is_ok());
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        root_values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &root_values[0],
        &root_values[2],
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &root_values[0],
        &root_values[1],
    ));
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());
}


#[tokio::test]
async fn observed_selected_graph_poll_drop_publishes_nothing_and_recovers_same_dice() {
    let io = Arc::new(CancelOnceRegistryIo {
        calls: AtomicUsize::new(0),
        bytes: Arc::from(
            b"module(name='dep',version='1')\nprint('graph-recovered')\n".as_slice(),
        ),
    });
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n";
    let mut cancelled = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://cancel.invalid"],
        940,
        PathObservationEpoch::empty(),
    )
    .await;
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while io.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(future);
    drop(cancelled);
    assert!(tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _)| owner != &key.to_string()));
    assert!(tracker.batches.lock().unwrap().iter().all(
        |(owner, _, batch)| owner != &key.to_string() || batch.is_none()
    ));

    let mut recovered = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://cancel.invalid"],
        940,
        PathObservationEpoch::empty(),
    )
    .await;
    let recovered = recovered.compute(&key).await.unwrap();
    let observed = complete_observed_selected_graph(&recovered);
    assert!(observed.result().is_ok());
    assert_eq!(io.calls.load(Ordering::SeqCst), 2);
    assert!(tracker.batches.lock().unwrap().iter().all(
        |(owner, _, batch)| owner != &key.to_string() || batch.is_none()
    ));
    for forbidden in [
        "host-selected-registry-repo",
        "host-selected-module-routes",
        "host-selected-extension",
        "public-",
        "accepted-",
    ] {
        assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
            !owner.starts_with(forbidden)
                && dependencies
                    .iter()
                    .all(|dependency| !dependency.starts_with(forbidden))
        }));
    }
}

#[tokio::test]
async fn observed_selected_graph_root_need_and_semantic_error_suppress_later_work() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\n";
    let transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        950,
        PathObservationEpoch::empty(),
    )
    .await;
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())])
        .unwrap();
    let mut transaction = updater.commit().await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let need = transaction.compute(&key).await.unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    assert!(!HostSelectedModuleGraphObservationKey::validity(&need));
    assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
        let keys = std::iter::once(owner).chain(dependencies);
        keys.into_iter().all(|key| {
            !key.starts_with("observed-host-effective-module-override:")
                && !key.starts_with("observed-host-discovered-module:")
                && !key.starts_with("root-module-command-policy:")
        })
    }));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _, batch)| owner != &key.to_string() || batch.is_none()));

    let mut error_transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        "module(name=\n",
        &["https://registry.invalid"],
        951,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let error = error_transaction.compute(&key).await.unwrap();
    let observed = complete_observed_selected_graph(&error);
    assert!(matches!(
        observed.result().as_ref(),
        Err(crate::selected_graph::HostSelectedModuleGraphError::Input { owner, .. })
            if owner == "root MODULE files"
    ));
    assert!(!observed.observations().observations().is_empty());
    let row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        row,
        vec![
            RootModuleFilesObservationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .to_string(),
        ]
    );
    assert!(tracker.rows.lock().unwrap().iter().all(|(owner, dependencies)| {
        let keys = std::iter::once(owner).chain(dependencies);
        keys.into_iter().all(|key| {
            !key.starts_with("observed-host-effective-module-override:")
                && !key.starts_with("observed-host-discovered-module:")
                && !key.starts_with("root-module-command-policy:")
        })
    }));
    assert!(tracker
        .batches
        .lock()
        .unwrap()
        .iter()
        .all(|(owner, _, batch)| owner != &key.to_string() || batch.is_none()));
}

#[tokio::test]
async fn observed_selected_graph_command_policy_effective_restores_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\n";
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());
    let variants: [&[&str]; 3] = [&[], &["dep=/workspace/dep"], &[]];
    let mut values = Vec::new();
    for overrides in variants {
        let transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &[],
            970,
            PathObservationEpoch::empty(),
        )
        .await;
        let mut updater = transaction.into_updater();
        crate::inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                Path::new("/workspace"),
                overrides.iter().copied(),
            )
            .unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        tracker.rows.lock().unwrap().clear();
        tracker.batches.lock().unwrap().clear();
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        assert_selected_epoch(&mut transaction, observed.observations(), observed.observations())
            .await;
        let row = tracker.rows.lock().unwrap().iter()
            .find(|(owner, _)| owner == &key.to_string()).unwrap().1.clone();
        assert_eq!(row[..2], [
            RootModuleFilesObservationKey::new(workspace.dupe()).to_string(),
            RootModuleCommandPolicyKey { workspace: PathBuf::from("/workspace") }.to_string(),
        ]);
        assert_eq!(row.len(), if overrides.is_empty() { 2 } else { 3 });
        if !overrides.is_empty() {
            assert_eq!(
                row[2],
                HostEffectiveModuleOverrideObservationKey::new(
                    workspace.dupe(),
                    "dep".into(),
                )
                .to_string()
            );
        }
        assert!(tracker.batches.lock().unwrap().iter().all(
            |(owner, _, batch)| owner != &key.to_string() || batch.is_none()
        ));
        values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[1]
    ));
    let held_result = complete_observed_selected_graph(&values[0]).result().dupe();
    let held_epoch = complete_observed_selected_graph(&values[0]).observations().dupe();
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());
}

#[tokio::test]
async fn observed_selected_graph_nonregistry_restores_with_unaffected_root_arc() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep')\nlocal_path_override(module_name='dep',path='dep')\n";
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let variants = [
        (b"module(name='dep')\nprint('a')\n".as_slice(), 981),
        (b"module(name='dep')\nprint('b')\n".as_slice(), 982),
        (b"module(name='dep')\nprint('a')\n".as_slice(), 981),
    ];
    let mut values = Vec::new();
    for (source, variant) in variants {
        let mut transaction = selected_graph_nonregistry_transaction(
            &dice,
            tracker.dupe(),
            root,
            source,
            variant,
            &[],
        )
        .await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        assert!(observed.result().is_ok());
        assert_selected_epoch(&mut transaction, observed.observations(), observed.observations())
            .await;
        values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[1]
    ));
    let first = complete_observed_selected_graph(&values[0]);
    let changed = complete_observed_selected_graph(&values[1]);
    let restored = complete_observed_selected_graph(&values[2]);
    let root_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
        PathObservationOperation::FileBytes,
    );
    assert!(Arc::ptr_eq(
        first.observations().get(&root_demand).unwrap(),
        changed.observations().get(&root_demand).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        first.observations().get(&root_demand).unwrap(),
        restored.observations().get(&root_demand).unwrap(),
    ));
    let held_result = first.result().dupe();
    let held_epoch = first.observations().dupe();
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());
}

#[tokio::test]
async fn observed_selected_graph_diamond_cycle_nodep_rounds_are_exact() {
    let module = |name: &str, body: &str| {
        Arc::from(format!("module(name='{name}',version='1')\nprint('{name}')\n{body}").into_bytes())
    };
    let io = Arc::new(ModuleSourceRegistryIo::new([
        (
            "https://registry.invalid/modules/a/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(module(
                "a",
                "bazel_dep(name='b',version='1')\nbazel_dep(name='b',version='2',repo_name=None)\n",
            )),
        ),
        (
            "https://registry.invalid/modules/b/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(module(
                "b",
                "bazel_dep(name='c',version='1')\n",
            )),
        ),
        (
            "https://registry.invalid/modules/b/2/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='b',version='2')\nprint('b2')\nbazel_dep(name='c',version='1')\n"
                    .as_slice(),
            )),
        ),
        (
            "https://registry.invalid/modules/c/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(module(
                "c",
                "bazel_dep(name='a',version='1')\n",
            )),
        ),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='a',version='1')\n";
    let mut transaction = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        root,
        &["https://registry.invalid"],
        990,
        PathObservationEpoch::empty(),
    )
    .await;
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let observed_key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let observed_value = transaction.compute(&observed_key).await.unwrap();
    let observed = complete_observed_selected_graph(&observed_value);
    let graph = observed.result().as_ref().as_ref().unwrap();
    assert_eq!(
        graph
            .unpruned
            .iter()
            .map(|entry| match &entry.key {
                HostGraphModuleKey::Root => None,
                HostGraphModuleKey::Module { name, .. } => {
                    Some(name.to_string())
                }
            })
            .collect::<Vec<_>>(),
        vec![
            None,
            Some("a".to_owned()),
            Some("b".to_owned()),
            Some("b".to_owned()),
            Some("c".to_owned()),
        ]
    );
    assert_eq!(
        graph
            .resolved
            .iter()
            .map(|entry| match &entry.key {
                HostGraphModuleKey::Root => None,
                HostGraphModuleKey::Module { name, .. } => {
                    Some(name.to_string())
                }
            })
            .collect::<Vec<_>>(),
        vec![
            None,
            Some("a".to_owned()),
            Some("b".to_owned()),
            Some("c".to_owned()),
        ]
    );
    assert_selected_epoch(
        &mut transaction,
        observed.observations(),
        observed.observations(),
    )
    .await;
    let observed_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_key.to_string())
        .unwrap()
        .1
        .clone();
    let observed_events = discovered_eventful(&tracker);
    assert_eq!(
        discovered_event_values(&observed_events)
            .iter()
            .map(|batch| {
                batch
                    .events()
                    .iter()
                    .map(|event| match event {
                        EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                        EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![
            Vec::<&str>::new(),
            vec!["a"],
            vec!["b"],
            vec!["c"],
            vec!["b2"],
        ]
    );

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let legacy_key = HostSelectedModuleGraphKey::new(workspace);
    let SourcePreparationOutcome::Complete(legacy) =
        transaction.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy topology must complete")
    };
    assert_eq!(legacy.as_ref(), observed.result().as_ref());
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
        discovered_eventful(&tracker)
            .iter()
            .map(|(owner, _)| owner.clone())
            .collect::<Vec<_>>(),
        legacy_dependency_order(&observed_events.iter().map(|(owner, _)| owner.clone()).collect::<Vec<_>>()),
    );
    assert_eq!(legacy_row, legacy_dependency_order(&observed_row));
    assert_eq!(
        discovered_event_values(&discovered_eventful(&tracker)),
        discovered_event_values(&observed_events)
    );
}

#[tokio::test]
async fn observed_selected_graph_recursive_mixed_horizon_restores_independently() {
    let reg_url = "https://registry.invalid/modules/reg/1/MODULE.bazel";
    let io = Arc::new(ModuleSourceRegistryIo::new([
        (
            "https://registry.invalid/modules/side/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='side',version='1')\nprint('side')\n".as_slice(),
            )),
        ),
        (
            reg_url,
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='reg',version='1')\nprint('a')\n".as_slice(),
            )),
        ),
        (
            "https://registry.invalid/modules/leaf/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='leaf',version='1')\n".as_slice(),
            )),
        ),
        (
            "https://registry.invalid/modules/leaf/2/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='leaf',version='2')\n".as_slice(),
            )),
        ),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io.dupe());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let root = "module(name='bazel_tools')\nbazel_dep(name='dep')\nlocal_path_override(module_name='dep',path='dep')\nbazel_dep(name='side',version='1')\n";
    let source =
        b"module(name='dep')\nbazel_dep(name='reg',version='1')\nprint('local')\n";
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let variants = [
        b"module(name='reg',version='1')\nbazel_dep(name='leaf',version='1')\n".as_slice(),
        b"module(name='reg',version='1')\nbazel_dep(name='leaf',version='2')\n".as_slice(),
        b"module(name='reg',version='1')\nbazel_dep(name='leaf',version='1')\n".as_slice(),
    ];
    let mut values = Vec::new();
    for (index, variant) in variants.into_iter().enumerate() {
        io.responses.lock().unwrap().insert(
            reg_url.to_owned(),
            ModuleSourceRegistryResponse::Found(Arc::from(variant)),
        );
        let transaction = selected_graph_nonregistry_transaction(
            &dice,
            tracker.dupe(),
            root,
            source,
            1000,
            &["https://registry.invalid"],
        )
        .await;
        let mut updater = transaction.into_updater();
        crate::inject_registry_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::RegistryUrls::new(["https://registry.invalid"]),
            crate::RegistryRequestGeneration(1000 + index as u64),
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        let graph = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(graph.resolved.len(), 5);
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[1]
    ));
    let first = complete_observed_selected_graph(&values[0]);
    let changed = complete_observed_selected_graph(&values[1]);
    let restored = complete_observed_selected_graph(&values[2]);
    for demand in [
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
            PathObservationOperation::FileBytes,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap(),
            PathObservationOperation::FileBytes,
        ),
    ] {
        assert!(Arc::ptr_eq(
            first.observations().get(&demand).unwrap(),
            changed.observations().get(&demand).unwrap(),
        ));
        assert!(Arc::ptr_eq(
            first.observations().get(&demand).unwrap(),
            restored.observations().get(&demand).unwrap(),
        ));
    }
    let held_result = first.result().dupe();
    let held_epoch = first.observations().dupe();
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());
}

#[tokio::test]
async fn observed_selected_graph_implicit_builtin_and_duplicate_candidate_are_exact() {
    let mut builder = Dice::builder();
    crate::install_registry_io(
        &mut builder,
        Arc::new(ModuleSourceRegistryIo::new(std::iter::empty::<(
            &'static str,
            ModuleSourceRegistryResponse,
        )>())),
    );
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let observed_key = HostSelectedModuleGraphObservationKey::new(workspace.dupe());

    let root = "module(name='bazel_tools')\nbazel_dep(name='dep')\nlocal_path_override(module_name='dep',path='dep')\n";
    let transaction = selected_graph_nonregistry_transaction(
        &dice,
        tracker.dupe(),
        root,
        b"module(name='dep')\n",
        1010,
        &[],
    )
    .await;
    let mut updater = transaction.into_updater();
    crate::inject_root_module_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            Path::new("/workspace"),
            ["dep=/workspace/dep"],
        )
        .unwrap(),
        crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        crate::LockfileMode::Update,
    )
    .unwrap();
    let mut duplicate =
        complete_preflight_transaction(updater.commit().await, false).await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let value = duplicate.compute(&observed_key).await.unwrap();
    assert!(matches!(value, SourcePreparationOutcome::Need(_)), "{value:?}");
    assert!(!HostSelectedModuleGraphObservationKey::validity(&value));
    let duplicate_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_key.to_string())
        .unwrap()
        .1
        .clone();
    let effective_dep =
        HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "dep".into())
            .to_string();
    assert_eq!(
        duplicate_row
            .iter()
            .filter(|dependency| dependency == &&effective_dep)
            .count(),
        1
    );
    let SourcePreparationOutcome::Need(needs) = &value else {
        unreachable!()
    };
    let request = needs
        .repository_materializations()
        .iter()
        .next()
        .unwrap()
        .1
        .dupe();
    let mut updater = duplicate.into_updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.dupe(),
            },
            RepositoryMaterializationResultEpoch::new(
                workspace.dupe(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                }],
            )
            .unwrap(),
        )])
        .unwrap();
    duplicate = updater.commit().await;
    tracker.rows.lock().unwrap().clear();
    let completed = duplicate.compute(&observed_key).await.unwrap();
    let completed = complete_observed_selected_graph(&completed);
    let observed_complete_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(
        observed_complete_row.iter().filter(|dependency| dependency == &&effective_dep).count(),
        1
    );
    let SourcePreparationOutcome::Complete(Ok(effective)) =
        duplicate.compute(&HostEffectiveModuleOverrideObservationKey::new(
            workspace.dupe(),
            "dep".into(),
        )).await.unwrap()
    else { panic!("effective carrier must complete") };
    for (demand, result) in effective.observations().observations() {
        assert!(Arc::ptr_eq(
            result,
            completed.observations().get(demand).unwrap()
        ));
    }
    tracker.rows.lock().unwrap().clear();
    let legacy_duplicate_key = HostSelectedModuleGraphKey::new(workspace.dupe());
    let SourcePreparationOutcome::Complete(legacy_duplicate) =
        duplicate.compute(&legacy_duplicate_key).await.unwrap()
    else { panic!("legacy duplicate graph must complete") };
    assert_eq!(legacy_duplicate.as_ref(), completed.result().as_ref());
    let legacy_complete_row = tracker.rows.lock().unwrap().iter()
        .find(|(owner, _)| owner == &legacy_duplicate_key.to_string()).unwrap().1.clone();
    assert_eq!(legacy_complete_row, legacy_dependency_order(&observed_complete_row));

    let implicit_root = "module(name='root')\n";
    let mut implicit = module_source_registry_transaction(
        &dice,
        tracker.dupe(),
        implicit_root,
        &["https://registry.invalid"],
        1011,
        PathObservationEpoch::empty(),
    )
    .await;
    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let implicit_value = implicit.compute(&observed_key).await.unwrap();
    let implicit_observed = complete_observed_selected_graph(&implicit_value);
    assert!(matches!(
        implicit_observed.result().as_ref(),
        Err(crate::selected_graph::HostSelectedModuleGraphError::DiscoveryLeaf {
            module: HostGraphModuleKey::Module { name, .. },
            ..
        }) if name == "rules_license"
    ));
    let observed_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &observed_key.to_string())
        .unwrap()
        .1
        .clone();
    let effective_tools =
        HostEffectiveModuleOverrideObservationKey::new(workspace.dupe(), "bazel_tools".into())
            .to_string();
    assert_eq!(
        observed_row
            .iter()
            .filter(|dependency| dependency == &&effective_tools)
            .count(),
        1
    );
    assert!(observed_row.iter().any(|dependency| {
        dependency == &observed_discovered_key("bazel_tools", "").to_string()
    }));
    let observed_events = discovered_eventful(&tracker);

    tracker.rows.lock().unwrap().clear();
    tracker.batches.lock().unwrap().clear();
    let legacy_key = HostSelectedModuleGraphKey::new(workspace);
    let SourcePreparationOutcome::Complete(legacy) =
        implicit.compute(&legacy_key).await.unwrap()
    else {
        panic!("legacy implicit builtin must complete")
    };
    assert_eq!(legacy.as_ref(), implicit_observed.result().as_ref());
    let legacy_row = tracker
        .rows
        .lock()
        .unwrap()
        .iter()
        .find(|(owner, _)| owner == &legacy_key.to_string())
        .unwrap()
        .1
        .clone();
    assert_eq!(legacy_row, legacy_dependency_order(&observed_row));
    assert_eq!(
        discovered_event_values(&discovered_eventful(&tracker)),
        discovered_event_values(&observed_events)
    );
    assert_eq!(
        discovered_eventful(&tracker)
            .iter()
            .map(|(owner, _)| owner.clone())
            .collect::<Vec<_>>(),
        legacy_dependency_order(
            &observed_events.iter().map(|(owner, _)| owner.clone()).collect::<Vec<_>>()
        ),
    );
}

#[tokio::test]
async fn observed_selected_graph_effective_override_restores_with_fixed_policy() {
    let io = Arc::new(ModuleSourceRegistryIo::new([
        (
            "https://registry.invalid/modules/dep/1/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='dep',version='1')\n".as_slice(),
            )),
        ),
        (
            "https://registry.invalid/modules/dep/2/MODULE.bazel",
            ModuleSourceRegistryResponse::Found(Arc::from(
                b"module(name='dep',version='2')\n".as_slice(),
            )),
        ),
    ]));
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let tracker = Arc::new(NonregistryPreflightTracker::default());
    let key = HostSelectedModuleGraphObservationKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
    );
    let roots = [
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\nsingle_version_override(module_name='dep',version='1')\n",
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\nsingle_version_override(module_name='dep',version='2')\n",
        "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\nsingle_version_override(module_name='dep',version='1')\n",
    ];
    let mut values = Vec::new();
    for root in roots {
        let mut transaction = module_source_registry_transaction(
            &dice,
            tracker.dupe(),
            root,
            &["https://registry.invalid"],
            1020,
            PathObservationEpoch::empty(),
        )
        .await;
        tracker.rows.lock().unwrap().clear();
        tracker.batches.lock().unwrap().clear();
        let value = transaction.compute(&key).await.unwrap();
        let observed = complete_observed_selected_graph(&value);
        assert!(observed.result().is_ok());
        assert_selected_epoch(
            &mut transaction,
            observed.observations(),
            observed.observations(),
        )
        .await;
        let row = tracker
            .rows
            .lock()
            .unwrap()
            .iter()
            .find(|(owner, _)| owner == &key.to_string())
            .unwrap()
            .1
            .clone();
        assert_eq!(
            row[..3],
            [
                RootModuleFilesObservationKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap()
                )
                .to_string(),
                RootModuleCommandPolicyKey {
                    workspace: PathBuf::from("/workspace")
                }
                .to_string(),
                HostEffectiveModuleOverrideObservationKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    "dep".into(),
                )
                .to_string(),
            ]
        );
        values.push(value);
    }
    assert!(HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[2]
    ));
    assert!(!HostSelectedModuleGraphObservationKey::equality(
        &values[0], &values[1]
    ));
    let held_result = complete_observed_selected_graph(&values[0]).result().dupe();
    let held_epoch = complete_observed_selected_graph(&values[0])
        .observations()
        .dupe();
    assert!(held_result.is_ok());
    assert!(!held_epoch.observations().is_empty());
}
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryName;
