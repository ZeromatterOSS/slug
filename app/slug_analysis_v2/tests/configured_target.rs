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

use slug_analysis_v2::AnalysisDiagnostic;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredDependency;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::DiagnosticSeverity;
use slug_analysis_v2::TransitionEdge;
use slug_analysis_v2::TransitionKind;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::UserProvider;
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
fn configured_dependency_records_transition_output_configuration() {
    let dep = ConfiguredDependency::new(
        canonical("@@//dep:lib"),
        TransitionEdge::new(
            TransitionKind::Exec,
            ConfigurationKey::exec("execplatform1").unwrap(),
        ),
    );

    let key = dep.configured_key();
    assert_eq!(dep.transition().kind(), &TransitionKind::Exec);
    assert_eq!(key.label().to_string(), "@@//dep:lib");
    assert_eq!(key.configuration().stable_serialize(), "exec:execplatform1");
}

#[test]
fn analysis_result_keeps_provider_collection_outputs_and_diagnostics() {
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), "custom".to_owned());
    let files = Depset::from_direct(DepsetOrder::Default, vec!["pkg/out.txt".to_owned()]).unwrap();
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(files)),
        ProviderValue::User(UserProvider::new("MyInfo", fields).unwrap()),
    ])
    .unwrap();

    let result = AnalysisResult::new(
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
        result.key().stable_serialize(),
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
fn analysis_result_capability_is_borrowed_and_participates_in_equality() {
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

    let absent = AnalysisResult::new(key.clone(), providers.clone(), None);
    let executable_result =
        AnalysisResult::new(key.clone(), providers.clone(), Some(executable.clone()));
    let renamed = AnalysisResult::new(
        key.clone(),
        providers.clone(),
        Some(RuleCapability {
            rule_class: "other_rule".into(),
            ..executable.clone()
        }),
    );
    let test_result = AnalysisResult::new(key, providers, Some(test));

    assert_eq!(executable_result.rule_capability(), Some(&executable));
    assert_ne!(absent, executable_result);
    assert_ne!(executable_result, renamed);
    assert_ne!(executable_result, test_result);
}
