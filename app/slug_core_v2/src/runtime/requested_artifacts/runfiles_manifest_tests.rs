//! Native manifest preparation uses retained owners and observed source paths.
use slug_analysis_v2::ConfiguredTargetKey;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_workspace_v2::PathObservationOperation;

use super::*;
use crate::runtime::PreparedRunfilesManifests;

type Prepared = AcceptedCommand<Arc<PreparedRunfilesManifests>>;

fn owner(workspace: &Workspace, overlay: CommandConfigurationOverlay) -> ConfiguredTargetKey {
    let accepted = workspace
        .runtime
        .build_command_with_repository_environment(
            &[TargetPattern::parse("//:binary").unwrap()],
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            overlay,
        )
        .unwrap();
    evaluation(&accepted)
        .analyses()
        .find(|node| node.key().label().target().as_str() == "binary")
        .unwrap()
        .configured_target_key()
        .unwrap()
        .clone()
}

fn prepare(
    workspace: &Workspace,
    owner: ConfiguredTargetKey,
    overlay: CommandConfigurationOverlay,
) -> Result<Prepared, BuildCommandError> {
    workspace
        .runtime
        .prepare_runfiles_manifests_with_repository_environment(
            &[TargetPattern::parse("//:binary").unwrap()],
            owner,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            overlay,
        )
}

fn write_binary(workspace: &Workspace, backing: &str, source: &str) {
    fs::write(workspace.root.path().join("defs.bzl"), DEFS.replace("GROUP_VARIANT", "group_a").replace(
        "    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list()))]",
        &format!("    backing = ctx.actions.declare_file('{backing}')\n    ctx.actions.write(backing, 'backing')\n    return [DefaultInfo(files = depset([out]), executable = out, runfiles = ctx.runfiles(files = [backing] + ctx.attr.source[DefaultInfo].files.to_list()))]"),
    )).unwrap();
    fs::write(
        workspace.root.path().join("BUILD.bazel"),
        BUILD.replace(
            "binary(name = \"binary\", source = \":input\")",
            &format!("binary(name = \"binary\", source = \"{source}\")"),
        ),
    )
    .unwrap();
}

fn check_frontier(workspace: &Workspace, value: &PreparedRunfilesManifests) {
    for source in value.sources() {
        assert!(
            value
                .observations()
                .observations()
                .keys()
                .any(|demand| demand.namespace() == source.namespace()
                    && demand.path() == source.real_path()
                    && demand.operation() == PathObservationOperation::FileDigest)
        );
    }

    let state = workspace
        .runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap();
    for (demand, result) in value.observations().observations() {
        assert!(Arc::ptr_eq(
            result,
            state.accepted.path_observations.get(demand).unwrap()
        ));
    }
    assert!(
        value
            .observations()
            .observations()
            .keys()
            .any(
                |demand| demand.path().as_path() == workspace.root.path().join("MODULE.bazel")
                    && demand.operation() == PathObservationOperation::FileBytes
            )
    );
}

fn check_no_effects(workspace: &Workspace, value: &PreparedRunfilesManifests) {
    assert!(
        value
            .evaluation()
            .requested_action_prerequisites()
            .unwrap_err()
            .contains("unsupported")
    );
    for node in value
        .evaluation()
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
        for output in node.actions().iter().flat_map(|action| action.outputs()) {
            assert_eq!(
                fs::symlink_metadata(root.join(output.path()))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }
}

fn expected_manifest(
    workspace: &Workspace,
    key: &ConfiguredTargetKey,
    backing: &str,
    source: &str,
) -> Vec<u8> {
    let root = crate::runtime::configured_output_root(
        workspace.root.path(),
        key.configuration().slug_configuration().unwrap(),
    );
    format!("_main/binary.bin {}/binary.bin\n_main/{backing} {}/{backing}\n_main/{source} {}/{source}\n_repo_mapping {}/binary.bin.repo_mapping\n",
        root.display(),root.display(),workspace.root.path().display(),root.display()).into_bytes()
}

#[test]
fn native_manifest_bytes_and_source_content_restore_without_effects() {
    let workspace = Workspace::new();
    write_binary(&workspace, "data_a", ":input");
    let key = owner(&workspace, Default::default());
    let absent = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:absent").unwrap(),
        key.configuration().clone(),
    );
    assert!(
        prepare(&workspace, absent, Default::default())
            .unwrap_err()
            .to_string()
            .contains("owner is absent")
    );

    let a = prepare(&workspace, key.clone(), Default::default()).unwrap();
    let a = a.terminal_for_test();
    let bytes = a.source_manifest_bytes().unwrap();
    assert_eq!(
        bytes,
        expected_manifest(&workspace, &key, "data_a", "input")
    );
    assert_eq!(a.sources().len(), 1);
    check_frontier(&workspace, a);
    check_no_effects(&workspace, a);
    let digest_a = a.sources().next().unwrap().digest();
    fs::write(workspace.root.path().join("input"), "bbb").unwrap();
    let b = prepare(&workspace, key.clone(), Default::default()).unwrap();
    let b = b.terminal_for_test();
    assert_ne!(digest_a, b.sources().next().unwrap().digest());
    assert_eq!(bytes, b.source_manifest_bytes().unwrap());
    check_frontier(&workspace, b);
    fs::write(workspace.root.path().join("input"), "aaa").unwrap();
    let restored = prepare(&workspace, key.clone(), Default::default()).unwrap();
    assert_eq!(
        digest_a,
        restored
            .terminal_for_test()
            .sources()
            .next()
            .unwrap()
            .digest()
    );
    assert_eq!(
        bytes,
        restored
            .terminal_for_test()
            .source_manifest_bytes()
            .unwrap()
    );

    write_binary(&workspace, "data_b", ":tool");
    let changed = prepare(&workspace, key.clone(), Default::default()).unwrap();
    assert_eq!(
        changed.terminal_for_test().source_manifest_bytes().unwrap(),
        expected_manifest(&workspace, &key, "data_b", "tool")
    );
    assert_eq!(
        a.source_manifest_bytes().unwrap(),
        bytes,
        "held metadata never reopens current state"
    );
    write_binary(&workspace, "data_a", ":input");
    let restored = prepare(&workspace, key.clone(), Default::default()).unwrap();
    assert_eq!(
        bytes,
        restored
            .terminal_for_test()
            .source_manifest_bytes()
            .unwrap()
    );
    fs::remove_file(workspace.root.path().join("input")).unwrap();
    assert!(prepare(&workspace, key.clone(), Default::default()).is_err());
    assert_eq!(a.source_manifest_bytes().unwrap(), bytes);
    fs::write(workspace.root.path().join("input"), "aaa").unwrap();
    let repaired = prepare(&workspace, key, Default::default()).unwrap();
    assert_eq!(
        bytes,
        repaired
            .terminal_for_test()
            .source_manifest_bytes()
            .unwrap()
    );
    check_no_effects(&workspace, repaired.terminal_for_test());
}

#[cfg(unix)]
#[test]
fn native_manifest_source_symlink_keeps_requested_path_and_tracks_retargeting() {
    let workspace = Workspace::new();
    fs::write(workspace.root.path().join("input_a"), "aaa").unwrap();
    fs::write(workspace.root.path().join("input_b"), "bbb").unwrap();
    fs::remove_file(workspace.root.path().join("input")).unwrap();
    std::os::unix::fs::symlink("input_a", workspace.root.path().join("input")).unwrap();
    let key = owner(&workspace, Default::default());
    let a = prepare(&workspace, key.clone(), Default::default()).unwrap();
    let bytes = a.terminal_for_test().source_manifest_bytes().unwrap();
    let source_a = a.terminal_for_test().sources().next().unwrap().clone();
    assert_eq!(
        source_a.requested_path().as_path(),
        workspace.root.path().join("input")
    );
    assert_eq!(
        source_a.real_path().as_path(),
        workspace.root.path().join("input_a")
    );
    assert!(String::from_utf8(bytes.clone()).unwrap().contains(&format!(
        "_main/input {}\n",
        workspace.root.path().join("input").display()
    )));
    for (target, equal) in [("input_b", false), ("input_a", true)] {
        fs::remove_file(workspace.root.path().join("input")).unwrap();
        std::os::unix::fs::symlink(target, workspace.root.path().join("input")).unwrap();
        let next = prepare(&workspace, key.clone(), Default::default()).unwrap();
        let value = next.terminal_for_test();
        assert_eq!(bytes, value.source_manifest_bytes().unwrap());
        let source = value.sources().next().unwrap();
        assert_eq!(source.digest() == source_a.digest(), equal);
        assert_eq!(
            source.real_path().as_path(),
            workspace.root.path().join(target)
        );
        assert!(
            value
                .observations()
                .observations()
                .keys()
                .any(|demand| demand.operation() == PathObservationOperation::ReadLink)
        );
        check_frontier(&workspace, value);
    }
    assert_eq!(
        bytes,
        a.terminal_for_test().source_manifest_bytes().unwrap()
    );
}

#[test]
fn native_manifest_external_mapping_and_configuration_paths_are_owned() {
    let workspace = Workspace::new();
    let module = fs::read_to_string(workspace.root.path().join("MODULE.bazel")).unwrap();
    fs::write(workspace.root.path().join("MODULE.bazel"), format!("{module}\nbazel_dep(name='dep', version='1.0.0')\nlocal_path_override(module_name='dep', path='dep')\n")).unwrap();
    fs::create_dir_all(workspace.root.path().join("dep/pkg")).unwrap();
    fs::write(
        workspace.root.path().join("dep/MODULE.bazel"),
        "module(name='dep', version='1.0.0')\n",
    )
    .unwrap();
    fs::write(
        workspace.root.path().join("dep/pkg/BUILD.bazel"),
        "exports_files(['source.txt'])\n",
    )
    .unwrap();
    fs::write(workspace.root.path().join("dep/pkg/source.txt"), "external").unwrap();
    write_binary(&workspace, "data_a", "@dep//pkg:source.txt");
    let mut held = Vec::new();
    for mode in ["fastbuild", "opt", "fastbuild"] {
        let overlay =
            CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
                NativeCommandOption::CompilationMode,
                Some(mode),
                false,
            )]);
        let key = owner(&workspace, overlay.clone());
        let prepared = prepare(&workspace, key.clone(), overlay).unwrap();
        let value = prepared.terminal_for_test();
        let source = value.sources().next().unwrap();
        assert_eq!(
            source.label(),
            &CanonicalLabel::parse("@@dep+//pkg:source.txt").unwrap()
        );
        assert_eq!(
            source.requested_path().as_path(),
            workspace.root.path().join("dep/pkg/source.txt")
        );
        let manifest = String::from_utf8(value.source_manifest_bytes().unwrap()).unwrap();
        assert!(manifest.contains(&format!(
            "dep+/pkg/source.txt {}\n",
            workspace.root.path().join("dep/pkg/source.txt").display()
        )));
        let root = crate::runtime::configured_output_root(
            workspace.root.path(),
            key.configuration().slug_configuration().unwrap(),
        );
        assert!(manifest.contains(&format!("_main/binary.bin {}/binary.bin\n", root.display())));
        let mapping = String::from_utf8(value.repo_mapping_manifest_bytes().unwrap()).unwrap();
        assert!(mapping.lines().any(|line| line == ",dep,dep+"), "{mapping}");
        check_frontier(&workspace, value);
        check_no_effects(&workspace, value);
        held.push(prepared);
    }
    assert_ne!(
        held[0].terminal_for_test().source_manifest_bytes().unwrap(),
        held[1].terminal_for_test().source_manifest_bytes().unwrap()
    );
    assert_eq!(
        held[0].terminal_for_test().source_manifest_bytes().unwrap(),
        held[2].terminal_for_test().source_manifest_bytes().unwrap()
    );
    assert_eq!(
        held[0]
            .terminal_for_test()
            .repo_mapping_manifest_bytes()
            .unwrap(),
        held[1]
            .terminal_for_test()
            .repo_mapping_manifest_bytes()
            .unwrap()
    );
}

#[test]
fn native_manifest_observes_overridden_source_constituents() {
    let workspace = Workspace::new();
    let definitions = DEFS.replace("GROUP_VARIANT", "group_a").replace(
        "ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list())",
        r#"ctx.runfiles(files = ctx.attr.source[DefaultInfo].files.to_list(), root_symlinks = {"_main/input": out})"#,
    );
    fs::write(workspace.root.path().join("defs.bzl"), definitions).unwrap();
    let key = owner(&workspace, Default::default());
    let first = prepare(&workspace, key.clone(), Default::default()).unwrap();
    let a = first.terminal_for_test();
    let bytes = a.source_manifest_bytes().unwrap();
    assert_eq!(a.sources().len(), 1);
    assert_eq!(
        a.sources().next().unwrap().label(),
        &CanonicalLabel::parse("@@//:input").unwrap()
    );
    assert!(
        !String::from_utf8(bytes.clone()).unwrap().contains(&format!(
            " {}\n",
            workspace.root.path().join("input").display()
        ))
    );
    check_frontier(&workspace, a);
    fs::write(workspace.root.path().join("input"), "changed hidden source").unwrap();
    let changed = prepare(&workspace, key, Default::default()).unwrap();
    let b = changed.terminal_for_test();
    assert_ne!(
        a.sources().next().unwrap().digest(),
        b.sources().next().unwrap().digest()
    );
    assert_eq!(bytes, b.source_manifest_bytes().unwrap());
    check_frontier(&workspace, b);
    check_no_effects(&workspace, b);
}
