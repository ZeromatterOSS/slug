use std::fs;
use std::sync::Arc;

use slug_core_v2::runtime::WorkspaceDirectoryObservation;
use slug_core_v2::runtime::WorkspaceFileObservation;
use slug_core_v2::runtime::WorkspaceObservation;
use slug_core_v2::runtime::WorkspaceRuntime;
use slug_core_v2::runtime::evaluate_workspace;
use slug_core_v2::runtime::evaluate_workspace_query;
use slug_core_v2::runtime::evaluate_workspace_query_with_policy;
use slug_core_v2::runtime::evaluate_workspace_targets;
use slug_core_v2::runtime::observe_workspace;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryPolicy;

#[test]
fn query_policy_defaults_and_primary_runtime_path_toggle_without_semantic_state() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"runtime_test\")\n",
    )
    .unwrap();
    fs::create_dir_all(workspace.path().join("pkg")).unwrap();
    fs::write(
        workspace.path().join("pkg/BUILD.bazel"),
        "filegroup(name = \"plain\")\ntest_suite(name = \"suite\", tests = [\":plain\"])\n",
    )
    .unwrap();
    let expression = "tests(//pkg:suite)";

    let default = evaluate_workspace_query(workspace.path(), expression, QueryOrder::Auto).unwrap();
    assert!(default.stdout().is_empty());
    let strict = evaluate_workspace_query_with_policy(
        workspace.path(),
        expression,
        QueryOrder::Auto,
        QueryPolicy {
            strict_test_suite: true,
        },
    )
    .unwrap_err();
    assert!(strict.to_string().contains(
        "The label '//pkg:plain' in the test_suite '//pkg:suite' does not refer to a test or test_suite rule!"
    ));

    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let observe = || observe_workspace(workspace.path()).unwrap();
    let default = runtime
        .query_observations(observe(), expression, QueryOrder::Auto)
        .unwrap();
    assert!(default.stdout().is_empty());
    let strict = runtime
        .query_observations_with_policy(
            observe(),
            expression,
            QueryOrder::Auto,
            QueryPolicy {
                strict_test_suite: true,
            },
        )
        .unwrap_err();
    assert!(
        strict
            .to_string()
            .contains("does not refer to a test or test_suite rule")
    );
    let default_again = runtime
        .query_observations(observe(), expression, QueryOrder::Auto)
        .unwrap();
    assert!(default_again.stdout().is_empty());
}

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

#[test]
fn root_and_package_share_one_committed_revision_across_module_edit_and_delete() {
    let workspace = tempfile::tempdir().unwrap();
    let package = workspace.path().join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"before\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "").unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        "filegroup(name = \"probe\", srcs = [])\n",
    )
    .unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let target = TargetPattern::parse("//pkg:probe").unwrap();
    let observe = || observe_workspace(workspace.path()).unwrap();

    let first = runtime
        .evaluate_observations(observe(), &[target.clone()])
        .unwrap();
    assert_eq!(first.revision, first.workspace.revision);
    assert_eq!(first.revision, first.packages[0].revision);

    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"after\")\n",
    )
    .unwrap();
    let edited = runtime
        .evaluate_observations(observe(), &[target.clone()])
        .unwrap();
    assert_eq!(edited.revision, edited.workspace.revision);
    assert_eq!(edited.revision, edited.packages[0].revision);

    fs::write(workspace.path().join("MODULE.bazel"), "module(name = )\n").unwrap();
    let invalid = runtime.evaluate_observations(observe(), &[target.clone()]);
    assert!(invalid.unwrap_err().to_string().contains("MODULE.bazel"));

    fs::remove_file(workspace.path().join("MODULE.bazel")).unwrap();
    let deleted = runtime.evaluate_observations(observe(), &[target]);
    assert!(deleted.unwrap_err().to_string().contains("MODULE.bazel"));
}

#[test]
fn retained_runtime_uses_root_build_when_build_bazel_is_deleted() {
    let workspace = tempfile::tempdir().unwrap();
    let module = workspace.path().join("MODULE.bazel");
    let primary = workspace.path().join("BUILD.bazel");
    let fallback = workspace.path().join("BUILD");
    fs::write(&module, "module(name = \"root\")\n").unwrap();
    fs::write(&primary, "primary = True\n").unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let observe = || {
        [&module, &primary, &fallback]
            .into_iter()
            .map(WorkspaceFileObservation::read)
            .collect::<Vec<_>>()
    };

    let first = runtime.evaluate(observe(), &[]).unwrap();
    assert!(first.workspace.build.path.ends_with("BUILD.bazel"));

    fs::remove_file(&primary).unwrap();
    fs::write(&fallback, "fallback = True\n").unwrap();
    let second = runtime.evaluate(observe(), &[]).unwrap();
    assert!(second.workspace.build.path.ends_with("/BUILD"));
}

#[test]
fn read_error_observation_is_not_treated_as_file_absence() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"root\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "").unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let error = runtime
        .evaluate(
            [
                WorkspaceFileObservation::read(workspace.path().join("MODULE.bazel")),
                WorkspaceFileObservation {
                    path: workspace.path().join("BUILD.bazel"),
                    value: WorkspaceFileValue::ReadError(Arc::new("permission denied".to_owned())),
                },
            ],
            &[],
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("permission denied"), "{error}");
}

#[test]
fn directory_observer_records_sorted_direct_entries_without_following_symlinks() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    fs::write(root.join("z-file"), "").unwrap();
    fs::create_dir(root.join("a-dir")).unwrap();
    fs::write(root.join("a-dir").join("nested"), "").unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink("a-dir", root.join("m-link")).unwrap();
    #[cfg(unix)]
    let _other = std::os::unix::net::UnixListener::bind(root.join("b-socket")).unwrap();

    let observation = observe_workspace(root).unwrap();
    let root_listing = observation
        .directories
        .iter()
        .find(|directory| directory.path == root)
        .unwrap();
    let WorkspaceDirectoryValue::Present(entries) = &root_listing.value else {
        panic!("root listing was not present: {root_listing:?}");
    };
    let names = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
    assert!(entries.iter().any(|entry| {
        entry.name == "z-file"
            && entry.kind == slug_loading_v2::keys::WorkspaceDirectoryEntryKind::RegularFile
    }));
    assert!(entries.iter().any(|entry| {
        entry.name == "a-dir"
            && entry.kind == slug_loading_v2::keys::WorkspaceDirectoryEntryKind::Directory
    }));
    #[cfg(unix)]
    assert!(entries.iter().any(|entry| {
        entry.name == "m-link"
            && entry.kind == slug_loading_v2::keys::WorkspaceDirectoryEntryKind::Symlink
    }));
    #[cfg(unix)]
    assert!(entries.iter().any(|entry| {
        entry.name == "b-socket"
            && entry.kind == slug_loading_v2::keys::WorkspaceDirectoryEntryKind::Other
    }));
    #[cfg(unix)]
    assert!(
        !observation
            .directories
            .iter()
            .any(|directory| directory.path == root.join("m-link"))
    );
}

#[test]
fn directory_observation_paths_must_be_normalized_and_contained() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"root\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "").unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let error = runtime
        .evaluate_observations(
            WorkspaceObservation {
                files: vec![
                    WorkspaceFileObservation::read(workspace.path().join("MODULE.bazel")),
                    WorkspaceFileObservation::read(workspace.path().join("BUILD.bazel")),
                ],
                directories: vec![WorkspaceDirectoryObservation {
                    path: workspace.path().join("nested/../outside"),
                    value: WorkspaceDirectoryValue::Absent,
                }],
            },
            &[],
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("not normalized"), "{error}");
}

#[cfg(unix)]
#[test]
fn directory_observation_paths_must_not_alias_through_symlinks() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name = \"root\")\n",
    )
    .unwrap();
    fs::write(workspace.path().join("BUILD.bazel"), "").unwrap();
    fs::create_dir(workspace.path().join("actual")).unwrap();
    std::os::unix::fs::symlink("actual", workspace.path().join("alias")).unwrap();
    let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
    let error = runtime
        .evaluate_observations(
            WorkspaceObservation {
                files: vec![
                    WorkspaceFileObservation::read(workspace.path().join("MODULE.bazel")),
                    WorkspaceFileObservation::read(workspace.path().join("BUILD.bazel")),
                ],
                directories: vec![WorkspaceDirectoryObservation {
                    path: workspace.path().join("alias"),
                    value: WorkspaceDirectoryValue::Absent,
                }],
            },
            &[],
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("aliases through"), "{error}");
}
