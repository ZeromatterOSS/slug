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
use slug_bzlmod_v2::BzlmodExtensionUsageDigest;
use slug_bzlmod_v2::BzlmodModuleFileDigest;
use slug_bzlmod_v2::BzlmodRegistryPolicyEntry;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ResolvedBzlmodGraphDiceKey;
use slug_bzlmod_v2::YankedVersionPolicy;
use slug_bzlmod_v2::digest_included_module_files;
use slug_bzlmod_v2::digest_module_extension_usages;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::digest_registry_policy;

fn registry_policy_digest() -> String {
    digest_registry_policy([BzlmodRegistryPolicyEntry::new(
        "file:///registries/one",
        digest_module_file_content(b"registry one"),
    )
    .unwrap()])
}

fn extension_usage_digest(content: impl AsRef<[u8]>) -> String {
    digest_module_extension_usages([BzlmodExtensionUsageDigest::new(
        "//:ext.bzl%ext",
        digest_module_file_content(content),
    )
    .unwrap()])
    .unwrap()
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
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        "lockfileabc",
        lockfile_mode,
        BzlmodCommandPolicyKey::from_allow_yanked_versions_flag(flag_value).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(env_value).unwrap(),
    )
    .unwrap()
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
fn resolved_graph_key_changes_when_extension_usage_changes() {
    let root = ModuleKey::new("root", "0.1.0");
    let before = ResolvedBzlmodGraphDiceKey::new(
        root.clone(),
        BzlmodDiceInputs::new(
            digest_module_file_content(b"module(name='root')"),
            digest_included_module_files([]).unwrap(),
            registry_policy_digest(),
            extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
            "lockfileabc",
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
            extension_usage_digest(b"ext.repo(name='tagged', message='two')"),
            "lockfileabc",
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
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        "lockfileabc",
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
        extension_usage_digest(b"ext.repo(name='tagged', message='one')"),
        "lockfileabc",
        LockfileMode::Update,
        command,
        env,
    )
    .unwrap_err();
    assert!(bad.contains("invalid root_module_digest"));
}
