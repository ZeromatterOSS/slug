/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fs;

use serde_json::Value;
use slug_bzlmod_v2::BAZEL_9_LOCK_FILE_VERSION;
use slug_bzlmod_v2::BazelLockfileRecordedInput;
use slug_bzlmod_v2::BzlmodHiddenLockfileDigest;
use slug_bzlmod_v2::BzlmodVisibleLockfileDigest;
use slug_bzlmod_v2::HiddenLockfileInput;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::LockfileReadInputs;
use slug_bzlmod_v2::ModuleExtensionReplayInputs;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::VisibleLockfileApply;
use slug_bzlmod_v2::VisibleLockfileInput;
use slug_bzlmod_v2::VisibleLockfilePlan;
use slug_bzlmod_v2::VisibleLockfileRead;
use slug_bzlmod_v2::apply_visible_lockfile_plan;
use slug_bzlmod_v2::empty_bazel_lockfile;
use slug_bzlmod_v2::parse_bazel_lockfile;
use slug_bzlmod_v2::parse_hidden_lockfile_fail_open;
use slug_bzlmod_v2::parse_visible_lockfile_for_mode;
use slug_bzlmod_v2::plan_visible_lockfile;
use slug_bzlmod_v2::render_bazel_lockfile;
use slug_bzlmod_v2::validate_lockfile_version;
use slug_bzlmod_v2::validate_module_extension_bzl_transitive_digests;
use slug_bzlmod_v2::validate_module_extension_generated_repo_specs;
use slug_bzlmod_v2::validate_module_extension_recorded_env_inputs;
use slug_bzlmod_v2::validate_module_extension_recorded_file_inputs;
use slug_bzlmod_v2::validate_module_extension_replay_inputs;
use slug_bzlmod_v2::validate_module_extension_usage_digests;
use slug_bzlmod_v2::validate_registry_file_hashes;
use slug_bzlmod_v2::validate_required_registry_file_hashes;

#[test]
fn parses_visible_lockfile_registry_and_yanked_fields() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "file:///workspace/registry/bazel_registry.json": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "file:///workspace/registry/modules/yyy/1.0.0/MODULE.bazel": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  },
  "selectedYankedVersions": {
    "yyy@1.0.0": "bad release"
  },
  "moduleExtensions": {},
  "facts": {}
}"#,
    )
    .unwrap();

    assert_eq!(lockfile.lock_file_version, 28);
    assert_eq!(
        lockfile
            .registry_file_hashes
            .get("file:///workspace/registry/bazel_registry.json"),
        Some(&"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned())
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
fn rejects_malformed_registry_checksum_during_lockfile_parsing() {
    let error = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://example.test/unused": "not-a-sha256"
  }
}"#,
    )
    .unwrap_err();

    assert!(error.contains("Invalid checksum for registry file https://example.test/unused"));
}

#[test]
fn parses_module_extension_generated_repo_specs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
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
fn parses_module_extension_recorded_env_inputs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": ["ENV:SLUG_STAGE5_ENV one"]
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
        vec![BazelLockfileRecordedInput::Env {
            name: "SLUG_STAGE5_ENV".to_owned(),
            value: "one".to_owned(),
        }]
    );
}

#[test]
fn validates_module_extension_recorded_env_inputs_against_observed_map() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": ["ENV:SLUG_STAGE5_ENV one"]
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed =
        std::collections::BTreeMap::from([("SLUG_STAGE5_ENV".to_owned(), "one".to_owned())]);

    validate_module_extension_recorded_env_inputs(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_stale_module_extension_recorded_env_input_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "recordedInputs": ["ENV:SLUG_STAGE5_ENV one"]
      }
    }
  }
}"#,
    )
    .unwrap();
    let observed =
        std::collections::BTreeMap::from([("SLUG_STAGE5_ENV".to_owned(), "two".to_owned())]);

    let err = validate_module_extension_recorded_env_inputs(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(err.contains("input to the extension '@@//:ext.bzl%ext' changed"));
    assert!(err.contains("environment variable SLUG_STAGE5_ENV changed: 'one' -> 'two'"));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}

#[test]
fn parses_module_extension_recorded_file_inputs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
fn validates_module_extension_generated_repo_specs_against_observed_map() {
    let lockfile = module_extension_replay_lockfile();
    let observed = observed_generated_repo_specs(&lockfile);

    validate_module_extension_generated_repo_specs(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_stale_module_extension_generated_repo_spec_like_bazel() {
    let lockfile = module_extension_replay_lockfile();
    let mut observed = observed_generated_repo_specs(&lockfile);
    observed
        .get_mut("//:ext.bzl%ext")
        .unwrap()
        .get_mut("tagged")
        .unwrap()
        .attributes
        .insert("message".to_owned(), Value::String("changed".to_owned()));

    let err = validate_module_extension_generated_repo_specs(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(
        err.contains("generated repository tagged from extension '@@//:ext.bzl%ext' has changed")
    );
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}

#[test]
fn validates_module_extension_replay_inputs_together() {
    let lockfile = module_extension_replay_lockfile();
    let observed = ModuleExtensionReplayInputs {
        usage_digests: std::collections::BTreeMap::from([(
            "//:ext.bzl%ext".to_owned(),
            "usage-digest".to_owned(),
        )]),
        bzl_transitive_digests: std::collections::BTreeMap::from([(
            "//:ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
        )]),
        recorded_env_values: std::collections::BTreeMap::from([(
            "SLUG_STAGE5_ENV".to_owned(),
            "one".to_owned(),
        )]),
        recorded_file_digests: std::collections::BTreeMap::from([(
            "@@//seed.txt".to_owned(),
            "file-digest".to_owned(),
        )]),
        generated_repo_specs: observed_generated_repo_specs(&lockfile),
    };

    validate_module_extension_replay_inputs(&lockfile, &observed).unwrap();
}

#[test]
fn replay_inputs_propagate_bazel_shaped_stale_usage_error() {
    let lockfile = module_extension_replay_lockfile();
    let observed = ModuleExtensionReplayInputs {
        usage_digests: std::collections::BTreeMap::from([(
            "//:ext.bzl%ext".to_owned(),
            "new-usage-digest".to_owned(),
        )]),
        bzl_transitive_digests: std::collections::BTreeMap::from([(
            "//:ext.bzl%ext".to_owned(),
            "bzl-digest".to_owned(),
        )]),
        recorded_env_values: std::collections::BTreeMap::from([(
            "SLUG_STAGE5_ENV".to_owned(),
            "one".to_owned(),
        )]),
        recorded_file_digests: std::collections::BTreeMap::from([(
            "@@//seed.txt".to_owned(),
            "file-digest".to_owned(),
        )]),
        generated_repo_specs: observed_generated_repo_specs(&lockfile),
    };

    let err = validate_module_extension_replay_inputs(&lockfile, &observed).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock is no longer up-to-date"));
    assert!(err.contains("usages of the extension '@@//:ext.bzl%ext' have changed"));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}

#[test]
fn accepts_absent_optional_visible_lockfile_fields() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 28}"#).unwrap();

    assert_eq!(lockfile.lock_file_version, 28);
    assert!(lockfile.registry_file_hashes.is_empty());
    assert!(lockfile.selected_yanked_versions.is_empty());
    assert!(lockfile.module_extensions.is_empty());
    assert!(lockfile.facts.is_empty());
    assert!(lockfile.facts_versions.is_empty());
}

#[test]
fn validates_supported_lockfile_version() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 28}"#).unwrap();

    validate_lockfile_version(&lockfile, BAZEL_9_LOCK_FILE_VERSION).unwrap();
}

#[test]
fn rejects_unsupported_lockfile_version_like_bazel() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 25}"#).unwrap();

    let err = validate_lockfile_version(&lockfile, BAZEL_9_LOCK_FILE_VERSION).unwrap_err();

    assert!(
        err.contains("The version of MODULE.bazel.lock is not supported by this version of Bazel")
    );
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}
#[test]
fn rejects_malformed_selected_yanked_key() {
    let err = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
  "lockFileVersion": 28,
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
fn validates_required_registry_file_hash_entries() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_shell/0.6.1/MODULE.bazel": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  }
}"#,
    )
    .unwrap();

    validate_required_registry_file_hashes(
        &lockfile,
        &["https://bcr.bazel.build/modules/rules_shell/0.6.1/MODULE.bazel"],
    )
    .unwrap();
}

#[test]
fn rejects_missing_registry_file_hash_like_bazel_error_mode() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 28}"#).unwrap();

    let err = validate_required_registry_file_hashes(
        &lockfile,
        &["https://bcr.bazel.build/modules/rules_shell/0.6.1/MODULE.bazel"],
    )
    .unwrap_err();

    assert!(err.contains("Missing checksum for registry file https://bcr.bazel.build/modules/rules_shell/0.6.1/MODULE.bazel"));
    assert!(err.contains("not permitted with --lockfile_mode=error"));
    assert!(err.contains("bazel mod deps --lockfile_mode=update"));
}
#[test]
fn validates_registry_hashes_against_observed_digest_map() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  }
}"#,
    )
    .unwrap();
    let observed = std::collections::BTreeMap::from([(
        "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel".to_owned(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
    )]);

    validate_registry_file_hashes(&lockfile, &observed).unwrap();
}

#[test]
fn rejects_mismatched_registry_hash_like_bazel() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "0000000000000000000000000000000000000000000000000000000000000000"
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
    assert!(err.contains("Checksum was 184960 but wanted 0000000000000000000000000000000000000000000000000000000000000000"));
}

#[test]
fn renders_visible_lockfile_with_bazel_top_level_shape() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/source.json": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
  },
  "selectedYankedVersions": {
    "rules_cc@0.2.17": "test yanked reason"
  },
  "moduleExtensions": {},
  "facts": {}
}"#,
    )
    .unwrap();

    let rendered = render_bazel_lockfile(&lockfile).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "{\n",
            "  \"lockFileVersion\": 28,\n",
            "  \"registryFileHashes\": {\n",
            "    \"https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel\": \"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\",\n",
            "    \"https://bcr.bazel.build/modules/rules_cc/0.2.17/source.json\": \"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\"\n",
            "  },\n",
            "  \"selectedYankedVersions\": {\n",
            "    \"rules_cc@0.2.17\": \"test yanked reason\"\n",
            "  },\n",
            "  \"moduleExtensions\": {},\n",
            "  \"facts\": {}\n",
            "}\n",
        )
    );
    assert_eq!(parse_bazel_lockfile(&rendered).unwrap(), lockfile);
}

#[test]
fn renders_module_extension_lockfile_replay_shape() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {},
  "selectedYankedVersions": {},
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "bzlTransitiveDigest": "bzl-digest",
        "usagesDigest": "usage-digest",
        "recordedInputs": [
          "ENV:FOO bar",
          "FILE://:input.txt abc123"
        ],
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
  "factsVersions": {
    "//:ext.bzl%ext": 1
  }
}"#,
    )
    .unwrap();

    let rendered = render_bazel_lockfile(&lockfile).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "{\n",
            "  \"lockFileVersion\": 28,\n",
            "  \"registryFileHashes\": {},\n",
            "  \"selectedYankedVersions\": {},\n",
            "  \"moduleExtensions\": {\n",
            "    \"//:ext.bzl%ext\": {\n",
            "      \"general\": {\n",
            "        \"bzlTransitiveDigest\": \"bzl-digest\",\n",
            "        \"usagesDigest\": \"usage-digest\",\n",
            "        \"recordedInputs\": [\n",
            "          \"ENV:FOO bar\",\n",
            "          \"FILE://:input.txt abc123\"\n",
            "        ],\n",
            "        \"generatedRepoSpecs\": {\n",
            "          \"tagged\": {\n",
            "            \"repoRuleId\": \"@@//:ext.bzl%tagged_repo\",\n",
            "            \"attributes\": {\n",
            "              \"message\": \"hello from tag\"\n",
            "            }\n",
            "          }\n",
            "        }\n",
            "      }\n",
            "    }\n",
            "  },\n",
            "  \"facts\": {},\n",
            "  \"factsVersions\": {\n",
            "    \"//:ext.bzl%ext\": 1\n",
            "  }\n",
            "}\n",
        )
    );
    assert_eq!(parse_bazel_lockfile(&rendered).unwrap(), lockfile);
}

#[test]
fn render_bazel_lockfile_is_deterministic_across_input_order() {
    let first = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://z.example.test/MODULE.bazel": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    "https://a.example.test/MODULE.bazel": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  },
  "selectedYankedVersions": {
    "zzz@1.0.0": "z reason",
    "aaa@1.0.0": "a reason"
  },
  "moduleExtensions": {
    "//:z_ext.bzl%ext": {
      "general": {
        "generatedRepoSpecs": {
          "z_repo": {
            "repoRuleId": "@@//:z_ext.bzl%z_repo",
            "attributes": {"z": true, "a": false}
          }
        }
      }
    },
    "//:a_ext.bzl%ext": {
      "general": {
        "generatedRepoSpecs": {
          "a_repo": {
            "repoRuleId": "@@//:a_ext.bzl%a_repo",
            "attributes": {"z": 2, "a": 1}
          }
        }
      }
    }
  },
  "facts": {}
}"#,
    )
    .unwrap();
    let second = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://a.example.test/MODULE.bazel": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "https://z.example.test/MODULE.bazel": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
  },
  "selectedYankedVersions": {
    "aaa@1.0.0": "a reason",
    "zzz@1.0.0": "z reason"
  },
  "moduleExtensions": {
    "//:a_ext.bzl%ext": {
      "general": {
        "generatedRepoSpecs": {
          "a_repo": {
            "repoRuleId": "@@//:a_ext.bzl%a_repo",
            "attributes": {"a": 1, "z": 2}
          }
        }
      }
    },
    "//:z_ext.bzl%ext": {
      "general": {
        "generatedRepoSpecs": {
          "z_repo": {
            "repoRuleId": "@@//:z_ext.bzl%z_repo",
            "attributes": {"a": false, "z": true}
          }
        }
      }
    }
  },
  "facts": {}
}"#,
    )
    .unwrap();

    let rendered_first = render_bazel_lockfile(&first).unwrap();
    let rendered_second = render_bazel_lockfile(&second).unwrap();

    assert_eq!(rendered_first, rendered_second);
    assert!(
        rendered_first.find("https://a.example.test").unwrap()
            < rendered_first.find("https://z.example.test").unwrap()
    );
    assert!(rendered_first.find("aaa@1.0.0").unwrap() < rendered_first.find("zzz@1.0.0").unwrap());
    assert!(
        rendered_first.find("//:a_ext.bzl%ext").unwrap()
            < rendered_first.find("//:z_ext.bzl%ext").unwrap()
    );
    assert!(
        rendered_first.find("\"a_repo\"").unwrap() < rendered_first.find("\"z_repo\"").unwrap()
    );
}
#[test]
fn visible_lockfile_plan_honors_bazel_modes() {
    let desired = simple_visible_lockfile();
    let rendered = render_bazel_lockfile(&desired).unwrap();

    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Off, None, &desired).unwrap(),
        VisibleLockfilePlan::Ignore
    );
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Update, Some(&rendered), &desired).unwrap(),
        VisibleLockfilePlan::Keep
    );
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Refresh, Some(&rendered), &desired).unwrap(),
        VisibleLockfilePlan::Keep
    );
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Update, None, &desired).unwrap(),
        VisibleLockfilePlan::Write {
            content: rendered.clone()
        }
    );
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Refresh, Some("{}\n"), &desired).unwrap(),
        VisibleLockfilePlan::Write { content: rendered }
    );
}

#[test]
fn visible_lockfile_error_mode_rejects_missing_stale_and_bad_versions() {
    let desired = simple_visible_lockfile();
    let rendered = render_bazel_lockfile(&desired).unwrap();

    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Error, Some(&rendered), &desired).unwrap(),
        VisibleLockfilePlan::Keep
    );

    let missing = plan_visible_lockfile(&LockfileMode::Error, None, &desired).unwrap();
    assert!(matches!(missing, VisibleLockfilePlan::Error { .. }));
    if let VisibleLockfilePlan::Error { message } = missing {
        assert!(message.contains("MODULE.bazel.lock is missing"));
        assert!(message.contains("--lockfile_mode=update"));
    }

    let stale = rendered.replace(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    );
    let stale_plan = plan_visible_lockfile(&LockfileMode::Error, Some(&stale), &desired).unwrap();
    assert!(matches!(stale_plan, VisibleLockfilePlan::Error { .. }));
    if let VisibleLockfilePlan::Error { message } = stale_plan {
        assert!(message.contains("MODULE.bazel.lock is no longer up-to-date"));
        assert!(message.contains("--lockfile_mode=update"));
    }

    let unsupported = rendered.replace("\"lockFileVersion\": 28", "\"lockFileVersion\": 27");
    let unsupported_plan =
        plan_visible_lockfile(&LockfileMode::Error, Some(&unsupported), &desired).unwrap();
    assert!(matches!(
        unsupported_plan,
        VisibleLockfilePlan::Error { .. }
    ));
    if let VisibleLockfilePlan::Error { message } = unsupported_plan {
        assert!(message.contains("The version of MODULE.bazel.lock is not supported"));
        assert!(message.contains("--lockfile_mode=update"));
    }
}

#[test]
fn applies_visible_lockfile_plan_with_atomic_write_boundary() {
    let dir = scratch_dir("visible-lockfile-apply");
    let lockfile_path = dir.join("MODULE.bazel.lock");
    let desired = simple_visible_lockfile();
    let rendered = render_bazel_lockfile(&desired).unwrap();

    assert_eq!(
        apply_visible_lockfile_plan(&lockfile_path, &VisibleLockfilePlan::Ignore).unwrap(),
        VisibleLockfileApply::Ignored
    );
    assert!(!lockfile_path.exists());
    assert_eq!(
        apply_visible_lockfile_plan(&lockfile_path, &VisibleLockfilePlan::Keep).unwrap(),
        VisibleLockfileApply::Kept
    );
    assert!(!lockfile_path.exists());

    assert_eq!(
        apply_visible_lockfile_plan(
            &lockfile_path,
            &VisibleLockfilePlan::Write {
                content: rendered.clone(),
            },
        )
        .unwrap(),
        VisibleLockfileApply::Written {
            bytes: rendered.len(),
        }
    );
    assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), rendered);

    let updated = rendered.replace(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );
    apply_visible_lockfile_plan(
        &lockfile_path,
        &VisibleLockfilePlan::Write {
            content: updated.clone(),
        },
    )
    .unwrap();
    assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), updated);

    let entries = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![std::ffi::OsString::from("MODULE.bazel.lock")]);
}

#[test]
fn apply_visible_lockfile_plan_returns_error_without_writing() {
    let dir = scratch_dir("visible-lockfile-error");
    let lockfile_path = dir.join("MODULE.bazel.lock");

    let err = apply_visible_lockfile_plan(
        &lockfile_path,
        &VisibleLockfilePlan::Error {
            message: "stale lockfile".to_owned(),
        },
    )
    .unwrap_err();

    assert_eq!(err, "stale lockfile");
    assert!(!lockfile_path.exists());
}

#[test]
fn visible_lockfile_input_bridges_dice_bytes_to_planner() {
    let absent = VisibleLockfileInput::from_optional_bytes(None).unwrap();
    assert_eq!(absent.digest(), &BzlmodVisibleLockfileDigest::absent());
    assert_eq!(absent.existing_content(), None);

    let existing = b"{\"lockFileVersion\":28}\n";
    let present = VisibleLockfileInput::from_optional_bytes(Some(existing)).unwrap();
    assert_eq!(
        present.digest(),
        &BzlmodVisibleLockfileDigest::from_content(existing)
    );
    assert_eq!(
        present.existing_content(),
        Some("{\"lockFileVersion\":28}\n")
    );

    let desired = simple_visible_lockfile();
    let plan =
        plan_visible_lockfile(&LockfileMode::Update, present.existing_content(), &desired).unwrap();
    assert!(matches!(plan, VisibleLockfilePlan::Write { .. }));
}

#[test]
fn visible_lockfile_input_rejects_invalid_utf8() {
    let err = VisibleLockfileInput::from_optional_bytes(Some(&[0xff])).unwrap_err();

    assert!(err.contains("MODULE.bazel.lock"));
    assert!(err.contains("UTF-8"));
}

#[test]
fn hidden_lockfile_input_bridges_dice_bytes_to_replay_parser() {
    let absent = HiddenLockfileInput::from_optional_bytes(None).unwrap();
    assert_eq!(absent.digest(), &BzlmodHiddenLockfileDigest::absent());
    assert_eq!(absent.existing_content(), None);

    let existing = br#"{"lockFileVersion":28,"moduleExtensions":{}}"#;
    let present = HiddenLockfileInput::from_optional_bytes(Some(existing)).unwrap();
    assert_eq!(
        present.digest(),
        &BzlmodHiddenLockfileDigest::from_content(existing)
    );
    assert_eq!(
        parse_bazel_lockfile(present.existing_content().unwrap())
            .unwrap()
            .lock_file_version,
        BAZEL_9_LOCK_FILE_VERSION
    );
}

#[test]
fn hidden_lockfile_input_rejects_invalid_utf8() {
    let err = HiddenLockfileInput::from_optional_bytes(Some(&[0xff])).unwrap_err();

    assert!(err.contains("hidden MODULE.bazel.lock"));
    assert!(err.contains("UTF-8"));
}

#[test]
fn hidden_lockfile_parse_fail_open_keeps_current_valid_content() {
    let input = HiddenLockfileInput::from_optional_bytes(Some(
        br#"{"lockFileVersion":28,"registryFileHashes":{"https://example.test/MODULE.bazel":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
    ))
    .unwrap();

    let parsed = input.parse_fail_open();

    assert_eq!(parsed.lock_file_version, BAZEL_9_LOCK_FILE_VERSION);
    assert_eq!(
        parsed
            .registry_file_hashes
            .get("https://example.test/MODULE.bazel"),
        Some(&"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned())
    );
}

#[test]
fn hidden_lockfile_parse_fail_open_uses_empty_for_absent_malformed_or_old_version() {
    let absent = HiddenLockfileInput::absent();
    assert_eq!(absent.parse_fail_open(), empty_bazel_lockfile());

    let malformed = HiddenLockfileInput::from_optional_bytes(Some(b"{ nope")).unwrap();
    assert_eq!(malformed.parse_fail_open(), empty_bazel_lockfile());

    let old_version =
        HiddenLockfileInput::from_optional_bytes(Some(br#"{"lockFileVersion":24}"#)).unwrap();
    assert_eq!(old_version.parse_fail_open(), empty_bazel_lockfile());

    assert_eq!(
        parse_hidden_lockfile_fail_open(Some("not json")),
        empty_bazel_lockfile()
    );
}

#[test]
fn visible_lockfile_read_honors_bazel_modes() {
    let current = VisibleLockfileInput::from_optional_bytes(Some(
        br#"{"lockFileVersion":28,"registryFileHashes":{"https://example.test/MODULE.bazel":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
    ))
    .unwrap();
    let visible = parse_visible_lockfile_for_mode(&LockfileMode::Update, &current).unwrap();
    let lockfile = visible.parsed().unwrap();
    assert_eq!(
        lockfile
            .registry_file_hashes
            .get("https://example.test/MODULE.bazel"),
        Some(&"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned())
    );

    let absent = VisibleLockfileInput::absent();
    assert_eq!(
        parse_visible_lockfile_for_mode(&LockfileMode::Error, &absent).unwrap(),
        VisibleLockfileRead::Parsed(empty_bazel_lockfile().into())
    );

    let old_version =
        VisibleLockfileInput::from_optional_bytes(Some(br#"{"lockFileVersion":24}"#)).unwrap();
    assert_eq!(
        parse_visible_lockfile_for_mode(&LockfileMode::Update, &old_version).unwrap(),
        VisibleLockfileRead::Parsed(empty_bazel_lockfile().into())
    );
    let err = parse_visible_lockfile_for_mode(&LockfileMode::Error, &old_version).unwrap_err();
    assert!(err.contains("version of MODULE.bazel.lock is not supported"));

    let malformed_without_marker =
        VisibleLockfileInput::from_optional_bytes(Some(b"{ nope")).unwrap();
    assert_eq!(
        parse_visible_lockfile_for_mode(&LockfileMode::Refresh, &malformed_without_marker).unwrap(),
        VisibleLockfileRead::Parsed(empty_bazel_lockfile().into())
    );
}

#[test]
fn visible_lockfile_read_scans_version_before_json_like_bazel() {
    let read = |mode: LockfileMode, content: &[u8]| {
        let input = VisibleLockfileInput::from_optional_bytes(Some(content)).unwrap();
        parse_visible_lockfile_for_mode(&mode, &input)
    };
    let empty = VisibleLockfileRead::Parsed(empty_bazel_lockfile().into());

    assert_eq!(read(LockfileMode::Update, b"{ nope").unwrap(), empty);
    assert!(
        read(LockfileMode::Error, b"{ nope")
            .unwrap_err()
            .contains("version of MODULE.bazel.lock is not supported")
    );

    let stale_and_malformed = br#"{"lockFileVersion":27, nope"#;
    assert_eq!(
        read(LockfileMode::Refresh, stale_and_malformed).unwrap(),
        empty
    );
    assert!(
        read(LockfileMode::Error, stale_and_malformed)
            .unwrap_err()
            .contains("version of MODULE.bazel.lock is not supported")
    );

    let current_and_malformed = br#"{"lockFileVersion":28, nope"#;
    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        let error = read(mode, current_and_malformed).unwrap_err();
        assert!(
            error.contains("Failed to read and parse the MODULE.bazel.lock file"),
            "{error}"
        );
    }

    let first_numeric_marker_wins = br#"{"lockFileVersion":27,"lockFileVersion":28}"#;
    assert_eq!(
        read(LockfileMode::Update, first_numeric_marker_wins).unwrap(),
        empty
    );
    assert!(
        read(LockfileMode::Error, first_numeric_marker_wins)
            .unwrap_err()
            .contains("version of MODULE.bazel.lock is not supported")
    );

    let overflow = br#"{"lockFileVersion":2147483648}"#;
    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        let error = read(mode, overflow).unwrap_err();
        assert!(
            error.contains("For input string: \"2147483648\""),
            "{error}"
        );
    }

    let negative = br#"{"lockFileVersion":-1}"#;
    assert_eq!(read(LockfileMode::Update, negative).unwrap(), empty);

    let ascii_whitespace_first = b"{\"lockFileVersion\":\x0b27,\"lockFileVersion\":28}";
    assert_eq!(
        read(LockfileMode::Update, ascii_whitespace_first).unwrap(),
        empty
    );
    let unicode_whitespace_first =
        "{\"lockFileVersion\":\u{00a0}27,\"lockFileVersion\":28}".as_bytes();
    assert!(
        read(LockfileMode::Update, unicode_whitespace_first)
            .unwrap_err()
            .contains("Failed to read and parse the MODULE.bazel.lock file")
    );

    let reordered = read(
        LockfileMode::Update,
        br#"{
  "facts": {},
  "moduleExtensions": {},
  "lockFileVersion": 28,
  "selectedYankedVersions": {},
  "registryFileHashes": {}
}"#,
    )
    .unwrap();
    let compact = read(LockfileMode::Update, br#"{"lockFileVersion":28}"#).unwrap();
    assert_eq!(reordered, compact);
}

#[test]
fn lockfile_read_inputs_skip_all_reads_in_off_mode_and_parse_hidden_fail_open() {
    let visible = VisibleLockfileInput::from_optional_bytes(Some(b"{ nope")).unwrap();
    let hidden =
        HiddenLockfileInput::from_optional_bytes(Some(br#"{"lockFileVersion":24}"#)).unwrap();
    let off = LockfileReadInputs {
        mode: LockfileMode::Off,
        visible,
        hidden,
    }
    .read()
    .unwrap();
    assert_eq!(off.visible, VisibleLockfileRead::Ignored);
    assert_eq!(off.hidden, None);

    let visible = VisibleLockfileInput::absent();
    let hidden = HiddenLockfileInput::from_optional_bytes(Some(b"{ nope")).unwrap();
    let update = LockfileReadInputs {
        mode: LockfileMode::Update,
        visible,
        hidden,
    }
    .read()
    .unwrap();
    assert_eq!(
        update.visible,
        VisibleLockfileRead::Parsed(empty_bazel_lockfile().into())
    );
    assert_eq!(update.hidden, Some(empty_bazel_lockfile()));
}

fn module_extension_replay_lockfile() -> slug_bzlmod_v2::BazelLockfile {
    parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "moduleExtensions": {
    "//:ext.bzl%ext": {
      "general": {
        "bzlTransitiveDigest": "bzl-digest",
        "usagesDigest": "usage-digest",
        "recordedInputs": [
          "ENV:SLUG_STAGE5_ENV one",
          "FILE:@@//seed.txt file-digest"
        ],
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
  }
}"#,
    )
    .unwrap()
}

fn observed_generated_repo_specs(
    lockfile: &slug_bzlmod_v2::BazelLockfile,
) -> std::collections::BTreeMap<
    String,
    std::collections::BTreeMap<String, slug_bzlmod_v2::BazelLockfileRepoSpec>,
> {
    let general = lockfile
        .module_extensions
        .get("//:ext.bzl%ext")
        .unwrap()
        .general
        .as_ref()
        .unwrap();
    std::collections::BTreeMap::from([(
        "//:ext.bzl%ext".to_owned(),
        general.generated_repo_specs.clone(),
    )])
}

fn simple_visible_lockfile() -> slug_bzlmod_v2::BazelLockfile {
    parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  },
  "selectedYankedVersions": {},
  "moduleExtensions": {},
  "facts": {}
}"#,
    )
    .unwrap()
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".codex-cargo-target")
        .join("slug_bzlmod_v2_tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
