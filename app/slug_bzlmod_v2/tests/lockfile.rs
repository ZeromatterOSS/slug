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

use slug_bzlmod_v2::BAZEL_9_LOCK_FILE_VERSION;
use slug_bzlmod_v2::BzlmodHiddenLockfileDigest;
use slug_bzlmod_v2::BzlmodVisibleLockfileDigest;
use slug_bzlmod_v2::HiddenLockfileInput;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::LockfileParseErrorSurface;
use slug_bzlmod_v2::LockfileReadInputs;
use slug_bzlmod_v2::LockfileRenderErrorKind;
use slug_bzlmod_v2::RegistryFileExpectation;
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

fn extension_fields(recorded_inputs: &str, metadata: &str) -> String {
    format!(
        r#"{{
          "bzlTransitiveDigest": "AQ==",
          "usagesDigest": "AgM=",
          "recordedInputs": {recorded_inputs},
          "generatedRepoSpecs": {{
            "repo": {{
              "repoRuleId": "//:ext.bzl%repo_rule",
              "attributes": {{
                "label": "@@subject+//:probe",
                "number": 4294967297,
                "sequence": ["z", "a"]
              }}
            }}
          }}{metadata}
        }}"#
    )
}

fn comprehensive_source() -> String {
    let general = extension_fields(
        r#"[
            "ENV:LOCKFILE_ENV value",
            "FILE:@@//input.txt file-digest",
            "DIRENTS:@@//dir entries-digest",
            "DIRTREE:@@//tree?/../excludes=a+b,c%2Bd tree-digest",
            "REPO_MAPPING:,subject subject+"
          ]"#,
        r#",
          "moduleExtensionMetadata": {
            "explicitRootModuleDirectDeps": ["repo"],
            "explicitRootModuleDirectDevDeps": [],
            "useAllRepos": "NO",
            "reproducible": false
          }"#,
    );
    let platform = extension_fields("[]", "");
    format!(
        r#"{{
  "lockFileVersion": 28,
  "registryFileHashes": {{
    "sha": "{}",
    "missing": "not found"
  }},
  "selectedYankedVersions": {{
    "subject@1.0.0+discarded": "schema oracle"
  }},
  "moduleExtensions": {{
    "@@//:ext.bzl%schema": {{
      "arch:amd64,os:linux": {general},
      "general": {platform}
    }}
  }},
  "facts": {{
    "@@//:ext.bzl%schema": {{
      "z": {{"nested": [{{"b": 2, "a": 1}}, true, null]}},
      "a": "first"
    }}
  }},
  "factsVersions": {{
    "@@//:ext.bzl%schema": 7
  }}
}}"#,
        "AB".repeat(32),
    )
}

#[test]
fn lockfile_public_owner_renders_comprehensive_extension_and_facts_exactly() {
    let value = parse_bazel_lockfile(&comprehensive_source()).unwrap();
    let rendered = render_bazel_lockfile(&value).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "{\n",
            "  \"lockFileVersion\": 28,\n",
            "  \"registryFileHashes\": {\n",
            "    \"missing\": \"not found\",\n",
            "    \"sha\": \"SHA_LOWER\"\n",
            "  },\n",
            "  \"selectedYankedVersions\": {\n",
            "    \"subject@1.0.0\": \"schema oracle\"\n",
            "  },\n",
            "  \"moduleExtensions\": {\n",
            "    \"//:ext.bzl%schema\": {\n",
            "      \"general\": {\n",
            "        \"bzlTransitiveDigest\": \"AQ==\",\n",
            "        \"usagesDigest\": \"AgM=\",\n",
            "        \"recordedInputs\": [],\n",
            "        \"generatedRepoSpecs\": {\n",
            "          \"repo\": {\n",
            "            \"repoRuleId\": \"@@//:ext.bzl%repo_rule\",\n",
            "            \"attributes\": {\n",
            "              \"label\": \"@@subject+//:probe\",\n",
            "              \"number\": 1,\n",
            "              \"sequence\": [\n",
            "                \"z\",\n",
            "                \"a\"\n",
            "              ]\n",
            "            }\n",
            "          }\n",
            "        }\n",
            "      },\n",
            "      \"os:linux,arch:amd64\": {\n",
            "        \"bzlTransitiveDigest\": \"AQ==\",\n",
            "        \"usagesDigest\": \"AgM=\",\n",
            "        \"recordedInputs\": [\n",
            "          \"ENV:LOCKFILE_ENV value\",\n",
            "          \"FILE:@@//input.txt file-digest\",\n",
            "          \"DIRENTS:@@//dir entries-digest\",\n",
            "          \"DIRTREE:@@//tree?/../excludes=a+b,c%2Bd tree-digest\",\n",
            "          \"REPO_MAPPING:,subject subject+\"\n",
            "        ],\n",
            "        \"generatedRepoSpecs\": {\n",
            "          \"repo\": {\n",
            "            \"repoRuleId\": \"@@//:ext.bzl%repo_rule\",\n",
            "            \"attributes\": {\n",
            "              \"label\": \"@@subject+//:probe\",\n",
            "              \"number\": 1,\n",
            "              \"sequence\": [\n",
            "                \"z\",\n",
            "                \"a\"\n",
            "              ]\n",
            "            }\n",
            "          }\n",
            "        },\n",
            "        \"moduleExtensionMetadata\": {\n",
            "          \"explicitRootModuleDirectDeps\": [\n",
            "            \"repo\"\n",
            "          ],\n",
            "          \"explicitRootModuleDirectDevDeps\": [],\n",
            "          \"useAllRepos\": \"NO\",\n",
            "          \"reproducible\": false\n",
            "        }\n",
            "      }\n",
            "    }\n",
            "  },\n",
            "  \"facts\": {\n",
            "    \"//:ext.bzl%schema\": {\n",
            "      \"a\": \"first\",\n",
            "      \"z\": {\n",
            "        \"nested\": [\n",
            "          {\n",
            "            \"a\": 1,\n",
            "            \"b\": 2\n",
            "          },\n",
            "          true,\n",
            "          null\n",
            "        ]\n",
            "      }\n",
            "    }\n",
            "  },\n",
            "  \"factsVersions\": {\n",
            "    \"//:ext.bzl%schema\": 7\n",
            "  }\n",
            "}\n"
        )
        .replace("SHA_LOWER", &"ab".repeat(32))
    );
    assert_eq!(parse_bazel_lockfile(&rendered).unwrap(), value);
    for recorded in ["ENV:", "FILE:", "DIRENTS:", "DIRTREE:", "REPO_MAPPING:"] {
        assert!(
            rendered.contains(recorded),
            "{recorded} missing from {rendered}"
        );
    }
    assert!(rendered.contains("\"general\""));
    assert!(rendered.contains("\"os:linux,arch:amd64\""));
}

#[test]
fn lockfile_nonrenderable_recorded_input_and_repo_rule_holes_keep_typed_errors() {
    let bad_input = comprehensive_source().replace("\"ENV:LOCKFILE_ENV value\",", "\"bad\",");
    let input_error =
        render_bazel_lockfile(&parse_bazel_lockfile(&bad_input).unwrap()).unwrap_err();
    assert_eq!(
        input_error.kind(),
        LockfileRenderErrorKind::RecordedInputParseFailureSentinel
    );

    let null_rule = comprehensive_source().replace(
        "\"repoRuleId\": \"//:ext.bzl%repo_rule\"",
        "\"repoRuleId\": \"rule\"",
    );
    let rule_error = render_bazel_lockfile(&parse_bazel_lockfile(&null_rule).unwrap()).unwrap_err();
    assert_eq!(
        rule_error.kind(),
        LockfileRenderErrorKind::RepoRuleIdWithoutLabel
    );
}

#[test]
fn lockfile_every_extension_factor_metadata_fact_and_version_affects_planning() {
    let baseline = comprehensive_source();
    let desired = parse_bazel_lockfile(&baseline).unwrap();
    let variants = [
        baseline.replace("\"general\": {", "\"os:mac\": {"),
        baseline.replace("\"arch:amd64,os:linux\"", "\"arch:arm64,os:linux\""),
        baseline.replace("\"arch:amd64,os:linux\"", "\"os:mac\""),
        baseline.replace(
            "\"explicitRootModuleDirectDeps\": [\"repo\"]",
            "\"explicitRootModuleDirectDeps\": [\"other\"]",
        ),
        baseline.replace(
            "\"explicitRootModuleDirectDevDeps\": []",
            "\"explicitRootModuleDirectDevDeps\": [\"dev\"]",
        ),
        baseline.replace("\"useAllRepos\": \"NO\"", "\"useAllRepos\": \"REGULAR\""),
        baseline.replace("\"reproducible\": false", "\"reproducible\": true"),
        baseline.replace("\"a\": \"first\"", "\"a\": \"changed\""),
        baseline.replace("%schema\": 7", "%schema\": 8"),
    ];

    for variant in variants {
        let parsed = parse_bazel_lockfile(&variant).unwrap();
        assert_ne!(parsed, desired);
        assert!(matches!(
            plan_visible_lockfile(&LockfileMode::Update, Some(&variant), &desired).unwrap(),
            VisibleLockfilePlan::Write { .. }
        ));
        assert!(matches!(
            plan_visible_lockfile(&LockfileMode::Error, Some(&variant), &desired).unwrap(),
            VisibleLockfilePlan::Error { .. }
        ));
    }
}

#[test]
fn lockfile_semantic_planning_prunes_formatting_and_restores_a_b_a() {
    let source_a = comprehensive_source();
    let desired_a = parse_bazel_lockfile(&source_a).unwrap();
    let canonical_a = render_bazel_lockfile(&desired_a).unwrap();
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Update, Some(&source_a), &desired_a).unwrap(),
        VisibleLockfilePlan::Keep
    );

    let source_b = source_a.replace("\"a\": \"first\"", "\"a\": \"middle\"");
    let desired_b = parse_bazel_lockfile(&source_b).unwrap();
    assert!(matches!(
        plan_visible_lockfile(&LockfileMode::Update, Some(&canonical_a), &desired_b).unwrap(),
        VisibleLockfilePlan::Write { .. }
    ));
    assert_eq!(
        plan_visible_lockfile(
            &LockfileMode::Update,
            Some(&render_bazel_lockfile(&desired_b).unwrap()),
            &desired_a,
        )
        .unwrap(),
        VisibleLockfilePlan::Write {
            content: canonical_a.clone()
        }
    );
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Update, Some(&canonical_a), &desired_a).unwrap(),
        VisibleLockfilePlan::Keep
    );

    let empty = empty_bazel_lockfile();
    assert_eq!(
        plan_visible_lockfile(&LockfileMode::Update, Some("{}"), &empty).unwrap(),
        VisibleLockfilePlan::Keep
    );
    assert!(
        plan_visible_lockfile(
            &LockfileMode::Update,
            Some("{\"lockFileVersion\":28, nope"),
            &desired_a,
        )
        .is_err()
    );
}

#[test]
fn lockfile_registry_expectation_projects_absent_not_found_and_decoded_sha() {
    let lockfile = parse_bazel_lockfile(&comprehensive_source()).unwrap();
    assert_eq!(
        lockfile.registry_file_expectation("unrecorded").unwrap(),
        RegistryFileExpectation::Unrecorded
    );
    assert_eq!(
        lockfile.registry_file_expectation("missing").unwrap(),
        RegistryFileExpectation::RecordedAbsent
    );
    assert_eq!(
        lockfile.registry_file_expectation("sha").unwrap(),
        RegistryFileExpectation::RecordedSha256([0xab; 32])
    );
}

#[test]
fn lockfile_visible_modes_use_raw_bytes_and_preserve_error_surfaces() {
    let read = |mode, bytes: &[u8]| {
        let input = VisibleLockfileInput::from_optional_bytes(Some(bytes)).unwrap();
        parse_visible_lockfile_for_mode(&mode, &input)
    };
    let empty = VisibleLockfileRead::Parsed(empty_bazel_lockfile().into());

    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        assert_eq!(
            parse_visible_lockfile_for_mode(&mode, &VisibleLockfileInput::absent()).unwrap(),
            empty
        );
        assert_eq!(read(mode, b"{\"lockFileVersion\":28}").unwrap(), empty);
    }
    assert_eq!(
        read(
            LockfileMode::Update,
            b"{\"unknown\":\"\xff\",\"lockFileVersion\":28}"
        )
        .unwrap(),
        empty
    );
    let leading_zero_current = br#"{"decoy":{"lockFileVersion":028},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#;
    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        let VisibleLockfileRead::Parsed(value) = read(mode, leading_zero_current).unwrap() else {
            panic!("active mode must parse a current leading-zero marker");
        };
        assert_eq!(
            value.registry_file_expectation("u").unwrap(),
            RegistryFileExpectation::RecordedAbsent
        );
    }
    let leading_zero_noncurrent =
        br#"{"decoy":{"lockFileVersion":027},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#;
    for mode in [LockfileMode::Update, LockfileMode::Refresh] {
        assert_eq!(read(mode, leading_zero_noncurrent).unwrap(), empty);
    }
    assert!(
        read(LockfileMode::Error, leading_zero_noncurrent)
            .unwrap_err()
            .contains("version of MODULE.bazel.lock is not supported")
    );
    assert_eq!(
        read(LockfileMode::Refresh, b"{\"lockFileVersion\":27, nope").unwrap(),
        empty
    );
    assert_eq!(
        read(LockfileMode::Update, b"{\"lockFileVersion\":27, nope").unwrap(),
        empty
    );
    assert!(
        read(LockfileMode::Error, b"{\"lockFileVersion\":27, nope")
            .unwrap_err()
            .contains("version of MODULE.bazel.lock is not supported")
    );
    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        assert!(
            read(mode.clone(), b"{\"lockFileVersion\":28, nope")
                .unwrap_err()
                .contains("Failed to read and parse the MODULE.bazel.lock file")
        );
        assert!(
            read(mode, b"{\"lockFileVersion\":2147483648}")
                .unwrap_err()
                .contains("Failed to read and parse the MODULE.bazel.lock file")
        );
    }
    assert_eq!(
        read(LockfileMode::Off, b"{\"lockFileVersion\":28, nope").unwrap(),
        VisibleLockfileRead::Ignored
    );
}

#[test]
fn lockfile_hidden_read_fails_open_only_for_caught_surfaces() {
    assert_eq!(
        parse_hidden_lockfile_fail_open(None).unwrap(),
        empty_bazel_lockfile()
    );
    assert_eq!(
        parse_hidden_lockfile_fail_open(Some(b"{\"lockFileVersion\":27, nope")).unwrap(),
        empty_bazel_lockfile()
    );
    assert_eq!(
        parse_hidden_lockfile_fail_open(Some(b"{\"lockFileVersion\":28, nope")).unwrap(),
        empty_bazel_lockfile()
    );
    let leading_zero_current = parse_hidden_lockfile_fail_open(Some(
        br#"{"decoy":{"lockFileVersion":028},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#,
    ))
    .unwrap();
    assert_eq!(
        leading_zero_current.registry_file_expectation("u").unwrap(),
        RegistryFileExpectation::RecordedAbsent
    );
    assert_eq!(
        parse_hidden_lockfile_fail_open(Some(
            br#"{"decoy":{"lockFileVersion":027},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#,
        ))
        .unwrap(),
        empty_bazel_lockfile()
    );

    let direct = parse_hidden_lockfile_fail_open(Some(
        b"{\"lockFileVersion\":28,\"registryFileHashes\":{\"u\":\"bad\"}}",
    ))
    .unwrap_err();
    assert_eq!(
        direct.surface(),
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
    let delimiter = parse_hidden_lockfile_fail_open(Some(
        br#"{"lockFileVersion":28,"moduleExtensions":{"//:ext.bzl":{"general":{}}}}"#,
    ))
    .unwrap_err();
    assert_eq!(
        delimiter.surface(),
        LockfileParseErrorSurface::DelimiterIndexOutOfBounds
    );
}

#[test]
fn lockfile_raw_inputs_digest_exact_bytes_and_hidden_replaces_invalid_utf8() {
    let bytes = b"{\"unknown\":\"\xff\",\"lockFileVersion\":28}";
    let visible = VisibleLockfileInput::from_optional_bytes(Some(bytes)).unwrap();
    assert_eq!(
        visible.digest(),
        &BzlmodVisibleLockfileDigest::from_content(bytes)
    );
    let hidden = HiddenLockfileInput::from_optional_bytes(Some(bytes)).unwrap();
    assert_eq!(
        hidden.digest(),
        &BzlmodHiddenLockfileDigest::from_content(bytes)
    );
    assert_eq!(hidden.parse_fail_open().unwrap(), empty_bazel_lockfile());
}

#[test]
fn lockfile_read_snapshot_preserves_off_and_hidden_behavior() {
    let malformed = b"{\"lockFileVersion\":28, nope";
    let off = LockfileReadInputs {
        mode: LockfileMode::Off,
        visible: VisibleLockfileInput::from_optional_bytes(Some(malformed)).unwrap(),
        hidden: HiddenLockfileInput::from_optional_bytes(Some(malformed)).unwrap(),
    }
    .read()
    .unwrap();
    assert_eq!(off.visible, VisibleLockfileRead::Ignored);
    assert_eq!(off.hidden, None);

    let update = LockfileReadInputs {
        mode: LockfileMode::Update,
        visible: VisibleLockfileInput::absent(),
        hidden: HiddenLockfileInput::from_optional_bytes(Some(malformed)).unwrap(),
    }
    .read()
    .unwrap();
    assert_eq!(
        update.visible,
        VisibleLockfileRead::Parsed(empty_bazel_lockfile().into())
    );
    assert_eq!(update.hidden, Some(empty_bazel_lockfile()));
}

#[test]
fn lockfile_atomic_apply_writes_only_write_plans_and_errors_never_write() {
    let dir = scratch_dir("visible-lockfile-apply");
    let lockfile_path = dir.join("MODULE.bazel.lock");
    let content = render_bazel_lockfile(&empty_bazel_lockfile()).unwrap();

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
                content: content.clone()
            },
        )
        .unwrap(),
        VisibleLockfileApply::Written {
            bytes: content.len()
        }
    );
    assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), content);

    let before = fs::read_to_string(&lockfile_path).unwrap();
    let error = apply_visible_lockfile_plan(
        &lockfile_path,
        &VisibleLockfilePlan::Error {
            message: "stale".to_owned(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "stale");
    assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), before);
}

#[test]
fn lockfile_version_constant_matches_the_sole_owner() {
    assert_eq!(BAZEL_9_LOCK_FILE_VERSION, 28);
    assert_eq!(
        parse_bazel_lockfile(r#"{"lockFileVersion":28}"#)
            .unwrap()
            .lock_file_version(),
        BAZEL_9_LOCK_FILE_VERSION
    );
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let root = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
        });
    let dir = root
        .join(".codex-cargo-target")
        .join("slug_bzlmod_v2_tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
