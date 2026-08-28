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

use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::PlatformInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderError;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderName;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RunEnvironmentInfo;

fn files(items: &[&str]) -> Depset<String> {
    Depset::from_direct(
        DepsetOrder::Default,
        items.iter().map(|item| item.to_string()).collect(),
    )
    .unwrap()
}

fn user_provider(
    id: ProviderId,
    fields: impl IntoIterator<Item = (impl Into<compact_str::CompactString>, AnalysisValue)>,
) -> ProviderValue {
    ProviderValue::Occurrence(ProviderOccurrence::new(ProviderIdentity::user(id), fields))
}

#[test]
fn provider_collection_requires_default_info_for_rules() {
    let err = ProviderCollection::new(vec![user_provider(
        ProviderId::unqualified("MyInfo").unwrap(),
        std::iter::empty::<(String, AnalysisValue)>(),
    )])
    .unwrap_err();

    assert_eq!(err, ProviderError::MissingDefaultInfo);
    assert_eq!(
        err.to_string(),
        "collection did not receive a `DefaultInfo` provider"
    );
}

#[test]
fn provider_collection_rejects_duplicate_provider_keys() {
    let duplicate = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
    ])
    .unwrap_err();

    assert_eq!(
        duplicate,
        ProviderError::DuplicateProvider {
            name: ProviderName::new("MyInfo").unwrap()
        }
    );
    assert_eq!(duplicate.to_string(), "provider MyInfo specified twice");
}

#[test]
fn provider_collection_exposes_bazel_native_and_user_providers() {
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(files(&["pkg/custom.txt"]))),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            [("value", AnalysisValue::string("custom"))],
        ),
        ProviderValue::RunEnvironmentInfo(RunEnvironmentInfo::empty()),
        ProviderValue::PlatformInfo(PlatformInfo::new("@platforms//host:host")),
    ])
    .unwrap();

    let names = collection
        .names()
        .map(|name| name.as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "DefaultInfo".to_owned(),
            "MyInfo".to_owned(),
            "RunEnvironmentInfo".to_owned(),
            "PlatformInfo".to_owned(),
        ]
    );
    assert!(collection.contains(&ProviderIdentity::user(
        ProviderId::unqualified("MyInfo").unwrap()
    )));
    assert_eq!(
        collection.default_info().unwrap().files.to_list(),
        vec!["pkg/custom.txt".to_owned()]
    );
}

#[test]
fn executable_default_info_uses_the_executable_for_implicit_files_and_runfiles() {
    let info = DefaultInfo::from_executable("pkg/tool".to_owned(), None);

    assert_eq!(info.files.to_list(), ["pkg/tool"]);
    assert_eq!(info.executable.as_deref(), Some("pkg/tool"));
    assert_eq!(info.files_to_run.executable.as_deref(), Some("pkg/tool"));
    assert_eq!(info.default_runfiles.files.to_list(), ["pkg/tool"]);
    assert_eq!(info.data_runfiles.files.to_list(), ["pkg/tool"]);
    assert!(info.files_to_run.runfiles_manifest.is_none());
    assert!(info.files_to_run.repo_mapping_manifest.is_none());
    assert!(info.default_runfiles.symlinks.is_empty());
    assert!(info.data_runfiles.symlinks.is_empty());
}

#[test]
fn executable_default_info_preserves_an_explicit_files_override() {
    let info =
        DefaultInfo::from_executable("pkg/tool".to_owned(), Some(files(&["pkg/explicit.txt"])));

    assert_eq!(info.files.to_list(), ["pkg/explicit.txt"]);
    assert_eq!(info.default_runfiles.files.to_list(), ["pkg/tool"]);
    assert_eq!(info.data_runfiles.files.to_list(), ["pkg/tool"]);
}

#[test]
fn provider_collection_looks_up_user_providers_by_structural_export_identity() {
    let constructor_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let independently_reconstructed_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(constructor_id, [("value", AnalysisValue::string("custom"))]),
    ])
    .unwrap();

    assert_eq!(collection.len(), 2);
    assert_eq!(
        collection
            .user(&independently_reconstructed_id)
            .unwrap()
            .field("value"),
        Some(&AnalysisValue::string("custom"))
    );
    assert!(
        collection
            .user(&ProviderId::new("//other:defs.bzl", "MyInfo").unwrap())
            .is_none()
    );
}

#[test]
fn toolchain_info_uses_builtin_identity_not_a_user_provider_name() {
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(
            ProviderId::unqualified("DefaultInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        user_provider(
            ProviderId::unqualified("ToolchainInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string("selected"))],
        )),
    ])
    .unwrap();

    assert_eq!(collection.len(), 4);
    assert_eq!(collection.default_info(), Some(&DefaultInfo::empty()));
    assert_eq!(
        collection.toolchain_info().unwrap().field("marker"),
        Some(&AnalysisValue::string("selected"))
    );
    assert!(
        collection
            .user(&ProviderId::unqualified("ToolchainInfo").unwrap())
            .is_some()
    );
    assert!(
        collection
            .user(&ProviderId::unqualified("DefaultInfo").unwrap())
            .is_some()
    );
}

#[test]
fn output_group_info_keeps_named_file_depsets() {
    let mut groups = BTreeMap::new();
    groups.insert("validation".to_owned(), files(&["pkg/validation.txt"]));
    groups.insert("hidden_top_level".to_owned(), files(&["pkg/hidden.txt"]));

    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::OutputGroupInfo(OutputGroupInfo::new(groups)),
    ])
    .unwrap();

    match collection
        .get(&ProviderIdentity::builtin("OutputGroupInfo"))
        .unwrap()
    {
        ProviderValue::OutputGroupInfo(info) => {
            assert_eq!(
                info.groups["hidden_top_level"].to_list(),
                vec!["pkg/hidden.txt".to_owned()]
            );
            assert_eq!(
                info.groups["validation"].to_list(),
                vec!["pkg/validation.txt".to_owned()]
            );
        }
        other => panic!("expected OutputGroupInfo, got {other:?}"),
    }
}
