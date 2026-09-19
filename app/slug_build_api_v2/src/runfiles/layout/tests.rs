use std::sync::Arc;

use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionOutput;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::Depset;
use crate::DepsetOrder;
use crate::RetainedRunfiles;
use crate::RunfilesSymlink;

fn source(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}

fn derived(
    label: &str,
    configuration: &[u8],
    path: &str,
    kind: ActionOutputKind,
) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            configuration,
        ),
        output: ActionOutput::new(path, kind),
    }
}

fn file(path: &str) -> AnalysisArtifact {
    derived("@@//:binary", b"config", path, ActionOutputKind::File)
}

fn artifacts(values: Vec<AnalysisArtifact>, children: Vec<AnalysisDepset>) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        values.into_iter().map(AnalysisValue::artifact).collect(),
        children,
    )
    .unwrap()
}

fn links(values: Vec<(&str, AnalysisArtifact)>) -> crate::RunfilesSymlinkDepset {
    Depset::from_direct(
        DepsetOrder::Default,
        values
            .into_iter()
            .map(|(path, artifact)| RunfilesSymlink::new(path, artifact))
            .collect(),
    )
    .unwrap()
}

fn support(runfiles: RetainedRunfiles) -> RunfilesSupport {
    RunfilesSupport {
        runfiles,
        tree: derived(
            "@@//:binary",
            b"config",
            "binary.runfiles",
            ActionOutputKind::RunfilesTree,
        ),
        input_manifest: file("binary.runfiles_manifest"),
        manifest: Some(file("binary.runfiles/MANIFEST")),
        repo_mapping_manifest: Some(file("binary.repo_mapping")),
    }
}

fn target<'a>(layout: &'a RunfilesLayout<'_>, path: &str) -> &'a RunfilesLayoutTarget {
    layout
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .unwrap()
        .target()
}

#[test]
fn canonical_repository_placement_and_physical_manifest_are_separate() {
    let main = source("@@//pkg:input");
    let external = source("@@dep+//pkg:input");
    let generated = derived(
        "@@dep+//pkg:producer",
        b"other",
        "pkg/generated",
        ActionOutputKind::File,
    );
    let tree = derived("@@//:tree", b"config", "tree", ActionOutputKind::Directory);
    let mut runfiles = RetainedRunfiles::empty();
    runfiles.files = artifacts(
        vec![
            main.clone(),
            external.clone(),
            generated.clone(),
            tree.clone(),
        ],
        vec![],
    );
    runfiles.symlinks = links(vec![("alias", external.clone())]);
    runfiles.root_symlinks = links(vec![
        ("root", generated.clone()),
        ("\u{e000}", main.clone()),
        ("\u{10000}", main.clone()),
    ]);
    let support = support(runfiles);
    let layout = support.layout().unwrap();
    assert!(std::ptr::eq(layout.support(), &support));
    assert_eq!(
        target(&layout, "_main/pkg/input"),
        &RunfilesLayoutTarget::Artifact(main.clone())
    );
    assert_eq!(
        target(&layout, "dep+/pkg/input"),
        &RunfilesLayoutTarget::Artifact(external.clone())
    );
    assert_eq!(
        target(&layout, "dep+/pkg/generated"),
        &RunfilesLayoutTarget::Artifact(generated.clone())
    );
    assert_eq!(
        target(&layout, "_main/tree"),
        &RunfilesLayoutTarget::Artifact(tree)
    );
    assert_eq!(
        target(&layout, "_main/alias"),
        &RunfilesLayoutTarget::Artifact(external)
    );
    assert_eq!(
        target(&layout, "root"),
        &RunfilesLayoutTarget::Artifact(generated)
    );
    assert_eq!(
        target(&layout, "_repo_mapping"),
        &RunfilesLayoutTarget::Artifact(support.repo_mapping_manifest.clone().unwrap())
    );
    assert_eq!(
        layout
            .entries()
            .iter()
            .map(RunfilesLayoutEntry::path)
            .collect::<Vec<_>>(),
        [
            "_main/alias",
            "_main/pkg/input",
            "_main/tree",
            "_repo_mapping",
            "dep+/pkg/generated",
            "dep+/pkg/input",
            "root",
            "\u{e000}",
            "\u{10000}",
        ]
    );
    let manifest = layout.manifest_link().unwrap();
    assert!(std::ptr::eq(
        manifest.output(),
        support.manifest.as_ref().unwrap()
    ));
    assert!(std::ptr::eq(manifest.target(), &support.input_manifest));
    assert!(
        !layout
            .entries()
            .iter()
            .any(|entry| entry.path() == "MANIFEST")
    );
    assert_eq!(layout.constituents().len(), 7);
    assert!(!layout.constituents().contains(&support.tree));
    assert!(layout.diagnostics().is_empty());
}

#[test]
fn overwrite_order_empty_entries_and_complete_constituents_follow_source() {
    let first = source("@@//:first");
    let second = source("@@//:second");
    let canonical = source("@@//pkg:item");
    let root_override = source("@@//:root_override");
    let mut runfiles = RetainedRunfiles::empty();
    runfiles.symlinks = links(vec![
        ("same", first.clone()),
        ("same", second.clone()),
        ("pkg/item", first.clone()),
        ("empty", first.clone()),
        ("under", first.clone()),
    ]);
    runfiles.files = artifacts(vec![canonical.clone()], vec![]);
    runfiles.empty_filenames = Depset::from_direct(
        DepsetOrder::Default,
        vec!["empty".to_owned(), "under/empty".to_owned()],
    )
    .unwrap();
    runfiles.root_symlinks = links(vec![
        ("_main/pkg/item", root_override.clone()),
        ("_repo_mapping", first.clone()),
        ("_main/same", root_override.clone()),
        ("_main/under/root", second.clone()),
        ("MANIFEST", first.clone()),
    ]);
    let support = support(runfiles);
    let layout = support.layout().unwrap();
    assert_eq!(
        target(&layout, "_main/same"),
        &RunfilesLayoutTarget::Artifact(root_override.clone())
    );
    assert_eq!(
        target(&layout, "_main/pkg/item"),
        &RunfilesLayoutTarget::Artifact(root_override)
    );
    assert_eq!(target(&layout, "_main/empty"), &RunfilesLayoutTarget::Empty);
    // Empty entries and root symlinks arrive after obscuring-prefix filtering.
    assert_eq!(
        target(&layout, "_main/under/empty"),
        &RunfilesLayoutTarget::Empty
    );
    assert_eq!(
        target(&layout, "_main/under/root"),
        &RunfilesLayoutTarget::Artifact(second.clone())
    );
    assert_eq!(
        target(&layout, "MANIFEST"),
        &RunfilesLayoutTarget::Artifact(first.clone())
    );
    assert_eq!(
        layout.manifest_link().unwrap().target(),
        &support.input_manifest
    );
    assert_eq!(
        target(&layout, "_repo_mapping"),
        &RunfilesLayoutTarget::Artifact(support.repo_mapping_manifest.clone().unwrap())
    );
    assert!(
        layout.diagnostics().is_empty(),
        "same-path replacement is silent"
    );
    for artifact in [&first, &second, &canonical] {
        assert!(
            layout.constituents().contains(artifact),
            "overridden dependency {artifact:?}"
        );
    }
    assert_eq!(layout.constituents().len(), 7);

    // Without the later root override, ordinary artifacts replace symlink entries.
    let mut without_roots = support.clone();
    without_roots.runfiles.root_symlinks = Depset::empty();
    let layout = without_roots.layout().unwrap();
    assert_eq!(
        target(&layout, "_main/pkg/item"),
        &RunfilesLayoutTarget::Artifact(canonical)
    );
    assert_eq!(
        target(&layout, "_main/same"),
        &RunfilesLayoutTarget::Artifact(second)
    );
}

#[test]
fn obscuring_prefix_uses_repository_configuration_namespace_and_policy() {
    for policy in [RunfilesConflictPolicy::Warn, RunfilesConflictPolicy::Error] {
        let parent = derived("@@//:left", b"A", "base", ActionOutputKind::Directory);
        let child = derived("@@//:right", b"A", "base/child", ActionOutputKind::File);
        let foreign_config = derived("@@//:right", b"B", "base/child", ActionOutputKind::File);
        let foreign_repo = derived("@@dep+//:right", b"A", "base/child", ActionOutputKind::File);
        let mut runfiles = RetainedRunfiles::empty();
        runfiles.conflict_policy = policy;
        runfiles.symlinks = links(vec![
            ("benign", parent.clone()),
            ("benign/child", child.clone()),
            ("config", parent.clone()),
            ("config/child", foreign_config.clone()),
            ("repo", parent.clone()),
            ("repo/child", foreign_repo.clone()),
            ("source", source("@@//:base")),
            ("source/child", source("@@//:base/child")),
            ("kind", source("@@//:base")),
            ("kind/child", child.clone()),
            ("punct", parent.clone()),
            ("punct.other", child.clone()),
            ("punct/child", foreign_config.clone()),
        ]);
        let support = support(runfiles);
        let layout = support.layout().unwrap();
        assert_eq!(layout.diagnostics().len(), 4);
        for diagnostic in layout.diagnostics() {
            let RunfilesLayoutDiagnostic::Obscured {
                policy: actual,
                path,
                ancestor_path,
                ..
            } = diagnostic
            else {
                panic!("prefix diagnostic")
            };
            assert_eq!(*actual, policy);
            assert_eq!(path.as_str(), format!("{ancestor_path}/child"));
        }
        assert_eq!(
            layout
                .entries()
                .iter()
                .filter(|entry| entry.path().ends_with("/child"))
                .count(),
            0
        );
        assert_eq!(
            target(&layout, "_main/punct.other"),
            &RunfilesLayoutTarget::Artifact(child.clone())
        );
        for artifact in [parent, child, foreign_config, foreign_repo] {
            assert!(layout.constituents().contains(&artifact));
        }
    }
}

#[test]
fn external_only_fallback_nested_tree_diagnostics_and_invalid_paths() {
    let external = source("@@dep+//:input");
    let nested = derived(
        "@@//:nested",
        b"config",
        "nested.runfiles",
        ActionOutputKind::RunfilesTree,
    );
    let mut runfiles = RetainedRunfiles::empty();
    runfiles.files = artifacts(vec![external.clone(), nested.clone()], vec![]);
    runfiles.root_symlinks = links(vec![("nested-root", nested.clone())]);
    let support = support(runfiles);
    let layout = support.layout().unwrap();
    assert_eq!(
        target(&layout, "_main/.runfile"),
        &RunfilesLayoutTarget::Empty
    );
    assert_eq!(
        target(&layout, "dep+/input"),
        &RunfilesLayoutTarget::Artifact(external)
    );
    assert_eq!(layout.diagnostics().len(), 2);
    assert!(
        layout
            .diagnostics()
            .iter()
            .all(|diagnostic| matches!(diagnostic,
        RunfilesLayoutDiagnostic::NestedRunfilesTree { artifact, .. } if artifact == &nested))
    );
    assert!(layout.constituents().contains(&nested));
    assert!(
        !layout
            .entries()
            .iter()
            .any(|entry| entry.path().contains("nested"))
    );
    let mut self_reference = support.clone();
    self_reference.runfiles.root_symlinks = links(vec![("self", self_reference.tree.clone())]);
    let self_layout = self_reference.layout().unwrap();
    assert!(!self_layout.constituents().contains(&self_reference.tree));
    assert!(self_layout.diagnostics().iter().any(|diagnostic| matches!(diagnostic,
        RunfilesLayoutDiagnostic::NestedRunfilesTree { artifact, .. } if artifact == &self_reference.tree)));
    for path in ["", "/absolute", "a/../b", "a//b", "a/./b", "nul\0name"] {
        let mut malformed = support.clone();
        malformed.runfiles.root_symlinks = links(vec![(path, file("value"))]);
        assert_eq!(
            malformed.layout().unwrap_err(),
            RunfilesLayoutError::InvalidPath(path.into())
        );
    }
    let mut root_present = support.clone();
    root_present.runfiles.root_symlinks = links(vec![("_main/visible", file("value"))]);
    assert!(
        !root_present
            .layout()
            .unwrap()
            .entries()
            .iter()
            .any(|entry| entry.path() == "_main/.runfile")
    );
}

#[test]
fn projections_preserve_shared_depsets_and_distinct_structural_constituents() {
    let first = file("same");
    let other_owner = derived("@@//:other", b"config", "same", ActionOutputKind::File);
    let other_config = derived("@@//:binary", b"other", "same", ActionOutputKind::File);
    let mut same_namespace_different_platform = first.clone();
    if let AnalysisArtifact::Derived { owner, .. } = &mut same_namespace_different_platform {
        *owner = owner.clone().with_toolchain_execution_platform(Arc::new(
            CanonicalLabel::parse("@@//:platform").unwrap(),
        ));
    }
    let shared = artifacts(
        vec![
            first.clone(),
            other_owner.clone(),
            other_config.clone(),
            same_namespace_different_platform.clone(),
        ],
        vec![],
    );
    let mut runfiles = RetainedRunfiles::empty();
    runfiles.files = artifacts(vec![], vec![shared.clone(), shared.clone()]);
    let support = support(runfiles);
    let held = support.clone();
    let before = support.runfiles.files.storage_stats();
    let a = support.layout().unwrap();
    let b = support.layout().unwrap();
    assert_eq!(a.entries(), b.entries());
    assert_eq!(a.constituents(), b.constituents());
    assert!(
        support
            .runfiles
            .files
            .shares_occurrence_with(&held.runfiles.files)
    );
    assert!(
        support
            .runfiles
            .files
            .shares_store_with(&held.runfiles.files)
    );
    assert_eq!(support.runfiles.files.storage_stats(), before);
    for artifact in [
        first,
        other_owner,
        other_config,
        same_namespace_different_platform.clone(),
    ] {
        assert_eq!(
            a.constituents()
                .iter()
                .filter(|value| **value == artifact)
                .count(),
            1
        );
    }
    assert_eq!(
        target(&a, "_main/same"),
        &RunfilesLayoutTarget::Artifact(same_namespace_different_platform)
    );
    assert_eq!(a.constituents().len(), 7);
}
