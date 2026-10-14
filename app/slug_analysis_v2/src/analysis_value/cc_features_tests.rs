//! Native-model publication proof, not the public rules_cc configure_features route.
use std::sync::Arc;

use slug_build_api_v2::cc_features::*;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleIdentity;
use slug_loading_v2::BzlModuleSourceProvenance;
use slug_loading_v2::subrule_invocation::AnalysisActionSink;
use slug_loading_v2::subrule_invocation::AnalysisEvaluationContext;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[derive(Debug)]
struct NoActions;
impl AnalysisActionSink for NoActions {
    fn declare_file(
        &self,
        _: &str,
        _: Option<&AnalysisArtifact>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!("feature configuration does not declare actions")
    }
    fn write<'v>(&self, _: &'v Module, _: Value<'v>, _: Value<'v>, _: bool) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run_shell(&self, _: Value<'_>, _: &str, _: Value<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn artifact_symlink(
        &self,
        _: Value<'_>,
        _: Value<'_>,
        _: bool,
        _: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!()
    }
    fn absolute_symlink(&self, _: Value<'_>, _: &str, _: Option<&str>) -> anyhow::Result<()> {
        unreachable!()
    }
}

fn context() -> AnalysisEvaluationContext {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//test:feature_retention.bzl").unwrap(),
        workspace_path: "/fixture/test/feature_retention.bzl".into(),
        repository_mapping: Arc::from([]),
    };
    AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        identity.label.clone(),
        Arc::new(NoActions),
        FrozenValue::new_none(),
        Arc::from([
            (
                CompactString::new("/fixture/test/feature_retention.bzl"),
                BzlModuleSourceProvenance::new(identity.clone(), [0; 32]),
            ),
            (
                CompactString::new("assert.bzl"),
                BzlModuleSourceProvenance::new(
                    BzlModuleIdentity {
                        workspace_path: "assert.bzl".into(),
                        ..identity
                    },
                    [0; 32],
                ),
            ),
        ]),
    )
}

fn evaluate(module: &Module, source: &str) -> Result<(), String> {
    let ast = AstModule::parse(
        "/fixture/test/feature_retention.bzl",
        source.to_owned(),
        &Dialect::Bazel,
    )
    .map_err(|error| error.to_string())?;
    let context = context();
    let mut eval = Evaluator::new(module);
    eval.extra = Some(&context);
    eval.eval_module(ast, &Globals::standard())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn provider(value: AnalysisValue) -> ProviderOccurrence {
    ProviderOccurrence::new(
        ProviderIdentity::user(
            slug_build_api_v2::ProviderId::new("//test:feature_retention.bzl", "CcCarrier")
                .unwrap(),
        ),
        [("feature", value)],
    )
}

fn toolchain(
    directory: &str,
    flag: &str,
    artifact: Option<AnalysisArtifact>,
) -> CcToolchainFeatures {
    let feature = CcFeature {
        name: "selected".into(),
        enabled: true,
        flag_sets: vec![CcFlagSet {
            actions: ["c++-compile".into()].into_iter().collect(),
            with_features: CcSet::new(),
            flag_groups: vec![CcFlagGroup {
                expandables: vec![CcExpandable::Group(CcFlagGroup {
                    expandables: vec![CcExpandable::Flag(parse_cc_string(flag).unwrap().into())]
                        .into(),
                    iterate_over: Some("defines".into()),
                    expand_if_available: Some("defines".into()),
                    expand_if_not_available: None,
                    expand_if_true: None,
                    expand_if_false: None,
                    expand_if_equal: None,
                })]
                .into(),
                iterate_over: None,
                expand_if_available: None,
                expand_if_not_available: None,
                expand_if_true: None,
                expand_if_false: None,
                expand_if_equal: None,
            }]
            .into(),
        }]
        .into(),
        env_sets: vec![].into(),
        requires: vec![].into(),
        implies: vec![].into(),
        provides: vec![].into(),
    };
    let tools = match artifact {
        Some(artifact) => vec![
            CcTool::new(
                None,
                Some(artifact),
                CcSet::new(),
                CcSet::new(),
                CcExecOs::Linux,
            )
            .unwrap(),
        ],
        None => vec![],
    };
    let action = CcActionConfig {
        action_name: "c++-compile".into(),
        enabled: false,
        tools: tools.into(),
        flag_sets: vec![].into(),
        implies: vec![].into(),
    };
    CcToolchainFeatures::new(
        vec![feature],
        vec![action],
        vec![],
        directory,
        CcExecOs::Linux,
    )
    .unwrap()
}

#[test]
fn native_cc_features_round_trip_outlives_heap_and_invokes_after_gc() {
    let retained = {
        let source = FrozenHeap::new();
        let a = toolchain("tools", "-DKEEP", None);
        let b = toolchain("tools", "-DKEEP", None);
        let c = a
            .configure_features(vec!["selected".into(), "unknown".into()])
            .unwrap();
        let other_c = a
            .configure_features(vec!["selected".into(), "unknown".into()])
            .unwrap();
        let values = [
            AnalysisValue::cc_toolchain_features(a.clone()),
            AnalysisValue::cc_toolchain_features(a),
            AnalysisValue::cc_toolchain_features(b),
            AnalysisValue::cc_feature_configuration(c.clone()),
            AnalysisValue::cc_feature_configuration(c.clone()),
            AnalysisValue::cc_feature_configuration(other_c),
            AnalysisValue::provider(provider(AnalysisValue::cc_feature_configuration(c))),
        ];
        let value = AnalysisValueMaterializer::new(&source)
            .value(&AnalysisValue::tuple(values.to_vec()))
            .unwrap();
        AnalysisValueLowerer::default()
            .lower(value.to_value(), "source")
            .unwrap()
    }; // The complete source heap is gone before rematerialization.
    let module = Module::new();
    let restored = AnalysisValueMaterializer::new(module.frozen_heap())
        .value(&retained)
        .unwrap();
    module.set("values", restored.to_value());
    evaluate(
        &module,
        r#"
def inspect():
    a, alias, b, c, c_alias, c_fresh, carrier = values
    return (type(a), type(c), a == alias, a == b, c == c_alias, c == c_fresh,
            a.default_features_and_action_configs(), c.is_enabled("selected"),
            c.is_requested("unknown"), c.is_enabled("unknown"),
            carrier.feature.is_requested("selected"))
result = inspect()
"#,
    )
    .unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "(\"CcToolchainFeatures\", \"FeatureConfiguration\", True, False, True, False, [\"selected\"], True, True, False, True)"
    );
    // Freeze the destination, then run the evaluator's supported GC test modes.
    evaluate(&module, "again = inspect()\n").unwrap();
    let frozen = module.freeze().unwrap();
    static CONTEXT: std::sync::LazyLock<AnalysisEvaluationContext> =
        std::sync::LazyLock::new(context);
    let mut gc = starlark::assert::Assert::new();
    gc.disable_static_typechecking();
    gc.setup_eval(|eval| eval.extra = Some(&*CONTEXT));
    gc.module_add("retained", frozen.clone());
    // Assert runs Auto, Never and Always GC modes. Its garbage_collect builtin
    // schedules collection at a safe bytecode boundary, with all values rooted.
    gc.pass(
        r#"
load("retained", "values")
def select():
    return values[0].configure_features(["selected", "unknown"])
c = select()
garbage_collect()
def inspect_live():
    return c.is_enabled("selected") and c.is_requested("unknown") and not c.is_enabled("unknown")
assert_true(inspect_live())
"#,
    );
    let restored = frozen.get("values").unwrap();
    let twice = AnalysisValueLowerer::default()
        .lower(restored.value(), "restored")
        .unwrap();
    assert_eq!(retained, twice);
    assert!(retained.publication_eq(&twice));
    for value in TupleRef::from_value(restored.value()).unwrap().iter() {
        assert!(value.get_hashed().is_err());
    }
}

#[test]
fn native_cc_features_publication_preserves_aliases_across_providers() {
    let a = toolchain("tools", "-DKEEP", None);
    let b = toolchain("tools", "-DKEEP", None);
    let c = toolchain("tools", "-DKEEP", None);
    let left = [
        provider(AnalysisValue::cc_toolchain_features(a.clone())),
        provider(AnalysisValue::cc_toolchain_features(a)),
    ];
    let right = [
        provider(AnalysisValue::cc_toolchain_features(b.clone())),
        provider(AnalysisValue::cc_toolchain_features(b.clone())),
    ];
    let split = [
        provider(AnalysisValue::cc_toolchain_features(b)),
        provider(AnalysisValue::cc_toolchain_features(c)),
    ];
    assert!(ProviderOccurrence::publication_eq_pairs(
        left.iter().zip(&right)
    ));
    assert!(!ProviderOccurrence::publication_eq_pairs(
        left.iter().zip(&split)
    ));
    assert!(!ProviderOccurrence::publication_eq_pairs(
        split.iter().zip(&left)
    ));

    let model = toolchain("tools", "-DKEEP", None);
    let a = model.configure_features(vec!["selected".into()]).unwrap();
    let b = model.configure_features(vec!["selected".into()]).unwrap();
    let c = model.configure_features(vec!["selected".into()]).unwrap();
    let pair = |a, b| {
        AnalysisValue::tuple(vec![
            AnalysisValue::cc_feature_configuration(a),
            AnalysisValue::cc_feature_configuration(b),
        ])
    };
    assert!(pair(a.clone(), a.clone()).publication_eq(&pair(b.clone(), b.clone())));
    assert!(!pair(a.clone(), a.clone()).publication_eq(&pair(b.clone(), c.clone())));
    assert!(!pair(b, c).publication_eq(&pair(a.clone(), a)));
}

#[test]
fn native_cc_features_publication_compares_unexpanded_metadata_and_requests() {
    let artifact = |configuration: &[u8]| AnalysisArtifact::Derived {
        owner: slug_build_api_v2::AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//tools:compiler").unwrap(),
            configuration,
        ),
        output: slug_build_api_v2::ActionOutput::new(
            crate::analysis_value::test_artifact_root(),
            "bin/compiler",
            slug_build_api_v2::ActionOutputKind::File,
        ),
    };
    let a = toolchain("tools", "-DKEEP", Some(artifact(b"one")));
    for changed in [
        toolchain("other-tools", "-DKEEP", Some(artifact(b"one"))),
        toolchain("tools", "-DCHANGED", Some(artifact(b"one"))),
        toolchain("tools", "-DKEEP", Some(artifact(b"two"))),
    ] {
        assert!(
            !AnalysisValue::cc_toolchain_features(a.clone())
                .publication_eq(&AnalysisValue::cc_toolchain_features(changed.clone()))
        );
        let left = a.configure_features(vec!["selected".into()]).unwrap();
        let right = changed.configure_features(vec!["selected".into()]).unwrap();
        assert!(left.is_enabled("selected") && right.is_enabled("selected"));
        assert!(
            !AnalysisValue::cc_feature_configuration(left)
                .publication_eq(&AnalysisValue::cc_feature_configuration(right))
        );
    }
    let cfg = |requests: &[&str]| {
        AnalysisValue::cc_feature_configuration(
            a.configure_features(requests.iter().map(|s| (*s).into()).collect())
                .unwrap(),
        )
    };
    assert!(!cfg(&["selected"]).publication_eq(&cfg(&["selected", "unknown"])));
    assert!(!cfg(&["selected", "unknown"]).publication_eq(&cfg(&["unknown", "selected"])));
    assert!(cfg(&["selected", "selected"]).publication_eq(&cfg(&["selected"])));
}

#[test]
fn native_cc_features_retained_hash_barriers_include_containing_providers() {
    let model = toolchain("tools", "-DKEEP", None);
    let config = model.configure_features(vec![]).unwrap();
    for value in [
        AnalysisValue::cc_toolchain_features(model),
        AnalysisValue::cc_feature_configuration(config),
    ] {
        for value in [
            value.clone(),
            AnalysisValue::tuple(vec![value.clone()]),
            AnalysisValue::strukt([("value", value.clone())]),
            AnalysisValue::provider(provider(value)),
        ] {
            assert!(!value.is_starlark_immutable());
            assert!(!value.is_starlark_hashable());
            assert!(value.starlark_hash().is_err());
            assert!(AnalysisValue::dictionary([(value.clone(), AnalysisValue::none())]).is_err());
            assert!(
                AnalysisDepset::new(slug_build_api_v2::DepsetOrder::Default, vec![value], vec![])
                    .is_err()
            );
        }
    }
}
