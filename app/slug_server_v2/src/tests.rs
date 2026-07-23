/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use slug_identity_v2::TargetPattern;
use slug_query_v2::QueryOrder;
use slug_reapi_v2::RemoteConfig;

use crate::BuildRequest;
use crate::Daemon;
use crate::DaemonRequest;
use crate::DaemonResponse;
use crate::QueryRequest;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-server-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn remote_disabled() -> RemoteConfig {
    RemoteConfig {
        executor: None,
        cache: None,
        instance_name: None,
        headers: BTreeMap::new(),
        timeout_seconds: None,
        retry_attempts: None,
        default_exec_properties: BTreeMap::new(),
    }
}

fn target(label: &str) -> TargetPattern {
    TargetPattern::parse(label).unwrap()
}

const DEFS_BZL: &str = "\
load(\":message.bzl\", \"MESSAGE\")
def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + \".txt\")
    ctx.actions.write(out, MESSAGE + \"\\n\")
    return [DefaultInfo(files = depset([out]))]
message_rule = rule(implementation = _impl)
";

const BUILD_BAZEL: &str =
    "load(\":defs.bzl\", \"message_rule\")\nmessage_rule(name = \"message\")\n";

/// The first build populates the digest cache; zero files are invalidated
/// because nothing was previously cached.
#[test]
fn first_build_invalidates_zero_files() {
    let workspace = scratch("first-build");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\", srcs = [])\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let result = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert_eq!(result.invalidated_files, 0);
    assert!(result.stderr.contains("\"invalidated_files\":0"));
}

/// Editing a loaded `.bzl` file between builds causes the daemon to invalidate
/// exactly one path and recompute the dependent package.
#[test]
fn bzl_edit_invalidates_one_file_on_second_build() {
    let workspace = scratch("bzl-edit");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(&package.join("message.bzl"), "MESSAGE = \"one\"\n");
    write(&package.join("defs.bzl"), DEFS_BZL);
    write(&package.join("BUILD.bazel"), BUILD_BAZEL);

    let mut daemon = Daemon::new(&workspace).unwrap();
    // First build: populates cache, 0 invalidated.
    let first = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(first.invalidated_files, 0);

    // Edit message.bzl: "one" -> "two".
    write(&package.join("message.bzl"), "MESSAGE = \"two\"\n");

    // Second build: exactly 1 file invalidated (message.bzl).
    let second = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(second.invalidated_files, 1);
}

/// A third build with no further edits invalidates zero files (the digest
/// cache matches the current state).
#[test]
fn third_build_after_no_edit_invalidates_zero() {
    let workspace = scratch("third-build");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(&package.join("message.bzl"), "MESSAGE = \"one\"\n");
    write(&package.join("defs.bzl"), DEFS_BZL);
    write(&package.join("BUILD.bazel"), BUILD_BAZEL);

    let mut daemon = Daemon::new(&workspace).unwrap();
    daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    write(&package.join("message.bzl"), "MESSAGE = \"two\"\n");
    let second = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(second.invalidated_files, 1);
    let third = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(third.invalidated_files, 0);
}

#[test]
fn missing_loaded_bzl_is_absent_then_create_is_observed_without_a_key_panic() {
    let workspace = scratch("missing-then-create-bzl");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let missing = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert!(
        missing.stderr.contains("workspace file is absent"),
        "{missing:?}"
    );

    write(
        &package.join("defs.bzl"),
        "def declare():\n    native.filegroup(name = \"probe\", srcs = [])\n",
    );
    let created = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert_eq!(created.invalidated_files, 1);
    assert!(
        !created.stderr.contains("build_runtime_error"),
        "{created:?}"
    );
    assert!(created.stderr.contains("dice_starlark_package_loading"));
}

#[test]
fn retained_runtime_switches_from_build_bazel_to_build_fallback() {
    let workspace = scratch("build-fallback-transition");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    let primary = package.join("BUILD.bazel");
    write(&primary, "filegroup(name = \"primary\", srcs = [])\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.build(&[target("//pkg:primary")], &remote_disabled(), &[]);
    assert!(!first.stderr.contains("build_runtime_error"), "{first:?}");

    fs::remove_file(&primary).unwrap();
    write(
        &package.join("BUILD"),
        "filegroup(name = \"fallback\", srcs = [])\n",
    );
    let fallback = daemon.build(&[target("//pkg:fallback")], &remote_disabled(), &[]);
    assert_eq!(fallback.invalidated_files, 2);
    assert!(
        !fallback.stderr.contains("build_runtime_error"),
        "{fallback:?}"
    );
    assert!(fallback.stderr.contains("dice_starlark_package_loading"));
}

#[test]
fn tagged_query_protocol_carries_only_raw_expression_and_order() {
    let request = DaemonRequest::Query(QueryRequest {
        expression: "deps(//pkg:bin)".to_owned(),
        order_output: "full".to_owned(),
    });
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["kind"], "query");
    assert_eq!(json["request"]["expression"], "deps(//pkg:bin)");
    assert_eq!(json["request"]["order_output"], "full");
    assert_eq!(json["request"].as_object().unwrap().len(), 2);
}

#[test]
fn tagged_build_protocol_preserves_existing_fields_and_common_response() {
    let request = DaemonRequest::Build(BuildRequest {
        targets: vec!["//pkg:one".to_owned(), "//pkg:two".to_owned()],
        executor: Some("grpc://executor".to_owned()),
        default_exec_properties: vec![
            ("cpu".to_owned(), "x86_64".to_owned()),
            ("os".to_owned(), "linux".to_owned()),
        ],
    });
    let json = serde_json::to_string(&request).unwrap();
    let round_trip: DaemonRequest = serde_json::from_str(&json).unwrap();
    let DaemonRequest::Build(build) = round_trip else {
        panic!("expected tagged build request");
    };
    assert_eq!(build.targets, ["//pkg:one", "//pkg:two"]);
    assert_eq!(build.executor.as_deref(), Some("grpc://executor"));
    assert_eq!(
        build.default_exec_properties,
        [
            ("cpu".to_owned(), "x86_64".to_owned()),
            ("os".to_owned(), "linux".to_owned())
        ]
    );

    let response = DaemonResponse {
        exit_code: 2,
        stdout: String::new(),
        stderr: "{\"error\":\"analysis_not_implemented\"}".to_owned(),
        invalidated_files: 3,
    };
    let response: DaemonResponse =
        serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();
    assert_eq!(response.exit_code, 2);
    assert!(response.stdout.is_empty());
    assert_eq!(response.stderr, "{\"error\":\"analysis_not_implemented\"}");
    assert_eq!(response.invalidated_files, 3);
}

#[test]
fn retained_daemon_query_observes_build_dependency_edits() {
    let workspace = scratch("query-build-edit");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &package.join("BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"one.txt\"])\n",
    );
    write(&package.join("one.txt"), "one\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "//pkg:bin\n//pkg:one.txt\n");
    assert_eq!(first.invalidated_files, 0);

    write(
        &package.join("BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"two.txt\"])\n",
    );
    write(&package.join("two.txt"), "two\n");
    let second = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(second.exit_code, 0, "{second:?}");
    assert_eq!(second.stdout, "//pkg:bin\n//pkg:two.txt\n");
    assert_eq!(second.invalidated_files, 2);

    let third = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(third.exit_code, 0, "{third:?}");
    assert_eq!(third.invalidated_files, 0);
}

#[test]
fn retained_daemon_some_observes_candidate_create_rename_delete_recreate() {
    let workspace = scratch("some-candidate-transitions");
    let build = workspace.join("cand/BUILD.bazel");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&build, "filegroup(name = \"one\")\n");
    let expression = "some(//cand:all, 10)";
    let mut daemon = Daemon::new(&workspace).unwrap();

    let initial = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//cand:one\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"two\")\n",
    );
    let created = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(created.exit_code, 0, "{created:?}");
    assert_eq!(created.stdout, "//cand:one\n//cand:two\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"middle\")\n",
    );
    let renamed = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(renamed.exit_code, 0, "{renamed:?}");
    assert_eq!(renamed.stdout, "//cand:middle\n//cand:one\n");

    write(&build, "filegroup(name = \"one\")\n");
    let deleted = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(deleted.exit_code, 0, "{deleted:?}");
    assert_eq!(deleted.stdout, "//cand:one\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"zeta\")\n",
    );
    let recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert_eq!(recreated.stdout, "//cand:one\n//cand:zeta\n");
}

#[test]
fn retained_daemon_reverse_query_observes_edge_and_subtree_transitions() {
    let workspace = scratch("reverse-query-transitions");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//leaf:item\"])\n",
    );
    write(
        &workspace.join("leaf/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [])\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.query("rdeps(//app:top, //leaf:item)", QueryOrder::Auto);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "//app:top\n//leaf:item\n");

    write(
        &workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [])\n",
    );
    let lost = daemon.query("rdeps(//app:top, //leaf:item)", QueryOrder::Auto);
    assert_eq!(lost.exit_code, 0, "{lost:?}");
    assert!(lost.stdout.is_empty(), "{lost:?}");

    write(
        &workspace.join("tree/base/BUILD.bazel"),
        "filegroup(name = \"base\", srcs = [])\n",
    );
    let subtree = daemon.query("//tree/...", QueryOrder::Auto);
    assert_eq!(subtree.exit_code, 0, "{subtree:?}");
    assert_eq!(subtree.stdout, "//tree/base:base\n");

    write(
        &workspace.join("tree/dynamic/BUILD.bazel"),
        "filegroup(name = \"dynamic\", srcs = [])\n",
    );
    let created = daemon.query("//tree/...", QueryOrder::Auto);
    assert_eq!(created.exit_code, 0, "{created:?}");
    assert_eq!(created.stdout, "//tree/base:base\n//tree/dynamic:dynamic\n");
}

#[test]
fn retained_daemon_path_query_observes_edge_and_reachable_package_transitions() {
    let workspace = scratch("path-query-transitions");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("dest/BUILD.bazel"),
        "filegroup(name = \"end\", srcs = [])\n",
    );
    let expression = "somepath(//origin:top, //dest:end)";

    let mut daemon = Daemon::new(&workspace).unwrap();
    let initial = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [])\n",
    );
    let lost = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(lost.exit_code, 0, "{lost:?}");
    assert!(lost.stdout.is_empty(), "{lost:?}");

    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    let restored = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(restored.exit_code, 0, "{restored:?}");
    assert_eq!(restored.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("branch/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//branch:item\"])\n",
    );
    let gained = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(gained.exit_code, 0, "{gained:?}");
    assert_eq!(gained.stdout, "//origin:top\n//branch:item\n//dest:end\n");

    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    fs::remove_file(workspace.join("branch/BUILD.bazel")).unwrap();
    let removed = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(removed.exit_code, 0, "{removed:?}");
    assert_eq!(removed.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("branch/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//branch:item\"])\n",
    );
    let recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert_eq!(
        recreated.stdout,
        "//origin:top\n//branch:item\n//dest:end\n"
    );
}
