use prost::Message;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;

use super::*;
use crate::ReapiDigest;
use crate::proto;

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod fixture;

struct Workspace {
    runtime: WorkspaceRuntime,
    root: std::path::PathBuf,
    owner: ConfiguredTargetKey,
    build: String,
    defs: String,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn overlay() -> CommandConfigurationOverlay {
    CommandConfigurationOverlay::from(vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::ActionEnv,
        Some("CLIENT_INHERITED"),
        false,
    )])
}

impl Workspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "slug-source-spawn-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fixture::write(&root);
        let build = std::fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace(
                "stage(name='one', input='input', tool='tool')",
                "stage(name='one', input='input', tool='tool', exec_properties={'overlap': 'target'})",
            );
        let defs = fixture::DEFS
            .replace(
                "    args = ctx.actions.args()",
                "    first = ctx.actions.declare_file('a.out')\n    args = ctx.actions.args()",
            )
            .replace("outputs = [out]", "outputs = [out, first]")
            .replace(
                "arguments = [args]",
                "arguments = ['before', args, 'after']",
            )
            .replace(
                "ctx.actions.run(",
                "ctx.actions.run(env = {'Z': 'last', 'A': 'first'}, ",
            );
        std::fs::write(root.join("BUILD.bazel"), &build).unwrap();
        std::fs::write(root.join("defs.bzl"), &defs).unwrap();
        let runtime = WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap();
        let mut owner = None;
        drop(
            runtime
                .build_command_with_repository_environment(
                    &[TargetPattern::parse("//:one").unwrap()],
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[format!("file://{}/empty-registry", root.display())],
                    Default::default(),
                    overlay(),
                )
                .unwrap()
                .project(|value| {
                    owner = value
                        .as_ref()
                        .as_ref()
                        .unwrap()
                        .analyses()
                        .find_map(|value| {
                            value
                                .configured_target_key()
                                .filter(|key| key.label().target().as_str() == "one")
                                .cloned()
                        });
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        Self {
            runtime,
            root,
            owner: owner.unwrap(),
            build,
            defs,
        }
    }

    fn prepare(&self) -> Arc<PreparedSourceActionInputs> {
        let mut prepared = None;
        drop(
            self.runtime
                .prepare_source_action_inputs_with_repository_environment(
                    &[TargetPattern::parse("//:one").unwrap()],
                    self.owner.clone(),
                    0,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[format!("file://{}/empty-registry", self.root.display())],
                    Default::default(),
                    overlay(),
                )
                .unwrap()
                .project(|value| {
                    prepared = Some(value.clone());
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        prepared.unwrap()
    }

    fn plan(&self) -> Result<SourceSpawnReapiPlan, String> {
        SourceSpawnReapiPlan::from_prepared(self.prepare(), &defaults())
    }

    fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        std::fs::write(self.root.join(path), contents).unwrap();
    }
}

fn defaults() -> BTreeMap<String, String> {
    [("default-only", "remote"), ("overlap", "remote")]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into_iter()
        .collect()
}

#[test]
fn native_spawn_plan_preserves_fields_and_content_identity() {
    let workspace = Workspace::new();
    let prepared = workspace.prepare();
    assert_eq!(
        prepared.configured_action().context().owner(),
        &workspace.owner
    );
    let plan = SourceSpawnReapiPlan::from_prepared(prepared.clone(), &defaults()).unwrap();
    let command = proto::Command::decode(plan.command().serialized().as_slice()).unwrap();
    assert_eq!(
        command.arguments,
        ["tool", "before", "@shared.out-0.params", "after"]
    );
    assert_eq!(command.output_files, ["a.out", "shared.out"]);
    assert_eq!(command.output_paths, command.output_files);
    assert!(command.output_directories.is_empty());
    assert!(command.working_directory.is_empty());
    assert_eq!(
        command
            .environment_variables
            .iter()
            .map(|row| (row.name.as_str(), row.value.as_str()))
            .collect::<Vec<_>>(),
        [("A", "first"), ("Z", "last")]
    );
    assert_eq!(
        plan.command().platform_properties,
        [
            ("default-only".to_owned(), "remote".to_owned()),
            ("overlap".to_owned(), "target".to_owned())
        ]
        .into()
    );
    let action = proto::Action::decode(plan.identity().action_bytes()).unwrap();
    assert_eq!(action.platform, command.platform);
    assert!(action.timeout.is_none());
    assert!(!action.do_not_cache);
    assert!(action.salt.is_empty());
    assert_eq!(
        plan.identity().command_digest,
        ReapiDigest::of_bytes(&command.encode_to_vec())
    );
    assert_eq!(
        plan.identity().action_digest,
        ReapiDigest::of_bytes(&action.encode_to_vec())
    );
    assert_eq!(
        action.command_digest,
        Some(crate::command::digest_to_proto(
            &plan.identity().command_digest
        ))
    );
    assert_eq!(
        action.input_root_digest,
        Some(crate::command::digest_to_proto(
            plan.inputs().input_tree().root_digest()
        ))
    );
    let param = plan.inputs().input_tree().inline_blobs().first().unwrap();
    assert_eq!(param.data(), b"parameter\n");
    assert!(
        plan.inputs()
            .input_tree()
            .entries()
            .iter()
            .any(|entry| entry.path() == "shared.out-0.params" && entry.digest() == param.digest())
    );

    let original = plan.identity().clone();
    for (path, changed, restored) in [
        ("input", "changed source".to_owned(), "aaa".to_owned()),
        (
            "defs.bzl",
            workspace.defs.replace("'parameter'", "'changed argument'"),
            workspace.defs.clone(),
        ),
        (
            "defs.bzl",
            workspace.defs.replace("'before'", "'changed literal'"),
            workspace.defs.clone(),
        ),
        (
            "defs.bzl",
            workspace.defs.replace("'first'", "'changed environment'"),
            workspace.defs.clone(),
        ),
        (
            "defs.bzl",
            workspace.defs.replace("'a.out'", "'b.out'"),
            workspace.defs.clone(),
        ),
        (
            "BUILD.bazel",
            workspace.build.replace("'target'", "'changed property'"),
            workspace.build.clone(),
        ),
    ] {
        workspace.write(path, changed);
        let changed = workspace.plan().unwrap();
        assert_ne!(
            changed.identity().action_digest,
            original.action_digest,
            "{path}"
        );
        if path == "input" {
            assert_eq!(changed.identity().command_digest, original.command_digest);
            assert_ne!(
                changed.identity().input_root_digest,
                original.input_root_digest
            );
        }
        workspace.write(path, restored);
        assert_eq!(workspace.plan().unwrap().identity(), &original, "{path}");
    }
    workspace.write(
        "defs.bzl",
        workspace.defs.replace(
            "ctx.actions.run(",
            "ctx.actions.run(mnemonic='ChangedDiagnostic', progress_message='diagnostic only', ",
        ),
    );
    assert_eq!(workspace.plan().unwrap().identity(), &original);

    let mut changed_defaults = defaults();
    changed_defaults.insert("default-only".to_owned(), "changed".to_owned());
    assert_ne!(
        SourceSpawnReapiPlan::from_prepared(prepared, &changed_defaults)
            .unwrap()
            .identity()
            .action_digest,
        original.action_digest
    );
    workspace.write("BUILD.bazel", workspace.build.replace("platform(name='platform')", "platform(name='platform', exec_properties={'platform-only': 'selected', 'overlap': 'platform'})"));
    let selected = workspace.prepare();
    let selected_plan = SourceSpawnReapiPlan::from_prepared(selected.clone(), &defaults()).unwrap();
    assert_eq!(
        selected_plan.command().platform_properties,
        [
            ("platform-only".to_owned(), "selected".to_owned()),
            ("overlap".to_owned(), "target".to_owned())
        ]
        .into()
    );
    assert_eq!(
        selected_plan.identity(),
        SourceSpawnReapiPlan::from_prepared(selected, &changed_defaults)
            .unwrap()
            .identity()
    );

    // Another group's properties are filtered out of the effective action map.
    // With no combined properties, PlatformUtils uses the raw platform map.
    let no_target_properties = workspace
        .build
        .replace(", exec_properties={'overlap': 'target'}", "");
    workspace.write(
        "BUILD.bazel",
        no_target_properties.replace(
            "platform(name='platform')",
            "platform(name='platform', exec_properties={'other.property': 'raw'})",
        ),
    );
    let filtered = workspace.prepare();
    let context = filtered.configured_action().context();
    assert_eq!(
        context.raw_platform_fact().unwrap().exec_properties.len(),
        1
    );
    assert!(context.platform_fact().unwrap().exec_properties.is_empty());
    let filtered_plan = SourceSpawnReapiPlan::from_prepared(filtered, &defaults()).unwrap();
    assert_eq!(
        filtered_plan.command().platform_properties,
        [("other.property".to_owned(), "raw".to_owned())].into()
    );
    workspace.write("BUILD.bazel", no_target_properties);
    assert_eq!(
        workspace.plan().unwrap().command().platform_properties,
        defaults()
    );
}

#[test]
fn native_spawn_plan_rejects_unmodeled_policy_and_output_collisions() {
    let workspace = Workspace::new();
    for (changed, expected) in [
        (
            workspace.defs.replace(
                "ctx.actions.run(",
                "ctx.actions.run(use_default_shell_env=True, ",
            ),
            "resolved inherited environment",
        ),
        (
            workspace.defs.replace(
                "ctx.actions.run(",
                "ctx.actions.run(execution_requirements={'no-cache': '1'}, ",
            ),
            "execution requirements",
        ),
        (
            workspace.defs.replace("'a.out'", "'input'"),
            "conflicts with input input",
        ),
        (
            workspace.defs.replace("'a.out'", "'input/child'"),
            "conflicts with input input",
        ),
        (
            workspace.defs.replace("'a.out'", "'shared.out-0.params'"),
            "virtual parameter file shared.out-0.params conflicts with output",
        ),
    ] {
        workspace.write("defs.bzl", changed);
        let error = workspace.plan().unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    // The reverse prefix case requires an input beneath the declared output.
    std::fs::create_dir_all(workspace.root.join("folder")).unwrap();
    workspace.write("folder/input", "source under output");
    workspace.write(
        "BUILD.bazel",
        workspace.build.replace("'input'", "'folder/input'"),
    );
    workspace.write("defs.bzl", workspace.defs.replace("'a.out'", "'folder'"));
    let error = workspace.plan().unwrap_err();
    assert!(
        error.contains("conflicts with input folder/input"),
        "{error}"
    );
    workspace.write("BUILD.bazel", &workspace.build);
    workspace.write("defs.bzl", &workspace.defs);
    workspace.plan().unwrap();
}

#[test]
fn output_projection_rejects_non_file_and_malformed_paths() {
    // The Starlark producer currently lacks declare_directory; exercise the
    // projection guard directly without admitting another declaration surface.
    for kind in [
        ActionOutputKind::Directory,
        ActionOutputKind::Symlink,
        ActionOutputKind::RunfilesTree,
    ] {
        assert!(
            regular_output_paths(&[ActionOutput::new("out", kind)])
                .unwrap_err()
                .contains("regular file outputs")
        );
    }
    assert!(
        regular_output_paths(&[])
            .unwrap_err()
            .contains("declared file outputs")
    );
    for path in ["", "/out", "../out", "dir/./out", "dir//out", "dir\\out"] {
        assert!(
            regular_output_paths(&[ActionOutput::new(path, ActionOutputKind::File)]).is_err(),
            "{path}"
        );
    }
}
