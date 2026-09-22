use std::hash::Hash;
use std::hash::Hasher;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::ActionKind;
use crate::ActionRegistry;
use crate::ActionSpec;
use crate::CtxActions;
use crate::ReapiCommandProjection;
use crate::WorkspaceStatusArtifacts;
use crate::artifact_root::test_support as roots;

fn recipe(kind: BuildInfoTransformKind) -> BuildInfoCallbackRecipe {
    BuildInfoCallbackRecipe::new(
        kind,
        CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
        BuildInfoCallbackRecipe::SOURCE_DIGEST,
    )
    .unwrap()
}
fn template(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@bazel_tools//templates:{name}")).unwrap(),
    )
}
fn spec(
    kind: BuildInfoTransformKind,
    workspace: &str,
    name: &str,
    root: crate::DerivedArtifactRoot,
    output: &str,
) -> BuildInfoTransformSpec {
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new(workspace).unwrap());
    let (status, output) = match kind {
        BuildInfoTransformKind::Info => (
            status.stable(),
            ActionOutput::new(root, output, ActionOutputKind::File),
        ),
        BuildInfoTransformKind::Version => {
            (status.volatile(), ActionOutput::new_constant(root, output))
        }
    };
    BuildInfoTransformSpec::new(status.dupe(), template(name), output, recipe(kind)).unwrap()
}

#[test]
fn build_info_specs_preserve_all_owned_inputs_and_metadata() {
    let a = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "volatile",
        roots::root(),
        "pkg/v.h",
    );
    assert_eq!(
        a.inputs(),
        &[
            AnalysisArtifact::WorkspaceStatus(a.status().dupe()),
            template("volatile")
        ]
    );
    assert!(a.is_volatile() && a.execute_unconditionally());
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (template, count) in [(status.volatile(), 1), (status.stable(), 2)] {
        let input = AnalysisArtifact::WorkspaceStatus(template.dupe());
        let same_role = BuildInfoTransformSpec::new(
            status.volatile().dupe(),
            input.clone(),
            a.output().dupe(),
            recipe(BuildInfoTransformKind::Version),
        )
        .unwrap();
        assert_eq!(same_role.inputs().len(), count);
        assert_eq!(
            same_role.template(),
            &input,
            "role retention is independent of input deduplication"
        );
        assert_eq!(
            same_role.inputs()[0],
            AnalysisArtifact::WorkspaceStatus(status.volatile().dupe())
        );
    }
    assert_eq!(a.output().metadata(), ActionOutputMetadata::Constant);
    let action = ActionSpec::build_info_transform(a.dupe());
    assert_eq!(action.kind(), &ActionKind::BuildInfoTransform);
    assert_eq!(action.mnemonic(), "TranslateBuildInfo");
    assert_eq!(action.outputs(), &[a.output().dupe()]);
    assert_eq!(
        action.progress_message(),
        Some("Translating volatile BuildInfo file")
    );
    assert!(action.render_argv().is_err());
    assert!(ReapiCommandProjection::from_action(&action).is_err());
    for other in [
        spec(
            BuildInfoTransformKind::Info,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/other",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "different",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root_for(b"B"),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/other.h",
        ),
    ] {
        assert_ne!(a, other);
    }
    assert_eq!(
        a,
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h"
        )
    );
    assert!(
        BuildInfoCallbackRecipe::new(
            BuildInfoTransformKind::Info,
            CanonicalLabel::parse("@@foreign//tools/build_defs/build_info:bazel_cc_build_info.bzl")
                .unwrap(),
            BuildInfoCallbackRecipe::SOURCE_DIGEST
        )
        .is_err()
    );
    assert!(
        BuildInfoCallbackRecipe::new(
            BuildInfoTransformKind::Info,
            CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
            [0; 32]
        )
        .is_err()
    );
}

#[test]
fn constant_metadata_is_orthogonal_and_cannot_enter_other_action_families() {
    let ordinary = ActionOutput::new(roots::root(), "out", ActionOutputKind::File);
    let constant = ActionOutput::new_constant(roots::root(), "out");
    assert_eq!(ordinary.path(), constant.path());
    assert_eq!(ordinary.kind(), constant.kind());
    assert_ne!(ordinary, constant);
    assert_ne!(ordinary.cmp(&constant), std::cmp::Ordering::Equal);
    let hash = |output: &ActionOutput| {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        output.hash(&mut hash);
        hash.finish()
    };
    assert_ne!(hash(&ordinary), hash(&constant));
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (status, output, kind) in [
        (
            status.volatile(),
            ordinary.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "dir", ActionOutputKind::Directory),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "../escape", ActionOutputKind::File),
            BuildInfoTransformKind::Info,
        ),
    ] {
        assert!(
            BuildInfoTransformSpec::new(status.dupe(), template("t"), output, recipe(kind))
                .is_err()
        );
    }
    for action in [
        ActionSpec::new(
            ActionKind::BuildInfoTransform,
            "TranslateBuildInfo",
            vec![ordinary],
        ),
        ActionSpec::new(
            ActionKind::Write {
                content: "fake".into(),
                is_executable: false,
            },
            "FileWrite",
            vec![constant],
        ),
    ] {
        assert!(ActionRegistry::new().register(action.clone()).is_err());
        assert!(ReapiCommandProjection::from_action(&action).is_err());
    }
}

#[test]
fn build_info_registration_is_ordered_atomic_and_conflicts_across_metadata() {
    let mut actions = CtxActions::new(roots::root());
    let volatile = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "v",
        roots::root(),
        "pkg/v.h",
    );
    let stable = spec(
        BuildInfoTransformKind::Info,
        "/workspace",
        "i",
        roots::root(),
        "pkg/i.h",
    );
    assert_eq!(
        actions
            .register_build_info_transform(volatile.dupe())
            .unwrap(),
        0
    );
    assert_eq!(
        actions
            .register_build_info_transform(stable.dupe())
            .unwrap(),
        1
    );
    let before = actions.clone();
    assert!(
        actions
            .write(
                ActionOutput::new(roots::root(), "pkg/v.h", ActionOutputKind::File),
                "collision",
                false
            )
            .is_err()
    );
    assert_eq!(before, actions);
    assert!(actions.register_build_info_transform(volatile).is_err());
    assert_eq!(before, actions);
    let mut registry = ActionRegistry::new();
    let stable = ActionSpec::build_info_transform(stable);
    assert!(registry.register_batch([stable.clone(), stable]).is_err());
    assert!(registry.actions().is_empty());
    let malformed = ActionSpec::new(
        ActionKind::Write {
            content: "x".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new_constant(roots::root(), "bad")],
    );
    assert!(actions.replace_actions(vec![malformed]).is_err());
    assert_eq!(before, actions);
}
