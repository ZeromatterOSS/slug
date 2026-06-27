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
use slug_analysis_v2::AnalysisDiceInputs;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredDependency;
use slug_analysis_v2::ConfiguredTargetDiceKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::DiagnosticSeverity;
use slug_analysis_v2::TransitionEdge;
use slug_analysis_v2::TransitionKind;
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
fn dice_key_changes_when_semantic_inputs_change() {
    let target = ConfiguredTargetKey::new(canonical("@@//pkg:target"), target_config());
    let base_inputs = AnalysisDiceInputs::new("cmd1", "settings1", "repos1", "tools1").unwrap();
    let changed_settings =
        AnalysisDiceInputs::new("cmd1", "settings2", "repos1", "tools1").unwrap();
    let changed_repos = AnalysisDiceInputs::new("cmd1", "settings1", "repos2", "tools1").unwrap();

    let base = ConfiguredTargetDiceKey::new(target.clone(), base_inputs);
    let settings = ConfiguredTargetDiceKey::new(target.clone(), changed_settings);
    let repos = ConfiguredTargetDiceKey::new(target, changed_repos);

    assert_ne!(base, settings);
    assert_ne!(base, repos);
    assert!(base.stable_serialize().contains("settings=settings1"));
    assert!(settings.stable_serialize().contains("settings=settings2"));
    assert!(repos.stable_serialize().contains("repos=repos2"));
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
    )
    .with_declared_outputs(vec!["pkg/out.txt".to_owned()])
    .with_diagnostics(vec![AnalysisDiagnostic::new(
        DiagnosticSeverity::Warning,
        "placeholder analysis warning",
    )]);

    assert_eq!(
        result.key().stable_serialize(),
        "@@//pkg:custom [target:targetabc]"
    );
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
