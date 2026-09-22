//! Source regressions for CompileCommandLine76–154 and CppCompileAction979–992.
use super::*;
use crate::AnalysisValue;
use crate::cc_features::*;
use crate::cc_variables::*;

pub(in crate::actions::cpp_compile) fn group(flags: &[&str]) -> CcFlagGroup {
    CcFlagGroup {
        expandables: flags
            .iter()
            .map(|text| CcExpandable::Flag(parse_cc_string(text).unwrap().into()))
            .collect(),
        iterate_over: None,
        expand_if_available: None,
        expand_if_not_available: None,
        expand_if_true: None,
        expand_if_false: None,
        expand_if_equal: None,
    }
}
pub(in crate::actions::cpp_compile) fn configuration(
    groups: Vec<CcFlagGroup>,
    env: &[(&str, &str)],
) -> CcFeatureConfiguration {
    let set = |groups: Vec<CcFlagGroup>| CcFlagSet {
        actions: ["c++-compile".into()].into_iter().collect(),
        with_features: CcSet::new(),
        flag_groups: groups.into(),
    };
    CcToolchainFeatures::new(
        vec![CcFeature {
            name: "probe".into(),
            enabled: true,
            flag_sets: vec![set(groups)].into(),
            env_sets: vec![CcEnvSet {
                actions: ["c++-compile".into()].into_iter().collect(),
                with_features: CcSet::new(),
                env_entries: env
                    .iter()
                    .map(|(key, value)| CcEnvEntry {
                        key: (*key).into(),
                        value: parse_cc_string(value).unwrap().into(),
                        expand_if_available: None,
                    })
                    .collect(),
            }]
            .into(),
            requires: Box::new([]),
            implies: Box::new([]),
            provides: Box::new([]),
        }],
        vec![CcActionConfig {
            action_name: "c++-compile".into(),
            enabled: true,
            tools: vec![
                CcTool::new(
                    Some("cc"),
                    None,
                    CcSet::new(),
                    CcSet::new(),
                    CcExecOs::Linux,
                )
                .unwrap(),
            ]
            .into(),
            flag_sets: vec![set(vec![group(&["--action-first", "", "--same"])])].into(),
            implies: Box::new([]),
        }],
        vec![],
        "tools",
        CcExecOs::Linux,
    )
    .unwrap()
    .configure_features(vec!["c++-compile".into(), "probe".into()])
    .unwrap()
}
fn spec(groups: Vec<CcFlagGroup>, env: &[(&str, &str)]) -> CppCompileSpec {
    let mut data = super::super::tests::data();
    data.construction.feature_configuration = configuration(groups, env);
    CppCompileSpec::new(data).unwrap()
}

#[test]
fn compile_argv_orders_tool_action_and_features_without_shell_or_implicit_flags() {
    let recipe = spec(vec![group(&["--same", "", "a b", "'literal'"])], &[]);
    let before = recipe.clone();
    for _ in 0..2 {
        assert_eq!(
            recipe.expand_command_line().unwrap(),
            [
                "tools/cc",
                "--action-first",
                "",
                "--same",
                "--same",
                "",
                "a b",
                "'literal'"
            ]
        );
        assert_eq!(recipe, before);
    }
}

#[test]
fn compile_argv_uses_fresh_scope_while_environment_preserves_original_seed() {
    let mut data = spec(vec![group(&["%{obj.field}"])], &[("PROBE", "%{obj.field}")])
        .data()
        .clone();
    data.variables = CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        locals: vec![(
            "obj".into(),
            AnalysisValue::strukt([("field", AnalysisValue::string("fresh"))]),
        )]
        .into(),
        cache: vec![(
            "obj.field".into(),
            CcVariableCacheEntry::Value(AnalysisValue::string("cached")),
        )]
        .into(),
    });
    let expected_environment = CanonicalStringMap::from_pairs(
        data.environment
            .fixed()
            .iter()
            .chain([("PWD", "/proc/self/cwd"), ("PROBE", "cached")]),
    );
    let recipe = CppCompileSpec::new(data).unwrap();
    let before = recipe.clone();
    for _ in 0..2 {
        assert_eq!(
            recipe.expand_command_line().unwrap(),
            ["tools/cc", "--action-first", "", "--same", "fresh"]
        );
        assert_eq!(
            recipe
                .effective_environment(&CanonicalStringMap::default())
                .unwrap(),
            expected_environment
        );
        assert_eq!(recipe, before);
    }
}

#[test]
fn compile_expansion_errors_are_lazy_and_do_not_mutate_published_recipe() {
    let mut data = spec(vec![group(&["%{bad}"])], &[("BAD", "%{bad}")])
        .data()
        .clone();
    data.variables = CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        locals: vec![("bad".into(), AnalysisValue::integer(17))].into(),
        cache: Box::new([]),
    });
    let recipe = CppCompileSpec::new(data).unwrap();
    let before = recipe.clone();
    for _ in 0..2 {
        assert!(recipe.expand_command_line().is_err());
        assert!(
            recipe
                .effective_environment(&CanonicalStringMap::default())
                .is_err()
        );
        assert_eq!(recipe, before);
    }
}

fn inherited_environment() -> slug_configuration_v2::RetainedActionEnvironment {
    use std::sync::Arc;

    use slug_configuration_v2::CommandConfigurationOccurrence;
    use slug_configuration_v2::CommandConfigurationOverlay;
    use slug_configuration_v2::NativeCommandOption;
    use slug_configuration_v2::SlugConfiguration;
    use slug_configuration_v2::native::host::ActionEnvironmentHost;
    use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    let base = SlugConfiguration::default_target(&host).unwrap();
    let overlay: CommandConfigurationOverlay =
        ["FIXED=fixed", "INHERITED", "MISSING", "PWD=configured"]
            .into_iter()
            .map(|value| {
                CommandConfigurationOccurrence::native(
                    NativeCommandOption::ActionEnv,
                    Some(value),
                    false,
                )
            })
            .collect::<Vec<_>>()
            .into();
    base.with_command_configuration(base.starlark_options().clone(), &overlay)
        .unwrap()
        .configured_action_environment()
        .unwrap()
}

#[test]
fn compile_environment_resolves_only_declared_client_names_and_native_precedence() {
    let mut data = spec(vec![], &[]).data().clone();
    data.construction.environment = inherited_environment();
    let client = CanonicalStringMap::from_pairs([
        ("FIXED", "client-ignored"),
        ("INHERITED", "client"),
        ("UNDECLARED", "ignored"),
        ("PWD", "client-ignored"),
    ]);
    let mut expected = CanonicalStringMap::from_pairs(
        data.environment
            .fixed()
            .iter()
            .chain([("INHERITED", "client"), ("PWD", "/proc/self/cwd")]),
    );
    let recipe = CppCompileSpec::new(data.clone()).unwrap();
    assert_eq!(recipe.effective_environment(&client).unwrap(), expected);
    assert_eq!(recipe.effective_environment(&client).unwrap(), expected);
    assert_eq!(recipe, CppCompileSpec::new(data.clone()).unwrap());
    for value in ["", "false"] {
        data.construction.execution_requirements =
            CanonicalStringMap::from_pairs([("requires-darwin", value)]);
        expected = CanonicalStringMap::from_pairs(
            data.environment
                .fixed()
                .iter()
                .chain([("INHERITED", "client")]),
        );
        assert_eq!(
            CppCompileSpec::new(data.clone())
                .unwrap()
                .effective_environment(&client)
                .unwrap(),
            expected
        );
    }
    // Feature environment overwrites fixed, inherited and synthesized values.
    data.construction.feature_configuration = configuration(
        vec![],
        &[
            ("FIXED", "feature"),
            ("INHERITED", "feature"),
            ("PWD", "feature"),
        ],
    );
    for darwin in [false, true] {
        data.construction.execution_requirements = if darwin {
            CanonicalStringMap::from_pairs([("requires-darwin", "")])
        } else {
            CanonicalStringMap::default()
        };
        expected = CanonicalStringMap::from_pairs(data.environment.fixed().iter().chain([
            ("FIXED", "feature"),
            ("INHERITED", "feature"),
            ("PWD", "feature"),
        ]));
        assert_eq!(
            CppCompileSpec::new(data.clone())
                .unwrap()
                .effective_environment(&client)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn compile_feature_environment_duplicates_are_errors_not_last_writer_wins() {
    let recipe = spec(vec![], &[("DUP", "a"), ("DUP", "b")]);
    let before = recipe.clone();
    assert!(
        recipe
            .effective_environment(&CanonicalStringMap::from_pairs([("DUP", "client")]))
            .is_err()
    );
    assert_eq!(recipe, before);
}
