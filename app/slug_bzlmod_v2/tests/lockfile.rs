/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::parse_bazel_lockfile;
use slug_bzlmod_v2::validate_registry_file_hashes;

#[test]
fn parses_visible_lockfile_registry_and_yanked_fields() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "file:///workspace/registry/bazel_registry.json": "abc123",
    "file:///workspace/registry/modules/yyy/1.0.0/MODULE.bazel": "def456"
  },
  "selectedYankedVersions": {
    "yyy@1.0.0": "bad release"
  },
  "moduleExtensions": {},
  "facts": {}
}"#,
    )
    .unwrap();

    assert_eq!(lockfile.lock_file_version, 26);
    assert_eq!(
        lockfile
            .registry_file_hashes
            .get("file:///workspace/registry/bazel_registry.json"),
        Some(&"abc123".to_owned())
    );
    assert_eq!(
        lockfile
            .selected_yanked_versions
            .get(&ModuleKey::new("yyy", "1.0.0")),
        Some(&"bad release".to_owned())
    );
}

#[test]
fn accepts_absent_optional_visible_lockfile_fields() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 26}"#).unwrap();

    assert_eq!(lockfile.lock_file_version, 26);
    assert!(lockfile.registry_file_hashes.is_empty());
    assert!(lockfile.selected_yanked_versions.is_empty());
}

#[test]
fn rejects_malformed_selected_yanked_key() {
    let err = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "selectedYankedVersions": {"yyy": "bad release"}
}"#,
    )
    .unwrap_err();

    assert!(err.contains("selectedYankedVersions key yyy must be module@version"));
}

#[test]
fn rejects_missing_lockfile_version() {
    let err = parse_bazel_lockfile(r#"{"selectedYankedVersions": {}}"#).unwrap_err();

    assert!(err.contains("missing numeric lockFileVersion"));
}
#[test]
fn validates_registry_hashes_against_observed_digest_map() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "wanted"
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel".to_owned(),
        "wanted".to_owned(),
    )]);

    validate_registry_file_hashes(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_mismatched_registry_hash_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "000000"
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel".to_owned(),
        "184960".to_owned(),
    )]);

    let err = validate_registry_file_hashes(&lockfile, &observed).unwrap_err();

    assert!(err.contains(
        "Failed to fetch registry file https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel"
    ));
    assert!(err.contains("Checksum was 184960 but wanted 000000"));
}
