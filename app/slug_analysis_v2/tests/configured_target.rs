/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::sync::Arc;

use slug_analysis_v2::AnalysisDiagnostic;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionExecGroup;
use slug_analysis_v2::ConfiguredEdge;
use slug_analysis_v2::ConfiguredEdgeKind;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::DiagnosticSeverity;
use slug_analysis_v2::ToolchainSelection;
use slug_analysis_v2::ToolchainTopology;
use slug_analysis_v2::key::RootStringSettingValue;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ParamFile;
use slug_build_api_v2::ParamFileFormat;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::UserProvider;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::TestRuleKind;
use slug_workspace_v2::NormalizedAbsolutePath;

fn target_config() -> ConfigurationKey {
    ConfigurationKey::target("targetabc").unwrap()
}

fn structural_configurations() -> [ConfigurationKey; 3] {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    [
        SlugConfiguration::default_target(&host).unwrap(),
        SlugConfiguration::default_exec(&host).unwrap(),
        SlugConfiguration::default_host_like(&host).unwrap(),
    ]
    .map(ConfigurationKey::from_slug)
}

fn canonical(value: &str) -> CanonicalLabel {
    CanonicalLabel::parse(value).unwrap()
}

fn mapped_label(mapping_name: &str, repo_version: &str) -> CanonicalLabel {
    let apparent = ApparentLabel::parse("@dep//pkg:target").unwrap();
    let mut mapping = RepositoryMapping::new(RepositoryMappingId::new(mapping_name).unwrap());
    mapping.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new(repo_version).unwrap(),
    );
    apparent.resolve(&mapping)
}

fn file_write_result(
    configuration: ConfigurationKey,
    platform_label: &str,
    content: &str,
    output_path: &str,
) -> ConfiguredNodeResult {
    let owner = ConfiguredTargetKey::new(canonical("@@//:probe"), configuration.clone());
    let platform = ConfiguredTargetKey::new(
        canonical(platform_label),
        structural_configurations()[1].clone(),
    );
    let topology = ToolchainTopology::new(
        vec![platform.clone()],
        Some(ToolchainSelection::new(
            platform,
            canonical("@@//:toolchain"),
            ConfiguredTargetKey::new(canonical("@@//:type"), configuration.clone()),
            ConfiguredTargetKey::new(canonical("@@//:implementation"), configuration),
        )),
    )
    .unwrap();
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    ConfiguredNodeResult::new_rule(owner, providers, None)
        .with_actions(vec![ActionSpec::new(
            ActionKind::Write {
                content: content.to_owned(),
                is_executable: false,
            },
            "FileWrite",
            vec![ActionOutput::new(output_path, ActionOutputKind::File)],
        )])
        .with_toolchain_topology(topology)
}

fn only_file_write(result: &ConfiguredNodeResult) -> slug_analysis_v2::ConfiguredActionView<'_> {
    result
        .configured_file_write_actions()
        .unwrap()
        .next()
        .unwrap()
}

#[test]
fn configured_target_key_serializes_label_mapping_and_configuration() {
    let first = ConfiguredTargetKey::new(mapped_label("first", "dep~1.0.0"), target_config());
    let second = ConfiguredTargetKey::new(mapped_label("second", "dep~1.0.0"), target_config());
    let exec = ConfiguredTargetKey::new(
        mapped_label("first", "dep~1.0.0"),
        ConfigurationKey::exec("execabc").unwrap(),
    );

    assert_ne!(first, second);
    assert_ne!(first.stable_serialize(), second.stable_serialize());
    assert_eq!(
        first.stable_serialize(),
        "@@dep~1.0.0//pkg:target@mapping:first [target:targetabc]"
    );
    assert_eq!(
        exec.stable_serialize(),
        "@@dep~1.0.0//pkg:target@mapping:first [exec:execabc]"
    );
}

#[test]
fn configured_node_key_distinguishes_configured_null_and_configuration_kinds() {
    let label = canonical("@@//pkg:target");
    let [target_configuration, exec_configuration, host_configuration] =
        structural_configurations();
    let target = ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
        label.clone(),
        target_configuration,
    ));
    let exec =
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(label.clone(), exec_configuration));
    let host_like =
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(label.clone(), host_configuration));
    let null = ConfiguredNodeKey::null(label);

    assert_ne!(target, exec);
    assert_ne!(target, host_like);
    assert_ne!(exec, host_like);
    assert_ne!(target, null);
    assert!(null.configured_target().is_none());
    assert_eq!(null.label().to_string(), "@@//pkg:target");
}

#[test]
fn configured_analysis_key_rejects_legacy_configuration_identity() {
    let target = ConfiguredTargetKey::new(canonical("@@//pkg:target"), target_config());
    let error =
        ConfiguredNodeAnalysisKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), target)
            .unwrap_err();
    assert_eq!(
        error.to_string(),
        "production configured-node analysis requires a structural Slug configuration"
    );
}

#[test]
fn configured_edge_records_transition_output_and_fixed_bits() {
    let edge = ConfiguredEdge::new(
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
            canonical("@@//dep:lib"),
            ConfigurationKey::exec("execplatform1").unwrap(),
        )),
        ConfiguredEdgeKind::TransitionedAttribute {
            attribute: "deps".into(),
            index: 0,
            output: canonical("@@//settings:exec"),
        },
    );

    let key = edge.configured_target().unwrap();
    assert_eq!(key.label().to_string(), "@@//dep:lib");
    assert_eq!(key.configuration().stable_serialize(), "exec:execplatform1");
    assert_eq!(edge.implicit(), false);
    assert_eq!(edge.tool(), false);
}

#[test]
fn configured_edges_preserve_transition_convergence_order_and_fixed_bits() {
    let target = ConfiguredTargetKey::new(canonical("@@//dep:lib"), target_config());
    let first = ConfiguredEdge::new(
        target.clone().into(),
        ConfiguredEdgeKind::TransitionedAttribute {
            attribute: "left".into(),
            index: 0,
            output: canonical("@@//settings:out"),
        },
    );
    let second = ConfiguredEdge::new(
        target.clone().into(),
        ConfiguredEdgeKind::TransitionedAttribute {
            attribute: "right".into(),
            index: 1,
            output: canonical("@@//settings:out"),
        },
    );
    assert_ne!(first, second);
    assert_eq!(first.configured_target(), second.configured_target());

    let kinds = vec![
        (
            ConfiguredEdgeKind::OrdinaryAttribute {
                attribute: "deps".into(),
                index: 0,
            },
            false,
        ),
        (
            ConfiguredEdgeKind::TransitionedAttribute {
                attribute: "deps".into(),
                index: 1,
                output: canonical("@@//settings:out"),
            },
            false,
        ),
        (ConfiguredEdgeKind::AliasActual, false),
        (ConfiguredEdgeKind::GeneratedBy, false),
        (ConfiguredEdgeKind::Source, false),
        (ConfiguredEdgeKind::DeclaringVisibility, false),
        (ConfiguredEdgeKind::PackageGroupInclude { index: 0 }, true),
        (ConfiguredEdgeKind::ToolchainRequirement, true),
        (ConfiguredEdgeKind::SelectedToolchainImplementation, true),
        (
            ConfiguredEdgeKind::CandidateExecutionPlatform { index: 0 },
            true,
        ),
        (ConfiguredEdgeKind::HostPlatform, true),
        (ConfiguredEdgeKind::PlatformConstraint { index: 0 }, true),
        (ConfiguredEdgeKind::ConstraintSetting, true),
        (ConfiguredEdgeKind::FunctionTransitionAllowlist, true),
    ];
    for (kind, implicit) in kinds {
        let edge = ConfiguredEdge::new(target.clone().into(), kind);
        assert_eq!(edge.implicit(), implicit);
        assert!(!edge.tool());
    }

    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let ordered = ConfiguredNodeResult::new_rule(target.clone(), providers.clone(), None)
        .with_edges(vec![first.clone(), second.clone()]);
    let reordered =
        ConfiguredNodeResult::new_rule(target, providers, None).with_edges(vec![second, first]);
    assert_ne!(ordered, reordered);
    assert_eq!(
        ordered.edges()[0].kind(),
        &ConfiguredEdgeKind::TransitionedAttribute {
            attribute: "left".into(),
            index: 0,
            output: canonical("@@//settings:out")
        }
    );
}

#[test]
fn configured_node_result_keeps_provider_collection_outputs_and_diagnostics() {
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), "custom".to_owned());
    let files = Depset::from_direct(DepsetOrder::Default, vec!["pkg/out.txt".to_owned()]).unwrap();
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(files)),
        ProviderValue::User(UserProvider::new("MyInfo", fields).unwrap()),
    ])
    .unwrap();

    let result = ConfiguredNodeResult::new_rule(
        ConfiguredTargetKey::new(canonical("@@//pkg:custom"), target_config()),
        providers,
        None,
    )
    .with_actions(vec![ActionSpec::new(
        ActionKind::Write {
            content: "out".to_owned(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
    )])
    .with_declared_outputs(vec!["pkg/out.txt".to_owned()])
    .with_diagnostics(vec![AnalysisDiagnostic::new(
        DiagnosticSeverity::Warning,
        "placeholder analysis warning",
    )]);

    assert_eq!(
        result.configured_target_key().unwrap().stable_serialize(),
        "@@//pkg:custom [target:targetabc]"
    );
    assert_eq!(result.actions()[0].mnemonic(), "FileWrite");
    assert_eq!(result.declared_outputs(), &["pkg/out.txt".to_owned()]);
    assert_eq!(
        result.providers().default_info().unwrap().files.to_list(),
        vec!["pkg/out.txt".to_owned()]
    );
    assert_eq!(
        result.diagnostics()[0].severity(),
        DiagnosticSeverity::Warning
    );
    assert_eq!(
        result.diagnostics()[0].message(),
        "placeholder analysis warning"
    );
}

#[test]
fn configured_node_result_capability_is_borrowed_and_participates_in_equality() {
    let key = ConfiguredTargetKey::new(canonical("@@//pkg:custom"), target_config());
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let executable = RuleCapability {
        rule_class: "custom_rule".into(),
        executable: true,
        test_kind: None,
    };
    let test = RuleCapability {
        rule_class: "custom_test".into(),
        executable: true,
        test_kind: Some(TestRuleKind::Test),
    };

    let absent = ConfiguredNodeResult::new_rule(key.clone(), providers.clone(), None);
    let executable_result =
        ConfiguredNodeResult::new_rule(key.clone(), providers.clone(), Some(executable.clone()));
    let renamed = ConfiguredNodeResult::new_rule(
        key.clone(),
        providers.clone(),
        Some(RuleCapability {
            rule_class: "other_rule".into(),
            ..executable.clone()
        }),
    );
    let test_result = ConfiguredNodeResult::new_rule(key, providers, Some(test));

    assert_eq!(executable_result.rule_capability(), Some(&executable));
    assert_ne!(absent, executable_result);
    assert_ne!(executable_result, renamed);
    assert_ne!(executable_result, test_result);
}

#[test]
fn toolchain_topology_is_ordered_role_checked_and_structurally_equal() {
    let candidate = ConfiguredTargetKey::new(
        canonical("@@//:platform"),
        ConfigurationKey::exec("exec").unwrap(),
    );
    let selection = ToolchainSelection::new(
        candidate.clone(),
        canonical("@@//:declaration"),
        ConfiguredTargetKey::new(canonical("@@//:type"), target_config()),
        ConfiguredTargetKey::new(canonical("@@//:implementation"), target_config()),
    );
    let topology = ToolchainTopology::new(vec![candidate.clone()], Some(selection)).unwrap();
    assert_eq!(
        topology.selection().unwrap().execution_platform(),
        &candidate
    );
    assert!(
        ToolchainTopology::new(
            vec![ConfiguredTargetKey::new(
                canonical("@@//:wrong_role"),
                target_config(),
            )],
            None,
        )
        .is_err()
    );

    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let key = ConfiguredTargetKey::new(canonical("@@//:root"), target_config());
    let plain = ConfiguredNodeResult::new_rule(key.clone(), providers.clone(), None);
    let retained = ConfiguredNodeResult::new_rule(key, providers, None)
        .with_toolchain_topology(topology.clone());
    assert_ne!(plain, retained);
    assert_eq!(retained.toolchain_topology(), Some(&topology));
}

#[test]
fn configured_file_write_view_tracks_and_restores_structural_identity() {
    let c0 = structural_configurations()[0].clone();
    let c1 = c0.with_root_string_setting(RootStringSettingValue::new("c1"));
    let baseline = file_write_result(c0.clone(), "@@//:p0", "content-A", "path-A.txt");
    let changed_configuration = file_write_result(c1, "@@//:p0", "content-A", "path-A.txt");
    let changed_platform = file_write_result(c0.clone(), "@@//:p1", "content-A", "path-A.txt");
    let changed_content = file_write_result(c0.clone(), "@@//:p0", "content-B", "path-A.txt");
    let changed_path = file_write_result(c0.clone(), "@@//:p0", "content-A", "path-B.txt");
    let restored = file_write_result(c0, "@@//:p0", "content-A", "path-A.txt");

    let baseline = only_file_write(&baseline);
    assert_eq!(baseline.exec_group(), ConfiguredActionExecGroup::Default);
    assert_eq!(baseline.owner().label(), &canonical("@@//:probe"));
    assert_eq!(baseline.execution_platform().label(), &canonical("@@//:p0"));
    assert_eq!(baseline.output().path(), "path-A.txt");
    assert!(matches!(
        baseline.spec().kind(),
        ActionKind::Write { content, is_executable: false } if content == "content-A"
    ));
    assert_ne!(baseline, only_file_write(&changed_configuration));
    assert_ne!(baseline, only_file_write(&changed_platform));
    assert_ne!(baseline, only_file_write(&changed_content));
    assert_ne!(baseline, only_file_write(&changed_path));
    assert_eq!(baseline, only_file_write(&restored));
}

#[test]
fn configured_file_write_view_rejects_unowned_or_ambiguous_shapes() {
    let c0 = structural_configurations()[0].clone();
    let baseline = file_write_result(c0, "@@//:p0", "content-A", "path-A.txt");
    let action = baseline.actions()[0].clone();
    let without_platform = ConfiguredNodeResult::new_rule(
        baseline.configured_target_key().unwrap().clone(),
        baseline.providers().clone(),
        None,
    )
    .with_actions(vec![action.clone()]);
    let empty = without_platform.clone().with_actions(Vec::new());
    assert_eq!(empty.configured_file_write_actions().unwrap().len(), 0);
    assert_eq!(
        without_platform.configured_file_write_actions().err(),
        Some("configured FileWrite action requires a selected toolchain platform")
    );

    let sole = ConfiguredTargetKey::new(
        canonical("@@//:sole"),
        structural_configurations()[1].clone(),
    );
    let derived = without_platform
        .clone()
        .with_toolchain_topology(ToolchainTopology::new(vec![sole.clone()], None).unwrap());
    assert_eq!(only_file_write(&derived).execution_platform(), &sole);
    for candidates in [
        Vec::new(),
        vec![
            sole,
            ConfiguredTargetKey::new(
                canonical("@@//:other"),
                structural_configurations()[1].clone(),
            ),
        ],
    ] {
        let ambiguous = without_platform
            .clone()
            .with_toolchain_topology(ToolchainTopology::new(candidates, None).unwrap());
        assert_eq!(
            ambiguous.configured_file_write_actions().err(),
            Some("configured FileWrite action requires a selected toolchain platform")
        );
    }

    let unsupported_shapes = vec![
        ActionSpec::new(ActionKind::Run, "Spawn", action.outputs().to_vec()),
        ActionSpec::new(
            action.kind().clone(),
            "FileWrite",
            vec![ActionOutput::new("tree", ActionOutputKind::Directory)],
        ),
        action.clone().with_exec_group("named"),
    ];
    for unsupported in unsupported_shapes {
        let result = baseline.clone().with_actions(vec![unsupported]);
        assert!(result.configured_file_write_actions().is_err());
    }

    let mut field = BTreeMap::new();
    field.insert("key".to_owned(), "value".to_owned());
    let unsupported_execution_fields = vec![
        action.clone().with_argv(vec!["literal".to_owned()]),
        action
            .clone()
            .with_inputs(vec![ActionInput::new("input", None)]),
        action
            .clone()
            .with_tools(vec![ActionInput::new("tool", None)]),
        action.clone().with_param_files(vec![ParamFile::new(
            "params",
            vec!["arg".to_owned()],
            ParamFileFormat::Multiline,
        )]),
        action.clone().with_env(field.clone()),
        action.clone().with_execution_requirements(field.clone()),
        action.clone().with_exec_properties(field),
        action.with_progress_message("writing"),
    ];
    for unsupported in unsupported_execution_fields {
        let result = baseline.clone().with_actions(vec![unsupported]);
        assert_eq!(
            result.configured_file_write_actions().err(),
            Some("configured FileWrite action has unsupported execution fields")
        );
    }
}
