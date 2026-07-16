use std::fs;

use slug_core_v2::runtime::evaluate_workspace;
use slug_core_v2::runtime::evaluate_workspace_targets;
use slug_identity_v2::TargetPattern;

#[test]
fn root_module_and_build_are_evaluated_through_dice_and_starlark() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"runtime_test\", version = \"0.1.0\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "answer = 40 + 2\n").unwrap();

    let result = evaluate_workspace(workspace.path()).unwrap();

    assert!(result.module.error.is_none(), "{result:?}");
    assert!(result.build.error.is_none(), "{result:?}");
}

#[test]
fn starlark_evaluation_errors_are_reported_from_the_dice_result() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"runtime_test\")\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("BUILD.bazel"),
        "this is not valid Starlark\n",
    )
    .unwrap();

    let error = evaluate_workspace(workspace.path())
        .unwrap_err()
        .to_string();

    assert!(error.contains("BUILD.bazel"), "{error}");
}

#[test]
fn loaded_custom_rule_reaches_analysis_and_declares_an_action() {
    let workspace = tempfile::tempdir().unwrap();
    let package = workspace.path().join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"runtime_test\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.write(out, \"hello\\n\")\n    return [DefaultInfo(files = depset([out]))]\n\nwrite_file = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"write_file\")\nwrite_file(name = \"write_file\")\n",
    )
    .unwrap();

    let result = evaluate_workspace_targets(
        workspace.path(),
        &[TargetPattern::parse("//pkg:write_file").unwrap()],
    )
    .unwrap();
    let analysis = result.packages[0].analysis.as_ref().unwrap();
    assert_eq!(analysis.declared_outputs(), &["pkg/write_file.txt"]);
    assert_eq!(analysis.actions().len(), 1);
}
