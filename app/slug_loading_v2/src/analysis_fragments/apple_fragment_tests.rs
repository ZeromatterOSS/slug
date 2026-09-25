//! The rule-only Apple facade uses the existing declaration and call lifetime.

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::subrule_invocation::AnalysisEvaluationContext;

fn apple(heap: &FrozenHeap) -> FrozenValue {
    let configuration = SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap(),
    )
    .unwrap();
    let projection = AppleFragmentProjection::new(configuration).unwrap();
    heap.alloc(AppleFragmentValue::new(projection))
}

fn context() -> AnalysisEvaluationContext {
    AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:apple_fragment_test").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([]),
    )
}

fn eval(module: &Module, source: &str) -> anyhow::Result<()> {
    let mut evaluator = Evaluator::new(module);
    evaluator
        .eval_module(
            AstModule::parse(
                "apple_fragment_test.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .map_err(starlark::Error::into_anyhow)?,
            &Globals::standard(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}

#[test]
fn apple_rule_fragment_is_declared_and_expires_with_its_root_call() {
    let heap = FrozenHeap::new();
    let apple = apple(&heap);
    let context = context();
    let declarations = Arc::new(SmallSet::from_iter([CompactString::new("apple")]));
    let module = Module::new();
    module.set(
        "fragments",
        module.heap().alloc_simple(RuleFragmentCollection::new(
            context.root_token(),
            declarations,
            FrozenValue::new_none(),
            apple,
            FrozenValue::new_none(),
        )),
    );
    let guard = context
        .enter_root_context(&module, Value::new_none())
        .unwrap();
    eval(
        &module,
        "apple = fragments.apple\nRESULT = [\n    apple.xcode_version_flag == None,\n    apple.ios_sdk_version_flag == None,\n    apple.macos_sdk_version_flag == None,\n    apple.tvos_sdk_version_flag == None,\n    apple.watchos_sdk_version_flag == None,\n    apple.ios_minimum_os_flag == None,\n    apple.macos_minimum_os_flag == None,\n    apple.tvos_minimum_os_flag == None,\n    apple.watchos_minimum_os_flag == None,\n    apple.prefer_mutual_xcode,\n    not apple.include_xcode_exec_requirements,\n]\n",
    )
    .unwrap();
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "[True, True, True, True, True, True, True, True, True, True, True]"
    );
    drop(guard);
    assert!(
        eval(&module, "EXPIRED = fragments.apple\n")
            .unwrap_err()
            .to_string()
            .contains("outside of its own implementation function")
    );
}

#[test]
fn apple_rule_fragment_rejects_undeclared_access() {
    let heap = FrozenHeap::new();
    let context = context();
    let module = Module::new();
    module.set(
        "fragments",
        module.heap().alloc_simple(RuleFragmentCollection::new(
            context.root_token(),
            Arc::new(SmallSet::new()),
            FrozenValue::new_none(),
            apple(&heap),
            FrozenValue::new_none(),
        )),
    );
    let _guard = context
        .enter_root_context(&module, Value::new_none())
        .unwrap();
    assert!(
        eval(&module, "UNDECLARED = fragments.apple\n")
            .unwrap_err()
            .to_string()
            .contains("rule has to declare 'apple'")
    );
}
