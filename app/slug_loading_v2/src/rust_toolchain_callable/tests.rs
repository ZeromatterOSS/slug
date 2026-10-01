//! Synthetic helper proves wrapper binding, guard order and code lifetime only.
//! Full source/Target/action behavior is exercised by Analysis integration.
use dupe::Dupe;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::RustToolchainCallableModel;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::AllocList;
use starlark_map::small_set::SmallSet;

use super::*;
use crate::bzl_module::rust_toolchain_helper::synthetic_test_binding;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule::SubruleIdentity;
use crate::subrule_invocation::AnalysisActionCallScope;
use crate::subrule_invocation::AnalysisActions;
use crate::subrule_invocation::AnalysisEvaluationContext;
use crate::subrule_invocation::PreparedSubruleInvocation;

const HELPER: &str = "_private = ['kept']\ndef make_libstd_and_allocator_ccinfo(*, cc_toolchain, feature_configuration, experimental_link_std_dylib, rust_std, label, actions, allocator_library, std):\n    if allocator_library == 'fail': fail('synthetic helper reached')\n    return [label, std, experimental_link_std_dylib, _private[0]]\n";

fn execute(eval: &mut Evaluator<'_, '_, '_>, source: &str) -> anyhow::Result<()> {
    eval.eval_module(
        AstModule::parse("ordinary.bzl", source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}

fn model(
    binding: &PreparedRustToolchainHelperBinding,
    original: Value<'_>,
) -> RustToolchainCallable {
    let identity = original
        .request_value::<&dyn StarlarkFunctionIdentity>()
        .unwrap();
    RustToolchainCallable::new(RustToolchainCallableModel {
        source: binding.authority().dupe(),
        identity: identity.issue_identity(),
        name_hash: identity.name_hash().get(),
        cc_toolchain: AnalysisValue::none(),
        feature_configuration: AnalysisValue::none(),
        experimental_link_std_dylib: AnalysisValue::boolean(false),
        rust_std: AnalysisValue::configured_target(ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:stdlib").unwrap(),
                Arc::<[u8]>::from([]),
            ),
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
        )),
    })
    .unwrap()
}

fn install(module: &Module) -> RustToolchainCallable {
    let source = Module::new();
    execute(
        &mut Evaluator::new(&source),
        "def original(label, actions, allocator_library, std): pass\n",
    )
    .unwrap();
    let original_module = source.freeze().unwrap();
    let original = original_module.get("original").unwrap();
    let binding = synthetic_test_binding(HELPER);
    let model = model(&binding, original.value());
    module.set("original", original.owned_value(module.frozen_heap()));
    let callable = alloc_frozen_rust_toolchain_callable(
        module.frozen_heap(),
        model.clone(),
        [
            FrozenValue::new_none(),
            FrozenValue::new_none(),
            module.frozen_heap().alloc(false),
            FrozenValue::new_none(),
        ],
        &binding,
    )
    .unwrap();
    module.set("f", callable.to_value());
    let mut wrong = model.model().source.as_ref().clone();
    wrong.helper.fingerprint[0] ^= 1;
    let wrong = RustToolchainCallable::new(RustToolchainCallableModel {
        source: Arc::new(wrong),
        identity: Arc::new(()),
        name_hash: 0,
        cc_toolchain: AnalysisValue::none(),
        feature_configuration: AnalysisValue::none(),
        experimental_link_std_dylib: AnalysisValue::boolean(false),
        rust_std: model.model().rust_std.clone(),
    })
    .unwrap();
    assert!(
        alloc_frozen_rust_toolchain_callable(
            module.frozen_heap(),
            wrong,
            [FrozenValue::new_none(); 4],
            &binding
        )
        .unwrap_err()
        .contains("authority mismatch")
    );
    model
}

fn evaluation_context(direct: Arc<[Arc<SubruleIdentity>]>) -> AnalysisEvaluationContext {
    let prepared = direct
        .iter()
        .map(|identity| {
            PreparedSubruleInvocation::new(
                identity.clone(),
                Arc::<[(compact_str::CompactString, FrozenValue)]>::from([]),
                Arc::new(SmallSet::new()),
            )
        })
        .collect::<Vec<_>>();
    AnalysisEvaluationContext::new(
        direct,
        prepared,
        CanonicalLabel::parse("@@//:consumer").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([]),
    )
}

fn actions<'v>(module: &'v Module, context: &AnalysisEvaluationContext) -> Value<'v> {
    module.heap().alloc_simple(AnalysisActions::new(
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        context.root_token(),
        "rule context",
        AnalysisActionCallScope::Root,
    ))
}

fn subrules() -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@//:subrule.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    execute(&mut eval, "def _active(ctx, f):\n    return f('subrule-label', ctx.actions, None, 'std')\nactive = subrule(implementation=_active)\ndef _suspended(ctx, f, root_actions):\n    return f('subrule-label', root_actions, 'fail', 'std')\nsuspended = subrule(implementation=_suspended)\n").unwrap();
    drop(eval);
    module.freeze().unwrap()
}

#[test]
fn wrapper_binding_current_actions_and_suspended_expired_guards_are_ordered() {
    let module = Module::new();
    install(&module);
    module.import_public_symbols(&subrules());
    let identities = ["active", "suspended"].map(|name| {
        Arc::new(SubruleIdentity {
            defining_label: CanonicalLabel::parse("@@//:subrule.bzl").unwrap(),
            exported_name: name.into(),
        })
    });
    let context = evaluation_context(Arc::from(identities));
    module.set("actions", actions(&module, &context));
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    for call in [
        "f()",
        "f('l', None, None)",
        "f('l', None, None, 'std', 1)",
        "f(label='l', actions=None, allocator_library=None, std='std', extra=1)",
        "f('l', actions=None, label='twice', allocator_library=None, std='std')",
    ] {
        assert!(execute(&mut eval, call).is_err(), "{call}");
    }
    assert!(
        execute(&mut eval, "f('l', actions, 'fail', 'std')")
            .unwrap_err()
            .to_string()
            .contains("registered")
    );
    let guard = context
        .enter_root_context(&module, module.heap().alloc(AllocList([7])))
        .unwrap();
    for input in ["None", "1", "struct()"] {
        assert!(
            execute(&mut eval, &format!("f('l', {input}, 'fail', 'std')"))
                .unwrap_err()
                .to_string()
                .contains("genuine current")
        );
    }
    let foreign = evaluation_context(Arc::from([]));
    module.set("foreign", actions(&module, &foreign));
    assert!(
        execute(&mut eval, "f('l', foreign, 'fail', 'std')")
            .unwrap_err()
            .to_string()
            .contains("another evaluation")
    );
    execute(&mut eval, "result=f(std='std', allocator_library=None, actions=actions, label='different-from-owner')").unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[\"different-from-owner\", \"std\", False, \"kept\"]"
    );
    let wrapped = crate::subrule_invocation::link_actions::wrap(
        module.get("actions").unwrap(),
        None,
        false,
        &mut eval,
    )
    .unwrap();
    module.set("wrapped", wrapped);
    execute(
        &mut eval,
        "result=f('wrapped-label', wrapped, None, 'other-mode')",
    )
    .unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[\"wrapped-label\", \"other-mode\", False, \"kept\"]"
    );
    execute(&mut eval, "result=active(f)").unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[\"subrule-label\", \"std\", False, \"kept\"]"
    );
    assert!(
        execute(&mut eval, "suspended(f, actions)")
            .unwrap_err()
            .to_string()
            .contains("outside of its own implementation")
    );
    assert!(
        execute(&mut eval, "f('l', actions, 'fail', 'std')")
            .unwrap_err()
            .to_string()
            .contains("synthetic helper reached")
    );
    execute(&mut eval, "result=f('restored', actions, None, 'std')").unwrap();
    drop(guard);
    assert!(
        execute(&mut eval, "f('l', actions, 'fail', 'std')")
            .unwrap_err()
            .to_string()
            .contains("expired")
    );
}

#[test]
fn wrapper_function_identity_hash_and_helper_heap_survive_gc_freeze_and_disposal() {
    let producer = Module::new();
    let retained = install(&producer);
    execute(&mut Evaluator::new(&producer), "alias=f\ndef check_identity():\n    lookup={original: 1}\n    lookup[f]=2\n    if len(lookup) != 1 or f != original or original != f: fail('identity')\n    return lookup\nlookup=check_identity()\n").unwrap();
    let frozen = producer.freeze().unwrap();
    let consumer = Module::new();
    consumer.import_public_symbols(&frozen);
    drop(frozen);
    let context = evaluation_context(Arc::from([]));
    let _guard = context
        .enter_root_context(&consumer, consumer.heap().alloc(AllocList([7])))
        .unwrap();
    consumer.set("actions", actions(&consumer, &context));
    let mut eval = Evaluator::new(&consumer);
    eval.extra = Some(&context);
    execute(&mut eval, "def churn():\n    for i in range(1000):\n        temporary=[str(i)] * 256\nchurn()\ncollected=True\n").unwrap();
    assert!(consumer.heap().allocated_bytes() < consumer.heap().peak_allocated_bytes());
    execute(&mut eval, "result=alias('kept-after-GC', actions, None, 'std')\ndef check_frozen():\n    if lookup[f] != 2 or alias != f or original != f: fail('frozen identity')\ncheck_frozen()\nrestored_callable=f\n").unwrap();
    assert_eq!(
        consumer.get("result").unwrap().to_repr(),
        "[\"kept-after-GC\", \"std\", False, \"kept\"]"
    );
    let (restored, captures) =
        rust_toolchain_callable(consumer.get("restored_callable").unwrap()).unwrap();
    assert_eq!(restored, retained);
    assert_eq!(captures.to_repr(), "(None, None, False, None)");
}
