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

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-build-load-files-provenance/workspace")
}

#[test]
fn graph_output_matches_each_bazel_provenance_oracle_row() {
    let cases = [
        (
            "loadfiles_direct",
            "loadfiles(//a:one)",
            "  \"//shared:one.bzl\\n//shared:two.bzl\"\n",
        ),
        ("buildfiles_root", "buildfiles(//:root)", ""),
        (
            "buildfiles_fallback",
            "buildfiles(//fallback:only)",
            "  \"//shared:BUILD.bazel\\n//shared:one.bzl\\n//shared:two.bzl\"\n",
        ),
        (
            "buildfiles_diamond",
            "buildfiles(//diamond:probe)",
            "  \"//diamond:leaf.bzl\\n//diamond:left.bzl\\n//diamond:right.bzl\"\n",
        ),
        (
            "buildfiles_multi_package",
            "buildfiles(set(//a:one //fallback:only))",
            "  \"//shared:BUILD.bazel\\n//shared:one.bzl\\n//shared:two.bzl\"\n",
        ),
        (
            "deps_buildfiles_direct",
            "deps(buildfiles(//a:one))",
            "  \"//a:BUILD.bazel\\n//shared:BUILD.bazel\\n//shared:one.bzl\\n//shared:two.bzl\"\n",
        ),
        (
            "buildfiles_union_real",
            "buildfiles(//a:one) union //a:one",
            "  \"//a:one\\n//shared:BUILD.bazel\\n//shared:one.bzl\\n//shared:two.bzl\"\n",
        ),
    ];

    for (name, expression, nodes) in cases {
        let output = slug()
            .current_dir(workspace())
            .args([
                "query",
                "--order_output=full",
                "--output=graph",
                "--graph:factored",
                expression,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{name}: {:?}", output.stderr);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("digraph mygraph {{\n  node [shape=box];\n{nodes}}}\n"),
            "{name}"
        );
    }
}

#[test]
fn graph_output_honors_unfactored_mode() {
    let output = slug()
        .current_dir(workspace())
        .args([
            "query",
            "--order_output=full",
            "--output=graph",
            "--graph:factored=false",
            "loadfiles(//a:one)",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "digraph mygraph {\n",
            "  node [shape=box];\n",
            "  \"//shared:two.bzl\"\n",
            "  \"//shared:one.bzl\"\n",
            "}\n",
        )
    );
}
