use std::fs;

use super::*;

#[path = "../source_staging/test_workspace.rs"]
mod fixture;

const DEFS: &str = r#"
def _forest(ctx):
    seed = ctx.actions.declare_file("seed")
    done = ctx.actions.declare_file("done")
    ctx.actions.write(seed, "seed")
    ctx.actions.run(executable = ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs = [seed, ctx.attr.input[DefaultInfo].files.to_list()[0]], outputs = [done])
    return [DefaultInfo(files = depset([done])), OutputGroupInfo(_validation = ctx.attr.source[DefaultInfo].files)]
forest = rule(implementation = _forest, attrs = {"input": attr.label(allow_files = True), "tool": attr.label(allow_files = True), "source": attr.label(allow_files = True)})
def _source(ctx): return [DefaultInfo(files = ctx.attr.source[DefaultInfo].files)]
source_only = rule(implementation = _source, attrs = {"source": attr.label(allow_files = True)})
def _empty(ctx): return []
empty = rule(implementation = _empty)
"#;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
}
impl Workspace {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp743/fixtures");
        fs::create_dir_all(&parent).unwrap();
        let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
        fixture::write(root.path());
        fs::write(root.path().join("defs.bzl"), DEFS).unwrap();
        fs::write(root.path().join("standalone"), "aaa").unwrap();
        fs::write(root.path().join("unselected"), "ignored").unwrap();
        fs::write(
            root.path().join("BUILD.bazel"),
            r#"
load(":defs.bzl", "forest", "source_only", "empty")
platform(name = "platform")
exports_files(["input", "tool", "standalone", "unselected"])
forest(name = "forest", input = ":input", tool = ":tool", source = ":standalone")
source_only(name = "source_only", source = ":standalone")
alias(name = "source_alias", actual = ":standalone")
empty(name = "empty")
"#,
        )
        .unwrap();
        let runtime =
            WorkspaceRuntime::new(root.path(), crate::runtime::ProcessHostOwner::native()).unwrap();
        Self { root, runtime }
    }

    fn prepare(&self, requested: &[&str]) -> AcceptedCommand<Arc<PreparedActionChainInputs>> {
        self.runtime
            .prepare_requested_action_inputs_with_repository_environment(
                &requested
                    .iter()
                    .map(|target| TargetPattern::parse(target).unwrap())
                    .collect::<Vec<_>>(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!(
                    "file://{}/empty-registry",
                    self.root.path().display()
                )],
                Default::default(),
                Default::default(),
            )
            .unwrap()
    }
}

#[test]
fn requested_staging_certifies_group_only_sources_and_keeps_selection_identity() {
    let workspace = Workspace::new();
    let first = workspace.prepare(&["//:forest"]);
    let prepared = first.terminal_for_test();
    let plan = prepared.plan().unwrap();
    assert!(plan.selected_action().is_none());
    let requested = plan.requested().unwrap();
    assert_eq!(requested.selection().targets()[0].pattern(), "//:forest");
    assert_eq!(requested.artifact_producers(), &[None, Some(1)]);
    assert_eq!(plan.actions().len(), 2);
    let labels = prepared
        .sources()
        .map(|source| source.label().target().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(labels, ["standalone", "input", "tool"]);
    assert!(
        plan.actions()
            .iter()
            .flat_map(|step| step.inputs())
            .all(|input| input.artifact().path() != "standalone")
    );
    let owner = plan.actions()[1].action().context().owner().clone();
    let selected = workspace
        .runtime
        .prepare_action_chain_inputs_with_repository_environment(
            &[TargetPattern::parse("//:forest").unwrap()],
            owner,
            1,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            Default::default(),
        )
        .unwrap();
    let selected = selected.terminal_for_test();
    assert!(selected.plan().unwrap().requested().is_none());
    assert_eq!(
        selected
            .plan()
            .unwrap()
            .selected_action()
            .unwrap()
            .outputs()[0]
            .path(),
        "done"
    );
    assert_eq!(
        selected
            .sources()
            .map(|source| source.label().target().as_str())
            .collect::<Vec<_>>(),
        ["input", "tool"]
    );
    let mut digests = Vec::new();
    for bytes in [b"aaa", b"bbb", b"aaa"] {
        fs::write(workspace.root.path().join("standalone"), bytes).unwrap();
        let accepted = workspace.prepare(&["//:forest"]);
        let prepared = accepted.terminal_for_test();
        let source = prepared
            .sources()
            .find(|source| source.label().target().as_str() == "standalone")
            .unwrap();
        digests.push(source.digest().clone());
        assert!(prepared.observations().observations().iter().any(|(demand, result)| {
            demand.path().as_path() == workspace.root.path().join("standalone")
                && matches!(result.as_ref(), PathObservationResult::FileDigest(PathOperationResult::Present(digest)) if digest == &source.digest())
        }));
    }
    assert_ne!(digests[0], digests[1]);
    assert_eq!(digests[0], digests[2]);
}

#[test]
fn requested_staging_retains_source_only_empty_and_no_target_frontiers() {
    let workspace = Workspace::new();
    for requested in [
        vec![],
        vec!["//:empty"],
        vec!["//:source_only"],
        vec!["//:standalone"],
        vec!["//:source_alias"],
        vec!["//:source_only", "//:standalone", "//:empty"],
    ] {
        let accepted = workspace.prepare(&requested);
        let prepared = accepted.terminal_for_test();
        let plan = prepared.plan().unwrap();
        assert!(plan.actions().is_empty());
        assert!(plan.selected_action().is_none());
        let selection = plan.requested().unwrap().selection();
        assert_eq!(
            selection
                .targets()
                .iter()
                .map(|target| target.pattern())
                .collect::<Vec<_>>(),
            requested
        );
        assert!(!prepared.observations().observations().is_empty());
        let has_source = requested.iter().any(|target| *target != "//:empty");
        assert_eq!(prepared.sources().len(), usize::from(has_source));
        assert_eq!(selection.artifacts().len(), usize::from(has_source));
        assert!(
            plan.requested()
                .unwrap()
                .artifact_producers()
                .iter()
                .all(Option::is_none)
        );
    }
}
