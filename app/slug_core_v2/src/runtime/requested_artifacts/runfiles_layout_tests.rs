//! Configured binary layout remains metadata until all runfiles effects are admitted.
use slug_build_api_v2::RunfilesLayout;
use slug_build_api_v2::RunfilesLayoutTarget;

use super::*;

fn check_layout<'a>(
    workspace: &Workspace,
    accepted: &'a AcceptedBuild,
    generated_name: &str,
    source_name: &str,
) -> RunfilesLayout<'a> {
    let value = evaluation(accepted);
    let analysis = value
        .analyses()
        .find(|analysis| analysis.key().label().target().as_str() == "binary")
        .unwrap();
    let info = analysis.providers().default_info().unwrap();
    let support = info.files_to_run.support.as_ref().unwrap();
    let layout = support.layout().unwrap();
    assert!(std::ptr::eq(layout.support(), support.as_ref()));
    assert!(layout.diagnostics().is_empty());

    let selection = value.requested_artifacts().unwrap();
    let hidden = selection.targets()[0]
        .groups()
        .iter()
        .find(|group| group.name() == "_hidden_top_level_INTERNAL_")
        .unwrap();
    let [tree] = hidden.artifact_indices() else {
        panic!("one hidden runfiles tree")
    };
    assert_eq!(&selection.artifacts()[*tree], &support.tree);

    let executable = info.executable.as_ref().unwrap();
    let AnalysisArtifact::Derived { owner, .. } = executable else {
        panic!("configured executable must be derived")
    };
    let generated = AnalysisArtifact::Derived {
        owner: owner.clone(),
        output: slug_build_api_v2::ActionOutput::new(generated_name, ActionOutputKind::File),
    };
    let source =
        AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//:{source_name}")).unwrap());
    let prefix = support.runfiles.repository_prefix.as_str();
    for (path, artifact) in [
        (format!("{prefix}/binary.bin"), executable),
        (format!("{prefix}/{generated_name}"), &generated),
        (format!("{prefix}/{source_name}"), &source),
        (
            "_repo_mapping".to_owned(),
            support.repo_mapping_manifest.as_ref().unwrap(),
        ),
    ] {
        let entry = layout
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .unwrap();
        assert_eq!(
            entry.target(),
            &RunfilesLayoutTarget::Artifact(artifact.clone())
        );
        assert!(layout.constituents().contains(artifact));
    }
    let default = selection.targets()[0]
        .groups()
        .iter()
        .find(|group| group.name() == "default")
        .unwrap();
    assert_eq!(default.artifact_indices().len(), 1);
    assert_eq!(
        &selection.artifacts()[default.artifact_indices()[0]],
        executable
    );
    assert!(!selection.artifacts().contains(&generated));
    assert!(!selection.artifacts().contains(&source));
    let manifest = layout.manifest_link().unwrap();
    assert_eq!(manifest.output(), support.manifest.as_ref().unwrap());
    assert_eq!(manifest.target(), &support.input_manifest);
    assert!(layout.constituents().contains(&support.input_manifest));
    assert!(
        layout
            .constituents()
            .contains(support.manifest.as_ref().unwrap())
    );
    assert!(
        !layout
            .entries()
            .iter()
            .any(|entry| entry.path() == "MANIFEST")
    );

    assert!(
        value
            .requested_action_prerequisites()
            .unwrap_err()
            .contains("unsupported")
    );
    for analysis in value
        .analyses()
        .filter(|analysis| !analysis.actions().is_empty())
    {
        let configuration = analysis
            .configured_target_key()
            .unwrap()
            .configuration()
            .slug_configuration()
            .unwrap();
        let root = crate::runtime::configured_output_root(workspace.root.path(), configuration);
        for output in analysis
            .actions()
            .iter()
            .flat_map(|action| action.outputs())
        {
            assert_eq!(
                fs::symlink_metadata(root.join(output.path()))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }
    layout
}

#[test]
fn configured_binary_layout_retains_backing_artifacts_and_held_a_b_a_without_effects() {
    let workspace = Workspace::new();
    let build = |variant: &str, source: &str| {
        let definitions = DEFS.replace("GROUP_VARIANT", "group_a").replace(
            "    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list()))]",
            &format!("    backing = ctx.actions.declare_file('{variant}')\n    ctx.actions.write(backing, 'backing bytes')\n    return [DefaultInfo(files = depset([out]), executable = out, runfiles = ctx.runfiles(files = [backing] + ctx.attr.source[DefaultInfo].files.to_list()))]"),
        );
        assert!(definitions.contains("backing ="));
        fs::write(workspace.root.path().join("defs.bzl"), definitions).unwrap();
        fs::write(
            workspace.root.path().join("BUILD.bazel"),
            BUILD.replace(
                "binary(name = \"binary\", source = \":input\")",
                &format!("binary(name = \"binary\", source = \":{source}\")"),
            ),
        )
        .unwrap();
        workspace.build(&["//:binary"])
    };
    let a = build("data_a", "input");
    let layout_a = check_layout(&workspace, &a, "data_a", "input");
    let b = build("data_b", "tool");
    let layout_b = check_layout(&workspace, &b, "data_b", "tool");
    let restored = build("data_a", "input");
    let layout_restored = check_layout(&workspace, &restored, "data_a", "input");
    assert_eq!(layout_a.entries(), layout_restored.entries());
    assert_eq!(layout_a.constituents(), layout_restored.constituents());
    assert_ne!(layout_a.entries(), layout_b.entries());
    assert_ne!(layout_a.constituents(), layout_b.constituents());
    assert_eq!(
        layout_a.entries(),
        check_layout(&workspace, &a, "data_a", "input").entries()
    );
    assert_eq!(
        layout_b.entries(),
        check_layout(&workspace, &b, "data_b", "tool").entries()
    );
}
