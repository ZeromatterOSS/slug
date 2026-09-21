use std::sync::Arc;

use slug_build_api_v2::cc_features::CcArtifactNamePattern;
use slug_build_api_v2::cc_features::CcExecOs;
use slug_build_api_v2::cc_features::CcToolchainFeatures;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenValue;

use super::*;
use crate::cc_features::alloc_frozen_cc_toolchain_features;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

const LABEL: &str = "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl";
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl"
));

fn definition(label: &str, source: &str) -> FrozenModule {
    let module = Module::new();
    let context = BzlEvaluationContext::new(label);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("definition.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn evaluate(
    definition: &FrozenModule,
    body: &str,
    host: Option<ActionEnvironmentHostOs>,
) -> anyhow::Result<FrozenModule> {
    let module = Module::new();
    module.import_public_symbols(definition);
    // The complete unchanged provider exposes a raw constructor privately.
    // Host access here is a test boundary, not a new Starlark private import.
    if let Ok((raw, _)) = definition.get_assigned("_") {
        module.set("Raw", raw.value());
    }
    let features = CcToolchainFeatures::new(
        vec![],
        vec![],
        vec![CcArtifactNamePattern::new("object_file", "obj_", ".obj").unwrap()],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap();
    module.set(
        "MODEL",
        alloc_frozen_cc_toolchain_features(module.frozen_heap(), features).to_value(),
    );
    // Obtain genuine internal API under its existing exposure check; actual
    // query is invoked with an ordinary consumer label and no private gate.
    let internal = definition_internal();
    module.import_public_symbols(&internal);
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
    eval.eval_module(
        AstModule::parse("consumer.bzl", body.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    drop(eval);
    Ok(module.freeze()?)
}

fn definition_internal() -> FrozenModule {
    definition(
        "@@rules_cc+//cc/private:naming_test.bzl",
        "i = cc_common.internal_DO_NOT_USE()",
    )
}

#[test]
fn genuine_raw_provider_and_frozen_schemaful_provider_use_native_naming() {
    let definition = definition(LABEL, SOURCE);
    for host in [
        ActionEnvironmentHostOs::Linux,
        ActionEnvironmentHostOs::Macos,
        ActionEnvironmentHostOs::Freebsd,
        ActionEnvironmentHostOs::Openbsd,
        ActionEnvironmentHostOs::Unknown,
    ] {
        let result = evaluate(&definition, r#"
tc = Raw(_toolchain_features = MODEL)
result = i.get_artifact_name_for_category(cc_toolchain = tc, category = 'OBJECT_FILE', output_name = 'dir//sub/../unit.cc')
backslash = i.get_artifact_name_for_category(cc_toolchain = tc, category = 'OBJECT_FILE', output_name = 'dir\\unit')
"#, Some(host)).unwrap();
        assert_eq!(
            result.get("result").unwrap().value().unpack_str(),
            Some("dir/obj_unit.cc.obj")
        );
        assert_eq!(
            result.get("backslash").unwrap().value().unpack_str(),
            Some("obj_dir\\unit.obj")
        );
        let frozen = evaluate(&result, "result = i.get_artifact_name_for_category(cc_toolchain=tc, category='INCLUDED_FILE_LIST', output_name='dir/unit')", Some(host)).unwrap();
        assert_eq!(
            frozen.get("result").unwrap().value().unpack_str(),
            Some("dir/unit.d")
        );
    }
}

#[test]
fn naming_rejects_forged_keys_schemaless_providers_and_non_native_models() {
    for (label, source, construct) in [
        (
            "@@foreign+//cc/private/rules_impl:cc_toolchain_info.bzl",
            "CcToolchainInfo = provider(fields=['_toolchain_features'])",
            "CcToolchainInfo(_toolchain_features=MODEL)",
        ),
        (
            LABEL,
            "Other = provider(fields=['_toolchain_features'])",
            "Other(_toolchain_features=MODEL)",
        ),
        (
            LABEL,
            "CcToolchainInfo = provider()",
            "CcToolchainInfo(_toolchain_features=MODEL)",
        ),
        (LABEL, SOURCE, "struct(_toolchain_features=MODEL)"),
        (LABEL, SOURCE, "Raw()"),
        (LABEL, SOURCE, "Raw(_toolchain_features=struct())"),
        (LABEL, SOURCE, "Raw(_toolchain_features=None)"),
    ] {
        let definition = definition(label, source);
        let body = format!(
            "tc = {construct}\nresult = i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE', output_name='unit')"
        );
        let error = evaluate(&definition, &body, Some(ActionEnvironmentHostOs::Linux))
            .err()
            .expect("invalid provider/model must fail");
        assert!(
            error.to_string().contains("CcToolchain"),
            "{label}: {error:#}"
        );
    }
}

#[test]
fn naming_requires_keyword_arguments_typed_inputs_and_observed_posix_host() {
    let definition = definition(LABEL, SOURCE);
    let call = "i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE', output_name='unit')";
    for host in [None, Some(ActionEnvironmentHostOs::Windows)] {
        let error = evaluate(
            &definition,
            &format!("tc = Raw(_toolchain_features=MODEL)\nresult = {call}"),
            host,
        )
        .err()
        .expect("Host boundary must reject");
        assert!(
            error.to_string().contains(if host.is_none() {
                "observed analysis Host"
            } else {
                "Windows artifact naming"
            }),
            "{error:#}"
        );
    }
    for call in [
        "i.get_artifact_name_for_category(tc, 'OBJECT_FILE', 'unit')",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE')",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category=1, output_name='unit')",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE', output_name=None)",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category='object_file', output_name='unit')",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE', output_name='/')",
        "i.get_artifact_name_for_category(cc_toolchain=tc, category='OBJECT_FILE', output_name='unit', extra=True)",
    ] {
        let body = format!("tc = Raw(_toolchain_features=MODEL)\nresult = {call}");
        assert!(
            evaluate(&definition, &body, Some(ActionEnvironmentHostOs::Linux)).is_err(),
            "{call}"
        );
    }
}
