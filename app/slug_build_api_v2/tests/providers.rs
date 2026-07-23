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

use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::PlatformInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderError;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderName;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RunEnvironmentInfo;
use slug_build_api_v2::UserProvider;

fn files(items: &[&str]) -> Depset<String> {
    Depset::from_direct(
        DepsetOrder::Default,
        items.iter().map(|item| item.to_string()).collect(),
    )
    .unwrap()
}

#[test]
fn provider_collection_requires_default_info_for_rules() {
    let err = ProviderCollection::new(vec![ProviderValue::User(
        UserProvider::new("MyInfo", BTreeMap::new()).unwrap(),
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
        ProviderValue::User(UserProvider::new("MyInfo", BTreeMap::new()).unwrap()),
        ProviderValue::User(UserProvider::new("MyInfo", BTreeMap::new()).unwrap()),
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
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), "custom".to_owned());

    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::from_files(files(&["pkg/custom.txt"]))),
        ProviderValue::User(UserProvider::new("MyInfo", fields).unwrap()),
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
    assert!(collection.contains(&ProviderName::new("MyInfo").unwrap()));
    assert_eq!(
        collection.default_info().unwrap().files.to_list(),
        vec!["pkg/custom.txt".to_owned()]
    );
}

#[test]
fn provider_collection_looks_up_user_providers_by_structural_export_identity() {
    let constructor_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let independently_reconstructed_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::User(
            UserProvider::with_id(constructor_id, [("value".to_owned(), "custom".to_owned())])
                .unwrap(),
        ),
    ])
    .unwrap();

    assert_eq!(
        collection
            .user(&independently_reconstructed_id)
            .unwrap()
            .field("value"),
        Some("custom")
    );
    assert!(
        collection
            .user(&ProviderId::new("//other:defs.bzl", "MyInfo").unwrap())
            .is_none()
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
        .get(&ProviderName::new("OutputGroupInfo").unwrap())
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
