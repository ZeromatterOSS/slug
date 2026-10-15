// Genuine module singleton ownership across operational revisions.
use super::*;

// Reuse the genuine selected-registry fixture so the tracked builtin mapping
// owner is available independently of the main repository's apparent aliases.
const COHORT_MODULE: &str = "module(name='consumer')\n";
fn cohort_dice() -> Arc<Dice> {
    crate::canonical_repository_route_tests::tests::builtin_graph_dice()
}

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
    let dice = cohort_dice();
    let source_a = child_source(1);
    let source_b = child_source(2);
    let _initial_inputs =
        case_transaction_with_other(&dice, COHORT_MODULE, PARENT, &source_a, OTHER, true, None)
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

    let _changed_inputs =
        case_transaction_with_other(&dice, COHORT_MODULE, PARENT, &source_b, OTHER, true, None)
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
        COHORT_MODULE,
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

async fn load_ordinary_mode(
    tx: &mut DiceTransaction,
    observed: bool,
    bzlmod: bool,
) -> FrozenBzlModule {
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let label = observed_test_label("ext.bzl");
    if observed {
        let key = if bzlmod {
            HostBzlModuleObservationKey::new_bzlmod(workspace, label)
        } else {
            HostBzlModuleObservationKey::new(workspace, label)
        };
        let outcome = tx.compute(&key).await.unwrap();
        observed_module(&outcome).result().as_ref().unwrap().clone()
    } else {
        let key = if bzlmod {
            HostBzlModuleEvalKey::new_bzlmod(workspace, label)
        } else {
            HostBzlModuleEvalKey::new(workspace, label)
        };
        let SourcePreparationOutcome::Complete(value) = tx.compute(&key).await.unwrap() else {
            panic!("complete ordinary source inputs");
        };
        value.as_ref().as_ref().unwrap().clone()
    }
}

#[tokio::test]
async fn ordinary_bzlmod_children_receive_original_exports_with_distinct_native_mode_and_shared_producers()
 {
    const PARENT: &str = "load('//:child.bzl', 'APPLE', 'VERSION', 'MODE')\nOWN = apple_common\n";
    const CHILD: &str = "APPLE = apple_common\nTOOLCHAIN = apple_common.apple_toolchain()\nVERSION = str(apple_common.dotted_version('1.2.3'))\nMODE = hasattr(native, 'bazel_version')\n";
    for observed in [false, true] {
        let dice = cohort_dice();
        let _inputs = case_transaction(&dice, COHORT_MODULE, PARENT, CHILD, true, None).await;
        let mut tx = observing_transaction(&dice).await;
        let build = load_ordinary_mode(&mut tx, observed, false).await;
        let bzlmod = load_ordinary_mode(&mut tx, observed, true).await;
        let consumer = Module::new();
        for (module, mode) in [(&build, false), (&bzlmod, true)] {
            assert_eq!(
                original(module, "MODE", &consumer).unpack_bool(),
                Some(mode)
            );
            assert_eq!(
                original(module, "VERSION", &consumer).unpack_str(),
                Some("1.2.3")
            );
            assert!(
                original(module, "APPLE", &consumer).ptr_eq(original(module, "OWN", &consumer)),
                "loaded child and parent retain the same original export"
            );
            let context = module
                .module
                .client_data()
                .and_then(|data| {
                    crate::starlark_label::defining_module_source_context(data.to_value())
                })
                .unwrap();
            assert_eq!(context.environment_bindings().len(), 1);
            assert_eq!(context.environment_bindings()[0].global, "apple_common");
            assert_eq!(context.raw_load_manifest().reachable.len(), 2);
            assert_eq!(
                context.effective_reachable().len(),
                8,
                "raw parent/child plus complete six-source builtin closure"
            );
        }
        assert!(original(&build, "OWN", &consumer).ptr_eq(original(&bzlmod, "OWN", &consumer)));
        let owner = |module: &FrozenBzlModule| {
            module
                .module
                .client_data()
                .and_then(|data| {
                    crate::starlark_label::defining_module_function_owner(data.to_value())
                })
                .unwrap()
                .clone()
        };
        assert_ne!(owner(&build), owner(&bzlmod));
        let restored = load_ordinary_mode(&mut tx, observed, true).await;
        assert_eq!(owner(&bzlmod), owner(&restored));
        assert!(original(&bzlmod, "OWN", &consumer).ptr_eq(original(&restored, "OWN", &consumer)));
    }
}

#[tokio::test]
async fn original_builtin_mapping_is_canonical_across_main_apparent_alias_a_b_a() {
    const BASE: &str = "module(name='consumer')\nbazel_dep(name='bazel_tools', repo_name='private_tools')\nbazel_dep(name='platforms', version='1.0.0', repo_name='other_tools')\n";
    const SOURCE: &str =
        "APPLE = apple_common\nVERSION = str(apple_common.dotted_version('3.4.0'))\n";
    let redirected = BASE.replace("repo_name='other_tools'", "repo_name='bazel_tools'");
    for observed in [false, true] {
        let dice = cohort_dice();
        let mut originals = Vec::new();
        for module_file in [BASE, redirected.as_str(), BASE] {
            let mut tx =
                case_transaction(&dice, module_file, SOURCE, "marker = 0\n", true, None).await;
            for bzlmod in [false, true] {
                let value = load_ordinary_mode(&mut tx, observed, bzlmod).await;
                let context = value
                    .module
                    .client_data()
                    .and_then(|data| {
                        crate::starlark_label::defining_module_source_context(data.to_value())
                    })
                    .unwrap();
                let alias = context
                    .raw_load_manifest()
                    .root
                    .repository_mapping
                    .iter()
                    .find(|(apparent, _)| apparent.as_str() == "bazel_tools");
                if bzlmod {
                    assert!(
                        context
                            .raw_load_manifest()
                            .root
                            .repository_mapping
                            .is_empty()
                    );
                } else if module_file == redirected {
                    // The selected graph reserves the canonical `platforms` spelling.
                    assert_eq!(alias.unwrap().1.as_str(), "platforms");
                } else {
                    assert!(
                        alias.is_none(),
                        "tools is available only by its private alias"
                    );
                }
                let consumer = Module::new();
                assert_eq!(
                    original(&value, "VERSION", &consumer).unpack_str(),
                    Some("3.4.0")
                );
                originals.push(value);
            }
        }
        let consumer = Module::new();
        for value in &originals[1..] {
            assert!(
                original(&originals[0], "APPLE", &consumer)
                    .ptr_eq(original(value, "APPLE", &consumer)),
                "main aliases and native consumer modes cannot select a different builtin producer"
            );
        }
        let owner = |module: &FrozenBzlModule| {
            module
                .module
                .client_data()
                .and_then(|data| {
                    crate::starlark_label::defining_module_function_owner(data.to_value())
                })
                .unwrap()
                .clone()
        };
        assert_ne!(
            owner(&originals[0]),
            owner(&originals[2]),
            "consumer mapping remains semantic input"
        );
        assert_eq!(owner(&originals[0]), owner(&originals[4]));
        assert_eq!(owner(&originals[1]), owner(&originals[5]));
    }
}
