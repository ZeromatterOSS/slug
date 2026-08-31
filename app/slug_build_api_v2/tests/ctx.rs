/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AttributeMap;
use slug_build_api_v2::AttributeValue;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RuleContext;
use slug_build_api_v2::RunfilesConflictPolicy;
use slug_build_api_v2::RunfilesSymlink;
use slug_identity_v2::CanonicalLabel;

fn label(value: &str) -> CanonicalLabel {
    CanonicalLabel::parse(value).unwrap()
}

#[test]
fn rule_context_exposes_prepared_attrs_files_executable_and_toolchains() {
    let mut attrs = AttributeMap::new();
    attrs.insert("name", AttributeValue::String("demo".to_owned()));
    attrs.insert("dep", AttributeValue::Label(label("@@//dep:lib")));

    let output = CtxActions::new().declare_file("pkg/demo.out").unwrap();
    let ctx = RuleContext::builder(label("@@//pkg:demo"))
        .attrs(attrs)
        .file("src", "pkg/main.in")
        .files("srcs", vec!["pkg/a.in".to_owned(), "pkg/b.in".to_owned()])
        .executable("tool", "tools/tool.sh")
        .output("out", output)
        .fragment("cpp", "enabled")
        .toolchain("//toolchains:demo_type", "//toolchains:linux_toolchain")
        .exec_group("compile", "//platforms:linux_exec")
        .var("COMPILATION_MODE", "fastbuild")
        .location("//pkg:main.in", "pkg/main.in")
        .build();

    assert_eq!(ctx.label().to_string(), "@@//pkg:demo");
    assert_eq!(ctx.attr().get_string("name"), Some("demo"));
    assert_eq!(
        ctx.attr().get_label("dep").unwrap().to_string(),
        "@@//dep:lib"
    );
    assert_eq!(ctx.file("src"), Some("pkg/main.in"));
    assert_eq!(
        ctx.files("srcs").unwrap(),
        &["pkg/a.in".to_owned(), "pkg/b.in".to_owned()]
    );
    assert_eq!(ctx.executable("tool"), Some("tools/tool.sh"));
    assert_eq!(ctx.output("out").unwrap().path(), "pkg/demo.out");
    assert_eq!(ctx.fragment("cpp"), Some("enabled"));
    assert_eq!(
        ctx.toolchain("//toolchains:demo_type"),
        Some("//toolchains:linux_toolchain")
    );
    assert_eq!(ctx.exec_group("compile"), Some("//platforms:linux_exec"));
    assert_eq!(ctx.var("COMPILATION_MODE"), Some("fastbuild"));
}

#[test]
fn expand_location_and_resolve_command_use_prepared_location_map() {
    let ctx = RuleContext::builder(label("@@//pkg:demo"))
        .location("//pkg:data", "pkg/data.txt")
        .build();

    assert_eq!(
        ctx.expand_location("cat $(location //pkg:data)").unwrap(),
        "cat pkg/data.txt"
    );
    let resolved = ctx
        .resolve_command(
            "cat $(location //pkg:data)",
            vec!["pkg/data.txt".to_owned()],
        )
        .unwrap();
    assert_eq!(resolved.command(), "cat pkg/data.txt");
    assert_eq!(resolved.inputs(), &["pkg/data.txt".to_owned()]);
    assert!(
        ctx.expand_location("cat $(location //pkg:missing)")
            .is_err()
    );
}

#[test]
fn retained_runfiles_keeps_typed_files_and_symlinks() {
    let app = AnalysisArtifact::Source(label("@@//pkg:app"));
    let data = AnalysisArtifact::Source(label("@@//pkg:data.txt"));
    let runfiles = RetainedRunfiles::from_parts(
        vec![app, data.clone()],
        Vec::new(),
        vec![RunfilesSymlink::new("repo/data.txt", data)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        RunfilesConflictPolicy::Error,
    )
    .unwrap();

    assert_eq!(runfiles.files.to_list().len(), 2);
    assert_eq!(runfiles.symlinks.to_list()[0].path, "repo/data.txt");
    assert!(runfiles.empty_filenames.is_empty());
}
