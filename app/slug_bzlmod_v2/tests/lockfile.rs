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
use slug_bzlmod_v2::BzlmodVisibleLockfileDigest;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::VisibleLockfileApply;
use slug_bzlmod_v2::VisibleLockfileInput;
use slug_bzlmod_v2::VisibleLockfilePlan;
use slug_bzlmod_v2::apply_visible_lockfile_plan;
use slug_bzlmod_v2::parse_bazel_lockfile;
use slug_bzlmod_v2::plan_visible_lockfile;
use slug_bzlmod_v2::render_bazel_lockfile;
use slug_bzlmod_v2::validate_lockfile_version;
use slug_bzlmod_v2::validate_module_extension_bzl_transitive_digests;
use slug_bzlmod_v2::validate_module_extension_recorded_env_inputs;
use slug_bzlmod_v2::validate_module_extension_recorded_file_inputs;
use slug_bzlmod_v2::validate_module_extension_usage_digests;
use slug_bzlmod_v2::validate_registry_file_hashes;
use slug_bzlmod_v2::validate_required_registry_file_hashes;

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
fn parses_module_extension_recorded_env_inputs() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
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
  "lockFileVersion": 26,
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
  "lockFileVersion": 26,
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
fn validates_supported_lockfile_version() {
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 26}"#).unwrap();

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
fn validates_required_registry_file_hash_entries() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_shell/0.6.1/MODULE.bazel": "wanted"
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
    let lockfile = parse_bazel_lockfile(r#"{"lockFileVersion": 26}"#).unwrap();

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

#[test]
fn renders_visible_lockfile_with_bazel_top_level_shape() {
    let lockfile = parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "module-digest",
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/source.json": "source-digest"
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
            "  \"lockFileVersion\": 26,\n",
            "  \"registryFileHashes\": {\n",
            "    \"https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel\": \"module-digest\",\n",
            "    \"https://bcr.bazel.build/modules/rules_cc/0.2.17/source.json\": \"source-digest\"\n",
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
  "lockFileVersion": 26,
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
            "  \"lockFileVersion\": 26,\n",
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

    let stale = rendered.replace("module-digest", "old-digest");
    let stale_plan = plan_visible_lockfile(&LockfileMode::Error, Some(&stale), &desired).unwrap();
    assert!(matches!(stale_plan, VisibleLockfilePlan::Error { .. }));
    if let VisibleLockfilePlan::Error { message } = stale_plan {
        assert!(message.contains("MODULE.bazel.lock is no longer up-to-date"));
        assert!(message.contains("--lockfile_mode=update"));
    }

    let unsupported = rendered.replace("\"lockFileVersion\": 26", "\"lockFileVersion\": 25");
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

    let updated = rendered.replace("module-digest", "updated-digest");
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

    let existing = b"{\"lockFileVersion\":26}\n";
    let present = VisibleLockfileInput::from_optional_bytes(Some(existing)).unwrap();
    assert_eq!(
        present.digest(),
        &BzlmodVisibleLockfileDigest::from_content(existing)
    );
    assert_eq!(
        present.existing_content(),
        Some("{\"lockFileVersion\":26}\n")
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

fn simple_visible_lockfile() -> slug_bzlmod_v2::BazelLockfile {
    parse_bazel_lockfile(
        r#"{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel": "module-digest"
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
