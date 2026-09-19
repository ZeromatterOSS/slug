//! Native preflight and physical publication projections of ordinary binary roots.
use super::*;
use crate::runtime::PreparedActionChainInputs;
use crate::runtime::PreparedRunfilesAction;

fn prepare(
    workspace: &Workspace,
    target: &str,
) -> Result<AcceptedCommand<Arc<PreparedActionChainInputs>>, BuildCommandError> {
    workspace
        .runtime
        .prepare_requested_action_inputs_with_repository_environment(
            &[TargetPattern::parse(target).unwrap()],
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
}

#[test]
fn native_binary_preflight_observes_hidden_sources_and_couples_manifest_publication() {
    let workspace = Workspace::new();
    super::runfiles_manifest_tests::write_binary(&workspace, "hidden", ":input");
    let accepted = prepare(&workspace, "//:binary").unwrap();
    let inputs = accepted.terminal_for_test();
    let plan = inputs.plan().unwrap();
    assert_eq!(plan.actions().len(), 6);
    assert_eq!(inputs.sources().len(), 1);
    let mut local = Vec::new();
    for (index, step) in plan.actions().iter().enumerate() {
        if let Some(result) = inputs.prepare_runfiles_action(step.action()).unwrap() {
            local.push((index, result));
        }
    }
    assert_eq!(local.len(), 4);
    let tree_index = local
        .iter()
        .find_map(|(index, result)| {
            matches!(result, PreparedRunfilesAction::RunfilesTree { .. }).then_some(*index)
        })
        .unwrap();
    let tree_action = plan.actions()[tree_index].action();
    let original = tree_action.runfiles_support_spec().unwrap();
    for change_output in [true, false] {
        let mut support = original.support().clone();
        let output = if change_output {
            slug_build_api_v2::ActionOutput::new("forged.runfiles", ActionOutputKind::RunfilesTree)
        } else {
            Arc::make_mut(&mut support).runfiles.repository_prefix = "different_workspace".into();
            original.output().clone()
        };
        let node = slug_analysis_v2::ConfiguredNodeResult::new_rule(
            tree_action.context().owner().clone(),
            slug_build_api_v2::ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            slug_build_api_v2::RunfilesPackageDepset::empty(),
        )
        .with_action_specs(
            vec![slug_build_api_v2::ActionSpec::runfiles_support(
                slug_build_api_v2::RunfilesSupportActionSpec::RunfilesTree { support, output },
            )],
            vec![tree_action.context().clone()],
        )
        .unwrap();
        let error = inputs
            .prepare_runfiles_action(&node.actions()[0])
            .unwrap_err();
        assert!(
            error.contains(if change_output {
                "exact support artifact"
            } else {
                "exact retained support"
            }),
            "{error}"
        );
    }
    let links = inputs.runfiles_links(tree_index).unwrap();
    assert!(
        links
            .entries
            .iter()
            .any(|(path, target)| path == "_main/hidden"
                && target.as_ref().unwrap().ends_with("/hidden"))
    );
    let owner = crate::runtime::configured_output::ConfiguredOutputOwner::new(
        workspace.root.path().to_owned(),
    );
    let stages = owner.stage_prepared_outputs(inputs).unwrap();
    assert_eq!(stages.last().unwrap().action_index(), tree_index);
    let outputs = stages
        .iter()
        .flat_map(|stage| stage.staging().outputs())
        .map(|out| out.path())
        .collect::<Vec<_>>();
    assert!(outputs.contains(&"hidden"));
    assert!(outputs.contains(&"binary.bin.runfiles_manifest"));
    assert!(outputs.contains(&"binary.bin.repo_mapping"));
    assert!(!outputs.contains(&"binary.bin.runfiles/MANIFEST"));
    assert!(
        stages
            .last()
            .unwrap()
            .staging()
            .create_file(0, "forged")
            .is_err()
    );
    // Preparation/staging has not installed any declared artifact.
    for node in inputs
        .evaluation()
        .unwrap()
        .analyses()
        .filter(|node| !node.actions().is_empty())
    {
        let root = crate::runtime::configured_output_root(
            workspace.root.path(),
            node.configured_target_key()
                .unwrap()
                .configuration()
                .slug_configuration()
                .unwrap(),
        );
        for action in node.actions() {
            for out in action.outputs() {
                assert!(fs::symlink_metadata(root.join(out.path())).is_err());
            }
        }
    }
}

#[test]
fn native_runfiles_manifest_overlay_preserves_authored_bytes_and_rejects_late_prefix_conflicts() {
    let workspace = Workspace::new();
    let source = "ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list())";
    let defs = DEFS.replace("GROUP_VARIANT", "group_a");
    fs::write(workspace.root.path().join("defs.bzl"),defs.replace(source,"ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list(), root_symlinks = {'MANIFEST': out})")).unwrap();
    let accepted = prepare(&workspace, "//:binary").unwrap();
    let inputs = accepted.terminal_for_test();
    let plan = inputs.plan().unwrap();
    let index = plan
        .actions()
        .iter()
        .position(|step| {
            matches!(
                step.action().runfiles_support_spec(),
                Some(slug_build_api_v2::RunfilesSupportActionSpec::RunfilesTree { .. })
            )
        })
        .unwrap();
    let links = inputs.runfiles_links(index).unwrap();
    let manifest = links
        .entries
        .iter()
        .find(|(path, _)| path == "MANIFEST")
        .unwrap();
    assert!(
        manifest
            .1
            .as_ref()
            .unwrap()
            .ends_with("/binary.bin.runfiles_manifest")
    );
    let source_bytes = plan
        .actions()
        .iter()
        .find_map(|step| {
            if matches!(
                step.action().runfiles_support_spec(),
                Some(slug_build_api_v2::RunfilesSupportActionSpec::SourceSymlinkManifest { .. })
            ) {
                match inputs
                    .prepare_runfiles_action(step.action())
                    .unwrap()
                    .unwrap()
                {
                    PreparedRunfilesAction::Manifest { bytes, .. } => Some(bytes),
                    _ => unreachable!(),
                }
            } else {
                None
            }
        })
        .unwrap();
    let source_text = std::str::from_utf8(&source_bytes).unwrap();
    assert!(
        source_text
            .lines()
            .any(|line| line.starts_with("MANIFEST ") && line.ends_with("/binary.bin"))
    );
    drop(accepted);
    for roots in [
        "{'MANIFEST/child': out}",
        "{'under': out, 'under/child': out}",
        "{'_main': out}",
    ] {
        fs::write(workspace.root.path().join("defs.bzl"),defs.replace(source,&format!("ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list(), root_symlinks = {roots})"))).unwrap();
        let error = prepare(&workspace, "//:binary").unwrap_err().to_string();
        assert!(
            error.contains("conflicts with a required directory"),
            "{error}"
        );
    }
}

#[test]
fn native_runfiles_warnings_are_selected_once_and_restore_after_removal() {
    let workspace = Workspace::new();
    fs::create_dir(workspace.root.path().join("under")).unwrap();
    fs::write(workspace.root.path().join("under/input"), "source").unwrap();
    fs::write(workspace.root.path().join("BUILD.bazel"),"load(':defs.bzl','binary')\nplatform(name='platform')\nexports_files(['under/input'])\nbinary(name='binary',source='under/input')\n").unwrap();
    let source = "ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list())";
    let defs = DEFS.replace("GROUP_VARIANT","group_a").replace("    out = ctx.actions.declare_file(\"binary.bin\")", "    under = ctx.actions.declare_file('under')\n    ctx.actions.write(under, 'backing')\n    out = ctx.actions.declare_file(\"binary.bin\")");
    let changed = defs.replace(
        source,
        "ctx.runfiles(files = [under] + ctx.attr.source[DefaultInfo].files.to_list())",
    );
    let expected =
        "WARNING: runfiles symlink under/input -> under/input obscured by under -> under\n";
    for (index, warning) in [true, true, false, true].into_iter().enumerate() {
        if index != 1 {
            fs::write(
                workspace.root.path().join("defs.bzl"),
                if warning { &changed } else { &defs },
            )
            .unwrap();
        }
        let accepted = prepare(&workspace, "//:binary").unwrap();
        let (_, status, stdout, stderr) = accepted
            .project(|_| crate::runtime::TerminalOutput::new(0, String::new(), String::new()))
            .publish()
            .into_parts();
        assert_eq!(status, 0);
        assert!(stdout.is_empty());
        assert_eq!(stderr, if warning && index != 1 { expected } else { "" });
    }
}
