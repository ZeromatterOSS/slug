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
use slug_reapi_v2::RemoteConfig;

use crate::Daemon;

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
