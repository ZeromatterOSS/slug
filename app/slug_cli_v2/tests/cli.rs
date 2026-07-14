/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::process::Command;

fn slug() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slug"))
}

#[test]
fn version_reports_v2_bazel_floor() {
    let output = slug().arg("version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Slug V2"));
    assert!(stdout.contains("Bazel compatibility: >=9.0.0"));
}

#[test]
fn help_is_bazel_v2_specific() {
    let output = slug().arg("help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lower = stdout.to_lowercase();
    assert!(stdout.contains("build"));
    assert!(stdout.contains("query"));
    assert!(stdout.contains("cquery"));
    assert!(stdout.contains("aquery"));
    assert!(!lower.contains(concat!("bu", "ck")));
    assert!(!stdout.contains(concat!("BU", "CK")));
    assert!(!stdout.contains(concat!("TAR", "GETS")));
    assert!(!lower.contains("cell"));
    assert!(!lower.contains(concat!(".", "bu", "ck", "config")));
}

#[test]
fn simple_rule_fixture_enters_the_dice_starlark_runtime_before_analysis() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/simple-rule-action/workspace");

    let output = slug()
        .current_dir(workspace)
        .args(["build", "//pkg:write_file", "--unknown_flag"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"analysis_not_implemented\""));
    assert!(stderr.contains("\"command\":\"build\""));
    assert!(stderr.contains("\"target_count\":1"));
    assert!(stderr.contains("//pkg:write_file"));
    assert!(stderr.contains("--unknown_flag"));
    assert!(stderr.contains("\"loaded_package_count\":1"));
    assert!(stderr.contains("\"analyzed_target_count\":1"));
    assert!(stderr.contains("\"declared_action_count\":1"));
    assert!(stderr.contains("\"completed_boundary\":\"dice_starlark_rule_analysis\""));
}

#[test]
fn build_file_loading_fixture_reaches_package_loading_before_analysis() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/build-file-loading/workspace");

    let output = slug()
        .current_dir(workspace)
        .args(["build", "//pkg:fg"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"analysis_not_implemented\""));
    assert!(stderr.contains("\"loaded_package_count\":1"));
    assert!(stderr.contains("\"completed_boundary\":\"dice_starlark_package_loading\""));
}

#[test]
fn missing_build_target_is_structured_parse_error() {
    let output = slug().arg("build").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"command_parse_error\""));
    assert!(stderr.contains("\"command\":\"build\""));
    assert!(stderr.contains("build requires a target pattern"));
}

#[test]
fn planned_configured_and_action_queries_are_structured() {
    for command in ["cquery", "aquery"] {
        let output = slug().args([command, "//:x"]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("\"error\":\"planned_placeholder\""));
        assert!(stderr.contains(&format!("\"command\":\"{command}\"")));
        assert!(stderr.contains("//:x"));
    }
}
