use std::fs;

use super::*;

#[test]
fn build_only_observations_survive_the_narrow_source_certificate() {
    let observation = |name: &str| {
        (
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/{name}")).unwrap(),
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                b"content".as_slice(),
            ))),
        )
    };
    let epoch =
        PathObservationEpoch::new([observation("BUILD.bazel"), observation("MODULE.bazel")])
            .unwrap();
    let (demand, value) = epoch.observations().iter().next().unwrap();
    let certificate = SourceCertificate::new(demand.dupe(), value.dupe());
    let retained = checked_build_frontier(Some(&epoch), Some(&certificate)).unwrap();
    assert_eq!(retained.observations().len(), 2);
    for (demand, result) in epoch.observations() {
        assert!(Arc::ptr_eq(result, retained.get(demand).unwrap()));
    }
    let detached = SourceCertificate::new(demand.dupe(), Arc::new(value.as_ref().clone()));
    assert!(checked_build_frontier(Some(&epoch), Some(&detached)).is_err());
    assert!(checked_build_frontier(None, Some(&certificate)).is_err());
}

#[path = "test_workspace.rs"]
mod fixture;

fn request(runtime: &WorkspaceRuntime) -> NativeDemandRequestInputBundle {
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.registry_urls = RegistryUrls::new([format!(
        "file://{}/empty-registry",
        runtime.workspace.display()
    )]);
    request
}

fn key(
    runtime: &WorkspaceRuntime,
    targets: &[&str],
    owner: ConfiguredTargetKey,
) -> SourceStagingKey {
    let bundle = request(runtime);
    let (build, _) = runtime
        .prepare_build_request(
            &targets
                .iter()
                .map(|target| TargetPattern::parse(target).unwrap())
                .collect::<Vec<_>>(),
            bundle.command_policy,
            bundle.environment_policy,
            bundle.lockfile_mode,
            &[format!(
                "file://{}/empty-registry",
                runtime.workspace.display()
            )],
            bundle.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    SourceStagingKey {
        build,
        owner,
        action: 0,
    }
}

#[test]
fn native_source_staging_tracks_sources_and_rejects_conflicting_roots() {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp731/fixtures");
    fs::create_dir_all(&parent).unwrap();
    let workspace = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    fixture::write(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let owner = build_owner(&runtime, "one");
    let selected = key(&runtime, &["//:one"], owner.clone());
    let first = runtime
        .drive_command(request(&runtime), selected.clone())
        .unwrap()
        .accepted;
    let first = first.terminal_for_test();
    assert_unsupported_shapes(first.spawn());
    assert!(
        first
            .evaluation
            .as_ref()
            .as_ref()
            .unwrap()
            .source_certificate()
            .is_none(),
        "the build-only observation is outside the predecessor source certificate"
    );
    assert!(
        first.observations().observations().keys().any(|demand| {
            demand.path().as_path() == workspace.path().join("MODULE.bazel")
                && demand.operation() == PathObservationOperation::FileBytes
        }),
        "the complete build-only frontier survives source staging"
    );

    assert_eq!(
        first.sources().len(),
        2,
        "input, tool and executable deduplicate structurally"
    );
    for bytes in [b"bbb", b"aaa"] {
        fs::write(workspace.path().join("input"), bytes).unwrap();
        let driven = runtime
            .drive_command(request(&runtime), selected.clone())
            .unwrap();
        let value = driven.accepted.terminal_for_test();
        assert_eq!(
            first
                .sources()
                .map(|source| source.digest())
                .collect::<Vec<_>>()
                == value
                    .sources()
                    .map(|source| source.digest())
                    .collect::<Vec<_>>(),
            bytes == b"aaa"
        );
        let accepted = runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .clone();
        for (demand, result) in value.observations().observations() {
            assert!(Arc::ptr_eq(
                result,
                accepted.path_observations.get(demand).unwrap()
            ));
        }
    }
    let other = build_owner(&runtime, "two");
    runtime
        .drive_command(request(&runtime), key(&runtime, &["//:two"], other))
        .unwrap();
    let mut absent = selected.clone();
    absent.action = usize::MAX;
    assert!(
        runtime
            .drive_command(request(&runtime), absent)
            .err()
            .unwrap()
            .to_string()
            .contains("ordinal")
    );
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let combined = key(&runtime, &["//:one", "//:two"], owner);
    let conflict = runtime
        .drive_command(request(&runtime), combined)
        .err()
        .expect("conflicting roots reject staging");
    assert!(
        conflict
            .to_string()
            .contains("unsupported equivalence at shared.out"),
        "{conflict}"
    );
    fs::remove_file(workspace.path().join("input")).unwrap();
    assert!(runtime.drive_command(request(&runtime), selected).is_err());
    let after = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(before.inputs, after.inputs);
    assert_eq!(before.path_observations, after.path_observations);
    assert_eq!(before.selected, after.selected);
    let index = first
        .sources()
        .position(|source| source.label().target().as_str() == "input")
        .unwrap();
    fs::create_dir(workspace.path().join("input")).unwrap();
    assert!(first.open_source(index).is_err());
    #[cfg(unix)]
    {
        fs::remove_dir(workspace.path().join("input")).unwrap();
        nix::unistd::mkfifo(
            &workspace.path().join("input"),
            nix::sys::stat::Mode::S_IRUSR,
        )
        .unwrap();
        assert!(
            first.open_source(index).is_err(),
            "replacement FIFO must not block open"
        );
    }
}

fn assert_unsupported_shapes(spawn: &SpawnSpec) {
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionOutputKind;
    use slug_build_api_v2::AnalysisConfiguredTargetKey;
    use slug_build_api_v2::RetainedRunfiles;
    use slug_build_api_v2::RunfilesSupport;
    let replacement = |invocation, inputs, unused| {
        SpawnSpec::new(
            invocation,
            spawn.command_line().clone(),
            inputs,
            spawn.tools().clone(),
            spawn.outputs().to_vec(),
            unused,
            spawn.environment().clone(),
            spawn.execution_requirements().clone(),
            spawn.mnemonic(),
            spawn.progress_message(),
        )
    };
    let artifact = declared_sources(spawn)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .clone();
    for kind in [ActionOutputKind::File, ActionOutputKind::Directory] {
        let derived = AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:generated").unwrap(),
                b"cfg".as_slice(),
            ),
            output: ActionOutput::new("generated", kind),
        };
        let invalid = replacement(
            spawn.invocation().clone(),
            ArtifactInputs::new(vec![ArtifactInputSource::Direct(derived)]),
            None,
        );
        assert!(
            declared_sources(&invalid)
                .unwrap_err()
                .contains("generated")
        );
    }
    let shell = replacement(
        RetainedSpawnInvocation::Shell {
            command: "shell command".into(),
            pad_dollar_zero: false,
        },
        spawn.inputs().clone(),
        None,
    );
    assert!(
        declared_sources(&shell)
            .unwrap_err()
            .contains("artifact-backed")
    );
    let pruning = replacement(
        spawn.invocation().clone(),
        spawn.inputs().clone(),
        Some(artifact.clone()),
    );
    assert!(declared_sources(&pruning).unwrap_err().contains("pruning"));
    let mut provider = FilesToRunProvider::empty();
    provider.executable = Some(artifact.clone());
    provider.support = Some(Arc::new(RunfilesSupport {
        runfiles: RetainedRunfiles::empty(),
        tree: artifact.clone(),
        input_manifest: artifact,
        manifest: None,
        repo_mapping_manifest: None,
    }));
    for (invocation, inputs) in [
        (
            RetainedSpawnInvocation::Executable(SpawnExecutable::FilesToRun(provider.clone())),
            spawn.inputs().clone(),
        ),
        (
            spawn.invocation().clone(),
            ArtifactInputs::new(vec![ArtifactInputSource::FilesToRun(provider)]),
        ),
    ] {
        assert!(
            declared_sources(&replacement(invocation, inputs, None))
                .unwrap_err()
                .contains("runfiles")
        );
    }
}

fn build_owner(runtime: &WorkspaceRuntime, name: &str) -> ConfiguredTargetKey {
    let bundle = request(runtime);
    let accepted = runtime
        .build_command_with_repository_environment(
            &[TargetPattern::parse(&format!("//:{name}")).unwrap()],
            bundle.command_policy,
            bundle.environment_policy,
            bundle.lockfile_mode,
            &[format!(
                "file://{}/empty-registry",
                runtime.workspace.display()
            )],
            bundle.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
    evaluation
        .analyses()
        .find(|owner| {
            owner
                .configured_target_key()
                .unwrap()
                .label()
                .target()
                .as_str()
                == name
        })
        .unwrap()
        .configured_target_key()
        .unwrap()
        .clone()
}
