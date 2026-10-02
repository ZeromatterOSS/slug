//! Normally computed first-process source prefix; no process result or native authority.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::ObservedSelectedLabelSource;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_bzlmod_v2::SelectedScratchError;
use slug_bzlmod_v2::SelectedScratchExternalPath;
use slug_bzlmod_v2::SelectedScratchPolicy;
use slug_bzlmod_v2::SelectedScratchPrefix;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;
mod context;
pub(super) use context::CheckpointContext;
use context::CheckpointRecorder;
use context::DiscardPrint;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedExtensionScratchPrefixKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    policy: SelectedScratchPolicy,
}
impl HostSelectedExtensionScratchPrefixKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        policy: SelectedScratchPolicy,
    ) -> Self {
        Self {
            workspace,
            owner,
            policy,
        }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn policy(&self) -> &SelectedScratchPolicy {
        &self.policy
    }
}
impl fmt::Display for HostSelectedExtensionScratchPrefixKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selected-scratch-prefix:{}:{}",
            self.workspace,
            self.owner.unique_name()
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Allocative)]
pub struct SelectedScratchPreparationDeadline(#[allocative(skip)] Instant);
impl SelectedScratchPreparationDeadline {
    pub fn new(deadline: Instant) -> Self {
        Self(deadline)
    }
    pub fn deadline(&self) -> Instant {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
enum PrefixErrorKind {
    Shared(SelectedScratchError),
    Label(SelectedLabelSourceError),
    Owner(OwnerPureResult),
    Observation(HostSelectedExtensionOwnerPureObservationError),
    Operational,
    Unsupported,
}
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedScratchPrefixError(PrefixErrorKind);
impl SelectedScratchPrefixError {
    pub(crate) fn operational() -> Self {
        Self(PrefixErrorKind::Operational)
    }
    pub(super) fn unsupported() -> Self {
        Self(PrefixErrorKind::Unsupported)
    }
    pub fn is_transient(&self) -> bool {
        match &self.0 {
            PrefixErrorKind::Shared(e) => e.is_transient(),
            PrefixErrorKind::Label(e) => e.is_transient(),
            PrefixErrorKind::Owner(e) => e
                .as_ref()
                .as_ref()
                .err()
                .is_some_and(|error| error.is_transient()),
            PrefixErrorKind::Observation(_) | PrefixErrorKind::Operational => true,
            PrefixErrorKind::Unsupported => false,
        }
    }
}
impl fmt::Debug for SelectedScratchPrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Retained source witnesses include raw environment; diagnostics expose none.
        f.debug_struct("SelectedScratchPrefixError")
            .field("transient", &self.is_transient())
            .finish()
    }
}
impl fmt::Display for SelectedScratchPrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("selected scratch preparation refused")
    }
}
impl std::error::Error for SelectedScratchPrefixError {}
impl From<SelectedScratchError> for SelectedScratchPrefixError {
    fn from(e: SelectedScratchError) -> Self {
        Self(PrefixErrorKind::Shared(e))
    }
}
impl From<SelectedLabelSourceError> for SelectedScratchPrefixError {
    fn from(e: SelectedLabelSourceError) -> Self {
        Self(PrefixErrorKind::Label(e))
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedScratchLabelSource {
    event: SelectedScratchExternalPath,
    selection: ObservedSelectedLabelSource,
}
impl SelectedScratchLabelSource {
    pub fn event_index(&self) -> usize {
        self.event.event_index()
    }
    pub fn selection(&self) -> &ObservedSelectedLabelSource {
        &self.selection
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct PrefixInner {
    prefix: SelectedScratchPrefix,
    inputs: Arc<HostSelectedExtensionOwnerInputs>,
    first: Arc<OwnerProjection>,
    factors: Arc<SelectedExtensionEvalFactors>,
    platform: RepositoryPlatform,
    observations: PathObservationEpoch,
    labels: Vec<SelectedScratchLabelSource>,
    downloads: Vec<Arc<SelectedExtensionDownloadRequest>>,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedExtensionScratchPrefix(BudgetedImageBlock<PrefixInner>);
impl ObservedSelectedExtensionScratchPrefix {
    pub fn prefix(&self) -> &SelectedScratchPrefix {
        &self.0.get().prefix
    }
    pub fn inputs(&self) -> &Arc<HostSelectedExtensionOwnerInputs> {
        &self.0.get().inputs
    }
    pub fn platform(&self) -> &RepositoryPlatform {
        &self.0.get().platform
    }
    pub fn inherited_environment(&self) -> &RepositoryEnvironmentSnapshot {
        self.prefix().process().inherited_environment()
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn label_sources(&self) -> &[SelectedScratchLabelSource] {
        &self.0.get().labels
    }
    pub fn download_requests(&self) -> &[Arc<SelectedExtensionDownloadRequest>] {
        &self.0.get().downloads
    }
    pub fn matches_key(&self, key: &HostSelectedExtensionScratchPrefixKey) -> bool {
        self.prefix().workspace().as_path() == key.workspace.as_path()
            && self.prefix().owner() == &key.owner
            && self.prefix().policy() == &key.policy
            && self.inputs().owner() == &key.owner
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.prefix().same_resources(resources)
            && self
                .label_sources()
                .iter()
                .all(|label| label.selection().same_budget(resources.descriptor_budget()))
    }
}

type Outcome = SourcePreparationOutcome<
    Result<ObservedSelectedExtensionScratchPrefix, SelectedScratchPrefixError>,
>;
#[async_trait]
impl Key for HostSelectedExtensionScratchPrefixKey {
    type Value = Outcome;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        match compute(ctx, self, cancellation).await {
            Ok(value) => value,
            Err(error) => SourcePreparationOutcome::Complete(Err(error)),
        }
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        Self::validity(a) && Self::validity(b) && a.complete_eq(b)
    }
    fn validity(value: &Self::Value) -> bool {
        match value {
            SourcePreparationOutcome::Need(_) => false,
            SourcePreparationOutcome::Complete(result) => {
                result.as_ref().err().is_none_or(|e| !e.is_transient())
            }
        }
    }
}

fn lower(outcome: OwnerPureDriver) -> Result<Outcome, SelectedScratchPrefixError> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(e)) => {
            Err(SelectedScratchPrefixError(PrefixErrorKind::Observation(e)))
        }
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            if result.as_ref().is_err() {
                Err(SelectedScratchPrefixError(PrefixErrorKind::Owner(result)))
            } else {
                Err(SelectedScratchPrefixError::operational())
            }
        }
    }
}

async fn compute(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionScratchPrefixKey,
    cancellation: &CancellationContext,
) -> Result<Outcome, SelectedScratchPrefixError> {
    let resources = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
        .map_err(|_| SelectedScratchPrefixError::operational())?
        .dupe();
    let deadline = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchPreparationDeadline>()
        .map_or_else(
            |_| Instant::now() + Duration::from_secs(60),
            |d| d.deadline(),
        );
    let active = || !cancellation.is_cancelled() && Instant::now() < deadline;
    if !active() {
        return Err(SelectedScratchPrefixError::operational());
    }
    let pure_key = HostSelectedExtensionOwnerPureKey::new(key.workspace.dupe(), key.owner.clone());
    let owner_invocation::PreparedOwnerInvocation {
        inputs,
        first,
        host,
        factors,
        modules,
        owner,
        ready,
        module,
        mut observations,
        root_usage,
    } = match owner_invocation::prepare_owner_invocation(
        ctx,
        &pure_key,
        PureInvocationsMode::Observed,
    )
    .await
    {
        Ok(value) => value,
        Err(outcome) => return lower(outcome),
    };
    let inherited = ctx
        .compute(&RepositoryEnvironmentSnapshotKey::new(key.workspace.dupe()))
        .await
        .map_err(|_| SelectedScratchPrefixError::operational())?;
    let transaction = ctx
        .per_transaction_data()
        .data
        .get::<RepositoryHostInputTransaction>()
        .map_err(|_| SelectedScratchPrefixError::operational())?;
    if transaction.snapshot() != &inherited {
        return Err(SelectedScratchPrefixError::operational());
    }
    let definition = module
        .module
        .get_assigned(inputs.request().parts().1)
        .expect("authenticated export")
        .0
        .downcast::<FrozenModuleExtensionDefinition>()
        .expect("authenticated kind");
    let mut ready_paths = SelectedPathReady::default();
    let mut ready_environment = Vec::new();
    loop {
        if !active() {
            return Err(SelectedScratchPrefixError::operational());
        }
        let download = Arc::new(SelectedDownloadInvocation::new(
            key.workspace.dupe(),
            key.owner.clone(),
            ready.clone(),
        ));
        let watch = Arc::new(SelectedWatchInvocation::new());
        let path = SelectedPathInvocation::new(ready_paths.clone())?;
        let getenv = Arc::new(SelectedGetenvInvocation::new(ready_environment.clone()));
        let recorder = CheckpointRecorder::new(key, &resources, &inherited)?;
        let (returned, calls) = {
            let invocation = Module::new();
            let modules = modules
                .iter()
                .cloned()
                .map(|module| module.materialize(invocation.frozen_heap()))
                .collect::<Vec<_>>()
                .into();
            let base =
                InvocationContext::new_modules(modules, &owner, root_usage, Some(host.clone()))
                    .with_selected_download(download.clone())
                    .with_selected_watch(watch.clone())
                    .with_selected_path(path.clone())
                    .with_selected_getenv(getenv.clone());
            let context = invocation
                .heap()
                .alloc_simple(CheckpointContext::new(base, recorder.clone()));
            let repository_rules =
                RepositoryRuleInvocationState::new().with_bzl_manifest(&module.manifest);
            let mut eval = Evaluator::new(&invocation);
            eval.extra = Some(&repository_rules);
            eval.set_print_handler(&DiscardPrint);
            eval.set_check_cancelled(Box::new(|| !active()));
            let returned = eval
                .eval_function(definition.implementation.to_value(), &[context], &[])
                .map(|_| ())
                .map_err(|_| ());
            drop(eval);
            (returned, repository_rules.records())
        };
        if !active() {
            return Err(SelectedScratchPrefixError::operational());
        }
        let reads = getenv
            .reads()
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let invocation_factors = Arc::new(factors.with_dynamic_environment(&reads));
        if let Some(name) = getenv
            .pending()
            .map_err(|_| SelectedScratchPrefixError::operational())?
        {
            let value = match observe_selected_getenv(ctx, &key.workspace, name.clone()).await {
                SourcePreparationOutcome::Need(need) => {
                    return Ok(SourcePreparationOutcome::Need(need));
                }
                SourcePreparationOutcome::Complete(Err(_)) => {
                    return Err(SelectedScratchPrefixError::operational());
                }
                SourcePreparationOutcome::Complete(Ok(value)) => value,
            };
            ready_environment.push((name, value));
            continue;
        }
        if let Some(need) = download
            .need()
            .map_err(|_| SelectedScratchPrefixError::operational())?
        {
            return Ok(SourcePreparationOutcome::Need(
                SourcePreparationNeeds::selected_extension_download((*need).clone()),
            ));
        }
        let pending_path = path
            .pending()
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let used = download
            .used()
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        if let Some(used) = &used {
            ctx.compute(&UsedDownloadRequestKey::new(used.clone()))
                .await
                .map_err(|_| SelectedScratchPrefixError::operational())?;
        }
        let watched = watch
            .called()
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        observations = match observe_selected_watches(
            ctx,
            &pure_key,
            &inputs,
            &first,
            &calls,
            &watched,
            observations,
        )
        .await
        {
            Ok(value) => value,
            Err(outcome) => return lower(factor_owner_terminal(&invocation_factors, outcome)),
        };
        if let Some(error) = path.operation_error() {
            return lower(factor_owner_terminal(
                &invocation_factors,
                owner_complete(Err(source_owner_after(&inputs, calls, error)), observations),
            ));
        }
        if let Some(label) = pending_path {
            let (prepared, next) = match observe_selected_path_label(
                ctx,
                &pure_key,
                &inputs,
                &first,
                &calls,
                &label,
                observations,
            )
            .await
            {
                Ok(value) => value,
                Err(outcome) => return lower(factor_owner_terminal(&invocation_factors, outcome)),
            };
            observations = next;
            ready_paths = ready_paths.append(prepared)?;
            continue;
        }
        if !calls.is_empty() {
            return Err(SelectedScratchPrefixError::unsupported());
        }
        let recorded = recorder.take(returned.is_err())?;
        let mut credit = resources
            .descriptor_budget()
            .try_reserve(
                recorded
                    .labels
                    .get()
                    .len()
                    .checked_mul(std::mem::size_of::<SelectedScratchLabelSource>())
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(SelectedScratchPrefixError::operational)?,
            )
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(recorded.labels.get().len())
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        for issued in recorded.labels.get().iter() {
            let selection = match crate::dependency_cycle::compute_selected_label_source(
                ctx,
                &SelectedLabelSourceKey::new(issued.recipe.dupe()),
            )
            .await?
            {
                SourcePreparationOutcome::Need(need) => {
                    return Ok(SourcePreparationOutcome::Need(need));
                }
                SourcePreparationOutcome::Complete(value) => value?,
            };
            if selection.recipe() != &issued.recipe || selection.selected_file() != &issued.spelling
            {
                return Err(SelectedScratchPrefixError::operational());
            }
            observations =
                merge_checkpoint_frontier(&mut credit, &observations, selection.observations())?;
            labels.push(SelectedScratchLabelSource {
                event: issued.event.dupe(),
                selection,
            });
        }
        credit
            .try_grow(std::mem::size_of::<Arc<SelectedExtensionDownloadRequest>>())
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let mut downloads = Vec::new();
        if let Some(used) = used {
            downloads
                .try_reserve_exact(1)
                .map_err(|_| SelectedScratchPrefixError::operational())?;
            downloads.push(used);
        }
        if !active() {
            return Err(SelectedScratchPrefixError::operational());
        }
        let output = credit
            .freeze(PrefixInner {
                prefix: recorded.prefix,
                inputs,
                first,
                factors: invocation_factors,
                platform: host.platform.clone(),
                observations,
                labels,
                downloads,
            })
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        return Ok(SourcePreparationOutcome::Complete(Ok(
            ObservedSelectedExtensionScratchPrefix(output),
        )));
    }
}

/// Merge newly owned checkpoint rows only after reserving copy/replacement peaks.
pub(super) fn merge_checkpoint_frontier(
    credit: &mut ImageDescriptorReservation,
    observations: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, SelectedScratchPrefixError> {
    let rows = observations
        .observations()
        .len()
        .checked_add(incoming.observations().len())
        .ok_or_else(SelectedScratchPrefixError::operational)?;
    let bytes = rows
        .checked_mul(
            std::mem::size_of::<(
                slug_workspace_v2::PathObservationDemand,
                Arc<slug_workspace_v2::PathObservationResult>,
            )>() * 16,
        )
        .and_then(|n| n.checked_add(512))
        .ok_or_else(SelectedScratchPrefixError::operational)?;
    credit
        .try_grow(bytes)
        .map_err(|_| SelectedScratchPrefixError::operational())?;
    union_pure_observations(observations, incoming)
        .map_err(|_| SelectedScratchPrefixError::operational())
}
