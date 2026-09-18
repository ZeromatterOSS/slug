//! Closure-owned source staging. This never authorizes action execution.

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;

use super::*;
use crate::runtime::ObservedSourceArtifactInput;
use crate::runtime::SourceArtifactInput;
use crate::runtime::SourceArtifactInputObservationKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct SourceStagingKey {
    build: BuildCommandRootKey,
    owner: ConfiguredTargetKey,
    action: usize,
}

/// A complete declared source-only input set selected from a validated closure.
/// Its native request has been validated; later execution must validate again
/// after transfer. CAS staging alone cannot publish an action or build result.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PreparedSourceActionInputs {
    evaluation: Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
    owner: usize,
    action: usize,
    sources: Arc<[Arc<ObservedSourceArtifactInput>]>,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
}

impl PreparedSourceActionInputs {
    pub(super) fn check_execution_representative(&self) -> Result<(), NativeDemandSessionError> {
        require_execution_representative(
            self.evaluation.as_ref().as_ref().unwrap(),
            self.owner,
            self.action,
        )
    }

    /// Borrow the producer-selected action and its retained execution context.
    /// This conveys the staging selection, not authority to execute or publish.
    pub fn configured_action(&self) -> &slug_analysis_v2::ConfiguredAction {
        &self
            .evaluation
            .as_ref()
            .as_ref()
            .unwrap()
            .action_closure
            .owners()[self.owner]
            .actions()[self.action]
    }

    pub fn spawn(&self) -> &SpawnSpec {
        self.configured_action().spawn_spec().unwrap()
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceArtifactInput> {
        self.sources
            .iter()
            .map(|source| source.result().as_ref().unwrap())
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }

    /// Open only a source selected by this prepared set. The caller must verify
    /// transfer bytes against its digest, and must not infer current provenance
    /// from a successful open. The handle is never retained in DICE.
    pub fn open_source(&self, index: usize) -> std::io::Result<std::fs::File> {
        let source = self
            .sources
            .get(index)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "unknown prepared source")
            })?
            .result()
            .as_ref()
            .unwrap();
        let mut options = std::fs::File::options();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NONBLOCK);
        }
        let file = options.open(source.real_path().as_path())?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "prepared source is no longer a regular file",
            ));
        }
        Ok(file)
    }
}

pub(super) fn require_execution_representative(
    evaluation: &BuildCommandEvaluation,
    owner: usize,
    action: usize,
) -> Result<(), NativeDemandSessionError> {
    if !evaluation
        .action_closure
        .execution_representative(owner, action)
    {
        return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
            "selected source action is not an execution representative"
        )));
    }
    Ok(())
}

fn error(value: impl fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

fn declared_sources(spawn: &SpawnSpec) -> Result<SmallSet<AnalysisArtifact>, Arc<str>> {
    if spawn.unused_inputs_list().is_some() {
        return Err(error(
            "unused-input pruning is not admitted for source staging",
        ));
    }
    let mut sources = SmallSet::new();
    fn provider(
        provider: &FilesToRunProvider,
        sources: &mut SmallSet<AnalysisArtifact>,
    ) -> Result<(), Arc<str>> {
        if provider.support.is_some() {
            return Err(error("runfiles support is not admitted for source staging"));
        }
        RetainedArtifactInputs::new(provider.files().clone())
            .map_err(error)?
            .visit(|artifact| {
                sources.insert(artifact.clone());
            })
            .map_err(error)?;
        if let Some(executable) = &provider.executable {
            sources.insert(executable.clone());
        }
        Ok(())
    }
    fn inputs(
        inputs: &ArtifactInputs,
        sources: &mut SmallSet<AnalysisArtifact>,
    ) -> Result<(), Arc<str>> {
        for input in inputs.sources() {
            if let ArtifactInputSource::FilesToRun(value) = input {
                provider(value, sources)?;
            }
        }
        inputs
            .visit(|artifact| {
                sources.insert(artifact.clone());
            })
            .map_err(error)
    }
    inputs(spawn.inputs(), &mut sources)?;
    inputs(spawn.tools(), &mut sources)?;
    match spawn.invocation() {
        RetainedSpawnInvocation::Executable(SpawnExecutable::Artifact(artifact)) => {
            sources.insert(artifact.clone());
        }
        RetainedSpawnInvocation::Executable(SpawnExecutable::FilesToRun(value)) => {
            provider(value, &mut sources)?;
            if value.executable.is_none() {
                return Err(error("source staging requires a declared executable"));
            }
        }
        _ => {
            return Err(error(
                "source staging requires an artifact-backed executable",
            ));
        }
    }
    if sources
        .iter()
        .any(|artifact| !matches!(artifact, AnalysisArtifact::Source(_)))
    {
        return Err(error("generated file/tree source staging is not admitted"));
    }
    Ok(sources)
}

fn checked_build_frontier(
    observations: Option<&PathObservationEpoch>,
    certificate: Option<&SourceCertificate>,
) -> Result<PathObservationEpoch, Arc<str>> {
    let observations = observations.ok_or_else(|| error("build root has no observed epoch"))?;
    if let Some(certificate) = certificate {
        for (demand, result) in certificate.observations().observations() {
            if !observations
                .get(demand)
                .is_some_and(|value| Arc::ptr_eq(value, result))
            {
                return Err(error(
                    "build source certificate is not associated with its observed epoch",
                ));
            }
        }
    }
    Ok(observations.dupe())
}

impl SourceStagingKey {
    pub(super) fn new(
        build: BuildCommandRootKey,
        owner: ConfiguredTargetKey,
        action: usize,
    ) -> Self {
        Self {
            build,
            owner,
            action,
        }
    }

    async fn prepare(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<PreparationOutcome<Arc<PreparedSourceActionInputs>>, Arc<str>> {
        let root = BuildCommandRootObservationKey::for_source_staging(self.build.clone())
            .ok_or_else(|| error("source staging requires an observed build root"))?;
        let terminal = match ctx.compute(&root).await.map_err(error)? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result.map_err(error)?,
        };
        let mut observations =
            checked_build_frontier(Some(terminal.observations()), terminal.source_certificate())?;
        let evaluation = terminal.result;
        let build = evaluation.as_ref().as_ref().map_err(error)?;
        let owner_index = build
            .action_closure
            .owners()
            .iter()
            .position(|owner| owner.configured_target_key() == Some(&self.owner))
            .ok_or_else(|| {
                error("selected source action owner is absent from the validated closure")
            })?;
        let action = build.action_closure.owners()[owner_index]
            .actions()
            .get(self.action)
            .ok_or_else(|| error("selected action ordinal is absent from the validated closure"))?;
        let spawn = action
            .spawn_spec()
            .ok_or_else(|| error("source staging requires a typed Spawn"))?;
        // Validate the entire declared set before any content observation.
        let artifacts = declared_sources(spawn)?;
        let mut sources = Vec::with_capacity(artifacts.len());
        for artifact in &artifacts {
            let key = SourceArtifactInputObservationKey::new(self.build.workspace.dupe(), artifact)
                .map_err(|value| error(format!("{value:?}")))?;
            match ctx.compute(&key).await.map_err(error)? {
                PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                PreparationOutcome::Complete(result) => {
                    let source = result.map_err(error)?;
                    source
                        .result()
                        .as_ref()
                        .map_err(|value| error(format!("{value:?}")))?;
                    observations = union_build_observations(&observations, source.observations())
                        .map_err(error)?;
                    sources.push(source);
                }
            }
        }
        let certificate = SourceCertificate::from_epoch(observations.dupe()).map_err(error)?;
        Ok(PreparationOutcome::Complete(Arc::new(
            PreparedSourceActionInputs {
                evaluation,
                owner: owner_index,
                action: self.action,
                sources: sources.into(),
                observations,
                certificate,
            },
        )))
    }
}

impl fmt::Display for SourceStagingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source-staging:{}:{:?}:{}",
            self.build, self.owner, self.action
        )
    }
}

#[async_trait]
impl Key for SourceStagingKey {
    type Value = PreparationOutcome<Result<Arc<PreparedSourceActionInputs>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one source staging event batch");
        match self.prepare(ctx).await {
            Ok(outcome) => outcome.map(Ok),
            Err(error) => PreparationOutcome::Complete(Err(error)),
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        Self::validity(x) && Self::validity(y) && x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, PreparationOutcome::Complete(Ok(_)))
    }
}

#[async_trait]
impl NativeCommandRoot for SourceStagingKey {
    type Terminal = Arc<PreparedSourceActionInputs>;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(&terminal.observations)
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(&terminal.certificate)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        match tx
            .compute(self)
            .await
            .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}")))?
        {
            PreparationOutcome::Need(need) => Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result
                .map(PreparationOutcome::Complete)
                .map_err(|value| NativeDemandSessionError::Computation(anyhow::anyhow!("{value}"))),
        }
    }
}

impl WorkspaceRuntime {
    /// Opt-in source/CAS staging only. This does not execute a build.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_source_action_inputs_with_repository_environment(
        &self,
        targets: &[TargetPattern],
        owner: ConfiguredTargetKey,
        action: usize,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
    ) -> Result<AcceptedCommand<Arc<PreparedSourceActionInputs>>, BuildCommandError> {
        let (build, request) = self.prepare_build_request(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
        )?;
        self.drive_command(
            request,
            SourceStagingKey {
                build,
                owner,
                action,
            },
        )
        .map(|driven| driven.accepted)
        .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "source_staging/tests.rs"]
mod tests;
