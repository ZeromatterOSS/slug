//! Requested forests through native analysis; no execution or source staging.

use std::fs;
use std::path::Path;

use super::*;
use crate::runtime::AcceptedCommand;
use crate::runtime::BuildCommandError;
use crate::runtime::BuildCommandEvaluation;
use crate::runtime::BzlmodCommandPolicyKey;
use crate::runtime::BzlmodEnvironmentPolicyKey;
use crate::runtime::LockfileMode;
use crate::runtime::ProcessHostOwner;
use crate::runtime::TargetPattern;
use crate::runtime::WorkspaceRuntime;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
}

impl Workspace {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp742/fixtures");
        fs::create_dir_all(&parent).unwrap();
        let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
        fixture::write(root.path());
        fs::write(root.path().join("BUILD.bazel"), BUILD).unwrap();
        fs::write(
            root.path().join("defs.bzl"),
            DEFS.replace("GROUP_EXTRA", "extra_a"),
        )
        .unwrap();
        let runtime = WorkspaceRuntime::new(root.path(), ProcessHostOwner::native()).unwrap();
        Self { root, runtime }
    }

    fn build(
        &self,
        targets: &[&str],
    ) -> AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>> {
        self.runtime
            .build_command_with_repository_environment(
                &targets
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
            .unwrap_or_else(|error| panic!("building {targets:?}: {error}"))
    }
}

const DEFS: &str = r#"
def _forest(ctx):
    seed = ctx.actions.declare_file("seed")
    file = ctx.actions.declare_file("file")
    tree = ctx.actions.declare_directory("tree")
    left = ctx.outputs.left
    right = ctx.actions.declare_file("right")
    extra_a = ctx.actions.declare_file("extra_a")
    extra_b = ctx.actions.declare_file("extra_b")
    validation = ctx.actions.declare_file("validation")
    temporary = ctx.actions.declare_file("temporary")
    unused = ctx.actions.declare_file("unused")
    tool = ctx.attr.tool[DefaultInfo].files.to_list()[0]
    source = ctx.attr.source[DefaultInfo].files.to_list()[0]
    ctx.actions.write(seed, "seed")
    ctx.actions.run(executable = tool, inputs = [seed, source], outputs = [file, tree])
    ctx.actions.run(executable = tool, inputs = [file, tree], outputs = [left])
    ctx.actions.run(executable = tool, inputs = [seed, tree], outputs = [right])
    for output in [extra_a, extra_b, validation, temporary, unused]:
        ctx.actions.write(output, "bytes")
    return [
        DefaultInfo(files = depset([left, right]), runfiles = ctx.runfiles(files = [source])),
        OutputGroupInfo(default = depset([right, file, tree, GROUP_EXTRA]), _validation = depset([validation]), temp_files_INTERNAL_ = depset([temporary])),
    ]
forest = rule(implementation = _forest, attrs = {"left": attr.output(mandatory = True), "tool": attr.label(allow_files = True), "source": attr.label(allow_files = True)})
def _empty(ctx): return []
empty = rule(implementation = _empty)
"#;

const BUILD: &str = r#"
load(":defs.bzl", "forest", "empty")
platform(name = "platform")
exports_files(["input", "tool"])
forest(name = "forest", left = "left", source = ":input", tool = ":tool")
alias(name = "forest_alias", actual = ":forest")
alias(name = "source_alias", actual = ":input")
empty(name = "empty")
"#;

fn action_outputs(plan: &RequestedActionPrerequisitePlan<'_>) -> Vec<Vec<String>> {
    plan.actions()
        .iter()
        .map(|step| {
            step.action()
                .outputs()
                .iter()
                .map(|output| output.path().to_owned())
                .collect()
        })
        .collect()
}

fn group_snapshot(
    target: &crate::runtime::RequestedTargetArtifacts<'_>,
) -> Vec<(&'static str, bool, Vec<usize>)> {
    target
        .groups()
        .iter()
        .map(|group| {
            (
                group.name(),
                group.is_important(),
                group.artifact_indices().to_vec(),
            )
        })
        .collect()
}

#[test]
fn native_requested_forest_preserves_membership_and_shares_diamond_and_cooutputs() {
    let workspace = Workspace::new();
    let requested = [
        "//:forest",
        "//:forest_alias",
        "//:left",
        "//:input",
        "//:empty",
        "//:forest",
    ];
    let accepted = workspace.build(&requested);
    let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
    let plan = evaluation.requested_action_prerequisites().unwrap();
    let selection = plan.selection();
    assert_eq!(
        selection
            .targets()
            .iter()
            .map(|target| target.pattern())
            .collect::<Vec<_>>(),
        requested
    );
    assert_eq!(
        selection
            .artifacts()
            .iter()
            .map(|artifact| artifact.path().into_owned())
            .collect::<Vec<_>>(),
        [
            "input",
            "validation",
            "left",
            "right",
            "file",
            "tree",
            "extra_a",
            "temporary"
        ]
    );
    assert_eq!(
        plan.artifact_producers(),
        &[
            None,
            Some(0),
            Some(3),
            Some(4),
            Some(2),
            Some(2),
            Some(5),
            Some(6)
        ]
    );
    assert_eq!(
        action_outputs(&plan),
        [
            vec!["validation"],
            vec!["seed"],
            vec!["file", "tree"],
            vec!["left"],
            vec!["right"],
            vec!["extra_a"],
            vec!["temporary"]
        ]
    );
    assert_eq!(evaluation.declared_action_count(), 9);
    let groups = group_snapshot(&selection.targets()[0]);
    assert_eq!(
        groups,
        vec![
            ("_hidden_top_level_INTERNAL_", false, vec![0]),
            ("_validation", false, vec![1]),
            ("default", true, vec![2, 3, 4, 5, 6]),
            ("temp_files_INTERNAL_", true, vec![7]),
        ]
    );
    for index in [1, 5] {
        assert_eq!(group_snapshot(&selection.targets()[index]), groups);
        assert_eq!(
            selection.targets()[index].actual_target(),
            selection.targets()[0].actual_target()
        );
    }
    assert_eq!(
        selection.targets()[1]
            .requested_target()
            .label()
            .target()
            .as_str(),
        "forest_alias"
    );
    assert_eq!(
        group_snapshot(&selection.targets()[2]),
        vec![("_validation", false, vec![1]), ("default", true, vec![2])]
    );
    assert_eq!(
        group_snapshot(&selection.targets()[3]),
        vec![("default", true, vec![0])]
    );
    assert!(selection.targets()[4].groups().is_empty());

    // Each generated input keeps its exact artifact and its shared action index.
    let edges = plan
        .actions()
        .iter()
        .map(|step| {
            step.inputs()
                .iter()
                .filter_map(|input| {
                    input
                        .producer()
                        .map(|producer| (input.artifact().path().into_owned(), producer))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        edges,
        vec![
            vec![],
            vec![],
            vec![("seed".to_owned(), 1)],
            vec![("file".to_owned(), 2), ("tree".to_owned(), 2)],
            vec![("seed".to_owned(), 1), ("tree".to_owned(), 2)],
            vec![],
            vec![]
        ]
    );
    for (artifact, producer) in selection.artifacts().iter().zip(plan.artifact_producers()) {
        match artifact {
            AnalysisArtifact::Source(_) => assert_eq!(*producer, None),
            AnalysisArtifact::Derived { owner, output } => {
                let action = plan.actions()[producer.unwrap()].action();
                assert_eq!(owner, &action.context().owner().artifact_owner());
                assert!(action.outputs().contains(output));
            }
        }
    }
    assert!(!workspace.root.path().join("left").exists());
}

#[test]
fn native_requested_forest_retains_empty_source_and_source_alias_roots() {
    let workspace = Workspace::new();
    for requested in [
        vec![],
        vec!["//:empty"],
        vec!["//:input"],
        vec!["//:source_alias"],
        vec!["//:empty", "//:input", "//:source_alias", "//:empty"],
    ] {
        let accepted = workspace.build(&requested);
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        let plan = evaluation.requested_action_prerequisites().unwrap();
        assert!(plan.actions().is_empty());
        assert_eq!(evaluation.declared_action_count(), 0);
        assert_eq!(
            plan.selection()
                .targets()
                .iter()
                .map(|target| target.pattern())
                .collect::<Vec<_>>(),
            requested
        );
        let has_source = requested.iter().any(|target| *target != "//:empty");
        if has_source {
            assert_eq!(
                plan.selection().artifacts(),
                &[AnalysisArtifact::Source(
                    CanonicalLabel::parse("@@//:input").unwrap()
                )]
            );
            assert_eq!(plan.artifact_producers(), &[None]);
        } else {
            assert!(plan.selection().artifacts().is_empty());
            assert!(plan.artifact_producers().is_empty());
        }
        for target in plan.selection().targets() {
            if target.pattern() == "//:empty" {
                assert!(target.groups().is_empty());
            } else {
                assert_eq!(group_snapshot(target), vec![("default", true, vec![0])]);
                assert_eq!(
                    target.actual_target(),
                    &slug_analysis_v2::ConfiguredNodeKey::null(
                        CanonicalLabel::parse("@@//:input").unwrap()
                    )
                );
            }
        }
    }

    let mut contents = Vec::new();
    for bytes in [Some(b"aaa"), Some(b"bbb"), Some(b"aaa"), None, Some(b"aaa")] {
        if let Some(bytes) = bytes {
            fs::write(workspace.root.path().join("input"), bytes).unwrap();
        } else {
            fs::remove_file(workspace.root.path().join("input")).unwrap();
        }
        eprintln!("standalone source mutation phase: {bytes:?}");
        let accepted = workspace.build(&["//:input"]);
        let terminal = accepted.terminal_for_test();
        let Some(bytes) = bytes else {
            let error = terminal.as_ref().as_ref().unwrap_err();
            assert!(error.to_string().contains("Missing"), "{error}");
            continue;
        };
        let evaluation = terminal.as_ref().as_ref().unwrap();
        let plan = evaluation.requested_action_prerequisites().unwrap();
        assert!(plan.actions().is_empty());
        assert_eq!(evaluation.declared_action_count(), 0);
        assert_eq!(plan.artifact_producers(), &[None]);
        assert_eq!(
            plan.selection().artifacts(),
            &[AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:input").unwrap()
            )]
        );
        assert_eq!(plan.selection().targets()[0].pattern(), "//:input");
        let source_key =
            slug_analysis_v2::ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:input").unwrap());
        assert_eq!(
            plan.selection().targets()[0].requested_target(),
            &source_key
        );
        assert_eq!(plan.selection().targets()[0].actual_target(), &source_key);
        let observed = evaluation
            .source_observations_for_test()
            .unwrap()
            .observations()
            .iter()
            .find_map(|(demand, result)| {
                if demand.path().as_path() != workspace.root.path().join("input") {
                    return None;
                }
                match result.as_ref() {
                    slug_workspace_v2::PathObservationResult::FileBytes(
                        slug_workspace_v2::PathOperationResult::Present(bytes),
                    ) => Some(bytes.clone()),
                    _ => None,
                }
            })
            .expect("standalone source certificate retains observed bytes");
        assert_eq!(observed.as_ref(), bytes);
        contents.push(observed);
    }
    assert_ne!(contents[0], contents[1]);
    assert_eq!(contents[0], contents[2]);
    assert_eq!(
        contents[0], contents[3],
        "source recreation restores observed content"
    );
}

#[test]
fn native_requested_forest_group_mutation_restores_selected_actions_on_one_runtime() {
    let workspace = Workspace::new();
    let mut snapshots = Vec::new();
    let mut closures = Vec::new();
    for selected in ["extra_a", "extra_b", "extra_a"] {
        fs::write(
            workspace.root.path().join("defs.bzl"),
            DEFS.replace("GROUP_EXTRA", selected),
        )
        .unwrap();
        let accepted = workspace.build(&["//:forest_alias"]);
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        let plan = evaluation.requested_action_prerequisites().unwrap();
        let outputs = action_outputs(&plan);
        assert_eq!(outputs.len(), 7);
        assert_eq!(outputs[5], [selected]);
        assert!(!outputs.iter().flatten().any(|path| path == "unused"));
        closures.push(
            evaluation
                .analyses()
                .flat_map(|node| node.actions().iter().cloned())
                .collect::<Vec<_>>(),
        );
        snapshots.push((
            plan.selection().artifacts().to_vec(),
            plan.artifact_producers().to_vec(),
            group_snapshot(&plan.selection().targets()[0]),
            plan.actions()
                .iter()
                .map(|step| {
                    (
                        step.action().clone(),
                        step.inputs()
                            .iter()
                            .map(|input| (input.artifact().clone(), input.producer()))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }
    assert_ne!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], snapshots[2]);
    assert_eq!(closures[0], closures[1], "only group membership changed");
    assert_eq!(closures[0], closures[2]);
}
