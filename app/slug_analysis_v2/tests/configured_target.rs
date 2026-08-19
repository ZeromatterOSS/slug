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
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionExecGroup;
use slug_analysis_v2::ConfiguredActionExecutionState as State;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredActionPlatformConstraint;
use slug_analysis_v2::ConfiguredActionToolchainContext;
use slug_analysis_v2::ConfiguredEdge;
use slug_analysis_v2::ConfiguredEdgeKind;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::DiagnosticSeverity;
use slug_analysis_v2::PlatformSemanticFact;
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

fn default_action_context(
    owner: &ConfiguredTargetKey,
    platform_label: &str,
) -> (Arc<ConfiguredActionOwnerContext>, ToolchainTopology) {
    let platform = ConfiguredTargetKey::new(
        canonical(platform_label),
        structural_configurations()[1].clone(),
    );
    let selection = ToolchainSelection::new(
        platform.clone(),
        canonical("@@//:toolchain"),
        ConfiguredTargetKey::new(canonical("@@//:type"), owner.configuration().clone()),
        ConfiguredTargetKey::new(
            canonical("@@//:implementation"),
            owner.configuration().clone(),
        ),
    );
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredActionExecGroup::Default,
        platform.clone(),
        PlatformSemanticFact {
            exec_properties: Arc::from([]),
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        Vec::new(),
        Some(Arc::new(ConfiguredActionToolchainContext::new(
            selection.clone(),
            "marker".into(),
        ))),
        ConfiguredActionAspectProvenance::Absent,
    )
    .unwrap();
    (
        Arc::new(context),
        ToolchainTopology::new(vec![platform], Some(selection)).unwrap(),
    )
}

fn action_context(
    owner: &ConfiguredTargetKey,
    group: ConfiguredActionExecGroup,
    platform: ConfiguredTargetKey,
    platform_properties: &[(&str, &str)],
    target_properties: &[(&str, &str)],
    group_properties: &[(&str, &str)],
    marker: &str,
    constraints: Vec<ConfiguredActionPlatformConstraint>,
) -> Result<Arc<ConfiguredActionOwnerContext>, String> {
    let properties = |entries: &[(&str, &str)]| {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>()
    };
    let selection = ToolchainSelection::new(
        platform.clone(),
        canonical("@@//:toolchain"),
        ConfiguredTargetKey::new(canonical("@@//:type"), owner.configuration().clone()),
        ConfiguredTargetKey::new(
            canonical("@@//:implementation"),
            owner.configuration().clone(),
        ),
    );
    ConfiguredActionOwnerContext::new(
        owner.clone(),
        group,
        platform,
        PlatformSemanticFact {
            exec_properties: platform_properties
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .collect::<Vec<_>>()
                .into(),
        },
        &properties(target_properties),
        &properties(group_properties),
        constraints,
        Some(Arc::new(ConfiguredActionToolchainContext::new(
            selection,
            marker.into(),
        ))),
        ConfiguredActionAspectProvenance::Absent,
    )
    .map(Arc::new)
}

fn file_write_result(
    configuration: ConfigurationKey,
    platform_label: &str,
    content: &str,
    output_path: &str,
) -> ConfiguredNodeResult {
    let owner = ConfiguredTargetKey::new(canonical("@@//:probe"), configuration.clone());
    let (context, topology) = default_action_context(&owner, platform_label);
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    ConfiguredNodeResult::new_rule(owner, providers, None)
        .with_action_specs(
            vec![ActionSpec::new(
                ActionKind::Write {
                    content: content.to_owned(),
                    is_executable: false,
                },
                "FileWrite",
                vec![ActionOutput::new(output_path, ActionOutputKind::File)],
            )],
            vec![context],
        )
        .unwrap()
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

    let owner = ConfiguredTargetKey::new(canonical("@@//pkg:custom"), target_config());
    let (context, _) = default_action_context(&owner, "@@//:platform");
    let result = ConfiguredNodeResult::new_rule(owner, providers, None)
        .with_action_specs(
            vec![ActionSpec::new(
                ActionKind::Write {
                    content: "out".to_owned(),
                    is_executable: false,
                },
                "FileWrite",
                vec![ActionOutput::new("pkg/out.txt", ActionOutputKind::File)],
            )],
            vec![context],
        )
        .unwrap()
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
    assert_eq!(baseline.exec_group(), &ConfiguredActionExecGroup::Default);
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
fn configured_actions_share_group_contexts_merge_properties_and_reject_mismatches() {
    let owner = ConfiguredTargetKey::new(
        canonical("@@//:probe"),
        structural_configurations()[0].clone(),
    );
    let exec = structural_configurations()[1].clone();
    let platform = |label: &str| ConfiguredTargetKey::new(canonical(label), exec.clone());
    let constraint = ConfiguredActionPlatformConstraint::new(
        ConfiguredTargetKey::new(canonical("@@//:linux"), exec.clone()),
        ConfiguredTargetKey::new(canonical("@@//:os"), exec.clone()),
    );
    let default = action_context(
        &owner,
        ConfiguredActionExecGroup::Default,
        platform("@@//:p0"),
        &[("a", "platform"), ("z", "platform")],
        &[("a", "target"), ("b", "target")],
        &[("a", "default"), ("c", "default")],
        "marker-A",
        vec![constraint.clone()],
    )
    .unwrap();
    let named = action_context(
        &owner,
        ConfiguredActionExecGroup::Named("named".into()),
        platform("@@//:p1"),
        &[("a", "platform")],
        &[("a", "target")],
        &[("a", "named")],
        "marker-B",
        vec![constraint],
    )
    .unwrap();
    let spec = |path: &str| {
        ActionSpec::new(
            ActionKind::Write {
                content: path.to_owned(),
                is_executable: false,
            },
            "FileWrite",
            vec![ActionOutput::new(path, ActionOutputKind::File)],
        )
    };
    let providers =
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap();
    let result = ConfiguredNodeResult::new_rule(owner.clone(), providers.clone(), None)
        .with_action_specs(
            vec![spec("a"), spec("b"), spec("c").with_exec_group("named")],
            vec![default.clone(), named.clone()],
        )
        .unwrap();
    assert_eq!(
        result
            .actions()
            .iter()
            .map(|row| row.outputs()[0].path())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert!(Arc::ptr_eq(
        result.actions()[0].context(),
        result.actions()[1].context()
    ));
    assert!(Arc::ptr_eq(result.actions()[0].context(), &default));
    assert!(Arc::ptr_eq(result.actions()[2].context(), &named));
    assert_eq!(
        default.platform_fact().unwrap().exec_properties.as_ref(),
        &[
            ("a".into(), "default".into()),
            ("b".into(), "target".into()),
            ("c".into(), "default".into()),
            ("z".into(), "platform".into()),
        ]
    );
    assert_eq!(default.toolchain().unwrap().marker(), "marker-A");
    assert_eq!(named.toolchain().unwrap().marker(), "marker-B");
    assert_eq!(default.execution_state(), State::SelectedToolchain);
    assert_ne!(default, named);

    let platform_only = Arc::new(
        ConfiguredActionOwnerContext::new(
            owner.clone(),
            ConfiguredActionExecGroup::Default,
            platform("@@//:p0"),
            PlatformSemanticFact {
                exec_properties: Arc::from([]),
            },
            &BTreeMap::new(),
            &BTreeMap::new(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap(),
    );
    assert_eq!(platform_only.execution_state(), State::SelectedPlatformOnly);
    assert!(platform_only.toolchain().is_none());

    let restored = action_context(
        &owner,
        ConfiguredActionExecGroup::Default,
        platform("@@//:p0"),
        &[("a", "platform"), ("z", "platform")],
        &[("a", "target"), ("b", "target")],
        &[("a", "default"), ("c", "default")],
        "marker-A",
        vec![ConfiguredActionPlatformConstraint::new(
            ConfiguredTargetKey::new(canonical("@@//:linux"), exec.clone()),
            ConfiguredTargetKey::new(canonical("@@//:os"), exec.clone()),
        )],
    )
    .unwrap();
    assert_eq!(default, restored);
    assert_ne!(
        default,
        action_context(
            &owner,
            ConfiguredActionExecGroup::Default,
            platform("@@//:p0"),
            &[("a", "platform"), ("z", "platform")],
            &[],
            &[("a", "edited")],
            "marker-A",
            Vec::new(),
        )
        .unwrap()
    );

    let wrong_owner =
        ConfiguredTargetKey::new(canonical("@@//:other"), owner.configuration().clone());
    let wrong_context = action_context(
        &wrong_owner,
        ConfiguredActionExecGroup::Default,
        platform("@@//:p0"),
        &[],
        &[],
        &[],
        "marker",
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        ConfiguredNodeResult::new_rule(owner.clone(), providers.clone(), None)
            .with_action_specs(vec![spec("out")], vec![wrong_context])
            .unwrap_err(),
        "configured action context has mismatched owner"
    );
    assert_eq!(
        ConfiguredNodeResult::new_rule(owner.clone(), providers.clone(), None)
            .with_action_specs(vec![spec("out")], vec![default.clone(), default.clone()])
            .unwrap_err(),
        "configured action contexts contain duplicate group"
    );
    assert!(
        action_context(
            &owner,
            ConfiguredActionExecGroup::Default,
            ConfiguredTargetKey::new(canonical("@@//:bad"), target_config()),
            &[],
            &[],
            &[],
            "marker",
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        action_context(
            &owner,
            ConfiguredActionExecGroup::Default,
            platform("@@//:p0"),
            &[("z", "last"), ("a", "first")],
            &[],
            &[],
            "marker",
            Vec::new(),
        )
        .is_err()
    );
    let bad_constraint = ConfiguredActionPlatformConstraint::new(
        ConfiguredTargetKey::new(canonical("@@//:linux"), target_config()),
        ConfiguredTargetKey::new(canonical("@@//:os"), target_config()),
    );
    assert!(
        action_context(
            &owner,
            ConfiguredActionExecGroup::Default,
            platform("@@//:p0"),
            &[],
            &[],
            &[],
            "marker",
            vec![bad_constraint],
        )
        .is_err()
    );
}

#[test]
fn configured_file_write_view_uses_retained_context_and_rejects_shapes() {
    let c0 = structural_configurations()[0].clone();
    let baseline = file_write_result(c0, "@@//:p0", "content-A", "path-A.txt");
    let action = ActionSpec::clone(&baseline.actions()[0]);
    let context = baseline.actions()[0].context().clone();
    let empty = ConfiguredNodeResult::new_rule(
        baseline.configured_target_key().unwrap().clone(),
        baseline.providers().clone(),
        None,
    );
    assert_eq!(empty.configured_file_write_actions().unwrap().len(), 0);
    let unresolved = Arc::new(
        ConfiguredActionOwnerContext::unresolved_default(
            baseline.configured_target_key().unwrap().clone(),
        )
        .unwrap(),
    );
    assert_eq!(unresolved.execution_state(), State::UnresolvedDefault);
    let unresolved = empty
        .clone()
        .with_action_specs(vec![action.clone()], vec![unresolved])
        .unwrap();
    assert!(unresolved.configured_file_write_actions().is_err());
    assert_eq!(
        empty
            .clone()
            .with_action_specs(vec![action.clone()], Vec::new())
            .unwrap_err(),
        "configured action has no matching exec-group context"
    );

    let unrelated = ConfiguredTargetKey::new(
        canonical("@@//:unrelated"),
        structural_configurations()[1].clone(),
    );
    let retained = baseline
        .clone()
        .with_toolchain_topology(ToolchainTopology::new(vec![unrelated], None).unwrap());
    assert_eq!(
        only_file_write(&retained).execution_platform().label(),
        &canonical("@@//:p0")
    );

    let unsupported_shapes = vec![
        ActionSpec::new(ActionKind::Run, "Spawn", action.outputs().to_vec()),
        ActionSpec::new(
            action.kind().clone(),
            "FileWrite",
            vec![ActionOutput::new("tree", ActionOutputKind::Directory)],
        ),
    ];
    for unsupported in unsupported_shapes {
        let result = baseline
            .clone()
            .with_action_specs(vec![unsupported], vec![context.clone()])
            .unwrap();
        assert!(result.configured_file_write_actions().is_err());
    }
    assert_eq!(
        baseline
            .clone()
            .with_action_specs(
                vec![action.clone().with_exec_group("named")],
                vec![context.clone()],
            )
            .unwrap_err(),
        "configured action has no matching exec-group context"
    );

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
        let result = baseline
            .clone()
            .with_action_specs(vec![unsupported], vec![context.clone()])
            .unwrap();
        assert_eq!(
            result.configured_file_write_actions().err(),
            Some("configured FileWrite action has unsupported execution fields")
        );
    }
}
