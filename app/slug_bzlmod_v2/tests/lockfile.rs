/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use serde_json::Value;
use slug_bzlmod_v2::BazelLockfileRecordedInput;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::parse_bazel_lockfile;
use slug_bzlmod_v2::validate_module_extension_bzl_transitive_digests;
use slug_bzlmod_v2::validate_module_extension_recorded_file_inputs;
use slug_bzlmod_v2::validate_module_extension_usage_digests;
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
    assert!(lockfile.module_extensions.is_empty());
    assert!(lockfile.facts.is_empty());
}

#[test]
fn parses_module_extension_generated_repo_specs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "bzlTransitiveDigest": "bzl-digest",
        "usagesDigest": "usage-digest",
        "recordedInputs": [],
        "generatedRepoSpecs": {
          "tagged": {
            "repoRuleId": "@@//:ext.bzl%tagged_repo",
            "attributes": {
              "message": "hello from tag"
            }
          }
        }
      }
    }
  },
  "facts": {},
  "factsVersions": {"//:ext.bzl%ext": 1}
}"#,
    )
    .unwrap();

    let extension = lockfile.module_extensions.get("//:ext.bzl%ext").unwrap();
    let general = extension.general.as_ref().unwrap();
    assert_eq!(general.bzl_transitive_digest.as_deref(), Some("bzl-digest"));
    assert_eq!(general.usages_digest.as_deref(), Some("usage-digest"));
    assert!(general.recorded_inputs.is_empty());

    let tagged = general.generated_repo_specs.get("tagged").unwrap();
    assert_eq!(tagged.repo_rule_id, "@@//:ext.bzl%tagged_repo");
    assert_eq!(
        tagged.attributes.get("message"),
        Some(&Value::String("hello from tag".to_owned()))
    );
    assert_eq!(
        lockfile.facts_versions.get("//:ext.bzl%ext"),
        Some(&Value::from(1))
    );
}

#[test]
fn parses_module_extension_recorded_file_inputs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": [
          "FILE:@@//seed.txt 2c8b08da5ce60398e1f19af0e5dccc744df274b826abe585eaba68c525434806"
        ]
      }
    }
  }
}"#,
    )
    .unwrap();

    let extension = lockfile.module_extensions.get("//:ext.bzl%ext").unwrap();
    let general = extension.general.as_ref().unwrap();
    assert_eq!(
        general.recorded_inputs,
        vec![BazelLockfileRecordedInput::File {
            label: "@@//seed.txt".to_owned(),
            digest: "2c8b08da5ce60398e1f19af0e5dccc744df274b826abe585eaba68c525434806".to_owned(),
        }]
    );
}

#[test]
fn validates_module_extension_recorded_file_inputs_against_observed_map() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": ["FILE:@@//seed.txt old-file-digest"]
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "@@//seed.txt".to_owned(),
        "old-file-digest".to_owned(),
    )]);

    validate_module_extension_recorded_file_inputs(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_stale_module_extension_recorded_file_input_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": ["FILE:@@//seed.txt old-file-digest"]
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "@@//seed.txt".to_owned(),
        "new-file-digest".to_owned(),
    )]);

    let err = validate_module_extension_recorded_file_inputs(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(err.contains("input to the extension '@@//:ext.bzl%ext' changed"));
    assert!(err.contains("file info or contents of @@//seed.txt changed"));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}

#[test]
fn accepts_absent_optional_visible_lockfile_fields() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 26}"#).unwrap();

    assert_eq!(lockfile.lock_file_version, 26);
    assert!(lockfile.registry_file_hashes.is_empty());
    assert!(lockfile.selected_yanked_versions.is_empty());
    assert!(lockfile.module_extensions.is_empty());
    assert!(lockfile.facts.is_empty());
    assert!(lockfile.facts_versions.is_empty());
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
fn rejects_malformed_module_extension_shape() {
    let err = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "generatedRepoSpecs": {
          "tagged": {"attributes": {}}
        }
      }
    }
  }
}"#,
    )
    .unwrap_err();

    assert!(err.contains("generatedRepoSpecs entry tagged is missing string repoRuleId"));
}

#[test]
fn rejects_missing_lockfile_version() {
    let err = parse_bazel_lockfile(r#"{"selectedYankedVersions": {}}"#).unwrap_err();

    assert!(err.contains("missing numeric lockFileVersion"));
}

#[test]
fn validates_module_extension_usage_digests_against_observed_map() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "usagesDigest": "usage-digest"
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "//:ext.bzl%ext".to_owned(),
        "usage-digest".to_owned(),
    )]);

    validate_module_extension_usage_digests(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_stale_module_extension_usage_digest_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "usagesDigest": "old-usage-digest"
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "//:ext.bzl%ext".to_owned(),
        "new-usage-digest".to_owned(),
    )]);

    let err = validate_module_extension_usage_digests(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(err.contains("usages of the extension '@@//:ext.bzl%ext' have changed"));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}

#[test]
fn rejects_stale_module_extension_bzl_digest_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "bzlTransitiveDigest": "old-bzl-digest"
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "//:ext.bzl%ext".to_owned(),
        "new-bzl-digest".to_owned(),
    )]);

    let err = validate_module_extension_bzl_transitive_digests(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(err.contains(
        "implementation of the extension '@@//:ext.bzl%ext' or one of its transitive .bzl files has changed"
    ));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
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
