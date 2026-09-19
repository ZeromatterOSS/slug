//! One attempt-owned prerequisite chain, inside native selection and finalization.

use super::action_chain_staging::ActionChainStagingKey;
use super::*;

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

/// Accessible through AcceptedCommand only after the complete source frontier
/// passes final validation. Output bytes remain operation-owned until dropped.
#[derive(Debug)]
pub struct ActionChainResult<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Arc<T>,
}

impl<T> ActionChainResult<T> {
    pub fn inputs(&self) -> &PreparedActionChainInputs {
        &self.inputs
    }
    pub fn output(&self) -> &T {
        &self.output
    }
}

struct ActionChainExecutionRoot<'a, T> {
    staging: ActionChainStagingKey,
    transport: &'a T,
}

impl<T> Clone for ActionChainExecutionRoot<'_, T> {
    fn clone(&self) -> Self {
        Self {
            staging: self.staging.clone(),
            transport: self.transport,
        }
    }
}

struct ActionChainExecutionTerminal<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Option<Arc<T>>,
}

impl<T> Clone for ActionChainExecutionTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            output: self.output.clone(),
        }
    }
}

#[async_trait]
impl<T: ActionChainTransport> NativeCommandRoot for ActionChainExecutionRoot<'_, T> {
    type Terminal = ActionChainExecutionTerminal<T::Output>;

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
            let output = self.transport.finish(session).await.map_err(|error| {
                NativeDemandSessionError::Computation(anyhow::Error::new(error))
            })?;
            terminal.output = Some(Arc::new(output));
            Ok(())
        }
    }
}

impl WorkspaceRuntime {
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
            ActionChainExecutionRoot {
                staging: ActionChainStagingKey::new(build, owner, action),
                transport,
            },
        )
        .map(|driven| {
            driven.accepted.map_terminal(|terminal| ActionChainResult {
                inputs: terminal.inputs,
                output: terminal
                    .output
                    .expect("accepted action chain completed transport"),
            })
        })
        .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "action_chain_execution/tests.rs"]
mod tests;
