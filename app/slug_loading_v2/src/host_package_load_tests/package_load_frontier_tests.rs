//! Actual package inventory child-load prerequisites, without native results.
use super::*;

const BUILD: &[u8] = b"load(':a.bzl','A')\nload(':b.bzl','B')\nprint('INVENTORY_EVALUATED')\nfilegroup(name=A)\nfilegroup(name=B)\n";

fn omitted() -> [PathObservationDemand; 2] {
    ["a.bzl", "b.bzl"].map(|name| {
        EpochBuilder::demand(
            &format!("/workspace/dep/{name}"),
            PathObservationOperation::Lstat,
        )
    })
}
struct Fixture {
    tx: DiceTransaction,
    route: RootRepositoryRoute,
    key: RepositoryPackageInventoryObservationKey,
    full: PathObservationEpoch,
    tracker: Arc<EventTracker>,
}
async fn fixture() -> Fixture {
    let builder = EpochBuilder::external_sources(
        &[
            ("BUILD.bazel", BUILD),
            ("a.bzl", b"A='first'\n"),
            ("b.bzl", b"B='second'\n"),
        ],
        901,
    );
    fixture_from(builder.build(), &omitted()).await
}

async fn fixture_from(full: PathObservationEpoch, excluded: &[PathObservationDemand]) -> Fixture {
    let cold = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| !excluded.contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let tracker = Arc::new(EventTracker::default());
    let dice = host_fixture_dice();
    let mut tx = transaction(&dice, cold, true, Some(tracker.dupe())).await;
    let route = external_route(&mut tx).await;
    let key = RepositoryPackageInventoryObservationKey::new(
        HostRepositorySourceRoute::root(route.clone()),
        PackagePath::parse("").unwrap(),
    );
    tracker.take();
    Fixture {
        tx,
        route,
        key,
        full,
        tracker,
    }
}
fn no_inventory_batch(
    tracker: &EventTracker,
    key: &RepositoryPackageInventoryObservationKey,
) -> Vec<TrackedBatch> {
    let rows = tracker.take();
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    assert!(
        rows.iter()
            .filter_map(|row| row.batch.as_ref())
            .all(|batch| !event_texts(batch).contains(&"INVENTORY_EVALUATED"))
    );
    rows
}

#[tokio::test]
async fn observed_inventory_load_siblings_collect_independent_lstat_prerequisites() {
    let Fixture {
        mut tx,
        key,
        tracker,
        full,
        ..
    } = fixture().await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(needs) = &value else {
        panic!("unobserved child files must suspend inventory: {value:?}")
    };
    let expected = slug_bzlmod_v2::SourcePreparationNeeds::path(
        slug_workspace_v2::NeedPathObservations::try_from_iter(omitted()).unwrap(),
    );
    assert_eq!(
        needs, &expected,
        "independent actual child loads must contribute both whole Lstat prerequisites"
    );
    assert!(!RepositoryPackageInventoryObservationKey::validity(&value));
    assert!(!RepositoryPackageInventoryObservationKey::equality(
        &value, &value
    ));
    // Count the actual returned DICE prerequisite keys, after their complete
    // semantic identity is established above. Construction pays field hashing;
    // repeated operational routing must read one cached scalar per demand.
    use std::hash::Hash;
    use std::hash::Hasher;
    #[derive(Default)]
    struct HashWork {
        writes: usize,
        bytes: usize,
    }
    impl Hasher for HashWork {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, bytes: &[u8]) {
            self.writes += 1;
            self.bytes += bytes.len();
        }
    }
    for demand in needs.path_observations().unwrap().demands() {
        let mut work = HashWork::default();
        for _ in 0..64 {
            demand.hash(&mut work);
        }
        assert_eq!(
            (work.writes, work.bytes),
            (64, 64 * std::mem::size_of::<u64>()),
            "actual inventory demand repeated hashes must have fixed postconstruction work"
        );
    }
    let rows = no_inventory_batch(&tracker, &key);
    for name in ["a.bzl", "b.bzl"] {
        assert!(
            rows.iter()
                .any(|row| row.key == format!("observed-external-bzl-module:@@dep+//:{name}"))
        );
    }
    // Fulfill exactly the discovered whole Need, preserving all prior facts.
    // A later withheld fact cannot sneak in through an unconditional full epoch.
    let returned = needs.path_observations().unwrap().demands();
    let retained = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| !omitted().contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let supplied = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| returned.contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    assert_eq!(supplied.observations().len(), returned.len());
    let fulfilled = retained.try_union(&supplied).unwrap();
    let mut tx = update(tx, &fulfilled).await;
    let complete = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &complete else {
        panic!("one fulfillment round must finish both real child loads: {complete:?}")
    };
    assert_eq!(
        observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, full.get(d).unwrap()));
    }
}

#[tokio::test]
async fn observed_inventory_load_siblings_repair_preserves_targets_order_and_certificate() {
    let Fixture {
        mut tx,
        route,
        key,
        full,
        tracker,
    } = fixture().await;
    let cold = tx.compute(&key).await.unwrap();
    assert!(matches!(cold, LoadingPreparationOutcome::Need(_)));
    no_inventory_batch(&tracker, &key);
    let mut update = tx.into_updater();
    update.changed_to(path_observation_shards(&full)).unwrap();
    update
        .changed_to([(PathObservationEpochKey, full.dupe())])
        .unwrap();
    let mut tx = update.commit().await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("real child observations must repair inventory: {value:?}")
    };
    let package = observed.result().as_ref().as_ref().unwrap();
    assert_eq!(
        package
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    let batches = tracker.take();
    assert_eq!(
        batches
            .iter()
            .filter(|row| row.key == key.to_string())
            .filter_map(|row| row.batch.as_ref())
            .flat_map(event_texts)
            .collect::<Vec<_>>(),
        ["INVENTORY_EVALUATED"]
    );

    let source = tx
        .compute(
            &RepositoryPackageSourceObservationKey::new(
                route.clone(),
                PackageIdentifier::new(
                    route.canonical_repo().clone(),
                    PackagePath::parse("").unwrap(),
                ),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(source)) = source else {
        panic!("actual BUILD source must complete")
    };
    let mut expected = source.observations().dupe();
    for name in ["a.bzl", "b.bzl"] {
        let child = tx
            .compute(&observed_external_bzl_key(route.clone(), "", name))
            .await
            .unwrap();
        expected = expected
            .try_union(observed_external(&child).observations())
            .unwrap();
        assert!(
            observed
                .observations()
                .get(&EpochBuilder::demand(
                    &format!("/workspace/dep/{name}"),
                    PathObservationOperation::FileBytes
                ))
                .is_some()
        );
    }
    assert_same_epoch_arcs(observed.observations(), &expected);
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, full.get(demand).unwrap()));
    }
    assert!(RepositoryPackageInventoryObservationKey::validity(&value));
}

const THREE_LOADS: &[u8] = b"load(':a.bzl','A')\nload(':b.bzl','B')\nload(':c.bzl','C')\nprint('INVENTORY_EVALUATED')\nfilegroup(name=A)\nfilegroup(name=B)\nfilegroup(name=C)\n";
fn file_demand(name: &str, operation: PathObservationOperation) -> PathObservationDemand {
    EpochBuilder::demand(&format!("/workspace/dep/{name}.bzl"), operation)
}
async fn update(tx: DiceTransaction, epoch: &PathObservationEpoch) -> DiceTransaction {
    let mut update = tx.into_updater();
    update.changed_to(path_observation_shards(epoch)).unwrap();
    update
        .changed_to([(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    update.commit().await
}
type InventoryOutcome = <RepositoryPackageInventoryObservationKey as Key>::Value;
fn pending(value: &InventoryOutcome, demands: &[PathObservationDemand]) {
    let LoadingPreparationOutcome::Need(actual) = value else {
        panic!("expected ordered transient inventory prerequisite: {value:?}")
    };
    assert_eq!(
        actual,
        &slug_bzlmod_v2::SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::try_from_iter(demands.iter().cloned())
                .unwrap()
        )
    );
    assert!(!RepositoryPackageInventoryObservationKey::validity(value));
}
fn no_c_or_parent_batch(
    tracker: &EventTracker,
    key: &RepositoryPackageInventoryObservationKey,
) -> Vec<TrackedBatch> {
    let rows = tracker.take();
    assert!(
        !rows
            .iter()
            .any(|row| row.key == "observed-external-bzl-module:@@dep+//:c.bzl")
    );
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    assert!(
        rows.iter()
            .filter_map(|row| row.batch.as_ref())
            .all(|batch| !event_texts(batch).contains(&"INVENTORY_EVALUATED"))
    );
    rows
}

#[tokio::test]
async fn observed_inventory_lstat_window_stops_at_whole_file_bytes_barrier() {
    let full = EpochBuilder::external_sources(
        &[
            ("BUILD.bazel", THREE_LOADS),
            ("a.bzl", b"A='first'\n"),
            ("b.bzl", b"B='second'\n"),
            ("c.bzl", b"C='third'\n"),
        ],
        902,
    )
    .build();
    let a = file_demand("a", PathObservationOperation::Lstat);
    let b = file_demand("b", PathObservationOperation::FileBytes);
    let Fixture {
        mut tx,
        key,
        tracker,
        ..
    } = fixture_from(full.dupe(), &[a.dupe(), b.dupe()]).await;
    pending(&tx.compute(&key).await.unwrap(), &[a]);
    no_c_or_parent_batch(&tracker, &key);
    let bytes_pending = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| *d != &b)
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let mut tx = update(tx, &bytes_pending).await;
    pending(&tx.compute(&key).await.unwrap(), &[b]);
    no_c_or_parent_batch(&tracker, &key);
    let mut tx = update(tx, &full).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("restored bytes must complete: {value:?}")
    };
    assert_eq!(
        observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .targets
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, full.get(d).unwrap()));
    }
}

#[tokio::test]
async fn observed_inventory_deferred_error_keeps_selected_event_closure_and_first_error() {
    let full = EpochBuilder::external_sources(
        &[
            ("BUILD.bazel", THREE_LOADS),
            ("a.bzl", b"A='first'\n"),
            (
                "b.bzl",
                b"print('DEFERRED_B_PRINT')\nfail('later B error')\nB='second'\n",
            ),
            ("c.bzl", b"C='third'\n"),
        ],
        903,
    )
    .build();
    let a = file_demand("a", PathObservationOperation::Lstat);
    let Fixture {
        mut tx,
        route,
        key,
        tracker,
        ..
    } = fixture_from(full.dupe(), &[a.dupe()]).await;
    pending(&tx.compute(&key).await.unwrap(), &[a.dupe()]);
    let mut captured = no_c_or_parent_batch(&tracker, &key);
    // Capture is not publication. Also capture B explicitly on the serial
    // control, preserving its real node and batch through the same retry owner.
    let b_key = observed_external_bzl_key(route, "", "b.bzl");
    let b_value = tx.compute(&b_key).await.unwrap();
    assert!(observed_external(&b_value).result().is_err());
    captured.extend(tracker.take());
    let marker_nodes = captured
        .iter()
        .filter(|row| {
            row.batch
                .as_ref()
                .is_some_and(|batch| event_texts(batch).contains(&"DEFERRED_B_PRINT"))
        })
        .map(|row| row.node)
        .collect::<Vec<_>>();
    assert!(!marker_nodes.is_empty());
    let failed_a = PathObservationEpoch::from_shared(full.observations().iter().map(|(d, r)| {
        (
            d.dupe(),
            if d == &a {
                Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing))
            } else {
                r.dupe()
            },
        )
    }))
    .unwrap();
    let mut tx = update(tx, &failed_a).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("observed missing A must terminate: {value:?}")
    };
    assert!(
        matches!(&observed.result().as_ref().as_ref().unwrap_err().inner, RepositoryPackageLoadErrorInner::Bzl {raw_load,..} if raw_load.as_ref() == ":a.bzl")
    );
    assert!(
        observed
            .observations()
            .get(&file_demand("b", PathObservationOperation::FileBytes))
            .is_none()
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, failed_a.get(d).unwrap()));
    }
    let rows = tracker.take();
    assert!(
        !rows
            .iter()
            .any(|row| row.key == "observed-external-bzl-module:@@dep+//:c.bzl")
    );
    let root = rows
        .iter()
        .rev()
        .find(|row| row.key == key.to_string())
        .unwrap()
        .node;
    let closure = tx.activation_closure([root]).await.unwrap();
    assert!(
        marker_nodes
            .iter()
            .all(|marker| !closure.nodes().iter().any(|node| node.node() == *marker))
    );

    let mut tx = update(tx, &full).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("successful A must expose warm B error: {value:?}")
    };
    assert!(
        matches!(&observed.result().as_ref().as_ref().unwrap_err().inner, RepositoryPackageLoadErrorInner::Bzl {raw_load,..} if raw_load.as_ref() == ":b.bzl")
    );
    assert!(
        observed
            .observations()
            .get(&file_demand("b", PathObservationOperation::FileBytes))
            .is_some()
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, full.get(d).unwrap()));
    }
    let rows = tracker.take();
    assert!(
        !rows
            .iter()
            .any(|row| row.key == "observed-external-bzl-module:@@dep+//:c.bzl")
    );
    let root = rows
        .iter()
        .rev()
        .find(|row| row.key == key.to_string())
        .unwrap()
        .node;
    let closure = tx.activation_closure([root]).await.unwrap();
    assert!(
        marker_nodes
            .iter()
            .any(|marker| closure.nodes().iter().any(|node| node.node() == *marker))
    );
    assert!(
        rows.iter()
            .any(|row| marker_nodes.contains(&row.node) && row.kind == ActivationKind::Reused)
    );
}

#[tokio::test]
async fn observed_inventory_pending_lstat_defers_later_cycle_and_fresh_retry_recovers() {
    let cycle = EpochBuilder::external_sources(
        &[
            ("BUILD.bazel", THREE_LOADS),
            ("a.bzl", b"A='first'\n"),
            ("b.bzl", b"load(':b.bzl','B')\nB='second'\n"),
            ("c.bzl", b"C='third'\n"),
        ],
        904,
    )
    .build();
    let a = file_demand("a", PathObservationOperation::Lstat);
    let Fixture {
        mut tx,
        key,
        tracker,
        ..
    } = fixture_from(cycle.dupe(), &[a.dupe()]).await;
    let value = tokio::time::timeout(Duration::from_secs(5), tx.compute(&key))
        .await
        .expect("deferred cyclic child must release")
        .unwrap();
    pending(&value, &[a]);
    no_c_or_parent_batch(&tracker, &key);
    let mut tx = cycle_retry(tx, &cycle, &tracker).await;
    let value = tokio::time::timeout(Duration::from_secs(5), tx.compute(&key))
        .await
        .expect("reachable cyclic child must release")
        .unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("complete A exposes the actual later cycle: {value:?}")
    };
    assert!(
        matches!(&observed.result().as_ref().as_ref().unwrap_err().inner,
        RepositoryPackageLoadErrorInner::Bzl {raw_load,error,..} if raw_load.as_ref() == ":b.bzl" && error.cycle().is_some())
    );
    assert!(
        !tracker
            .take()
            .iter()
            .any(|row| row.key == "observed-external-bzl-module:@@dep+//:c.bzl")
    );
    // Preserve unchanged canonical facts and change only the actual source
    // body. Equal metadata must not hide a repaired load graph.
    let b_bytes = file_demand("b", PathObservationOperation::FileBytes);
    let replacement = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(b"B='second'\n".as_slice())),
    ));
    let fixed = PathObservationEpoch::from_shared(cycle.observations().iter().map(|(d, r)| {
        (
            d.dupe(),
            if d == &b_bytes {
                replacement.dupe()
            } else {
                r.dupe()
            },
        )
    }))
    .unwrap();
    let mut tx = cycle_retry(tx, &fixed, &tracker).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("fresh detector and changed child must recover: {value:?}")
    };
    assert_eq!(
        observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .targets
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, fixed.get(d).unwrap()));
    }
}
async fn cycle_retry(
    tx: DiceTransaction,
    epoch: &PathObservationEpoch,
    tracker: &Arc<EventTracker>,
) -> DiceTransaction {
    let mut update = tx.into_updater();
    update.changed_to(path_observation_shards(epoch)).unwrap();
    update
        .changed_to([(PathObservationEpochKey, epoch.dupe())])
        .unwrap();
    let mut data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: Some(tracker.dupe()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    update.commit_with_data(data).await
}
