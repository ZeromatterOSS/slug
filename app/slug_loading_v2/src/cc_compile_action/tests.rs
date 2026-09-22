//! Authored native-boundary callers; genuine context/action publication lives
//! in Analysis. The sentinel cannot construct or publish a compiler action.
use std::sync::Arc;

use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Demand;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

mod gc;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct Sentinel;
starlark::starlark_simple_value!(Sentinel);
impl fmt::Display for Sentinel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<authored capability sentinel>")
    }
}
#[starlark_value(type = "cpp_constructor_test_context")]
impl<'v> StarlarkValue<'v> for Sentinel {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn crate::cc_context::CcCompileActionContext>(self);
    }
}
impl crate::cc_context::CcCompileActionContext for Sentinel {
    fn prepare_cc_compile<'v>(
        &self,
        request: &CcCompileRequest<'v>,
    ) -> anyhow::Result<PreparedCcCompile<'v>> {
        assert!(!request.configuration.coverage_enabled()?);
        assert_eq!(
            request.toolchain_type,
            "@bazel_tools//tools/cpp:toolchain_type"
        );
        assert!(crate::cc_variables::cc_variables_value(request.variables).is_some());
        anyhow::bail!("authored constructor capability reached")
    }
}

fn error(call: &str) -> String {
    let module = Module::new();
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
    module.set(
        "CONFIG",
        crate::cc_compile_outputs::alloc_configuration(module.heap(), configuration).unwrap(),
    );
    module.set("CTX", module.heap().alloc_simple(Sentinel));
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:constructor_test.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let source = format!(
        "i=cc_common.internal_DO_NOT_USE()\nv=i.cc_toolchain_variables(vars={{}})\n{call}\n"
    );
    eval.eval_module(
        AstModule::parse("constructor_test.bzl", source, &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .err()
    .expect("bounded rejected native call")
    .to_string()
}

const CALL: &str = "i.create_cc_compile_action(action_construction_context=CTX,cc_compilation_context=struct(),cc_toolchain=struct(),configuration=CONFIG,feature_configuration=None,source=None,compile_build_variables=v,toolchain_type='@bazel_tools//tools/cpp:toolchain_type'{extra})";
fn with(extra: &str) -> String {
    CALL.replace("{extra}", extra)
}

#[test]
fn constructor_binder_preserves_required_keywords_and_genuine_native_inputs() {
    assert!(error(&with("")).contains("authored constructor capability reached"));
    for (call, needle) in [
        (
            "i.create_cc_compile_action()".to_owned(),
            "action_construction_context",
        ),
        (
            "i.create_cc_compile_action(CTX)".to_owned(),
            "Missing named-only parameter `action_construction_context`",
        ),
        (
            with("").replace(
                "action_construction_context=CTX",
                "action_construction_context=struct()",
            ),
            "genuine rule ctx",
        ),
        (
            with("").replace("configuration=CONFIG", "configuration=struct()"),
            "genuine configuration",
        ),
        (
            with("").replace(
                "compile_build_variables=v",
                "compile_build_variables=struct()",
            ),
            "native CcToolchainVariables",
        ),
        (
            with("").replace(
                "toolchain_type='@bazel_tools//tools/cpp:toolchain_type'",
                "toolchain_type=None",
            ),
            "toolchain_type",
        ),
        (with(",use_pic='yes'"), "use_pic"),
        (
            with(",needs_include_validation=None"),
            "needs_include_validation",
        ),
        (with(",should_scan_includes=1"), "should_scan_includes"),
    ] {
        let actual = error(&call);
        assert!(actual.contains(needle), "{call}: {actual}");
    }
}

#[test]
fn constructor_binder_rejects_unsupported_branches_without_recording() {
    for name in [
        "diagnostics_file",
        "gcno_file",
        "dwo_file",
        "lto_indexing_file",
        "module_files",
        "modmap_file",
        "modmap_input_file",
    ] {
        let actual = error(&with(&format!(",{name}=struct()")));
        assert!(
            actual.contains(name) && actual.contains("unsupported"),
            "{actual}"
        );
        assert!(
            error(&with(&format!(",{name}=None")))
                .contains("authored constructor capability reached")
        );
    }
    for extra in [
        ",copts_filter='-f.*'",
        ",shareable=True",
        ",additional_outputs=[None]",
        ",additional_include_scanning_roots=[None]",
        ",additional_outputs=None",
    ] {
        let actual = error(&with(extra));
        assert!(!actual.contains("capability reached"), "{extra}: {actual}");
    }
    for extra in [
        ",copts_filter=None,action_name=None,should_scan_includes=None,shareable=None",
        ",copts_filter=''",
        ",shareable=False",
        ",additional_outputs=()",
        ",additional_outputs=range(0)",
        ",additional_include_scanning_roots=range(0)",
    ] {
        let actual = error(&with(extra));
        assert!(
            actual.contains("authored constructor capability reached"),
            "{extra}: {actual}"
        );
    }
}

#[test]
fn pending_slot_preserves_other_evaluator_roots() {
    let module = Module::new();
    let registry = crate::evaluator_roots::callable_registry(&module);
    crate::evaluator_roots::set_empty_depsets(&module, module.heap().alloc(11));
    crate::evaluator_roots::set_analysis_context(&module, module.heap().alloc(12));
    crate::evaluator_roots::set_pending_cc_actions(&module, module.heap().alloc(13));
    crate::evaluator_roots::set_analysis_context(&module, Value::new_none());
    assert!(crate::evaluator_roots::callable_registry(&module).ptr_eq(registry));
    assert_eq!(
        crate::evaluator_roots::empty_depsets(&module)
            .unwrap()
            .to_repr(),
        "11"
    );
    assert_eq!(
        crate::evaluator_roots::pending_cc_actions(&module).to_repr(),
        "13"
    );
    assert!(crate::evaluator_roots::analysis_context(&module).is_none());
}
