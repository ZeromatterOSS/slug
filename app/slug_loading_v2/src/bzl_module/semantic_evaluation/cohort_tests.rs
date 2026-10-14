// Genuine module singleton ownership across operational revisions.
use super::*;

const PARENT: &str =
    "load('//:other.bzl', 'payload')\nload('//:child.bzl', 'callback', 'separate', 'marker')\n";
const OTHER: &str = "load('//:child.bzl', 'empty')\npayload = empty\n";

fn child_source(marker: i32) -> String {
    format!(
        "Box = provider(fields=['value'])\nmarker = {marker}\nempty = Box(value=marker)\nseparate = Box(value=marker)\ndef callback():\n    return empty\n"
    )
}

fn original<'v>(
    module: &FrozenBzlModule,
    name: &str,
    consumer: &'v Module,
) -> starlark::values::Value<'v> {
    module
        .module
        .get(name)
        .unwrap()
        .owned_value(consumer.frozen_heap())
}

fn assert_coherent(module: &FrozenBzlModule, marker: i32) {
    let consumer = Module::new();
    let payload = original(module, "payload", &consumer);
    let callback = original(module, "callback", &consumer);
    let returned = Evaluator::new(&consumer)
        .eval_function(callback, &[], &[])
        .unwrap();
    assert!(
        payload.ptr_eq(returned),
        "payload and callback must share the actual module singleton"
    );
    assert!(
        !payload.ptr_eq(original(module, "separate", &consumer)),
        "separately constructed equal providers must remain distinct"
    );
    assert_eq!(
        original(module, "marker", &consumer).unpack_i32(),
        Some(marker)
    );
}

fn assert_same_original(left: &FrozenBzlModule, right: &FrozenBzlModule) {
    let consumer = Module::new();
    assert!(
        original(left, "payload", &consumer).ptr_eq(original(right, "payload", &consumer)),
        "unchanged semantic inputs must reuse the original provider"
    );
    assert!(
        original(left, "callback", &consumer).ptr_eq(original(right, "callback", &consumer)),
        "unchanged semantic inputs must reuse the original function"
    );
}

async fn observing_transaction(dice: &Arc<Dice>) -> DiceTransaction {
    let data = host_bzl_user_data(crate::cycle_detector::bzl_load_cycle_detector(), None);
    dice.updater_with_data(data).commit().await
}

#[tokio::test]
async fn original_module_cohort_survives_observation_refresh_and_source_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let source_a = child_source(1);
    let source_b = child_source(2);
    let _initial_inputs = case_transaction_with_other(
        &dice,
        "module(name='bazel_tools')\n",
        PARENT,
        &source_a,
        OTHER,
        true,
        None,
    )
    .await;
    let mut initial = observing_transaction(&dice).await;
    let initial_outcome = compute_observed_module(&mut initial, "ext.bzl").await;
    let first = observed_module(&initial_outcome).result().as_ref().unwrap();
    assert_coherent(first, 1);
    let epoch = initial.compute(&PathObservationEpochKey).await.unwrap();
    let changed_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(format!("{WORKSPACE}/child.bzl")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let metadata = Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
        PathLstat::new(PathNodeKind::RegularFile, 900, 1, 1, 1, 0o644),
    )));
    let refreshed_epoch =
        PathObservationEpoch::from_shared(epoch.observations().iter().map(|(demand, result)| {
            (
                demand.clone(),
                if demand == &changed_demand {
                    metadata.clone()
                } else {
                    result.clone()
                },
            )
        }))
        .unwrap();
    let _refreshed_inputs = transaction_with_epoch(
        &dice,
        refreshed_epoch,
        Arc::new(BzlEventTracker::default()),
        false,
    )
    .await;
    let mut refreshed = observing_transaction(&dice).await;
    let refreshed_outcome = compute_observed_module(&mut refreshed, "ext.bzl").await;
    let refreshed_carrier = observed_module(&refreshed_outcome);
    assert!(Arc::ptr_eq(
        refreshed_carrier
            .observations()
            .observations()
            .get(&changed_demand)
            .unwrap(),
        &metadata
    ));
    let refreshed_module = refreshed_carrier.result().as_ref().unwrap();
    assert_coherent(refreshed_module, 1);
    assert_same_original(first, refreshed_module);

    let _changed_inputs = case_transaction_with_other(
        &dice,
        "module(name='bazel_tools')\n",
        PARENT,
        &source_b,
        OTHER,
        true,
        None,
    )
    .await;
    let mut changed = observing_transaction(&dice).await;
    let changed_outcome = compute_observed_module(&mut changed, "ext.bzl").await;
    let changed_module = observed_module(&changed_outcome).result().as_ref().unwrap();
    assert_coherent(changed_module, 2);
    assert!(!HostBzlModuleObservationKey::equality(
        &initial_outcome,
        &changed_outcome
    ));
    let consumer = Module::new();
    assert!(!original(first, "payload", &consumer).ptr_eq(original(
        changed_module,
        "payload",
        &consumer
    )));

    // Source bytes come from separately allocated equal buffers on restoration.
    let source_a_again = child_source(1);
    let _restored_inputs = case_transaction_with_other(
        &dice,
        "module(name='bazel_tools')\n",
        PARENT,
        &source_a_again,
        OTHER,
        true,
        None,
    )
    .await;
    let mut restored = observing_transaction(&dice).await;
    let restored_outcome = compute_observed_module(&mut restored, "ext.bzl").await;
    let restored_module = observed_module(&restored_outcome)
        .result()
        .as_ref()
        .unwrap();
    assert_coherent(restored_module, 1);
    assert_same_original(first, restored_module);
    assert!(HostBzlModuleObservationKey::equality(
        &initial_outcome,
        &restored_outcome
    ));
}

mod lifecycle_tests {
    include!("lifecycle_tests.rs");
}
