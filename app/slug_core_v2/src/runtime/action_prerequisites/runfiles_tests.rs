use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_build_api_v2::RunfilesSymlinkMode;

use super::*;

fn fixture(owner: &ConfiguredTargetKey) -> (FilesToRunProvider, Vec<ActionSpec>, AnalysisArtifact) {
    let exe = artifact(owner, "exe", ActionOutputKind::File);
    let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:data").unwrap());
    let support = Arc::new(RunfilesSupport {
        runfiles: RetainedRunfiles::empty()
            .with_artifact(exe.clone())
            .unwrap()
            .with_artifact(source.clone())
            .unwrap()
            .with_artifact(artifact(owner, "directory", ActionOutputKind::Directory))
            .unwrap(),
        tree: artifact(owner, "exe.runfiles", ActionOutputKind::RunfilesTree),
        input_manifest: artifact(owner, "exe.runfiles_manifest", ActionOutputKind::File),
        manifest: Some(artifact(
            owner,
            "exe.runfiles/MANIFEST",
            ActionOutputKind::File,
        )),
        repo_mapping_manifest: Some(artifact(owner, "exe.repo_mapping", ActionOutputKind::File)),
    });
    let provider = DefaultInfo::from_executable(exe, None)
        .unwrap()
        .with_runfiles_support(support.clone())
        .unwrap()
        .files_to_run;
    let actions = vec![
        write("exe"),
        spawn(
            vec![],
            vec![ActionOutput::new("directory", ActionOutputKind::Directory)],
        ),
        ActionSpec::runfiles_support(RunfilesSupportActionSpec::RepoMappingManifest {
            support: support.clone(),
            packages: RunfilesPackageDepset::empty(),
            workspace_name: "_main".into(),
            emit_compact_repo_mapping: false,
            output: output("exe.repo_mapping"),
        }),
        ActionSpec::runfiles_support(RunfilesSupportActionSpec::SourceSymlinkManifest {
            support: support.clone(),
            remotable: false,
            output: output("exe.runfiles_manifest"),
        }),
        ActionSpec::runfiles_support(RunfilesSupportActionSpec::SymlinkTree {
            support: support.clone(),
            environment: Default::default(),
            mode: RunfilesSymlinkMode::Create,
            output: output("exe.runfiles/MANIFEST"),
        }),
        ActionSpec::runfiles_support(RunfilesSupportActionSpec::RunfilesTree {
            support,
            output: ActionOutput::new("exe.runfiles", ActionOutputKind::RunfilesTree),
        }),
    ];
    (provider, actions, source)
}

// Each position is independently meaningful: an executable is not necessarily
// repeated in inputs/tools, and tools may belong to a different configuration.
fn consumer(provider: FilesToRunProvider, position: usize, path: &str) -> ActionSpec {
    let mut inputs = Vec::new();
    let mut tools = Vec::new();
    let invocation = if position == 0 {
        SpawnExecutable::FilesToRun(provider)
    } else {
        if position == 1 {
            inputs.push(ArtifactInputSource::FilesToRun(provider));
        } else {
            tools.push(ArtifactInputSource::FilesToRun(provider));
        }
        SpawnExecutable::Artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:tool").unwrap(),
        ))
    };
    ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(invocation),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(inputs),
        ArtifactInputs::new(tools),
        vec![output(path)],
        None,
        Default::default(),
        Default::default(),
        "Consume",
        None::<String>,
    ))
}

#[test]
fn exact_files_to_run_occurrences_share_support_and_retain_source_bindings() {
    let tool = key("tool_owner", true);
    let target = key("consumer", false);
    let (provider, tool_actions, source) = fixture(&tool);
    let support = provider.support.as_ref().unwrap();
    let mut equivalent = provider.clone();
    equivalent.support = Some(Arc::new((**support).clone()));
    assert!(!Arc::ptr_eq(support, equivalent.support.as_ref().unwrap()));
    let closure = ValidatedActionClosure::new(Arc::from([
        node(tool, tool_actions),
        node(
            target.clone(),
            vec![
                consumer(provider.clone(), 0, "one"),
                consumer(equivalent, 1, "two"),
                consumer(provider.clone(), 2, "three"),
                spawn(vec![support.tree.clone()], vec![output("direct")]),
            ],
        ),
    ]))
    .unwrap();
    let roots = ["one", "two", "three", "direct"]
        .map(|path| artifact(&target, path, ActionOutputKind::File));
    let forest = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap();
    assert_eq!(forest.actions.len(), 10); // six shared producers, four consumers
    let trees = forest
        .actions
        .iter()
        .enumerate()
        .filter(|(_, step)| {
            matches!(
                step.action().runfiles_support_spec(),
                Some(RunfilesSupportActionSpec::RunfilesTree { .. })
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(trees.len(), 1);
    let (tree_index, tree) = trees[0];
    assert!(
        tree.inputs()
            .iter()
            .any(|input| input.artifact() == &source && input.producer().is_none())
    );
    for producer in &forest.producers {
        let consumer = &forest.actions[producer.unwrap()];
        assert!(
            consumer
                .inputs()
                .iter()
                .any(|input| input.artifact() == &support.tree
                    && input.producer() == Some(tree_index))
        );
    }
    // Planning retains typed provenance; it does not publish a tool's tree or
    // replace its declared Directory backing with a filesystem path.
    assert!(tree.inputs().iter().any(|input| matches!(input.artifact(), AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::Directory) && input.producer().is_some()));
}

#[test]
fn malformed_files_to_run_fails_the_whole_forest_in_every_occurrence() {
    let tool = key("tool_owner", true);
    let target = key("consumer", false);
    let (provider, actions, _) = fixture(&tool);
    let mut mismatched = provider.clone();
    let mut changed = (**provider.support.as_ref().unwrap()).clone();
    changed.runfiles = changed
        .runfiles
        .with_artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:different").unwrap(),
        ))
        .unwrap();
    mismatched.support = Some(Arc::new(changed));
    let mut missing_tree =
        FilesToRunProvider::single_executable_without_support(provider.executable.clone().unwrap());
    missing_tree.support = provider.support.clone();
    let mut wrong_executable = provider.clone();
    wrong_executable.executable = Some(artifact(
        &key("tool_owner", false),
        "exe",
        ActionOutputKind::File,
    ));
    for position in 0..3 {
        for (invalid, expected) in [
            (mismatched.clone(), "support differs"),
            (missing_tree.clone(), "files omit"),
            (wrong_executable.clone(), "files omit"),
        ] {
            let closure = ValidatedActionClosure::new(Arc::from([
                node(tool.clone(), actions.clone()),
                node(
                    target.clone(),
                    vec![write("valid"), consumer(invalid, position, "invalid")],
                ),
            ]))
            .unwrap();
            let roots = [
                artifact(&target, "valid", ActionOutputKind::File),
                artifact(&target, "invalid", ActionOutputKind::File),
            ];
            let error = plan_roots(&closure, PlanRoots::Artifacts(&roots)).unwrap_err();
            assert!(error.contains(expected), "position {position}: {error}");
        }
    }
    for missing_mapping in [false, true] {
        let mut malformed = (**provider.support.as_ref().unwrap()).clone();
        if missing_mapping {
            malformed.repo_mapping_manifest = None;
        } else {
            malformed.manifest = None;
        }
        let malformed = Arc::new(malformed);
        let mut invalid_provider = provider.clone();
        invalid_provider.support = Some(malformed.clone());
        let mut invalid_actions = actions.clone();
        *invalid_actions.last_mut().unwrap() =
            ActionSpec::runfiles_support(RunfilesSupportActionSpec::RunfilesTree {
                support: malformed.clone(),
                output: ActionOutput::new("exe.runfiles", ActionOutputKind::RunfilesTree),
            });
        let closure = ValidatedActionClosure::new(Arc::from([
            node(tool.clone(), invalid_actions),
            node(
                target.clone(),
                vec![write("valid"), consumer(invalid_provider, 0, "invalid")],
            ),
        ]))
        .unwrap();
        for invalid in [
            malformed.tree.clone(),
            artifact(&target, "invalid", ActionOutputKind::File),
        ] {
            let roots = [artifact(&target, "valid", ActionOutputKind::File), invalid];
            assert!(
                plan_roots(&closure, PlanRoots::Artifacts(&roots))
                    .unwrap_err()
                    .contains("requires public and repository mapping manifests")
            );
        }
    }
    let mut absent_tree = actions;
    absent_tree.pop();
    let closure = ValidatedActionClosure::new(Arc::from([
        node(tool, absent_tree),
        node(
            target.clone(),
            vec![write("valid"), consumer(provider, 0, "invalid")],
        ),
    ]))
    .unwrap();
    let roots = [
        artifact(&target, "valid", ActionOutputKind::File),
        artifact(&target, "invalid", ActionOutputKind::File),
    ];
    assert!(
        plan_roots(&closure, PlanRoots::Artifacts(&roots))
            .unwrap_err()
            .contains("missing generated producer")
    );
}
