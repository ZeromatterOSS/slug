use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering as AtomicOrdering;

use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::UserComputationData;

use super::*;

fn demand(
    namespace: PathObservationNamespace,
    path: &str,
    operation: PathObservationOperation,
) -> PathObservationDemand {
    PathObservationDemand::new(
        namespace,
        NormalizedAbsolutePath::new(path).unwrap(),
        operation,
    )
}

fn bytes(value: &'static [u8]) -> Arc<PathObservationResult> {
    Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(value)),
    ))
}

fn epoch(
    rows: impl IntoIterator<Item = (PathObservationDemand, Arc<PathObservationResult>)>,
) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(rows).unwrap()
}

fn complete(value: PathOutcome<Arc<PathObservationResult>>) -> Arc<PathObservationResult> {
    match value {
        PathOutcome::Complete(value) => value,
        PathOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
    }
}

#[tokio::test]
async fn per_demand_full_result_namespace_a_b_a_preserves_old_live_transaction() {
    let host = demand(
        PathObservationNamespace::Host,
        "/workspace/file",
        PathObservationOperation::FileBytes,
    );
    let materialized = demand(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
        "/workspace/file",
        PathObservationOperation::FileBytes,
    );
    let metadata = demand(
        PathObservationNamespace::Host,
        "/workspace/file",
        PathObservationOperation::Lstat,
    );
    let a = bytes(b"A");
    let b = bytes(b"B");
    let stat = |mode| {
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, mode),
        )))
    };
    let stat_a = stat(0o600);
    let stat_b = stat(0o755);
    let known = [host.dupe(), materialized.dupe(), metadata.dupe()];
    let desired_a = epoch([
        (host.dupe(), a.dupe()),
        (materialized.dupe(), b.dupe()),
        (metadata.dupe(), stat_a.dupe()),
    ]);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_path_observation_inputs(&mut updater, &desired_a, known.clone()).unwrap();
    let mut old = updater.commit().await;
    assert_eq!(
        old.compute(&PathObservationInputModeKey).await.unwrap(),
        PathObservationInputMode::PerDemand
    );
    assert_eq!(
        complete(
            old.compute(&PathObservationKey::new(host.dupe()))
                .await
                .unwrap()
        ),
        a
    );
    let missing = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Missing,
    ));
    let desired_b = epoch([
        (host.dupe(), b.dupe()),
        (materialized.dupe(), missing.dupe()),
        (metadata.dupe(), stat_b.dupe()),
    ]);
    let mut updater = dice.updater();
    inject_path_observation_inputs(&mut updater, &desired_b, known.clone()).unwrap();
    let mut changed = updater.commit().await;
    assert_eq!(
        complete(
            changed
                .compute(&PathObservationKey::new(host.dupe()))
                .await
                .unwrap()
        ),
        b
    );
    assert_eq!(
        complete(
            changed
                .compute(&PathObservationKey::new(materialized.dupe()))
                .await
                .unwrap()
        ),
        missing
    );
    assert_eq!(
        complete(
            changed
                .compute(&PathObservationKey::new(metadata.dupe()))
                .await
                .unwrap()
        ),
        stat_b
    );
    assert_eq!(
        complete(
            old.compute(&PathObservationKey::new(metadata.dupe()))
                .await
                .unwrap()
        ),
        stat_a
    );
    assert_eq!(
        complete(
            old.compute(&PathObservationKey::new(materialized.dupe()))
                .await
                .unwrap()
        ),
        b
    );
    let mut updater = changed.into_updater();
    inject_path_observation_inputs(&mut updater, &desired_a, known).unwrap();
    let mut restored = updater.commit().await;
    assert_eq!(
        complete(
            restored
                .compute(&PathObservationKey::new(host))
                .await
                .unwrap()
        ),
        a
    );
    assert_eq!(
        complete(
            restored
                .compute(&PathObservationKey::new(metadata))
                .await
                .unwrap()
        ),
        stat_a
    );
    assert_eq!(
        complete(
            restored
                .compute(&PathObservationKey::new(materialized))
                .await
                .unwrap()
        ),
        b
    );
}

#[tokio::test]
async fn per_demand_revoked_and_unknown_inputs_never_resurrect_legacy_maps() {
    let known = demand(
        PathObservationNamespace::Host,
        "/workspace/known",
        PathObservationOperation::FileBytes,
    );
    let unknown = demand(
        PathObservationNamespace::Host,
        "/workspace/unknown",
        PathObservationOperation::FileBytes,
    );
    let stale = bytes(b"stale");
    let legacy = epoch([(known.dupe(), stale.dupe()), (unknown.dupe(), stale.dupe())]);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(path_observation_shards(&legacy))
        .unwrap();
    updater
        .changed_to([(PathObservationEpochKey, legacy)])
        .unwrap();
    // No mode key exists yet: prove migration invalidates an already cached
    // legacy PathObservationKey while the legacy maps remain unchanged.
    let mut legacy_live = updater.commit().await;
    assert_eq!(
        complete(
            legacy_live
                .compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        stale
    );
    let mut updater = dice.updater();
    inject_path_observation_inputs(
        &mut updater,
        &epoch([(known.dupe(), bytes(b"current"))]),
        [known.dupe()],
    )
    .unwrap();
    let mut old = updater.commit().await;
    assert_eq!(
        complete(
            legacy_live
                .compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        stale
    );
    assert_eq!(
        complete(
            old.compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        bytes(b"current")
    );
    assert!(matches!(
        old.compute(&PathObservationKey::new(unknown.dupe()))
            .await
            .unwrap(),
        PathOutcome::Need(_)
    ));
    let mut updater = dice.updater();
    inject_path_observation_inputs(&mut updater, &PathObservationEpoch::empty(), [known.dupe()])
        .unwrap();
    let mut revoked = updater.commit().await;
    assert!(
        revoked
            .compute(&PathObservationInputKey::new(known.dupe()))
            .await
            .unwrap()
            .is_none()
    );
    for d in [&known, &unknown] {
        let need = revoked
            .compute(&PathObservationKey::new(d.dupe()))
            .await
            .unwrap();
        assert!(matches!(need, PathOutcome::Need(_)));
        assert!(!PathObservationKey::validity(&need));
        assert!(!PathObservationKey::equality(&need, &need));
    }
    assert_eq!(
        complete(
            old.compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        bytes(b"current")
    );
    let mut updater = revoked.into_updater();
    updater
        .changed_to([(
            PathObservationInputModeKey,
            PathObservationInputMode::LegacyEpoch,
        )])
        .unwrap();
    updater
        .changed([PathObservationInputModeProjectionKey])
        .unwrap();
    let mut legacy = updater.commit().await;
    assert_eq!(
        complete(
            legacy
                .compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        stale
    );
    let mut updater = legacy.into_updater();
    inject_path_observation_inputs(&mut updater, &PathObservationEpoch::empty(), [known.dupe()])
        .unwrap();
    let mut per_demand = updater.commit().await;
    assert!(matches!(
        per_demand
            .compute(&PathObservationKey::new(known.dupe()))
            .await
            .unwrap(),
        PathOutcome::Need(_)
    ));

    let cold_value = bytes(b"cold-original");
    let mut updater = dice.updater();
    inject_path_observation_inputs(
        &mut updater,
        &epoch([(known.dupe(), cold_value.dupe())]),
        [known.dupe(), unknown.dupe()],
    )
    .unwrap();
    // Do not compute mode, projection or paths anywhere at this version: live
    // cache sharing with another transaction would hide lost historical values.
    let mut cold = updater.commit().await;
    let mut updater = dice.updater();
    updater
        .changed_to([(
            PathObservationInputModeKey,
            PathObservationInputMode::LegacyEpoch,
        )])
        .unwrap();
    updater
        .changed([PathObservationInputModeProjectionKey])
        .unwrap();
    let mut later_legacy = updater.commit().await;
    assert_eq!(
        later_legacy
            .compute(&PathObservationInputModeProjectionKey)
            .await
            .unwrap(),
        PathObservationInputMode::LegacyEpoch,
    );
    assert!(Arc::ptr_eq(
        &complete(
            later_legacy
                .compute(&PathObservationKey::new(known.dupe()))
                .await
                .unwrap()
        ),
        &stale,
    ));
    let mut updater = dice.updater();
    inject_path_observation_inputs(
        &mut updater,
        &PathObservationEpoch::empty(),
        [known.dupe(), unknown.dupe()],
    )
    .unwrap();
    let mut later_per_demand = updater.commit().await;
    assert_eq!(
        later_per_demand
            .compute(&PathObservationInputModeProjectionKey)
            .await
            .unwrap(),
        PathObservationInputMode::PerDemand,
    );
    assert!(matches!(
        later_per_demand
            .compute(&PathObservationKey::new(known.dupe()))
            .await
            .unwrap(),
        PathOutcome::Need(_),
    ));

    assert_eq!(
        cold.compute(&PathObservationInputModeKey).await.unwrap(),
        PathObservationInputMode::PerDemand
    );
    assert_eq!(
        cold.compute(&PathObservationInputModeProjectionKey)
            .await
            .unwrap(),
        PathObservationInputMode::PerDemand
    );
    let original_cell = cold
        .compute(&PathObservationInputKey::new(known.dupe()))
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&original_cell, &cold_value));
    assert!(Arc::ptr_eq(
        &complete(cold.compute(&PathObservationKey::new(known)).await.unwrap()),
        &cold_value,
    ));
    assert!(
        cold.compute(&PathObservationInputKey::new(unknown.dupe()))
            .await
            .unwrap()
            .is_none()
    );
    let historical_need = cold
        .compute(&PathObservationKey::new(unknown.dupe()))
        .await
        .unwrap();
    assert!(matches!(historical_need, PathOutcome::Need(ref need) if need.demands() == &[unknown]));
    assert!(!PathObservationKey::validity(&historical_need));
    assert!(!PathObservationKey::equality(
        &historical_need,
        &historical_need
    ));
}

#[derive(Default)]
struct Tracker(AtomicUsize);
impl ActivationTracker for Tracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _dependencies: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        if key.downcast_ref::<PathObservationKey>().is_some()
            && matches!(activation, ActivationData::Evaluated(_))
        {
            self.0.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }
}

#[tokio::test]
async fn per_demand_unrelated_addition_keeps_exact_path_cached() {
    let a = demand(
        PathObservationNamespace::Host,
        "/workspace/a",
        PathObservationOperation::FileBytes,
    );
    let b = demand(
        PathObservationNamespace::Host,
        "/workspace/b",
        PathObservationOperation::FileBytes,
    );
    let value = bytes(b"unchanged");
    let tracker = Arc::new(Tracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    });
    inject_path_observation_inputs(&mut updater, &epoch([(a.dupe(), value.dupe())]), [a.dupe()])
        .unwrap();
    let mut tx = updater.commit().await;
    assert_eq!(
        complete(
            tx.compute(&PathObservationKey::new(a.dupe()))
                .await
                .unwrap()
        ),
        value
    );
    assert_eq!(tracker.0.load(AtomicOrdering::SeqCst), 1);
    let mut updater = tx.into_updater();
    inject_path_observation_inputs(
        &mut updater,
        &epoch([(a.dupe(), value.dupe()), (b.dupe(), bytes(b"added"))]),
        [a.dupe(), b.dupe()],
    )
    .unwrap();
    let mut tx = updater.commit().await;
    assert_eq!(
        complete(tx.compute(&PathObservationKey::new(a)).await.unwrap()),
        value
    );
    assert_eq!(tracker.0.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(
        complete(tx.compute(&PathObservationKey::new(b)).await.unwrap()),
        bytes(b"added")
    );
    assert_eq!(tracker.0.load(AtomicOrdering::SeqCst), 2);
}

async fn retained_first_result(per_demand: bool, additions: usize) -> usize {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = bytes(b"first");
    let mut rows = Vec::new();
    for i in 0..additions {
        let d = demand(
            PathObservationNamespace::Host,
            &format!("/workspace/{i}"),
            PathObservationOperation::FileBytes,
        );
        rows.push((
            d,
            if i == 0 {
                first.dupe()
            } else {
                bytes(b"later")
            },
        ));
        let desired = epoch(rows.iter().cloned());
        let mut updater = dice.updater();
        if per_demand {
            inject_path_observation_inputs(
                &mut updater,
                &desired,
                rows.iter().map(|(d, _)| d.dupe()),
            )
            .unwrap();
        } else {
            updater
                .changed_to([(PathObservationEpochKey, desired.dupe())])
                .unwrap();
        }
        drop(updater.commit().await);
        drop(desired);
        dice.wait_for_idle().await;
    }
    drop(rows);
    dice.wait_for_idle().await;
    let retained = Arc::strong_count(&first);
    println!(
        "path-input-history mode={} additions={additions} first_result_refs_before_engine_drop={retained}",
        if per_demand {
            "per-demand"
        } else {
            "aggregate"
        }
    );
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while Arc::strong_count(&first) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual injected history must retire with the last engine owner");
    println!(
        "path-input-history mode={} first_result_refs_after_engine_drop={}",
        if per_demand {
            "per-demand"
        } else {
            "aggregate"
        },
        Arc::strong_count(&first)
    );
    retained
}

#[tokio::test]
async fn per_demand_history_retains_constant_result_references_against_aggregate_control() {
    let additions = 256;
    let aggregate = retained_first_result(false, additions).await;
    let per_demand = retained_first_result(true, additions).await;
    assert_eq!(
        aggregate,
        additions + 1,
        "each changed cumulative epoch retains the first result again"
    );
    assert_eq!(
        per_demand, 2,
        "one exact input cell plus this test's owner, independent of retry count"
    );
}

/// Run only under the root's external time/RSS supervisor, one mode per process.
/// This scales retained descriptor rows, not file payload sizes.
#[tokio::test]
#[ignore = "supervised, bounded aggregate/per-demand RSS comparison"]
async fn supervised_path_input_history_rss_probe() {
    let mode = std::env::var("SLUG_MEMORY_PROBE_MODE")
        .expect("set aggregate or per-demand mode explicitly");
    let per_demand = match mode.as_str() {
        "aggregate" => false,
        "per-demand" => true,
        _ => panic!("SLUG_MEMORY_PROBE_MODE must be aggregate or per-demand"),
    };
    let additions = std::env::var("SLUG_MEMORY_PROBE_ADDITIONS")
        .map(|value| {
            value
                .parse::<usize>()
                .expect("addition count must be an integer")
        })
        .unwrap_or(2048);
    assert!(
        (1..=2048).contains(&additions),
        "probe addition count must be 1..=2048"
    );
    let retained = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        retained_first_result(per_demand, additions),
    )
    .await
    .expect("bounded RSS probe must finish within 30 seconds");
    assert_eq!(retained, if per_demand { 2 } else { additions + 1 });
}

// Deliberately re-read equal bytes. Reusing a fixture Arc would conceal the
// Some -> None -> freshly observed Some allocation regression.
struct PayloadHistoryFile(std::path::PathBuf);
impl PayloadHistoryFile {
    fn new(size: usize) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "slug-path-payload-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, &vec![0x81; size]).unwrap();
        Self(path)
    }
    fn read(&self) -> (Arc<PathObservationResult>, std::sync::Weak<[u8]>) {
        let payload: Arc<[u8]> = std::fs::read(&self.0).unwrap().into();
        let weak = Arc::downgrade(&payload);
        (
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Present(payload),
            )),
            weak,
        )
    }
}
impl Drop for PayloadHistoryFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn payload_ref(value: &PathObservationResult) -> &Arc<[u8]> {
    match value {
        PathObservationResult::FileBytes(PathOperationResult::Present(payload)) => payload,
        other => panic!("expected regular file bytes: {other:?}"),
    }
}

async fn retained_equal_payload_history(
    cycles: usize,
    size: usize,
    expect_reuse: bool,
    semantic_controls: bool,
) {
    let file = PayloadHistoryFile::new(size);
    assert!(
        std::fs::symlink_metadata(&file.0)
            .unwrap()
            .file_type()
            .is_file()
    );
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(&file.0).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let input_key = PathObservationInputKey::new(demand.dupe());
    let path_key = PathObservationKey::new(demand.dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut submissions = Vec::new();
    let mut payloads = Vec::new();
    let mut versions = Vec::new();
    let mut canonical = None;
    let mut first_nonshared_cycle = None;
    for cycle in 0..cycles {
        let (submitted, payload) = file.read();
        assert_eq!(payload_ref(&submitted).len(), size);
        assert!(payload_ref(&submitted).iter().all(|b| *b == 0x81));
        submissions.push(Arc::downgrade(&submitted));
        // Weak<[u8]> pins the entire inline Arc allocation after strong Drop.
        // Keep those handles only for the small semantic liveness test. The
        // RSS probe tracks small result wrappers so it cannot retain byte storage.
        if semantic_controls {
            payloads.push(payload);
        }
        let desired = epoch([(demand.dupe(), submitted)]);
        let mut updater = dice.updater();
        inject_path_observation_inputs(&mut updater, &desired, [demand.dupe()]).unwrap();
        let mut present = updater.commit().await;
        drop(desired);
        let raw = present.compute(&input_key).await.unwrap().unwrap();
        let observed = complete(present.compute(&path_key).await.unwrap());
        assert_eq!(observed, raw);
        assert_eq!(payload_ref(&raw).len(), size);
        if let Some(first) = &canonical {
            assert_eq!(&raw, first, "all A observations are semantically equal");
            if !Arc::ptr_eq(first, &raw) || !Arc::ptr_eq(payload_ref(first), payload_ref(&raw)) {
                first_nonshared_cycle.get_or_insert(cycle);
            }
        } else {
            canonical = Some(raw.dupe());
        }
        versions.push((present, true));
        let mut updater = dice.updater();
        inject_path_observation_inputs(
            &mut updater,
            &PathObservationEpoch::empty(),
            [demand.dupe()],
        )
        .unwrap();
        let mut revoked = updater.commit().await;
        assert!(revoked.compute(&input_key).await.unwrap().is_none());
        assert!(matches!(
            revoked.compute(&path_key).await.unwrap(),
            PathOutcome::Need(_)
        ));
        versions.push((revoked, false));
    }
    for (old, is_present) in &mut versions {
        let raw = old.compute(&input_key).await.unwrap();
        if *is_present {
            assert_eq!(raw.as_ref().unwrap(), canonical.as_ref().unwrap());
            assert_eq!(
                complete(old.compute(&path_key).await.unwrap()),
                raw.unwrap()
            );
        } else {
            assert!(raw.is_none(), "old revoked version cannot resurrect A");
            assert!(matches!(
                old.compute(&path_key).await.unwrap(),
                PathOutcome::Need(_)
            ));
        }
    }
    let live_results = submissions.iter().filter(|v| v.strong_count() != 0).count();
    let live_payloads = if semantic_controls {
        payloads.iter().filter(|v| v.strong_count() != 0).count()
    } else {
        // Each submitted wrapper owns one freshly read, distinct byte Arc.
        live_results
    };
    println!(
        "equal-path-payload-history cycles={cycles} bytes={size} live_results={live_results} live_payloads={live_payloads} engine_alive=true"
    );
    if semantic_controls {
        // A real content change must not reuse A. Returning A after B has no
        // equal penultimate A slot: demand full semantic restoration, not a
        // general-purpose value interner.
        for byte in [0x42, 0x81] {
            std::fs::write(&file.0, vec![byte; size]).unwrap();
            let (submitted, payload) = file.read();
            submissions.push(Arc::downgrade(&submitted));
            payloads.push(payload);
            let desired = epoch([(demand.dupe(), submitted)]);
            let mut updater = dice.updater();
            inject_path_observation_inputs(&mut updater, &desired, [demand.dupe()]).unwrap();
            let mut tx = updater.commit().await;
            drop(desired);
            let raw = tx.compute(&input_key).await.unwrap().unwrap();
            assert!(payload_ref(&raw).iter().all(|b| *b == byte));
            if byte == 0x42 {
                assert_ne!(&raw, canonical.as_ref().unwrap());
                assert!(!Arc::ptr_eq(&raw, canonical.as_ref().unwrap()));
            } else {
                assert_eq!(&raw, canonical.as_ref().unwrap());
            }
            assert_eq!(complete(tx.compute(&path_key).await.unwrap()), raw);
            versions.push((tx, true));
        }
        // Re-check an actual old A and old None after B -> A; version ranges
        // remain independent even when multiple versions share one allocation.
        assert_eq!(
            versions[0].0.compute(&input_key).await.unwrap().as_ref(),
            canonical.as_ref()
        );
        assert!(versions[1].0.compute(&input_key).await.unwrap().is_none());
        assert!(matches!(
            versions[1].0.compute(&path_key).await.unwrap(),
            PathOutcome::Need(_)
        ));
    }
    drop(canonical);
    drop(versions);
    dice.wait_for_idle().await;
    let history_results = submissions[..cycles]
        .iter()
        .filter(|v| v.strong_count() != 0)
        .count();
    println!(
        "equal-path-payload-history live_transactions=false history_result_owners={history_results} engine_alive=true"
    );
    assert!(
        submissions[0].strong_count() > 0,
        "engine history still owns original A after live transactions drop"
    );
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while submissions.iter().any(|v| v.strong_count() != 0)
            || payloads.iter().any(|v| v.strong_count() != 0)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all result and payload allocations must retire after actual last engine Drop");
    println!("equal-path-payload-history live_results=0 live_payloads=0 engine_alive=false");
    // Keep the expected baseline failure after the semantic and retirement
    // controls, so a failed sharing gate still discriminates actual ownership.
    if expect_reuse {
        assert_eq!(
            first_nonshared_cycle, None,
            "equal penultimate result/payload allocation must be reused"
        );
    }
    assert_eq!(live_results, if expect_reuse { 1 } else { cycles });
    assert_eq!(live_payloads, if expect_reuse { 1 } else { cycles });
    assert_eq!(history_results, if expect_reuse { 1 } else { cycles });
}

#[tokio::test]
async fn per_demand_equal_reread_reuses_penultimate_payload_with_revocation_and_old_versions() {
    retained_equal_payload_history(8, 64 * 1024, true, true).await;
}

/// One small payload-history case per externally supervised process. This is
/// distinct from the cumulative descriptor-row probe above.
#[tokio::test]
#[ignore = "supervised baseline/candidate equal-payload RSS comparison"]
async fn supervised_equal_path_payload_history_rss_probe() {
    let expect_reuse = match std::env::var("SLUG_MEMORY_PAYLOAD_EXPECT_REUSE").as_deref() {
        Ok("false") => false,
        Ok("true") | Err(_) => true,
        _ => panic!("SLUG_MEMORY_PAYLOAD_EXPECT_REUSE must be true or false"),
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        retained_equal_payload_history(32, 1024 * 1024, expect_reuse, false),
    )
    .await
    .expect("bounded equal-payload RSS probe must finish within 30 seconds");
}
