use std::fs;

use slug_workspace_v2::PathLstat;

use super::*;
use crate::runtime::ProcessHostOwner;
use crate::runtime::source_input::SourceArtifactInputError;

#[path = "../source_staging/test_workspace.rs"]
mod fixture;

struct Workspace {
    root: tempfile::TempDir,
    runtime: WorkspaceRuntime,
    configuration: ConfigurationKey,
}

impl Workspace {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fixture::write(root.path());
        fs::write(root.path().join("BUILD.bazel"), "platform(name='platform')\nexports_files(['input'])\nalias(name='selected', actual=':input')\n").unwrap();
        let runtime = WorkspaceRuntime::new(root.path(), ProcessHostOwner::native()).unwrap();
        let warm = runtime
            .build_command_with_repository_environment(
                &[TargetPattern::parse("//:selected").unwrap()],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", root.path().display())],
                Default::default(),
                Default::default(),
            )
            .unwrap();
        let configuration = warm.terminal_for_test().as_ref().as_ref().unwrap().targets[0]
            .analysis
            .as_ref()
            .unwrap()
            .configured_target_key()
            .unwrap()
            .configuration()
            .clone();
        Self {
            root,
            runtime,
            configuration,
        }
    }

    fn key(&self, target: &str) -> BuildCommandRootKey {
        BuildCommandRootKey::new(
            NormalizedAbsolutePath::new(self.root.path()).unwrap(),
            &[TargetPattern::parse(target).unwrap()],
            self.configuration.clone(),
        )
        .unwrap()
    }
}

#[test]
fn source_bytes_certificate_survives_authoritative_metadata_failure() {
    let workspace = Workspace::new();
    let source_path = NormalizedAbsolutePath::new(workspace.root.path().join("input")).unwrap();
    let bytes_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        source_path.dupe(),
        PathObservationOperation::FileBytes,
    );
    let bytes = Arc::new(PathObservationResult::FileBytes(
        PathOperationResult::Present(Arc::from(&b"aaa"[..])),
    ));
    let metadata_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        source_path,
        PathObservationOperation::Lstat,
    );
    let prior = workspace
        .runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .path_observations
        .clone();
    // An intentionally inconsistent virtual epoch discriminates the byte gate
    // from subsequent authoritative source metadata analysis. No native result
    // is accepted from this injected test state.
    let epoch = PathObservationEpoch::from_shared(
        prior
            .observations()
            .iter()
            .filter(|(demand, _)| *demand != &bytes_demand && *demand != &metadata_demand)
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain([
                (bytes_demand.dupe(), bytes.dupe()),
                (
                    metadata_demand.dupe(),
                    Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                        PathLstat::new(PathNodeKind::Directory, 1, 1, 1, 1, 0o755),
                    ))),
                ),
            ]),
    )
    .unwrap();
    let key = SingletonRootSingleBuildCommandKey::new(workspace.key("//:input")).unwrap();
    let terminal = workspace.runtime.runtime.block_on(async {
        let data = workspace.runtime.user_computation_data(None).unwrap();
        let mut updater = workspace.runtime.dice.updater_with_data(data);
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        let PreparationOutcome::Complete(Ok(terminal)) = outcome else {
            panic!("expected completed metadata failure: {outcome:?}");
        };
        terminal
    });
    let mut error = terminal.result.as_ref().as_ref().unwrap_err().clone();
    assert!(matches!(
        error.kind,
        BuildCommandErrorKind::SourceCertified { .. }
    ));
    assert!(error.is_analysis_error());
    assert!(key.allows_unavailable_terminal_roots(&terminal));
    assert!(matches!(
        key.terminal_observed_selection_association(&terminal),
        ObservedSelectionAssociation::SelectedDependencySuperset
    ));
    assert!(matches!(
        key.terminal_demand_association(&terminal),
        TerminalDemandAssociation::ClosureOnly
    ));
    let certificate = error.source_certificate().unwrap();
    assert!(Arc::ptr_eq(
        certificate.observations().get(&bytes_demand).unwrap(),
        &bytes
    ));
    assert!(Arc::ptr_eq(
        terminal
            .observations
            .as_ref()
            .unwrap()
            .get(&bytes_demand)
            .unwrap(),
        &bytes
    ));
    let taken = error.take_source_certificate().unwrap();
    assert!(Arc::ptr_eq(
        taken.observations().get(&bytes_demand).unwrap(),
        &bytes
    ));
    assert!(error.source_certificate().is_none());
    assert!(error.is_analysis_error());

    // Package loading can select repository dependencies even when a source
    // fails before metadata. Only certified errors get this association policy.
    let observation = PathObservationResult::FileBytes(PathOperationResult::Missing);
    let mut source_failure = SingletonRootSingleBuildCommandTerminal {
        result: Arc::new(Err(BuildCommandError::new(
            BuildCommandErrorKind::RootSource {
                source_certificate: Some(Box::new(SourceCertificate::new(
                    bytes_demand,
                    Arc::new(observation.clone()),
                ))),
                observation,
            },
        ))),
        observations: None,
    };
    assert!(matches!(
        key.terminal_observed_selection_association(&source_failure),
        ObservedSelectionAssociation::SelectedDependencySuperset
    ));
    assert!(!key.allows_unavailable_terminal_roots(&source_failure));
    let mut uncertified = source_failure.result.as_ref().as_ref().unwrap_err().clone();
    assert!(uncertified.take_source_certificate().is_some());
    source_failure.result = Arc::new(Err(uncertified));
    assert!(matches!(
        key.terminal_observed_selection_association(&source_failure),
        ObservedSelectionAssociation::StrictPathOnly
    ));
}

#[test]
fn legacy_source_alias_retains_actual_content_and_zero_actions() {
    let workspace = Workspace::new();
    // The legacy Analysis family deliberately consumes the snapshot adapters.
    // Seed those through the existing test observation owner before driving it.
    let observations = observe_workspace(workspace.root.path()).unwrap();
    workspace.runtime.runtime.block_on(async {
        let mut updater = workspace.runtime.dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: workspace.runtime.workspace.clone(),
                },
                Arc::new(WorkspaceSnapshot {
                    files: Arc::new(
                        observations
                            .files
                            .into_iter()
                            .map(|file| (file.path, file.value))
                            .collect(),
                    ),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: workspace.runtime.workspace.clone(),
                },
                Arc::new(WorkspaceRawSnapshot {
                    files: Arc::new(
                        observations
                            .raw_files
                            .into_iter()
                            .map(|file| (file.path, file.value))
                            .collect(),
                    ),
                }),
            )])
            .unwrap();
        updater.commit().await;
    });
    let driven = workspace
        .runtime
        .drive_command(
            NativeDemandRequestInputBundle::normalized_initial(),
            workspace.key("//:selected"),
        )
        .unwrap();
    let evaluation = driven
        .accepted
        .terminal_for_test()
        .as_ref()
        .as_ref()
        .unwrap();
    let alias = evaluation.targets[0].analysis.as_ref().unwrap();
    assert_eq!(alias.kind(), &ConfiguredNodeKind::Alias);
    assert_eq!(
        alias.actual_target().label(),
        &CanonicalLabel::parse("@@//:input").unwrap()
    );
    assert!(evaluation.source_certificate().is_some());
    assert_eq!(evaluation.declared_action_count(), 0);
    assert_eq!(evaluation.analyses().count(), 1);
    assert_eq!(
        evaluation.requested_artifacts().unwrap().artifacts(),
        &[AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:input").unwrap()
        )]
    );
}

#[test]
fn source_failure_before_first_observation_does_not_require_certificate() {
    let failure = complete_alias_source_observation(
        &Err(SourceArtifactInputError::Missing),
        &PathObservationEpoch::empty(),
        true,
    );
    let Err(BuildBranchResult::Outcome(PreparationOutcome::Complete(Err(error)))) = failure else {
        panic!("expected original source error");
    };
    assert!(matches!(
        error.kind,
        BuildCommandErrorKind::SourceArtifactInput(SourceArtifactInputError::Missing)
    ));
    assert!(error.source_certificate().is_none());
}
