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
fn build_load_files_provenance_fixture_matches_all_fifty_seven_text_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-build-load-files-provenance/workspace");
    let successes: &[(&str, &[&str], &str)] = &[
        (
            "buildfiles_direct_primary",
            &["query", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_direct_primary",
            &["query", "loadfiles(//a:one)"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_a",
            &["query", "siblings(loadfiles(//a:one))"],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_fake_b",
            &["query", "siblings(loadfiles(//b:two))"],
            "//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_two_consumers_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) union loadfiles(//b:two))",
            ],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_two_consumers_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) union loadfiles(//a:one))",
            ],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_real_fake_order",
            &["query", "siblings(//a:one union loadfiles(//b:two))"],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_fake_real_order",
            &["query", "siblings(loadfiles(//b:two) union //a:one)"],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "real_exported_bzl_siblings",
            &["query", "siblings(//shared:one.bzl)"],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_real_fake_same_label",
            &[
                "query",
                "siblings(//shared:one.bzl union loadfiles(//a:one))",
            ],
            "//a:BUILD.bazel\n//a:one\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_real_same_label",
            &[
                "query",
                "siblings(loadfiles(//a:one) union //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_buildfiles_companions",
            &["query", "siblings(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "buildfiles_diamond",
            &["query", "buildfiles(//diamond:probe)"],
            "//diamond:BUILD.bazel\n//diamond:leaf.bzl\n//diamond:left.bzl\n//diamond:right.bzl\n",
        ),
        (
            "loadfiles_diamond",
            &["query", "loadfiles(//diamond:probe)"],
            "//diamond:leaf.bzl\n//diamond:left.bzl\n//diamond:right.bzl\n",
        ),
        (
            "buildfiles_fallback",
            &["query", "buildfiles(//fallback:only)"],
            "//fallback:BUILD\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_fallback",
            &["query", "loadfiles(//fallback:only)"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_dual_primary",
            &["query", "buildfiles(//dual:preferred)"],
            "//dual:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_root",
            &["query", "buildfiles(//:root)"],
            "//:BUILD.bazel\n",
        ),
        (
            "buildfiles_multiple_packages",
            &["query", "buildfiles(set(//a:one //fallback:only))"],
            "//a:BUILD.bazel\n//fallback:BUILD\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_multiple_packages",
            &["query", "loadfiles(set(//a:one //fallback:only))"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_duplicate_operand",
            &["query", "buildfiles(set(//a:one //a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        ("loadfiles_empty", &["query", "loadfiles(set())"], ""),
        ("buildfiles_empty", &["query", "buildfiles(set())"], ""),
        (
            "buildfiles_idempotent",
            &["query", "buildfiles(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_union",
            &["query", "buildfiles(//a:one) union //fallback:only"],
            "//a:BUILD.bazel\n//fallback:only\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_intersect",
            &[
                "query",
                "buildfiles(//a:one) intersect set(//a:BUILD.bazel //shared:one.bzl //fallback:only)",
            ],
            "//a:BUILD.bazel\n//shared:one.bzl\n",
        ),
        (
            "loadfiles_except",
            &["query", "loadfiles(//a:one) except //shared:two.bzl"],
            "//shared:one.bzl\n",
        ),
        (
            "deps_function_fake_nodes",
            &["query", "deps(loadfiles(//a:one))"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "deps_buildfiles_fake_nodes",
            &["query", "deps(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "default_direct",
            &["query", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "auto_direct",
            &["query", "--order_output=auto", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "full_direct",
            &["query", "--order_output=full", "buildfiles(//a:one)"],
            "//shared:two.bzl\n//shared:one.bzl\n//shared:BUILD.bazel\n",
        ),
        (
            "full_wrapped_union",
            &[
                "query",
                "--order_output=full",
                "buildfiles(//a:one) union set()",
            ],
            "//shared:two.bzl\n//shared:one.bzl\n//shared:BUILD.bazel\n",
        ),
        (
            "buildfiles_broken_companions",
            &["query", "buildfiles(//consumer_bad:probe)"],
            "//broken_load:BUILD.bazel\n//broken_load:good.bzl\n//broken_syntax:BUILD.bazel\n//broken_syntax:good.bzl\n//consumer_bad:BUILD.bazel\n",
        ),
        (
            "loadfiles_broken_companions",
            &["query", "loadfiles(//consumer_bad:probe)"],
            "//broken_load:good.bzl\n//broken_syntax:good.bzl\n",
        ),
        (
            "siblings_loadfiles_intersect_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) intersect loadfiles(//b:two))",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_loadfiles_intersect_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) intersect loadfiles(//a:one))",
            ],
            "//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_loadfiles_except_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) except loadfiles(//b:two))",
            ],
            "",
        ),
        (
            "siblings_loadfiles_except_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) except loadfiles(//a:one))",
            ],
            "",
        ),
        (
            "siblings_real_fake_intersect",
            &[
                "query",
                "siblings(//shared:one.bzl intersect loadfiles(//a:one))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_real_intersect",
            &[
                "query",
                "siblings(loadfiles(//a:one) intersect //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_real_fake_except",
            &[
                "query",
                "siblings(//shared:one.bzl except loadfiles(//a:one))",
            ],
            "",
        ),
        (
            "siblings_fake_real_except",
            &[
                "query",
                "siblings(loadfiles(//a:one) except //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_single_fake_real_intersect",
            &[
                "query",
                "siblings(loadfiles(//single:only) intersect //shared:two.bzl)",
            ],
            "//single:BUILD.bazel\n//single:only\n",
        ),
        (
            "siblings_single_real_fake_intersect",
            &[
                "query",
                "siblings(//shared:two.bzl intersect loadfiles(//single:only))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_single_fake_real_except",
            &[
                "query",
                "siblings(loadfiles(//single:only) except //shared:two.bzl)",
            ],
            "",
        ),
        (
            "siblings_single_real_fake_except",
            &[
                "query",
                "siblings(//shared:two.bzl except loadfiles(//single:only))",
            ],
            "",
        ),
        (
            "siblings_single_fake_real_union",
            &[
                "query",
                "siblings(loadfiles(//single:only) union //shared:two.bzl)",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n//single:BUILD.bazel\n//single:only\n",
        ),
        (
            "siblings_single_real_fake_union",
            &[
                "query",
                "siblings(//shared:two.bzl union loadfiles(//single:only))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n//single:BUILD.bazel\n//single:only\n",
        ),
    ];
    let failures: &[(&str, &[&str], i32, &str)] = &[
        (
            "missing_load_failure",
            &["query", "buildfiles(//missing_load:probe)"],
            7,
            "cannot load '//missing_load:missing.bzl': no such file",
        ),
        (
            "broken_bzl_failure",
            &["query", "loadfiles(//broken_bzl:probe)"],
            7,
            "compilation of module 'broken_bzl/bad.bzl' failed",
        ),
        (
            "bzl_cycle_failure",
            &["query", "loadfiles(//bzl_cycle:probe)"],
            7,
            "cycle detected in extension files",
        ),
        (
            "buildfiles_too_few_arguments",
            &["query", "buildfiles()"],
            2,
            "too few arguments to function 'buildfiles'",
        ),
        (
            "loadfiles_too_many_arguments",
            &["query", "loadfiles(//a:one, //b:two)"],
            2,
            "too many arguments to function 'loadfiles'",
        ),
        (
            "buildfiles_syntax_failure",
            &["query", "buildfiles("],
            2,
            "premature end of input",
        ),
        (
            "buildfiles_missing_target",
            &["query", "buildfiles(//a:missing)"],
            7,
            "no such target '//a:missing'",
        ),
        (
            "buildfiles_later_error",
            &["query", "buildfiles(//a:one union //missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
    ];
    assert_eq!(successes.len() + failures.len(), 57);

    for (name, args, expected_stdout) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{name} {args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{name} {args:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected_stdout,
            "{name} {args:?}"
        );
    }
    for (name, args, exit, diagnostic) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(*exit),
            "{name} {args:?}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{name} {args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "{name} {args:?}: {stderr}");
    }
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
            std::str::from_utf8(&output.stdout).unwrap(),
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
fn labels_metadata_fixture_matches_twenty_nine_non_label_kind_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-labels-attribute-metadata/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "labels(one, //pkg:explicit)"],
            "//pkg:source.txt\n",
        ),
        (
            &["query", "labels(many, //pkg:explicit)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &["query", "labels(with_default, //pkg:omitted)"],
            "//pkg:default.txt\n",
        ),
        (&["query", "labels(many, //pkg:empty)"], ""),
        (&["query", "labels(note, //pkg:explicit)"], ""),
        (&["query", "labels(no_such_attr, //pkg:explicit)"], ""),
        (
            &["query", "labels($implicit, //pkg:implicit)"],
            "//pkg:implicit.txt\n",
        ),
        (&["query", "labels(_implicit, //pkg:implicit)"], ""),
        (
            &["query", "labels(chosen, //pkg:selecting)"],
            "//pkg:branch_arm.txt\n//pkg:branch_default.txt\n//pkg:branch_linux.txt\n",
        ),
        (
            &["query", "labels(combined, //pkg:selecting)"],
            "//pkg:branch_arm.txt\n//pkg:branch_default.txt\n//pkg:branch_linux.txt\n//pkg:shared.txt\n",
        ),
        (&["query", "labels(out, //pkg:outputs)"], "//pkg:one.out\n"),
        (
            &["query", "labels(outs, //pkg:outputs)"],
            "//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &["query", "labels(outs, //pkg:outputs_two)"],
            "//pkg:five.out\n//pkg:six.out\n",
        ),
        (
            &["query", "deps(labels(outs, //pkg:outputs))"],
            "//pkg:outputs\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &[
                "query",
                "deps(labels(outs, //pkg:outputs) union //pkg:outputs)",
            ],
            "//pkg:outputs\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &[
                "query",
                "deps(labels(outs, //pkg:outputs) union labels(outs, //pkg:outputs_two) union //pkg:outputs union //pkg:outputs_two)",
            ],
            "//pkg:five.out\n//pkg:outputs\n//pkg:outputs_two\n//pkg:six.out\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &["query", "labels(string_labels, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &["query", "labels(label_strings, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:default.txt\n",
        ),
        (
            &["query", "labels(label_lists, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:implicit.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "labels(many, set(//pkg:source.txt //pkg:alias //pkg:BUILD.bazel))",
            ],
            "",
        ),
        (
            &[
                "query",
                "labels(many, set(//pkg:explicit //pkg:explicit //other:consumer))",
            ],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "labels(one, //pkg:explicit) union labels(with_default, //pkg:omitted)",
            ],
            "//pkg:default.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "labels(many, //pkg:explicit)",
            ],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "--order_output=full",
                "labels(many, //pkg:explicit)",
            ],
            "//pkg:source.txt\n//other:cross.txt\n",
        ),
        // The plain default-order invocation is intentionally a distinct
        // oracle row from explicit --order_output=auto.
        (
            &["query", "labels(many, //pkg:explicit)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
    ];
    let graph_rows: &[(&[&str], &str)] = &[
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(labels(outs, //pkg:outputs) union //pkg:outputs)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:three.out\\n//pkg:two.out\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\" -> \"//pkg:outputs\"\n",
                "  \"//pkg:outputs\"\n",
                "}\n",
            ),
        ),
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(labels(outs, //pkg:outputs) union labels(outs, //pkg:outputs_two) union //pkg:outputs union //pkg:outputs_two)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:six.out\\n//pkg:five.out\"\n",
                "  \"//pkg:six.out\\n//pkg:five.out\" -> \"//pkg:outputs_two\"\n",
                "  \"//pkg:outputs_two\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\" -> \"//pkg:outputs\"\n",
                "  \"//pkg:outputs\"\n",
                "}\n",
            ),
        ),
    ];
    let failures: &[(&[&str], &[&str])] = &[
        (
            &["query", "labels(many, //pkg:missing_ref)"],
            &[
                "in 'many' of rule //pkg:missing_ref: no such package 'does_not_exist'",
                "Evaluation of query",
            ],
        ),
        (
            &["query", "//broken:must"],
            &[
                "missing value for mandatory attribute 'required'",
                "Evaluation of query",
            ],
        ),
    ];
    assert_eq!(successes.len() + graph_rows.len() + failures.len(), 29);

    for (argv, expected_stdout) in successes {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_stdout) in graph_rows {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_fragments) in failures {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert_eq!(output.status.code(), Some(7), "{argv:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        for fragment in *expected_fragments {
            assert!(stderr.contains(fragment), "{argv:?}: {stderr}");
        }
    }
}

#[test]
fn executables_rule_capability_fixture_matches_all_thirty_two_non_label_kind_bazel_rows() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-executables-rule-capability/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (&["query", "executables(//pkg:plain)"], ""),
        (&["query", "executables(//pkg:ordinary_target)"], ""),
        (&["query", "executables(//pkg:explicit_test_target)"], ""),
        (
            &["query", "executables(//pkg:target_test)"],
            "//pkg:target_test\n",
        ),
        (&["query", "executables(//pkg:data.txt)"], ""),
        (
            &[
                "query",
                "executables(set(//pkg:data.txt //pkg:generated.txt //pkg:BUILD.bazel //pkg:files //pkg:alias_exec //pkg:setting))",
            ],
            "",
        ),
        (&["query", "executables(//pkg:alias_exec)"], ""),
        (&["query", "executables(//pkg:BUILD.bazel)"], ""),
        (&["query", "executables(//pkg:generated.txt)"], ""),
        (&["query", "executables(//pkg:files)"], ""),
        (&["query", "executables(//pkg:setting)"], ""),
        (&["query", "executables(set())"], ""),
        (
            &[
                "query",
                "executables(set(//pkg:arbitrary_target //pkg:arbitrary_target //pkg:target_test))",
            ],
            "//pkg:arbitrary_target\n//pkg:target_test\n",
        ),
        (
            &[
                "query",
                "executables(//pkg:arbitrary_target) union //pkg:plain",
            ],
            "//pkg:arbitrary_target\n//pkg:plain\n",
        ),
        (
            &[
                "query",
                "executables(//pkg:all) intersect set(//pkg:target_test //pkg:plain)",
            ],
            "//pkg:target_test\n",
        ),
        (
            &["query", "executables(//pkg:all) except //pkg:target_test"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n",
        ),
        (
            &["query", "let x = //pkg:all in executables($x)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "executables(executables(//pkg:all))"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "--order_output=auto", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "--order_output=full", "executables(//pkg:all)"],
            "//pkg:target_test\n//pkg:edge_exec\n//pkg:arbitrary_target\n",
        ),
        (
            &["query", "deps(executables(//pkg:edge_exec))"],
            "//pkg:edge_dep\n//pkg:edge_exec\n",
        ),
        (&["query", "executables(//test_false:probe)"], ""),
    ];
    let graph_rows: &[(&[&str], &str)] = &[
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "executables(//pkg:edge_exec)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:edge_exec\"\n",
                "}\n",
            ),
        ),
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(executables(//pkg:edge_exec))",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:edge_exec\"\n",
                "  \"//pkg:edge_exec\" -> \"//pkg:edge_dep\"\n",
                "  \"//pkg:edge_dep\"\n",
                "}\n",
            ),
        ),
    ];
    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "executables(//missing:nope)"],
            7,
            "no such package 'missing'",
        ),
        (
            &["query", "executables()"],
            2,
            "too few arguments to function 'executables'",
        ),
        (
            &["query", "executables(//pkg:plain, //pkg:arbitrary_target)"],
            2,
            "too many arguments to function 'executables'",
        ),
        (
            &[
                "query",
                "executables(//pkg:arbitrary_target union //missing:nope)",
            ],
            7,
            "no such package 'missing'",
        ),
        (
            &["query", "//invalid_not_test:probe"],
            7,
            "Invalid rule class name 'not_test_suffix', test rule class names must end with '_test' and other rule classes must not",
        ),
        (
            &["query", "//invalid_suffix_test:probe"],
            7,
            "Invalid rule class name 'suffix_test', test rule class names must end with '_test' and other rule classes must not",
        ),
    ];
    assert_eq!(successes.len() + graph_rows.len() + failures.len(), 32);

    for (argv, expected_stdout) in successes.iter().chain(graph_rows) {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_exit, diagnostic) in failures {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(*expected_exit),
            "{argv:?}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        assert!(
            std::str::from_utf8(&output.stderr)
                .unwrap()
                .contains(diagnostic),
            "{argv:?}: {output:?}"
        );
    }
}

#[test]
fn output_base_executables_reuses_one_daemon_across_capability_transitions() {
    let workspace = scratch("executables-capability-workspace");
    let output_base = scratch("executables-capability-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let definition = |name: &str, arguments: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\n\n{name} = rule(implementation = _impl{arguments})\n"
        )
    };
    let build_file = |rule: &str, target: &str| {
        format!("load(\":defs.bzl\", \"{rule}\")\n{rule}(name = \"{target}\")\n")
    };
    let output_base_arg = format!("--output_base={}", output_base.display());
    let query = |expression: &str| {
        slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap()
    };
    let assert_query = |expression: &str, expected: &str| {
        let output = query(expression);
        assert!(output.status.success(), "{expression}: {output:?}");
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
    };

    write(&defs, &definition("probe", ", executable = False"));
    write(&build, &build_file("probe", "item"));
    assert_query("executables(//pkg:item)", "");
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();

    write(&defs, &definition("probe", ", executable = True"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&defs, &definition("renamed_exec", ", executable = False"));
    assert_query("executables(//pkg:item)", "");

    write(&defs, &definition("probe_test", ", test = True"));
    write(&build, &build_file("probe_test", "item"));
    assert_query("executables(//pkg:item)", "");

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&build, &build_file("renamed_exec", "item_test"));
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");

    write(
        &build,
        "# formatting only\nload( \":defs.bzl\", \"renamed_exec\" )\nrenamed_exec( name = \"item_test\" )\n",
    );
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");

    std::fs::remove_file(&build).unwrap();
    let deleted = query("executables(//pkg:item_test)");
    assert_eq!(deleted.status.code(), Some(7), "{deleted:?}");
    assert!(deleted.stdout.is_empty(), "{deleted:?}");
    assert!(
        std::str::from_utf8(&deleted.stderr)
            .unwrap()
            .contains("no such package 'pkg'"),
        "{deleted:?}"
    );

    write(&build, &build_file("renamed_exec", "item_test"));
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
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

#[test]
fn only_package_loading_query_errors_receive_evaluation_context() {
    let workspace = scratch("query-error-context");
    let output_base = scratch("query-error-context-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(workspace.join("BUILD.bazel"), "filegroup(name = \"one\")\n");
    let output_base_arg = format!("--output_base={}", output_base.display());

    for (expression, exit_code, diagnostic) in [
        ("some(//:one, -1)", 2, "syntax error at '- 1 )'"),
        ("some(set())", 7, "argument set is empty"),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "{expression}: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(diagnostic),
            "one-shot {expression}: {stderr}"
        );
        assert!(
            !stderr.contains("Evaluation of query"),
            "one-shot {expression}: {stderr}"
        );
    }

    for (expression, exit_code, diagnostic) in [
        ("some(//:one, -1)", 2, "syntax error at '- 1 )'"),
        ("some(set())", 7, "argument set is empty"),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "{expression}: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "daemon {expression}: {stderr}");
        assert!(
            !stderr.contains("Evaluation of query"),
            "daemon {expression}: {stderr}"
        );
    }
}

#[test]
fn output_base_labels_reuses_one_daemon_across_metadata_semantic_transitions() {
    let workspace = scratch("labels-metadata-workspace");
    let output_base = scratch("labels-metadata-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let schema = |default| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"dep\": attr.label(default = \":{default}\"), \"out\": attr.output(mandatory = True)}})\n"
        )
    };
    write(&defs, &schema("one.txt"));
    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", out = \"one.out\")\n",
    );
    let output_base_arg = format!("--output_base={}", output_base.display());
    let query = |expression: &str| {
        slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap()
    };
    let assert_query = |expression: &str, expected: &str| {
        let output = query(expression);
        assert!(output.status.success(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
    };

    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n");
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();
    write(
        &build,
        "# semantic no-op\nconfig_setting( name = \"linux\", values = {\"cpu\": \"k8\"} )\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe( name = \"rule\", out = \"one.out\" )\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n");

    write(&defs, &schema("two.txt"));
    assert_query("labels(dep, //pkg:rule)", "//pkg:two.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = \":three.txt\", out = \"one.out\")\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:three.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"one.out\")\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n//pkg:two.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"two.out\")\n",
    );
    assert_query("labels(out, //pkg:rule)", "//pkg:two.out\n");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}
