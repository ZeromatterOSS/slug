//! Shared genuine lower preparation for ordinary and private selected evaluation.
use super::*;

pub(super) struct PreparedOwnerInvocation {
    pub(super) inputs: Arc<HostSelectedExtensionOwnerInputs>,
    pub(super) first: Arc<OwnerProjection>,
    pub(super) host: ModuleExtensionOs,
    pub(super) factors: Arc<SelectedExtensionEvalFactors>,
    pub(super) modules: Arc<[PreparedInvocationModule]>,
    pub(super) owner: Arc<()>,
    pub(super) ready: Option<Arc<SelectedExtensionDownloadVerified>>,
    pub(super) module: FrozenBzlModule,
    pub(super) observations: PathObservationEpoch,
    pub(super) root_usage: Option<RootModuleExtensionUsage>,
}

#[rustfmt::skip]
pub(super) async fn prepare_owner_invocation(
    ctx: &mut DiceComputations<'_>, key: &HostSelectedExtensionOwnerPureKey,
    mode: PureInvocationsMode,
) -> Result<PreparedOwnerInvocation, OwnerPureDriver> {
    let (inputs, observations) = match mode {
        PureInvocationsMode::Legacy => match ctx.compute(&HostSelectedExtensionOwnerInputsKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => return Err(owner_complete(Err(HostSelectedExtensionOwnerPureError::Compute(error.to_string().into())), PathObservationEpoch::empty())),
        },
        PureInvocationsMode::Observed => match ctx.compute(&HostSelectedExtensionOwnerInputsObservationKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(HostSelectedExtensionOwnerPureObservationError::Inputs(error)))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => (value.result().dupe(), value.observations().dupe()),
            Err(error) => return Err(owner_complete(Err(HostSelectedExtensionOwnerPureError::Compute(error.to_string().into())), PathObservationEpoch::empty())),
        },
    };
    let inputs = match inputs.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => return Err(owner_complete(Err(HostSelectedExtensionOwnerPureError::Inputs(error.clone())), observations)),
    };
    let first_module = match owner_bzl(ctx, key, &inputs, None, observations, mode).await { Ok(value) => value, Err(outcome) => return Err(outcome) };
    let first = match owner_projection(&first_module.0, inputs.request()) {
        Ok(value) => Arc::new(value),
        Err(error) => return Err(owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1)),
    };
    drop(first_module.0);
    let host = match observe_selected_host(ctx, &key.workspace, &first.definition.environment).await {
        SourcePreparationOutcome::Need(need) => return Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => return Err(owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1)),
        SourcePreparationOutcome::Complete(Ok(host)) => host,
    };
    let factors = Arc::new(SelectedExtensionEvalFactors::from_host(&first.definition, &host));
    let owner = Arc::new(());
    let modules = match owner_invocation_modules(&inputs, &first, &owner, MAX_INVOCATION_TAGS_PER_MODULE) {
        Ok(value) => value,
        Err(error) => return Err(factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1))),
    };
    let ready = match selected_download_preflight(ctx, key).await {
        Ok(ready) => ready,
        Err(error) => return Err(factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1))),
    };
    let (module, mut observations) = match owner_bzl(ctx, key, &inputs, Some(&first), first_module.1, mode).await { Ok(value) => value, Err(outcome) => return Err(factor_owner_terminal(&factors, outcome)) };
    let second = match owner_projection(&module, inputs.request()) {
        Ok(value) => value,
        Err(error) => return Err(factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations))),
    };
    if second != *first {
        return Err(factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), "reacquired selected module extension differs")), observations)));
    }
    let root_usage = inputs.root_proxies().map(|proxies| RootModuleExtensionUsage::new(
        proxies.iter().any(|proxy| !proxy.dev_dependency),
        proxies.iter().any(|proxy| proxy.dev_dependency),
    ));
    Ok(PreparedOwnerInvocation { inputs, first, host, factors, modules, owner,
        ready, module, observations, root_usage })
}
