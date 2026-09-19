//! One attempt-owned prerequisite plan, inside native selection and finalization.

use super::action_chain_staging::ActionChainStagingKey;
use super::*;
use crate::runtime::ActionOutputStaging;
use crate::runtime::PublishedActionOutputs;
use crate::runtime::configured_output::ConfiguredOutputOwner;

/// Trusted transport extension point. Core owns selection and freshness; the
/// implementation owns its external effects and must not publish local outputs.
/// No callback receives graph, repository-session, or publication authority.
/// A session belongs to one native attempt. Implementations must preflight the
/// whole plan before effects and bind generated inputs only to verified producer
/// results from that session; errors and retries discard all provisional results.
pub trait ActionChainTransport: Sync {
    type Session: Send;
    type Staged: Send;
    type Output: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> impl std::future::Future<Output = Result<Self::Session, Self::Error>> + Send;
    fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> impl std::future::Future<Output = Result<Self::Staged, Self::Error>> + Send;
    fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
    fn finish(
        &self,
        session: Self::Session,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// Transport extension for verified transfer of the selected action's outputs.
/// The supplied capability permits private staging, never visible publication.
pub trait ActionChainOutputTransport: ActionChainTransport {
    fn stage_outputs(
        &self,
        session: &mut Self::Session,
        staging: &ActionOutputStaging,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}

trait PublicationPolicy<T: ActionChainTransport>: Clone + Sync {
    fn stage_outputs(
        &self,
        transport: &T,
        session: &mut T::Session,
        inputs: &PreparedActionChainInputs,
    ) -> impl std::future::Future<Output = Result<Option<ActionOutputStaging>, NativeDemandSessionError>>;
}

#[derive(Clone)]
struct NoPublication;
impl<T: ActionChainTransport> PublicationPolicy<T> for NoPublication {
    async fn stage_outputs(
        &self,
        _transport: &T,
        _session: &mut T::Session,
        _inputs: &PreparedActionChainInputs,
    ) -> Result<Option<ActionOutputStaging>, NativeDemandSessionError> {
        Ok(None)
    }
}

#[derive(Clone)]
struct PublishOutputs<'a>(&'a ConfiguredOutputOwner);
impl<T: ActionChainOutputTransport> PublicationPolicy<T> for PublishOutputs<'_> {
    async fn stage_outputs(
        &self,
        transport: &T,
        session: &mut T::Session,
        inputs: &PreparedActionChainInputs,
    ) -> Result<Option<ActionOutputStaging>, NativeDemandSessionError> {
        let plan = inputs
            .plan()
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error}")))?;
        let selected = plan.selected_action().ok_or_else(|| {
            NativeDemandSessionError::Computation(anyhow::anyhow!(
                "requested action output publication is unsupported"
            ))
        })?;
        let mut staging = self
            .0
            .stage_action_outputs(selected)
            .map_err(|error| NativeDemandSessionError::Computation(error.into()))?;
        transport
            .stage_outputs(session, &staging)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error)))?;
        staging
            .seal()
            .map_err(|error| NativeDemandSessionError::Computation(error.into()))?;
        Ok(Some(staging))
    }
}

/// Accessible through AcceptedCommand only after the complete source frontier
/// passes final validation. Output bytes remain operation-owned until dropped.
#[derive(Debug)]
pub struct ActionChainResult<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Arc<T>,
    published_outputs: Option<PublishedActionOutputs>,
}

impl<T> ActionChainResult<T> {
    pub fn inputs(&self) -> &PreparedActionChainInputs {
        &self.inputs
    }
    pub fn output(&self) -> &T {
        &self.output
    }
    pub fn published_outputs(&self) -> Option<&PublishedActionOutputs> {
        self.published_outputs.as_ref()
    }
}

/// A complete requested forest after native final validation. Zero-action
/// requests have no transport output; no requested outputs are published.
#[derive(Debug)]
pub struct RequestedActionResult<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Option<Arc<T>>,
}

impl<T> RequestedActionResult<T> {
    pub fn inputs(&self) -> &PreparedActionChainInputs {
        &self.inputs
    }

    pub fn output(&self) -> Option<&T> {
        self.output.as_deref()
    }
}

struct ActionChainExecutionRoot<'a, T, P> {
    staging: ActionChainStagingKey,
    transport: &'a T,
    publication: P,
}

impl<T, P: Clone> Clone for ActionChainExecutionRoot<'_, T, P> {
    fn clone(&self) -> Self {
        Self {
            staging: self.staging.clone(),
            transport: self.transport,
            publication: self.publication.clone(),
        }
    }
}

struct ActionChainExecutionTerminal<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Option<Arc<T>>,
    pending_outputs: Option<Arc<Mutex<ActionOutputStaging>>>,
    published_outputs: Option<PublishedActionOutputs>,
}

impl<T> Clone for ActionChainExecutionTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            output: self.output.clone(),
            pending_outputs: self.pending_outputs.clone(),
            published_outputs: self.published_outputs.clone(),
        }
    }
}

#[async_trait]
impl<T: ActionChainTransport, P: PublicationPolicy<T>> NativeCommandRoot
    for ActionChainExecutionRoot<'_, T, P>
{
    type Terminal = ActionChainExecutionTerminal<T::Output>;

    fn publish(&self, terminal: &mut Self::Terminal) -> std::io::Result<()> {
        if let Some(pending) = &terminal.pending_outputs {
            let published = pending
                .lock()
                .map_err(|_| std::io::Error::other("action output stage lock poisoned"))?
                .publish()?;
            terminal.published_outputs = Some(published);
        }
        Ok(())
    }

    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        terminal.published_outputs.is_some()
    }

    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.staging.observations(&terminal.inputs)
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        self.staging.source_certificate(&terminal.inputs)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.staging.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, terminal: &Self::Terminal) -> EventReconciliationPolicy {
        self.staging.event_reconciliation_policy(&terminal.inputs)
    }
    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(NativeCommandRoot::compute(&self.staging, transaction)
            .await?
            .map(|inputs| ActionChainExecutionTerminal {
                inputs,
                output: None,
                pending_outputs: None,
                published_outputs: None,
            }))
    }

    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            let steps = terminal
                .inputs
                .plan()
                .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error}")))?
                .actions()
                .len();
            if steps == 0 {
                // Native finalization still validates the full build/source
                // frontier, without inventing a backend session or result.
                return Ok(());
            }
            let mut session = self
                .transport
                .start(terminal.inputs.clone())
                .await
                .map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::Error::new(error))
                })?;
            for index in 0..steps {
                let staged = self
                    .transport
                    .stage(&mut session, index)
                    .await
                    .map_err(|error| {
                        NativeDemandSessionError::Computation(anyhow::Error::new(error))
                    })?;
                // Every consumer is guarded by the complete reachable source
                // frontier, including sources belonging only to its producers.
                context
                    .validate_sources(self.source_certificate(terminal).unwrap())
                    .await?;
                self.transport
                    .execute(&mut session, index, staged)
                    .await
                    .map_err(|error| {
                        NativeDemandSessionError::Computation(anyhow::Error::new(error))
                    })?;
            }
            let pending = self
                .publication
                .stage_outputs(self.transport, &mut session, &terminal.inputs)
                .await?;
            let output = self.transport.finish(session).await.map_err(|error| {
                NativeDemandSessionError::Computation(anyhow::Error::new(error))
            })?;
            terminal.output = Some(Arc::new(output));
            terminal.pending_outputs = pending.map(|stage| Arc::new(Mutex::new(stage)));
            Ok(())
        }
    }
}

impl WorkspaceRuntime {
    /// Execute all requested artifact prerequisites in one native attempt.
    /// Zero-action requests still receive final validation but skip transport.
    /// Results remain operation-owned; this operation publishes no outputs.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_requested_actions_with_repository_environment<T: ActionChainTransport>(
        &self,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
        transport: &T,
    ) -> Result<AcceptedCommand<RequestedActionResult<T::Output>>, BuildCommandError> {
        let (build, request) = self.prepare_build_request(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
        )?;
        self.drive_action_chain(
            request,
            ActionChainStagingKey::requested(build),
            transport,
            NoPublication,
        )
        .map(|accepted| {
            accepted.map_terminal(|terminal| RequestedActionResult {
                inputs: terminal.inputs,
                output: terminal.output,
            })
        })
    }

    /// Execute a selected action and its reachable prerequisites, prechecking
    /// their complete source frontier before every Execute and final acceptance.
    /// A post-Execute change retries with a fresh session; no outputs are published.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_action_chain_with_repository_environment<T: ActionChainTransport>(
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
        transport: &T,
    ) -> Result<AcceptedCommand<ActionChainResult<T::Output>>, BuildCommandError> {
        self.execute_action_chain_with_policy(
            targets,
            owner,
            action,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
            transport,
            NoPublication,
        )
    }

    /// Execute a selected chain and publish only its final action's verified
    /// outputs after final source validation. Replacement is atomic per artifact;
    /// a publication or later bookkeeping failure may leave some outputs changed.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_and_publish_action_chain_with_repository_environment<
        T: ActionChainOutputTransport,
    >(
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
        transport: &T,
    ) -> Result<AcceptedCommand<ActionChainResult<T::Output>>, BuildCommandError> {
        self.execute_action_chain_with_policy(
            targets,
            owner,
            action,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
            transport,
            PublishOutputs(&self.configured_output),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_action_chain_with_policy<T: ActionChainTransport, P: PublicationPolicy<T>>(
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
        transport: &T,
        publication: P,
    ) -> Result<AcceptedCommand<ActionChainResult<T::Output>>, BuildCommandError> {
        let (build, request) = self.prepare_build_request(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
        )?;
        self.drive_action_chain(
            request,
            ActionChainStagingKey::new(build, owner, action),
            transport,
            publication,
        )
        .map(|accepted| {
            accepted.map_terminal(|terminal| ActionChainResult {
                inputs: terminal.inputs,
                output: terminal
                    .output
                    .expect("accepted action chain completed transport"),
                published_outputs: terminal.published_outputs,
            })
        })
    }

    fn drive_action_chain<T: ActionChainTransport, P: PublicationPolicy<T>>(
        &self,
        request: NativeDemandRequestInputBundle,
        staging: ActionChainStagingKey,
        transport: &T,
        publication: P,
    ) -> Result<AcceptedCommand<ActionChainExecutionTerminal<T::Output>>, BuildCommandError> {
        self.drive_command(
            request,
            ActionChainExecutionRoot {
                staging,
                transport,
                publication,
            },
        )
        .map(|driven| driven.accepted)
        .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "action_chain_execution/tests.rs"]
mod tests;
