use super::*;

async fn presentation_transaction(
    dice: &Arc<Dice>,
    tracker: &Arc<BzlEventTracker>,
    capture: bool,
) -> DiceTransaction {
    // Build userdata explicitly: the existing shared fixture helper always
    // selects capture, and changing it would alter unrelated lifecycle tests.
    let mut data = UserComputationData {
        cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
        activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    if capture {
        data.data.set(CaptureEvaluationEvents);
    }
    dice.updater_with_data(data).commit().await
}

async fn lane_module(transaction: &mut DiceTransaction, observed: bool) -> FrozenBzlModule {
    if observed {
        let outcome = compute_observed_module(transaction, "ext.bzl").await;
        observed_module(&outcome).result().as_ref().unwrap().clone()
    } else {
        let outcome = transaction
            .compute(&HostBzlModuleEvalKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                observed_test_label("ext.bzl"),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("legacy module unexpectedly requires preparation");
        };
        value.as_ref().as_ref().unwrap().clone()
    }
}

fn assert_activation(tracker: &BzlEventTracker, capture: bool, kind: ActivationKind) {
    let activations = tracker.take();
    assert_eq!(activations.len(), 1);
    let activation = &activations[0];
    assert_eq!(activation.kind, kind);
    if capture && kind == ActivationKind::Evaluated {
        assert!(matches!(activation.batch.as_ref().map(EventBatch::events),
            Some([slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }])
                if text == "one actual evaluation"));
    } else {
        assert!(activation.batch.is_none());
    }
}

#[tokio::test]
async fn capture_transitions_share_semantic_evaluation_and_outer_reuse_stays_eventless() {
    let source =
        "print('one actual evaluation')\nBox=provider(fields=['value'])\nEMPTY=Box(value=1)\n";
    for first_capture in [false, true] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let seed = case_transaction_with_other(
            &dice,
            "module(name='bazel_tools')\n",
            source,
            "",
            "",
            true,
            None,
        )
        .await;
        drop(seed);
        let tracker = Arc::new(BzlEventTracker::default());
        // The two outer families are distinct attempts, while their complete
        // semantic inputs match. This exercises an actual inner cache hit.
        let mut first_tx = presentation_transaction(&dice, &tracker, first_capture).await;
        let first = lane_module(&mut first_tx, first_capture).await;
        assert_activation(&tracker, first_capture, ActivationKind::Evaluated);

        let mut second_tx = presentation_transaction(&dice, &tracker, !first_capture).await;
        let second = lane_module(&mut second_tx, !first_capture).await;
        assert_activation(&tracker, !first_capture, ActivationKind::Evaluated);
        let consumer = Module::new();
        assert!(original(&first, "EMPTY", &consumer).ptr_eq(original(&second, "EMPTY", &consumer)));

        // A reused outer attempt never executes store_evaluation_data. DICE
        // reports Reused + None; it does not fabricate another print batch.
        let mut warm_tx = presentation_transaction(&dice, &tracker, !first_capture).await;
        let warm = lane_module(&mut warm_tx, !first_capture).await;
        assert_activation(&tracker, !first_capture, ActivationKind::Reused);
        assert!(original(&second, "EMPTY", &consumer).ptr_eq(original(&warm, "EMPTY", &consumer)));
    }
}

#[test]
fn closed_engine_releases_semantic_keys_while_retained_originals_remain_callable() {
    // A private runtime gives a deterministic close boundary for DICE's state
    // actor. No new heap/drop instrumentation or production weak API is needed.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (retained, inputs, engine) = runtime.block_on(async {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction = case_transaction_with_other(
            &dice,
            "module(name='bazel_tools')\n",
            PARENT,
            &child_source(1),
            OTHER,
            true,
            None,
        )
        .await;
        let outcome = compute_observed_module(&mut transaction, "ext.bzl").await;
        let retained = observed_module(&outcome).result().as_ref().unwrap().clone();
        let inputs = Arc::downgrade(retained.semantic_inputs.as_ref().unwrap());
        let engine = Arc::downgrade(&dice);
        drop(outcome);
        drop(transaction);
        dice.wait_for_idle().await;
        drop(dice);
        (retained, inputs, engine)
    });
    drop(runtime);
    assert!(engine.upgrade().is_none());
    assert!(inputs.upgrade().is_some());
    assert_coherent(&retained, 1);
    drop(retained);
    assert!(
        inputs.upgrade().is_none(),
        "no key/result ownership cycle after engine close"
    );
}
