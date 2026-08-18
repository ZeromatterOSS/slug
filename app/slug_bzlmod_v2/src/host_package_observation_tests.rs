use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::ObservedExternalRepositoryPackageLookup;
use crate::host_package::ObservedRepositoryPackageSource;
use crate::host_package::RepositoryPackageSourceObservationKey;
use crate::host_package::project_legacy_repository_package_source;
use crate::host_package::repository_package_source_observed_child;
use crate::host_package::union_observations;
use crate::repository_ignore::HostRouteRepositoryIgnoreKey;
use crate::repository_ignore::HostRouteRepositoryIgnoreObservationKey;
use crate::source_preparation::HostRepositoryPathKey;
use crate::source_preparation::HostRepositoryPathObservationKey;
use crate::source_preparation::HostRepositorySourceFileObservationKey;
use crate::source_preparation::HostRepositorySourceFileValue;
use crate::source_preparation::direct_local_module_support_observed;

fn observed_external_key(
    root: &str,
    package: &str,
) -> ExternalRepositoryPackageLookupObservationKey {
    let route = local_route(root);
    ExternalRepositoryPackageLookupObservationKey::new(
        route.clone(),
        PackageIdentifier::new(
            route.canonical_repo().clone(),
            PackagePath::parse(package).unwrap(),
        ),
    )
    .unwrap()
}


fn observed_source_key(
    root: &str,
    package: &str,
) -> RepositoryPackageSourceObservationKey {
    let route = local_route(root);
    RepositoryPackageSourceObservationKey::new(
        route.clone(),
        PackageIdentifier::new(
            route.canonical_repo().clone(),
            PackagePath::parse(package).unwrap(),
        ),
    )
    .unwrap()
}

fn observed_source_value(
    outcome: &<RepositoryPackageSourceObservationKey as Key>::Value,
) -> &ObservedRepositoryPackageSource {
    let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("observed package source did not complete semantically: {outcome:?}")
    };
    value
}

fn assert_exact_source_epoch(expected: &PathObservationEpoch, actual: &PathObservationEpoch) {
    assert_eq!(actual, expected);
    for (demand, result) in expected.observations() {
        assert!(Arc::ptr_eq(actual.get(demand).unwrap(), result));
    }
}

fn source_entries(
    primary: Option<&'static [u8]>,
    fallback: Option<&'static [u8]>,
    variant: i64,
) -> Vec<ScriptEntry> {
    let mut entries = route_prelude("dep", Some(b"print('REPO')\n"), None, variant);
    entries.push(present(
        "/workspace/dep/pkg",
        PathNodeKind::Directory,
        variant,
    ));
    for (name, value) in [("BUILD.bazel", primary), ("BUILD", fallback)] {
        let path = format!("/workspace/dep/pkg/{name}");
        match value {
            Some(value) => {
                entries.push(present(&path, PathNodeKind::RegularFile, variant));
                entries.push(bytes(&path, value));
            }
            None => entries.push(missing(&path)),
        }
    }
    entries
}

#[tokio::test]
async fn observed_package_source_retains_exact_children_bytes_events_and_lifecycle() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(LookupActivationTracker::default());
    let key = observed_source_key("dep", "pkg");
    assert_eq!(
        key.to_string(),
        "observed-repository-package-source:@@dep+//pkg"
    );

    let first_entries = source_entries(Some(b"A"), Some(b"fallback"), 200);
    let mut first =
        observed_transaction(&dice, first_entries.clone(), &[], tracker.dupe()).await;
    let cold_outcome = first.compute(&key).await.unwrap();
    let cold = observed_source_value(&cold_outcome);
    let source = cold.result().as_ref().as_ref().unwrap();
    assert_eq!(source.build_file_name(), "BUILD.bazel");
    assert_eq!(source.bytes().as_ref(), b"A");

    let route = local_route("dep");
    let support = direct_local_module_support_observed(&mut first, &route).await;
    let SourcePreparationOutcome::Complete(Ok(support)) = support else {
        panic!("observed support must complete")
    };
    let lookup = first
        .compute(&observed_external_key("dep", "pkg"))
        .await
        .unwrap();
    let lookup = observed_value(&lookup);
    let selected = first
        .compute(&HostRepositorySourceFileObservationKey::new(
            route.clone(),
            PathBuf::from("pkg/BUILD.bazel"),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(selected)) = selected else {
        panic!("observed selected source must complete")
    };
    let expected =
        union_observations(support.observations(), lookup.observations()).unwrap();
    let expected = union_observations(&expected, selected.observations()).unwrap();
    assert_exact_source_epoch(&expected, cold.observations());
    let Ok(HostRepositorySourceFileValue::Present { bytes, .. }) =
        selected.result().as_ref()
    else {
        panic!("selected source must be present")
    };
    assert!(Arc::ptr_eq(source.bytes(), bytes));

    let cold_result = cold.result().dupe();
    let activations = tracker.take();
    assert!(activations.iter().all(|(name, batch)| {
        !name.starts_with("observed-repository-package-source:") || batch.is_none()
    }));
    assert!(activations.iter().any(|(name, batch)| {
        name.starts_with("observed-host-route-repo-file:")
            && matches!(
                batch.as_ref().map(EventBatch::events),
                Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "REPO"
            )
    }));
    let batches: Vec<_> = activations
        .iter()
        .filter_map(|(name, batch)| batch.as_ref().map(|batch| (name, batch.events())))
        .collect();
    assert!(matches!(
        batches.as_slice(),
        [(root, []), (evaluation, []), (repo, [EvaluationEvent::StarlarkPrint { text, .. }])]
            if root.starts_with("bzlmod-observed-host-root-module-file:")
                && evaluation.starts_with("observed-direct-local-module-evaluation:")
                && repo.starts_with("observed-host-route-repo-file:")
                && text == "REPO"
    ));
    assert!(!activations.iter().any(|(name, _)| [
        "bzlmod-host-root-module-file:",
        "direct-local-module-evaluation:",
        "direct-local-module-preparation:",
        "external-repository-package-lookup:",
        "host-repository-path:",
        "host-repository-source-file:",
        "host-route-repo-file:",
        "host-route-repository-ignore:",
        "repository-package-source:",
        "root-repository-route:",
    ]
    .iter()
    .any(|legacy| name.starts_with(legacy))
        || name.contains("external-bzl")
        || name.contains("repository-package-load")
        || name.contains("query")
        || name.contains("build-command")));

    let warm = first.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        observed_source_value(&warm).result(),
        &cold_result
    ));
    assert!(tracker
        .take()
        .iter()
        .all(|(_, batch)| batch.is_none()));

    let mut legacy =
        observed_transaction(&dice, first_entries.clone(), &[], tracker.dupe()).await;
    let legacy = legacy
        .compute(&external_source_key("dep", "pkg"))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy source must complete")
    };
    assert_eq!(legacy.as_ref(), cold_result.as_ref());
    assert!(!tracker
        .take()
        .iter()
        .any(|(name, _)| name.starts_with("observed-")));

    let second_entries = source_entries(None, Some(b"B"), 201);
    let mut second =
        observed_transaction(&dice, second_entries, &[], tracker.dupe()).await;
    let second = second.compute(&key).await.unwrap();
    let second = observed_source_value(&second).result().as_ref().as_ref().unwrap();
    assert_eq!(second.build_file_name(), "BUILD");
    assert_eq!(second.bytes().as_ref(), b"B");

    let mut restored =
        observed_transaction(&dice, first_entries, &[], tracker.dupe()).await;
    let restored = restored.compute(&key).await.unwrap();
    assert_eq!(
        observed_source_value(&restored).result().as_ref(),
        cold_result.as_ref()
    );
}

#[tokio::test]
async fn observed_package_source_need_outer_cancellation_and_projection_keep_polarity() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(LookupActivationTracker::default());
    let key = observed_source_key("dep", "pkg");

    let mut cancelled =
        observed_transaction(&dice, Vec::new(), &[], tracker.dupe()).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(future.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().iter().all(|(_, batch)| batch.is_none()));

    let mut recovered = observed_transaction(
        &dice,
        source_entries(Some(b"recovered"), None, 209),
        &[],
        tracker.dupe(),
    )
    .await;
    assert!(observed_source_value(&recovered.compute(&key).await.unwrap())
        .result()
        .is_ok());
    tracker.take();
    for (position, entries) in [Vec::new(), route_prelude("dep", None, None, 209)]
        .into_iter()
        .enumerate()
    {
        let mut transaction =
            observed_transaction(&dice, entries, &[], tracker.dupe()).await;
        let outcome = transaction.compute(&key).await.unwrap();
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
        let activations = tracker.take();
        assert!(!activations.iter().any(|(name, _)| {
            name.starts_with("observed-host-repository-source-file:")
                && name.contains(":pkg/")
                || position == 0
                    && name.starts_with("observed-external-repository-package-lookup:")
                    && name.contains("PackagePath(\"pkg\")")
        }));
    }

    let mut pending = route_prelude("dep", None, None, 210);
    pending.extend([
        present("/workspace/dep/pkg", PathNodeKind::Directory, 210),
        missing("/workspace/dep/pkg/BUILD.bazel"),
        present(
            "/workspace/dep/pkg/BUILD",
            PathNodeKind::RegularFile,
            210,
        ),
    ]);
    let mut transaction =
        observed_transaction(&dice, pending, &[], tracker.dupe()).await;
    let need = transaction.compute(&key).await.unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    assert!(!RepositoryPackageSourceObservationKey::validity(&need));
    assert!(!RepositoryPackageSourceObservationKey::equality(&need, &need));
    assert!(tracker.take().iter().all(|(name, batch)| {
        !name.starts_with("observed-repository-package-source:") || batch.is_none()
    }));

    let conflict = ObservedPathFrontierError::from(
        slug_workspace_v2::PathObservationEpochError::ConflictingDemand(demand(
            "/workspace/dep/pkg/BUILD.bazel",
            PathObservationOperation::Lstat,
        )),
    );
    for source_position in [false, true] {
        let entries = source_entries(Some(b"A"), None, 211);
        let transaction =
            observed_transaction(&dice, entries, &[], tracker.dupe()).await;
        let mut updater = transaction.into_updater();
        let route = local_route("dep");
        if source_position {
            updater
                .changed_to(vec![(
                    HostRepositorySourceFileObservationKey::new(
                        route,
                        PathBuf::from("pkg/BUILD.bazel"),
                    ),
                    SourcePreparationOutcome::Complete(Err(conflict.clone())),
                )])
                .unwrap();
        } else {
            updater
                .changed_to(vec![(
                    observed_external_key("dep", "pkg"),
                    SourcePreparationOutcome::Complete(Err(conflict.clone())),
                )])
                .unwrap();
        }
        let outcome = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(Err(ref error)) if error == &conflict
        ));
        assert!(RepositoryPackageSourceObservationKey::validity(&outcome));
        assert!(RepositoryPackageSourceObservationKey::equality(
            &outcome, &outcome
        ));
        let activations = tracker.take();
        assert!(source_position
            || !activations.iter().any(|(name, _)| {
                name.starts_with("observed-host-repository-source-file:")
                    && name.contains(":pkg/")
            }));
    }

    let entries = source_entries(Some(b"A"), None, 212);
    let transaction = observed_transaction(&dice, entries, &[], tracker.dupe()).await;
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            observed_external_key("dep", "pkg"),
            SourcePreparationOutcome::Complete(Ok(
                ObservedExternalRepositoryPackageLookup {
                    result: Arc::new(Ok(ExternalRepositoryPackageLookup::Package(
                        HostBuildFileName::BuildDotBazel,
                    ))),
                    observations: PathObservationEpoch::new([(
                        demand(
                            "/workspace/dep/MODULE.bazel",
                            PathObservationOperation::FileBytes,
                        ),
                        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                            b"conflict".as_slice(),
                        ))),
                    )])
                    .unwrap(),
                },
            )),
        )])
        .unwrap();
    let union_conflict = updater.commit().await.compute(&key).await.unwrap();
    assert!(matches!(
        union_conflict,
        SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::ConflictingDemand(_)
        )))
    ));

    for (package, entries, deleted, case) in [
        ("pkg", route_prelude("dep", None, Some(b"pkg\n"), 213), &[][..], 0),
        ("pkg", source_entries(None, None, 214), &[][..], 1),
        ("pkg", route_prelude("dep", None, None, 215), &["@dep+//pkg"][..], 2),
        ("bad:name", route_prelude("dep", None, None, 216), &[][..], 3),
        ("pkg", route_prelude("dep", None, Some(b"/absolute\n"), 217), &[][..], 4),
    ] {
        let case_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction =
            observed_transaction(&case_dice, entries, deleted, tracker.dupe()).await;
        let support = direct_local_module_support_observed(&mut transaction, &local_route("dep")).await;
        let SourcePreparationOutcome::Complete(Ok(support)) = support else {
            panic!("support must complete")
        };
        let lookup = transaction
            .compute(&observed_external_key("dep", package))
            .await
            .unwrap();
        let lookup = observed_value(&lookup);
        let expected = union_observations(support.observations(), lookup.observations()).unwrap();
        let case_key = observed_source_key("dep", package);
        let outcome = transaction.compute(&case_key).await.unwrap();
        let value = observed_source_value(&outcome);
        assert!(matches!(
            (case, &value.result().as_ref().as_ref().unwrap_err().inner),
            (0 | 2, RepositoryPackageSourceErrorInner::Deleted { .. })
                | (1, RepositoryPackageSourceErrorInner::NoBuildFile { .. })
                | (3, RepositoryPackageSourceErrorInner::InvalidPackageName { .. })
                | (4, RepositoryPackageSourceErrorInner::Lookup { .. })
        ));
        assert_exact_source_epoch(&expected, value.observations());
        assert!(RepositoryPackageSourceObservationKey::validity(&outcome)
            && RepositoryPackageSourceObservationKey::equality(&outcome, &outcome));
    }

    let replace_module = |entries: &mut Vec<ScriptEntry>, source| {
        let module = demand(
            "/workspace/dep/MODULE.bazel",
            PathObservationOperation::FileBytes,
        );
        entries.retain(|(found, _)| found != &module);
        entries.push(bytes("/workspace/dep/MODULE.bazel", source));
    };
    let mut evaluation = route_prelude("dep", None, None, 218);
    replace_module(&mut evaluation, b"fail('support-evaluation')\n");
    let cycle_source = b"include('//p:a.MODULE.bazel')\n";
    let mut unsupported = route_prelude("dep", None, None, 219);
    replace_module(&mut unsupported, cycle_source);
    unsupported.extend([
        present("/workspace/dep/p", PathNodeKind::Directory, 219),
        present(
            "/workspace/dep/p/BUILD.bazel",
            PathNodeKind::RegularFile,
            219,
        ),
        present("/workspace/dep/p/a.MODULE.bazel", PathNodeKind::RegularFile, 219),
        bytes("/workspace/dep/p/a.MODULE.bazel", cycle_source),
    ]);
    tracker.take();
    for (entries, is_unsupported) in [(evaluation, false), (unsupported, true)] {
        let case_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction =
            observed_transaction(&case_dice, entries, &[], tracker.dupe()).await;
        let support = direct_local_module_support_observed(&mut transaction, &local_route("dep")).await;
        let SourcePreparationOutcome::Complete(Ok(support)) = support else {
            panic!("support terminal must complete")
        };
        let outcome = transaction.compute(&key).await.unwrap();
        let value = observed_source_value(&outcome);
        assert!(matches!(
            (&value.result().as_ref().as_ref().unwrap_err().inner, is_unsupported),
            (RepositoryPackageSourceErrorInner::Unsupported { .. }, true)
                | (RepositoryPackageSourceErrorInner::ModuleEvaluation { .. }, false)
        ));
        assert_exact_source_epoch(support.observations(), value.observations());
        assert!(!tracker.take().iter().any(|(name, _)| {
            name.starts_with("observed-external-repository-package-lookup:")
                && name.contains("PackagePath(\"pkg\")")
                || name.starts_with("observed-host-repository-source-file:")
                    && name.contains(":pkg/")
        }));
    }

    let held = Arc::new(Err(RepositoryPackageSourceError::new(
        RepositoryPackageSourceErrorInner::NoBuildFile {
            package: PackageIdentifier::new(
                local_route("dep").canonical_repo().clone(),
                PackagePath::parse("pkg").unwrap(),
            ),
        },
    )));
    let projected = project_legacy_repository_package_source(
        SourcePreparationOutcome::Complete(Ok(ObservedRepositoryPackageSource {
            result: held.dupe(),
            observations: PathObservationEpoch::empty(),
        })),
    );
    let SourcePreparationOutcome::Complete(projected) = projected else {
        panic!("legacy projection must complete")
    };
    assert!(Arc::ptr_eq(&held, &projected));
}

#[test]
fn observed_package_source_support_outer_stops_in_shared_child_reducer() {
    let outer = ObservedPathFrontierError::Epoch(
        slug_workspace_v2::PathObservationEpochError::OperationMismatch {
            demand: demand(
                "/workspace/dep/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
            result_operation: PathObservationOperation::Lstat,
        },
    );
    let ControlFlow::Break(outcome) = repository_package_source_observed_child::<()>(
        SourcePreparationOutcome::Complete(Err(outer.clone())),
    ) else {
        panic!("support outer must stop the source driver")
    };
    assert!(matches!(
        outcome,
        SourcePreparationOutcome::Complete(Err(found)) if found == outer
    ));
}

fn observed_value(
    outcome: &<ExternalRepositoryPackageLookupObservationKey as Key>::Value,
) -> &ObservedExternalRepositoryPackageLookup {
    let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("observed external lookup did not complete semantically: {outcome:?}")
    };
    value
}

#[derive(Default)]
struct LookupActivationTracker(Mutex<Vec<(String, Option<EventBatch>)>>);

impl LookupActivationTracker {
    fn take(&self) -> Vec<(String, Option<EventBatch>)> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

impl ActivationTracker for LookupActivationTracker {
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
        self.0.lock().unwrap().push((
            key.to_string(),
            activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        ));
    }
}

async fn observed_transaction(
    dice: &Arc<Dice>,
    entries: Vec<ScriptEntry>,
    deleted: &[&str],
    tracker: Arc<LookupActivationTracker>,
) -> DiceTransaction {
    let mut data = UserComputationData {
        activation_tracker: Some(tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    inject_root_package_policy_inputs(&mut updater, inputs(&[], deleted, None)).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        std::path::Path::new("/workspace"),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch(&entries))])
        .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: path("/workspace"),
            },
            route_materialization("dep"),
        )])
        .unwrap();
    updater.commit().await
}

#[tokio::test]
async fn observed_external_lookup_retains_ordered_prefixes_and_exact_child_arcs() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(LookupActivationTracker::default());
    let mut entries = route_prelude("dep", Some(b"print('LOOKUP')\n"), None, 80);
    entries.extend([
        present("/workspace/dep/pkg", PathNodeKind::Directory, 80),
        present(
            "/workspace/dep/pkg/BUILD.bazel",
            PathNodeKind::Directory,
            80,
        ),
        present(
            "/workspace/dep/pkg/BUILD",
            PathNodeKind::SpecialFile,
            80,
        ),
    ]);
    let legacy_entries = entries.clone();
    let mut transaction = observed_transaction(&dice, entries, &[], tracker.dupe()).await;
    let key = observed_external_key("dep", "pkg");
    let cold = transaction.compute(&key).await.unwrap();
    let cold = observed_value(&cold);
    assert!(matches!(
        cold.result().as_ref(),
        Ok(ExternalRepositoryPackageLookup::Package(HostBuildFileName::Build))
    ));

    let route = local_route("dep");
    let ignore = transaction
        .compute(&HostRouteRepositoryIgnoreObservationKey(
            HostRouteRepositoryIgnoreKey::new(route.clone()),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(ignore)) = ignore else {
        panic!("observed ignore must complete")
    };
    for (demand, result) in ignore.observations().observations() {
        assert!(Arc::ptr_eq(
            cold.observations().get(demand).unwrap(),
            result
        ));
    }
    for marker in ["pkg/BUILD.bazel", "pkg/BUILD"] {
        let path = transaction
            .compute(&HostRepositoryPathObservationKey(
                HostRepositoryPathKey::new(route.clone(), PathBuf::from(marker)),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(path)) = path else {
            panic!("observed marker path must complete")
        };
        for (demand, result) in path.observations.observations() {
            assert!(Arc::ptr_eq(
                cold.observations().get(demand).unwrap(),
                result
            ));
        }
    }

    let warm = transaction.compute(&key).await.unwrap();
    let warm = observed_value(&warm);
    assert!(Arc::ptr_eq(cold.result(), warm.result()));
    let activations = tracker.take();
    assert!(activations.iter().any(|(key, batch)| {
        key.starts_with("observed-host-route-repo-file:")
            && matches!(
                batch.as_ref().map(EventBatch::events),
                Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "LOOKUP"
            )
    }));
    assert!(activations.iter().all(|(key, batch)| {
        !key.starts_with("observed-external-repository-package-lookup:") || batch.is_none()
    }));
    let child = activations
        .iter()
        .position(|(key, _)| key.starts_with("observed-host-route-repo-file:"))
        .unwrap();
    let parent = activations
        .iter()
        .position(|(key, _)| key.starts_with("observed-external-repository-package-lookup:"))
        .unwrap();
    assert!(child < parent);
    assert!(!activations.iter().any(|(key, _)| {
        key.starts_with("external-repository-package-lookup:")
            || key.starts_with("host-route-repository-ignore:")
            || key.starts_with("host-repository-path:")
    }));

    let mut legacy = observed_transaction(&dice, legacy_entries, &[], tracker.dupe()).await;
    legacy.compute(&external_key("dep", "pkg")).await.unwrap();
    assert!(!tracker
        .take()
        .iter()
        .any(|(key, _)| key.starts_with("observed-")));
}

#[tokio::test]
async fn observed_external_lookup_distinguishes_deleted_prefixes_and_marker_priority() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(LookupActivationTracker::default());
    let key = observed_external_key("dep", "pkg");

    let mut canonical =
        observed_transaction(&dice, Vec::new(), &["@dep+//pkg"], tracker.dupe()).await;
    let canonical = canonical.compute(&key).await.unwrap();
    let canonical = observed_value(&canonical);
    assert!(matches!(
        canonical.result().as_ref(),
        Ok(ExternalRepositoryPackageLookup::Deleted)
    ));
    assert!(canonical.observations().observations().is_empty());

    let ignored_entries = route_prelude("dep", None, Some(b"pkg\n"), 81);
    let mut ignored =
        observed_transaction(&dice, ignored_entries, &[], tracker.dupe()).await;
    let ignored = ignored.compute(&key).await.unwrap();
    let ignored = observed_value(&ignored);
    assert!(matches!(
        ignored.result().as_ref(),
        Ok(ExternalRepositoryPackageLookup::Deleted)
    ));
    assert!(!ignored.observations().observations().is_empty());

    let cases = [
        (
            Some(PathNodeKind::RegularFile),
            Some(PathNodeKind::RegularFile),
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
        ),
        (
            Some(PathNodeKind::Directory),
            Some(PathNodeKind::RegularFile),
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::Build),
        ),
        (None, None, ExternalRepositoryPackageLookup::NoBuildFile),
        (
            Some(PathNodeKind::RegularFile),
            None,
            ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
        ),
    ];
    let mut outcomes = Vec::new();
    for (offset, (primary, fallback, expected)) in cases.into_iter().enumerate() {
        let variant = 90 + offset as i64;
        let mut entries = route_prelude("dep", None, None, variant);
        entries.push(present(
            "/workspace/dep/pkg",
            PathNodeKind::Directory,
            variant,
        ));
        for (name, kind) in [("BUILD.bazel", primary), ("BUILD", fallback)] {
            let marker = format!("/workspace/dep/pkg/{name}");
            entries.push(match kind {
                Some(kind) => present(&marker, kind, variant),
                None => missing(&marker),
            });
        }
        let mut transaction =
            observed_transaction(&dice, entries, &[], tracker.dupe()).await;
        let value = transaction.compute(&key).await.unwrap();
        let value = observed_value(&value);
        assert_eq!(value.result().as_ref().as_ref().unwrap(), &expected);
        outcomes.push(value.result().dupe());
    }
    assert_eq!(outcomes[0], outcomes[3]);
    assert_ne!(outcomes[0], outcomes[1]);
}

#[tokio::test]
async fn observed_external_lookup_child_need_outer_and_semantic_prefixes_keep_polarity() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(LookupActivationTracker::default());
    let key = observed_external_key("dep", "pkg");
    let mut cancelled = observed_transaction(&dice, Vec::new(), &[], tracker.dupe()).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(future.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    assert!(tracker.take().iter().all(|(_, batch)| batch.is_none()));
    let mut first = route_prelude("dep", None, None, 110);
    let mut second = first.clone();
    second.extend([
        present("/workspace/dep/pkg", PathNodeKind::Directory, 110),
        missing("/workspace/dep/pkg/BUILD.bazel"),
    ]);
    let mut recovery = second.clone();
    recovery.push(present("/workspace/dep/pkg/BUILD", PathNodeKind::RegularFile, 110));
    let mut recovered = observed_transaction(&dice, recovery, &[], tracker.dupe()).await;
    let recovered = recovered.compute(&key).await.unwrap();
    assert!(observed_value(&recovered).result().is_ok());
    for entries in [Vec::new(), std::mem::take(&mut first), second.clone()] {
        let mut transaction = observed_transaction(&dice, entries, &[], tracker.dupe()).await;
        let outcome = transaction.compute(&key).await.unwrap();
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
        assert!(!ExternalRepositoryPackageLookupObservationKey::validity(&outcome)
            && !ExternalRepositoryPackageLookupObservationKey::equality(&outcome, &outcome));
    }
    let conflict = ObservedPathFrontierError::from(
        slug_workspace_v2::PathObservationEpochError::ConflictingDemand(demand(
            "/workspace/dep/pkg/BUILD.bazel",
            PathObservationOperation::Lstat,
        )),
    );
    for position in 0..3 {
        let entries = if position == 2 {
            second.clone()
        } else {
            route_prelude("dep", None, None, 111)
        };
        let transaction = observed_transaction(&dice, entries, &[], tracker.dupe()).await;
        let mut updater = transaction.into_updater();
        let route = local_route("dep");
        if position == 0 {
            updater
                .changed_to(vec![(
                    HostRouteRepositoryIgnoreObservationKey(
                        HostRouteRepositoryIgnoreKey::new(route),
                    ),
                    SourcePreparationOutcome::Complete(Err(conflict.clone())),
                )])
                .unwrap();
        } else {
            let marker = if position == 1 {
                "pkg/BUILD.bazel"
            } else {
                "pkg/BUILD"
            };
            updater
                .changed_to(vec![(
                    HostRepositoryPathObservationKey(HostRepositoryPathKey::new(
                        route,
                        PathBuf::from(marker),
                    )),
                    SourcePreparationOutcome::Complete(Err(conflict.clone())),
                )])
                .unwrap();
        }
        let outcome = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(outcome, SourcePreparationOutcome::Complete(Err(ref error)) if error == &conflict));
        assert!(ExternalRepositoryPackageLookupObservationKey::validity(&outcome)
            && ExternalRepositoryPackageLookupObservationKey::equality(&outcome, &outcome));
    }
    let mut path_error = route_prelude("dep", None, None, 113);
    path_error.extend([
        present("/workspace/dep/pkg", PathNodeKind::Directory, 113),
        lstat_error("/workspace/dep/pkg/BUILD.bazel"),
    ]);
    for entries in [
        route_prelude("dep", None, Some(b"/absolute\n"), 112),
        path_error,
    ] {
        let semantic_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction =
            observed_transaction(&semantic_dice, entries, &[], tracker.dupe()).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let value = observed_value(&outcome);
        assert!(value.result().is_err());
        assert!(!value.observations().observations().is_empty());
    }
}
use std::ops::ControlFlow;
