//! The rule-only Apple facade uses the existing declaration and call lifetime.

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::structs::AllocStruct;

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
    let module_heap = module.heap();
    module.set(
        "ORDINARY",
        module_heap.alloc(AllocStruct([
            ("name", module_heap.alloc("macos")),
            ("platform_type", module_heap.alloc("macos")),
            ("name_in_plist", module_heap.alloc("MacOSX")),
            ("is_device", module_heap.alloc(true)),
        ])),
    );
    eval(
        &module,
        "PLATFORM = fragments.apple.single_arch_platform\n\
         PLATFORM_AGAIN = fragments.apple.single_arch_platform\n\
         PLATFORM_RESULT = [\n\
             type(PLATFORM) == 'apple_platform',\n\
             repr(PLATFORM) == 'macos',\n\
             PLATFORM.name == 'macos',\n\
             PLATFORM.platform_type == 'macos',\n\
             PLATFORM.name_in_plist == 'MacOSX',\n\
             PLATFORM.is_device == True,\n\
             dir(PLATFORM) == ['is_device', 'name', 'name_in_plist', 'platform_type'],\n\
             PLATFORM == PLATFORM_AGAIN,\n\
             {PLATFORM: 'macos'}[PLATFORM_AGAIN] == 'macos',\n\
             PLATFORM != ORDINARY,\n\
         ]\n",
    )
    .unwrap();
    assert_eq!(
        module.get("PLATFORM_RESULT").unwrap().to_repr(),
        "[True, True, True, True, True, True, True, True, True, True]"
    );
    let first = module.get("PLATFORM").unwrap();
    let second = module.get("PLATFORM_AGAIN").unwrap();
    assert!(!first.ptr_eq(second));
    assert!(first.equals(second).unwrap());
    assert_eq!(
        first.get_hashed().unwrap().hash(),
        second.get_hashed().unwrap().hash()
    );
    assert!(
        eval(
            &module,
            "UNKNOWN = fragments.apple.single_arch_platform.unknown\n"
        )
        .unwrap_err()
        .to_string()
        .contains("has no attribute `unknown`")
    );
    drop(guard);
    assert!(
        eval(&module, "EXPIRED = fragments.apple.single_arch_platform\n")
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
        eval(
            &module,
            "UNDECLARED = fragments.apple.single_arch_platform\n"
        )
        .unwrap_err()
        .to_string()
        .contains("rule has to declare 'apple'")
    );
}

#[test]
fn default_macos_platform_freezes_without_losing_type_or_fields() {
    let module = Module::new();
    module.set(
        "PLATFORM",
        module.heap().alloc_simple(DefaultMacosApplePlatform),
    );
    let frozen = module.freeze().unwrap();
    let binding = frozen.get("PLATFORM").unwrap();
    let platform = binding.value();
    assert_eq!(platform.get_type(), "apple_platform");
    assert_eq!(platform.to_repr(), "macos");
    let scratch = Module::new();
    assert_eq!(
        platform
            .get_attr("platform_type", scratch.heap())
            .unwrap()
            .unwrap()
            .unpack_str(),
        Some("macos")
    );
    assert!(platform.get_hashed().is_ok());
}
