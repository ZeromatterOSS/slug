//! Native boundary regressions, not a full compile-action or rule invocation.
use std::sync::Arc;

use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

// Authored capability sentinel only. Real ctx/sink lifetime and registration are
// tested in Analysis; this isolates the native binder's Host/type gate ordering.
#[derive(
    Debug,
    allocative::Allocative,
    starlark::any::ProvidesStaticType,
    starlark::values::NoSerialize
)]
struct ContextSentinel;
starlark::starlark_simple_value!(ContextSentinel);
impl std::fmt::Display for ContextSentinel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<test ctx capability>")
    }
}
#[starlark::values::starlark_value(type = "test_ctx_capability")]
impl<'v> starlark::values::StarlarkValue<'v> for ContextSentinel {
    fn provide(&'v self, demand: &mut starlark::values::Demand<'_, 'v>) {
        demand.provide_value::<&dyn crate::cc_context::CcCompileOutputContext>(self);
    }
}
impl crate::cc_context::CcCompileOutputContext for ContextSentinel {
    fn declare_compile_output(
        &self,
        label: &CanonicalLabel,
        output_name: &str,
        configuration: &SlugConfiguration,
    ) -> anyhow::Result<crate::subrule_invocation::AnalysisArtifactValue> {
        assert_eq!(label.to_string(), "@@//:location");
        assert_eq!(output_name, "unit.o");
        assert_eq!(configuration, &config(false));
        anyhow::bail!("test declaration capability reached")
    }
}

fn config(coverage: bool) -> SlugConfiguration {
    let base = SlugConfiguration::default_target(
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
    if !coverage {
        return base;
    }
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("compile-output-tests").unwrap());
    base.with_command_configuration_context(
        base.starlark_options().clone(),
        &CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
            NativeCommandOption::CollectCodeCoverage,
            Some("true"),
            false,
        )]),
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap()
}

fn internal() -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:compile_outputs_test.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(
            "internal.bzl",
            "i = cc_common.internal_DO_NOT_USE()".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn evaluate(source: &str, coverage: bool) -> anyhow::Result<String> {
    let module = Module::new();
    module.import_public_symbols(&internal());
    let structural = config(coverage);
    let value = alloc_configuration(module.heap(), structural.clone())?;
    assert_eq!(configuration::unpack(value)?, &structural);
    module.set("CONFIG", value);
    // These entrypoints have no additional private caller gate.
    let context = BzlEvaluationContext::new("@@//:ordinary_consumer.bzl");
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("consumer.bzl", source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(module.get("result").unwrap().to_repr())
}

#[test]
fn prefix_uses_required_string_suffix_and_preserves_structural_coverage() {
    for (purpose, expected) in [
        ("cc_library-compile", ""),
        ("", ""),
        ("_objc_arc", "arc"),
        ("compile_objc_arc", "arc"),
        ("compile_non_objc_arc", "non_arc"),
        ("_non_objc_arc", "non_arc"),
        ("compile_OBJC_ARC", ""),
        ("compile_objc_arc_suffix", ""),
        ("objc_arc", ""),
    ] {
        assert_eq!(evaluate(&format!("result = i.compute_output_name_prefix_dir(configuration=CONFIG, purpose={purpose:?})"), false).unwrap(), format!("{expected:?}"));
    }
    for (coverage, expected) in [(false, "False"), (true, "True")] {
        assert_eq!(
            evaluate("result = CONFIG.coverage_enabled", coverage).unwrap(),
            expected
        );
        assert!(evaluate("result = CONFIG.coverage_enabled()", coverage).is_err());
        assert!(evaluate("CONFIG.coverage_enabled = False\nresult = None", coverage).is_err());
    }
    assert_ne!(config(false), config(true));
}

#[test]
fn native_compile_output_binders_reject_missing_positional_and_forged_inputs() {
    for call in [
        "i.compute_output_name_prefix_dir()",
        "i.compute_output_name_prefix_dir(configuration=CONFIG)",
        "i.compute_output_name_prefix_dir(purpose='x')",
        "i.compute_output_name_prefix_dir(CONFIG, 'x')",
        "i.compute_output_name_prefix_dir(configuration=CONFIG,purpose=None)",
        "i.compute_output_name_prefix_dir(configuration=CONFIG,purpose=1)",
        "i.compute_output_name_prefix_dir(configuration=struct(coverage_enabled=False),purpose='x')",
        "i.compute_output_name_prefix_dir(configuration=None,purpose='x')",
        "i.compute_output_name_prefix_dir(configuration=CONFIG,purpose='x',extra=True)",
        "i.declare_compile_output_file()",
        "i.declare_compile_output_file(struct(),Label('//:x'),'a.o',CONFIG)",
        "i.declare_compile_output_file(ctx=struct(),label=Label('//:x'),configuration=CONFIG)",
        "i.declare_compile_output_file(ctx=struct(),label=Label('//:x'),output_name=None,configuration=CONFIG)",
        "i.declare_compile_output_file(ctx=struct(),label=Label('//:x'),output_name=7,configuration=CONFIG)",
    ] {
        assert!(
            evaluate(&format!("result = {call}"), false).is_err(),
            "{call}"
        );
    }
    let error = evaluate("result = i.declare_compile_output_file(ctx=struct(),label=Label('//:x'),output_name='a.o',configuration=CONFIG)", false).unwrap_err();
    assert!(
        error.to_string().contains("requires a rule ctx"),
        "{error:#}"
    );
}

#[test]
fn compile_output_coordinates_normalize_then_enforce_active_package_containment() {
    for (active, location, name, expected) in [
        (
            "@@//pkg:owner",
            "@@//pkg:other",
            "unit.o",
            "pkg/_objs/other/unit.o",
        ),
        (
            "@@//pkg:owner",
            "@@//pkg/child:other",
            "unit.o",
            "pkg/child/_objs/other/unit.o",
        ),
        (
            "@@//pkg:owner",
            "@@//pkg:dir/other",
            "unit.o",
            "pkg/_objs/dir/other/unit.o",
        ),
        (
            "@@//pkg:owner",
            "@@//pkg:other",
            "a//./b/../unit.o",
            "pkg/_objs/other/a/unit.o",
        ),
        (
            "@@//pkg:owner",
            "@@//pkg:other",
            "../../inside.o",
            "pkg/inside.o",
        ),
        ("@@//pkg:owner", "@@//pkg:other", "", "pkg/_objs/other"),
        ("@@//:owner", "@@//:other", "unit.o", "_objs/other/unit.o"),
        (
            "@@//:owner",
            "@@//child:other",
            "unit.o",
            "child/_objs/other/unit.o",
        ),
        (
            "@@repo+//pkg:owner",
            "@@repo+//pkg:other",
            "unit.o",
            "external/repo+/pkg/_objs/other/unit.o",
        ),
        (
            "@@repo+//:owner",
            "@@repo+//:other",
            "unit.o",
            "external/repo+/_objs/other/unit.o",
        ),
    ] {
        assert_eq!(
            compile_output_path(
                &CanonicalLabel::parse(active).unwrap(),
                &CanonicalLabel::parse(location).unwrap(),
                name
            )
            .unwrap(),
            expected
        );
    }
    for (active, location, name) in [
        ("@@//pkg:owner", "@@//sibling:other", "unit.o"),
        ("@@//pkg:owner", "@@//pkg_suffix:other", "unit.o"),
        ("@@repo+//pkg:owner", "@@other+//pkg:other", "unit.o"),
        ("@@repo+//pkg:owner", "@@//pkg:other", "unit.o"),
        ("@@//pkg:owner", "@@//pkg:other", "../../../outside.o"),
        ("@@//:owner", "@@//:other", "../../../outside.o"),
        ("@@//pkg:owner", "@@//pkg:other", "/absolute.o"),
    ] {
        assert!(
            compile_output_path(
                &CanonicalLabel::parse(active).unwrap(),
                &CanonicalLabel::parse(location).unwrap(),
                name
            )
            .is_err(),
            "{active}, {location}, {name}"
        );
    }
}

#[test]
fn declaration_checks_genuine_values_and_observed_host_before_capability() {
    use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
    use starlark::values::FrozenValue;
    for (host, label, configuration, expected) in [
        (None, "LABEL", "CONFIG", "observed analysis Host"),
        (
            Some(ActionEnvironmentHostOs::Windows),
            "LABEL",
            "CONFIG",
            "Windows compile output paths",
        ),
        (
            Some(ActionEnvironmentHostOs::Linux),
            "LABEL",
            "CONFIG",
            "test declaration capability reached",
        ),
        (
            Some(ActionEnvironmentHostOs::Linux),
            "'@@//:location'",
            "CONFIG",
            "requires a Label",
        ),
        (
            Some(ActionEnvironmentHostOs::Linux),
            "LABEL",
            "struct(coverage_enabled=False)",
            "requires a genuine configuration",
        ),
    ] {
        let module = Module::new();
        module.import_public_symbols(&internal());
        module.set("CTX", module.heap().alloc_simple(ContextSentinel));
        module.set(
            "LABEL",
            crate::provider::alloc_starlark_label(
                module.heap(),
                CanonicalLabel::parse("@@//:location").unwrap(),
            ),
        );
        module.set(
            "CONFIG",
            alloc_configuration(module.heap(), config(false)).unwrap(),
        );
        let context = AnalysisEvaluationContext::new(
            Arc::from([]),
            [],
            CanonicalLabel::parse("@@//:consumer").unwrap(),
            Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
            FrozenValue::new_none(),
            Arc::from([]),
        )
        .with_host_os(host);
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        let source = format!(
            "i.declare_compile_output_file(ctx=CTX,label={label},output_name='unit.o',configuration={configuration})"
        );
        let error = eval
            .eval_module(
                AstModule::parse("host_boundary.bzl", source, &Dialect::Bazel).unwrap(),
                &loading_globals(),
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error:#}");
    }
}
