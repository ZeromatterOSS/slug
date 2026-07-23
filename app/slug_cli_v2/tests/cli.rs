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
fn siblings_build_file_node_fixture_matches_all_oracle_rows() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-siblings-build-file-node/workspace");
    const MODERN: &str = "//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n";
    const MODERN_FULL: &str = "//modern:rule\n//modern:leaf\n//modern:implicit.txt\n//modern:explicit.txt\n//modern:cycle_b\n//modern:cycle_a\n//modern:custom\n//modern:alias\n//modern:BUILD.bazel\n";
    let successful = [
        (
            vec!["query", "//modern:BUILD.bazel"],
            "//modern:BUILD.bazel\n",
        ),
        (vec!["query", "//fallback:BUILD"], "//fallback:BUILD\n"),
        (
            vec!["query", "siblings(//fallback:BUILD)"],
            "//fallback:BUILD\n//fallback:only\n",
        ),
        (vec!["query", "//:BUILD.bazel"], "//:BUILD.bazel\n"),
        (
            vec!["query", "siblings(//:BUILD.bazel)"],
            "//:BUILD.bazel\n//:root_rule\n",
        ),
        (vec!["query", "//dual:preferred"], "//dual:preferred\n"),
        (vec!["query", "//dual:BUILD.bazel"], "//dual:BUILD.bazel\n"),
        (
            vec!["query", "siblings(//dual:BUILD.bazel)"],
            "//dual:BUILD.bazel\n//dual:preferred\n",
        ),
        (vec!["query", "siblings(//modern:BUILD.bazel)"], MODERN),
        (vec!["query", "siblings(//modern:rule)"], MODERN),
        (vec!["query", "siblings(//modern:explicit.txt)"], MODERN),
        (vec!["query", "siblings(//modern:implicit.txt)"], MODERN),
        (vec!["query", "siblings(//modern:alias)"], MODERN),
        (vec!["query", "siblings(//modern:custom)"], MODERN),
        (
            vec!["query", "siblings(//namedall:BUILD.bazel)"],
            "//namedall:BUILD.bazel\n//namedall:all\n//namedall:other\n",
        ),
        (
            vec![
                "query",
                "siblings(set(//modern:rule //modern:alias //modern:rule))",
            ],
            MODERN,
        ),
        (
            vec!["query", "siblings(set(//modern:rule //fallback:only))"],
            "//fallback:BUILD\n//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (vec!["query", "siblings(set())"], ""),
        (
            vec!["query", "siblings(//modern:rule) union //fallback:only"],
            "//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec!["query", "siblings(set(//modern:rule //fallback:only))"],
            "//fallback:BUILD\n//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec![
                "query",
                "siblings(//modern:rule) intersect set(//modern:BUILD.bazel //modern:rule //fallback:only)",
            ],
            "//modern:BUILD.bazel\n//modern:rule\n",
        ),
        (
            vec![
                "query",
                "siblings(//modern:rule) except set(//modern:BUILD.bazel //modern:implicit.txt)",
            ],
            "//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec!["query", "deps(//modern:BUILD.bazel)"],
            "//modern:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "rdeps(siblings(//modern:rule), //modern:BUILD.bazel)",
            ],
            "//modern:BUILD.bazel\n",
        ),
        (
            vec!["query", "same_pkg_direct_rdeps(//modern:BUILD.bazel)"],
            "",
        ),
        (
            vec!["query", "--order_output=auto", "siblings(//modern:rule)"],
            MODERN,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//modern:rule)"],
            MODERN_FULL,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//modern:cycle_a)"],
            MODERN_FULL,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//provenance:a)"],
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "--order_output=full",
                "siblings(//provenance:a) union set()",
            ],
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "--order_output=full",
                "siblings(deps(//provenance:a))",
            ],
            "//provenance:z\n//provenance:y\n//provenance:a\n//provenance:zz\n//provenance:BUILD.bazel\n",
        ),
    ];
    let failures = [
        (
            vec!["query", "//modern:BUILD"],
            7,
            "no such target '//modern:BUILD'",
        ),
        (
            vec!["query", "//fallback:BUILD.bazel"],
            7,
            "no such target '//fallback:BUILD.bazel'",
        ),
        (vec!["query", "//:BUILD"], 7, "no such target '//:BUILD'"),
        (
            vec!["query", "//dual:BUILD"],
            7,
            "no such target '//dual:BUILD'",
        ),
        (
            vec!["query", "//dual:ignored"],
            7,
            "no such target '//dual:ignored'",
        ),
        (
            vec!["query", "siblings()"],
            2,
            "too few arguments to function 'siblings'",
        ),
        (
            vec!["query", "siblings(//modern:rule, //fallback:only)"],
            2,
            "too many arguments to function 'siblings'",
        ),
        (vec!["query", "siblings("], 2, "premature end of input"),
        (
            vec!["query", "siblings(//modern:missing)"],
            7,
            "no such target '//modern:missing'",
        ),
        (
            vec!["query", "siblings(//missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
        (
            vec!["query", "siblings(//modern:rule union //modern:missing)"],
            7,
            "no such target '//modern:missing'",
        ),
        (
            vec!["query", "siblings(//modern:rule union //missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
    ];
    assert_eq!(successful.len() + failures.len(), 43);

    for (argv, expected_stdout) in successful {
        let output = slug().current_dir(&workspace).args(&argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_exit, expected_message) in failures {
        let output = slug().current_dir(&workspace).args(&argv).output().unwrap();
        assert_eq!(output.status.code(), Some(expected_exit), "{argv:?}");
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected_message), "{argv:?}: {stderr}");
    }
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
fn reverse_query_fixture_matches_all_twenty_six_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-rdeps-and-subtree-patterns/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "//tree/left/..."],
            "//tree/left:cross_only\n//tree/left:custom_parent\n//tree/left:cycle_a\n//tree/left:cycle_b\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:via_alias\n//tree/left/nested:nested\n",
        ),
        (&["query", "//nonpackage/..."], "//nonpackage/desc:desc\n"),
        (
            &["query", "rdeps(//..., //tree/left:source.txt)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 0)"],
            "//tree/left:source.txt\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 1)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 2)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &[
                "query",
                "rdeps(//tree/right:right_parent, //tree/left:source.txt)",
            ],
            "",
        ),
        (
            &[
                "query",
                "rdeps(set(//tree/left:parent_one //tree/right:right_parent), //tree/left:source.txt)",
            ],
            "//tree/left:parent_one\n//tree/left:source.txt\n",
        ),
        (
            &[
                "query",
                "rdeps(//..., set(//tree/left:source.txt //tree/left:source.txt))",
            ],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//tree/left:cycle_a, //tree/left:cycle_b)"],
            "//tree/left:cycle_a\n//tree/left:cycle_b\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:leaf)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "rdeps(//tree/right:right_parent, //tree/left:leaf)",
            ],
            "",
        ),
        (
            &["query", "rdeps(//tree/..., //tree/left:leaf)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "rdeps(//tree/..., //tree/left:leaf)",
            ],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "--order_output=full",
                "rdeps(//tree/..., //tree/left:leaf)",
            ],
            "//tree/left:custom_parent\n//tree/left:via_alias\n//tree/left:leaf\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:source.txt)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n",
        ),
        (
            &[
                "query",
                "same_pkg_direct_rdeps(set(//tree/left:source.txt //tree/left:source.txt))",
            ],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:leaf)"],
            "//tree/left:via_alias\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:via_alias)"],
            "//tree/left:custom_parent\n",
        ),
        (
            &[
                "query",
                "same_pkg_direct_rdeps(set(//tree/left:source.txt //tree/right:right_source.txt))",
            ],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/right:right_both\n//tree/right:right_parent\n",
        ),
    ];
    for (args, expected) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected,
            "{args:?}"
        );
    }

    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "//empty/..."],
            7,
            "no targets found beneath 'empty'",
        ),
        (
            &["query", "//missing/..."],
            7,
            "no targets found beneath 'missing'",
        ),
        (
            &["query", "rdeps(//..., //tree/left:leaf, 1, 2)"],
            2,
            "too many arguments to function 'rdeps'",
        ),
        (
            &["query", "rdeps(//..., 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "same_pkg_direct_rdeps()"],
            2,
            "too few arguments to function 'same_pkg_direct_rdeps'",
        ),
        (
            &["query", "same_pkg_direct_rdeps(1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
    ];
    for (args, exit, message) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(output.status.code(), Some(*exit), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn path_topology_fixture_covers_all_forty_three_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-path-topology/workspace");
    const LINEAR_ALL_AUTO: &str = "//:linear_end\n//:linear_mid\n//:linear_start\n";
    const LINEAR_FORWARD: &str = "//:linear_start\n//:linear_mid\n//:linear_end\n";
    const EMPTY: &str = "";
    let successes: &[(&[&str], &[&str])] = &[
        (
            &["query", "allpaths(//:linear_start, //:linear_end)"],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "allpaths(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "allpaths(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "somepath(//:linear_start, //:linear_end)"],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "somepath(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, //:linear_end) union //:disconnected",
            ],
            &["//:disconnected\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "somepath(//:linear_start, //:linear_end) union //:disconnected",
            ],
            &["//:disconnected\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "allpaths(//:diamond_start, //:diamond_end)"],
            &[
                "//:diamond_end\n//:diamond_left\n//:diamond_right\n//:diamond_split\n//:diamond_start\n",
            ],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:diamond_start, //:diamond_end)",
            ],
            &[
                "//:diamond_start\n//:diamond_split\n//:diamond_left\n//:diamond_end\n",
                "//:diamond_start\n//:diamond_split\n//:diamond_right\n//:diamond_end\n",
            ],
        ),
        (
            &["query", "allpaths(//:cycle_a, //:cycle_end)"],
            &["//:cycle_a\n//:cycle_b\n//:cycle_end\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:cycle_a, //:cycle_end)",
            ],
            &["//:cycle_a\n//:cycle_b\n//:cycle_end\n"],
        ),
        (
            &["query", "allpaths(//:linear_mid, //:linear_mid)"],
            &["//:linear_mid\n"],
        ),
        (
            &["query", "somepath(//:linear_mid, //:linear_mid)"],
            &["//:linear_mid\n"],
        ),
        (
            &["query", "allpaths(//:linear_start, //:disconnected)"],
            &[EMPTY],
        ),
        (
            &["query", "somepath(//:linear_start, //:disconnected)"],
            &[EMPTY],
        ),
        (&["query", "allpaths(set(), //:linear_end)"], &[EMPTY]),
        (&["query", "allpaths(//:linear_start, set())"], &[EMPTY]),
        (&["query", "somepath(set(), //:linear_end)"], &[EMPTY]),
        (&["query", "somepath(//:linear_start, set())"], &[EMPTY]),
        (
            &[
                "query",
                "allpaths(set(//:multi_a //:multi_b), set(//:multi_end_a //:multi_end_b))",
            ],
            &["//:multi_a\n//:multi_b\n//:multi_end_a\n//:multi_end_b\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(set(//:one_pair_origin //:one_pair_other_origin), set(//:one_pair_destination //:one_pair_other_destination))",
            ],
            &["//:one_pair_origin\n//:one_pair_destination\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(set(//:ambiguous_a //:ambiguous_b), set(//:ambiguous_a_end //:ambiguous_b_end))",
            ],
            &[
                "//:ambiguous_a\n//:ambiguous_a_left\n//:ambiguous_a_end\n",
                "//:ambiguous_a\n//:ambiguous_a_right\n//:ambiguous_a_end\n",
                "//:ambiguous_b\n//:ambiguous_b_left\n//:ambiguous_b_end\n",
                "//:ambiguous_b\n//:ambiguous_b_right\n//:ambiguous_b_end\n",
            ],
        ),
        (
            &[
                "query",
                "allpaths(//:linear_start, set(//:linear_end //:disconnected))",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, set(//:linear_end //:disconnected))",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "allpaths(set(//:linear_start //:linear_start), set(//:linear_end //:linear_end))",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "somepath(set(//:linear_start //:linear_start), set(//:linear_end //:linear_end))",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "allpaths(//:source_parent, //:source.txt)"],
            &["//:source.txt\n//:source_parent\n"],
        ),
        (
            &["query", "somepath(//:source_parent, //:source.txt)"],
            &["//:source_parent\n//:source.txt\n"],
        ),
        (
            &["query", "allpaths(//:source.txt, //:source_parent)"],
            &[EMPTY],
        ),
        (
            &["query", "somepath(//:source.txt, //:source_parent)"],
            &[EMPTY],
        ),
        (
            &["query", "allpaths(//:alias_start, //:linear_end)"],
            &["//:alias_start\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &["query", "somepath(//:alias_start, //:linear_end)"],
            &["//:alias_start\n//:linear_start\n//:linear_mid\n//:linear_end\n"],
        ),
        (
            &["query", "allpaths(//:custom_start, //:custom_end)"],
            &["//:custom_end\n//:custom_mid\n//:custom_start\n"],
        ),
        (
            &["query", "somepath(//:custom_start, //:custom_end)"],
            &["//:custom_start\n//:custom_mid\n//:custom_end\n"],
        ),
    ];
    for (args, alternatives) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(
            alternatives.contains(&stdout),
            "{args:?}: unexpected complete-path alternative {stdout:?}"
        );
    }

    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "allpaths()"],
            2,
            "too few arguments to function 'allpaths'",
        ),
        (
            &[
                "query",
                "allpaths(//:linear_start, //:linear_end, //:linear_mid)",
            ],
            2,
            "too many arguments to function 'allpaths'",
        ),
        (
            &["query", "somepath()"],
            2,
            "too few arguments to function 'somepath'",
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, //:linear_end, //:linear_mid)",
            ],
            2,
            "too many arguments to function 'somepath'",
        ),
        (
            &["query", "allpaths(1, //:linear_end)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "allpaths(//:linear_start, 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "somepath(1, //:linear_end)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "somepath(//:linear_start, 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
    ];
    for (args, exit, message) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(output.status.code(), Some(*exit), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn some_selection_fixture_covers_all_forty_two_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-some-selection/workspace");
    const ONE_OF_THREE: &[&str] = &["//:zeta\n", "//:alpha\n", "//:middle\n"];
    const TWO_AUTO: &[&str] = &[
        "//:alpha\n//:middle\n",
        "//:alpha\n//:zeta\n",
        "//:middle\n//:zeta\n",
    ];
    const TWO_FULL: &[&str] = &[
        "//:alpha\n//:middle\n",
        "//:alpha\n//:zeta\n",
        "//:middle\n//:alpha\n",
        "//:middle\n//:zeta\n",
        "//:zeta\n//:alpha\n",
        "//:zeta\n//:middle\n",
    ];
    let successes: &[(&str, Option<&str>, &[&str])] = &[
        ("some(//:single)", None, &["//:single\n"]),
        ("some(//:single)", Some("auto"), &["//:single\n"]),
        ("some(//:single)", Some("full"), &["//:single\n"]),
        ("some(set(//:zeta //:alpha //:middle))", None, ONE_OF_THREE),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            Some("auto"),
            TWO_AUTO,
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            Some("full"),
            TWO_FULL,
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 3)",
            Some("auto"),
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 3)",
            Some("full"),
            &["//:zeta\n//:middle\n//:alpha\n"],
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 5)",
            None,
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        (
            "some(set(//:zeta //:zeta //:alpha), 5)",
            None,
            &["//:alpha\n//:zeta\n"],
        ),
        (
            "some(some(set(//:zeta //:alpha), 2) union some(set(//:alpha //:middle), 2), 5)",
            None,
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        ("some(//:single, 2147483647)", None, &["//:single\n"]),
        (
            "some(deps(//:cycle_a), 5)",
            None,
            &["//:cycle_a\n//:cycle_b\n"],
        ),
        (
            "some(set(//:early //other:later), 1)",
            None,
            &["//:early\n", "//other:later\n"],
        ),
        (
            "some(set(//:early //other:later), 5)",
            None,
            &["//:early\n//other:later\n"],
        ),
        (
            "some(//recursive/..., 5)",
            None,
            &["//recursive:rec_zeta\n//recursive/nested:rec_alpha\n"],
        ),
        (
            "deps(//:depth_root, 2147483647)",
            None,
            &["//:depth_child\n//:depth_root\n"],
        ),
        ("deps(//:depth_root, '-1')", None, &[""]),
        ("deps(//:depth_root, '-2147483648')", None, &[""]),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, '-1')",
            None,
            &[""],
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, '-2147483648')",
            None,
            &[""],
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, 2147483647)",
            None,
            &["//:depth_child\n//:depth_root\n"],
        ),
    ];
    let failures: &[(&str, i32, &str)] = &[
        (
            "some(//:early union //:missing_target, 1)",
            7,
            "no such target '//:missing_target'",
        ),
        (
            "some(set(//:early //:missing_target), 1)",
            7,
            "no such target '//:missing_target'",
        ),
        (
            "some(//:early union //missing:target, 1)",
            7,
            "no such package 'missing'",
        ),
        (
            "some(set(//:early //missing:target), 1)",
            7,
            "no such package 'missing'",
        ),
        ("some(set())", 7, "argument set is empty"),
        ("some(//:single, 0)", 7, "argument set is empty"),
        ("some(set(), 0)", 7, "argument set is empty"),
        ("some(//:single, '-1')", 7, "argument set is empty"),
        ("some(set(), '-1')", 7, "argument set is empty"),
        ("some(//:single, '-2147483648')", 7, "argument set is empty"),
        ("some(//:single, -1)", 2, "syntax error at '- 1 )'"),
        (
            "some(//missing:target, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
        (
            "some(//missing:target, '-2147483649')",
            2,
            "expected an integer literal: '-2147483649'",
        ),
        (
            "some(//missing:target, 2_147_483_647)",
            2,
            "expected an integer literal: '2_147_483_647'",
        ),
        (
            "some(//:single, nope)",
            2,
            "expected an integer literal: 'nope'",
        ),
        ("some()", 2, "too few arguments to function 'some'"),
        (
            "some(//:single, 1, 2)",
            2,
            "too many arguments to function 'some'",
        ),
        ("some(2147483648)", 7, "no such target '//:2147483648'"),
        (
            "deps(//:depth_root, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
    ];
    assert_eq!(successes.len() + failures.len(), 42);

    for (expression, order, alternatives) in successes {
        let mut command = slug();
        command.current_dir(&workspace).arg("query");
        if let Some(order) = order {
            command.arg(format!("--order_output={order}"));
        }
        let output = command.arg(expression).output().unwrap();
        assert!(output.status.success(), "{expression}: {output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(alternatives.contains(&stdout), "{expression}: {stdout:?}");
    }
    for (expression, exit, message) in failures {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(*exit),
            "{expression}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{expression}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{expression}: {stderr}");
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
