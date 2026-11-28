//! One attempt-owned prerequisite plan, inside native selection and finalization.

use std::num::NonZeroI32;

use super::action_chain_staging::ActionChainStagingKey;
use super::*;
use crate::runtime::PlannedActionOutputStaging;
use crate::runtime::PublishedActionOutputs;
use crate::runtime::PublishedPlannedActionOutputs;
use crate::runtime::configured_output::ConfiguredOutputOwner;

/// Bounded diagnostic bytes from a completed process with a nonzero exit code.
/// The transport verifies the bytes before handing them to the attempt owner.
#[derive(Eq, PartialEq, Allocative)]
pub struct CompletedActionFailure {
    exit_code: NonZeroI32,
    stderr: Vec<u8>,
}

impl CompletedActionFailure {
    pub const MAX_STDERR_BYTES: usize = 1024 * 1024;

    pub fn new(exit_code: NonZeroI32, stderr: Vec<u8>) -> Option<Self> {
        (stderr.len() <= Self::MAX_STDERR_BYTES).then_some(Self { exit_code, stderr })
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code.get()
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl fmt::Debug for CompletedActionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletedActionFailure")
            .field("exit_code", &self.exit_code)
            .field("stderr_bytes", &self.stderr.len())
            .finish()
    }
}

/// Staging either leaves an action for execution or commits a provisional
/// local result to the current attempt's ordered result table.
pub enum ActionChainStage<S> {
    RequiresExecute(S),
    CompletedLocally,
}

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
        inputs: crate::runtime::native_source_access::NativeActionChainInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> impl std::future::Future<Output = Result<Self::Session, Self::Error>> + Send;
    fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> impl std::future::Future<Output = Result<Self::Staged, Self::Error>> + Send;
    /// A transport may complete a step locally during staging. This commits
    /// only an attempt-local result. Remote execution and action cache lookup
    /// require `RequiresExecute` and Core's full source revalidation.
    fn stage_for_execution(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> impl std::future::Future<Output = Result<ActionChainStage<Self::Staged>, Self::Error>> + Send
    {
        async move {
            self.stage(session, index)
                .await
                .map(ActionChainStage::RequiresExecute)
        }
    }
    fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
    /// A requested Build may stop on a completed process failure. The attempt
    /// still needs final source validation before diagnostics can be published.
    fn execute_requested(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> impl std::future::Future<Output = Result<Option<CompletedActionFailure>, Self::Error>> + Send
    {
        async move {
            self.execute(session, index, staged).await?;
            Ok(None)
        }
    }
    fn finish(
        &self,
        session: Self::Session,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// Verified transfer of the complete operation's selected output groups.
/// Capabilities permit private staging, never visible publication. The transport
/// must reconcile every full producer result and selected subset before transfer.
pub trait ActionChainOutputTransport: ActionChainTransport {
    fn stage_outputs(
        &self,
        session: &mut Self::Session,
        staging: &[PlannedActionOutputStaging],
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}

trait PublicationPolicy<T: ActionChainTransport>: Clone + Sync {
    fn stage_outputs(
        &self,
        transport: &T,
        session: &mut T::Session,
        inputs: &crate::runtime::native_source_access::NativeActionChainInputs,
    ) -> impl std::future::Future<
        Output = Result<Option<Vec<PlannedActionOutputStaging>>, NativeDemandSessionError>,
    >;
}

#[derive(Clone)]
struct NoPublication;
impl<T: ActionChainTransport> PublicationPolicy<T> for NoPublication {
    async fn stage_outputs(
        &self,
        _transport: &T,
        _session: &mut T::Session,
        _inputs: &crate::runtime::native_source_access::NativeActionChainInputs,
    ) -> Result<Option<Vec<PlannedActionOutputStaging>>, NativeDemandSessionError> {
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
        inputs: &crate::runtime::native_source_access::NativeActionChainInputs,
    ) -> Result<Option<Vec<PlannedActionOutputStaging>>, NativeDemandSessionError> {
        let mut staging = self
            .0
            .stage_prepared_outputs(inputs)
            .map_err(|error| NativeDemandSessionError::Computation(error.into()))?;
        transport
            .stage_outputs(session, &staging)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::Error::new(error)))?;
        for group in &mut staging {
            group
                .seal()
                .map_err(|error| NativeDemandSessionError::Computation(error.into()))?;
        }
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
/// requests have no transport output. Publication metadata retains each producer
/// index; execution-only and zero-action results have no published outputs.
#[derive(Debug)]
pub struct RequestedActionResult<T> {
    inputs: Arc<PreparedActionChainInputs>,
    output: Option<Arc<T>>,
    published_outputs: Arc<[PublishedPlannedActionOutputs]>,
}

impl<T> RequestedActionResult<T> {
    pub fn inputs(&self) -> &PreparedActionChainInputs {
        &self.inputs
    }

    pub fn output(&self) -> Option<&T> {
        self.output.as_deref()
    }

    pub fn published_outputs(&self) -> &[PublishedPlannedActionOutputs] {
        &self.published_outputs
    }
}

struct ActionChainExecutionRoot<'a, T, P> {
    staging: ActionChainStagingKey,
    transport: &'a T,
    configured_output: &'a ConfiguredOutputOwner,
    publication: P,
}

impl<T, P: Clone> Clone for ActionChainExecutionRoot<'_, T, P> {
    fn clone(&self) -> Self {
        Self {
            staging: self.staging.clone(),
            transport: self.transport,
            configured_output: self.configured_output,
            publication: self.publication.clone(),
        }
    }
}

struct ActionChainExecutionTerminal<T> {
    inputs: Arc<PreparedActionChainInputs>,
    native: Option<crate::runtime::native_source_access::NativeActionChainInputs>,
    output: Option<Arc<T>>,
    failure: Option<BuildCommandError>,
    pending_outputs: Option<Arc<Mutex<Vec<PlannedActionOutputStaging>>>>,
    published_outputs: Option<Arc<[PublishedPlannedActionOutputs]>>,
}

impl<T> Clone for ActionChainExecutionTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            native: self.native.clone(),
            output: self.output.clone(),
            failure: self.failure.clone(),
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
            let mut stages = pending
                .lock()
                .map_err(|_| std::io::Error::other("action output stage lock poisoned"))?;
            terminal.published_outputs =
                Some(PlannedActionOutputStaging::publish_all(&mut stages)?);
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
    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        self.staging
            .allows_unavailable_terminal_roots(&terminal.inputs)
    }
    fn terminal_demand_association(&self, terminal: &Self::Terminal) -> TerminalDemandAssociation {
        self.staging.terminal_demand_association(&terminal.inputs)
    }
    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(NativeCommandRoot::compute(&self.staging, transaction)
            .await?
            .map(|inputs| ActionChainExecutionTerminal {
                inputs,
                native: None,
                output: None,
                failure: None,
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
            if terminal.inputs.evaluation().is_err() {
                // Retain the typed terminal for native validation and selected
                // diagnostics, without planning or creating transport effects.
                return Ok(());
            }
            // Claim every planned physical namespace before any transport effect,
            // including execute-only attempts. Configuration equality is structural;
            // the directory token alone never authorizes reuse. claim releases its
            // registry mutex before filesystem work and no guard crosses the await.
            let (steps, requested) = {
                let plan = terminal.inputs.plan().map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::anyhow!("{error}"))
                })?;
                if plan.actions().is_empty() {
                    // Native finalization still validates the full frontier.
                    return Ok(());
                }
                let mut claimed = starlark_map::small_set::SmallSet::new();
                for step in plan.actions() {
                    for output in step.action().outputs() {
                        if claimed.insert(output.root()) {
                            self.configured_output
                                .claim(output.root().configuration())
                                .map_err(|error| {
                                    NativeDemandSessionError::Computation(error.into())
                                })?;
                        }
                    }
                }
                (plan.actions().len(), plan.requested().is_some())
            };
            let native = context.native_action_chain_inputs(terminal.inputs.clone())?;
            terminal.native = Some(native.clone());
            let args_context = super::source_execution::RequestArgsContext::new(
                context.command.command_source_scope().map(Arc::as_ref),
            );
            let mut session = self
                .transport
                .start(native.clone(), &args_context)
                .await
                .map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::Error::new(error))
                })?;
            for index in 0..steps {
                let ActionChainStage::RequiresExecute(staged) = self
                    .transport
                    .stage_for_execution(&mut session, index)
                    .await
                    .map_err(|error| {
                        NativeDemandSessionError::Computation(anyhow::Error::new(error))
                    })?
                else {
                    // Final source certification still gates the provisional
                    // local result and any planned output publication.
                    continue;
                };
                // Every execution is guarded by the complete reachable source
                // frontier, including sources belonging only to its producers.
                context
                    .validate_sources(self.source_certificate(terminal).unwrap())
                    .await?;
                let execution = if requested {
                    self.transport
                        .execute_requested(&mut session, index, staged)
                        .await
                } else {
                    self.transport
                        .execute(&mut session, index, staged)
                        .await
                        .map(|()| None)
                };
                if let Some(failure) = execution.map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::Error::new(error))
                })? {
                    let plan = terminal.inputs.plan().map_err(|error| {
                        NativeDemandSessionError::Computation(anyhow::anyhow!("{error}"))
                    })?;
                    let owner = plan.actions()[index]
                        .action()
                        .context()
                        .owner()
                        .label()
                        .clone();
                    terminal.failure =
                        Some(BuildCommandError::action_failed(owner, index, failure));
                    // Drop the failed session and its prior results. Returning
                    // normally keeps this attempt inside final certification.
                    return Ok(());
                }
            }
            let pending = self
                .publication
                .stage_outputs(self.transport, &mut session, &native)
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
    /// Source-certified build and completed-action failures are accepted typed
    /// terminals. Preparation and transport errors remain outer errors.
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
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        self.execute_requested_actions_with_policy(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
            crate::runtime::ArchiveDistdirPolicy::default(),
            transport,
            NoPublication,
        )
    }

    /// Execute one requested forest and publish selected Derived outputs plus
    /// required runfiles backing and coupled support outputs after full-frontier
    /// validation. Later rename/bookkeeping failures may leave earlier artifacts
    /// changed; no accepted result is then returned.
    /// Source-certified build failures skip transport. Completed-action failures
    /// skip output transfer and publication.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_and_publish_requested_actions_with_repository_environment<
        T: ActionChainOutputTransport,
    >(
        &self,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
        transport: &T,
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        self.execute_and_publish_requested_actions_with_repository_environment_and_distdirs(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
            crate::runtime::ArchiveDistdirPolicy::default(),
            transport,
        )
    }

    /// Use the same requested Build session with request-local archive hints.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_and_publish_requested_actions_with_repository_environment_and_distdirs<
        T: ActionChainOutputTransport,
    >(
        &self,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
        archive_distdirs: crate::runtime::ArchiveDistdirPolicy,
        transport: &T,
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        self.execute_requested_actions_with_policy(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
            archive_distdirs,
            transport,
            PublishOutputs(&self.configured_output),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_requested_actions_with_policy<T: ActionChainTransport, P: PublicationPolicy<T>>(
        &self,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
        configuration_overlay: CommandConfigurationOverlay,
        archive_distdirs: crate::runtime::ArchiveDistdirPolicy,
        transport: &T,
        publication: P,
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        // The first output claim must not change workspace-root metadata after
        // that root enters the source certificate.
        self.configured_output
            .prepare_marker_dir()
            .map_err(BuildCommandError::infrastructure)?;
        let (build, mut request) = self.prepare_build_request(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            repository_environment,
            configuration_overlay,
        )?;
        request.archive_distdirs = archive_distdirs;
        self.drive_action_chain(
            request,
            ActionChainStagingKey::requested(build),
            transport,
            publication,
        )
        .map(|accepted| {
            accepted.map_terminal(|terminal| {
                terminal.inputs.evaluation().map_err(Clone::clone)?;
                if let Some(failure) = terminal.failure {
                    return Err(failure);
                }
                Ok(RequestedActionResult {
                    inputs: terminal.inputs,
                    output: terminal.output,
                    published_outputs: terminal.published_outputs.unwrap_or_default(),
                })
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
        // Claiming the first configured output must not mutate the workspace
        // root after its source certificate has been observed.
        self.configured_output
            .prepare_marker_dir()
            .map_err(BuildCommandError::infrastructure)?;
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
                published_outputs: terminal.published_outputs.map(|groups| {
                    let [group] = groups.as_ref() else {
                        unreachable!("selected-action publication has exactly one output group")
                    };
                    group.outputs().clone()
                }),
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
                configured_output: &self.configured_output,
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
