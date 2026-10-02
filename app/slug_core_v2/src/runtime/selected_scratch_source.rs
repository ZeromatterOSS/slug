//! Normally computed first-process prefixes, consumed only by private staging.
//! Staging storage is neither a materialization namespace nor process scratch.

use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use sha2::Digest;
use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::SelectedScratchEntry;
use slug_bzlmod_v2::SelectedScratchOperation;
use slug_bzlmod_v2::SelectedScratchPrefix;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::HostSelectedExtensionScratchPrefixKey;
use slug_loading_v2::ObservedSelectedExtensionScratchPrefix;
use slug_loading_v2::SelectedScratchPreparationDeadline;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;
use tempfile::TempDir;

use super::*;
use crate::runtime::input_image;
use crate::runtime::input_image::LogicalByteReservation;
use crate::runtime::input_image::ScopedImageSource;

#[cfg(all(test, target_os = "linux"))]
#[path = "selected_scratch_source/tests.rs"]
mod tests;

#[derive(Debug)]
enum ScratchSourceFailure {
    Unauthorized,
    Cancelled,
    StaleSource,
    UnsupportedPlatform,
    InvalidPrefix,
    NoProgress,
    Io(io::Error),
}
impl fmt::Display for ScratchSourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "selected scratch source I/O: {error}"),
            _ => write!(f, "selected scratch source: {self:?}"),
        }
    }
}
impl std::error::Error for ScratchSourceFailure {}
fn failure(reason: ScratchSourceFailure) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::Error::new(reason))
}
fn io_failure(error: io::Error) -> NativeDemandSessionError {
    failure(ScratchSourceFailure::Io(error))
}

struct StageOwner {
    observed: Option<ObservedSelectedExtensionScratchPrefix>,
    // Factory-minted pins preserve real Label roots across command discard.
    sources: Vec<ScopedImageSource>,
    root: File,
    stage: Option<TempDir>,
    logical: Option<LogicalByteReservation>,
}
impl Drop for StageOwner {
    fn drop(&mut self) {
        if self
            .stage
            .take()
            .is_some_and(|stage| stage.close().is_err())
        {
            // Storage was not proved removed: do not issue its credit again.
            if let Some(logical) = self.logical.take() {
                std::mem::forget(logical);
            }
        }
    }
}

/// No production path/FD accessor exists. Full immutable provenance and all
/// storage credits remain pinned through the shared allocation's actual Drop.
#[derive(Clone, Dupe)]
pub(super) struct SelectedScratchSourceLease(BudgetedImageBlock<StageOwner>);
impl fmt::Debug for SelectedScratchSourceLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchSourceLease")
            .finish_non_exhaustive()
    }
}
impl SelectedScratchSourceLease {
    #[allow(dead_code)]
    pub(super) fn observed(&self) -> &ObservedSelectedExtensionScratchPrefix {
        self.0.get().observed.as_ref().expect("promoted prefix")
    }
    #[cfg(test)]
    fn test_path(&self) -> &Path {
        self.0.get().stage.as_ref().expect("live stage").path()
    }
}

struct PreparedStage {
    // Owner drops before credit, including early errors and panic unwinding.
    owner: StageOwner,
    reservation: ImageDescriptorReservation,
}
impl PreparedStage {
    fn new(runtime: &WorkspaceRuntime) -> Result<Self, NativeDemandSessionError> {
        #[cfg(target_os = "linux")]
        {
            let reservation = runtime
                .scratch_resources
                .descriptor_budget()
                .try_reserve(4352)
                .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
            let stage = tempfile::Builder::new()
                .prefix("slug-selected-prefix-")
                .tempdir()
                .map_err(io_failure)?;
            let normalized = NormalizedAbsolutePath::new(stage.path().to_owned())
                .map_err(|_| failure(ScratchSourceFailure::Unauthorized))?;
            let root = input_image::open_image_anchor(&normalized)
                .map_err(NativeDemandSessionError::InputImage)?;
            if stage.path().starts_with(&runtime.workspace) {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
            Ok(Self {
                owner: StageOwner {
                    observed: None,
                    sources: Vec::new(),
                    root,
                    stage: Some(stage),
                    logical: None,
                },
                reservation,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = runtime;
            Err(failure(ScratchSourceFailure::UnsupportedPlatform))
        }
    }
}

enum PrefixAttempt {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Complete(
        ObservedSelectedExtensionScratchPrefix,
        NativeDemandTerminalSelection,
    ),
}

#[allow(dead_code)]
impl WorkspaceRuntime {
    pub(super) fn stage_selected_scratch_source(
        &self,
        key: HostSelectedExtensionScratchPrefixKey,
    ) -> Result<SelectedScratchSourceLease, NativeDemandSessionError> {
        self.stage_selected_scratch_source_inner(
            key,
            #[cfg(test)]
            None,
        )
    }

    fn stage_selected_scratch_source_inner(
        &self,
        key: HostSelectedExtensionScratchPrefixKey,
        #[cfg(test)] hook: Option<&dyn Fn(StagePhase) -> Result<(), NativeDemandSessionError>>,
    ) -> Result<SelectedScratchSourceLease, NativeDemandSessionError> {
        if key.workspace().as_path() != self.workspace {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        let attempts = key
            .policy()
            .limits()
            .max_operations
            .checked_mul(4)
            .and_then(|n| n.checked_add(self.workspace.components().count()))
            .and_then(|n| n.checked_add(key.policy().limits().max_depth))
            .and_then(|n| n.checked_add(6))
            .ok_or_else(|| failure(ScratchSourceFailure::InvalidPrefix))?;
        // This must precede command preflight/normal Label ancestor facts.
        let mut stage = PreparedStage::new(self)?;
        #[cfg(test)]
        if let Some(hook) = hook {
            hook(StagePhase::Prepared(
                stage.owner.stage.as_ref().expect("prepared stage").path(),
            ))?;
        }
        let mut guard =
            NativeDemandAbortGuard::new(self.begin_native_demand_command()?.into_command());
        let result = (|| {
            for _ in 0..attempts {
                active(guard.command(), deadline)?;
                guard.begin_attempt()?;
                let attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        prefix_attempt(self, &mut guard, &key, deadline),
                    )
                    .await
                    .map_err(|_| failure(ScratchSourceFailure::Cancelled))?
                });
                // Dropped transaction/future and drained workers precede ALL
                // progress, source I/O, capture, retries and restoration.
                self.runtime.block_on(self.dice.wait_for_idle());
                active(guard.command(), deadline)?;
                match attempt? {
                    PrefixAttempt::Need(need) => {
                        let progress = guard.progress(&need)?;
                        if matches!(
                            progress,
                            NativeDemandProgress::Repositories | NativeDemandProgress::Downloads
                        ) {
                            #[cfg(test)]
                            if let Some(hook) = hook {
                                hook(StagePhase::BeforeRefresh)?;
                            }
                            refresh_after_progress(
                                guard.command.as_mut().expect("live command"),
                                deadline,
                                #[cfg(test)]
                                hook,
                            )?;
                        }
                    }
                    PrefixAttempt::Complete(observed, selected) => {
                        let command = guard.command();
                        let reachable = selected.sidecars().demands();
                        validate(command, &key, &observed, reachable, deadline)?;
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(StagePhase::BeforeCopy)?;
                        }
                        active(command, deadline)?;
                        if !stage
                            .reservation
                            .same_budget(self.scratch_resources.descriptor_budget())
                        {
                            return Err(failure(ScratchSourceFailure::Unauthorized));
                        }
                        pin_sources(command, &observed, &mut stage, deadline)?;
                        verify_current(command, &key, &observed, reachable, deadline)?;
                        stage.owner.logical = Some(
                            self.image_resources
                                .reserve_logical(
                                    u64::try_from(observed.prefix().occurrence_bytes()).map_err(
                                        |_| failure(ScratchSourceFailure::InvalidPrefix),
                                    )?,
                                )
                                .map_err(NativeDemandSessionError::InputImage)?,
                        );
                        write_prefix(
                            &stage.owner.root,
                            observed.prefix(),
                            self,
                            &|| active(command, deadline),
                            #[cfg(test)]
                            hook,
                        )?;
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(StagePhase::AfterCopy)?;
                        }
                        verify_current(command, &key, &observed, reachable, deadline)?;
                        stage.owner.observed = Some(observed);
                        drop(selected);
                        return stage
                            .reservation
                            .freeze(stage.owner)
                            .map(SelectedScratchSourceLease)
                            .map_err(|error| NativeDemandSessionError::InputImage(error.into()));
                    }
                }
            }
            Err(failure(ScratchSourceFailure::NoProgress))
        })();
        // Even success is discard-only; lease export waits for restoration.
        let suppression = guard.suppress_attempt();
        let restoration = guard
            .command
            .as_mut()
            .expect("staging command")
            .discard_in_place();
        match restoration {
            Ok(()) => {
                guard.phase = NativeDemandAbortPhase::Closed;
                guard.command = None;
                suppression?;
                if Instant::now() >= deadline {
                    return Err(failure(ScratchSourceFailure::Cancelled));
                }
                result
            }
            Err(error) => {
                guard.phase = NativeDemandAbortPhase::FailClosed;
                Err(error)
            }
        }
    }
}

fn active(
    command: &NativeDemandCommand<'_>,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    if Instant::now() >= deadline
        || !command
            .runtime
            .repository_materializer
            .session_is_active(command.repository_session)
    {
        return Err(failure(ScratchSourceFailure::Cancelled));
    }
    Ok(())
}

fn refresh_after_progress(
    command: &mut NativeDemandCommand<'_>,
    deadline: Instant,
    #[cfg(test)] hook: Option<&dyn Fn(StagePhase) -> Result<(), NativeDemandSessionError>>,
) -> Result<(), NativeDemandSessionError> {
    active(command, deadline)?;
    let previous = &command.path_observations;
    // Precharge clone/header/row/replacement peaks before allocating. Results
    // remain raw baseline Core observations, including injected DICE history;
    // this transient credit is not a retained-prefix/result storage claim.
    let row = std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>();
    let charge = previous
        .observations()
        .iter()
        .try_fold(512usize, |total, (demand, _)| {
            if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
            demand
                .path()
                .as_path()
                .as_os_str()
                .len()
                .checked_add(row)
                .and_then(|n| n.checked_mul(16))
                .and_then(|n| total.checked_add(n))
                .ok_or_else(|| failure(ScratchSourceFailure::InvalidPrefix))
        })?;
    let _credit = command
        .runtime
        .scratch_resources
        .descriptor_budget()
        .try_reserve(charge)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
    let fresh = input_image_bridge::observe_command_paths(
        command,
        previous
            .observations()
            .keys()
            .map(|demand| demand.dupe())
            .collect(),
    )?;
    if fresh.observations().len() != previous.observations().len()
        || fresh
            .observations()
            .keys()
            .zip(previous.observations().keys())
            .any(|(left, right)| left != right)
    {
        return Err(failure(ScratchSourceFailure::Unauthorized));
    }
    active(command, deadline)?;
    #[cfg(test)]
    if let Some(hook) = hook {
        hook(StagePhase::Refreshed {
            previous,
            fresh: &fresh,
        })?;
    }
    // Only transport facts change. The next guarded compute derives a NEW
    // normal certificate from this complete epoch; no completed prefix is patched.
    command.path_observations = fresh;
    Ok(())
}

async fn prefix_attempt(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    key: &HostSelectedExtensionScratchPrefixKey,
    deadline: Instant,
) -> Result<PrefixAttempt, NativeDemandSessionError> {
    let mut data = guard.attempt_user_computation_data()?;
    data.data
        .set(SelectedScratchPreparationDeadline::new(deadline));
    let mut updater = runtime.dice.updater_with_data(data);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let observed = match slug_loading_v2::compute_selected_scratch_prefix(&mut transaction, key)
        .await
        .map_err(|_| failure(ScratchSourceFailure::InvalidPrefix))?
    {
        SourcePreparationOutcome::Need(need) => {
            guard.seal_retry()?;
            return Ok(PrefixAttempt::Need(need));
        }
        SourcePreparationOutcome::Complete(result) => {
            result.map_err(|_| failure(ScratchSourceFailure::InvalidPrefix))?
        }
    };
    let selected = guard.seal_terminal()?.select(&transaction).await?;
    validate(
        guard.command(),
        key,
        &observed,
        selected.sidecars().demands(),
        deadline,
    )?;
    Ok(PrefixAttempt::Complete(observed, selected))
}

fn validate(
    command: &NativeDemandCommand<'_>,
    key: &HostSelectedExtensionScratchPrefixKey,
    observed: &ObservedSelectedExtensionScratchPrefix,
    reachable: &SelectedWorkspaceDemands,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    active(command, deadline)?;
    let runtime = command.runtime;
    let prefix = observed.prefix();
    if !observed.matches_key(key)
        || !observed.same_resources(&runtime.scratch_resources)
        || key.workspace().as_path() != runtime.workspace
        || prefix.workspace().as_path() != runtime.workspace
        || prefix.owner() != key.owner()
        || prefix.policy() != key.policy()
        || observed.inputs().owner() != key.owner()
        || observed.inherited_environment() != &command.inputs.request.repository_environment
        || observed.platform()
            != &runtime
                .process_host
                .repository_platform()
                .map_err(|_| failure(ScratchSourceFailure::Unauthorized))?
    {
        return Err(failure(ScratchSourceFailure::Unauthorized));
    }
    for (index, witness) in observed.label_sources().iter().enumerate() {
        active(command, deadline)?;
        let selected = witness.selection();
        let recipe = selected.recipe();
        let image = selected.image_key();
        if !selected.same_budget(runtime.scratch_resources.descriptor_budget())
            || recipe.workspace().as_path() != runtime.workspace
            || witness.event_index() >= prefix.operations().len()
            || index > 0
                && observed.label_sources()[index - 1].event_index() >= witness.event_index()
            || !matches!(&prefix.operations()[witness.event_index()], SelectedScratchOperation::ExternalPath { path }
                if path.event_index() == witness.event_index()
                    && Some(path.spelling()) == selected.selected_file().as_path().to_str())
        {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
        match (recipe.source(), selected.repository()) {
            (HostRepositoryLabelPathSource::Root(root), None)
                if root.as_path() == runtime.workspace
                    && image.namespace() == PathObservationNamespace::Host
                    && image.anchor_root() == root
                    && recipe.label().package().repo().is_root() => {}
            (HostRepositoryLabelPathSource::External(route), Some((request, result))) => {
                let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
                    canonical_repo,
                    repo_spec,
                    generation_root,
                    observation_instance,
                    ..
                } = result
                else {
                    return Err(failure(ScratchSourceFailure::Unauthorized));
                };
                if !route.is_immutable_materialization()
                    || route.workspace().as_path() != runtime.workspace
                    || request.id.workspace.as_path() != runtime.workspace
                    || canonical_repo != &request.id.canonical_repo
                    || canonical_repo != route.canonical_repo()
                    || canonical_repo != recipe.label().package().repo()
                    || repo_spec != &request.repo_spec
                    || image.namespace()
                        != PathObservationNamespace::Materialization(*observation_instance)
                    || image.anchor_root().as_path() != generation_root
                    || !reachable.repository_requests().contains(request)
                {
                    return Err(failure(ScratchSourceFailure::Unauthorized));
                }
            }
            _ => return Err(failure(ScratchSourceFailure::Unauthorized)),
        }
        for (demand, expected) in selected.observations().observations() {
            let scoped = selected.repository().is_some_and(|(request, _)| {
                reachable
                    .repository_validations()
                    .iter()
                    .any(|scope| scope.request() == request && scope.paths().contains(demand))
            });
            if observed.observations().get(demand) != Some(expected)
                || !(reachable.unscoped_paths().contains(demand) || scoped)
            {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
        }
    }
    for (index, operation) in prefix.operations().iter().enumerate() {
        active(command, deadline)?;
        if matches!(operation, SelectedScratchOperation::ExternalPath { .. })
            && !observed
                .label_sources()
                .iter()
                .any(|witness| witness.event_index() == index)
        {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
    }
    for operand in prefix.process().arguments() {
        active(command, deadline)?;
        if let Some(path) = operand.external_path() {
            if !matches!(prefix.operations().get(path.event_index()),
                Some(SelectedScratchOperation::ExternalPath { path: issued }) if issued == path)
            {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
        }
    }
    for (demand, expected) in observed.observations().observations() {
        active(command, deadline)?;
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
        let in_closure = reachable.unscoped_paths().contains(demand)
            || reachable.repository_validations().iter().any(|scope| {
                scope.paths().contains(demand)
                    && reachable.repository_requests().contains(scope.request())
            });
        if !in_closure || command.path_observations.get(demand) != Some(expected) {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
    }
    for request in observed.download_requests() {
        active(command, deadline)?;
        if request.workspace().as_path() != runtime.workspace
            || request.owner() != key.owner()
            || !reachable.download_requests().contains(request)
            || !command
                .download_results
                .get(request.owner())
                .is_some_and(|result| result.request() == request)
        {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
    }
    for operation in prefix.operations() {
        active(command, deadline)?;
        if let SelectedScratchOperation::File { blob, .. }
        | SelectedScratchOperation::Download { blob, .. } = operation
        {
            if !blob.same_resources(&runtime.scratch_resources) {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
        }
        if let SelectedScratchOperation::Download { blob, path } = operation {
            let verified = blob
                .download()
                .ok_or_else(|| failure(ScratchSourceFailure::Unauthorized))?;
            let request = verified.request();
            if !observed.download_requests().contains(request)
                || request.output().as_path() != Path::new(path.relative())
                || blob.bytes() != verified.bytes().as_ref()
                || !command
                    .download_results
                    .get(request.owner())
                    .is_some_and(|current| current == verified)
            {
                return Err(failure(ScratchSourceFailure::Unauthorized));
            }
        }
    }
    // Check every reachable materialization binding, including Bzl definition
    // sources that are not issued Label operands. This owner check does no IO.
    for request in reachable.repository_requests() {
        if request.id.workspace.as_path() != runtime.workspace {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
    }
    let epoch_charge = reachable
        .repository_requests()
        .len()
        .checked_mul(4 * 4096 + 512)
        .ok_or_else(|| failure(ScratchSourceFailure::InvalidPrefix))?;
    let _epoch_credit = runtime
        .scratch_resources
        .descriptor_budget()
        .try_reserve(epoch_charge)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
    let _current = runtime
        .repository_materializer
        .selected_epoch(command.repository_session, reachable.repository_requests())
        .map_err(NativeDemandSessionError::Repository)?;
    // All known request bindings checked above; no source I/O has occurred.
    Ok(())
}

fn pin_sources(
    command: &NativeDemandCommand<'_>,
    observed: &ObservedSelectedExtensionScratchPrefix,
    stage: &mut PreparedStage,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    let first_root = |index: usize| {
        let selected = observed.label_sources()[index].selection();
        !observed.label_sources()[..index].iter().any(|earlier| {
            let earlier = earlier.selection();
            earlier.image_key().namespace() == selected.image_key().namespace()
                && earlier.image_key().anchor_root() == selected.image_key().anchor_root()
                && earlier.repository() == selected.repository()
        })
    };
    let mut count: usize = 0;
    for index in 0..observed.label_sources().len() {
        active(command, deadline)?;
        if first_root(index) {
            count += 1;
        }
    }
    let charge = count
        .checked_mul(std::mem::size_of::<ScopedImageSource>() + 4 * 4096)
        .ok_or_else(|| failure(ScratchSourceFailure::InvalidPrefix))?;
    stage
        .reservation
        .try_grow(charge)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
    stage
        .owner
        .sources
        .try_reserve_exact(count)
        .map_err(|_| failure(ScratchSourceFailure::InvalidPrefix))?;
    for (index, witness) in observed.label_sources().iter().enumerate() {
        active(command, deadline)?;
        if !first_root(index) {
            continue;
        }
        let selected = witness.selection();
        stage.owner.sources.push(
            command
                .runtime
                .repository_materializer
                .image_source(
                    command.repository_session,
                    selected.image_key().namespace(),
                    selected.image_key().anchor_root(),
                    selected.repository(),
                )
                .map_err(NativeDemandSessionError::Repository)?,
        );
    }
    Ok(())
}

fn verify_current(
    command: &NativeDemandCommand<'_>,
    key: &HostSelectedExtensionScratchPrefixKey,
    observed: &ObservedSelectedExtensionScratchPrefix,
    reachable: &SelectedWorkspaceDemands,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    validate(command, key, observed, reachable, deadline)?;
    let count = observed.observations().observations().len();
    let charge = count
        .checked_mul(
            std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>() * 16,
        )
        .and_then(|n| n.checked_add(512))
        .ok_or_else(|| failure(ScratchSourceFailure::InvalidPrefix))?;
    let _credit = command
        .runtime
        .scratch_resources
        .descriptor_budget()
        .try_reserve(charge)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
    let fresh = input_image_bridge::observe_command_paths(
        command,
        observed
            .observations()
            .observations()
            .iter()
            .map(|(demand, _)| demand.clone())
            .collect(),
    )?;
    if &fresh != observed.observations() {
        return Err(failure(ScratchSourceFailure::StaleSource));
    }
    let _factory_credit = command
        .runtime
        .scratch_resources
        .descriptor_budget()
        .try_reserve(4 * 4096 + 256)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
    // Exact materialization session/request/result checked again via its factory.
    for witness in observed.label_sources() {
        let selected = witness.selection();
        let _pin = command
            .runtime
            .repository_materializer
            .image_source(
                command.repository_session,
                selected.image_key().namespace(),
                selected.image_key().anchor_root(),
                selected.repository(),
            )
            .map_err(NativeDemandSessionError::Repository)?;
    }
    validate(command, key, observed, reachable, deadline)
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum StagePhase<'a> {
    Prepared(&'a Path),
    BeforeCopy,
    Chunk,
    AfterCopy,
    BeforeRefresh,
    Refreshed {
        previous: &'a PathObservationEpoch,
        fresh: &'a PathObservationEpoch,
    },
}

fn write_prefix(
    root: &File,
    prefix: &SelectedScratchPrefix,
    runtime: &WorkspaceRuntime,
    active: &dyn Fn() -> Result<(), NativeDemandSessionError>,
    #[cfg(test)] hook: Option<&dyn Fn(StagePhase) -> Result<(), NativeDemandSessionError>>,
) -> Result<(), NativeDemandSessionError> {
    #[cfg(target_os = "linux")]
    {
        let _scratch = runtime
            .scratch_resources
            .descriptor_budget()
            .try_reserve(64 * 1024 + 3 * 4096 + 256)
            .map_err(|error| NativeDemandSessionError::InputImage(error.into()))?;
        for operation in prefix.operations() {
            let (path, blob, mode) = match operation {
                SelectedScratchOperation::File {
                    path,
                    blob,
                    executable,
                } => (path, blob, if *executable { 0o755 } else { 0o644 }),
                SelectedScratchOperation::Download { path, blob } => {
                    let verified = blob
                        .download()
                        .ok_or_else(|| failure(ScratchSourceFailure::Unauthorized))?;
                    (
                        path,
                        blob,
                        if verified.request().executable() {
                            0o755
                        } else {
                            0o644
                        },
                    )
                }
                _ => continue,
            };
            active()?;
            let mut output = fs::file(root, path.relative(), true, active)?;
            let mut hash = sha2::Sha256::new();
            for chunk in blob.bytes().chunks(64 * 1024) {
                active()?;
                let mut remaining = chunk;
                while !remaining.is_empty() {
                    active()?;
                    let written = output.write(remaining).map_err(io_failure)?;
                    if written == 0 {
                        return Err(io_failure(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "staging made no write progress",
                        )));
                    }
                    remaining = &remaining[written..];
                }
                hash.update(chunk);
                #[cfg(test)]
                if let Some(hook) = hook {
                    hook(StagePhase::Chunk)?;
                }
            }
            active()?;
            if blob
                .download()
                .is_some_and(|verified| hash.finalize().as_slice() != verified.request().sha256())
            {
                return Err(failure(ScratchSourceFailure::StaleSource));
            }
            fs::mode(&output, mode)?;
        }
        let mut buffer = [0u8; 64 * 1024];
        for entry in prefix.entries() {
            active()?;
            match entry {
                SelectedScratchEntry::Directory { path } => {
                    fs::directory(root, path.relative(), true, active)?;
                }
                SelectedScratchEntry::File { path, blob, .. } => {
                    let mut file = fs::file(root, path.relative(), false, active)?;
                    let metadata = file.metadata().map_err(io_failure)?;
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.len() != blob.bytes().len() as u64
                        || metadata.permissions().mode() & 0o7777 != entry.mode()
                    {
                        return Err(failure(ScratchSourceFailure::StaleSource));
                    }
                    let mut offset = 0;
                    loop {
                        active()?;
                        let n = file.read(&mut buffer).map_err(io_failure)?;
                        if n == 0 {
                            break;
                        }
                        if blob.bytes().get(offset..offset + n) != Some(&buffer[..n]) {
                            return Err(failure(ScratchSourceFailure::StaleSource));
                        }
                        offset += n;
                    }
                    if offset != blob.bytes().len() {
                        return Err(failure(ScratchSourceFailure::StaleSource));
                    }
                }
            }
        }
        active()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, prefix, runtime, active);
        #[cfg(test)]
        let _ = hook;
        Err(failure(ScratchSourceFailure::UnsupportedPlatform))
    }
}

#[cfg(target_os = "linux")]
mod fs {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;

    use super::*;

    fn name(component: &str) -> Result<CString, NativeDemandSessionError> {
        if component.is_empty() || matches!(component, "." | "..") || component.contains('/') {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
        CString::new(component).map_err(|_| failure(ScratchSourceFailure::Unauthorized))
    }
    fn opened(fd: libc::c_int) -> Result<File, NativeDemandSessionError> {
        if fd < 0 {
            Err(io_failure(io::Error::last_os_error()))
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    pub(super) fn directory(
        root: &File,
        relative: &str,
        create: bool,
        active: &dyn Fn() -> Result<(), NativeDemandSessionError>,
    ) -> Result<File, NativeDemandSessionError> {
        let mut current = root.try_clone().map_err(io_failure)?;
        if relative.is_empty() {
            if create {
                mode(&current, 0o755)?;
            }
            return Ok(current);
        }
        for component in relative.split('/') {
            active()?;
            let component = name(component)?;
            if create {
                let result =
                    unsafe { libc::mkdirat(current.as_raw_fd(), component.as_ptr(), 0o755) };
                if result != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                    return Err(io_failure(io::Error::last_os_error()));
                }
            }
            current = opened(unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    component.as_ptr(),
                    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                )
            })?;
            if create {
                mode(&current, 0o755)?;
            }
        }
        Ok(current)
    }
    pub(super) fn file(
        root: &File,
        relative: &str,
        write: bool,
        active: &dyn Fn() -> Result<(), NativeDemandSessionError>,
    ) -> Result<File, NativeDemandSessionError> {
        let (parent, basename) = relative.rsplit_once('/').unwrap_or(("", relative));
        let parent = directory(root, parent, write, active)?;
        let basename = name(basename)?;
        active()?;
        let flags = libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if write {
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
            } else {
                libc::O_RDONLY
            };
        let file =
            opened(unsafe { libc::openat(parent.as_raw_fd(), basename.as_ptr(), flags, 0o600) })?;
        if !file.metadata().map_err(io_failure)?.is_file() {
            return Err(failure(ScratchSourceFailure::Unauthorized));
        }
        Ok(file)
    }
    pub(super) fn mode(file: &File, mode: u32) -> Result<(), NativeDemandSessionError> {
        if unsafe { libc::fchmod(file.as_raw_fd(), mode) } != 0 {
            return Err(io_failure(io::Error::last_os_error()));
        }
        Ok(())
    }
}
