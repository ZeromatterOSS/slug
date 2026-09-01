/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionSource;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputs;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsError;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsKey;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsObservationKey;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
#[cfg(test)]
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

use crate::attrs::CoercedAttributeValue;
use crate::bzl_module::ExternalBzlModuleEvalKey;
use crate::bzl_module::ExternalBzlModuleObservationKey;
use crate::bzl_module::FrozenBzlModule;
use crate::bzl_module::HostBzlModuleError;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostBzlModuleObservationKey;
use crate::bzl_module::HostPreparedModuleExtensionInputs;
use crate::bzl_module::HostPreparedModuleExtensionInputsError;
use crate::bzl_module::HostPreparedModuleExtensionInputsKey;
use crate::bzl_module::HostPreparedModuleExtensionInputsObservationError;
use crate::bzl_module::HostPreparedModuleExtensionInputsObservationKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::bzl_module::PreparedModuleExtensionInput;
use crate::bzl_module::PreparedModuleExtensionTag;
use crate::bzl_module::RepositoryBzlLabel;
use crate::module_extension_repository_rule::RepositoryRuleCallRecord;
use crate::module_extension_repository_rule::RepositoryRuleInvocationState;
use crate::package::FrozenModuleExtensionDefinition;
use crate::package::ModuleExtensionDefinitionProjection;
use crate::package::prepare_module_extension_tag_attributes;
use crate::package::validate_module_extension_tag_schema;
use crate::starlark_label::StarlarkLabel;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocations {
    pub(crate) prepared: Arc<HostPreparedModuleExtensionInputs>,
    pub(crate) invoked: Arc<[HostPureModuleExtensionInvocationReceipt]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocationReceipt {
    pub(crate) request: HostSelectedExtensionDefinitionLoadRequest,
    pub(crate) repository_rule_calls: Arc<[RepositoryRuleCallRecord]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPureModuleExtensionInvocationsError {
    Prepared(HostPreparedModuleExtensionInputsError),
    PreparedCompute(CompactString),
    AfterPrepared {
        prepared: Arc<HostPreparedModuleExtensionInputs>,
        request: Option<HostSelectedExtensionDefinitionLoadRequest>,
        completed: Arc<[HostPureModuleExtensionInvocationReceipt]>,
        current_calls: Arc<[RepositoryRuleCallRecord]>,
        error: HostPureModuleExtensionInvocationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPureModuleExtensionInvocationError {
    UnsupportedFactors,
    Label(CompactString),
    Bzl(HostBzlModuleError),
    SelectedBzl(crate::bzl_module::ExternalBzlModuleError),
    Drift(CompactString),
    Invocation(CompactString),
    Result(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocationsKey {
    workspace: NormalizedAbsolutePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling; a later packet owns consumer activation.
pub(crate) struct HostPureModuleExtensionInvocationsObservationKey(
    HostPureModuleExtensionInvocationsKey,
);

#[allow(dead_code)]
impl HostPureModuleExtensionInvocationsObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostPureModuleExtensionInvocationsKey::new(workspace))
    }
}

#[rustfmt::skip]
impl fmt::Display for HostPureModuleExtensionInvocationsObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) }
}

type PureInvocationsResult =
    Arc<Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed sibling.
pub(crate) struct ObservedHostPureModuleExtensionInvocations {
    result: PureInvocationsResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostPureModuleExtensionInvocations {
    pub(crate) fn result(
        &self,
    ) -> &Arc<Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>>
    {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[rustfmt::skip]
enum PureModuleExtensionInvocationsObservationError {
    Prepared(HostPreparedModuleExtensionInputsObservationError),
    HostBzl { prepared: Arc<HostPreparedModuleExtensionInputs>, index: usize, error: ObservedPathFrontierError },
    Merge { prepared: Arc<HostPreparedModuleExtensionInputs>, index: usize, error: ObservedPathFrontierError },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPureModuleExtensionInvocationsObservationError(
    PureModuleExtensionInvocationsObservationError,
);

impl HostPureModuleExtensionInvocationsKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostPureModuleExtensionInvocationsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-pure-module-extension-invocations:{}",
            self.workspace
        )
    }
}

pub(crate) type HostPureModuleExtensionInvocationsOutcome =
    SourcePreparationOutcome<PureInvocationsResult>;

type PureInvocationsDriverOutcome = SourcePreparationOutcome<
    Result<
        (PureInvocationsResult, PathObservationEpoch),
        PureModuleExtensionInvocationsObservationError,
    >,
>;

#[derive(Clone, Copy)]
enum PureInvocationsMode {
    Legacy,
    Observed,
}

#[rustfmt::skip]
fn pure_complete(value: Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>, observations: PathObservationEpoch) -> PureInvocationsDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

#[derive(Default)]
struct InvocationPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl InvocationPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for InvocationPrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }
}

fn union_pure_observations(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                right
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(ObservedPathFrontierError::from)
}

#[rustfmt::skip]
async fn pure_prepared(ctx: &mut DiceComputations<'_>, workspace: &NormalizedAbsolutePath, mode: PureInvocationsMode) -> Result<(Arc<HostPreparedModuleExtensionInputs>, PathObservationEpoch), PureInvocationsDriverOutcome> {
    macro_rules! compute_error {
        ($error:expr) => {
            pure_complete(
                Err(HostPureModuleExtensionInvocationsError::PreparedCompute(
                    $error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )
        };
    }
    let child = match mode {
        PureInvocationsMode::Legacy => match ctx.compute(&HostPreparedModuleExtensionInputsKey::new(workspace.dupe())).await {
            Err(error) => return Err(compute_error!(error)),
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => SourcePreparationOutcome::Complete(Ok((result, PathObservationEpoch::empty()))),
        },
        PureInvocationsMode::Observed => match ctx.compute(&HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe())).await {
            Err(error) => return Err(compute_error!(error)),
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => SourcePreparationOutcome::Complete(Err(error)),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => SourcePreparationOutcome::Complete(Ok((observed.result().dupe(), observed.observations().dupe()))),
        },
    };
    let (result, observations) = match child {
        SourcePreparationOutcome::Need(need) => return Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => return Err(SourcePreparationOutcome::Complete(Err(PureModuleExtensionInvocationsObservationError::Prepared(error)))),
        SourcePreparationOutcome::Complete(Ok(value)) => value,
    };
    match result.as_ref() {
        Ok(prepared) => Ok((Arc::new(prepared.clone()), observations)),
        Err(error) => Err(pure_complete(Err(HostPureModuleExtensionInvocationsError::Prepared(error.clone())), observations)),
    }
}

struct PurePreflight {
    module: starlark::environment::FrozenModule,
    implementation: starlark::values::FrozenValue,
    tag_classes: Arc<[CompactString]>,
}

type PureHostBzlChild = SourcePreparationOutcome<
    Result<(PureBzlCarrier, PathObservationEpoch), ObservedPathFrontierError>,
>;

enum PureBzlCarrier {
    Root(Arc<Result<FrozenBzlModule, HostBzlModuleError>>),
    Selected(Arc<Result<FrozenBzlModule, crate::bzl_module::ExternalBzlModuleError>>),
}

#[rustfmt::skip]
async fn pure_host_bzl(ctx: &mut DiceComputations<'_>, workspace: &NormalizedAbsolutePath, prepared: &Arc<HostPreparedModuleExtensionInputs>, index: usize, request: &HostSelectedExtensionDefinitionLoadRequest, label: HostRootBzlLabel, observations: PathObservationEpoch, mode: PureInvocationsMode) -> Result<(FrozenBzlModule, PathObservationEpoch), PureInvocationsDriverOutcome> {
    let after = |error| HostPureModuleExtensionInvocationsError::AfterPrepared {
        prepared: prepared.clone(),
        request: Some(request.clone()),
        completed: Arc::from([]),
        current_calls: Arc::from([]),
        error,
    };
    let target = RootPackageBzlTarget::parse(request.parts().0.target().as_str()).map_err(|error| {
        pure_complete(Err(after(HostPureModuleExtensionInvocationError::Label(error.to_string().into()))), observations.dupe())
    })?;
    let selected = match request.source() {
        HostSelectedExtensionDefinitionSource::Root => None,
        source => RootRepositoryRoute::for_selected_extension_definition(workspace.dupe(), source),
    };
    let child: PureHostBzlChild = match (mode, selected) {
        (PureInvocationsMode::Legacy, None) => match ctx.compute(&HostBzlModuleEvalKey::new_bzlmod(workspace.dupe(), label)).await {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => SourcePreparationOutcome::Complete(Ok((PureBzlCarrier::Root(result), PathObservationEpoch::empty()))),
            Err(error) => return Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()))), observations)),
        },
        (PureInvocationsMode::Observed, None) => match ctx.compute(&HostBzlModuleObservationKey::new_bzlmod(workspace.dupe(), label)).await {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(PureModuleExtensionInvocationsObservationError::HostBzl { prepared: prepared.dupe(), index, error }))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => SourcePreparationOutcome::Complete(Ok((PureBzlCarrier::Root(Arc::new(observed.result().clone())), observed.observations().dupe()))),
            Err(error) => return Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()))), observations)),
        },
        (PureInvocationsMode::Legacy, Some(route)) => match ctx.compute(&ExternalBzlModuleEvalKey::new_bzlmod(route, RepositoryBzlLabel::new(request.parts().0.package().package().clone(), target.dupe()).expect("selected target was parsed"))).await {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(result)) => SourcePreparationOutcome::Complete(Ok((PureBzlCarrier::Selected(result), PathObservationEpoch::empty()))),
            Err(error) => return Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()))), observations)),
        },
        (PureInvocationsMode::Observed, Some(route)) => match ctx.compute(&ExternalBzlModuleObservationKey::new_bzlmod(route, RepositoryBzlLabel::new(request.parts().0.package().package().clone(), target).expect("selected target was parsed"))).await {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(PureModuleExtensionInvocationsObservationError::HostBzl { prepared: prepared.dupe(), index, error }))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => SourcePreparationOutcome::Complete(Ok((PureBzlCarrier::Selected(observed.result().dupe()), observed.observations().dupe()))),
            Err(error) => return Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()))), observations)),
        },
    };
    let (result, incoming) = match child {
        SourcePreparationOutcome::Need(need) => return Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Ok(value)) => value,
        SourcePreparationOutcome::Complete(Err(_)) => unreachable!("Host-Bzl outer is handled before the shared finisher"),
    };
    let observations = union_pure_observations(&observations, &incoming).map_err(|error| {
        SourcePreparationOutcome::Complete(Err(PureModuleExtensionInvocationsObservationError::Merge { prepared: prepared.dupe(), index, error }))
    })?;
    match result {
        PureBzlCarrier::Root(result) => match result.as_ref() {
            Ok(module) => Ok((module.clone(), observations)),
            Err(error) => Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::Bzl(error.clone()))), observations)),
        },
        PureBzlCarrier::Selected(result) => match result.as_ref() {
            Ok(module) => Ok((module.clone(), observations)),
            Err(error) => Err(pure_complete(Err(after(HostPureModuleExtensionInvocationError::SelectedBzl(error.clone()))), observations)),
        },
    }
}

#[rustfmt::skip]
async fn preflight_pure_invocations(ctx: &mut DiceComputations<'_>, workspace: &NormalizedAbsolutePath, prepared: &Arc<HostPreparedModuleExtensionInputs>, mut observations: PathObservationEpoch, mode: PureInvocationsMode) -> Result<(Vec<PurePreflight>, PathObservationEpoch), PureInvocationsDriverOutcome> {
    let after = |request: Option<&HostSelectedExtensionDefinitionLoadRequest>,
                 completed: &[HostPureModuleExtensionInvocationReceipt],
                 current_calls: Arc<[RepositoryRuleCallRecord]>,
                 error| {
        HostPureModuleExtensionInvocationsError::AfterPrepared {
            prepared: prepared.clone(),
            request: request.cloned(),
            completed: completed.to_vec().into(),
            current_calls,
            error,
        }
    };
    let mut preflight = Vec::with_capacity(prepared.inputs.len());
    for (index, input) in prepared.inputs.iter().enumerate() {
        let (request, _, _, _) = input.input.parts().0.parts();
        let loaded = &prepared.definitions.definitions[index];
        let loaded_manifest = &loaded.manifest;
        let loaded_definition = &loaded.definition;
        if !loaded_definition.environment.is_empty()
            || loaded_definition.os_dependent
            || loaded_definition.arch_dependent
            || loaded_definition.facts_version != 0
        {
            return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::UnsupportedFactors)), observations));
        }
        let target = match RootPackageBzlTarget::parse(request.target().as_str()) {
            Ok(target) => target,
            Err(error) => return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::Label(error.to_string().into()))), observations)),
        };
        let label = HostRootBzlLabel::new(request.package().package().clone(), target);
        let (module, next_observations) = pure_host_bzl(
            ctx,
            workspace,
            prepared,
            index,
            input.input.parts().0,
            label,
            observations,
            mode,
        )
        .await?;
        observations = next_observations;
        if &module.manifest != loaded_manifest {
            return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::Drift("reacquired manifest differs".into()))), observations));
        }
        let export = match module.module.get_assigned(input.input.parts().0.parts().1) {
            Ok((value, _visibility)) => value,
            Err(error) => return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::Drift(error.to_string().into()))), observations)),
        };
        let definition = match export.downcast::<FrozenModuleExtensionDefinition>() {
            Ok(value) => value,
            Err(_) => return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::Drift("reacquired export is not module_extension".into()))), observations)),
        };
        if &definition.projection() != loaded_definition {
            return Err(pure_complete(Err(after(Some(input.input.parts().0), &[], Arc::from([]), HostPureModuleExtensionInvocationError::Drift("reacquired definition differs".into()))), observations));
        }
        preflight.push(PurePreflight {
            module: module.module.dupe(),
            implementation: definition.implementation,
            tag_classes: loaded_definition
                .tag_classes
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
                .into(),
        });
    }
    Ok((preflight, observations))
}

#[rustfmt::skip]
fn invoke_pure_preflight(prepared: Arc<HostPreparedModuleExtensionInputs>, preflight: Vec<PurePreflight>, observations: PathObservationEpoch, capture_events: bool, event_batch: &mut EventBatch) -> PureInvocationsDriverOutcome {
    let after = |request: Option<&HostSelectedExtensionDefinitionLoadRequest>,
                 completed: &[HostPureModuleExtensionInvocationReceipt],
                 current_calls: Arc<[RepositoryRuleCallRecord]>,
                 error| {
        HostPureModuleExtensionInvocationsError::AfterPrepared {
            prepared: prepared.clone(),
            request: request.cloned(),
            completed: completed.to_vec().into(),
            current_calls,
            error,
        }
    };
    let mut invoked = Vec::with_capacity(prepared.inputs.len());
    for (input, preflight) in prepared.inputs.iter().zip(preflight) {
        let _module_lifetime = preflight.module;
        let invocation_module = Module::new();
        let owner = Arc::new(());
        let context = invocation_module
            .heap()
            .alloc_simple(InvocationContext::new(
                input,
                preflight.tag_classes,
                &owner,
                invocation_module.frozen_heap(),
            ));
        let capture = capture_events.then(InvocationPrintCapture::default);
        let repository_rules = RepositoryRuleInvocationState::new();
        let returned = {
            let mut evaluator = Evaluator::new(&invocation_module);
            evaluator.extra = Some(&repository_rules);
            if let Some(capture) = capture.as_ref() {
                evaluator.set_print_handler(capture);
            }
            let result =
                evaluator.eval_function(preflight.implementation.to_value(), &[context], &[]);
            drop(evaluator);
            result
        };
        if let Some(capture) = capture {
            let capture = capture.into_batch();
            *event_batch = EventBatch::from_events(
                event_batch
                    .events()
                    .iter()
                    .cloned()
                    .chain(capture.events().iter().cloned()),
            );
        }
        let repository_rule_calls = repository_rules.records();
        let returned = match returned {
            Ok(value) => value,
            Err(error) => {
                return pure_complete(Err(after(Some(input.input.parts().0), &invoked, repository_rule_calls, HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()))), observations);
            }
        };
        if !returned.is_none() {
            return pure_complete(Err(after(Some(input.input.parts().0), &invoked, repository_rule_calls, HostPureModuleExtensionInvocationError::Result(format!("module extension must return None, got {}", returned.get_type()).into()))), observations);
        }
        invoked.push(HostPureModuleExtensionInvocationReceipt {
            request: input.input.parts().0.clone(),
            repository_rule_calls,
        });
    }
    pure_complete(Ok(HostPureModuleExtensionInvocations { prepared, invoked: invoked.into() }), observations)
}

async fn compute_pure_invocations(
    ctx: &mut DiceComputations<'_>,
    key: &HostPureModuleExtensionInvocationsKey,
    mode: PureInvocationsMode,
) -> PureInvocationsDriverOutcome {
    let capture_events = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok();
    let mut event_batch = EventBatch::empty();
    let (prepared, observations) = match pure_prepared(ctx, &key.workspace, mode).await {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let value = match preflight_pure_invocations(ctx, &key.workspace, &prepared, observations, mode)
        .await
    {
        Ok((preflight, observations)) => invoke_pure_preflight(
            prepared,
            preflight,
            observations,
            capture_events,
            &mut event_batch,
        ),
        Err(terminal) => terminal,
    };
    if capture_events && matches!(value, SourcePreparationOutcome::Complete(Ok(_))) {
        ctx.store_evaluation_data(event_batch)
            .expect("pure module-extension invocation stores one local Complete event batch");
    }
    value
}

#[async_trait]
impl Key for HostPureModuleExtensionInvocationsKey {
    type Value = HostPureModuleExtensionInvocationsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_pure_invocations(ctx, self, PureInvocationsMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy pure invocations have no observed frontier")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostPureModuleExtensionInvocationsObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostPureModuleExtensionInvocations,
            HostPureModuleExtensionInvocationsObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_pure_invocations(ctx, &self.0, PureInvocationsMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostPureModuleExtensionInvocationsObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostPureModuleExtensionInvocations {
                    result,
                    observations,
                }))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostSelectedExtensionOwnerPureResult { pub(crate) inputs: Arc<HostSelectedExtensionOwnerInputs>, pub(crate) receipt: HostPureModuleExtensionInvocationReceipt }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostSelectedExtensionOwnerPureError { Inputs(HostSelectedExtensionOwnerInputsError), Compute(CompactString), AfterInputs { inputs: Arc<HostSelectedExtensionOwnerInputs>, request: HostSelectedExtensionDefinitionLoadRequest, current_calls: Arc<[RepositoryRuleCallRecord]>, message: CompactString } }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostSelectedExtensionOwnerPureKey { workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner> }
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostSelectedExtensionOwnerPureObservationKey(HostSelectedExtensionOwnerPureKey);
#[rustfmt::skip]
impl HostSelectedExtensionOwnerPureKey { pub(crate) fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self { workspace, owner } } }
#[rustfmt::skip]
impl HostSelectedExtensionOwnerPureObservationKey { pub(crate) fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self(HostSelectedExtensionOwnerPureKey::new(workspace, owner)) } }
#[rustfmt::skip]
impl fmt::Display for HostSelectedExtensionOwnerPureKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "host-selected-extension-owner-pure:{}:{:?}", self.workspace, self.owner) } }
#[rustfmt::skip]
impl fmt::Display for HostSelectedExtensionOwnerPureObservationKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) } }
type OwnerPureResult =
    Arc<Result<HostSelectedExtensionOwnerPureResult, HostSelectedExtensionOwnerPureError>>;
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostSelectedExtensionOwnerPure { result: OwnerPureResult, observations: PathObservationEpoch }
#[rustfmt::skip]
impl ObservedHostSelectedExtensionOwnerPure { pub(crate) fn result(&self) -> &OwnerPureResult { &self.result } pub(crate) fn observations(&self) -> &PathObservationEpoch { &self.observations } }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct OwnerProjection { manifest: crate::bzl_module::BzlLoadManifest, definition: ModuleExtensionDefinitionProjection }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostSelectedExtensionOwnerPureObservationError { Inputs(HostSelectedExtensionOwnerInputsObservationError), HostBzl { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Option<Arc<OwnerProjection>>, error: ObservedPathFrontierError }, Merge { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Option<Arc<OwnerProjection>>, error: ObservedPathFrontierError } }
type OwnerPureDriver = SourcePreparationOutcome<
    Result<(OwnerPureResult, PathObservationEpoch), HostSelectedExtensionOwnerPureObservationError>,
>;
#[rustfmt::skip]
fn owner_complete(value: Result<HostSelectedExtensionOwnerPureResult, HostSelectedExtensionOwnerPureError>, observations: PathObservationEpoch) -> OwnerPureDriver { SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations))) }
#[rustfmt::skip]
fn owner_after(inputs: &Arc<HostSelectedExtensionOwnerInputs>, calls: Arc<[RepositoryRuleCallRecord]>, message: impl Into<CompactString>) -> HostSelectedExtensionOwnerPureError { HostSelectedExtensionOwnerPureError::AfterInputs { inputs: inputs.clone(), request: inputs.request().clone(), current_calls: calls, message: message.into() } }

#[rustfmt::skip]
fn owner_projection(module: &FrozenBzlModule, request: &HostSelectedExtensionDefinitionLoadRequest) -> Result<OwnerProjection, CompactString> {
    let (export, _visibility) = module.module.get_assigned(request.parts().1).map_err(|error| CompactString::from(error.to_string()))?;
    let definition = export.downcast::<FrozenModuleExtensionDefinition>().map_err(|_| CompactString::from("selected export is not module_extension"))?;
    let projection = definition.projection();
    if !projection.environment.is_empty() || projection.os_dependent || projection.arch_dependent || projection.facts_version != 0 {
        return Err("selected module extension has unsupported factors".into());
    }
    Ok(OwnerProjection { manifest: module.manifest.clone(), definition: projection })
}

#[rustfmt::skip]
fn owner_invocation_modules(inputs: &HostSelectedExtensionOwnerInputs, projection: &OwnerProjection, owner: &Arc<()>) -> Result<Arc<[PreparedInvocationModule]>, CompactString> {
    for (_, schema) in projection.definition.tag_classes.iter() {
        validate_module_extension_tag_schema(schema).map_err(|error| CompactString::from(error.to_string()))?;
    }
    let classes: Arc<[CompactString]> = projection.definition.tag_classes.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>().into();
    inputs.modules().iter().enumerate().map(|(module_index, input)| {
        let (context_repo, name, version, is_root, mapping, tags) = input.parts();
        let tags = tags.iter().enumerate().map(|(tag_index, tag)| {
            let schema = projection.definition.tag_classes.iter().find_map(|(name, schema)| (name == &tag.tag_class).then_some(schema.as_ref())).ok_or_else(|| CompactString::from(format!("unknown tag class '{}'", tag.tag_class)))?;
            let attributes = prepare_module_extension_tag_attributes(schema, &tag.attributes, context_repo, mapping).map_err(|error| CompactString::from(error.to_string()))?;
            Ok(PreparedModuleExtensionTag { tag_class: tag.tag_class.clone(), attributes, dev_dependency: tag.dev_dependency, location: tag.location.clone(), module_index, tag_index })
        }).collect::<Result<Vec<_>, CompactString>>()?;
        Ok(PreparedInvocationModule { name: name.into(), version: version.into(), is_root, tag_classes: classes.clone(), tags: tags.into(), owner: owner.clone() })
    }).collect::<Result<Vec<_>, CompactString>>().map(Into::into)
}

#[rustfmt::skip]
async fn owner_bzl(ctx: &mut DiceComputations<'_>, key: &HostSelectedExtensionOwnerPureKey, inputs: &Arc<HostSelectedExtensionOwnerInputs>, first: Option<&Arc<OwnerProjection>>, observations: PathObservationEpoch, mode: PureInvocationsMode) -> Result<(FrozenBzlModule, PathObservationEpoch), OwnerPureDriver> {
    let request = inputs.request();
    let target = RootPackageBzlTarget::parse(request.parts().0.target().as_str()).map_err(|error| owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations.dupe()))?;
    let label = HostRootBzlLabel::new(request.parts().0.package().package().clone(), target.dupe());
    let selected = match request.source() {
        HostSelectedExtensionDefinitionSource::Root => None,
        source => RootRepositoryRoute::for_selected_extension_definition(key.workspace.dupe(), source),
    };
    let child = match (mode, selected) {
        (PureInvocationsMode::Legacy, None) => match ctx.compute(&HostBzlModuleEvalKey::new_bzlmod(key.workspace.dupe(), label.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(result)) => (result.as_ref().clone().map_err(|error| error.to_string()), PathObservationEpoch::empty()),
            Err(error) => return Err(owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations)),
        },
        (PureInvocationsMode::Observed, None) => match ctx.compute(&HostBzlModuleObservationKey::new_bzlmod(key.workspace.dupe(), label.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(HostSelectedExtensionOwnerPureObservationError::HostBzl { inputs: inputs.clone(), first: first.cloned(), error }))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => (value.result().clone().map_err(|error| error.to_string()), value.observations().dupe()),
            Err(error) => return Err(owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations)),
        },
        (PureInvocationsMode::Legacy, Some(route)) => match ctx.compute(&ExternalBzlModuleEvalKey::new_bzlmod(route, RepositoryBzlLabel::new(request.parts().0.package().package().clone(), target.dupe()).expect("selected target was parsed"))).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(result)) => (result.as_ref().clone().map_err(|error| error.to_string()), PathObservationEpoch::empty()),
            Err(error) => return Err(owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations)),
        },
        (PureInvocationsMode::Observed, Some(route)) => match ctx.compute(&ExternalBzlModuleObservationKey::new_bzlmod(route, RepositoryBzlLabel::new(request.parts().0.package().package().clone(), target).expect("selected target was parsed"))).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(HostSelectedExtensionOwnerPureObservationError::HostBzl { inputs: inputs.clone(), first: first.cloned(), error }))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => (value.result().as_ref().clone().map_err(|error| error.to_string()), value.observations().dupe()),
            Err(error) => return Err(owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations)),
        },
    };
    let observations = union_pure_observations(&observations, &child.1).map_err(|error| SourcePreparationOutcome::Complete(Err(HostSelectedExtensionOwnerPureObservationError::Merge { inputs: inputs.clone(), first: first.cloned(), error })))?;
    match child.0 {
        Ok(module) => Ok((module, observations)),
        Err(error) => Err(owner_complete(Err(owner_after(inputs, Arc::from([]), error.to_string())), observations)),
    }
}

#[rustfmt::skip]
async fn compute_owner_pure(ctx: &mut DiceComputations<'_>, key: &HostSelectedExtensionOwnerPureKey, mode: PureInvocationsMode) -> OwnerPureDriver {
    let (inputs, observations) = match mode {
        PureInvocationsMode::Legacy => match ctx.compute(&HostSelectedExtensionOwnerInputsKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => return owner_complete(Err(HostSelectedExtensionOwnerPureError::Compute(error.to_string().into())), PathObservationEpoch::empty()),
        },
        PureInvocationsMode::Observed => match ctx.compute(&HostSelectedExtensionOwnerInputsObservationKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostSelectedExtensionOwnerPureObservationError::Inputs(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => (value.result().dupe(), value.observations().dupe()),
            Err(error) => return owner_complete(Err(HostSelectedExtensionOwnerPureError::Compute(error.to_string().into())), PathObservationEpoch::empty()),
        },
    };
    let inputs = match inputs.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => return owner_complete(Err(HostSelectedExtensionOwnerPureError::Inputs(error.clone())), observations),
    };
    let first_module = match owner_bzl(ctx, key, &inputs, None, observations, mode).await { Ok(value) => value, Err(outcome) => return outcome };
    let first = match owner_projection(&first_module.0, inputs.request()) {
        Ok(value) => Arc::new(value),
        Err(error) => return owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1),
    };
    drop(first_module.0);
    let owner = Arc::new(());
    let modules = match owner_invocation_modules(&inputs, &first, &owner) {
        Ok(value) => value,
        Err(error) => return owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1),
    };
    let (module, observations) = match owner_bzl(ctx, key, &inputs, Some(&first), first_module.1, mode).await { Ok(value) => value, Err(outcome) => return outcome };
    let second = match owner_projection(&module, inputs.request()) {
        Ok(value) => value,
        Err(error) => return owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations),
    };
    if second != *first {
        return owner_complete(Err(owner_after(&inputs, Arc::from([]), "reacquired selected module extension differs")), observations);
    }
    let definition = module.module.get_assigned(inputs.request().parts().1).expect("projection authenticated export").0.downcast::<FrozenModuleExtensionDefinition>().expect("projection authenticated kind");
    let invocation_module = Module::new();
    let modules = modules
        .iter()
        .cloned()
        .map(|module| module.materialize(invocation_module.frozen_heap()))
        .collect::<Vec<_>>()
        .into();
    let context = invocation_module.heap().alloc_simple(InvocationContext::new_modules(modules, &owner));
    let capture = ctx.per_transaction_data().data.get::<CaptureEvaluationEvents>().is_ok().then(InvocationPrintCapture::default);
    let repository_rules = RepositoryRuleInvocationState::new();
    let returned = {
        let mut evaluator = Evaluator::new(&invocation_module);
        evaluator.extra = Some(&repository_rules);
        if let Some(capture) = capture.as_ref() {
            evaluator.set_print_handler(capture);
        }
        evaluator.eval_function(definition.implementation.to_value(), &[context], &[])
    };
    let calls = repository_rules.records();
    let request = inputs.request().clone();
    let result = match returned {
        Err(error) => Err(owner_after(&inputs, calls, error.to_string())),
        Ok(value) if !value.is_none() => Err(owner_after(&inputs, calls, format!("module extension must return None, got {}", value.get_type()))),
        Ok(_) => Ok(HostSelectedExtensionOwnerPureResult {
            inputs,
            receipt: HostPureModuleExtensionInvocationReceipt { request, repository_rule_calls: calls },
        }),
    };
    if let Some(capture) = capture {
        ctx.store_evaluation_data(capture.into_batch()).expect("selected owner invocation stores one local Complete event batch");
    }
    owner_complete(result, observations)
}

#[async_trait]
#[rustfmt::skip]
impl Key for HostSelectedExtensionOwnerPureKey {
    type Value = SourcePreparationOutcome<OwnerPureResult>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value { match compute_owner_pure(ctx, self, PureInvocationsMode::Legacy).await { SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need), SourcePreparationOutcome::Complete(Ok((result, _))) => SourcePreparationOutcome::Complete(result), SourcePreparationOutcome::Complete(Err(_)) => unreachable!() } }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool { x.complete_eq(y) }
    fn validity(value: &Self::Value) -> bool { value.is_complete() }
}

#[async_trait]
#[rustfmt::skip]
impl Key for HostSelectedExtensionOwnerPureObservationKey {
    type Value = SourcePreparationOutcome<Result<ObservedHostSelectedExtensionOwnerPure, HostSelectedExtensionOwnerPureObservationError>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value { match compute_owner_pure(ctx, &self.0, PureInvocationsMode::Observed).await { SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need), SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(error)), SourcePreparationOutcome::Complete(Ok((result, observations))) => SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedExtensionOwnerPure { result, observations })) } }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool { x.complete_eq(y) }
    fn validity(value: &Self::Value) -> bool { value.is_complete() }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationContext {
    modules: Arc<[InvocationModule]>,
    #[allocative(skip)]
    owner: Arc<()>,
}

impl InvocationContext {
    fn new(
        input: &PreparedModuleExtensionInput,
        tag_classes: Arc<[CompactString]>,
        owner: &Arc<()>,
        frozen_heap: &FrozenHeap,
    ) -> Self {
        let (_, _, name, version, is_root, _) = input.input.parts();
        Self::new_modules(
            Arc::from([InvocationModule {
                name: name.into(),
                version: version.into(),
                is_root,
                tag_classes,
                tags: input
                    .tags
                    .iter()
                    .cloned()
                    .map(|tag| InvocationTag::new(tag, owner, frozen_heap))
                    .collect::<Vec<_>>()
                    .into(),
            }]),
            owner,
        )
    }

    fn new_modules(modules: Arc<[InvocationModule]>, owner: &Arc<()>) -> Self {
        Self {
            modules,
            owner: owner.clone(),
        }
    }
}

impl fmt::Display for InvocationContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<module_ctx>")
    }
}
starlark::starlark_simple_value!(InvocationContext);

#[starlark_value(type = "module_ctx")]
impl<'v> StarlarkValue<'v> for InvocationContext {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "modules").then(|| heap.alloc_simple(InvocationModuleList(self.modules.clone())))
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(invocation_context_methods)
    }
}

#[starlark_module]
fn invocation_context_methods(builder: &mut MethodsBuilder) {
    fn is_dev_dependency(this: Value, tag: Value) -> anyhow::Result<bool> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let tag = InvocationTag::from_value(tag)
            .ok_or_else(|| anyhow::anyhow!("expected a tag from ctx.modules"))?;
        if !Arc::ptr_eq(&this.owner, &tag.owner) {
            anyhow::bail!("tag belongs to another module_ctx");
        }
        Ok(tag.dev_dependency)
    }
    fn tag_sort_key(this: Value, tag: Value) -> anyhow::Result<InvocationTagSortKey> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let tag = InvocationTag::from_value(tag)
            .ok_or_else(|| anyhow::anyhow!("expected a tag from ctx.modules"))?;
        if !Arc::ptr_eq(&this.owner, &tag.owner) {
            anyhow::bail!("tag belongs to another module_ctx");
        }
        Ok(InvocationTagSortKey(tag.module_index, tag.tag_index))
    }
}

#[derive(Debug, Clone, Allocative)]
struct PreparedInvocationModule {
    name: CompactString,
    version: CompactString,
    is_root: bool,
    tag_classes: Arc<[CompactString]>,
    tags: Arc<[PreparedModuleExtensionTag]>,
    #[allocative(skip)]
    owner: Arc<()>,
}

impl PreparedInvocationModule {
    fn materialize(self, frozen_heap: &FrozenHeap) -> InvocationModule {
        InvocationModule {
            name: self.name,
            version: self.version,
            is_root: self.is_root,
            tag_classes: self.tag_classes,
            tags: self
                .tags
                .iter()
                .cloned()
                .map(|tag| InvocationTag::new(tag, &self.owner, frozen_heap))
                .collect::<Vec<_>>()
                .into(),
        }
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationModule {
    name: CompactString,
    version: CompactString,
    is_root: bool,
    tag_classes: Arc<[CompactString]>,
    tags: Arc<[InvocationTag]>,
}
impl fmt::Display for InvocationModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<bazel_module>")
    }
}
starlark::starlark_simple_value!(InvocationModule);
#[starlark_value(type = "bazel_module")]
impl<'v> StarlarkValue<'v> for InvocationModule {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc_str(&self.name).to_value()),
            "version" => Some(heap.alloc_str(&self.version).to_value()),
            "is_root" => Some(Value::new_bool(self.is_root)),
            "tags" => Some(heap.alloc_simple(InvocationTags {
                classes: self.tag_classes.clone(),
                tags: self.tags.clone(),
            })),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTags {
    classes: Arc<[CompactString]>,
    tags: Arc<[InvocationTag]>,
}
impl fmt::Display for InvocationTags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<bazel_module_tags>")
    }
}
starlark::starlark_simple_value!(InvocationTags);
#[starlark_value(type = "bazel_module_tags")]
impl<'v> StarlarkValue<'v> for InvocationTags {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.classes.iter().any(|class| class == name).then(|| {
            heap.alloc_simple(InvocationTagList(
                self.tags
                    .iter()
                    .filter(|tag| tag.class == name)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ))
        })
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTag {
    class: CompactString,
    #[allocative(skip)]
    attributes: Arc<[(CompactString, FrozenValue)]>,
    dev_dependency: bool,
    location: slug_bzlmod_v2::LogicalSpan,
    module_index: usize,
    tag_index: usize,
    #[allocative(skip)]
    owner: Arc<()>,
}
impl InvocationTag {
    fn new(tag: PreparedModuleExtensionTag, owner: &Arc<()>, frozen_heap: &FrozenHeap) -> Self {
        Self {
            class: tag.tag_class,
            attributes: tag
                .attributes
                .iter()
                .map(|(name, value)| (name.clone(), allocate_frozen_attribute(value, frozen_heap)))
                .collect::<Vec<_>>()
                .into(),
            dev_dependency: tag.dev_dependency,
            location: tag.location,
            module_index: tag.module_index,
            tag_index: tag.tag_index,
            owner: owner.clone(),
        }
    }
}
impl fmt::Display for InvocationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "'{}' tag at {}:{}:{}",
            self.class, self.location.file.0, self.location.start_line, self.location.start_column
        )
    }
}
starlark::starlark_simple_value!(InvocationTag);
#[starlark_value(type = "bazel_module_tag")]
impl<'v> StarlarkValue<'v> for InvocationTag {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let _ = heap;
        self.attributes
            .iter()
            .find_map(|(attribute, value)| (attribute == name).then(|| value.to_value()))
    }

    fn dir_attr(&self) -> Vec<String> {
        self.attributes
            .iter()
            .map(|(name, _)| name.to_string())
            .collect()
    }
}

fn allocate_frozen_attribute(value: &CoercedAttributeValue, heap: &FrozenHeap) -> FrozenValue {
    let label = |value: &slug_identity_v2::CanonicalLabel| {
        heap.alloc_simple(StarlarkLabel::new(value.clone()))
    };
    match value {
        CoercedAttributeValue::String(value) => heap.alloc_str(value).to_frozen_value(),
        CoercedAttributeValue::Boolean(value) => FrozenValue::new_bool(*value),
        CoercedAttributeValue::Integer(value) => heap.alloc(*value),
        CoercedAttributeValue::IntegerList(values) => heap.alloc(AllocList(
            values.iter().copied().map(|value| heap.alloc(value)),
        )),
        CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => label(value),
        CoercedAttributeValue::None => FrozenValue::new_none(),
        CoercedAttributeValue::StringList(values) => heap.alloc(AllocList(
            values
                .iter()
                .map(|value| heap.alloc_str(value).to_frozen_value()),
        )),
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            heap.alloc(AllocList(values.iter().map(label)))
        }
        CoercedAttributeValue::StringDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (
                    heap.alloc_str(key).to_frozen_value(),
                    heap.alloc_str(value).to_frozen_value(),
                )
            })))
        }
        CoercedAttributeValue::StringListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    heap.alloc_str(key).to_frozen_value(),
                    heap.alloc(AllocList(
                        values
                            .iter()
                            .map(|value| heap.alloc_str(value).to_frozen_value()),
                    )),
                )
            })))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (heap.alloc_str(key).to_frozen_value(), label(value))
            })))
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (label(key), heap.alloc_str(value).to_frozen_value())
            })))
        }
        CoercedAttributeValue::LabelListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    heap.alloc_str(key).to_frozen_value(),
                    heap.alloc(AllocList(values.iter().map(label))),
                )
            })))
        }
        CoercedAttributeValue::Selector { .. } | CoercedAttributeValue::Concatenation(_, _) => {
            unreachable!("module-extension tag values are non-configurable")
        }
    }
}

fn list_index(index: i32, len: usize) -> starlark::Result<usize> {
    let index = if index < 0 {
        len as i64 + index as i64
    } else {
        index as i64
    };
    (index >= 0 && index < len as i64)
        .then_some(index as usize)
        .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationModuleList(Arc<[InvocationModule]>);
impl fmt::Display for InvocationModuleList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[<bazel_module>]")
    }
}
starlark::starlark_simple_value!(InvocationModuleList);
#[starlark_value(type = "list")]
impl<'v> StarlarkValue<'v> for InvocationModuleList {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let index = index
            .unpack_i32()
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index must be int")))?;
        self.0
            .get(list_index(index, self.0.len())?)
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
    }
    fn length(&self) -> starlark::Result<i32> {
        Ok(self.0.len() as i32)
    }
    fn iterate_collect(&self, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self
            .0
            .iter()
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .collect())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTagList(Arc<[InvocationTag]>);
impl fmt::Display for InvocationTagList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[<bazel_module_tag>]")
    }
}
starlark::starlark_simple_value!(InvocationTagList);
#[starlark_value(type = "list")]
impl<'v> StarlarkValue<'v> for InvocationTagList {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let index = index
            .unpack_i32()
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index must be int")))?;
        self.0
            .get(list_index(index, self.0.len())?)
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
    }
    fn length(&self) -> starlark::Result<i32> {
        Ok(self.0.len() as i32)
    }
    fn iterate_collect(&self, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self
            .0
            .iter()
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .collect())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTagSortKey(usize, usize);
impl fmt::Display for InvocationTagSortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<sort_key>")
    }
}
starlark::starlark_simple_value!(InvocationTagSortKey);
#[starlark_value(type = "sort_key")]
impl<'v> StarlarkValue<'v> for InvocationTagSortKey {
    fn compare(&self, other: Value<'v>) -> starlark::Result<Ordering> {
        let other = Self::from_value(other).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("sort keys can only compare with sort keys"))
        })?;
        Ok((self.0, self.1).cmp(&(other.0, other.1)))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    #[derive(Debug, Clone, Allocative)]
    pub(crate) struct InvokePreparedKey {
        pub(crate) workspace: NormalizedAbsolutePath,
        pub(crate) prepared: Arc<HostPreparedModuleExtensionInputs>,
        pub(crate) id: u64,
    }

    impl PartialEq for InvokePreparedKey {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for InvokePreparedKey {}
    impl std::hash::Hash for InvokePreparedKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }
    impl fmt::Display for InvokePreparedKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-invoke-prepared:{}", self.id)
        }
    }

    #[derive(Debug, Clone, Allocative)]
    pub(crate) struct InvokePreparedValue {
        pub(crate) outcome: HostPureModuleExtensionInvocationsOutcome,
        pub(crate) prints: Arc<[CompactString]>,
    }

    #[async_trait]
    impl Key for InvokePreparedKey {
        type Value = Arc<InvokePreparedValue>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            let mut events = EventBatch::empty();
            let observations = PathObservationEpoch::empty();
            let outcome = match preflight_pure_invocations(
                ctx,
                &self.workspace,
                &self.prepared,
                observations,
                PureInvocationsMode::Legacy,
            )
            .await
            {
                Ok((preflight, observations)) => invoke_pure_preflight(
                    self.prepared.clone(),
                    preflight,
                    observations,
                    true,
                    &mut events,
                ),
                Err(terminal) => terminal,
            };
            let outcome = match outcome {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                    debug_assert!(observations.observations().is_empty());
                    SourcePreparationOutcome::Complete(result)
                }
                SourcePreparationOutcome::Complete(Err(_)) => {
                    unreachable!("legacy prepared injection has no observed frontier")
                }
            };
            let prints = events
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .into();
            Arc::new(InvokePreparedValue { outcome, prints })
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.prints == y.prints
                && HostPureModuleExtensionInvocationsKey::equality(&x.outcome, &y.outcome)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
    pub(crate) struct InvocationConsumerKey {
        pub(crate) workspace: NormalizedAbsolutePath,
        pub(crate) id: u64,
    }
    impl fmt::Display for InvocationConsumerKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-invocation-consumer:{}", self.id)
        }
    }
    #[async_trait]
    impl Key for InvocationConsumerKey {
        type Value = HostPureModuleExtensionInvocationsOutcome;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            ctx.compute(&HostPureModuleExtensionInvocationsKey::new(
                self.workspace.clone(),
            ))
            .await
            .unwrap()
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochError;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark::environment::Globals;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::bzl_module::HostPreparedModuleExtensionInputsObservationError;
    use crate::bzl_module::HostPreparedModuleExtensionInputsObservationKey;
    use crate::bzl_module::ObservedHostPreparedModuleExtensionInputs;

    #[test]
    fn prepared_observation_surface_is_sibling_module_usable() {
        let key = HostPreparedModuleExtensionInputsObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-prepared-module-extension-inputs:\"/workspace\""
        );

        fn inspect(
            _value: &<HostPreparedModuleExtensionInputsObservationKey as Key>::Value,
            observed: &ObservedHostPreparedModuleExtensionInputs,
            _error: &HostPreparedModuleExtensionInputsObservationError,
        ) {
            let _: &Arc<
                Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }

        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostPreparedModuleExtensionInputs,
                        HostPreparedModuleExtensionInputsObservationError,
                    >,
                >,
                &ObservedHostPreparedModuleExtensionInputs,
                &HostPreparedModuleExtensionInputsObservationError,
            );
    }

    fn empty_module(_owner: &Arc<()>) -> InvocationModule {
        InvocationModule {
            name: "root".into(),
            version: "".into(),
            is_root: true,
            tag_classes: Arc::from([CompactString::from("tag")]),
            tags: Arc::from([]),
        }
    }

    fn tag(
        owner: &Arc<()>,
        index: usize,
        dev_dependency: bool,
        frozen_heap: &FrozenHeap,
    ) -> InvocationTag {
        InvocationTag::new(prepared_tag(index, dev_dependency), owner, frozen_heap)
    }

    fn prepared_tag(index: usize, dev_dependency: bool) -> PreparedModuleExtensionTag {
        PreparedModuleExtensionTag {
            tag_class: "tag".into(),
            attributes: Arc::from([(
                CompactString::from("value"),
                CoercedAttributeValue::String(format!("v{index}").into()),
            )]),
            dev_dependency,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: index as u32 + 1,
                start_column: 1,
                end_line: index as u32 + 1,
                end_column: 2,
            },
            module_index: 0,
            tag_index: index,
        }
    }

    fn call(
        source: &str,
        values: impl FnOnce(&Module) -> Vec<Value<'_>>,
    ) -> Result<String, String> {
        let module = Module::new();
        let ast = AstModule::parse("test.bzl", source.to_owned(), &Dialect::Standard).unwrap();
        let mut evaluator = Evaluator::new(&module);
        evaluator
            .eval_module(ast, &Globals::standard())
            .map_err(|error| error.to_string())?;
        let function = module.get("f").unwrap();
        let values = values(&module);
        evaluator
            .eval_function(function, &values, &[])
            .map(|value| value.to_repr())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn immutable_lists_and_foreign_tags_fail_closed() {
        let owner = Arc::new(());
        let context = InvocationContext {
            modules: Arc::from([empty_module(&owner)]),
            owner: owner.clone(),
        };
        let modules = call("def f(ctx):\n  ctx.modules.append(1)\n", |module| {
            vec![module.heap().alloc_simple(context.clone())]
        });
        assert!(modules.unwrap_err().contains("append"));

        let tag_lists = call(
            "def f(ctx):\n  ctx.modules[0].tags.tag.append(1)\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        );
        assert!(tag_lists.unwrap_err().contains("append"));

        let foreign = InvocationTag {
            class: "tag".into(),
            attributes: Arc::from([]),
            dev_dependency: true,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
            },
            module_index: 0,
            tag_index: 0,
            owner: Arc::new(()),
        };
        let error = call(
            "def f(ctx, tag):\n  return ctx.is_dev_dependency(tag)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context),
                    module.heap().alloc_simple(foreign),
                ]
            },
        )
        .unwrap_err();
        assert!(error.contains("another module_ctx"));

        let owner = Arc::new(());
        let tags: Arc<[InvocationTag]> = Arc::from([]);
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                tags,
                ..empty_module(&owner)
            }]),
            owner: owner.clone(),
        };
        let foreign_heap = FrozenHeap::new();
        let foreign = tag(&Arc::new(()), 0, false, &foreign_heap);
        let error = call(
            "def f(ctx, tag):\n  return ctx.tag_sort_key(tag)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context),
                    module.heap().alloc_simple(foreign),
                ]
            },
        )
        .unwrap_err();
        assert!(error.contains("another module_ctx"));
    }

    #[test]
    fn immutable_lists_support_exact_negative_indexing_and_tag_order() {
        let owner = Arc::new(());
        let frozen_heap = FrozenHeap::new();
        let tags: Arc<[InvocationTag]> = Arc::from([
            tag(&owner, 0, false, &frozen_heap),
            tag(&owner, 1, true, &frozen_heap),
        ]);
        let modules = InvocationModuleList(Arc::from([empty_module(&owner)]));
        let value = call(
            "def f(modules, tags):\n  return [modules[-1].name, tags[-1].value, tags[-2].value]\n",
            |module| {
                vec![
                    module.heap().alloc_simple(modules.clone()),
                    module.heap().alloc_simple(InvocationTagList(tags.clone())),
                ]
            },
        )
        .unwrap();
        assert_eq!(value, "[\"root\", \"v1\", \"v0\"]");
        for source in [
            "def f(modules, tags):\n  return modules[-2]\n",
            "def f(modules, tags):\n  return tags[-3]\n",
        ] {
            let error = call(source, |module| {
                vec![
                    module.heap().alloc_simple(modules.clone()),
                    module.heap().alloc_simple(InvocationTagList(tags.clone())),
                ]
            })
            .unwrap_err();
            assert!(error.contains("list index out of range"), "{error}");
        }
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                tags: Arc::from([
                    tag(&owner, 0, false, &frozen_heap),
                    tag(&owner, 1, true, &frozen_heap),
                ]),
                ..empty_module(&owner)
            }]),
            owner: owner.clone(),
        };
        let value = call(
            "def f(ctx):\n  tags=ctx.modules[0].tags.tag\n  return [len(tags), ctx.is_dev_dependency(tags[-1]), ctx.tag_sort_key(tags[0]) < ctx.tag_sort_key(tags[1])]\n",
            |module| vec![module.heap().alloc_simple(context)],
        )
        .unwrap();
        assert_eq!(value, "[2, True, True]");
    }

    #[test]
    fn forbidden_abi_and_cross_context_captured_tags_fail_closed() {
        let owner = Arc::new(());
        let frozen_heap = FrozenHeap::new();
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                tags: Arc::from([tag(&owner, 0, false, &frozen_heap)]),
                ..empty_module(&owner)
            }]),
            owner,
        };
        for name in [
            "facts",
            "is_isolated",
            "root_module_has_non_dev_dependency",
            "extension_metadata",
            "wait",
            "download",
            "download_and_extract",
            "extract",
            "file",
            "getenv",
            "path",
            "read",
            "watch",
            "report_progress",
            "os",
            "execute",
            "load_wasm",
            "execute_wasm",
            "which",
        ] {
            let source = format!("def f(ctx):\n  return ctx.{name}\n");
            assert!(
                call(&source, |module| vec![
                    module.heap().alloc_simple(context.clone())
                ])
                .unwrap_err()
                .contains("has no attribute")
            );
        }
        for source in [
            "def f(ctx):\n  return ctx.modules[0].missing\n",
            "def f(ctx):\n  return ctx.modules[0].tags.missing\n",
            "def f(ctx):\n  return ctx.modules[0].tags.tag[0].missing\n",
        ] {
            assert!(
                call(source, |module| vec![
                    module.heap().alloc_simple(context.clone())
                ])
                .is_err()
            );
        }
        let other_owner = Arc::new(());
        let other_frozen_heap = FrozenHeap::new();
        let other = InvocationContext {
            modules: Arc::from([InvocationModule {
                tags: Arc::from([tag(&other_owner, 0, true, &other_frozen_heap)]),
                ..empty_module(&other_owner)
            }]),
            owner: other_owner,
        };
        for method in ["is_dev_dependency", "tag_sort_key"] {
            let source = format!(
                "def f(left, right):\n  tag=right.modules[0].tags.tag[0]\n  return left.{method}(tag)\n"
            );
            let error = call(&source, |module| {
                vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(other.clone()),
                ]
            })
            .unwrap_err();
            assert!(error.contains("another module_ctx"), "{error}");
        }
        let label = StarlarkLabel::new(CanonicalLabel::parse("@@//pkg:item").unwrap());
        for name in ["workspace_root", "relative"] {
            let source = format!("def f(label):\n  return label.{name}\n");
            assert!(
                call(&source, |module| vec![
                    module.heap().alloc_simple(label.clone())
                ])
                .is_err()
            );
        }
    }

    #[test]
    fn scalar_none_label_and_exact_read_only_abi_are_visible() {
        let module = Module::new();
        assert!(
            allocate_frozen_attribute(&CoercedAttributeValue::None, module.frozen_heap()).is_none()
        );
        let owner = Arc::new(());
        let label = CanonicalLabel::parse("@@dep+//pkg:item").unwrap();
        let tag = InvocationTag::new(
            PreparedModuleExtensionTag {
                tag_class: "tag".into(),
                attributes: Arc::from([(
                    CompactString::from("target"),
                    CoercedAttributeValue::Label(label),
                )]),
                dev_dependency: true,
                location: slug_bzlmod_v2::LogicalSpan {
                    file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                    start_line: 3,
                    start_column: 2,
                    end_line: 3,
                    end_column: 4,
                },
                module_index: 0,
                tag_index: 0,
            },
            &owner,
            module.frozen_heap(),
        );
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                name: "root".into(),
                version: "".into(),
                is_root: true,
                tag_classes: Arc::from([CompactString::from("tag")]),
                tags: Arc::from([]),
            }]),
            owner,
        };
        let value = call("def f(ctx, tag):\n  label=tag.target\n  return [len(ctx.modules), ctx.modules[0].name, ctx.modules[0].version, ctx.modules[0].is_root, ctx.is_dev_dependency(tag), label.name, label.package, label.repo_name, label.workspace_name, str(label), repr(label), '%s' % label, '%r' % label, '{}'.format(label), '{!s}'.format(label), '{!r}'.format(label), label.same_package_label('other').name, {label: 1}[label], label == label]\n", |module| vec![module.heap().alloc_simple(context), module.heap().alloc_simple(tag)]).unwrap();
        assert_eq!(
            value,
            "[1, \"root\", \"\", True, True, \"item\", \"pkg\", \"dep+\", \"dep+\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"@@dep+//pkg:item\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"other\", 1, True]"
        );
    }

    #[test]
    fn complete_tag_values_are_frozen_ordered_and_publicly_named() {
        fn complete_tag() -> PreparedModuleExtensionTag {
            let local = CanonicalLabel::parse("@@//:local").unwrap();
            let dep = CanonicalLabel::parse("@@dep+//pkg:item").unwrap();
            PreparedModuleExtensionTag {
                tag_class: "tag".into(),
                attributes: Arc::from([
                    ("_private".into(), CoercedAttributeValue::Boolean(true)),
                    ("count".into(), CoercedAttributeValue::Integer(7)),
                    (
                        "ints".into(),
                        CoercedAttributeValue::IntegerList(Arc::from([1, -2])),
                    ),
                    ("text".into(), CoercedAttributeValue::String("value".into())),
                    (
                        "strings".into(),
                        CoercedAttributeValue::StringList(Arc::from(["one".into(), "two".into()])),
                    ),
                    (
                        "string_dict".into(),
                        CoercedAttributeValue::StringDict(Arc::from([("a".into(), "one".into())])),
                    ),
                    (
                        "string_lists".into(),
                        CoercedAttributeValue::StringListDict(Arc::from([(
                            "a".into(),
                            Arc::from(["one".into(), "two".into()]),
                        )])),
                    ),
                    ("label".into(), CoercedAttributeValue::Label(dep.clone())),
                    (
                        "labels".into(),
                        CoercedAttributeValue::LabelList(Arc::from([local.clone(), dep.clone()])),
                    ),
                    (
                        "labels_by_string".into(),
                        CoercedAttributeValue::StringKeyedLabelDict(Arc::from([(
                            "a".into(),
                            dep.clone(),
                        )])),
                    ),
                    (
                        "strings_by_label".into(),
                        CoercedAttributeValue::LabelKeyedStringDict(Arc::from([(
                            dep.clone(),
                            "value".into(),
                        )])),
                    ),
                    (
                        "label_lists".into(),
                        CoercedAttributeValue::LabelListDict(Arc::from([(
                            "a".into(),
                            Arc::from([local.clone(), dep.clone()]),
                        )])),
                    ),
                    (
                        "output".into(),
                        CoercedAttributeValue::Output(local.clone()),
                    ),
                    (
                        "outputs".into(),
                        CoercedAttributeValue::OutputList(Arc::from([local])),
                    ),
                ]),
                dev_dependency: false,
                location: slug_bzlmod_v2::LogicalSpan {
                    file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 2,
                },
                module_index: 0,
                tag_index: 0,
            }
        }

        let owner = Arc::new(());
        let value = call(
            "def f(tag):\n  return [tag._private, tag.count, tag.ints, tag.strings, tag.string_dict, tag.string_lists, str(tag.label), [str(x) for x in tag.labels], {k: str(v) for k, v in tag.labels_by_string.items()}, {str(k): v for k, v in tag.strings_by_label.items()}, {k: [str(x) for x in v] for k, v in tag.label_lists.items()}, str(tag.output), [str(x) for x in tag.outputs], '_private' in dir(tag), dir(tag)[0]]\n",
            |module| {
                vec![module.heap().alloc_simple(InvocationTag::new(
                    complete_tag(),
                    &owner,
                    module.frozen_heap(),
                ))]
            },
        )
        .unwrap();
        assert!(value.contains("[1, -2]"), "{value}");
        assert!(value.contains("@@dep+//pkg:item"), "{value}");
        assert!(value.contains("True, \"_private\""), "{value}");

        for source in [
            "def f(tag):\n  tag.ints.append(3)\n",
            "def f(tag):\n  tag.string_dict.clear()\n",
            "def f(tag):\n  tag.string_lists['a'].append('three')\n",
            "def f(tag):\n  tag.label_lists['a'].append(tag.label)\n",
        ] {
            let owner = owner.clone();
            assert!(
                call(source, |module| vec![module.heap().alloc_simple(
                    InvocationTag::new(complete_tag(), &owner, module.frozen_heap())
                )])
                .is_err(),
                "frozen tag collection accepted mutation: {source}"
            );
        }
    }

    const REPO_RULE_WORKSPACE: &str = "/module-extension-repository-rule";

    #[derive(Debug)]
    struct RepositoryRuleActivation {
        key: String,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct RepositoryRuleActivationTracker {
        activations: Mutex<Vec<RepositoryRuleActivation>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl RepositoryRuleActivationTracker {
        fn take(&self) -> Vec<RepositoryRuleActivation> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }

        fn take_dependencies(&self) -> Vec<(String, Vec<String>)> {
            std::mem::take(&mut *self.dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for RepositoryRuleActivationTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.activations
                .lock()
                .unwrap()
                .push(RepositoryRuleActivation {
                    key: key.to_string(),
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
        }
    }

    fn repository_rule_epoch(
        module_source: &str,
        sources: &[(&str, Option<&str>)],
        metadata_bias: i64,
    ) -> PathObservationEpoch {
        let path = |name: &str| {
            NormalizedAbsolutePath::new(if name.starts_with('/') {
                name.to_owned()
            } else {
                format!("{REPO_RULE_WORKSPACE}/{name}")
            })
            .unwrap()
        };
        let demand = |name, operation| {
            PathObservationDemand::new(PathObservationNamespace::Host, path(name), operation)
        };
        let lstat = |kind, id| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind,
                id + metadata_bias,
                1,
                1,
                1,
                0o755,
            )))
        };
        let mut observations = vec![
            (
                demand("/", PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, 1),
            ),
            (
                demand(REPO_RULE_WORKSPACE, PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory, 2),
            ),
            (
                demand("REPO.bazel", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand(".bazelignore", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand("BUILD", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand("MODULE.bazel.lock", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
            (
                demand("MODULE.bazel", PathObservationOperation::Lstat),
                lstat(PathNodeKind::RegularFile, 9),
            ),
            (
                demand("MODULE.bazel", PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    module_source.as_bytes(),
                ))),
            ),
            (
                demand("BUILD.bazel", PathObservationOperation::Lstat),
                lstat(PathNodeKind::RegularFile, 10),
            ),
        ];
        for (index, (name, source)) in sources.iter().enumerate() {
            observations.push((
                demand(name, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(source.map_or(PathOperationResult::Missing, |_| {
                    PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        11 + index as i64 + metadata_bias,
                        1,
                        1,
                        1,
                        0o644,
                    ))
                })),
            ));
            observations.push((
                demand(name, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(
                    source.map_or(PathOperationResult::Missing, |source| {
                        PathOperationResult::Present(Arc::from(source.as_bytes()))
                    }),
                ),
            ));
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn repository_rule_sources_transaction(
        dice: &Arc<Dice>,
        module_source: &str,
        sources: &[(&str, Option<&str>)],
        metadata_bias: i64,
        tracker: Option<Arc<RepositoryRuleActivationTracker>>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let mut user_data = UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter(
                        [(
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module_source.to_owned())),
                        )]
                        .into_iter()
                        .chain(sources.iter().map(|(name, source)| {
                            (
                                workspace.as_path().join(name),
                                source.map_or(WorkspaceFileValue::Absent, |source| {
                                    WorkspaceFileValue::Present(Arc::new(source.to_owned()))
                                }),
                            )
                        }))
                        .chain(std::iter::once((
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ))),
                    )),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                repository_rule_epoch(module_source, sources, metadata_bias),
            )])
            .unwrap();
        updater.commit().await
    }

    async fn repository_rule_transaction(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        metadata_bias: i64,
        tracker: Option<Arc<RepositoryRuleActivationTracker>>,
    ) -> dice::DiceTransaction {
        repository_rule_sources_transaction(
            dice,
            module_source,
            &[("ext.bzl", extension_present.then_some(extension_source))],
            metadata_bias,
            tracker,
        )
        .await
    }

    async fn compute_repository_rule_case(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        tracker: Option<Arc<RepositoryRuleActivationTracker>>,
    ) -> HostPureModuleExtensionInvocationsOutcome {
        repository_rule_transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            0,
            tracker,
        )
        .await
        .compute(&HostPureModuleExtensionInvocationsKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        ))
        .await
        .unwrap()
    }

    async fn compute_observed_repository_rule_case(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        tracker: Option<Arc<RepositoryRuleActivationTracker>>,
    ) -> <HostPureModuleExtensionInvocationsObservationKey as Key>::Value {
        repository_rule_transaction(
            dice,
            module_source,
            extension_source,
            extension_present,
            0,
            tracker,
        )
        .await
        .compute(&HostPureModuleExtensionInvocationsObservationKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
        ))
        .await
        .unwrap()
    }

    fn observed_pure_carrier(
        outcome: &<HostPureModuleExtensionInvocationsObservationKey as Key>::Value,
    ) -> &ObservedHostPureModuleExtensionInvocations {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed pure carrier: {value:?}"),
        }
    }

    #[derive(Debug, Clone, Allocative)]
    #[rustfmt::skip]
    struct ObservePreparedKey {
        workspace: NormalizedAbsolutePath,
        prepared: Arc<HostPreparedModuleExtensionInputs>,
        observations: PathObservationEpoch,
        id: u64,
    }
    #[rustfmt::skip]
    impl PartialEq for ObservePreparedKey { fn eq(&self, other: &Self) -> bool { self.id == other.id } }
    impl Eq for ObservePreparedKey {}
    #[rustfmt::skip]
    impl Hash for ObservePreparedKey { fn hash<H: Hasher>(&self, state: &mut H) { self.id.hash(state); } }
    #[rustfmt::skip]
    impl fmt::Display for ObservePreparedKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "test-observe-prepared:{}", self.id) } }
    #[async_trait]
    #[rustfmt::skip]
    impl Key for ObservePreparedKey {
        type Value = <HostPureModuleExtensionInvocationsObservationKey as Key>::Value;
        async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
            let mut events = EventBatch::empty();
            let outcome = match preflight_pure_invocations(ctx, &self.workspace, &self.prepared, self.observations.dupe(), PureInvocationsMode::Observed).await {
                Ok((preflight, observations)) => invoke_pure_preflight(self.prepared.clone(), preflight, observations, true, &mut events),
                Err(terminal) => terminal,
            };
            match outcome {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                    Err(HostPureModuleExtensionInvocationsObservationError(error)),
                ),
                SourcePreparationOutcome::Complete(Ok((result, observations))) => SourcePreparationOutcome::Complete(Ok(ObservedHostPureModuleExtensionInvocations { result, observations })),
            }
        }
        fn equality(x: &Self::Value, y: &Self::Value) -> bool { x.complete_eq(y) }
        fn validity(value: &Self::Value) -> bool { value.is_complete() }
    }

    #[rustfmt::skip]
    fn assert_observed_pure_outer_stages(
        prepared: &Arc<HostPreparedModuleExtensionInputs>,
        lower_error: ObservedPathFrontierError,
        merge_error: ObservedPathFrontierError,
    ) {
        let host_outer: <HostPureModuleExtensionInvocationsObservationKey as Key>::Value = SourcePreparationOutcome::Complete(Err(HostPureModuleExtensionInvocationsObservationError(PureModuleExtensionInvocationsObservationError::HostBzl {
            prepared: prepared.dupe(), index: 0, error: lower_error,
        })));
        let merge_outer: <HostPureModuleExtensionInvocationsObservationKey as Key>::Value = SourcePreparationOutcome::Complete(Err(HostPureModuleExtensionInvocationsObservationError(PureModuleExtensionInvocationsObservationError::Merge {
            prepared: prepared.dupe(), index: 0, error: merge_error,
        })));
        assert!(HostPureModuleExtensionInvocationsObservationKey::validity(&host_outer));
        assert!(HostPureModuleExtensionInvocationsObservationKey::equality(&host_outer, &host_outer));
        assert!(matches!(host_outer, SourcePreparationOutcome::Complete(Err(
            HostPureModuleExtensionInvocationsObservationError(PureModuleExtensionInvocationsObservationError::HostBzl { .. })
        ))));
        assert!(matches!(merge_outer, SourcePreparationOutcome::Complete(Err(
            HostPureModuleExtensionInvocationsObservationError(PureModuleExtensionInvocationsObservationError::Merge { .. })
        ))));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_pure_identity_finisher_and_prefix_algebra() {
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe());
        let same = HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe());
        let other = HostPureModuleExtensionInvocationsObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        assert_eq!(
            key.to_string(),
            "observed-host-pure-module-extension-invocations:\"/module-extension-repository-rule\""
        );
        let hash = |key: &HostPureModuleExtensionInvocationsObservationKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let source = "def impl(ctx):\n    return None\next=module_extension(implementation=impl)\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let observed =
            compute_observed_repository_rule_case(&dice, module, source, true, None).await;
        let carrier = observed_pure_carrier(&observed);
        assert!(carrier.result().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostPureModuleExtensionInvocationsObservationKey::validity(
            &observed
        ));
        assert!(HostPureModuleExtensionInvocationsObservationKey::equality(
            &observed, &observed
        ));

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            workspace,
            PathObservationOperation::FileBytes,
        );
        let first = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(&b"first"[..])),
        ));
        let second = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(&b"second"[..])),
        ));
        let left = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let duplicate = PathObservationEpoch::from_shared([(demand.dupe(), first.dupe())]).unwrap();
        let merged = union_pure_observations(&left, &duplicate).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
        let conflicting = PathObservationEpoch::from_shared([(demand.dupe(), second)]).unwrap();
        let merge_error = union_pure_observations(&left, &conflicting).unwrap_err();

        let prepared = match carrier.result().as_ref() {
            Ok(value) => value.prepared.dupe(),
            Err(error) => panic!("expected success: {error:?}"),
        };
        let lower_error =
            ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::Lstat,
            });
        assert_observed_pure_outer_stages(&prepared, lower_error, merge_error);
        let bzl_source = include_str!("bzl_module.rs");
        let selected_source = include_str!("../../slug_bzlmod_v2/src/selected_repo_spec.rs");
        let pure_source = include_str!("module_extension.rs");
        assert!(bzl_source.contains("RootPackageBzlTarget::parse(label.target().as_str())"));
        assert!(pure_source.contains("RootPackageBzlTarget::parse(request.target().as_str())"));
        assert_eq!(selected_source.matches("Ok(HostSelectedExtensionEvaluationInput {").count(), 1);
        assert!(selected_source.contains("pub struct HostSelectedExtensionEvaluationInput {\n    load_request:"));
        assert!(bzl_source.contains("if request != &loaded.request"));
        assert_eq!(&prepared.inputs[0].input, &prepared.raw.parts().1[0]);
        assert!(RootPackageBzlTarget::parse(prepared.inputs[0].input.parts().0.parts().0.target().as_str()).is_ok());

        let need_module = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n";
        let need = compute_observed_repository_rule_case(
            &Dice::builder().build(DetectCycles::Enabled),
            need_module,
            source,
            true,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostPureModuleExtensionInvocationsObservationKey::validity(
            &need
        ));
        assert!(!HostPureModuleExtensionInvocationsObservationKey::equality(
            &need, &need
        ));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_pure_real_order_terminals_events_and_parity() {
        let module = "module(name='bazel_tools')\na=use_extension('//:first.bzl','first')\nb=use_extension('//:second.bzl','second')\n";
        let first = "print('load-first')\ndef impl(ctx):\n    print('invoke-first')\nfirst=module_extension(implementation=impl)\n";
        let second = "print('load-second')\ndef impl(ctx):\n    print('invoke-second')\nsecond=module_extension(implementation=impl)\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let mut transaction = repository_rule_sources_transaction(&dice, module, &[("first.bzl", Some(first)), ("second.bzl", Some(second))], 0, Some(tracker.clone())).await;
        let legacy = transaction.compute(&HostPureModuleExtensionInvocationsKey::new(workspace.dupe())).await.unwrap();
        let observed = transaction.compute(&HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe())).await.unwrap();
        let prepared = transaction.compute(&HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe())).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(prepared)) = prepared else { panic!("prepared observation must complete") };
        let prepared_value = Arc::new(prepared.result().as_ref().as_ref().unwrap().clone());
        let SourcePreparationOutcome::Complete(legacy_result) = legacy else { panic!("legacy pure invocation must complete") };
        assert_eq!(legacy_result.as_ref(), observed_pure_carrier(&observed).result().as_ref());
        let rows = tracker.take();
        let dependencies = tracker.take_dependencies();
        let observed_parent_dependencies = &dependencies.iter().find(|(name, _)| name.starts_with("observed-host-pure-module-extension-invocations:")).unwrap().1;
        assert_eq!(observed_parent_dependencies, &["observed-host-prepared-module-extension-inputs:\"/module-extension-repository-rule\"", "observed-bzlmod-host-bzl-module:\"/module-extension-repository-rule\"://:first.bzl", "observed-bzlmod-host-bzl-module:\"/module-extension-repository-rule\"://:second.bzl"]);
        let parent = rows.iter().find(|row| row.key.starts_with("observed-host-pure-module-extension-invocations:")).unwrap();
        let prints = parent.batch.as_ref().unwrap().events().iter().filter_map(|event| match event { EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()), _ => None }).collect::<Vec<_>>();
        assert_eq!(prints, ["invoke-first", "invoke-second"]);
        let children = rows.iter().enumerate().filter(|(_, row)| row.key.starts_with("observed-bzlmod-host-bzl-module:")).collect::<Vec<_>>();
        assert_eq!(children.iter().filter_map(|(_, row)| row.batch.as_ref()).flat_map(|batch| batch.events()).filter_map(|event| match event { EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()), _ => None }).collect::<Vec<_>>(), ["load-first", "load-second"]);
        assert_eq!(children.iter().map(|(_, row)| (row.key.contains("first.bzl"), row.kind)).collect::<Vec<_>>(), [(true, ActivationKind::Evaluated), (false, ActivationKind::Evaluated), (true, ActivationKind::Reused), (false, ActivationKind::Reused)]);
        assert!(children.windows(2).all(|pair| pair[0].1.key != pair[1].1.key || pair[1].1.kind == ActivationKind::Reused));
        assert!(children.iter().any(|(_, row)| row.kind == ActivationKind::Reused && row.batch.is_none()));
        let child_index = children[0].0;
        let parent_index = rows.iter().position(|row| row.key.starts_with("observed-host-pure-module-extension-invocations:")).unwrap();
        assert!(child_index < parent_index);
        assert!(children.iter().all(|(index, _)| *index < parent_index));

        for (id, replacement, terminal) in [(1, second.replace("invoke-second", "drifted").replace("print('load-second')", "print('changed-load')"), "drift"), (2, "this is not valid Starlark".to_owned(), "bzl")] {
            let injected_tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let mut injected = repository_rule_sources_transaction(&dice, module, &[("first.bzl", Some(first)), ("second.bzl", Some(&replacement))], 0, Some(injected_tracker.clone())).await;
            let observed_injected = injected.compute(&ObservePreparedKey { workspace: workspace.dupe(), prepared: prepared_value.dupe(), observations: PathObservationEpoch::empty(), id }).await.unwrap();
            let legacy_injected = injected.compute(&test_support::InvokePreparedKey { workspace: workspace.dupe(), prepared: prepared_value.dupe(), id: id + 10 }).await.unwrap();
            let observed_result = observed_pure_carrier(&observed_injected).result();
            let SourcePreparationOutcome::Complete(legacy_result) = &legacy_injected.outcome else { panic!("legacy injection must complete") };
            assert_eq!(legacy_result, observed_result);
            assert!(legacy_injected.prints.is_empty());
            assert!(matches!((terminal, observed_result.as_ref()), ("drift", Err(HostPureModuleExtensionInvocationsError::AfterPrepared { error: HostPureModuleExtensionInvocationError::Drift(_), .. })) | ("bzl", Err(HostPureModuleExtensionInvocationsError::AfterPrepared { error: HostPureModuleExtensionInvocationError::Bzl(_), .. }))));
            assert!(injected_tracker.take().iter().find(|row| row.key.starts_with("test-observe-prepared:")).is_some_and(|row| row.batch.is_none()));
        }

        for (source, expected) in [("def impl(ctx):\n    fail('boom')\next=module_extension(implementation=impl)\n", "invocation"), ("def impl(ctx):\n    return 1\next=module_extension(implementation=impl)\n", "result"), ("def impl(ctx):\n    print('must-not-run')\next=module_extension(implementation=impl,os_dependent=True)\n", "factor")] {
            let one = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
            let terminal_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let terminal_tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let outcome = compute_observed_repository_rule_case(&terminal_dice, one, source, true, Some(terminal_tracker.clone())).await;
            let legacy = compute_repository_rule_case(&terminal_dice, one, source, true, None).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy else { panic!("legacy failure must complete") };
            assert_eq!(legacy, *observed_pure_carrier(&outcome).result());
            let error = observed_pure_carrier(&outcome).result().as_ref().as_ref().unwrap_err();
            assert!(matches!((expected, error), ("invocation", HostPureModuleExtensionInvocationsError::AfterPrepared { error: HostPureModuleExtensionInvocationError::Invocation(_), .. }) | ("result", HostPureModuleExtensionInvocationsError::AfterPrepared { error: HostPureModuleExtensionInvocationError::Result(_), .. }) | ("factor", HostPureModuleExtensionInvocationsError::AfterPrepared { error: HostPureModuleExtensionInvocationError::UnsupportedFactors, .. })));
            if expected == "factor" { let terminal_rows = terminal_tracker.take(); assert!(terminal_rows.iter().find(|row| row.key.starts_with("observed-host-pure-module-extension-invocations:")).is_some_and(|row| row.batch.as_ref().is_some_and(|batch| batch.events().is_empty()))); }
        }

        let prepared_tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let prepared_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut prepared_tx = repository_rule_transaction(&prepared_dice, "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n", first, false, 0, Some(prepared_tracker.clone())).await;
        let prepared_legacy = prepared_tx.compute(&HostPureModuleExtensionInvocationsKey::new(workspace.dupe())).await.unwrap();
        let prepared_observed = prepared_tx.compute(&HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe())).await.unwrap();
        let SourcePreparationOutcome::Complete(prepared_legacy) = prepared_legacy else { panic!("legacy prepared failure must complete") };
        assert_eq!(&prepared_legacy, observed_pure_carrier(&prepared_observed).result());
        assert!(matches!(prepared_legacy.as_ref(), Err(HostPureModuleExtensionInvocationsError::Prepared(_))));
        let prepared_rows = prepared_tracker.take();
        assert_eq!(prepared_rows.iter().filter(|row| row.key.contains("host-pure-module-extension-invocations:")).count(), 2);
        assert!(prepared_rows.iter().filter(|row| row.key.contains("host-pure-module-extension-invocations:")).all(|row| row.batch.is_none()));
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_pure_lifecycle_cancellation_and_nonactivation() {
        let module_a = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\ne.tag(value='tag-a')\n";
        let module_b = module_a.replace("tag-a", "tag-b");
        let source_a = "tag=tag_class(attrs={'value':attr.string()})\ndef impl(ctx):\n    print(ctx.modules[0].tags.tag[0].value + '-source-a')\next=module_extension(implementation=impl,tag_classes={'tag':tag})\n";
        let source_b = source_a.replace("source-a", "source-b");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe());
        let mut held = Vec::new();
        for (module, source, bias) in [
            (module_a, source_a, 0),
            (module_b.as_str(), source_a, 0),
            (module_a, source_a, 0),
            (module_a, source_b.as_str(), 0),
            (module_a, source_a, 0),
            (module_a, source_a, 100),
        ] {
            let mut transaction =
                repository_rule_transaction(&dice, module, source, true, bias, None).await;
            let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
            let value = transaction.compute(&key).await.unwrap();
            let carrier = observed_pure_carrier(&value).dupe();
            let prepared = transaction.compute(&HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe())).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(prepared)) = prepared else { panic!("prepared observation must complete") };
            let prepared_value = prepared.result().as_ref().as_ref().unwrap();
            let (request, _, _, _) = prepared_value.inputs[0].input.parts().0.parts();
            let label = HostRootBzlLabel::new(request.package().package().clone(), RootPackageBzlTarget::parse(request.target().as_str()).unwrap());
            let child = transaction.compute(&HostBzlModuleObservationKey::new(workspace.dupe(), label)).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else { panic!("Host-Bzl observation must complete") };
            for epoch in [carrier.observations(), prepared.observations(), child.observations()] {
                for (demand, result) in epoch.observations() { assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref()); }
            }
            held.push((carrier, prepared, child, global));
        }
        assert_ne!(held[0].0.result(), held[1].0.result());
        assert_ne!(held[0].1.result(), held[1].1.result());
        assert_eq!(held[0].2.result(), held[1].2.result());
        assert_eq!(held[0].0.result(), held[2].0.result());
        assert_eq!(held[0].1.result(), held[2].1.result());
        assert_eq!(held[0].2.result(), held[2].2.result());
        assert_ne!(held[2].0.result(), held[3].0.result());
        assert_ne!(held[2].1.result(), held[3].1.result());
        assert_ne!(held[2].2.result(), held[3].2.result());
        assert_eq!(held[2].0.result(), held[4].0.result());
        assert_eq!(held[2].1.result(), held[4].1.result());
        assert_eq!(held[2].2.result(), held[4].2.result());
        assert_eq!(held[0].0.result(), held[5].0.result());
        assert_eq!(held[0].1.result(), held[5].1.result());
        assert_eq!(held[0].2.result(), held[5].2.result());
        assert_ne!(held[0].0.observations(), held[5].0.observations());
        assert_ne!(held[0].1.observations(), held[5].1.observations());
        assert_ne!(held[0].2.observations(), held[5].2.observations());

        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let mut warm =
            repository_rule_transaction(&dice, module_a, source_a, true, 0, Some(tracker.clone()))
                .await;
        let first = observed_pure_carrier(&warm.compute(&key).await.unwrap()).dupe();
        tracker.take();
        tracker.take_dependencies();
        let repeated = observed_pure_carrier(&warm.compute(&key).await.unwrap()).dupe();
        assert!(Arc::ptr_eq(first.result(), repeated.result()));
        let warm_rows = tracker.take();
        assert!(warm_rows.iter().any(|row| row.key.starts_with("observed-host-pure-module-extension-invocations:") && row.kind == ActivationKind::Reused && row.batch.is_none()));
        assert!(warm_rows.iter().all(|row| row.batch.is_none()));

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let mut cancelled = repository_rule_transaction(
            &cancel_dice,
            module_a,
            source_a,
            true,
            0,
            Some(cancel_tracker.clone()),
        )
        .await;
        let cancel_key = HostPureModuleExtensionInvocationsObservationKey::new(workspace.dupe());
        let mut future = Box::pin(cancelled.compute(&cancel_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(cancel_tracker.take().iter().all(|row| !row.key.starts_with("observed-host-pure-module-extension-invocations:")));
        assert!(cancel_tracker.take_dependencies().iter().all(|(name, _)| !name.starts_with("observed-host-pure-module-extension-invocations:")));
        let mut recovery = repository_rule_transaction(&cancel_dice, module_a, source_a, true, 0, Some(cancel_tracker)).await;
        let own_global = recovery.compute(&PathObservationEpochKey).await.unwrap();
        let recovered = recovery.compute(&cancel_key).await.unwrap();
        let recovered = observed_pure_carrier(&recovered);
        assert!(recovered.result().is_ok());
        let recovery_prepared = recovery.compute(&HostPreparedModuleExtensionInputsObservationKey::new(workspace.dupe())).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(recovery_prepared)) = recovery_prepared else { panic!("recovery prepared must complete") };
        let recovery_input = recovery_prepared.result().as_ref().as_ref().unwrap();
        let (request, _, _, _) = recovery_input.inputs[0].input.parts().0.parts();
        let recovery_label = HostRootBzlLabel::new(request.package().package().clone(), RootPackageBzlTarget::parse(request.target().as_str()).unwrap());
        let recovery_child = recovery.compute(&HostBzlModuleObservationKey::new(workspace.dupe(), recovery_label)).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(recovery_child)) = recovery_child else { panic!("recovery Host-Bzl must complete") };
        for epoch in [recovered.observations(), recovery_prepared.observations(), recovery_child.observations()] {
            for (demand, result) in epoch.observations() { assert_eq!(result.as_ref(), own_global.get(demand).unwrap().as_ref()); }
        }
        for epoch in [recovery_prepared.observations(), recovery_child.observations()] {
            for (demand, result) in epoch.observations() { assert_eq!(result.as_ref(), recovered.observations().get(demand).unwrap().as_ref()); }
        }

        let rows = tracker.take_dependencies();
        for forbidden in [
            "host-prepared-module-extension-inputs:",
            "host-bzl-module:",
            "host-pure-module-extension-invocations:",
            "host-instantiated-module-extension-repositories:",
            "host-validated-module-extension-repositories:",
            "host-root-repository-mapping:",
            "host-canonical-selected-module-definition:",
            "host-generated-repository-definition:",
            "slug-command:",
        ] {
            assert!(warm_rows.iter().map(|row| row.key.as_str()).chain(rows.iter().flat_map(|(name, dependencies)| std::iter::once(name.as_str()).chain(dependencies.iter().map(String::as_str)))).all(|name| !name.starts_with(forbidden)));
        }

        let legacy_tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let _ = compute_repository_rule_case(
            &Dice::builder().build(DetectCycles::Enabled),
            module_a,
            source_a,
            true,
            Some(legacy_tracker.clone()),
        )
        .await;
        let legacy_activations = legacy_tracker.take();
        let legacy_rows = legacy_tracker.take_dependencies();
        assert!(legacy_activations.iter().map(|row| row.key.as_str()).chain(legacy_rows.iter().flat_map(|(name, dependencies)| std::iter::once(name.as_str()).chain(dependencies.iter().map(String::as_str)))).all(|name| !name.starts_with("observed-")));
    }

    #[tokio::test]
    async fn real_repository_rule_calls_retain_order_prefix_and_reuse() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let source =
            |rule: &str, schema: &str, name: &str, first: &str, second: &str, value: &str| {
                format!(
                    "{rule}=repository_rule(lambda ctx: None, attrs={{'value':{schema}}})\n\
             def impl(ctx):\n    {rule}(name='{name}', {first}='{value}', {second}=True)\n\
             ext=module_extension(implementation=impl)\n"
                )
            };
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let a = compute_repository_rule_case(
            &dice,
            module,
            &source("repo", "attr.string()", "one", "raw", "enabled", "A"),
            true,
            Some(tracker.clone()),
        )
        .await;
        let warm = compute_repository_rule_case(
            &dice,
            module,
            &source("repo", "attr.string()", "one", "raw", "enabled", "A"),
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(HostPureModuleExtensionInvocationsKey::equality(&a, &warm));
        let activations = tracker.take();
        assert!(activations.iter().any(|row| {
            row.key
                .starts_with("host-pure-module-extension-invocations:")
                && row.kind == ActivationKind::Evaluated
                && row.batch.is_some()
        }));
        assert!(activations.iter().any(|row| {
            row.key
                .starts_with("host-pure-module-extension-invocations:")
                && row.kind == ActivationKind::Reused
                && row.batch.is_none()
        }));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("repository-rule invocation must complete")
        };
        let calls = &a_value.as_ref().as_ref().unwrap().invoked[0].repository_rule_calls;
        assert_eq!(
            calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            ["one"]
        );
        assert_eq!(
            calls[0]
                .kwargs
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["name", "raw", "enabled"]
        );
        for changed in [
            source("other", "attr.string()", "one", "raw", "enabled", "A"),
            source(
                "repo",
                "attr.string(mandatory=True)",
                "one",
                "raw",
                "enabled",
                "A",
            ),
            source("repo", "attr.string()", "renamed", "raw", "enabled", "A"),
            source("repo", "attr.string()", "one", "enabled", "raw", "A"),
            source("repo", "attr.string()", "one", "raw", "enabled", "B"),
            source("repo", "attr.string()", "one", "raw", "enabled", "A")
                .replace("    repo", "\n    repo"),
        ] {
            let b = compute_repository_rule_case(&dice, module, &changed, true, None).await;
            assert!(!HostPureModuleExtensionInvocationsKey::equality(&a, &b));
            let restored = compute_repository_rule_case(
                &dice,
                module,
                &source("repo", "attr.string()", "one", "raw", "enabled", "A"),
                true,
                None,
            )
            .await;
            assert!(HostPureModuleExtensionInvocationsKey::equality(
                &a, &restored
            ));
        }

        let missing = compute_repository_rule_case(
            &dice,
            module,
            &source("repo", "attr.string()", "one", "raw", "enabled", "A"),
            false,
            None,
        )
        .await;
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostPureModuleExtensionInvocationsError::Prepared(_)))
        ));
    }

    #[tokio::test]
    async fn repository_rule_declaration_metadata_reloads_and_restores_a_b_a() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n";
        let source = |local: bool, configure: bool, environment: &str| {
            format!(
                "repo=repository_rule(lambda ctx: None, local={local}, configure={configure}, environ={environment})\n\
                 def impl(ctx):\n    repo(name='generated')\n\
                 ext=module_extension(implementation=impl)\n",
                local = if local { "True" } else { "False" },
                configure = if configure { "True" } else { "False" },
            )
        };
        let a_source = source(true, true, "['B','A','B']");
        let a = compute_repository_rule_case(&dice, module, &a_source, true, None).await;
        let warm = compute_repository_rule_case(&dice, module, &a_source, true, None).await;
        assert!(HostPureModuleExtensionInvocationsKey::equality(&a, &warm));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("repository-rule invocation must complete")
        };
        let definition =
            &a_value.as_ref().as_ref().unwrap().invoked[0].repository_rule_calls[0].definition;
        assert!(definition.local);
        assert!(definition.configure);
        assert_eq!(
            definition
                .environment
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["B", "A"]
        );

        for changed_source in [
            source(false, true, "['B','A']"),
            source(true, false, "['B','A']"),
            source(true, true, "['B','C']"),
        ] {
            let changed =
                compute_repository_rule_case(&dice, module, &changed_source, true, None).await;
            assert!(!HostPureModuleExtensionInvocationsKey::equality(
                &a, &changed
            ));
            let restored = compute_repository_rule_case(&dice, module, &a_source, true, None).await;
            assert!(HostPureModuleExtensionInvocationsKey::equality(
                &a, &restored
            ));
        }
    }

    #[tokio::test]
    async fn real_repository_rule_failure_retains_completed_and_current_prefixes() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nb=use_extension('//:ext.bzl','second')\n";
        let source = "repo=repository_rule(lambda ctx: None)\n\
            def first_impl(ctx):\n    repo(name='first')\n\
            def second_impl(ctx):\n    repo(name='second')\n    fail('boom')\n\
            first=module_extension(implementation=first_impl)\n\
            second=module_extension(implementation=second_impl)\n";
        let outcome = compute_repository_rule_case(&dice, module, source, true, None).await;
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("invocation failure is a complete terminal")
        };
        let Err(HostPureModuleExtensionInvocationsError::AfterPrepared {
            completed,
            current_calls,
            error: HostPureModuleExtensionInvocationError::Invocation(_),
            ..
        }) = value.as_ref()
        else {
            panic!("expected invocation terminal: {value:?}")
        };
        assert_eq!(completed[0].repository_rule_calls[0].name, "first");
        assert_eq!(current_calls[0].name, "second");
    }

    #[test]
    fn selected_owner_pure_uses_external_bzl_for_both_reacquisitions() {
        let source = include_str!("module_extension.rs");
        let production = &source[..source.find("mod tests {").unwrap()];
        let owner = &production[production.find("async fn owner_bzl").unwrap()..];
        assert!(owner.contains("RootRepositoryRoute::for_selected_extension_definition"));
        assert!(owner.contains("ExternalBzlModuleEvalKey::new_bzlmod"));
        assert!(owner.contains("ExternalBzlModuleObservationKey::new_bzlmod"));
        assert!(owner.contains("owner_bzl(ctx, key, &inputs, None"));
        assert!(owner.contains("owner_bzl(ctx, key, &inputs, Some(&first)"));
    }
}
