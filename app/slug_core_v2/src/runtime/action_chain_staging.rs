//! DICE-owned source preparation for selected actions and requested forests.

use slug_build_api_v2::AnalysisArtifact;

use super::source_staging::checked_build_frontier;
use super::source_staging::open_observed_source;
use super::*;
use crate::runtime::ActionPrerequisitePlan;
use crate::runtime::ObservedSourceArtifactInput;
use crate::runtime::PlannedAction;
use crate::runtime::RequestedActionPrerequisitePlan;
use crate::runtime::SourceArtifactInput;
use crate::runtime::SourceArtifactInputObservationKey;

#[path = "action_chain_staging/runfiles.rs"]
mod runfiles;
pub use runfiles::PreparedRunfilesAction;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct ActionChainStagingKey {
    build: BuildCommandRootKey,
    selection: ActionSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum ActionSelection {
    Selected {
        owner: ConfiguredTargetKey,
        action: usize,
    },
    Requested,
}

/// Borrowed action order with an explicit distinction between a selected action
/// and a complete requested-artifact forest. Neither conveys execution authority.
#[derive(Debug)]
pub enum PreparedActionPlan<'a> {
    Selected(ActionPrerequisitePlan<'a>),
    Requested(RequestedActionPrerequisitePlan<'a>),
}

impl<'a> PreparedActionPlan<'a> {
    pub fn actions(&self) -> &[PlannedAction<'a>] {
        match self {
            Self::Selected(plan) => plan.actions(),
            Self::Requested(plan) => plan.actions(),
        }
    }

    pub fn requested(&self) -> Option<&RequestedActionPrerequisitePlan<'a>> {
        match self {
            Self::Selected(_) => None,
            Self::Requested(plan) => Some(plan),
        }
    }

    pub fn selected_action(&self) -> Option<&'a slug_analysis_v2::ConfiguredAction> {
        match self {
            Self::Selected(plan) => plan.actions().last().map(PlannedAction::action),
            Self::Requested(_) => None,
        }
    }
}

impl ActionSelection {
    fn plan<'a>(
        &self,
        build: &'a BuildCommandEvaluation,
    ) -> Result<PreparedActionPlan<'a>, Arc<str>> {
        match self {
            Self::Selected { owner, action } => build
                .action_prerequisites(owner, *action)
                .map(PreparedActionPlan::Selected),
            Self::Requested => build
                .requested_action_prerequisites()
                .map(PreparedActionPlan::Requested),
        }
    }
}

/// Complete reachable source metadata, associated with the retained build frontier.
/// Transport effects and producer results never enter this DICE-owned value.
/// Requested preparation can retain a failed build; inspect `evaluation()` before
/// using its metadata. Such failures have no action sources or executable plan.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PreparedActionChainInputs {
    workspace: NormalizedAbsolutePath,
    evaluation: Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
    selection: ActionSelection,
    sources: Arc<[Arc<ObservedSourceArtifactInput>]>,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
}

impl PreparedActionChainInputs {
    pub fn evaluation(&self) -> Result<&BuildCommandEvaluation, &BuildCommandError> {
        self.evaluation.as_ref().as_ref()
    }

    pub fn plan(&self) -> Result<PreparedActionPlan<'_>, Arc<str>> {
        self.selection.plan(self.evaluation().map_err(error)?)
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
            selection: ActionSelection::Selected { owner, action },
        }
    }

    pub(super) fn requested(build: BuildCommandRootKey) -> Self {
        Self {
            build,
            selection: ActionSelection::Requested,
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
        if matches!(self.selection, ActionSelection::Requested) && evaluation.is_err() {
            let certificate = SourceCertificate::from_epoch(observations.dupe()).map_err(error)?;
            return Ok(PreparationOutcome::Complete(Arc::new(
                PreparedActionChainInputs {
                    workspace: self.build.workspace.dupe(),
                    evaluation,
                    selection: self.selection.clone(),
                    sources: Arc::from([]),
                    observations,
                    certificate,
                },
            )));
        }
        let build = evaluation.as_ref().as_ref().map_err(error)?;
        // Validate the entire reachable plan before observing source content.
        let plan = self.selection.plan(build)?;
        let mut artifacts = SmallSet::new();
        if let Some(requested) = plan.requested() {
            for artifact in requested.selection().artifacts() {
                if matches!(artifact, AnalysisArtifact::Source(_)) {
                    artifacts.insert(artifact.clone());
                }
            }
        }
        for step in plan.actions() {
            if let Some(spec) = step.action().runfiles_support_spec() {
                for artifact in spec.support().layout().map_err(error)?.constituents() {
                    if matches!(artifact, AnalysisArtifact::Source(_)) {
                        artifacts.insert(artifact.clone());
                    }
                }
            }
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
        let prepared = PreparedActionChainInputs {
            workspace: self.build.workspace.dupe(),
            evaluation,
            selection: self.selection.clone(),
            sources: sources.into(),
            observations,
            certificate,
        };
        // Pure preflight of all support ownership and physical topology before effects.
        for step in prepared.plan()?.actions() {
            prepared.prepare_runfiles_action(step.action())?;
        }
        Ok(PreparationOutcome::Complete(Arc::new(prepared)))
    }
}

impl fmt::Display for ActionChainStagingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "action-chain-staging:{}:{:?}",
            self.build, self.selection
        )
    }
}

#[async_trait]
impl Key for ActionChainStagingKey {
    type Value = PreparationOutcome<Result<Arc<PreparedActionChainInputs>, Arc<str>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let (value, events) = match self.prepare(ctx).await {
            Ok(PreparationOutcome::Complete(inputs)) => match inputs.warning_events() {
                Ok(events) => (PreparationOutcome::Complete(Ok(inputs)), events),
                Err(error) => (
                    PreparationOutcome::Complete(Err(error)),
                    EventBatch::empty(),
                ),
            },
            Ok(PreparationOutcome::Need(need)) => {
                (PreparationOutcome::Need(need), EventBatch::empty())
            }
            Err(error) => (
                PreparationOutcome::Complete(Err(error)),
                EventBatch::empty(),
            ),
        };
        ctx.store_evaluation_data(events)
            .expect("one action chain staging event batch");
        value
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!(x, PreparationOutcome::Complete(Ok(inputs)) if inputs.evaluation().is_ok())
            && matches!(y, PreparationOutcome::Complete(Ok(inputs)) if inputs.evaluation().is_ok())
            && x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        // Retain the observed build's diagnostic closure for typed failures,
        // while equality above prevents those failures from cutting off changes.
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
    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        matches!(self.selection, ActionSelection::Requested)
            && matches!(terminal.evaluation(), Err(error) if error.is_analysis_error())
    }
    fn terminal_demand_association(&self, terminal: &Self::Terminal) -> TerminalDemandAssociation {
        if self.allows_unavailable_terminal_roots(terminal) {
            TerminalDemandAssociation::TransientTerminalLocal
        } else {
            TerminalDemandAssociation::ClosureOnly
        }
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
        self.drive_command(request, ActionChainStagingKey::new(build, owner, action))
            .map(|driven| driven.accepted)
            .map_err(BuildCommandError::infrastructure)
    }

    /// Observe all selected sources and reachable action inputs for one requested
    /// forest, including zero-action roots. This metadata does not authorize execution.
    /// A source-certified build failure is retained in the accepted inputs;
    /// inspect `PreparedActionChainInputs::evaluation()` to distinguish it.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_requested_action_inputs_with_repository_environment(
        &self,
        targets: &[TargetPattern],
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
        self.drive_command(request, ActionChainStagingKey::requested(build))
            .map(|driven| driven.accepted)
            .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "action_chain_staging/tests.rs"]
mod tests;
