//! One operation-owned source action, inside native selection and finalization.

use super::source_staging::SourceStagingKey;
use super::*;

/// Trusted transport extension point. Core owns selection and freshness; the
/// implementation owns its external effects and must not publish local outputs.
/// Neither callback receives graph, repository-session, or publication authority.
pub trait SourceActionTransport: Sync {
    type Staged: Send;
    type Output: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    fn stage(
        &self,
        inputs: Arc<PreparedSourceActionInputs>,
    ) -> impl std::future::Future<Output = Result<Self::Staged, Self::Error>> + Send;
    fn execute(
        &self,
        staged: Self::Staged,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// Accessible through AcceptedCommand only after the complete source frontier
/// passes final validation. Output bytes remain operation-owned until dropped.
#[derive(Debug)]
pub struct SourceActionResult<T> {
    inputs: Arc<PreparedSourceActionInputs>,
    output: Arc<T>,
}

impl<T> SourceActionResult<T> {
    pub fn inputs(&self) -> &PreparedSourceActionInputs {
        &self.inputs
    }
    pub fn output(&self) -> &T {
        &self.output
    }
}

struct SourceExecutionRoot<'a, T> {
    staging: SourceStagingKey,
    transport: &'a T,
}

impl<T> Clone for SourceExecutionRoot<'_, T> {
    fn clone(&self) -> Self {
        Self {
            staging: self.staging.clone(),
            transport: self.transport,
        }
    }
}

struct SourceExecutionTerminal<T> {
    inputs: Arc<PreparedSourceActionInputs>,
    output: Option<Arc<T>>,
}

impl<T> Clone for SourceExecutionTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            output: self.output.clone(),
        }
    }
}

#[async_trait]
impl<T: SourceActionTransport> NativeCommandRoot for SourceExecutionRoot<'_, T> {
    type Terminal = SourceExecutionTerminal<T::Output>;

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
            .map(|inputs| SourceExecutionTerminal {
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
            terminal.inputs.check_execution_representative()?;
            let staged = self
                .transport
                .stage(terminal.inputs.clone())
                .await
                .map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::Error::new(error))
                })?;
            context
                .validate_sources(self.source_certificate(terminal).unwrap())
                .await?;
            let output = self.transport.execute(staged).await.map_err(|error| {
                NativeDemandSessionError::Computation(anyhow::Error::new(error))
            })?;
            terminal.output = Some(Arc::new(output));
            Ok(())
        }
    }
}

impl WorkspaceRuntime {
    /// Execute one selected source-only action, with freshness gates before
    /// Execute and before acceptance. This does not execute the entire build or
    /// materialize outputs. A changed post-Execute frontier retries the attempt.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_source_action_with_repository_environment<T: SourceActionTransport>(
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
    ) -> Result<AcceptedCommand<SourceActionResult<T::Output>>, BuildCommandError> {
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
            SourceExecutionRoot {
                staging: SourceStagingKey::new(build, owner, action),
                transport,
            },
        )
        .map(|driven| {
            driven.accepted.map_terminal(|terminal| SourceActionResult {
                inputs: terminal.inputs,
                output: terminal
                    .output
                    .expect("accepted source execution completed transport"),
            })
        })
        .map_err(BuildCommandError::infrastructure)
    }
}

#[cfg(test)]
#[path = "source_execution/tests.rs"]
mod tests;
