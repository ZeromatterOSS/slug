//! Authenticated constructor definitions plus authored native-boundary calls.
//! This does not invoke the public configure_features wrapper or cc_library.
use std::path::PathBuf;
use std::sync::Arc;

use compact_str::CompactString;
use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::ValueLike;
use starlark::values::structs::starlark_structural_is_immutable;

use super::*;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

const LIB: &str = include_str!(
    "../../../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_cc/cc/cc_toolchain_config_lib.bzl"
);
const FILENAME: &str = "feature_test.bzl";
const SETUP: &str = r#"
internal = cc_common.internal_DO_NOT_USE()
Config = provider(fields = ["_features_DO_NOT_USE", "_action_configs_DO_NOT_USE", "_artifact_name_patterns_DO_NOT_USE", "_exec_os_DO_NOT_USE"])
def info(features = [], actions = [], patterns = [], os = "LINUX"):
    return Config(_features_DO_NOT_USE = features, _action_configs_DO_NOT_USE = actions, _artifact_name_patterns_DO_NOT_USE = patterns, _exec_os_DO_NOT_USE = os)
def make(features = [], actions = [], patterns = [], os = "LINUX", directory = "tools"):
    return internal.cc_toolchain_features(toolchain_config_info = info(features, actions, patterns, os), tools_directory = directory)
def call(value):
    return value.default_features_and_action_configs()
"#;

fn context(label: &str, source: &str) -> BzlEvaluationContext {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: PathBuf::from(FILENAME),
        repository_mapping: Arc::from([]),
    };
    BzlEvaluationContext::macro_runtime_context(
        identity.clone(),
        Arc::from([(
            CompactString::new(FILENAME),
            BzlModuleSourceProvenance::new(identity, Sha256::digest(source).into()),
        )]),
    )
}

fn eval_in(module: &Module, source: &str, label: &str) -> anyhow::Result<()> {
    let context = context(label, source);
    let ast = AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
        .map_err(starlark::Error::into_anyhow)?;
    let prepared = crate::bzl_module::test_source::prepare(
        &module,
        &ast,
        context.source_identity().clone(),
        &[],
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(prepared.context());
    eval.eval_module(ast, &loading_globals())
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}
fn eval_hash_expression(module: &Module, source: &str) -> anyhow::Result<()> {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &loading_globals(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}

fn library() -> FrozenModule {
    let module = Module::new();
    eval_in(&module, LIB, "@@rules_cc+//cc:cc_toolchain_config_lib.bzl").unwrap();
    module.freeze().unwrap()
}
fn boundary(body: &str) -> anyhow::Result<FrozenModule> {
    let module = Module::new();
    module.import_public_symbols(&library());
    eval_in(
        &module,
        &format!("{SETUP}\n{body}"),
        "@@rules_cc+//cc/private:feature_test.bzl",
    )?;
    Ok(module.freeze()?)
}
fn model(module: &FrozenModule, name: &str) -> CcToolchainFeatures {
    cc_toolchain_features_value(module.get(name).unwrap().value()).unwrap()
}
fn config(module: &FrozenModule, name: &str) -> CcFeatureConfiguration {
    cc_feature_configuration_value(module.get(name).unwrap().value()).unwrap()
}

#[test]
fn authentic_definitions_feed_native_metadata_and_selection() {
    assert_eq!(
        format!("{:x}", Sha256::digest(LIB)),
        "f8418490663f7e188fa060265b215e80b154a5d190b32ef75fcb6d0254017808"
    );
    let module = boundary(r#"
def run():
    group = flag_group(flag_groups = [flag_group(flags = ["-I%{include}", "%%literal", "%{path:tools/../bin/cc}"], expand_if_equal = variable_with_value(name = "mode", value = "yes"))], iterate_over = "items", expand_if_available = "available", expand_if_not_available = "absent", expand_if_true = "yes", expand_if_false = "no")
    predicate = with_feature_set(features = ["base"], not_features = ["other"])
    sets = [flag_set(actions = ["c++-compile"], flag_groups = [group], with_features = [predicate])]
    envs = [env_set(actions = ["c++-compile"], env_entries = [env_entry("OUT", "%{output}", expand_if_available = "output")], with_features = [predicate])]
    action = action_config(action_name = "c++-compile", enabled = True, tools = [tool(path = "../bin/./cc", with_features = [predicate], execution_requirements = ["local", "local"])], flag_sets = [flag_set(flag_groups = [flag_group(flags = ["-c"])])], implies = ["base"])
    features = [feature(name = "base", enabled = True, flag_sets = sets, env_sets = envs), feature(name = "dependent", requires = [feature_set(["base"])], implies = ["base"])]
    m = make(features, [action], [artifact_name_pattern(category_name = "static_library", prefix = "custom", extension = ".a")])
    defaults = m.default_features_and_action_configs()
    c = m.configure_features(["dependent", "unknown"])
    checks = (c.is_requested("unknown"), c.is_enabled("unknown"), c.is_enabled("dependent"), c.is_enabled("base"), c.is_requested("base"))
    return m, defaults, c, checks
M, DEFAULTS, C, CHECKS = run()
"#).unwrap();
    assert_eq!(
        module.get("DEFAULTS").unwrap().value().to_repr(),
        "[\"base\", \"c++-compile\"]"
    );
    assert_eq!(
        module.get("CHECKS").unwrap().value().to_repr(),
        "(True, False, True, True, False)"
    );
    assert!(config(&module, "C").is_requested("unknown"));
    use slug_build_api_v2::cc_features::CcExpandable;
    use slug_build_api_v2::cc_features::CcStringChunk;
    use slug_build_api_v2::cc_features::CcToolPathOrigin;
    let payload = model(&module, "M");
    let metadata = payload.model();
    let tool = &metadata.action_configs[0].tools[0];
    assert_eq!(tool.path(), "../bin/cc");
    assert_eq!(tool.origin(), CcToolPathOrigin::CrosstoolPackage);
    assert_eq!(
        tool.execution_requirements
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        ["local"]
    );
    assert!(
        tool.with_features
            .iter()
            .next()
            .unwrap()
            .features
            .contains("base")
    );
    assert!(
        tool.with_features
            .iter()
            .next()
            .unwrap()
            .not_features
            .contains("other")
    );
    assert_eq!(
        metadata.action_configs[0].flag_sets[0]
            .actions
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        ["c++-compile"]
    );
    assert_eq!(
        metadata.artifact_name_patterns[0].category,
        "static_library"
    );
    assert_eq!(metadata.artifact_name_patterns[0].prefix, "custom");
    assert_eq!(metadata.artifact_name_patterns[0].extension, ".a");
    let env = &metadata.features[0].env_sets[0];
    assert!(env.actions.contains("c++-compile"));
    assert_eq!(env.env_entries[0].key, "OUT");
    assert_eq!(
        env.env_entries[0].value.as_ref(),
        &[CcStringChunk::Variable("output".into())]
    );
    assert_eq!(
        env.env_entries[0].expand_if_available.as_deref(),
        Some("output")
    );
    assert!(
        env.with_features
            .iter()
            .next()
            .unwrap()
            .not_features
            .contains("other")
    );
    let flags = &metadata.features[0].flag_sets[0];
    assert!(flags.actions.contains("c++-compile"));
    assert!(
        flags
            .with_features
            .iter()
            .next()
            .unwrap()
            .features
            .contains("base")
    );
    let group = &flags.flag_groups[0];
    assert_eq!(
        (
            group.iterate_over.as_deref(),
            group.expand_if_available.as_deref(),
            group.expand_if_not_available.as_deref(),
            group.expand_if_true.as_deref(),
            group.expand_if_false.as_deref()
        ),
        (
            Some("items"),
            Some("available"),
            Some("absent"),
            Some("yes"),
            Some("no")
        )
    );
    let CcExpandable::Group(nested) = &group.expandables[0] else {
        panic!("missing nested group")
    };
    assert_eq!(nested.expand_if_equal.as_ref().unwrap().name, "mode");
    assert_eq!(nested.expand_if_equal.as_ref().unwrap().value, "yes");
    assert_eq!(
        nested.expandables[0],
        CcExpandable::Flag(
            vec![
                CcStringChunk::Literal("-I".into()),
                CcStringChunk::Variable("include".into())
            ]
            .into()
        )
    );
    assert_eq!(
        nested.expandables[1],
        CcExpandable::Flag(vec![CcStringChunk::Literal("%literal".into())].into())
    );
    assert_eq!(
        nested.expandables[2],
        CcExpandable::Flag(vec![CcStringChunk::RelativePath("bin/cc".into())].into())
    );
    assert!(metadata.features[1].requires[0].contains("base"));
    // Inactive nested metadata must remain part of structural publication,
    // independently of the selected flags (expansion is not admitted here).
    let changed = boundary("M = make([feature(name = 'base', enabled = True)])").unwrap();
    assert_ne!(payload.model(), model(&changed, "M").model());
}

#[test]
fn native_parser_rejects_inactive_invalid_nested_metadata() {
    let cases = [
        (
            "M = make([struct(type_name='wrong', name='x', enabled=False)])",
            "type 'feature'",
        ),
        (
            "M = make([struct(type_name='feature', name='UPPER', enabled=False)])",
            "lowercase ASCII",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled='yes')])",
            "bool",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled=False, implies=[1])])",
            "strings",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled=False, env_sets=[struct(type_name='env_set', actions=[])])])",
            "nonempty",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled=False, flag_sets=[struct(type_name='flag_set', actions=['compile'], flag_groups=[struct(type_name='flag_group', flags=['%bad'])])])])",
            "expected '{'",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled=False, flag_sets=[struct(type_name='flag_set', actions=['compile'], flag_groups=[struct(type_name='flag_group', flags=['ok'])])])])",
            "iterate_over",
        ),
        (
            "M = make([struct(type_name='feature', name='x', enabled=False, flag_sets=[struct(type_name='flag_set')])])",
            "nonempty 'actions'",
        ),
        (
            "M = make(actions=[struct(type_name='action_config', action_name='compile', enabled=False, tools=[struct(type_name='tool', path='.')])])",
            "nonempty",
        ),
        (
            "M = make(actions=[struct(type_name='action_config', action_name='compile', enabled=False, tools=[struct(type_name='tool', path='x', tool='bad')])])",
            "File",
        ),
        (
            "M = make(actions=[struct(type_name='action_config', action_name='compile', enabled=False, flag_sets=[struct(type_name='flag_set', actions=['compile'])])])",
            "must not specify actions",
        ),
        ("M = make(os='WINDOWS')", "unsupported"),
        ("M = make(os='UNKNOWN')", "unsupported"),
        ("M = make(os='linux')", "unsupported"),
        (
            "M = make(patterns=[struct(type_name='artifact_name_pattern',category_name='static_library',prefix='',extension='.bad')])",
            "extension",
        ),
        (
            "M = internal.cc_toolchain_features(toolchain_config_info=42,tools_directory='')",
            "StarlarkInfo",
        ),
    ];
    for (source, expected) in cases {
        let error = boundary(source).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "{source}\nexpected {expected:?}, got {error}"
        );
    }
}

#[test]
fn native_binders_enforce_keyword_list_and_positional_membership() {
    for source in [
        "M = internal.cc_toolchain_features(info(), 'tools')",
        "M = internal.cc_toolchain_features(toolchain_config_info=info())",
        "def run(): return make().configure_features(())\nC=run()",
        "def run(): return make().configure_features([1])\nC=run()",
        "def run(): return make().configure_features([]).is_enabled(feature='x')\nC=run()",
        "def run(): return make().default_features_and_action_configs(1)\nC=run()",
        "def run():\n    defaults=make([feature(name='a',enabled=True)]).default_features_and_action_configs()\n    defaults.append('b')\nC=run()",
        "def run():\n    defaults=make([feature(name='a',enabled=True)]).default_features_and_action_configs()\n    defaults[0]='b'\nC=run()",
    ] {
        assert!(boundary(source).is_err(), "unexpected success: {source}");
    }
    let module = boundary(
        "def run(): return make().configure_features(requested_features=['unknown'])\nC=run()",
    )
    .unwrap();
    assert!(config(&module, "C").is_requested("unknown"));
}

#[test]
fn artifact_payload_and_inactive_metadata_are_owned() {
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionOutputKind;
    use slug_build_api_v2::AnalysisArtifact;
    use slug_build_api_v2::AnalysisConfiguredTargetKey;

    use crate::subrule_invocation::AnalysisArtifactValue;
    let artifact = AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@tools+//cc:compiler").unwrap(),
            Arc::<[u8]>::from(&b"configuration"[..]),
        ),
        output: ActionOutput::new(
            crate::artifact_root::test_root(),
            "cc/compiler",
            ActionOutputKind::File,
        ),
    };
    let module = Module::new();
    module.import_public_symbols(&library());
    module.set(
        "FILE",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(artifact.clone())),
    );
    eval_in(&module, &format!("{SETUP}\nM=make(actions=[action_config(action_name='compile',tools=[tool(tool=FILE)])])"), "@@rules_cc+//cc/private:feature_test.bzl").unwrap();
    let frozen = module.freeze().unwrap();
    let payload = model(&frozen, "M");
    assert_eq!(
        payload.model().action_configs[0].tools[0].artifact(),
        Some(&artifact)
    );
    drop(frozen);
    assert_eq!(
        payload.model().action_configs[0].tools[0].artifact(),
        Some(&artifact)
    );
    let source = |flag: &str, environment: &str, condition: &str| {
        format!(
            "M=make([feature(name='inactive',flag_sets=[flag_set(actions=['compile'],flag_groups=[flag_group(flags=['{flag}'],expand_if_available='{condition}')])],env_sets=[env_set(actions=['compile'],env_entries=[env_entry('KEY','{environment}')])])])"
        )
    };
    let baseline = boundary(&source("one", "value", "condition")).unwrap();
    for changed in [
        source("two", "value", "condition"),
        source("one", "changed", "condition"),
        source("one", "value", "other"),
    ] {
        let changed = boundary(&changed).unwrap();
        assert_ne!(model(&baseline, "M").model(), model(&changed, "M").model());
    }
    let snapshot = boundary(
        r#"
def snapshot():
    flags=['original']
    groups=[flag_group(flags=flags)]
    features=[feature(name='snapshot',flag_sets=[flag_set(actions=['compile'],flag_groups=groups)])]
    m=make(features)
    flags[0]='changed'
    groups.clear()
    features.clear()
    return m
M=snapshot()
"#,
    )
    .unwrap();
    use slug_build_api_v2::cc_features::CcExpandable;
    use slug_build_api_v2::cc_features::CcStringChunk;
    assert_eq!(
        model(&snapshot, "M").model().features[0].flag_sets[0].flag_groups[0].expandables[0],
        CcExpandable::Flag(vec![CcStringChunk::Literal("original".into())].into())
    );
}

#[test]
fn private_checks_follow_invoking_function_and_wrappers_remain_unhashable() {
    let producer =
        boundary("M=make()\ndef configure(): return M.configure_features([])\nC=configure()")
            .unwrap();
    let module = Module::new();
    module.import_public_symbols(&producer);
    let denied = "def denied(): return M.default_features_and_action_configs()\nX=denied()";
    let error = eval_in(&module, denied, "@@//ordinary:consumer.bzl")
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot use private API"), "{error}");
    for name in ["M", "C"] {
        let owned = producer.get(name).unwrap();
        let value = owned.value();
        assert!(value.write_hash(&mut StarlarkHasher::new()).is_err());
        assert!(!starlark_structural_is_immutable(value));
        let module = Module::new();
        module.set("value", value);
        assert!(eval_hash_expression(&module, "X = {value: 1}").is_err());
        assert!(eval_hash_expression(&module, "X = depset([value])").is_err());
    }
    let module = boundary(
        r#"
def run():
    a=make()
    b=make()
    c=a.configure_features([])
    d=a.configure_features([])
    return a, c, (a == a, a == b, c == c, c == d), type(a), type(c)
M,C,EQUAL,T1,T2=run()
STR=str(C)
REPR=repr(C)
"#,
    )
    .unwrap();
    assert_eq!(
        module.get("EQUAL").unwrap().value().to_repr(),
        "(True, False, True, False)"
    );
    assert_eq!(
        module.get("T1").unwrap().value().unpack_str(),
        Some("CcToolchainFeatures")
    );
    assert_eq!(
        module.get("T2").unwrap().value().unpack_str(),
        Some("FeatureConfiguration")
    );
    assert_eq!(
        module.get("STR").unwrap().value().unpack_str(),
        Some("<FeatureConfiguration(ENABLED:;REQUESTED:)>")
    );
    assert_eq!(
        module.get("REPR").unwrap().value().unpack_str(),
        Some(
            "<unknown object com.google.devtools.build.lib.rules.cpp.FeatureConfigurationForStarlark>"
        )
    );
}

#[test]
fn direct_and_containing_hash_barriers_before_and_after_freeze() {
    let module = Module::new();
    module.import_public_symbols(&library());
    eval_in(&module, &format!("{SETUP}\nM=make()\ndef configure(): return M.configure_features([])\nC=configure()\nHolder=provider(fields=['value'])\nPM=Holder(value=M)\nPC=Holder(value=C)\nSM=struct(value=M)\nSC=struct(value=C)"), "@@rules_cc+//cc/private:feature_test.bzl").unwrap();
    for name in ["M", "C", "PM", "PC", "SM", "SC"] {
        let value = module.get(name).unwrap();
        assert!(!starlark_structural_is_immutable(value), "{name}");
        assert!(
            value.write_hash(&mut StarlarkHasher::new()).is_err(),
            "{name}"
        );
        for expression in [format!("X={{{name}:1}}"), format!("X=depset([{name}])")] {
            assert!(
                eval_hash_expression(&module, &expression).is_err(),
                "{expression}"
            );
        }
    }
    let frozen = module.freeze().unwrap();
    let consumer = Module::new();
    consumer.import_public_symbols(&frozen);
    for name in ["M", "C", "PM", "PC", "SM", "SC"] {
        for expression in [format!("X={{{name}:1}}"), format!("X=depset([{name}])")] {
            assert!(
                eval_hash_expression(&consumer, &expression).is_err(),
                "frozen {expression}"
            );
        }
    }
}
