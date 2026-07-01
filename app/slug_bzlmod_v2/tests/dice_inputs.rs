/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodDiceInputs;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::BzlmodExtensionDefinitionDigest;
use slug_bzlmod_v2::BzlmodExtensionUsageDigest;
use slug_bzlmod_v2::BzlmodModuleFileDigest;
use slug_bzlmod_v2::BzlmodRegistryModuleFileDigest;
use slug_bzlmod_v2::BzlmodRegistryPolicyEntry;
use slug_bzlmod_v2::BzlmodRegistrySourceSpecDigest;
use slug_bzlmod_v2::BzlmodVisibleLockfileDigest;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ResolvedBzlmodGraphDiceKey;
use slug_bzlmod_v2::YankedVersionPolicy;
use slug_bzlmod_v2::digest_included_module_files;
use slug_bzlmod_v2::digest_module_extension_definitions;
use slug_bzlmod_v2::digest_module_extension_usages;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::digest_registry_module_files;
use slug_bzlmod_v2::digest_registry_policy;
use slug_bzlmod_v2::digest_registry_source_specs;

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

fn visible_lockfile_digest(content: impl AsRef<[u8]>) -> String {
    BzlmodVisibleLockfileDigest::from_content(content)
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

    assert_eq!(
        env_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0]"
    );
    assert_eq!(
        command_policy.stable_serialize(),
        env_policy.stable_serialize()
    );
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
            .contains("command=allow_yanked=[zzz@2.0.0]")
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
            .contains("command=allow_yanked=reject")
    );
    assert!(
        allow
            .stable_serialize()
            .contains("command=allow_yanked=[yyy@1.0.0]")
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
