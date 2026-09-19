use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;

use super::*;

fn key(name: &str, exec: bool) -> ConfiguredTargetKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
        ConfigurationKey::from_slug(if exec {
            SlugConfiguration::default_exec(&host).unwrap()
        } else {
            SlugConfiguration::default_target(&host).unwrap()
        }),
    )
}
fn artifact(owner: &ConfiguredTargetKey, path: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: owner.artifact_owner(),
        output: ActionOutput::new(path, kind),
    }
}
fn spawn(inputs: Vec<AnalysisArtifact>, outputs: Vec<ActionOutput>) -> ActionSpec {
    ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:tool").unwrap(),
        ))),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(
            inputs
                .into_iter()
                .map(ArtifactInputSource::Direct)
                .collect::<Vec<_>>(),
        ),
        ArtifactInputs::new(Vec::new()),
        outputs,
        None,
        Default::default(),
        Default::default(),
        "Spawn",
        None::<String>,
    ))
}
fn node(owner: ConfiguredTargetKey, actions: Vec<ActionSpec>) -> Arc<ConfiguredNodeResult> {
    let context =
        Arc::new(ConfiguredActionOwnerContext::unresolved_default(owner.clone()).unwrap());
    Arc::new(
        ConfiguredNodeResult::new_rule(
            owner,
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        )
        .with_action_specs(actions, vec![context])
        .unwrap(),
    )
}
fn write(path: &str) -> ActionSpec {
    ActionSpec::new(
        ActionKind::Write {
            content: "same".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new(path, ActionOutputKind::File)],
    )
}
fn output(path: &str) -> ActionOutput {
    ActionOutput::new(path, ActionOutputKind::File)
}

#[test]
fn diamond_orders_file_and_tree_producers_once_and_keeps_artifact_bindings() {
    let owner = key("owner", false);
    let file = artifact(&owner, "file", ActionOutputKind::File);
    let tree = artifact(&owner, "tree", ActionOutputKind::Directory);
    let closure = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            spawn(
                vec![],
                vec![
                    output("file"),
                    ActionOutput::new("tree", ActionOutputKind::Directory),
                ],
            ),
            spawn(vec![file.clone(), tree.clone()], vec![output("left")]),
            spawn(vec![tree.clone()], vec![output("right")]),
            spawn(
                vec![
                    artifact(&owner, "left", ActionOutputKind::File),
                    artifact(&owner, "right", ActionOutputKind::File),
                ],
                vec![output("done")],
            ),
        ],
    )]))
    .unwrap();
    let plan = ActionPrerequisitePlan::new(&closure, &owner, 3).unwrap();
    assert_eq!(
        plan.actions()
            .iter()
            .map(|step| step.action().outputs()[0].path())
            .collect::<Vec<_>>(),
        ["file", "left", "right", "done"]
    );
    assert_eq!(
        plan.actions()[1]
            .inputs()
            .iter()
            .filter_map(|input| input.producer())
            .collect::<Vec<_>>(),
        [0, 0]
    );
    assert_eq!(plan.actions()[1].inputs()[0].artifact(), &file);
    assert_eq!(plan.actions()[1].inputs()[1].artifact(), &tree);
    assert_eq!(
        plan.actions()[3]
            .inputs()
            .iter()
            .filter_map(|input| input.producer())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(
        plan.actions()[0]
            .inputs()
            .iter()
            .all(|input| input.producer().is_none())
    );
}

#[test]
fn generated_executable_and_tools_resolve_and_deduplicate_their_producers() {
    let owner = key("owner", false);
    let executable = artifact(&owner, "executable", ActionOutputKind::File);
    let direct_tool = artifact(&owner, "direct_tool", ActionOutputKind::File);
    let provider_tool = artifact(&owner, "provider_tool", ActionOutputKind::File);
    for executable in [
        SpawnExecutable::Artifact(executable.clone()),
        SpawnExecutable::FilesToRun(FilesToRunProvider::single_executable_without_support(
            executable.clone(),
        )),
    ] {
        let consumer = ActionSpec::spawn(SpawnSpec::new(
            RetainedSpawnInvocation::Executable(executable),
            RetainedCommandLine::new(Vec::new()),
            ArtifactInputs::new(Vec::new()),
            ArtifactInputs::new(vec![
                ArtifactInputSource::Direct(direct_tool.clone()),
                ArtifactInputSource::FilesToRun(
                    FilesToRunProvider::single_executable_without_support(provider_tool.clone()),
                ),
                ArtifactInputSource::FilesToRun(
                    FilesToRunProvider::single_executable_without_support(direct_tool.clone()),
                ),
            ]),
            vec![output("done")],
            None,
            Default::default(),
            Default::default(),
            "Spawn",
            None::<String>,
        ));
        let closure = ValidatedActionClosure::new(Arc::from([node(
            owner.clone(),
            vec![
                spawn(vec![], vec![output("direct_tool"), output("provider_tool")]),
                write("executable"),
                consumer,
            ],
        )]))
        .unwrap();
        let plan = ActionPrerequisitePlan::new(&closure, &owner, 2).unwrap();
        assert_eq!(plan.actions().len(), 3);
        assert_eq!(
            plan.actions()[0].action().outputs()[0].path(),
            "direct_tool"
        );
        assert_eq!(plan.actions()[1].action().outputs()[0].path(), "executable");
        let inputs = plan.actions()[2].inputs();
        assert_eq!(inputs.len(), 3);
        for (expected, producer) in [
            (direct_tool.clone(), 0),
            (provider_tool.clone(), 0),
            (artifact(&owner, "executable", ActionOutputKind::File), 1),
        ] {
            let input = inputs
                .iter()
                .find(|input| input.artifact() == &expected)
                .unwrap();
            assert_eq!(input.producer(), Some(producer));
        }
    }
}

#[test]
fn producer_lookup_preserves_configuration_preference_and_declared_owner() {
    let target = key("producer", false);
    let exec = key("producer", true);
    let consumer = key("consumer", false);
    let closure = ValidatedActionClosure::new(Arc::from([
        node(target.clone(), vec![write("shared")]),
        node(exec.clone(), vec![write("shared")]),
        node(
            consumer.clone(),
            vec![spawn(
                vec![
                    artifact(&exec, "shared", ActionOutputKind::File),
                    artifact(&target, "shared", ActionOutputKind::File),
                ],
                vec![output("done")],
            )],
        ),
    ]))
    .unwrap();
    let plan = ActionPrerequisitePlan::new(&closure, &consumer, 0).unwrap();
    assert_eq!(plan.actions().len(), 3);
    assert_eq!(plan.actions()[0].action().context().owner(), &exec);
    assert_eq!(plan.actions()[1].action().context().owner(), &target);
    let preferred = target.clone().with_toolchain_execution_platform(Arc::new(
        CanonicalLabel::parse("@@//:platform").unwrap(),
    ));
    assert_ne!(target.artifact_owner(), preferred.artifact_owner());
    // Equal path/configuration and even a shared physical FileWrite cannot
    // fabricate an absent preference-specific declared owner.
    let absent = ValidatedActionClosure::new(Arc::from([
        node(target.clone(), vec![write("shared")]),
        node(
            consumer.clone(),
            vec![spawn(
                vec![artifact(&preferred, "shared", ActionOutputKind::File)],
                vec![output("done")],
            )],
        ),
    ]))
    .unwrap();
    assert!(
        ActionPrerequisitePlan::new(&absent, &consumer, 0)
            .unwrap_err()
            .contains("missing generated producer")
    );
    let shared = ValidatedActionClosure::new(Arc::from([
        node(target.clone(), vec![write("shared")]),
        node(preferred.clone(), vec![write("shared")]),
        node(
            consumer.clone(),
            vec![spawn(
                vec![artifact(&preferred, "shared", ActionOutputKind::File)],
                vec![output("done")],
            )],
        ),
    ]))
    .unwrap();
    let plan = ActionPrerequisitePlan::new(&shared, &consumer, 0).unwrap();
    assert_eq!(plan.actions().len(), 2);
    assert_eq!(plan.actions()[0].action().context().owner(), &target);
    assert_eq!(
        plan.actions()[1].inputs()[0].artifact(),
        &artifact(&preferred, "shared", ActionOutputKind::File)
    );
    let ambiguous = ValidatedActionClosure::new(Arc::from([
        node(target.clone(), vec![write("a")]),
        node(target.clone(), vec![write("b")]),
    ]))
    .unwrap();
    assert!(
        ActionPrerequisitePlan::new(&ambiguous, &target, 0)
            .unwrap_err()
            .contains("ambiguous retained artifact owner")
    );
}

#[test]
fn missing_wrong_kind_cycles_and_unsupported_actions_reject_the_entire_plan() {
    let owner = key("owner", false);
    for (input, expected) in [
        (
            artifact(&key("absent", false), "first", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&owner, "absent", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&owner, "first", ActionOutputKind::Directory),
            "missing generated producer",
        ),
        (
            artifact(&owner, "first/child", ActionOutputKind::File),
            "missing generated producer",
        ),
        (
            artifact(&owner, "first", ActionOutputKind::Symlink),
            "unsupported generated prerequisite artifact kind",
        ),
        (artifact(&owner, "done", ActionOutputKind::File), "cycle"),
    ] {
        let closure = ValidatedActionClosure::new(Arc::from([node(
            owner.clone(),
            vec![write("first"), spawn(vec![input], vec![output("done")])],
        )]))
        .unwrap();
        assert!(
            ActionPrerequisitePlan::new(&closure, &owner, 1)
                .unwrap_err()
                .contains(expected)
        );
    }
    let cycle = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            spawn(
                vec![artifact(&owner, "b", ActionOutputKind::File)],
                vec![output("a")],
            ),
            spawn(
                vec![artifact(&owner, "a", ActionOutputKind::File)],
                vec![output("b")],
            ),
        ],
    )]))
    .unwrap();
    assert!(
        ActionPrerequisitePlan::new(&cycle, &owner, 0)
            .unwrap_err()
            .contains("cycle")
    );
    let unsupported = ValidatedActionClosure::new(Arc::from([node(
        owner.clone(),
        vec![
            ActionSpec::new(ActionKind::Run, "legacy", vec![output("first")]),
            spawn(
                vec![artifact(&owner, "first", ActionOutputKind::File)],
                vec![output("done")],
            ),
        ],
    )]))
    .unwrap();
    assert!(
        ActionPrerequisitePlan::new(&unsupported, &owner, 1)
            .unwrap_err()
            .contains("unsupported action family")
    );
    assert!(
        ActionPrerequisitePlan::new(&unsupported, &owner, 20)
            .unwrap_err()
            .contains("ordinal")
    );
}

#[path = "../source_staging/test_workspace.rs"]
mod fixture;

#[test]
fn native_generated_prerequisites_restore_and_source_staging_stays_closed() {
    use crate::runtime::BzlmodCommandPolicyKey;
    use crate::runtime::BzlmodEnvironmentPolicyKey;
    use crate::runtime::LockfileMode;
    use crate::runtime::ProcessHostOwner;
    use crate::runtime::TargetPattern;
    use crate::runtime::WorkspaceRuntime;
    let parent =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp736/fixtures");
    std::fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    fixture::write(root.path());
    let defs = r#"def _impl(ctx):
    seed = ctx.actions.declare_file('seed')
    tree = ctx.actions.declare_directory('tree')
    file = ctx.actions.declare_file('file')
    done = ctx.actions.declare_file('done')
    ctx.actions.write(seed, 'A')
    ctx.actions.run(executable=ctx.attr.tool, inputs=[seed, ctx.attr.input], outputs=[tree, file])
    ctx.actions.run(executable=ctx.attr.tool, inputs=[tree, file], outputs=[done])
    return [DefaultInfo(files=depset([done]))]
stage = rule(implementation=_impl, attrs={'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
"#;
    let runtime = WorkspaceRuntime::new(root.path(), ProcessHostOwner::native()).unwrap();
    let registry = [format!("file://{}/empty-registry", root.path().display())];
    type Snapshot = Vec<(ConfiguredAction, Vec<(AnalysisArtifact, Option<usize>)>)>;
    let mut original: Option<Snapshot> = None;
    let mut selected = None;
    for text in [
        defs.to_owned(),
        defs.replace("'A'", "'B'")
            .replace("inputs=[tree, file]", "inputs=depset([file])"),
        defs.replace("inputs=[tree, file]", "inputs=depset([tree, file])"),
        defs.to_owned(),
    ] {
        std::fs::write(root.path().join("defs.bzl"), &text).unwrap();
        let accepted = runtime
            .build_command_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &registry,
                Default::default(),
                Default::default(),
            )
            .unwrap();
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        let owner = evaluation
            .analyses()
            .find_map(|node| {
                node.configured_target_key()
                    .filter(|key| key.label().target().as_str() == "one")
            })
            .unwrap();
        let plan = evaluation.action_prerequisites(owner, 2).unwrap();
        assert_eq!(plan.actions().len(), 3);
        assert_eq!(plan.actions()[1].inputs()[0].producer(), Some(0));
        assert_eq!(plan.actions()[2].inputs()[0].producer(), Some(1));
        let snapshot = plan
            .actions()
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
            .collect::<Vec<_>>();
        if let Some(original) = &original {
            if text == defs {
                assert_eq!(&snapshot, original);
            } else if text.contains("'B'") {
                assert_ne!(&snapshot, original);
                assert_eq!(plan.actions()[2].inputs().len(), 2);
            } else {
                // A top-level depset retains a different input representation,
                // but resolves the same declared artifact edges as the list.
                assert_eq!(
                    snapshot
                        .iter()
                        .map(|(_, inputs)| inputs)
                        .collect::<Vec<_>>(),
                    original
                        .iter()
                        .map(|(_, inputs)| inputs)
                        .collect::<Vec<_>>()
                );
            }
        } else {
            original = Some(snapshot);
        }
        selected = Some(owner.clone());
    }
    let selected = selected.unwrap();
    let error = runtime
        .prepare_source_action_inputs_with_repository_environment(
            &[TargetPattern::parse("//:one").unwrap()],
            selected.clone(),
            2,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &registry,
            Default::default(),
            Default::default(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("generated file/tree"), "{error}");
    for (replacement, expected) in [
        (
            "executable=tree, inputs=[tree, file]",
            "ctx.actions.run executable must contain only regular Files",
        ),
        (
            "executable=ctx.attr.tool, tools=[tree], inputs=[tree, file]",
            "ctx.actions.run tools must contain only regular Files",
        ),
    ] {
        std::fs::write(
            root.path().join("defs.bzl"),
            defs.replace("executable=ctx.attr.tool, inputs=[tree, file]", replacement),
        )
        .unwrap();
        let error = runtime
            .prepare_source_action_inputs_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                selected.clone(),
                2,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &registry,
                Default::default(),
                Default::default(),
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    assert!(!root.path().join("done").exists());
}
