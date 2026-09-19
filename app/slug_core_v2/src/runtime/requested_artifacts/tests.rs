//! Ordinary requested output selection through the native command owner.

#[path = "runfiles_layout_tests.rs"]
mod runfiles_layout_tests;

#[path = "runfiles_manifest_tests.rs"]
mod runfiles_manifest_tests;

#[path = "runfiles_generation_tests.rs"]
mod runfiles_generation_tests;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

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
use crate::runtime::dice::BuildCommandRootKey;
use crate::runtime::dice::BuildCommandRootObservationKey;

#[path = "../source_staging/test_workspace.rs"]
mod fixture;

type AcceptedBuild = AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
}
impl Workspace {
    fn new() -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp741/fixtures");
        fs::create_dir_all(&parent).unwrap();
        let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
        fixture::write(root.path());
        fs::write(
            root.path().join("defs.bzl"),
            DEFS.replace("GROUP_VARIANT", "group_a"),
        )
        .unwrap();
        fs::write(root.path().join("BUILD.bazel"), BUILD).unwrap();
        let runtime = WorkspaceRuntime::new(root.path(), ProcessHostOwner::native()).unwrap();
        Self { root, runtime }
    }

    fn build(&self, targets: &[&str]) -> AcceptedBuild {
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
            .unwrap_or_else(|error| panic!("building requested targets {targets:?}: {error}"))
    }
}

const DEFS: &str = r#"
def _bundle(ctx):
    out = ctx.outputs.out
    shared = ctx.actions.declare_file("shared")
    extra = ctx.actions.declare_file("GROUP_VARIANT")
    validation = ctx.actions.declare_file("validation")
    temporary = ctx.actions.declare_file("temporary")
    unused = ctx.actions.declare_file("unused")
    for file in [out, shared, extra, validation, temporary, unused]:
        ctx.actions.write(file, "bytes")
    source = ctx.attr.source[DefaultInfo].files.to_list()[0]
    return [DefaultInfo(files = depset([out, shared]), runfiles = ctx.runfiles(files = [source])), OutputGroupInfo(
        default = depset([shared, extra]), _hidden_top_level_INTERNAL_ = depset([shared]),
        _validation = depset([validation]), temp_files_INTERNAL_ = depset([temporary]), unrelated = depset([unused]))]
bundle = rule(implementation = _bundle, attrs = {"out": attr.output(mandatory = True), "source": attr.label(allow_files = True)})
def _empty(ctx): return []
empty = rule(implementation = _empty)
def _binary(ctx):
    out = ctx.actions.declare_file("binary.bin")
    ctx.actions.write(out, '#!/bin/sh\n', is_executable = True)
    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list()))]
binary = rule(implementation = _binary, executable = True, attrs = {"source": attr.label(allow_files = True)})
"#;

const BUILD: &str = r#"
load(":defs.bzl", "bundle", "empty", "binary")
platform(name = "platform")
exports_files(["input", "tool"])
bundle(name = "bundle", out = "generated.out", source = ":input")
alias(name = "bundle_alias", actual = ":bundle")
alias(name = "source_inner", actual = ":input")
alias(name = "source_alias", actual = ":source_inner")
alias(name = "bad_alias", actual = ":platform")
empty(name = "empty")
binary(name = "binary", source = ":input")
filegroup(name = "native")
"#;

fn evaluation(accepted: &AcceptedBuild) -> &BuildCommandEvaluation {
    accepted.terminal_for_test().as_ref().as_ref().unwrap()
}

fn paths(artifacts: &[AnalysisArtifact]) -> Vec<String> {
    artifacts
        .iter()
        .map(|artifact| artifact.path().into_owned())
        .collect()
}

fn selected_paths(selection: &RequestedArtifacts<'_>, group: &RequestedOutputGroup) -> Vec<String> {
    group
        .artifact_indices()
        .iter()
        .map(|index| selection.artifacts()[*index].path().into_owned())
        .collect()
}

fn source_content(
    workspace: &Workspace,
    value: &BuildCommandEvaluation,
    suffix: &str,
    expected: &[u8],
) -> Arc<PathObservationResult> {
    let certificate = value
        .source_certificate()
        .expect("source root must certify actual bytes");
    let accepted = workspace
        .runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    certificate
        .observations()
        .observations()
        .iter()
        .find_map(|(demand, observation)| {
            if !demand.path().as_path().ends_with(suffix) {
                return None;
            }
            match observation.as_ref() {
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
                    assert_eq!(bytes.as_ref(), expected);
                }
                PathObservationResult::FileDigest(PathOperationResult::Present(digest)) => {
                    assert_eq!(
                        digest.sha256().as_slice(),
                        Sha256::digest(expected).as_slice()
                    );
                    assert_eq!(digest.size_bytes(), expected.len() as u64);
                }
                _ => return None,
            }
            assert!(Arc::ptr_eq(
                observation,
                accepted
                    .path_observations
                    .get(demand)
                    .expect("source content observation belongs to accepted frontier")
            ));
            Some(observation.clone())
        })
        .expect("source certificate contains content for actual source")
}

#[test]
fn native_requested_groups_preserve_root_order_membership_and_global_dedup() {
    let workspace = Workspace::new();
    let requested = [
        "//:bundle",
        "//:bundle_alias",
        "//:input",
        "//:generated.out",
        "//:bundle",
    ];
    let accepted = workspace.build(&requested);
    let value = evaluation(&accepted);
    let selection = value.requested_artifacts().unwrap();
    assert_eq!(
        selection
            .targets()
            .iter()
            .map(|target| target.pattern())
            .collect::<Vec<_>>(),
        requested
    );
    assert_eq!(
        paths(selection.artifacts()),
        [
            "shared",
            "input",
            "validation",
            "generated.out",
            "group_a",
            "temporary"
        ]
    );
    let first = &selection.targets()[0];
    assert_eq!(
        first
            .groups()
            .iter()
            .map(|group| (group.name(), group.is_important()))
            .collect::<Vec<_>>(),
        [
            ("_hidden_top_level_INTERNAL_", false),
            ("_validation", false),
            ("default", true),
            ("temp_files_INTERNAL_", true),
        ]
    );
    assert_eq!(
        selected_paths(&selection, &first.groups()[0]),
        ["shared", "input"]
    );
    assert_eq!(
        selected_paths(&selection, &first.groups()[2]),
        ["generated.out", "shared", "group_a"]
    );
    for index in [1, 4] {
        let repeated = &selection.targets()[index];
        assert_eq!(repeated.actual_target(), first.actual_target());
        for (left, right) in first.groups().iter().zip(repeated.groups()) {
            assert_eq!(left.artifact_indices(), right.artifact_indices());
        }
    }
    assert_eq!(
        selection.targets()[1]
            .requested_target()
            .label()
            .target()
            .as_str(),
        "bundle_alias"
    );
    let source = &selection.targets()[2];
    assert_eq!(
        source.actual_target(),
        &ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:input").unwrap())
    );
    assert_eq!(source.groups().len(), 1);
    assert_eq!(source.groups()[0].artifact_indices(), [1]);
    let generated = &selection.targets()[3];
    assert_eq!(
        generated
            .groups()
            .iter()
            .map(|group| group.name())
            .collect::<Vec<_>>(),
        ["_validation", "default"]
    );
    assert_eq!(generated.groups()[0].artifact_indices(), [2]);
    assert_eq!(generated.groups()[1].artifact_indices(), [3]);
    assert_eq!(
        value
            .analyses()
            .filter(|owner| owner.key().label().target().as_str() == "bundle")
            .count(),
        1
    );
    assert_eq!(
        value.declared_action_count(),
        6,
        "unrequested output action stays in closure but not artifact selection"
    );
}

#[test]
fn native_requested_selection_retains_runfiles_tree_and_distinguishes_empty_from_unsupported() {
    let workspace = Workspace::new();
    let accepted = workspace.build(&["//:binary"]);
    let selection = evaluation(&accepted).requested_artifacts().unwrap();
    let hidden = selection.targets()[0]
        .groups()
        .iter()
        .find(|group| group.name() == "_hidden_top_level_INTERNAL_")
        .unwrap();
    let [index] = hidden.artifact_indices() else {
        panic!("binary hidden group must select its runfiles tree")
    };
    assert!(
        matches!(&selection.artifacts()[*index], AnalysisArtifact::Derived { owner, output } if owner.label().target().as_str() == "binary" && output.kind() == ActionOutputKind::RunfilesTree)
    );
    for requested in [vec![], vec!["//:empty"]] {
        let accepted = workspace.build(&requested);
        let value = evaluation(&accepted);
        let selection = value.requested_artifacts().unwrap();
        assert_eq!(selection.targets().len(), requested.len());
        assert!(
            selection
                .targets()
                .iter()
                .all(|target| target.groups().is_empty())
        );
        assert!(selection.artifacts().is_empty());
        assert_eq!(value.declared_action_count(), 0);
    }
    let native = workspace.build(&["//:native"]);
    assert!(evaluation(&native).requested_artifacts().is_err());

    // Wildcard acceptance/expansion remains deferred. Check its existing
    // loading-only metadata key without claiming native command acceptance.
    let key = BuildCommandRootObservationKey::new(
        BuildCommandRootKey::new(
            slug_workspace_v2::NormalizedAbsolutePath::new(workspace.runtime.workspace.clone())
                .unwrap(),
            &[TargetPattern::parse("//:all").unwrap()],
            selection.targets()[0]
                .requested_target()
                .configured_target()
                .unwrap()
                .configuration()
                .clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let observed = workspace.runtime.runtime.block_on(async {
        let mut transaction = workspace
            .runtime
            .dice
            .updater_with_data(workspace.runtime.user_computation_data(None).unwrap())
            .existing_state()
            .await;
        transaction.compute(&key).await.unwrap()
    });
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
        panic!("wildcard metadata must complete: {observed:?}")
    };
    assert!(
        observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .requested_artifacts()
            .is_err()
    );
    let bad_alias = workspace.build(&["//:bad_alias"]);
    assert!(
        bad_alias.terminal_for_test().as_ref().is_err(),
        "unsupported actual kind must reject activation"
    );
}

#[test]
fn native_requested_source_and_group_mutations_restore_on_one_runtime() {
    let workspace = Workspace::new();
    let mut selected = Vec::new();
    let mut observed = Vec::new();
    for (bytes, group) in [("aaa", "group_a"), ("bbb", "group_b"), ("aaa", "group_a")] {
        fs::write(workspace.root.path().join("input"), bytes).unwrap();
        fs::write(
            workspace.root.path().join("defs.bzl"),
            DEFS.replace("GROUP_VARIANT", group),
        )
        .unwrap();
        let source = workspace.build(&["//:source_alias"]);
        let value = evaluation(&source);
        let selection = value.requested_artifacts().unwrap();
        assert_eq!(
            selection.targets()[0]
                .requested_target()
                .label()
                .target()
                .as_str(),
            "source_alias"
        );
        assert_eq!(
            selection.targets()[0].actual_target(),
            &ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:input").unwrap())
        );
        assert_eq!(
            selection.artifacts(),
            &[AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:input").unwrap()
            )]
        );
        assert_eq!(value.declared_action_count(), 0);
        let content = source_content(&workspace, value, "input", bytes.as_bytes());
        assert!(matches!(
            content.as_ref(),
            PathObservationResult::FileDigest(_)
        ));
        observed.push(content);
        let bundle = workspace.build(&["//:bundle"]);
        selected.push(
            evaluation(&bundle)
                .requested_artifacts()
                .unwrap()
                .artifacts()
                .to_vec(),
        );
    }
    assert_ne!(observed[0], observed[1]);
    assert_eq!(observed[0], observed[2]);
    assert_ne!(selected[0], selected[1]);
    assert_eq!(selected[0], selected[2]);
}

#[test]
fn native_requested_external_source_retains_canonical_identity_and_source_frontier() {
    let workspace = Workspace::new();
    let module = fs::read_to_string(workspace.root.path().join("MODULE.bazel")).unwrap();
    fs::write(workspace.root.path().join("MODULE.bazel"), format!("{module}\nbazel_dep(name='dep', version='1.0.0')\nlocal_path_override(module_name='dep', path='dep')\n")).unwrap();
    fs::write(
        workspace.root.path().join("BUILD.bazel"),
        format!("{BUILD}\nalias(name='external_alias', actual='@dep//pkg:source.txt')\n"),
    )
    .unwrap();
    fs::create_dir_all(workspace.root.path().join("dep/pkg/directory")).unwrap();
    fs::write(
        workspace.root.path().join("dep/MODULE.bazel"),
        "module(name='dep', version='1.0.0')\n",
    )
    .unwrap();
    fs::write(
        workspace.root.path().join("dep/pkg/BUILD.bazel"),
        "exports_files(['source.txt', 'directory'])\n",
    )
    .unwrap();
    let label = CanonicalLabel::parse("@@dep+//pkg:source.txt").unwrap();
    let mut alias_content = Vec::new();
    for bytes in ["first", "other"] {
        fs::write(workspace.root.path().join("dep/pkg/source.txt"), bytes).unwrap();
        let accepted = workspace.build(&["@dep//pkg:source.txt"]);
        let value = evaluation(&accepted);
        let selection = value.requested_artifacts().unwrap();
        assert_eq!(
            selection.artifacts(),
            &[AnalysisArtifact::Source(label.clone())]
        );
        assert_eq!(
            selection.targets()[0].actual_target(),
            &ConfiguredNodeKey::null(label.clone())
        );
        let content = source_content(&workspace, value, "dep/pkg/source.txt", bytes.as_bytes());
        assert!(matches!(
            content.as_ref(),
            PathObservationResult::FileBytes(_)
        ));
        assert_eq!(value.declared_action_count(), 0);

        let accepted = workspace.build(&["//:external_alias"]);
        let value = evaluation(&accepted);
        let selection = value.requested_artifacts().unwrap();
        assert_eq!(
            selection.targets()[0].requested_target().label(),
            &CanonicalLabel::parse("@@//:external_alias").unwrap()
        );
        assert_eq!(
            selection.targets()[0].actual_target(),
            &ConfiguredNodeKey::null(label.clone())
        );
        assert_eq!(
            selection.artifacts(),
            &[AnalysisArtifact::Source(label.clone())]
        );
        let content = source_content(&workspace, value, "dep/pkg/source.txt", bytes.as_bytes());
        assert!(matches!(
            content.as_ref(),
            PathObservationResult::FileDigest(_)
        ));
        alias_content.push(content);
        assert_eq!(value.declared_action_count(), 0);
    }
    assert_ne!(alias_content[0], alias_content[1]);
    let directory = workspace.build(&["@dep//pkg:directory"]);
    assert!(evaluation(&directory).requested_artifacts().is_err());
}
