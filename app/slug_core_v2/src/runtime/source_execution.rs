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
        inputs: crate::runtime::native_source_access::NativeSourceActionInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> impl std::future::Future<Output = Result<Self::Staged, Self::Error>> + Send;
    fn execute(
        &self,
        staged: Self::Staged,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// Borrowed only while the current consumer expands its arguments.
/// Semantic results and transport sessions never retain this request scope.
pub(super) struct RequestArgsContext<'a> {
    scope: Option<&'a source_service::SourceServiceScope>,
}

impl<'a> RequestArgsContext<'a> {
    pub(super) fn new(scope: Option<&'a source_service::SourceServiceScope>) -> Self {
        Self { scope }
    }
}

impl std::fmt::Debug for RequestArgsContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestArgsContext").finish_non_exhaustive()
    }
}

impl slug_build_api_v2::ArgsDirectoryContext for RequestArgsContext<'_> {
    fn is_cancelled(&self) -> bool {
        self.scope.is_some_and(|scope| scope.is_cancelled())
    }

    fn list(
        &self,
        artifact: &slug_build_api_v2::AnalysisArtifact,
    ) -> Result<Vec<slug_build_api_v2::AnalysisArtifact>, slug_build_api_v2::ArgsExpansionError>
    {
        slug_build_api_v2::ArgsDirectoryContext::list(
            &slug_build_api_v2::UnsupportedTreeArgsDirectoryContext,
            artifact,
        )
    }
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
    native: Option<crate::runtime::native_source_access::NativeSourceActionInputs>,
    output: Option<Arc<T>>,
}

impl<T> Clone for SourceExecutionTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            native: self.native.clone(),
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
                native: None,
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
            let args_context =
                RequestArgsContext::new(context.command.runtime.source_scope.as_deref());
            let native = context.native_source_inputs(terminal.inputs.clone())?;
            terminal.native = Some(native.clone());
            let staged = self
                .transport
                .stage(native, &args_context)
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
