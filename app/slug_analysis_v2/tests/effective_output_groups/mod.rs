//! Effective rule providers follow the pinned configured-target completion boundary.

use slug_build_api_v2::AnalysisArtifact;

use super::*;

const HIDDEN: &str = "_hidden_top_level_INTERNAL_";

struct Workspace(PathBuf);
impl Workspace {
    fn new(definitions: &str, build: &str) -> Self {
        let path = scratch();
        fs::write(path.join("MODULE.bazel"), "module(name = 'root')\n").unwrap();
        fs::write(path.join("defs.bzl"), definitions).unwrap();
        fs::write(path.join("BUILD.bazel"), build).unwrap();
        Self(path)
    }

    async fn analyze(&self, dice: &Arc<Dice>, name: &str) -> ConfiguredNodeResult {
        let key = ConfiguredTargetKey::new(
            CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
            typed_action_test_configuration(),
        );
        analyze_request(dice, &self.0, &key, None, false)
            .await
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn artifacts(result: &ConfiguredNodeResult, name: &str) -> Vec<AnalysisArtifact> {
    result
        .providers()
        .output_group_info()
        .unwrap()
        .groups()
        .get(name)
        .unwrap()
        .to_list()
        .into_iter()
        .map(|value| match value.kind() {
            AnalysisValueKind::Artifact(artifact) => artifact.clone(),
            _ => panic!("effective output group contains a non-artifact"),
        })
        .collect()
}

fn paths(result: &ConfiguredNodeResult, name: &str) -> Vec<String> {
    artifacts(result, name)
        .into_iter()
        .map(|artifact| artifact.path().into_owned())
        .collect()
}

#[tokio::test]
async fn nonbinary_hidden_group_unions_default_runfiles_targets_and_retains_empty_presence() {
    let workspace = Workspace::new(
        r#"
def _file(ctx, name):
    out = ctx.actions.declare_file(name)
    ctx.actions.write(out, name)
    return out

def _nonbinary(ctx):
    default = _file(ctx, "default")
    direct = _file(ctx, "direct")
    symlink = _file(ctx, "symlink-target")
    root = _file(ctx, "root-target")
    data = _file(ctx, "data-only")
    explicit = _file(ctx, "explicit")
    return [
        DefaultInfo(files = depset([default]), default_runfiles = ctx.runfiles(files = [direct], symlinks = {"logical": symlink}, root_symlinks = {"root-logical": root}), data_runfiles = ctx.runfiles(files = [data])),
        OutputGroupInfo(_hidden_top_level_INTERNAL_ = depset([explicit])),
    ]

def _empty(ctx):
    return []

nonbinary = rule(implementation = _nonbinary)
empty = rule(implementation = _empty)
"#,
        "load(':defs.bzl', 'nonbinary', 'empty')\nnonbinary(name = 'nonbinary')\nempty(name = 'empty')\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = workspace.analyze(&dice, "nonbinary").await;
    let actual = paths(&result, HIDDEN).into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        ["direct", "explicit", "root-target", "symlink-target"]
            .map(str::to_owned)
            .into()
    );
    assert_eq!(
        default_file_paths(result.providers().default_info().unwrap()),
        ["default"]
    );
    for artifact in artifacts(&result, HIDDEN) {
        let AnalysisArtifact::Derived { owner, .. } = artifact else {
            panic!("lost generated owner")
        };
        assert_eq!(
            owner.label(),
            &CanonicalLabel::parse("@@//:nonbinary").unwrap()
        );
    }
    let empty = workspace.analyze(&dice, "empty").await;
    assert!(paths(&empty, HIDDEN).is_empty());
    assert!(
        !empty
            .providers()
            .output_group_info()
            .unwrap()
            .groups()
            .contains_key("_validation")
    );
}

#[tokio::test]
async fn binary_hidden_group_contains_runfiles_tree_and_explicit_additions_only() {
    let workspace = Workspace::new(
        r#"
def _binary(ctx):
    executable = ctx.actions.declare_file("binary.bin")
    data = ctx.actions.declare_file("binary.data")
    explicit = ctx.actions.declare_file("binary.extra")
    for output in [executable, data, explicit]:
        ctx.actions.write(output, "content")
    return [DefaultInfo(executable = executable, runfiles = ctx.runfiles(files = [data])), OutputGroupInfo(_hidden_top_level_INTERNAL_ = [explicit])]

binary = rule(implementation = _binary, executable = True)
"#,
        "load(':defs.bzl', 'binary')\nbinary(name = 'binary')\n",
    );
    let result = workspace
        .analyze(&Dice::builder().build(DetectCycles::Enabled), "binary")
        .await;
    let default = result.providers().default_info().unwrap();
    let support = default.files_to_run.support.as_ref().unwrap();
    let hidden = artifacts(&result, HIDDEN);
    assert_eq!(hidden.len(), 2);
    assert!(hidden.contains(&support.tree));
    assert!(hidden.iter().any(|artifact| matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.path() == "binary.extra" && output.kind() == ActionOutputKind::File)));
    assert!(
        matches!(&support.tree, AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::RunfilesTree)
    );
    assert_eq!(default_file_paths(default), ["binary.bin"]);
    assert!(
        !paths(&result, HIDDEN)
            .iter()
            .any(|path| matches!(path.as_str(), "binary.bin" | "binary.data"))
    );
}

const VALIDATION_LEAF: &str = r#"
def _leaf(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".VARIANT")
    ctx.actions.write(out, "validation")
    return [DefaultInfo(files = depset([out])), OutputGroupInfo(_validation = [out])]

leaf = rule(implementation = _leaf)
"#;

#[tokio::test]
async fn target_late_bound_validation_propagates_while_literal_implicit_does_not() {
    let workspace = Workspace::new(
        r#"
def _root(ctx):
    if ctx.attr._late.label.name != "late" or ctx.attr._implicit.label.name != "implicit":
        fail("late-bound and literal dependencies must both be configured")
    return []

root = rule(implementation = _root, fragments = ["cpp"], attrs = {
    "_late": attr.label(default = configuration_field(fragment = "cpp", name = "fdo_profile")),
    "_implicit": attr.label(default = ":implicit"),
})
"#,
        "load(':leaf.bzl', 'leaf')\nload(':defs.bzl', 'root')\nleaf(name = 'late')\nleaf(name = 'implicit')\nroot(name = 'root')\n",
    );
    fs::write(
        workspace.0.join("leaf.bzl"),
        VALIDATION_LEAF.replace("VARIANT", "validation"),
    )
    .unwrap();
    let base = typed_action_test_configuration()
        .slug_configuration()
        .unwrap()
        .clone();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::FdoProfile,
        Some("//:late"),
        false,
    )]
    .into();
    let configuration = ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    );
    let key = ConfiguredTargetKey::new(CanonicalLabel::parse("@@//:root").unwrap(), configuration);
    let result = analyze_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace.0,
        &key,
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(paths(&result, "_validation"), ["late.validation"]);
    let group = artifacts(&result, "_validation");
    let AnalysisArtifact::Derived { owner, .. } = &group[0] else {
        panic!("late-bound validation lost its owner")
    };
    assert_eq!(owner.label(), &CanonicalLabel::parse("@@//:late").unwrap());
}

#[tokio::test]
async fn validation_groups_follow_eligible_prepared_attributes_and_dependency_a_b_a() {
    let workspace = Workspace::new(
        r#"
def _middle(ctx):
    return []

def _root(ctx):
    if ctx.attr.filtered != None:
        fail("filtered dependency unexpectedly survived preparation")
    own = ctx.actions.declare_file("own")
    ctx.actions.write(own, "own validation")
    return [OutputGroupInfo(_validation = [own])]

middle = rule(implementation = _middle, attrs = {"deps": attr.label_list()})
root = rule(implementation = _root, attrs = {
    "normal": attr.label(),
    "skipped": attr.label(skip_validations = True),
    "_implicit": attr.label(default = ":implicit"),
    "tool": attr.label(flags = ["IS_TOOL_DEPENDENCY"]),
    "exec_dep": attr.label(cfg = "exec"),
    "filtered": attr.label(allow_rules = ["different_rule"], flags = ["SILENT_RULECLASS_FILTER"]),
})
"#,
        r#"
load(":leaf.bzl", "leaf")
load(":defs.bzl", "middle", "root")
leaf(name = "left")
leaf(name = "right")
leaf(name = "skipped")
leaf(name = "implicit")
leaf(name = "tool")
leaf(name = "exec_dep")
leaf(name = "filtered")
middle(name = "middle", deps = [":left", ":right"])
root(name = "root", normal = ":middle", skipped = ":skipped", tool = ":tool", exec_dep = ":exec_dep", filtered = ":filtered")
"#,
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut results = Vec::new();
    for variant in ["a", "b", "a"] {
        fs::write(
            workspace.0.join("leaf.bzl"),
            VALIDATION_LEAF.replace("VARIANT", variant),
        )
        .unwrap();
        let result = workspace.analyze(&dice, "root").await;
        assert_eq!(
            paths(&result, "_validation"),
            [
                "own".to_owned(),
                format!("left.{variant}"),
                format!("right.{variant}")
            ]
        );
        let artifacts = artifacts(&result, "_validation");
        for (artifact, label) in artifacts.iter().zip(["root", "left", "right"]) {
            let AnalysisArtifact::Derived { owner, .. } = artifact else {
                panic!("validation lost artifact ownership")
            };
            assert_eq!(
                owner.label(),
                &CanonicalLabel::parse(&format!("@@//:{label}")).unwrap()
            );
        }
        assert!(
            result
                .providers()
                .default_info()
                .unwrap()
                .file_artifacts()
                .is_empty()
        );
        results.push(result);
    }
    assert_ne!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}
