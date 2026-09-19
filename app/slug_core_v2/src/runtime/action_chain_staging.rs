//! DICE-owned source preparation for a complete selected prerequisite chain.

use slug_build_api_v2::AnalysisArtifact;

use super::source_staging::checked_build_frontier;
use super::source_staging::open_observed_source;
use super::*;
use crate::runtime::ActionPrerequisitePlan;
use crate::runtime::ObservedSourceArtifactInput;
use crate::runtime::SourceArtifactInput;
use crate::runtime::SourceArtifactInputObservationKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct ActionChainStagingKey {
    build: BuildCommandRootKey,
    owner: ConfiguredTargetKey,
    action: usize,
}

/// Complete reachable source metadata, associated with the retained build frontier.
/// Transport effects and producer results never enter this DICE-owned value.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PreparedActionChainInputs {
    evaluation: Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
    owner: ConfiguredTargetKey,
    action: usize,
    sources: Arc<[Arc<ObservedSourceArtifactInput>]>,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
}

impl PreparedActionChainInputs {
    pub fn plan(&self) -> Result<ActionPrerequisitePlan<'_>, Arc<str>> {
        self.evaluation
            .as_ref()
            .as_ref()
            .unwrap()
            .action_prerequisites(&self.owner, self.action)
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceArtifactInput> {
        self.sources
            .iter()
            .map(|source| source.result().as_ref().unwrap())
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }

    /// Open an observed source; transport must verify its bytes against the digest.
    pub fn open_source(&self, index: usize) -> std::io::Result<std::fs::File> {
        open_observed_source(&self.sources, index)
    }
}

fn error(value: impl fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

impl ActionChainStagingKey {
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
    ) -> Result<PreparationOutcome<Arc<PreparedActionChainInputs>>, Arc<str>> {
        let root = BuildCommandRootObservationKey::for_source_staging(self.build.clone())
            .ok_or_else(|| error("action chain staging requires an observed build root"))?;
        let terminal = match ctx.compute(&root).await.map_err(error)? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(result) => result.map_err(error)?,
        };
        let mut observations =
            checked_build_frontier(Some(terminal.observations()), terminal.source_certificate())?;
        let evaluation = terminal.result;
        let build = evaluation.as_ref().as_ref().map_err(error)?;
        // Validate the entire reachable plan before observing source content.
        let plan = build.action_prerequisites(&self.owner, self.action)?;
        let mut artifacts = SmallSet::new();
        for step in plan.actions() {
            for input in step.inputs() {
                if matches!(input.artifact(), AnalysisArtifact::Source(_)) {
                    artifacts.insert(input.artifact().clone());
                }
            }
        }
        drop(plan);
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
            PreparedActionChainInputs {
                evaluation,
                owner: self.owner.clone(),
                action: self.action,
                sources: sources.into(),
                observations,
                certificate,
            },
        )))
    }
}

impl fmt::Display for ActionChainStagingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "action-chain-staging:{}:{:?}:{}",
            self.build, self.owner, self.action
        )
    }
}

#[async_trait]
impl Key for ActionChainStagingKey {
    type Value = PreparationOutcome<Result<Arc<PreparedActionChainInputs>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one action chain staging event batch");
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
impl NativeCommandRoot for ActionChainStagingKey {
    type Terminal = Arc<PreparedActionChainInputs>;
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
    /// Observe the complete source frontier of a selected prerequisite chain.
    /// This metadata alone does not authorize execution or publication.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_action_chain_inputs_with_repository_environment(
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
    ) -> Result<AcceptedCommand<Arc<PreparedActionChainInputs>>, BuildCommandError> {
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
            ActionChainStagingKey {
                build,
                owner,
                action,
            },
        )
        .map(|driven| driven.accepted)
        .map_err(BuildCommandError::infrastructure)
    }
}
