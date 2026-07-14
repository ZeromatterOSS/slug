/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::analyze_loaded_rule;
use slug_build_api_v2::ActionKind;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleEvaluator;

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-analysis-rule-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn frozen_loaded_rule_evaluates_into_default_info_and_write_action() {
    let workspace = scratch();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.write(out, \"hello from an action\\n\")\n    return [DefaultInfo(files = depset([out]))]\n\nwrite_file = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"write_file\")\nwrite_file(name = \"write_file\")\n",
    )
    .unwrap();

    let package = BzlModuleEvaluator::new(&workspace)
        .unwrap()
        .evaluate_package(&package)
        .unwrap();
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:write_file").unwrap(),
        ConfigurationKey::target("first-build").unwrap(),
    );
    let result = analyze_loaded_rule(&package, "write_file", key, "pkg").unwrap();

    assert_eq!(result.declared_outputs(), &["pkg/write_file.txt"]);
    assert_eq!(
        result.providers().default_info().unwrap().files.to_list(),
        ["pkg/write_file.txt"]
    );
    assert_eq!(result.actions().len(), 1);
    assert_eq!(
        result.actions()[0].outputs()[0].path(),
        "pkg/write_file.txt"
    );
    assert_eq!(
        result.actions()[0].kind(),
        &ActionKind::Write {
            content: "hello from an action\n".to_owned(),
            is_executable: false,
        }
    );
}
