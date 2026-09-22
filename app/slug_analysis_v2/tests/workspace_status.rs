//! Workspace-global descriptors, never synthesized status content.
use std::path::Path;

use slug_build_api_v2::AnalysisArtifact;

use super::*;

const DEFS: &str = r#"
def _files(ctx):
    files = [ctx.info_file, ctx.version_file]
    for file, name in zip(files, ["stable-status.txt", "volatile-status.txt"]):
        if (file.owner != None or file.path != "bazel-out/" + name or
            file.short_path != name or file.basename != name or
            file.dirname != "bazel-out" or file.extension != "txt" or
            file.root.path != "bazel-out" or file.is_source or
            file.is_directory or file.is_symlink):
            fail("incorrect workspace status File projection")
    if files[0].root != files[1].root or files[0] == files[1]:
        fail("status root/role identity lost")
    lookup = {files[0]: "stable", files[1]: "volatile"}
    lookup[ctx.info_file] = "same"
    if len(lookup) != 2 or lookup[files[0]] != "same":
        fail("status File equality/hash lost")
    if depset(files + files).to_list() != files:
        fail("status depset identity lost")
    return files

def _leaf(ctx):
    return [DefaultInfo(files = depset(_files(ctx)))]
leaf = rule(implementation = _leaf)

def _forward(ctx):
    expected = _files(ctx)
    for dep in [ctx.attr.target, ctx.attr.exec_dep, ctx.attr.external]:
        files = dep[DefaultInfo].files.to_list()
        if files != expected or files[0].root != expected[0].root:
            fail("workspace status varies by target, execution config or repository")
    return [DefaultInfo(files = ctx.attr.target[DefaultInfo].files)]
forward = rule(implementation = _forward, attrs = {
    "target": attr.label(), "exec_dep": attr.label(cfg = "exec"),
    "external": attr.label(),
})

def _invalid(ctx):
    ctx.actions.write(ctx.info_file, "not a declared output")
    return []
invalid = rule(implementation = _invalid)
"#;

fn workspace() -> PathBuf {
    let root = scratch();
    fs::create_dir(root.join("dep")).unwrap();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='dep',version='1.0.0')\nlocal_path_override(module_name='dep',path='dep')\n").unwrap();
    fs::write(
        root.join("dep/MODULE.bazel"),
        "module(name='dep',version='1.0.0')\n",
    )
    .unwrap();
    fs::write(root.join("dep/REPO.bazel"), "").unwrap();
    fs::write(root.join("dep/.bazelignore"), "").unwrap();
    for directory in [&root, &root.join("dep")] {
        fs::write(directory.join("defs.bzl"), DEFS).unwrap();
    }
    fs::write(
        root.join("dep/BUILD.bazel"),
        "load(':defs.bzl','leaf')\nleaf(name='leaf',visibility=['//visibility:public'])\n",
    )
    .unwrap();
    fs::write(root.join("BUILD.bazel"), "load(':defs.bzl','leaf','forward','invalid')\nleaf(name='leaf')\nforward(name='forward',target=':leaf',exec_dep=':leaf',external='@@dep+//:leaf')\ninvalid(name='invalid')\n").unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_inputs(
        dice,
        root,
        target,
        configuration,
        Arc::new(RootActivationTracker::default()),
        root_epoch(root),
        &[("dep+", "dep")],
    )
    .await
}

fn files(result: &ConfiguredNodeResult) -> Vec<AnalysisArtifact> {
    assert!(
        result.actions().is_empty(),
        "descriptors must not register per-rule actions"
    );
    result.providers().default_info().unwrap().file_artifacts()
}

#[tokio::test]
async fn status_files_share_across_repositories_configurations_and_forwarding() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = typed_action_test_configuration();
    let first = request(&dice, &root, "@@//:forward", a.clone())
        .await
        .unwrap();
    let expected = files(&first);
    assert_eq!(expected.len(), 2);
    assert!(
        expected
            .iter()
            .all(|file| matches!(file, AnalysisArtifact::WorkspaceStatus(_)))
    );
    let b =
        super::cc_configure_features::config(&[(NativeCommandOption::Copt, "-DSTATUS_CONFIG_B")]);
    let changed = request(&dice, &root, "@@//:forward", b).await.unwrap();
    assert_ne!(
        first.configured_target_key(),
        changed.configured_target_key()
    );
    assert_eq!(expected, files(&changed));
    let restored = request(&dice, &root, "@@//:forward", a.clone())
        .await
        .unwrap();
    assert_eq!(first, restored);
    let external = request(&dice, &root, "@@dep+//:leaf", a.clone())
        .await
        .unwrap();
    assert_eq!(expected, files(&external));
    let exec = ConfigurationKey::from_slug(
        a.slug_configuration()
            .unwrap()
            .to_exec_for_platform(&CanonicalLabel::parse("@@//.slug_test_host:host").unwrap())
            .unwrap(),
    );
    assert_eq!(
        expected,
        files(&request(&dice, &root, "@@//:leaf", exec).await.unwrap())
    );
    assert!(
        !root.join("bazel-out").exists(),
        "descriptor analysis must not produce files"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn status_files_keep_workspace_identity_and_reject_rule_output_authority() {
    let first_root = workspace();
    let second_root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = typed_action_test_configuration();
    let first = files(
        &request(&dice, &first_root, "@@//:leaf", configuration.clone())
            .await
            .unwrap(),
    );
    let second = files(
        &request(&dice, &second_root, "@@//:leaf", configuration.clone())
            .await
            .unwrap(),
    );
    for (left, right) in first.iter().zip(&second) {
        assert_eq!(left.path(), right.path());
        assert_ne!(left, right);
        assert_ne!(left.root(), right.root());
    }
    let error = request(&dice, &first_root, "@@//:invalid", configuration)
        .await
        .unwrap_err();
    assert!(error.contains("output"), "{error}");
    assert!(!first_root.join("bazel-out").exists());
    fs::remove_dir_all(first_root).unwrap();
    fs::remove_dir_all(second_root).unwrap();
}
