use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::ObservedExternalRepositoryPackageLookup;
use crate::repository_ignore::HostRouteRepositoryIgnoreKey;
use crate::repository_ignore::HostRouteRepositoryIgnoreObservationKey;
use crate::source_preparation::HostRepositoryPathKey;
use crate::source_preparation::HostRepositoryPathObservationKey;

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
