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
use std::time::SystemTime;

fn slug() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slug"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-cli-{name}-{nanos}"));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: impl AsRef<std::path::Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

struct DaemonCleanup(std::path::PathBuf);

impl Drop for DaemonCleanup {
    fn drop(&mut self) {
        let socket = slug_server_v2::socket_path(&self.0);
        if socket.exists() {
            let _ = slug_server_v2::send_shutdown(&socket);
        }
    }
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

#[test]
fn query_prints_text_labels_in_auto_and_full_order() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-parser-and-sets/workspace");
    let auto = slug()
        .current_dir(&workspace)
        .args(["query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(auto.status.success(), "{auto:?}");
    assert_eq!(
        String::from_utf8(auto.stdout).unwrap(),
        "//pkg:bin\n//pkg:data.txt\n//pkg:lib\n"
    );

    let full = slug()
        .current_dir(&workspace)
        .args(["query", "--order_output=full", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(full.status.success(), "{full:?}");
    assert_eq!(
        String::from_utf8(full.stdout).unwrap(),
        "//pkg:bin\n//pkg:lib\n//pkg:data.txt\n"
    );
}

#[test]
fn loading_query_fixture_matches_full_bazel_semantic_slice() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-loading-thin-vertical/workspace");
    let successful = [
        ("//app:app", "//app:app\n"),
        ("//lib:implicit.txt", "//lib:implicit.txt\n"),
        ("//lib:explicit.txt", "//lib:explicit.txt\n"),
        ("//lib:missing_input.txt", "//lib:missing_input.txt\n"),
        ("//lib:all", "//lib:leaf\n"),
        (
            "//...",
            "//:root\n//app:app\n//app:via_alias\n//cycle:a\n//cycle:b\n//lib:leaf\n//nested:branch\n//nested/child:child\n",
        ),
        (
            "deps(//app:via_alias)",
            "//app:via_alias\n//lib:explicit.txt\n//lib:implicit.txt\n//lib:leaf\n//lib:missing_input.txt\n",
        ),
        (
            "deps(//app:app)",
            "//app:app\n//app:via_alias\n//lib:explicit.txt\n//lib:implicit.txt\n//lib:leaf\n//lib:missing_input.txt\n//nested:branch\n//nested/child:child.txt\n",
        ),
        ("deps(//cycle:a)", "//cycle:a\n//cycle:b\n"),
        (
            "deps(//app:app) except //lib:explicit.txt intersect //...",
            "//app:app\n//app:via_alias\n//lib:leaf\n//nested:branch\n",
        ),
    ];
    for (expression, expected) in successful {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert!(output.status.success(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
    }

    for (expression, expected) in [
        (
            "//lib:missing",
            "no such target '//lib:missing': target 'missing' not declared in package 'lib'",
        ),
        (
            "//missing:target",
            "no such package 'missing': BUILD file not found",
        ),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7), "{expression}: {output:?}");
        assert!(output.stdout.is_empty(), "{expression}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{expression}: {stderr}");
    }
}

#[test]
fn output_base_query_reuses_one_daemon_across_build_edits() {
    let workspace = scratch("query-workspace");
    let output_base = scratch("query-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"one.txt\"])\n",
    );
    write(workspace.join("pkg/one.txt"), "one\n");

    let output_base_arg = format!("--output_base={}", output_base.display());
    let first = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        "//pkg:bin\n//pkg:one.txt\n"
    );
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();

    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"two.txt\"])\n",
    );
    write(workspace.join("pkg/two.txt"), "two\n");
    let second = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(second.status.success(), "{second:?}");
    assert_eq!(
        String::from_utf8(second.stdout).unwrap(),
        "//pkg:bin\n//pkg:two.txt\n"
    );
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}
