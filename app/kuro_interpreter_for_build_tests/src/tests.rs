/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use dice::DetectCycles;
use dice::Dice;
use dice::DiceTransaction;
use dice::UserComputationData;
use dupe::Dupe;
use indoc::indoc;
use kuro_common::dice::cells::SetCellResolver;
use kuro_common::dice::data::testing::SetTestingIoProvider;
use kuro_common::file_ops::io::initialize_read_dir_cache;
use kuro_common::legacy_configs::cells::ExternalBuckconfigData;
use kuro_common::legacy_configs::dice::SetLegacyConfigs;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::CellResolver;
use kuro_core::cells::cell_root_path::CellRootPathBuf;
use kuro_core::cells::name::CellName;
use kuro_core::fs::project::ProjectRootTemp;
use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
use kuro_core::package::PackageLabel;
use kuro_core::target::label::interner::ConcurrentTargetLabelInterner;
use kuro_events::dispatch::EventDispatcher;
use kuro_interpreter::dice::starlark_debug::SetStarlarkDebugger;
use kuro_interpreter::dice::starlark_types::SetStarlarkTypes;
use kuro_interpreter::extra::InterpreterHostArchitecture;
use kuro_interpreter::extra::InterpreterHostPlatform;
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::starlark_profiler::config::SetStarlarkProfilerInstrumentation;
use kuro_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use kuro_interpreter_for_build::interpreter::configuror::BuildInterpreterConfiguror;
use kuro_interpreter_for_build::interpreter::context::SetInterpreterContext;
use kuro_node::nodes::frontend::TargetGraphCalculation;

pub(crate) async fn calculation(fs: &ProjectRootTemp) -> DiceTransaction {
    let mut dice = Dice::builder();
    dice.set(EventDispatcher::null());
    dice.set_testing_io_provider(fs);
    let dice = dice.build(DetectCycles::Enabled);

    let mut per_transaction_data = UserComputationData::new();
    initialize_read_dir_cache(&mut per_transaction_data);
    per_transaction_data.data.set(EventDispatcher::null());
    per_transaction_data.set_starlark_debugger_handle(None);
    let mut ctx = dice.updater_with_data(per_transaction_data);

    let resolver = CellResolver::testing_with_name_and_path(
        CellName::testing_new("root"),
        CellRootPathBuf::new(ProjectRelativePathBuf::unchecked_new("".to_owned())),
    );

    ctx.set_cell_resolver(resolver.dupe()).unwrap();
    ctx.set_is_bzlmod(false).unwrap();
    ctx.set_interpreter_context(
        BuildInterpreterConfiguror::new(
            InterpreterHostPlatform::Linux,
            InterpreterHostArchitecture::X86_64,
            None,
            false,
            false,
            None,
            Arc::new(ConcurrentTargetLabelInterner::default()),
        )
        .unwrap(),
    )
    .unwrap();
    ctx.set_legacy_config_external_data(ExternalBuckconfigData::testing_default())
        .unwrap();
    ctx.set_starlark_profiler_configuration(StarlarkProfilerConfiguration::default())
        .unwrap();
    ctx.set_starlark_types(false, false).unwrap();
    ctx.commit().await
}

#[tokio::test]
async fn test_eval_import() {
    let fs = ProjectRootTemp::new().unwrap();
    fs.write_file(
        "pkg/two.bzl",
        indoc!(
            r#"
        message = "hello world!"
        "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let env = ctx
        .get_loaded_module_from_import_path(&ImportPath::testing_new("root//pkg:two.bzl"))
        .await
        .unwrap();
    assert_eq!(
        "hello world!",
        env.env().get("message").unwrap().unpack_str().unwrap()
    );
}

// TODO: this test require imports extractions
#[tokio::test]
async fn test_eval_import_with_load() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "imports/one.bzl",
        indoc!(
            r#"
                def concat(*args):
                    s = ""
                    for a in args:
                        s += a
                    return s
            "#
        ),
    );
    fs.write_file(
        "pkg/two.bzl",
        indoc!(
            r#"
                load("//imports:one.bzl", "concat")
                message = concat("hello", " ", "world!")
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;
    let env = ctx
        .get_loaded_module_from_import_path(&ImportPath::testing_new("root//pkg:two.bzl"))
        .await
        .unwrap();
    assert_eq!(
        "hello world!",
        env.env().get("message").unwrap().unpack_str().unwrap()
    );
}

// TODO: this test require imports extractions
#[tokio::test]
async fn test_eval_build_file() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                export_file = rule(
                    impl = _impl,
                    attrs = {
                        "src": attrs.string(),
                    },
                )

                java_library = rule(
                    impl = _impl,
                    attrs = {
                        "srcs": attrs.list(attrs.string()),
                    },
                )
        "#
        ),
    );

    fs.write_file(
        "imports/one.bzl",
        indoc!(
            r#"
                load("//rules.bzl", "export_file")

                def some_macro(name, **kwargs):
                    export_file(
                        name=name+"-exported",
                        **kwargs
                    )
            "#
        ),
    );
    fs.write_file("pkg/file1.java", "");
    fs.write_file("pkg/file2.java", "");
    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//imports:one.bzl", "some_macro")
                load("//rules.bzl", "java_library")

                some_macro(
                    name = "invoke_some",
                    src = "some.file",
                )
                java_library(
                    name = "java",
                    srcs = glob(["*.java"]),
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    assert_eq!(package, eval_result.package());
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["invoke_some-exported", "java"], target_names);
}

/// Test Bazel-style rule definition with `implementation` parameter
#[tokio::test]
async fn test_bazel_style_rule_with_implementation() {
    let fs = ProjectRootTemp::new().unwrap();

    // Bazel uses `implementation` parameter, not `impl`
    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _my_impl(ctx):
                    return DefaultInfo()

                # Bazel-style: uses "implementation" instead of "impl"
                my_rule = rule(
                    implementation = _my_impl,
                    attrs = {
                        "srcs": attrs.list(attrs.string()),
                    },
                )
            "#
        ),
    );

    fs.write_file("pkg/file1.txt", "");
    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "my_rule")

                my_rule(
                    name = "test_target",
                    srcs = ["file1.txt"],
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["test_target"], target_names);
}

/// Test Bazel-style attr.* module is usable in rules
#[tokio::test]
async fn test_bazel_attr_module_in_rules() {
    let fs = ProjectRootTemp::new().unwrap();

    // Test using Bazel's attr.* module
    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _my_impl(ctx):
                    return DefaultInfo()

                # Bazel-style: uses attr.* instead of attrs.*
                bazel_style_rule = rule(
                    implementation = _my_impl,
                    attrs = {
                        "deps": attr.label_list(default = []),  # Provide default
                        "data": attr.string_list(default = []),
                        "enabled": attr.bool(default = True),
                        "count": attr.int(default = 1),
                    },
                )
            "#
        ),
    );

    fs.write_file("pkg/file1.txt", "");
    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "bazel_style_rule")

                bazel_style_rule(
                    name = "bazel_target",
                    data = ["a", "b"],
                    enabled = False,
                    count = 42,
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["bazel_target"], target_names);
}

/// Test mixed Bazel and Kuro style in same rule
#[tokio::test]
async fn test_mixed_bazel_kuro_style() {
    let fs = ProjectRootTemp::new().unwrap();

    // Mix Bazel's attr.* with Kuro's attrs.* in same rule definition
    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                # Mix attr.* (Bazel) and attrs.* (Kuro) in same file
                mixed_rule = rule(
                    implementation = _impl,  # Bazel-style parameter
                    attrs = {
                        "bazel_deps": attr.label_list(default = []),      # Bazel-style
                        "kuro_deps": attrs.list(attrs.dep(), default = []), # Kuro-style
                        "bazel_str": attr.string(default = ""),           # Bazel-style
                        "kuro_str": attrs.string(default = ""),           # Kuro-style
                    },
                )
            "#
        ),
    );

    fs.write_file("pkg/file1.txt", "");
    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "mixed_rule")

                mixed_rule(
                    name = "mixed_target",
                    bazel_str = "hello",
                    kuro_str = "world",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["mixed_target"], target_names);
}

/// Test that code without type annotations works (types are optional)
#[tokio::test]
async fn test_code_without_type_annotations() {
    let fs = ProjectRootTemp::new().unwrap();

    // Standard code without any type annotations
    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def helper_func(x, y):
                    return x + y

                def _impl(ctx):
                    return DefaultInfo()

                simple_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "value": attrs.string(default = ""),
                    },
                )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "simple_rule")

                simple_rule(
                    name = "no_types",
                    value = "test",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["no_types"], target_names);
}

/// Test that type annotations are parsed in .bzl files
/// Note: Type annotations are supported in starlark-rust, which is ahead of
/// standard Bazel (Bazel 9.0 has experimental type support, 10.0 will have full support)
#[tokio::test]
async fn test_type_annotations_parse() {
    let fs = ProjectRootTemp::new().unwrap();

    // Code with type annotations (starlark-rust extension)
    fs.write_file(
        "typed_rules.bzl",
        indoc!(
            r#"
                # Type-annotated function (starlark-rust extension)
                def typed_helper(x: str, y: str) -> str:
                    return x + y

                def _impl(ctx):
                    return DefaultInfo()

                typed_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "value": attrs.string(default = ""),
                    },
                )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//typed_rules.bzl", "typed_rule")

                typed_rule(
                    name = "with_types",
                    value = "test",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["with_types"], target_names);
}
