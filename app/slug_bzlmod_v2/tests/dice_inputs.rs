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
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceTransaction;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodDiceInputs;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::BzlmodExtensionDefinitionDigest;
use slug_bzlmod_v2::BzlmodExtensionUsageDigest;
use slug_bzlmod_v2::BzlmodGeneratedRepoSpecDigest;
use slug_bzlmod_v2::BzlmodHiddenLockfileDigest;
use slug_bzlmod_v2::BzlmodModuleFileDigest;
use slug_bzlmod_v2::BzlmodRegistryModuleFileDigest;
use slug_bzlmod_v2::BzlmodRegistryPolicyEntry;
use slug_bzlmod_v2::BzlmodRegistrySourceSpecDigest;
use slug_bzlmod_v2::BzlmodRepoMappingDigest;
use slug_bzlmod_v2::BzlmodVisibleLockfileDigest;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ResolvedBzlmodGraphDiceKey;
use slug_bzlmod_v2::RootPackageLookupInputsProjectionKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::RootPackagePolicyProjectionError;
use slug_bzlmod_v2::RootRepoFileSemanticsProjectionKey;
use slug_bzlmod_v2::RootRepoFileUtf8Mode;
use slug_bzlmod_v2::RootRepositoryIgnoreInputsProjectionKey;
use slug_bzlmod_v2::YankedVersionPolicy;
use slug_bzlmod_v2::digest_generated_repo_specs;
use slug_bzlmod_v2::digest_included_module_files;
use slug_bzlmod_v2::digest_module_extension_definitions;
use slug_bzlmod_v2::digest_module_extension_usages;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::digest_registry_module_files;
use slug_bzlmod_v2::digest_registry_policy;
use slug_bzlmod_v2::digest_registry_source_specs;
use slug_bzlmod_v2::digest_repo_mapping_entries;
use slug_bzlmod_v2::digest_repo_mappings;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::PackageIdentifier;
use slug_workspace_v2::NormalizedAbsolutePath;

fn registry_policy_digest() -> String {
    digest_registry_policy([BzlmodRegistryPolicyEntry::new(
        "file:///registries/one",
        digest_module_file_content(b"registry one"),
    )
    .unwrap()])
}

fn registry_module_digest(content: impl AsRef<[u8]>) -> String {
    digest_registry_module_files([BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
}

fn registry_source_digest(content: impl AsRef<[u8]>) -> String {
    digest_registry_source_specs([BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
}

fn extension_definition_digest(content: impl AsRef<[u8]>) -> String {
    digest_module_extension_definitions([BzlmodExtensionDefinitionDigest::new(
        "//:ext.bzl%ext",
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
}

fn extension_usage_digest(content: impl AsRef<[u8]>) -> String {
    digest_module_extension_usages([BzlmodExtensionUsageDigest::new(
        "//:ext.bzl%ext",
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
}

fn generated_repo_spec_digest(content: impl AsRef<[u8]>) -> String {
    digest_generated_repo_specs([BzlmodGeneratedRepoSpecDigest::new(
        "//:ext.bzl%ext",
        "tagged",
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
}

fn repo_mapping_digest(entries: &[(&str, &str)]) -> String {
    let entries = entries
        .iter()
        .map(|(apparent, canonical)| ((*apparent).to_owned(), (*canonical).to_owned()))
        .collect::<BTreeMap<_, _>>();
    digest_repo_mappings([digest_repo_mapping_entries("aaa+", &entries).unwrap()]).unwrap()
}
fn visible_lockfile_digest(content: impl AsRef<[u8]>) -> String {
    BzlmodVisibleLockfileDigest::from_content(content)
        .stable_serialize()
        .to_owned()
}

fn hidden_lockfile_digest(content: impl AsRef<[u8]>) -> String {
    BzlmodHiddenLockfileDigest::from_content(content)
        .stable_serialize()
        .to_owned()
}

fn inputs(
    flag_value: Option<&str>,
    env_value: Option<&str>,
    lockfile_mode: LockfileMode,
) -> BzlmodDiceInputs {
    BzlmodDiceInputs::new(
        digest_module_file_content(b"module(name='root')"),
        digest_included_module_files([BzlmodModuleFileDigest::new(
            "deps.MODULE.bazel",
            digest_module_file_content(b"bazel_dep(name='dep', version='1.0.0')"),
        )
        .unwrap()])
        .unwrap(),
        registry_policy_digest(),
        registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
        registry_source_digest(br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#),
        extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
        lockfile_mode,
        BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(flag_value).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(env_value).unwrap(),
    )
    .unwrap()
}

#[test]
fn visible_lockfile_digest_distinguishes_absent_empty_and_content() {
    let absent = BzlmodVisibleLockfileDigest::absent();
    let empty = BzlmodVisibleLockfileDigest::from_content(b"");
    let one = BzlmodVisibleLockfileDigest::from_content(b"{\"lockFileVersion\":26}\n");
    let two = BzlmodVisibleLockfileDigest::from_content(b"{\"lockFileVersion\":27}\n");

    assert_eq!(absent.stable_serialize(), "absent");
    assert!(empty.stable_serialize().starts_with("present_"));
    assert_ne!(absent, empty);
    assert_ne!(empty, one);
    assert_ne!(one, two);
}

#[test]
fn hidden_lockfile_digest_distinguishes_absent_empty_and_content() {
    let absent = BzlmodHiddenLockfileDigest::absent();
    let empty = BzlmodHiddenLockfileDigest::from_content(b"");
    let one = BzlmodHiddenLockfileDigest::from_content(b"hidden-one");
    let two = BzlmodHiddenLockfileDigest::from_content(b"hidden-two");

    assert_eq!(absent.stable_serialize(), "absent");
    assert!(empty.stable_serialize().starts_with("present_"));
    assert_ne!(absent, empty);
    assert_ne!(empty, one);
    assert_ne!(one, two);
}

#[test]
fn module_file_content_digest_is_sha256_hex() {
    assert_eq!(
        digest_module_file_content(b"one\n"),
        "2c8b08da5ce60398e1f19af0e5dccc744df274b826abe585eaba68c525434806"
    );
    assert_ne!(
        digest_module_file_content(b"one\n"),
        digest_module_file_content(b"two\n")
    );
}

#[test]
fn included_module_digest_is_order_stable_and_content_sensitive() {
    let dep_one = BzlmodModuleFileDigest::new(
        "deps.MODULE.bazel",
        digest_module_file_content(b"bazel_dep(name='dep', version='1.0.0')"),
    )
    .unwrap();
    let root_extra = BzlmodModuleFileDigest::new(
        "fragments/root.MODULE.bazel",
        digest_module_file_content(b"register_toolchains('//:tc')"),
    )
    .unwrap();

    let forward = digest_included_module_files([dep_one.clone(), root_extra.clone()]).unwrap();
    let reverse = digest_included_module_files([root_extra, dep_one.clone()]).unwrap();
    let changed = digest_included_module_files([BzlmodModuleFileDigest::new(
        "deps.MODULE.bazel",
        digest_module_file_content(b"bazel_dep(name='dep', version='2.0.0')"),
    )
    .unwrap()])
    .unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn included_module_digest_rejects_duplicate_or_unstable_paths() {
    let digest = digest_module_file_content(b"content");
    let duplicate = digest_included_module_files([
        BzlmodModuleFileDigest::new("deps.MODULE.bazel", digest.clone()).unwrap(),
        BzlmodModuleFileDigest::new("deps.MODULE.bazel", digest).unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate included module file digest path"));

    let bad_path = BzlmodModuleFileDigest::new("../deps.MODULE.bazel", "abc").unwrap_err();
    assert!(bad_path.contains("normalized relative path"));
}

#[test]
fn registry_policy_digest_is_order_sensitive() {
    let first = BzlmodRegistryPolicyEntry::new(
        "file:///registries/first",
        digest_module_file_content(b"first registry"),
    )
    .unwrap();
    let second = BzlmodRegistryPolicyEntry::new(
        "file:///registries/second",
        digest_module_file_content(b"second registry"),
    )
    .unwrap();

    let first_then_second = digest_registry_policy([first.clone(), second.clone()]);
    let second_then_first = digest_registry_policy([second, first]);

    assert_ne!(first_then_second, second_then_first);
}

#[test]
fn registry_policy_digest_changes_with_registry_content_digest() {
    let before = digest_registry_policy([BzlmodRegistryPolicyEntry::new(
        "file:///registries/first",
        digest_module_file_content(b"registry before"),
    )
    .unwrap()]);
    let after = digest_registry_policy([BzlmodRegistryPolicyEntry::new(
        "file:///registries/first",
        digest_module_file_content(b"registry after"),
    )
    .unwrap()]);

    assert_ne!(before, after);
}

#[test]
fn registry_policy_entry_rejects_empty_url_or_bad_digest() {
    let empty_url =
        BzlmodRegistryPolicyEntry::new("", digest_module_file_content(b"registry")).unwrap_err();
    assert!(empty_url.contains("URL must not be empty"));

    let bad_digest =
        BzlmodRegistryPolicyEntry::new("file:///registries/first", "bad/digest").unwrap_err();
    assert!(bad_digest.contains("invalid registry_policy_entry_digest"));
}

#[test]
fn registry_module_digest_is_order_stable_and_content_sensitive() {
    let aaa = BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(
            b"module(name='aaa', version='1.0.0')\nbazel_dep(name='bbb', version='1.0.0')",
        ),
    )
    .unwrap();
    let bbb = BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("bbb", "1.0.0"),
        digest_module_file_content(b"module(name='bbb', version='1.0.0')"),
    )
    .unwrap();
    let aaa_changed = BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(
            b"module(name='aaa', version='1.0.0')\nbazel_dep(name='bbb', version='2.0.0')",
        ),
    )
    .unwrap();

    let forward = digest_registry_module_files([aaa.clone(), bbb.clone()]).unwrap();
    let reverse = digest_registry_module_files([bbb.clone(), aaa.clone()]).unwrap();
    let changed = digest_registry_module_files([aaa_changed, bbb]).unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn registry_module_digest_rejects_duplicate_or_unstable_identity() {
    let digest = digest_module_file_content(b"module(name='aaa', version='1.0.0')");
    let duplicate = digest_registry_module_files([
        BzlmodRegistryModuleFileDigest::new(
            "file:///%workspace%/registry",
            ModuleKey::new("aaa", "1.0.0"),
            digest.clone(),
        )
        .unwrap(),
        BzlmodRegistryModuleFileDigest::new(
            "file:///%workspace%/registry",
            ModuleKey::new("aaa", "1.0.0"),
            digest.clone(),
        )
        .unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate registry module file digest identity"));

    let empty_url =
        BzlmodRegistryModuleFileDigest::new("", ModuleKey::new("aaa", "1.0.0"), digest.clone())
            .unwrap_err();
    assert!(empty_url.contains("URL must not be empty"));

    let empty_name = BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("", "1.0.0"),
        digest.clone(),
    )
    .unwrap_err();
    assert!(empty_name.contains("module name must not be empty"));

    let bad_digest = BzlmodRegistryModuleFileDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        "bad/digest",
    )
    .unwrap_err();
    assert!(bad_digest.contains("invalid registry_module_file_digest"));
}

#[test]
fn registry_source_digest_is_order_stable_and_content_sensitive() {
    let aaa = BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(
            b"{\"url\":\"file:///aaa.tar.gz\",\"integrity\":\"sha256-aaa\"}",
        ),
    )
    .unwrap();
    let bbb = BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("bbb", "1.0.0"),
        digest_module_file_content(
            b"{\"url\":\"file:///bbb.tar.gz\",\"integrity\":\"sha256-bbb\"}",
        ),
    )
    .unwrap();
    let aaa_changed = BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        digest_module_file_content(
            b"{\"url\":\"file:///aaa-v2.tar.gz\",\"integrity\":\"sha256-aaa\"}",
        ),
    )
    .unwrap();

    let forward = digest_registry_source_specs([aaa.clone(), bbb.clone()]).unwrap();
    let reverse = digest_registry_source_specs([bbb.clone(), aaa.clone()]).unwrap();
    let changed = digest_registry_source_specs([aaa_changed, bbb]).unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn registry_source_digest_rejects_duplicate_or_unstable_identity() {
    let digest = digest_module_file_content(
        b"{\"url\":\"file:///aaa.tar.gz\",\"integrity\":\"sha256-aaa\"}",
    );
    let duplicate = digest_registry_source_specs([
        BzlmodRegistrySourceSpecDigest::new(
            "file:///%workspace%/registry",
            ModuleKey::new("aaa", "1.0.0"),
            digest.clone(),
        )
        .unwrap(),
        BzlmodRegistrySourceSpecDigest::new(
            "file:///%workspace%/registry",
            ModuleKey::new("aaa", "1.0.0"),
            digest.clone(),
        )
        .unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate registry source spec digest identity"));

    let empty_url =
        BzlmodRegistrySourceSpecDigest::new("", ModuleKey::new("aaa", "1.0.0"), digest.clone())
            .unwrap_err();
    assert!(empty_url.contains("URL must not be empty"));

    let empty_name = BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("", "1.0.0"),
        digest.clone(),
    )
    .unwrap_err();
    assert!(empty_name.contains("module name must not be empty"));

    let bad_digest = BzlmodRegistrySourceSpecDigest::new(
        "file:///%workspace%/registry",
        ModuleKey::new("aaa", "1.0.0"),
        "bad/digest",
    )
    .unwrap_err();
    assert!(bad_digest.contains("invalid registry_source_spec_digest"));
}

#[test]
fn resolved_graph_key_changes_when_registry_module_digest_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(
                b"module(name='aaa', version='1.0.0')\nbazel_dep(name='bbb', version='1.0.0')",
            ),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Refresh,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(
                b"module(name='aaa', version='1.0.0')\nbazel_dep(name='bbb', version='2.0.0')",
            ),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Refresh,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("registry_modules="));
}

#[test]
fn resolved_graph_key_changes_when_registry_source_digest_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                b"{\"url\":\"file:///archive.tar.gz\",\"integrity\":\"sha256-archive\"}",
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Refresh,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                b"{\"url\":\"file:///archive-v2.tar.gz\",\"integrity\":\"sha256-archive\"}",
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Refresh,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("registry_sources="));
}

#[test]
fn extension_usage_digest_is_order_stable_and_content_sensitive() {
    let alpha = BzlmodExtensionUsageDigest::new(
        "//:alpha.bzl%ext",
        digest_module_file_content(b"alpha usage"),
    )
    .unwrap();
    let beta = BzlmodExtensionUsageDigest::new(
        "//:beta.bzl%ext",
        digest_module_file_content(b"beta usage"),
    )
    .unwrap();

    let forward = digest_module_extension_usages([alpha.clone(), beta.clone()]).unwrap();
    let reverse = digest_module_extension_usages([beta, alpha]).unwrap();
    let changed = digest_module_extension_usages([BzlmodExtensionUsageDigest::new(
        "//:alpha.bzl%ext",
        digest_module_file_content(b"alpha usage changed"),
    )
    .unwrap()])
    .unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn extension_usage_digest_rejects_duplicate_or_unstable_ids() {
    let digest = digest_module_file_content(b"usage");
    let duplicate = digest_module_extension_usages([
        BzlmodExtensionUsageDigest::new("//:ext.bzl%ext", digest.clone()).unwrap(),
        BzlmodExtensionUsageDigest::new("//:ext.bzl%ext", digest).unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate module extension usage digest id"));

    let empty_id =
        BzlmodExtensionUsageDigest::new("", digest_module_file_content(b"usage")).unwrap_err();
    assert!(empty_id.contains("usage id must not be empty"));

    let bad_digest = BzlmodExtensionUsageDigest::new("//:ext.bzl%ext", "bad/digest").unwrap_err();
    assert!(bad_digest.contains("invalid extension_usage_digest"));
}

#[test]
fn repo_mapping_digest_is_order_stable_and_content_sensitive() {
    let alpha = BTreeMap::from([
        ("".to_owned(), "".to_owned()),
        ("aaa".to_owned(), "aaa+".to_owned()),
        ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
    ]);
    let beta = BTreeMap::from([
        ("bbb".to_owned(), "bbb+".to_owned()),
        ("ccc".to_owned(), "ccc+".to_owned()),
    ]);

    let alpha_digest = digest_repo_mapping_entries("aaa+", &alpha).unwrap();
    let beta_digest = digest_repo_mapping_entries("bbb+", &beta).unwrap();
    let forward = digest_repo_mappings([alpha_digest.clone(), beta_digest.clone()]).unwrap();
    let reverse = digest_repo_mappings([beta_digest, alpha_digest]).unwrap();

    let changed = BTreeMap::from([
        ("".to_owned(), "".to_owned()),
        ("aaa".to_owned(), "aaa+".to_owned()),
        ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
        ("extra".to_owned(), "extra+".to_owned()),
    ]);
    let changed =
        digest_repo_mappings([digest_repo_mapping_entries("aaa+", &changed).unwrap()]).unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn repo_mapping_digest_rejects_duplicate_or_unstable_ids() {
    let digest = digest_module_file_content(b"repo mapping");
    let duplicate = digest_repo_mappings([
        BzlmodRepoMappingDigest::new("aaa+", digest.clone()).unwrap(),
        BzlmodRepoMappingDigest::new("aaa+", digest).unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate repo mapping digest id"));

    let empty_repo =
        BzlmodRepoMappingDigest::new("", digest_module_file_content(b"repo mapping")).unwrap_err();
    assert!(empty_repo.contains("canonical repository must not be empty"));

    let bad_digest = BzlmodRepoMappingDigest::new("aaa+", "bad/digest").unwrap_err();
    assert!(bad_digest.contains("invalid repo_mapping_digest"));

    let bad_entry = BTreeMap::from([("bad\0entry".to_owned(), "aaa+".to_owned())]);
    let bad_entry = digest_repo_mapping_entries("aaa+", &bad_entry).unwrap_err();
    assert!(bad_entry.contains("must not contain NUL bytes"));
}
#[test]
fn generated_repo_spec_digest_is_order_stable_and_content_sensitive() {
    let alpha = BzlmodGeneratedRepoSpecDigest::new(
        "//:alpha.bzl%ext",
        "aaa",
        digest_module_file_content(b"alpha repo spec"),
    )
    .unwrap();
    let beta = BzlmodGeneratedRepoSpecDigest::new(
        "//:beta.bzl%ext",
        "bbb",
        digest_module_file_content(b"beta repo spec"),
    )
    .unwrap();

    let forward = digest_generated_repo_specs([alpha.clone(), beta.clone()]).unwrap();
    let reverse = digest_generated_repo_specs([beta, alpha]).unwrap();
    let changed = digest_generated_repo_specs([BzlmodGeneratedRepoSpecDigest::new(
        "//:alpha.bzl%ext",
        "aaa",
        digest_module_file_content(b"alpha repo spec changed"),
    )
    .unwrap()])
    .unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn generated_repo_spec_digest_rejects_duplicate_or_unstable_ids() {
    let digest = digest_module_file_content(b"repo spec");
    let duplicate = digest_generated_repo_specs([
        BzlmodGeneratedRepoSpecDigest::new("//:ext.bzl%ext", "tagged", digest.clone()).unwrap(),
        BzlmodGeneratedRepoSpecDigest::new("//:ext.bzl%ext", "tagged", digest).unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate generated repo spec digest id"));

    let empty_extension =
        BzlmodGeneratedRepoSpecDigest::new("", "tagged", digest_module_file_content(b"repo spec"))
            .unwrap_err();
    assert!(empty_extension.contains("extension id must not be empty"));

    let empty_repo = BzlmodGeneratedRepoSpecDigest::new(
        "//:ext.bzl%ext",
        "",
        digest_module_file_content(b"repo spec"),
    )
    .unwrap_err();
    assert!(empty_repo.contains("spec name must not be empty"));

    let bad_digest =
        BzlmodGeneratedRepoSpecDigest::new("//:ext.bzl%ext", "tagged", "bad/digest").unwrap_err();
    assert!(bad_digest.contains("invalid generated_repo_spec_digest"));
}

#[test]
fn extension_definition_digest_is_order_stable_and_content_sensitive() {
    let alpha = BzlmodExtensionDefinitionDigest::new(
        "//:alpha.bzl%ext",
        digest_module_file_content(b"alpha definition"),
    )
    .unwrap();
    let beta = BzlmodExtensionDefinitionDigest::new(
        "//:beta.bzl%ext",
        digest_module_file_content(b"beta definition"),
    )
    .unwrap();

    let forward = digest_module_extension_definitions([alpha.clone(), beta.clone()]).unwrap();
    let reverse = digest_module_extension_definitions([beta, alpha]).unwrap();
    let changed = digest_module_extension_definitions([BzlmodExtensionDefinitionDigest::new(
        "//:alpha.bzl%ext",
        digest_module_file_content(b"alpha definition changed"),
    )
    .unwrap()])
    .unwrap();

    assert_eq!(forward, reverse);
    assert_ne!(forward, changed);
}

#[test]
fn extension_definition_digest_rejects_duplicate_or_unstable_ids() {
    let digest = digest_module_file_content(b"definition");
    let duplicate = digest_module_extension_definitions([
        BzlmodExtensionDefinitionDigest::new("//:ext.bzl%ext", digest.clone()).unwrap(),
        BzlmodExtensionDefinitionDigest::new("//:ext.bzl%ext", digest).unwrap(),
    ])
    .unwrap_err();
    assert!(duplicate.contains("duplicate module extension definition digest id"));

    let empty_id =
        BzlmodExtensionDefinitionDigest::new("", digest_module_file_content(b"definition"))
            .unwrap_err();
    assert!(empty_id.contains("definition id must not be empty"));

    let bad_digest =
        BzlmodExtensionDefinitionDigest::new("//:ext.bzl%ext", "bad/digest").unwrap_err();
    assert!(bad_digest.contains("invalid extension_definition_digest"));
}

#[test]
fn resolved_graph_key_changes_when_extension_definition_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_two'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("extension_defs="));
}

#[test]
fn resolved_graph_key_changes_when_extension_usage_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='two')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("extensions="));
}

#[test]
fn resolved_graph_key_changes_when_repo_mappings_change() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new_with_repo_mappings(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            repo_mapping_digest(&[("aaa", "aaa+"), ("bazel_tools", "bazel_tools")]),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new_with_repo_mappings(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            repo_mapping_digest(&[
                ("aaa", "aaa+"),
                ("bazel_tools", "bazel_tools"),
                ("generated", "+ext+generated"),
            ]),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("repo_mappings="));
}
#[test]
fn resolved_graph_key_changes_when_generated_repo_specs_change() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new_with_generated_repo_specs(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            generated_repo_spec_digest(b"repo_rule=tagged_repo;message=one"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new_with_generated_repo_specs(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            generated_repo_spec_digest(b"repo_rule=tagged_repo;message=two"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(before.stable_serialize().contains("generated_repos="));
}

#[test]
fn lockfile_mode_parses_bazel_flag_values() {
    assert_eq!(
        LockfileMode::from_bazel_flag_value("off").unwrap(),
        LockfileMode::Off
    );
    assert_eq!(
        LockfileMode::from_bazel_flag_value("update").unwrap(),
        LockfileMode::Update
    );
    assert_eq!(
        LockfileMode::from_bazel_flag_value("refresh").unwrap(),
        LockfileMode::Refresh
    );
    assert_eq!(
        LockfileMode::from_bazel_flag_value("error").unwrap(),
        LockfileMode::Error
    );

    let err = LockfileMode::from_bazel_flag_value("bad").unwrap_err();
    assert_eq!(
        err,
        "Not a valid Lockfile mode: 'bad' (should be off, update, refresh or error)"
    );
}

#[test]
fn lockfile_mode_controls_visible_lockfile_read_write_policy() {
    assert!(!LockfileMode::Off.reads_visible_lockfile());
    assert!(!LockfileMode::Off.writes_visible_lockfile());

    assert!(LockfileMode::Update.reads_visible_lockfile());
    assert!(LockfileMode::Update.writes_visible_lockfile());

    assert!(LockfileMode::Refresh.reads_visible_lockfile());
    assert!(LockfileMode::Refresh.writes_visible_lockfile());

    assert!(LockfileMode::Error.reads_visible_lockfile());
    assert!(!LockfileMode::Error.writes_visible_lockfile());
}
#[test]
fn policy_keys_serialize_yanked_allowlists_stably() {
    let env_policy =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("zzz@2.0.0,yyy@1.0.0"))
            .unwrap();
    let command_policy =
        BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(Some("zzz@2.0.0,yyy@1.0.0"))
            .unwrap();
    let ignored_dev_policy =
        BzlmodCommandPolicyKey::from_flags(Some("zzz@2.0.0,yyy@1.0.0"), true).unwrap();

    assert_eq!(
        env_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0]"
    );
    assert_eq!(
        command_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0];ignore_dev_dependency=false"
    );
    assert_eq!(
        ignored_dev_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0];ignore_dev_dependency=true"
    );
    assert!(!command_policy.ignore_dev_dependency());
    assert!(ignored_dev_policy.ignore_dev_dependency());
    assert_eq!(
        env_policy.yanked_versions_policy(),
        &YankedVersionPolicy::AllowList(std::collections::BTreeSet::from([
            ModuleKey::new("yyy", "1.0.0"),
            ModuleKey::new("zzz", "2.0.0"),
        ]))
    );
}

#[test]
fn effective_yanked_policy_unions_command_and_environment() {
    let inputs = inputs(Some("zzz@2.0.0"), Some("yyy@1.0.0"), LockfileMode::Update);

    assert_eq!(
        inputs.effective_yanked_versions_policy(),
        YankedVersionPolicy::AllowList(std::collections::BTreeSet::from([
            ModuleKey::new("yyy", "1.0.0"),
            ModuleKey::new("zzz", "2.0.0"),
        ]))
    );
    assert!(
        inputs
            .stable_serialize()
            .contains("command=allow_yanked=[zzz@2.0.0];ignore_dev_dependency=false")
    );
    assert!(
        inputs
            .stable_serialize()
            .contains("env=allow_yanked=[yyy@1.0.0]")
    );
}

#[test]
fn resolved_graph_key_changes_when_environment_policy_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let reject =
        ResolvedBzlmodGraphDiceKey::new(root.clone(), inputs(None, None, LockfileMode::Update));
    let allow = ResolvedBzlmodGraphDiceKey::new(
        root,
        inputs(None, Some("yyy@1.0.0"), LockfileMode::Update),
    );

    assert_ne!(reject, allow);
    assert!(
        reject
            .stable_serialize()
            .contains("env=allow_yanked=reject")
    );
    assert!(
        allow
            .stable_serialize()
            .contains("env=allow_yanked=[yyy@1.0.0]")
    );
}

#[test]
fn resolved_graph_key_changes_when_command_policy_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let reject =
        ResolvedBzlmodGraphDiceKey::new(root.clone(), inputs(None, None, LockfileMode::Update));
    let allow = ResolvedBzlmodGraphDiceKey::new(
        root,
        inputs(Some("yyy@1.0.0"), None, LockfileMode::Update),
    );

    assert_ne!(reject, allow);
    assert!(
        reject
            .stable_serialize()
            .contains("command=allow_yanked=reject;ignore_dev_dependency=false")
    );
    assert!(
        allow
            .stable_serialize()
            .contains("command=allow_yanked=[yyy@1.0.0];ignore_dev_dependency=false")
    );
}

#[test]
fn resolved_graph_key_changes_when_ignore_dev_dependency_flag_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let include = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let ignore = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            LockfileMode::Update,
            BzlmodCommandPolicyKey::from_flags(None, true).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(include, ignore);
    assert!(!include.inputs().command_policy().ignore_dev_dependency());
    assert!(ignore.inputs().command_policy().ignore_dev_dependency());
    assert!(
        include
            .stable_serialize()
            .contains("command=allow_yanked=reject;ignore_dev_dependency=false")
    );
    assert!(
        ignore
            .stable_serialize()
            .contains("command=allow_yanked=reject;ignore_dev_dependency=true")
    );
}
#[test]
fn resolved_graph_key_changes_when_visible_lockfile_digest_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let old_inputs = BzlmodDiceInputs::new(
        digest_module_file_content(b"module(name='root')"),
        digest_included_module_files([BzlmodModuleFileDigest::new(
            "deps.MODULE.bazel",
            digest_module_file_content(b"bazel_dep(name='dep', version='1.0.0')"),
        )
        .unwrap()])
        .unwrap(),
        registry_policy_digest(),
        registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
        registry_source_digest(br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#),
        extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        visible_lockfile_digest(b"{\"lockFileVersion\":25}\n"),
        LockfileMode::Update,
        BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
    )
    .unwrap();
    let old = ResolvedBzlmodGraphDiceKey::new(root.clone(), old_inputs);
    let new = ResolvedBzlmodGraphDiceKey::new(root, inputs(None, None, LockfileMode::Update));

    assert_ne!(old, new);
    assert!(old.stable_serialize().contains("lockfile=present_"));
    assert!(new.stable_serialize().contains("lockfile=present_"));
}

#[test]
fn resolved_graph_key_changes_when_hidden_lockfile_digest_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new_with_hidden_lockfile(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            hidden_lockfile_digest(b"hidden-one"),
            LockfileMode::Error,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );
    let after = ResolvedBzlmodGraphDiceKey::new(
        root,
        BzlmodDiceInputs::new_with_hidden_lockfile(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
            registry_source_digest(
                br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
            ),
            extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
            hidden_lockfile_digest(b"hidden-two"),
            LockfileMode::Error,
            BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        )
        .unwrap(),
    );

    assert_ne!(before, after);
    assert!(
        before
            .stable_serialize()
            .contains("hidden_lockfile=present_")
    );
    assert!(
        after
            .stable_serialize()
            .contains("hidden_lockfile=present_")
    );
}

#[test]
fn resolved_graph_key_changes_when_lockfile_mode_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let update =
        ResolvedBzlmodGraphDiceKey::new(root.clone(), inputs(None, None, LockfileMode::Update));
    let error = ResolvedBzlmodGraphDiceKey::new(root, inputs(None, None, LockfileMode::Error));

    assert_ne!(update, error);
    assert!(update.stable_serialize().contains("mode=update"));
    assert!(error.stable_serialize().contains("mode=error"));
}

#[test]
fn dice_inputs_reject_empty_or_unstable_digests() {
    let command = BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(None).unwrap();
    let env = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();

    let empty = BzlmodDiceInputs::new(
        "",
        "includesabc",
        registry_policy_digest(),
        registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
        registry_source_digest(br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#),
        extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
        LockfileMode::Update,
        command.clone(),
        env.clone(),
    )
    .unwrap_err();
    assert!(empty.contains("root_module_digest must not be empty"));

    let bad = BzlmodDiceInputs::new(
        "root/abc",
        "includesabc",
        registry_policy_digest(),
        registry_module_digest(b"module(name = 'aaa', version = '1.0.0')"),
        registry_source_digest(br#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#),
        extension_definition_digest(b"_OUTPUT_NAME = 'impl_one'"),
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        visible_lockfile_digest(b"{\"lockFileVersion\":26}\n"),
        LockfileMode::Update,
        command,
        env,
    )
    .unwrap_err();
    assert!(bad.contains("invalid root_module_digest"));
}

fn normalized_path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn root_package_policy_inputs(
    workspace: &NormalizedAbsolutePath,
    package_roots: &[&str],
    deleted_packages: &[&str],
    vendor_directory: Option<&str>,
    utf8_mode: Option<&str>,
) -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        workspace.dupe(),
        package_roots
            .iter()
            .map(|path| normalized_path(path))
            .collect::<Vec<_>>(),
        deleted_packages.iter().copied(),
        vendor_directory.map(normalized_path),
        utf8_mode,
    )
    .unwrap()
}

#[test]
fn root_package_policy_normalizes_bazel_flags_without_inventing_package_roots() {
    let workspace = normalized_path("/work/root");
    let inputs = root_package_policy_inputs(
        &workspace,
        &["/roots/second", "/roots/first", "/roots/second"],
        &["", "pkg,,//pkg,", "@repo//x,@@repo//x"],
        Some("/outside/vendor"),
        None,
    );

    assert_eq!(inputs.workspace(), &workspace);
    assert_eq!(
        inputs.package_roots(),
        &[
            normalized_path("/roots/second"),
            normalized_path("/roots/first"),
            normalized_path("/roots/second"),
        ]
    );
    assert_eq!(
        inputs.repo_file_semantics().utf8_mode,
        RootRepoFileUtf8Mode::Warning
    );
    assert_eq!(
        inputs.vendor_directory(),
        Some(&normalized_path("/outside/vendor"))
    );
    assert_eq!(inputs.deleted_packages().len(), 3);
    for package in ["", "pkg", "@repo//x"] {
        assert!(
            inputs
                .deleted_packages()
                .contains(&PackageIdentifier::parse_bazel_package_identifier(package).unwrap()),
            "{package:?}"
        );
    }

    let empty = root_package_policy_inputs(&workspace, &[], &[], None, Some("warning"));
    assert!(empty.package_roots().is_empty());
    assert!(empty.deleted_packages().is_empty());
    assert_eq!(empty.vendor_directory(), None);

    let contained_vendor =
        root_package_policy_inputs(&workspace, &[], &[], Some("/work/root/vendor"), None);
    assert_eq!(
        contained_vendor.vendor_directory(),
        Some(&normalized_path("/work/root/vendor"))
    );
}

#[test]
fn root_repo_file_utf8_mode_matches_bazel_bool_or_enum_conversion() {
    for (value, expected) in [
        ("off", RootRepoFileUtf8Mode::Off),
        ("OFF", RootRepoFileUtf8Mode::Off),
        ("warning", RootRepoFileUtf8Mode::Warning),
        ("WaRnInG", RootRepoFileUtf8Mode::Warning),
        ("error", RootRepoFileUtf8Mode::Error),
        ("ERROR", RootRepoFileUtf8Mode::Error),
        ("true", RootRepoFileUtf8Mode::Error),
        ("TRUE", RootRepoFileUtf8Mode::Error),
        ("1", RootRepoFileUtf8Mode::Error),
        ("yes", RootRepoFileUtf8Mode::Error),
        ("t", RootRepoFileUtf8Mode::Error),
        ("y", RootRepoFileUtf8Mode::Error),
        ("false", RootRepoFileUtf8Mode::Off),
        ("FALSE", RootRepoFileUtf8Mode::Off),
        ("0", RootRepoFileUtf8Mode::Off),
        ("no", RootRepoFileUtf8Mode::Off),
        ("f", RootRepoFileUtf8Mode::Off),
        ("n", RootRepoFileUtf8Mode::Off),
    ] {
        assert_eq!(
            RootRepoFileUtf8Mode::from_bazel_flag_value(value).unwrap(),
            expected,
            "{value}"
        );
    }
    for value in ["", "warn", "on", "2", " error "] {
        assert!(
            RootRepoFileUtf8Mode::from_bazel_flag_value(value).is_err(),
            "{value:?}"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
enum RootPackagePolicyProjectionKind {
    Semantics,
    RepositoryIgnore,
    PackageLookup,
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct RootPackagePolicyProjectionCounterKey {
    workspace: NormalizedAbsolutePath,
    kind: RootPackagePolicyProjectionKind,
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

impl PartialEq for RootPackagePolicyProjectionCounterKey {
    fn eq(&self, other: &Self) -> bool {
        self.workspace == other.workspace
            && self.kind == other.kind
            && Arc::ptr_eq(&self.counter, &other.counter)
    }
}

impl Eq for RootPackagePolicyProjectionCounterKey {}

impl Hash for RootPackagePolicyProjectionCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.kind.hash(state);
        Arc::as_ptr(&self.counter).hash(state);
    }
}

impl fmt::Display for RootPackagePolicyProjectionCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-package-policy-projection-counter:{:?}:{}:{:p}",
            self.kind,
            self.workspace,
            Arc::as_ptr(&self.counter)
        )
    }
}

#[async_trait]
impl Key for RootPackagePolicyProjectionCounterKey {
    type Value = Result<usize, RootPackagePolicyProjectionError>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let projected = match self.kind {
            RootPackagePolicyProjectionKind::Semantics => ctx
                .compute(&RootRepoFileSemanticsProjectionKey::new(
                    self.workspace.dupe(),
                ))
                .await
                .unwrap()
                .map(|_| ()),
            RootPackagePolicyProjectionKind::RepositoryIgnore => ctx
                .compute(&RootRepositoryIgnoreInputsProjectionKey::new(
                    self.workspace.dupe(),
                ))
                .await
                .unwrap()
                .map(|_| ()),
            RootPackagePolicyProjectionKind::PackageLookup => ctx
                .compute(&RootPackageLookupInputsProjectionKey::new(
                    self.workspace.dupe(),
                ))
                .await
                .unwrap()
                .map(|_| ()),
        };
        projected.map(|()| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

async fn reinject_root_package_policy(
    transaction: DiceTransaction,
    inputs: RootPackagePolicyInputs,
) -> DiceTransaction {
    let mut updater = transaction.into_updater();
    inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
    updater.commit().await
}

#[tokio::test]
async fn root_package_policy_keys_are_workspace_scoped_and_missing_inputs_fail_closed() {
    let workspace_a = normalized_path("/work/a");
    let workspace_b = normalized_path("/work/b");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(
        &mut updater,
        root_package_policy_inputs(
            &workspace_a,
            &["/roots/a"],
            &["a"],
            Some("/vendor/a"),
            Some("warning"),
        ),
    )
    .unwrap();
    let mut transaction = updater.commit().await;

    let semantics_a = transaction
        .compute(&RootRepoFileSemanticsProjectionKey::new(workspace_a.dupe()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(semantics_a.utf8_mode, RootRepoFileUtf8Mode::Warning);
    assert_eq!(
        transaction
            .compute(&RootRepoFileSemanticsProjectionKey::new(workspace_b.dupe()))
            .await
            .unwrap(),
        Err(RootPackagePolicyProjectionError::MissingInput {
            workspace: workspace_b.dupe(),
        })
    );
    assert_eq!(
        transaction
            .compute(&RootRepositoryIgnoreInputsProjectionKey::new(
                workspace_b.dupe()
            ))
            .await
            .unwrap(),
        Err(RootPackagePolicyProjectionError::MissingInput {
            workspace: workspace_b.dupe(),
        })
    );
    assert_eq!(
        transaction
            .compute(&RootPackageLookupInputsProjectionKey::new(
                workspace_b.dupe()
            ))
            .await
            .unwrap(),
        Err(RootPackagePolicyProjectionError::MissingInput {
            workspace: workspace_b.dupe(),
        })
    );

    let two_workspace_dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = two_workspace_dice.updater();
    inject_root_package_policy_inputs(
        &mut updater,
        root_package_policy_inputs(
            &workspace_a,
            &["/roots/a"],
            &["a"],
            Some("/vendor/a"),
            Some("warning"),
        ),
    )
    .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        root_package_policy_inputs(
            &workspace_b,
            &["/roots/b"],
            &["b"],
            Some("/vendor/b"),
            Some("error"),
        ),
    )
    .unwrap();
    transaction = updater.commit().await;
    assert_eq!(
        transaction
            .compute(&RootRepoFileSemanticsProjectionKey::new(workspace_a.dupe()))
            .await
            .unwrap()
            .unwrap(),
        semantics_a
    );
    assert_eq!(
        transaction
            .compute(&RootRepoFileSemanticsProjectionKey::new(workspace_b))
            .await
            .unwrap()
            .unwrap()
            .utf8_mode,
        RootRepoFileUtf8Mode::Error
    );
}

#[tokio::test]
async fn root_package_policy_projections_prune_unrelated_changes_and_restore_a() {
    let workspace = normalized_path("/work/root");
    let semantics_counter = Arc::new(AtomicUsize::new(0));
    let ignore_counter = Arc::new(AtomicUsize::new(0));
    let lookup_counter = Arc::new(AtomicUsize::new(0));
    let semantics_key = RootPackagePolicyProjectionCounterKey {
        workspace: workspace.dupe(),
        kind: RootPackagePolicyProjectionKind::Semantics,
        counter: semantics_counter.dupe(),
    };
    let ignore_key = RootPackagePolicyProjectionCounterKey {
        workspace: workspace.dupe(),
        kind: RootPackagePolicyProjectionKind::RepositoryIgnore,
        counter: ignore_counter.dupe(),
    };
    let lookup_key = RootPackagePolicyProjectionCounterKey {
        workspace: workspace.dupe(),
        kind: RootPackagePolicyProjectionKind::PackageLookup,
        counter: lookup_counter.dupe(),
    };
    let state_a = || {
        root_package_policy_inputs(
            &workspace,
            &["/roots/a"],
            &["a"],
            Some("/vendor/a"),
            Some("warning"),
        )
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, state_a()).unwrap();
    let mut transaction = updater.commit().await;

    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(1));

    transaction = reinject_root_package_policy(transaction, state_a()).await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(1));

    transaction = reinject_root_package_policy(
        transaction,
        root_package_policy_inputs(
            &workspace,
            &["/roots/a"],
            &["b"],
            Some("/vendor/a"),
            Some("warning"),
        ),
    )
    .await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(2));

    transaction = reinject_root_package_policy(
        transaction,
        root_package_policy_inputs(
            &workspace,
            &["/roots/a"],
            &["b"],
            Some("/vendor/a"),
            Some("error"),
        ),
    )
    .await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(2));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(1));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(2));

    transaction = reinject_root_package_policy(
        transaction,
        root_package_policy_inputs(
            &workspace,
            &["/roots/a"],
            &["b"],
            Some("/vendor/b"),
            Some("error"),
        ),
    )
    .await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(2));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(2));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(2));

    transaction = reinject_root_package_policy(
        transaction,
        root_package_policy_inputs(
            &workspace,
            &["/roots/b"],
            &["b"],
            Some("/vendor/b"),
            Some("error"),
        ),
    )
    .await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(2));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(3));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(3));

    transaction = reinject_root_package_policy(transaction, state_a()).await;
    assert_eq!(transaction.compute(&semantics_key).await.unwrap(), Ok(3));
    assert_eq!(transaction.compute(&ignore_key).await.unwrap(), Ok(4));
    assert_eq!(transaction.compute(&lookup_key).await.unwrap(), Ok(4));
    let restored = transaction
        .compute(&RootPackageLookupInputsProjectionKey::new(workspace))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.package_roots(), &[normalized_path("/roots/a")]);
    assert!(
        restored
            .deleted_packages()
            .contains(&PackageIdentifier::parse_bazel_package_identifier("a").unwrap())
    );
}

#[test]
fn command_module_override_policy_has_canonical_semantic_identity() {
    let workspace = std::path::Path::new("/workspace");
    let a = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace,
        ["zed=/deps/zed", "alpha=deps/alpha"],
    )
    .unwrap();
    let b = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace,
        ["alpha=/workspace/deps/alpha", "zed=/deps/zed"],
    )
    .unwrap();
    let changed = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace,
        ["alpha=/workspace/deps/other", "zed=/deps/zed"],
    )
    .unwrap();
    assert_eq!(a, b);
    assert_ne!(a, changed);
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(hash(&a), hash(&changed));
    assert_eq!(
        a.module_overrides()
            .map(|(name, path)| (name.to_owned(), path.display().to_string()))
            .collect::<Vec<_>>(),
        [
            ("alpha".to_owned(), "/workspace/deps/alpha".to_owned()),
            ("zed".to_owned(), "/deps/zed".to_owned()),
        ]
    );

    let root: slug_bzlmod_v2::RootModuleCommandPolicy = a.clone().into();
    assert_eq!(
        root.command_module_overrides()
            .map(|(name, path)| (name.to_owned(), path.display().to_string()))
            .collect::<Vec<_>>(),
        [
            ("alpha".to_owned(), "/workspace/deps/alpha".to_owned()),
            ("zed".to_owned(), "/deps/zed".to_owned()),
        ]
    );
    assert!(a.module_overrides().any(|(name, _)| name == "zed"));
    assert!(
        BzlmodCommandPolicyKey::from_flags(None, false)
            .unwrap()
            .module_overrides()
            .all(|(name, _)| name != "bazel_tools")
    );
}

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct CommandOverrideCounterKey {
    workspace: NormalizedAbsolutePath,
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

impl PartialEq for CommandOverrideCounterKey {
    fn eq(&self, other: &Self) -> bool {
        self.workspace == other.workspace && Arc::ptr_eq(&self.counter, &other.counter)
    }
}

impl Eq for CommandOverrideCounterKey {}

impl Hash for CommandOverrideCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        Arc::as_ptr(&self.counter).hash(state);
    }
}

impl fmt::Display for CommandOverrideCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "command-module-override-counter:{}:{:p}",
            self.workspace.as_path().display(),
            Arc::as_ptr(&self.counter)
        )
    }
}

#[async_trait]
impl Key for CommandOverrideCounterKey {
    type Value = (usize, Arc<[(String, String)]>);

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let policy = ctx
            .compute(&slug_bzlmod_v2::RootModuleCommandPolicyKey {
                workspace: self.workspace.as_path().to_path_buf(),
            })
            .await
            .unwrap();
        (
            self.counter.fetch_add(1, Ordering::SeqCst) + 1,
            policy
                .command_module_overrides()
                .map(|(name, path)| (name.to_owned(), path.display().to_string()))
                .collect::<Vec<_>>()
                .into(),
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

async fn inject_command_override_policy(
    transaction: DiceTransaction,
    workspace: &std::path::Path,
    command: BzlmodCommandPolicyKey,
    environment: Option<&str>,
    lockfile_mode: LockfileMode,
) -> DiceTransaction {
    let mut updater = transaction.into_updater();
    slug_bzlmod_v2::inject_root_module_request_inputs(
        &mut updater,
        workspace,
        command,
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(environment).unwrap(),
        lockfile_mode,
    )
    .unwrap();
    updater.commit().await
}

#[tokio::test]
async fn command_module_override_dice_input_reuses_and_invalidates_a_b_a() {
    let workspace = normalized_path("/workspace");
    let default = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
    let override_a = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace.as_path(),
        ["zed=/deps/zed", "alpha=deps/alpha"],
    )
    .unwrap();
    let override_b = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace.as_path(),
        ["alpha=/workspace/deps/alpha", "zed=/deps/zed"],
    )
    .unwrap();
    assert_eq!(override_a, override_b);
    let changed_path = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        workspace.as_path(),
        ["alpha=/workspace/deps/changed", "zed=/deps/zed"],
    )
    .unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    slug_bzlmod_v2::inject_root_module_request_inputs(
        &mut updater,
        workspace.as_path(),
        default.clone(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let counter = Arc::new(AtomicUsize::new(0));
    let key = CommandOverrideCounterKey {
        workspace: workspace.dupe(),
        counter: counter.clone(),
    };
    let mut transaction = updater.commit().await;

    let a = transaction.compute(&key).await.unwrap();
    assert_eq!(a.0, 1);
    assert!(a.1.is_empty());
    assert_eq!(transaction.compute(&key).await.unwrap(), a);

    transaction = inject_command_override_policy(
        transaction,
        workspace.as_path(),
        override_a,
        None,
        LockfileMode::Update,
    )
    .await;
    let b = transaction.compute(&key).await.unwrap();
    assert_eq!(b.0, 2);
    assert_eq!(
        b.1.as_ref(),
        &[
            ("alpha".to_owned(), "/workspace/deps/alpha".to_owned()),
            ("zed".to_owned(), "/deps/zed".to_owned()),
        ]
    );

    transaction = inject_command_override_policy(
        transaction,
        workspace.as_path(),
        override_b.clone(),
        Some("all"),
        LockfileMode::Error,
    )
    .await;
    assert_eq!(transaction.compute(&key).await.unwrap(), b);
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    transaction = inject_command_override_policy(
        transaction,
        workspace.as_path(),
        changed_path,
        None,
        LockfileMode::Update,
    )
    .await;
    let changed = transaction.compute(&key).await.unwrap();
    assert_eq!(changed.0, 3);
    assert_eq!(changed.1[0].1, "/workspace/deps/changed");

    transaction = inject_command_override_policy(
        transaction,
        workspace.as_path(),
        override_b,
        None,
        LockfileMode::Update,
    )
    .await;
    let path_restored = transaction.compute(&key).await.unwrap();
    assert_eq!(path_restored.0, 4);
    assert_eq!(path_restored.1, b.1);

    transaction = inject_command_override_policy(
        transaction,
        workspace.as_path(),
        default,
        None,
        LockfileMode::Update,
    )
    .await;
    let restored = transaction.compute(&key).await.unwrap();
    assert_eq!(restored.0, 5);
    assert!(restored.1.is_empty());
}

#[test]
fn command_override_precedence_projection_leaves_root_inputs_distinct() {
    let root = BTreeMap::from([
        ("bazel_tools", "/builtin/bazel_tools"),
        ("dep", "/root/dep"),
        ("root_only", "/root/only"),
    ]);
    let command = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        std::path::Path::new("/workspace"),
        ["dep=/command/dep", "bazel_tools=/command/bazel_tools"],
    )
    .unwrap();
    let effective = |name: &str| {
        command
            .module_overrides()
            .find_map(|(candidate, path)| (candidate == name).then(|| path.display().to_string()))
            .or_else(|| root.get(name).map(|path| (*path).to_owned()))
    };
    assert_eq!(effective("dep").as_deref(), Some("/command/dep"));
    assert_eq!(
        effective("bazel_tools").as_deref(),
        Some("/command/bazel_tools")
    );
    assert_eq!(effective("root_only").as_deref(), Some("/root/only"));
    assert_eq!(root["dep"], "/root/dep");
    assert_eq!(root["bazel_tools"], "/builtin/bazel_tools");
    assert!(
        command
            .module_overrides()
            .any(|(name, _)| name == "bazel_tools")
    );
}
