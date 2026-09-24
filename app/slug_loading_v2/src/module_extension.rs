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
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::FactNumber;
use slug_bzlmod_v2::FactValue;
use slug_bzlmod_v2::Facts;
use slug_bzlmod_v2::HostRepositoryLabelPathObservationKey;
use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::HostRepositoryRelativePath;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionSource;
use slug_bzlmod_v2::HostSelectedExtensionDemandErrorDisposition;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationError;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputs;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsError;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsKey;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsObservationError;
use slug_bzlmod_v2::HostSelectedExtensionOwnerInputsObservationKey;
use slug_bzlmod_v2::ModuleExtensionMetadata;
use slug_bzlmod_v2::ModuleExtensionRepositorySelection;
use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
use slug_bzlmod_v2::RepositoryHostInputTransaction;
use slug_bzlmod_v2::RepositoryLabelPathAddress;
use slug_bzlmod_v2::RepositoryPlatform;
use slug_bzlmod_v2::RepositoryPlatformKey;
use slug_bzlmod_v2::RootModuleExtensionUsage;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SelectedExtensionDownloadEpoch;
use slug_bzlmod_v2::SelectedExtensionDownloadEpochKey;
use slug_bzlmod_v2::SelectedExtensionDownloadRequest;
use slug_bzlmod_v2::SelectedExtensionDownloadTransaction;
use slug_bzlmod_v2::SelectedExtensionDownloadVerified;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::UsedDownloadRequestKey;
use slug_bzlmod_v2::host_repository_relative_path;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathFileDigest;
use slug_workspace_v2::PathFileDigestObservationKey;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::ResolvedPathState;
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
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::float::StarlarkFloat;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::structs::AllocStruct;
use starlark::values::tuple::TupleRef;
use starlark_map::small_set::SmallSet;
use starlark_map::sorted_map::SortedMap;

use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
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
    pub(crate) metadata: ModuleExtensionMetadata,
    pub(crate) root_usage: Option<RootModuleExtensionUsage>,
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
fn invoke_pure_preflight(prepared: Arc<HostPreparedModuleExtensionInputs>, preflight: Vec<PurePreflight>, observations: PathObservationEpoch, capture_events: bool, event_batch: &mut EventBatch, max_tag_count: usize) -> PureInvocationsDriverOutcome {
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
    if let Err(error) = validate_invocation_tag_counts(prepared.inputs.iter().map(|input| input.tags.len()), max_tag_count) {
        return pure_complete(Err(after(None, &[], Arc::from([]), HostPureModuleExtensionInvocationError::Invocation(error))), observations);
    }
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
        let root_usage = InvocationContext::from_value(context)
            .expect("invocation context was just allocated")
            .root_usage;
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
        let metadata = match returned_metadata(returned) {
            Ok(metadata) if metadata.facts().is_empty() => metadata,
            Ok(_) => return pure_complete(Err(after(Some(input.input.parts().0), &invoked, repository_rule_calls, HostPureModuleExtensionInvocationError::Result("nonempty module extension facts require the deferred lockfile lifecycle".into()))), observations),
            Err(error) => return pure_complete(Err(after(Some(input.input.parts().0), &invoked, repository_rule_calls, HostPureModuleExtensionInvocationError::Result(error.to_string().into()))), observations),
        };
        invoked.push(HostPureModuleExtensionInvocationReceipt {
            request: input.input.parts().0.clone(),
            repository_rule_calls,
            metadata,
            root_usage,
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
            MAX_INVOCATION_TAGS_PER_MODULE,
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
pub(crate) struct HostSelectedExtensionOwnerPureResult { pub(crate) inputs: Arc<HostSelectedExtensionOwnerInputs>, pub(crate) factors: Arc<SelectedExtensionEvalFactors>, pub(crate) receipt: HostPureModuleExtensionInvocationReceipt }
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct SelectedExtensionEvalFactors {
    operating_system: Option<CompactString>,
    architecture: Option<CompactString>,
}
impl SelectedExtensionEvalFactors {
    fn from_host(
        definition: &ModuleExtensionDefinitionProjection,
        host: &ModuleExtensionOs,
    ) -> Self {
        Self {
            operating_system: definition
                .os_dependent
                .then(|| CompactString::new(host.platform.os_name())),
            architecture: definition
                .arch_dependent
                .then(|| CompactString::new(host.platform.arch())),
        }
    }
}
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostSelectedExtensionOwnerPureError { Inputs(HostSelectedExtensionOwnerInputsError), Compute(CompactString), AfterInputs { inputs: Arc<HostSelectedExtensionOwnerInputs>, request: HostSelectedExtensionDefinitionLoadRequest, current_calls: Arc<[RepositoryRuleCallRecord]>, message: CompactString }, Factored { factors: Arc<SelectedExtensionEvalFactors>, error: Arc<HostSelectedExtensionOwnerPureError> } }
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
pub(crate) enum HostSelectedExtensionOwnerPureObservationError { Inputs(HostSelectedExtensionOwnerInputsObservationError), HostBzl { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Option<Arc<OwnerProjection>>, error: ObservedPathFrontierError }, Demand { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Arc<OwnerProjection>, error: HostSelectedExtensionDemandObservationError }, CanonicalRoute { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Arc<OwnerProjection>, error: Arc<HostCanonicalRepositoryLoadRouteObservationError> }, Merge { inputs: Arc<HostSelectedExtensionOwnerInputs>, first: Option<Arc<OwnerProjection>>, error: ObservedPathFrontierError }, Factored { factors: Arc<SelectedExtensionEvalFactors>, error: Arc<HostSelectedExtensionOwnerPureObservationError> } }
impl HostSelectedExtensionOwnerPureObservationError {
    pub(crate) fn selected_frontier(&self) -> slug_bzlmod_v2::HostSelectedObservationFrontier {
        match self {
            Self::Inputs(error) => error.selected_frontier(),
            Self::Demand { error, .. } => error.selected_frontier(),
            Self::CanonicalRoute { error, .. } => error.selected_frontier(),
            Self::HostBzl { error, .. } | Self::Merge { error, .. } => {
                slug_bzlmod_v2::HostSelectedObservationFrontier::Path(error.clone())
            }
            Self::Factored { error, .. } => error.selected_frontier(),
        }
    }
}
type OwnerPureDriver = SourcePreparationOutcome<
    Result<(OwnerPureResult, PathObservationEpoch), HostSelectedExtensionOwnerPureObservationError>,
>;
#[rustfmt::skip]
fn owner_complete(value: Result<HostSelectedExtensionOwnerPureResult, HostSelectedExtensionOwnerPureError>, observations: PathObservationEpoch) -> OwnerPureDriver { SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations))) }
#[rustfmt::skip]
fn owner_after(inputs: &Arc<HostSelectedExtensionOwnerInputs>, calls: Arc<[RepositoryRuleCallRecord]>, message: impl Into<CompactString>) -> HostSelectedExtensionOwnerPureError { HostSelectedExtensionOwnerPureError::AfterInputs { inputs: inputs.clone(), request: inputs.request().clone(), current_calls: calls, message: message.into() } }

fn factor_owner_terminal(
    factors: &Arc<SelectedExtensionEvalFactors>,
    terminal: OwnerPureDriver,
) -> OwnerPureDriver {
    match terminal {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(
            HostSelectedExtensionOwnerPureObservationError::Factored {
                factors: factors.clone(),
                error: Arc::new(error),
            },
        )),
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            let result = if let Err(error) = result.as_ref() {
                Arc::new(Err(HostSelectedExtensionOwnerPureError::Factored {
                    factors: factors.clone(),
                    error: Arc::new(error.clone()),
                }))
            } else {
                result
            };
            SourcePreparationOutcome::Complete(Ok((result, observations)))
        }
    }
}

#[rustfmt::skip]
fn owner_projection(module: &FrozenBzlModule, request: &HostSelectedExtensionDefinitionLoadRequest) -> Result<OwnerProjection, CompactString> {
    let (export, _visibility) = module.module.get_assigned(request.parts().1).map_err(|error| CompactString::from(error.to_string()))?;
    let definition = export.downcast::<FrozenModuleExtensionDefinition>().map_err(|_| CompactString::from("selected export is not module_extension"))?;
    let projection = definition.projection();
    if projection.facts_version != 0 {
        return Err("selected module extension has unsupported factors".into());
    }
    Ok(OwnerProjection { manifest: module.manifest.clone(), definition: projection })
}

type SelectedHostObservation = SourcePreparationOutcome<Result<ModuleExtensionOs, CompactString>>;

async fn observe_selected_host(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    declared: &[CompactString],
) -> SelectedHostObservation {
    let transaction = match ctx
        .per_transaction_data()
        .data
        .get::<RepositoryHostInputTransaction>()
    {
        Ok(transaction) => transaction.clone(),
        Err(_) => {
            return SourcePreparationOutcome::Complete(Err(
                "selected module extension Host inputs unavailable".into(),
            ));
        }
    };
    let names = RepositoryEnvironmentNameFrontier::from_unsorted(declared.iter().cloned());
    let unknown = names
        .difference(transaction.frontier())
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return SourcePreparationOutcome::Need(SourcePreparationNeeds::environment(
            NeedRepositoryEnvironmentNames::new(
                workspace.dupe(),
                RepositoryEnvironmentNameFrontier::from_unsorted(unknown),
            )
            .expect("selected extension has nonempty unobserved environment names"),
        ));
    }
    let platform = match ctx
        .compute(&RepositoryPlatformKey::new(workspace.dupe()))
        .await
    {
        Ok(platform) => platform,
        Err(_) => {
            return SourcePreparationOutcome::Complete(Err(
                "selected module extension Host platform unavailable".into(),
            ));
        }
    };
    let mut environment = Vec::with_capacity(names.iter().count());
    for name in names.iter() {
        let cell = match ctx
            .compute(&RepositoryEnvironmentCellKey::new(
                workspace.dupe(),
                name.clone(),
            ))
            .await
        {
            Ok(cell) => cell,
            Err(_) => {
                return SourcePreparationOutcome::Complete(Err(format!(
                    "selected module extension environment name {name:?} unavailable"
                )
                .into()));
            }
        };
        let Some(value) = cell.value() else {
            return SourcePreparationOutcome::Complete(Err(format!(
                "selected module extension environment name {name:?} unauthorized"
            )
            .into()));
        };
        if value != &transaction.snapshot().get(name).cloned() {
            return SourcePreparationOutcome::Complete(Err(format!(
                "selected module extension environment name {name:?} differs from request"
            )
            .into()));
        }
        environment.push((name.clone(), value.clone()));
    }
    SourcePreparationOutcome::Complete(Ok(ModuleExtensionOs {
        platform,
        environment: environment.into(),
    }))
}

/// Authenticate the injected download cell before entering synchronous
/// Starlark. A cold owner has no authorized cell to read: its call discovers
/// the request through a transient Need instead.
async fn selected_download_preflight(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionOwnerPureKey,
) -> Result<Option<Arc<SelectedExtensionDownloadVerified>>, CompactString> {
    let transaction = ctx
        .per_transaction_data()
        .data
        .get::<SelectedExtensionDownloadTransaction>()
        .map_err(|_| CompactString::from("selected module extension download inputs unavailable"))?
        .clone();
    if transaction.workspace() != &key.workspace {
        return Err("selected module extension download workspace differs from request".into());
    }
    if !transaction.is_authorized(&key.owner) {
        return Ok(None);
    }
    let epoch = ctx
        .compute(&SelectedExtensionDownloadEpochKey::new(
            key.workspace.dupe(),
            key.owner.clone(),
        ))
        .await
        .map_err(|_| CompactString::from("selected module extension download cell unavailable"))?;
    let expected = transaction.get(&key.owner);
    match epoch {
        SelectedExtensionDownloadEpoch::Ready(actual)
            if expected.is_some_and(|expected| expected == &actual) =>
        {
            Ok(Some(actual))
        }
        SelectedExtensionDownloadEpoch::AuthorizedEmpty if expected.is_none() => Ok(None),
        SelectedExtensionDownloadEpoch::Unauthorized => {
            Err("selected module extension download cell unauthorized".into())
        }
        _ => Err("selected module extension download cell differs from request".into()),
    }
}

const MAX_INVOCATION_TAGS_PER_MODULE: usize = 1 << 20;

fn validate_invocation_tag_count(count: usize, max_tag_count: usize) -> Result<(), CompactString> {
    if count > max_tag_count {
        return Err(format!("module extension has more than {max_tag_count} tags").into());
    }
    Ok(())
}

fn validate_invocation_tag_counts(
    counts: impl IntoIterator<Item = usize>,
    max_tag_count: usize,
) -> Result<(), CompactString> {
    counts
        .into_iter()
        .try_for_each(|count| validate_invocation_tag_count(count, max_tag_count))
}

#[rustfmt::skip]
fn owner_invocation_modules(inputs: &HostSelectedExtensionOwnerInputs, projection: &OwnerProjection, owner: &Arc<()>, max_tag_count: usize) -> Result<Arc<[PreparedInvocationModule]>, CompactString> {
    for (_, schema) in projection.definition.tag_classes.iter() {
        validate_module_extension_tag_schema(schema).map_err(|error| CompactString::from(error.to_string()))?;
    }
    let classes: Arc<[CompactString]> = projection.definition.tag_classes.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>().into();
    inputs.modules().iter().enumerate().map(|(module_index, input)| {
        let (context_repo, name, version, is_root, mapping, tags) = input.parts();
        validate_invocation_tag_count(tags.len(), max_tag_count)?;
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

async fn observe_selected_watches(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionOwnerPureKey,
    inputs: &Arc<HostSelectedExtensionOwnerInputs>,
    first: &Arc<OwnerProjection>,
    calls: &Arc<[RepositoryRuleCallRecord]>,
    labels: &[CanonicalLabel],
    mut observations: PathObservationEpoch,
) -> Result<PathObservationEpoch, OwnerPureDriver> {
    macro_rules! merge {
        ($incoming:expr) => {
            observations = union_pure_observations(&observations, $incoming).map_err(|error| {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                ))
            })?;
        };
    }
    macro_rules! fail {
        ($message:expr) => {
            return Err(owner_complete(
                Err(owner_after(inputs, calls.clone(), $message)),
                observations,
            ))
        };
    }
    for label in labels {
        let address = RepositoryLabelPathAddress::from_label(label);
        let label_key =
            match HostRepositoryLabelPathObservationKey::new_root(key.workspace.dupe(), address) {
                Ok(key) => key,
                Err(error) => fail!(format!("module_ctx.watch Label is unsupported: {error:?}")),
            };
        let observed = match ctx.compute(&label_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!("module_ctx.watch Label lookup failed: {error}")),
        };
        merge!(observed.observations());
        let path = match observed.result().as_ref() {
            Ok(value) => value,
            Err(error) => fail!(format!("module_ctx.watch Label path failed: {error:?}")),
        };
        if !path.path().as_path().starts_with(key.workspace.as_path()) {
            fail!("module_ctx.watch Label resolves outside the root workspace");
        }
        if path.namespace() != PathObservationNamespace::Host {
            fail!("module_ctx.watch Label has an unsupported path namespace");
        }
        let logical_path = path.path().dupe();
        let resolved = match ctx
            .compute(&ResolvedPathObservationKey::new(
                PathObservationNamespace::Host,
                logical_path.dupe(),
            ))
            .await
        {
            Ok(PathOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(
                    SourcePreparationNeeds::path(need),
                ));
            }
            Ok(PathOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                )));
            }
            Ok(PathOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!("module_ctx.watch path resolution failed: {error}")),
        };
        merge!(resolved.observations());
        let resolved = match resolved.result() {
            Ok(value) => value,
            Err(error) => fail!(format!(
                "module_ctx.watch path resolution failed: {error:?}"
            )),
        };
        if !resolved.symlinks().is_empty() {
            fail!("module_ctx.watch does not support symlink paths");
        }
        match resolved.state() {
            ResolvedPathState::Missing => {}
            ResolvedPathState::Present(lstat) if lstat.kind() == PathNodeKind::RegularFile => {}
            ResolvedPathState::Present(_) => {
                fail!("module_ctx.watch supports only regular or missing files");
            }
        }
        let digest = match ctx
            .compute(&PathFileDigestObservationKey::new(
                PathObservationNamespace::Host,
                logical_path,
            ))
            .await
        {
            Ok(PathOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(
                    SourcePreparationNeeds::path(need),
                ));
            }
            Ok(PathOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                )));
            }
            Ok(PathOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!("module_ctx.watch file digest failed: {error}")),
        };
        merge!(digest.observations());
        match (resolved.state(), digest.result()) {
            (ResolvedPathState::Missing, Ok(PathFileDigest::Missing))
            | (ResolvedPathState::Present(_), Ok(PathFileDigest::Present(_))) => {}
            (_, Ok(_)) => fail!("module_ctx.watch file changed during observation"),
            (_, Err(error)) => fail!(format!("module_ctx.watch file digest failed: {error:?}")),
        }
    }
    Ok(observations)
}

async fn observe_selected_path_label(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionOwnerPureKey,
    inputs: &Arc<HostSelectedExtensionOwnerInputs>,
    first: &Arc<OwnerProjection>,
    calls: &Arc<[RepositoryRuleCallRecord]>,
    label: &CanonicalLabel,
    mut observations: PathObservationEpoch,
) -> Result<(HostRepositoryLabelPathValue, PathObservationEpoch), OwnerPureDriver> {
    macro_rules! merge {
        ($incoming:expr) => {
            observations = union_pure_observations(&observations, $incoming).map_err(|error| {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                ))
            })?;
        };
    }
    macro_rules! fail {
        ($message:expr) => {
            return Err(owner_complete(
                Err(owner_after(inputs, calls.clone(), $message)),
                observations,
            ))
        };
    }
    let address = RepositoryLabelPathAddress::from_label(label);
    let route = if address.repo().is_root() {
        None
    } else {
        // The demand key resolves generated ownership without evaluating the
        // owner. Never ask the load route for a generated repository here:
        // that can recurse into this owner (or a cross-owner cycle).
        let demand = match ctx
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                key.workspace.dupe(),
                address.repo().clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Demand {
                        inputs: inputs.clone(),
                        first: first.clone(),
                        error,
                    },
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!(
                "module_ctx.path generated-owner preflight failed: {error}"
            )),
        };
        merge!(demand.observations());
        match demand.result().as_ref() {
            Ok(_) => fail!("module_ctx.path does not support generated repository Labels"),
            Err(error)
                if error.disposition() == HostSelectedExtensionDemandErrorDisposition::Missing => {}
            Err(error) => fail!(format!(
                "module_ctx.path generated-owner preflight failed: {error}"
            )),
        }
        let route = match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                key.workspace.dupe(),
                address.repo().clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::CanonicalRoute {
                        inputs: inputs.clone(),
                        first: first.clone(),
                        error: Arc::new(error),
                    },
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!("module_ctx.path canonical route failed: {error}")),
        };
        merge!(route.observations());
        let route = match route.result().as_ref() {
            Ok(value) => value,
            Err(error) => fail!(format!("module_ctx.path canonical route failed: {error:?}")),
        };
        if route.route().view().generated_effect_seed().is_some() {
            fail!("module_ctx.path does not support generated repository Labels");
        }
        Some(HostRepositorySourceRoute::canonical(route.input().clone()))
    };
    let path_key = match route {
        None => HostRepositoryLabelPathObservationKey::new_root(key.workspace.dupe(), address),
        Some(route) => HostRepositoryLabelPathObservationKey::new_external(route, address),
    };
    let path_key = match path_key {
        Ok(value) => value,
        Err(error) => fail!(format!("module_ctx.path Label is unsupported: {error:?}")),
    };
    let observed = match ctx.compute(&path_key).await {
        Ok(SourcePreparationOutcome::Need(need)) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        Ok(SourcePreparationOutcome::Complete(Err(error))) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedExtensionOwnerPureObservationError::Merge {
                    inputs: inputs.clone(),
                    first: Some(first.clone()),
                    error,
                },
            )));
        }
        Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
        Err(error) => fail!(format!("module_ctx.path Label lookup failed: {error}")),
    };
    merge!(observed.observations());
    let path = match observed.result().as_ref() {
        Ok(value) => value.clone(),
        Err(error) => fail!(format!("module_ctx.path Label lookup failed: {error:?}")),
    };
    Ok((path, observations))
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
    let host = match observe_selected_host(ctx, &key.workspace, &first.definition.environment).await {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => return owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1),
        SourcePreparationOutcome::Complete(Ok(host)) => host,
    };
    let factors = Arc::new(SelectedExtensionEvalFactors::from_host(&first.definition, &host));
    let owner = Arc::new(());
    let modules = match owner_invocation_modules(&inputs, &first, &owner, MAX_INVOCATION_TAGS_PER_MODULE) {
        Ok(value) => value,
        Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1)),
    };
    let ready = match selected_download_preflight(ctx, key).await {
        Ok(ready) => ready,
        Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), first_module.1)),
    };
    let (module, mut observations) = match owner_bzl(ctx, key, &inputs, Some(&first), first_module.1, mode).await { Ok(value) => value, Err(outcome) => return factor_owner_terminal(&factors, outcome) };
    let second = match owner_projection(&module, inputs.request()) {
        Ok(value) => value,
        Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations)),
    };
    if second != *first {
        return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), "reacquired selected module extension differs")), observations));
    }
    let definition = module.module.get_assigned(inputs.request().parts().1).expect("projection authenticated export").0.downcast::<FrozenModuleExtensionDefinition>().expect("projection authenticated kind");
    let root_usage = inputs.root_proxies().map(|proxies| RootModuleExtensionUsage::new(
        proxies.iter().any(|proxy| !proxy.dev_dependency),
        proxies.iter().any(|proxy| proxy.dev_dependency),
    ));
    let mut ready_paths = Vec::new();
    loop {
        // Nothing emitted by an interrupted Starlark pass is published. The
        // path-ready map is the only evaluation state retained for replay.
        let selected_download = Arc::new(SelectedDownloadInvocation::new(key.workspace.dupe(), key.owner.clone(), ready.clone()));
        let selected_watch = Arc::new(SelectedWatchInvocation::new());
        let selected_path = Arc::new(SelectedPathInvocation::new(ready_paths.clone()));
        let (returned, calls, capture, pending_path) = {
            let invocation_module = Module::new();
            let modules = modules
                .iter()
                .cloned()
                .map(|module| module.materialize(invocation_module.frozen_heap()))
                .collect::<Vec<_>>()
                .into();
            let context = invocation_module.heap().alloc_simple(InvocationContext::new_modules(modules, &owner, root_usage, Some(host.clone())).with_selected_download(selected_download.clone()).with_selected_watch(selected_watch.clone()).with_selected_path(selected_path.clone()));
            let capture = ctx.per_transaction_data().data.get::<CaptureEvaluationEvents>().is_ok().then(InvocationPrintCapture::default);
            let repository_rules = RepositoryRuleInvocationState::new();
            let mut evaluator = Evaluator::new(&invocation_module);
            evaluator.extra = Some(&repository_rules);
            if let Some(capture) = capture.as_ref() {
                evaluator.set_print_handler(capture);
            }
            let returned = evaluator.eval_function(definition.implementation.to_value(), &[context], &[]);
            let pending = match selected_download.need() {
                Ok(pending) => pending,
                Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations)),
            };
            if let Some(need) = pending {
                return SourcePreparationOutcome::Need(SourcePreparationNeeds::selected_extension_download((*need).clone()));
            }
            let pending_path = match selected_path.pending() {
                Ok(pending) => pending,
                Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations)),
            };
            let returned = returned
                .map_err(|error| CompactString::from(error.to_string()))
                .and_then(|value| returned_metadata(value).map_err(|error| CompactString::from(error.to_string())));
            drop(evaluator);
            (returned, repository_rules.records(), capture, pending_path)
        };
        // A successful download is part of the selected closure even when
        // later Starlark evaluation, watch or Label preparation fails.
        let used = match selected_download.used() {
            Ok(used) => used,
            Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations)),
        };
        if let Some(used) = used {
            if let Err(error) = ctx.compute(&UsedDownloadRequestKey::new(used)).await {
                return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, calls, error.to_string())), observations));
            }
        }
        let watched = match selected_watch.called() {
            Ok(watched) => watched,
            Err(error) => return factor_owner_terminal(&factors, owner_complete(Err(owner_after(&inputs, Arc::from([]), error)), observations)),
        };
        observations = match observe_selected_watches(ctx, key, &inputs, &first, &calls, &watched, observations).await {
            Ok(observations) => observations,
            Err(terminal) => return factor_owner_terminal(&factors, terminal),
        };
        if let Some(label) = pending_path {
            let (path, next_observations) = match observe_selected_path_label(ctx, key, &inputs, &first, &calls, &label, observations).await {
                Ok(value) => value,
                Err(terminal) => return factor_owner_terminal(&factors, terminal),
            };
            observations = next_observations;
            ready_paths.push((label, path));
            continue;
        }
        let request = inputs.request().clone();
        let result = match returned {
            Err(error) => Err(owner_after(&inputs, calls, error)),
            Ok(metadata) => match metadata {
                metadata if metadata.facts().is_empty() => Ok(HostSelectedExtensionOwnerPureResult {
                    inputs,
                    factors: factors.clone(),
                    receipt: HostPureModuleExtensionInvocationReceipt { request, repository_rule_calls: calls, metadata, root_usage },
                }),
                _ => Err(owner_after(&inputs, calls, "nonempty module extension facts require the deferred lockfile lifecycle")),
            },
        };
        if let Some(capture) = capture {
            ctx.store_evaluation_data(capture.into_batch()).expect("selected owner invocation stores one local Complete event batch");
        }
        return factor_owner_terminal(&factors, owner_complete(result, observations));
    }
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
struct StarlarkModuleExtensionMetadata(ModuleExtensionMetadata);

impl fmt::Display for StarlarkModuleExtensionMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<extension_metadata>")
    }
}
starlark::starlark_simple_value!(StarlarkModuleExtensionMetadata);

#[starlark_value(type = "extension_metadata")]
impl<'v> StarlarkValue<'v> for StarlarkModuleExtensionMetadata {}

fn metadata_selection(
    regular: Option<Value<'_>>,
    dev: Option<Value<'_>>,
) -> anyhow::Result<(
    ModuleExtensionRepositorySelection,
    ModuleExtensionRepositorySelection,
)> {
    let regular = regular.filter(|value| !value.is_none());
    let dev = dev.filter(|value| !value.is_none());
    let is_all = |value: Option<Value<'_>>| value.and_then(Value::unpack_str) == Some("all");
    let is_empty_list = |value: Option<Value<'_>>| {
        value
            .and_then(ListRef::from_value)
            .is_some_and(|value| value.is_empty())
    };
    if regular.is_none() && dev.is_none() {
        return Ok((
            ModuleExtensionRepositorySelection::Unspecified,
            ModuleExtensionRepositorySelection::Unspecified,
        ));
    }
    if is_all(regular) && is_empty_list(dev) {
        return Ok((
            ModuleExtensionRepositorySelection::All,
            ModuleExtensionRepositorySelection::explicit([]),
        ));
    }
    if is_all(dev) && is_empty_list(regular) {
        return Ok((
            ModuleExtensionRepositorySelection::explicit([]),
            ModuleExtensionRepositorySelection::All,
        ));
    }
    if is_all(regular) || is_all(dev) {
        anyhow::bail!(
            "if one of root_module_direct_deps and root_module_direct_dev_deps is \"all\", the other must be an empty list"
        );
    }
    if regular.is_some_and(|value| value.unpack_str().is_some())
        || dev.is_some_and(|value| value.unpack_str().is_some())
    {
        anyhow::bail!(
            "root_module_direct_deps and root_module_direct_dev_deps must be None, \"all\", or a list of strings"
        );
    }
    if regular.is_none() != dev.is_none() {
        anyhow::bail!(
            "root_module_direct_deps and root_module_direct_dev_deps must both be specified or both be unspecified"
        );
    }
    let sequence = |name: &str, value: Value<'_>| -> anyhow::Result<Vec<CompactString>> {
        let values: Vec<Value<'_>> = if let Some(values) = ListRef::from_value(value) {
            values.iter().collect()
        } else if let Some(values) = TupleRef::from_value(value) {
            values.iter().collect()
        } else {
            anyhow::bail!("{name} must be a sequence of strings")
        };
        let mut unique = SmallSet::with_capacity(values.len());
        for value in values {
            let value = value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("{name} must be a sequence of strings"))?;
            let mut bytes = value.bytes();
            let valid = bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && bytes
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
            if !valid {
                anyhow::bail!(
                    "in {name}: invalid user-provided repo name '{value}': valid names may contain only A-Z, a-z, 0-9, '-', '_', '.', and must start with a letter or a number"
                )
            }
            if !unique.insert(value.into()) {
                anyhow::bail!("in {name}: duplicate entry '{value}'")
            }
        }
        Ok(unique.into_iter().collect())
    };
    let regular = sequence("root_module_direct_deps", regular.unwrap())?;
    let dev = sequence("root_module_direct_dev_deps", dev.unwrap())?;
    for value in &dev {
        if regular.iter().any(|regular| regular == value) {
            anyhow::bail!(
                "in root_module_direct_dev_deps: entry '{value}' is also in root_module_direct_deps"
            )
        }
    }
    Ok((
        ModuleExtensionRepositorySelection::explicit(regular),
        ModuleExtensionRepositorySelection::explicit(dev),
    ))
}

fn metadata_facts(value: Option<Value<'_>>) -> anyhow::Result<Facts> {
    let Some(value) = value else {
        return Ok(Facts::default());
    };
    let values = DictRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("facts must be a dict with string keys"))?;
    let mut active = SmallSet::new();
    let values = values
        .iter()
        .map(|(key, value)| {
            let key = key
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("Facts keys must be strings"))?;
            Ok((key.into(), metadata_fact_value(value, 6, &mut active)?))
        })
        .collect::<anyhow::Result<SortedMap<_, _>>>()?;
    Ok(Facts::new(values))
}

fn metadata_fact_value<'v>(
    value: Value<'v>,
    remaining_depth: i8,
    active: &mut SmallSet<starlark::values::ValueIdentity<'v>>,
) -> anyhow::Result<FactValue> {
    if remaining_depth < 0 {
        anyhow::bail!("Facts cannot be nested more than 7 levels deep")
    }
    if value.is_none() {
        return Ok(FactValue::Null);
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(FactValue::Bool(value));
    }
    if value.get_type() == "int" {
        return Ok(FactValue::Number(FactNumber::integer(
            value.to_string().into(),
        )));
    }
    if let Some(value) = value.downcast_ref::<StarlarkFloat>() {
        return FactNumber::finite_float(value.0)
            .map(FactValue::Number)
            .ok_or_else(|| anyhow::anyhow!("Facts floats must be finite"));
    }
    if let Some(value) = value.unpack_str() {
        return Ok(FactValue::String(value.into()));
    }
    let identity = value.identity();
    if let Some(values) = ListRef::from_value(value) {
        if !active.insert(identity) {
            anyhow::bail!("Facts must not contain cyclic values")
        }
        let result = values
            .iter()
            .map(|value| metadata_fact_value(value, remaining_depth - 1, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(FactValue::List);
        active.shift_remove(&identity);
        return result;
    }
    if let Some(values) = TupleRef::from_value(value) {
        let result = values
            .iter()
            .map(|value| metadata_fact_value(value, remaining_depth - 1, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(FactValue::List);
        return result;
    }
    if let Some(values) = DictRef::from_value(value) {
        if !active.insert(identity) {
            anyhow::bail!("Facts must not contain cyclic values")
        }
        let result = values
            .iter()
            .map(|(key, value)| {
                let key = key
                    .unpack_str()
                    .ok_or_else(|| anyhow::anyhow!("Facts keys must be strings"))?;
                Ok((
                    key.into(),
                    metadata_fact_value(value, remaining_depth - 1, active)?,
                ))
            })
            .collect::<anyhow::Result<SortedMap<_, _>>>()
            .map(FactValue::Dict);
        active.shift_remove(&identity);
        return result;
    }
    anyhow::bail!(
        "'{}' ({}) is not supported in facts",
        value.to_repr(),
        value.get_type()
    )
}

fn returned_metadata(value: Value<'_>) -> anyhow::Result<ModuleExtensionMetadata> {
    if value.is_none() {
        return Ok(ModuleExtensionMetadata::default());
    }
    StarlarkModuleExtensionMetadata::from_value(value)
        .map(|value| value.0.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "module extension must return None or extension_metadata, got {}",
                value.get_type()
            )
        })
}

#[derive(Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct ModuleExtensionOs {
    platform: RepositoryPlatform,
    environment: Arc<[(CompactString, Option<Arc<str>>)]>,
}

impl fmt::Debug for ModuleExtensionOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleExtensionOs")
            .field("platform", &self.platform)
            .field("declared_environment_names", &self.environment.len())
            .finish()
    }
}

impl fmt::Display for ModuleExtensionOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<module_extension_os>")
    }
}
starlark::starlark_simple_value!(ModuleExtensionOs);

#[starlark_value(type = "repository_os")]
impl<'v> StarlarkValue<'v> for ModuleExtensionOs {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc(self.platform.os_name())),
            "arch" => Some(heap.alloc(self.platform.arch())),
            "environ" => {
                Some(heap.alloc_simple(ModuleExtensionOsEnviron(self.environment.clone())))
            }
            _ => None,
        }
    }
}

#[derive(Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct ModuleExtensionOsEnviron(Arc<[(CompactString, Option<Arc<str>>)]>);

impl fmt::Debug for ModuleExtensionOsEnviron {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleExtensionOsEnviron")
            .field("declared_names", &self.0.len())
            .finish()
    }
}

impl ModuleExtensionOsEnviron {
    fn declared(&self, name: &str) -> anyhow::Result<Option<&str>> {
        self.0
            .iter()
            .find(|(declared, _)| declared == name)
            .map(|(_, value)| value.as_deref())
            .ok_or_else(|| anyhow::anyhow!("undeclared module extension environment name {name:?}"))
    }
}

impl fmt::Display for ModuleExtensionOsEnviron {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<declared_module_extension_environ>")
    }
}
starlark::starlark_simple_value!(ModuleExtensionOsEnviron);

#[starlark_value(type = "declared_module_extension_environ")]
impl<'v> StarlarkValue<'v> for ModuleExtensionOsEnviron {
    fn is_in(&self, index: Value<'v>) -> starlark::Result<bool> {
        let name = index.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "module extension environment names must be strings"
            ))
        })?;
        Ok(self
            .declared(name)
            .map_err(starlark::Error::new_other)?
            .is_some())
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(module_extension_os_environ_methods)
    }
}

#[starlark_module]
fn module_extension_os_environ_methods(builder: &mut MethodsBuilder) {
    fn get<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] name: &str,
        #[starlark(require = pos)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let environ = ModuleExtensionOsEnviron::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module extension environment receiver"))?;
        Ok(match environ.declared(name)? {
            Some(value) => eval.heap().alloc_str(value).to_value(),
            None => default.unwrap_or_else(Value::new_none),
        })
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationContext {
    modules: Arc<[InvocationModule]>,
    root_usage: Option<RootModuleExtensionUsage>,
    os: Option<ModuleExtensionOs>,
    selected_download: Option<Arc<SelectedDownloadInvocation>>,
    selected_watch: Option<Arc<SelectedWatchInvocation>>,
    selected_path: Option<Arc<SelectedPathInvocation>>,
    #[allocative(skip)]
    owner: Arc<()>,
}

const MAX_SELECTED_WATCHES: usize = 8;
const MAX_SELECTED_PATH_LABELS: usize = 8;

#[derive(Debug, Allocative)]
struct SelectedPathInvocation {
    ready: Vec<(CanonicalLabel, HostRepositoryLabelPathValue)>,
    pending: Mutex<Option<CanonicalLabel>>,
}

impl SelectedPathInvocation {
    fn new(ready: Vec<(CanonicalLabel, HostRepositoryLabelPathValue)>) -> Self {
        Self {
            ready,
            pending: Mutex::new(None),
        }
    }

    fn call(&self, label: &CanonicalLabel) -> anyhow::Result<HostRepositoryLabelPathValue> {
        if let Some((_, path)) = self.ready.iter().find(|(candidate, _)| candidate == label) {
            return Ok(path.clone());
        }
        if self.ready.len() == MAX_SELECTED_PATH_LABELS {
            anyhow::bail!(
                "module_ctx.path supports at most {MAX_SELECTED_PATH_LABELS} distinct Labels"
            );
        }
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("selected module extension path scratch unavailable"))?;
        *pending = Some(label.clone());
        anyhow::bail!("selected module extension Label path requires preparation")
    }

    fn pending(&self) -> Result<Option<CanonicalLabel>, CompactString> {
        self.pending
            .lock()
            .map(|pending| pending.clone())
            .map_err(|_| "selected module extension path scratch unavailable".into())
    }
}

#[derive(Debug, Allocative)]
struct SelectedWatchInvocation {
    called: Mutex<SmallSet<CanonicalLabel>>,
}

impl SelectedWatchInvocation {
    fn new() -> Self {
        Self {
            called: Mutex::new(SmallSet::new()),
        }
    }

    fn call(&self, label: &CanonicalLabel) -> anyhow::Result<()> {
        if !label.package().repo().is_root() {
            anyhow::bail!("module_ctx.watch supports only root repository Labels");
        }
        let mut called = self
            .called
            .lock()
            .map_err(|_| anyhow::anyhow!("selected module extension watch scratch unavailable"))?;
        if !called.contains(label) && called.len() == MAX_SELECTED_WATCHES {
            anyhow::bail!(
                "module_ctx.watch supports at most {MAX_SELECTED_WATCHES} distinct Labels"
            );
        }
        called.insert(label.clone());
        Ok(())
    }

    fn called(&self) -> Result<Vec<CanonicalLabel>, CompactString> {
        self.called
            .lock()
            .map(|called| called.iter().cloned().collect())
            .map_err(|_| "selected module extension watch scratch unavailable".into())
    }
}

#[derive(Debug, Allocative)]
struct SelectedDownloadInvocation {
    workspace: NormalizedAbsolutePath,
    selected_owner: Arc<HostSelectedExtensionOwner>,
    ready: Option<Arc<SelectedExtensionDownloadVerified>>,
    scratch: Mutex<SelectedDownloadScratch>,
}

#[derive(Debug, Default, Allocative)]
struct SelectedDownloadScratch {
    first: Option<Arc<SelectedExtensionDownloadRequest>>,
    need: Option<Arc<SelectedExtensionDownloadRequest>>,
    used: Option<Arc<SelectedExtensionDownloadRequest>>,
}

impl SelectedDownloadInvocation {
    fn new(
        workspace: NormalizedAbsolutePath,
        selected_owner: Arc<HostSelectedExtensionOwner>,
        ready: Option<Arc<SelectedExtensionDownloadVerified>>,
    ) -> Self {
        Self {
            workspace,
            selected_owner,
            ready,
            scratch: Mutex::new(SelectedDownloadScratch::default()),
        }
    }

    fn call(&self, request: Arc<SelectedExtensionDownloadRequest>) -> anyhow::Result<()> {
        let mut scratch = self.scratch.lock().map_err(|_| {
            anyhow::anyhow!("selected module extension download scratch unavailable")
        })?;
        if let Some(first) = &scratch.first {
            if first != &request {
                anyhow::bail!("multiple distinct module extension downloads are unsupported");
            }
        } else {
            scratch.first = Some(request.clone());
        }
        if self
            .ready
            .as_ref()
            .is_some_and(|ready| ready.request() == &request)
        {
            scratch.used = Some(request);
            Ok(())
        } else {
            scratch.need = Some(request);
            anyhow::bail!("selected module extension download requires preparation")
        }
    }

    fn need(&self) -> Result<Option<Arc<SelectedExtensionDownloadRequest>>, CompactString> {
        self.scratch
            .lock()
            .map(|scratch| scratch.need.clone())
            .map_err(|_| "selected module extension download scratch unavailable".into())
    }

    fn used(&self) -> Result<Option<Arc<SelectedExtensionDownloadRequest>>, CompactString> {
        self.scratch
            .lock()
            .map(|scratch| scratch.used.clone())
            .map_err(|_| "selected module extension download scratch unavailable".into())
    }
}

/// A logical extension-workdir path. It deliberately does not expose a host
/// filesystem path or implement the physical path operations of Bazel's
/// repository context.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectedModuleScratchPath {
    relative: HostRepositoryRelativePath,
    #[allocative(skip)]
    owner: Arc<()>,
}

impl fmt::Display for SelectedModuleScratchPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.relative.as_path().display().fmt(f)
    }
}
starlark::starlark_simple_value!(SelectedModuleScratchPath);

#[starlark_value(type = "module_extension_scratch_path")]
impl<'v> StarlarkValue<'v> for SelectedModuleScratchPath {}

/// A prepared Label path. Derivation is lexical and never probes the target.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectedModuleLabelPath(HostRepositoryLabelPathValue);

impl fmt::Display for SelectedModuleLabelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.path_str())
    }
}
starlark::starlark_simple_value!(SelectedModuleLabelPath);

#[starlark_value(type = "path")]
impl<'v> StarlarkValue<'v> for SelectedModuleLabelPath {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if name != "dirname" {
            return None;
        }
        let parent = self.0.path().as_path().parent()?;
        let parent = NormalizedAbsolutePath::new(parent.to_path_buf())
            .expect("the parent of a normalized absolute path is normalized");
        let parent = HostRepositoryLabelPathValue::new(parent, self.0.namespace())
            .expect("the parent of a Unicode path is Unicode");
        Some(heap.alloc_simple(Self(parent)))
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(selected_module_label_path_methods)
    }
}

#[starlark_module]
fn selected_module_label_path_methods(builder: &mut MethodsBuilder) {
    fn get_child<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] child: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = SelectedModuleLabelPath::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid selected Label path receiver"))?;
        let child = selected_scratch_relative(child)?;
        let joined = this.0.path().as_path().join(child.as_path());
        let joined = NormalizedAbsolutePath::new(joined)
            .map_err(|_| anyhow::anyhow!("module_ctx.path get_child requires a relative path"))?;
        let path = HostRepositoryLabelPathValue::new(joined, this.0.namespace())
            .map_err(|_| anyhow::anyhow!("module_ctx.path get_child requires Unicode"))?;
        Ok(eval.heap().alloc_simple(SelectedModuleLabelPath(path)))
    }
}

fn selected_scratch_relative(path: &str) -> anyhow::Result<HostRepositoryRelativePath> {
    // PathBuf's component iterator normalizes some spellings, so check the
    // spelling as well as the repository-relative path invariant.
    if path.is_empty()
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        || path
            .split('/')
            .next()
            .is_some_and(|component| component.ends_with(':'))
    {
        anyhow::bail!("module_ctx.path requires a normalized relative string");
    }
    let relative = host_repository_relative_path(PathBuf::from(path))
        .map_err(|_| anyhow::anyhow!("module_ctx.path requires a normalized relative string"))?;
    if relative.as_path().to_str() != Some(path) {
        anyhow::bail!("module_ctx.path requires a normalized relative string");
    }
    Ok(relative)
}

fn selected_download_sha256(text: &str) -> anyhow::Result<[u8; 32]> {
    if text.len() != 64 || !text.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        anyhow::bail!("module_ctx.download requires a 64-digit SHA-256 checksum");
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .expect("validated ASCII hex pair");
    }
    Ok(bytes)
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
            Some(RootModuleExtensionUsage::new(
                input
                    .input
                    .root_proxies()
                    .iter()
                    .any(|proxy| !proxy.dev_dependency),
                input
                    .input
                    .root_proxies()
                    .iter()
                    .any(|proxy| proxy.dev_dependency),
            )),
            None,
        )
    }

    fn new_modules(
        modules: Arc<[InvocationModule]>,
        owner: &Arc<()>,
        root_usage: Option<RootModuleExtensionUsage>,
        os: Option<ModuleExtensionOs>,
    ) -> Self {
        Self {
            modules,
            root_usage,
            os,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
            owner: owner.clone(),
        }
    }

    fn with_selected_download(mut self, download: Arc<SelectedDownloadInvocation>) -> Self {
        self.selected_download = Some(download);
        self
    }

    fn with_selected_watch(mut self, watch: Arc<SelectedWatchInvocation>) -> Self {
        self.selected_watch = Some(watch);
        self
    }

    fn with_selected_path(mut self, path: Arc<SelectedPathInvocation>) -> Self {
        self.selected_path = Some(path);
        self
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
        match name {
            "modules" => Some(heap.alloc_simple(InvocationModuleList(self.modules.clone()))),
            "root_module_has_non_dev_dependency" => Some(
                heap.alloc(
                    self.root_usage
                        .is_some_and(RootModuleExtensionUsage::has_non_dev_dependency),
                ),
            ),
            "os" => self.os.clone().map(|os| heap.alloc_simple(os)),
            _ => None,
        }
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(invocation_context_methods)
    }
}

#[starlark_module]
fn invocation_context_methods(builder: &mut MethodsBuilder) {
    fn watch<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] label: Value<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let watch = this.selected_watch.as_ref().ok_or_else(|| {
            anyhow::anyhow!("module_ctx.watch is only supported for a selected module extension")
        })?;
        let label = StarlarkLabel::from_value(label)
            .ok_or_else(|| anyhow::anyhow!("module_ctx.watch requires a Label"))?;
        watch.call(label.canonical())?;
        Ok(Value::new_none())
    }
    fn path<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] path: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        if this.selected_download.is_none() && this.selected_path.is_none() {
            anyhow::bail!("module_ctx.path is only supported for a selected module extension");
        }
        if let Some(path) = path.unpack_str() {
            return Ok(eval.heap().alloc_simple(SelectedModuleScratchPath {
                relative: selected_scratch_relative(path)?,
                owner: this.owner.clone(),
            }));
        }
        let label = StarlarkLabel::from_value(path)
            .ok_or_else(|| anyhow::anyhow!("module_ctx.path requires a string or Label"))?;
        let selected_path = this.selected_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "module_ctx.path Label is only supported for a selected module extension"
            )
        })?;
        let path = selected_path.call(label.canonical())?;
        Ok(eval.heap().alloc_simple(SelectedModuleLabelPath(path)))
    }
    fn download<'v>(
        this: Value<'v>,
        #[starlark(require = named)] url: &str,
        #[starlark(require = named)] output: Value<'v>,
        #[starlark(require = named)] sha256: &str,
        #[starlark(require = named, default = false)] executable: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let download = this.selected_download.as_ref().ok_or_else(|| {
            anyhow::anyhow!("module_ctx.download is only supported for a selected module extension")
        })?;
        let path = SelectedModuleScratchPath::from_value(output).ok_or_else(|| {
            anyhow::anyhow!("module_ctx.download output must be a module_ctx.path value")
        })?;
        if !Arc::ptr_eq(&this.owner, &path.owner) {
            anyhow::bail!("module_ctx.download output belongs to another module_ctx");
        }
        let request = Arc::new(SelectedExtensionDownloadRequest::new(
            download.workspace.dupe(),
            download.selected_owner.clone(),
            path.relative.clone(),
            url,
            selected_download_sha256(sha256)?,
            executable,
        )?);
        download.call(request.clone())?;
        let heap = eval.heap();
        Ok(heap.alloc(AllocStruct([
            ("success", heap.alloc(true)),
            ("sha256", heap.alloc(request.sha256_hex())),
            ("integrity", heap.alloc(request.sha256_sri())),
        ])))
    }
    fn extension_metadata<'v>(
        this: Value<'v>,
        #[starlark(require = named)] root_module_direct_deps: Option<Value<'v>>,
        #[starlark(require = named)] root_module_direct_dev_deps: Option<Value<'v>>,
        #[starlark(require = named, default = false)] reproducible: bool,
        #[starlark(require = named)] facts: Option<Value<'v>>,
    ) -> anyhow::Result<StarlarkModuleExtensionMetadata> {
        InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let facts = metadata_facts(facts)?;
        let (regular, dev) =
            metadata_selection(root_module_direct_deps, root_module_direct_dev_deps)?;
        Ok(StarlarkModuleExtensionMetadata(
            ModuleExtensionMetadata::new(regular, dev, reproducible, facts),
        ))
    }
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
            heap.alloc(AllocImmutableList(
                self.tags.iter().filter(|tag| tag.class == name).cloned(),
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
                    MAX_INVOCATION_TAGS_PER_MODULE,
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
    use std::path::PathBuf;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use sha2::Digest;
    use sha2::Sha256;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::HostRepositoryMaterializationDisposition;
    use slug_bzlmod_v2::HostRootRepositoryMappingKey;
    use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryFileUrl;
    use slug_bzlmod_v2::RegistryIo;
    use slug_bzlmod_v2::RegistryIoOutcome;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryTransportError;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryEnvironmentCell;
    use slug_bzlmod_v2::RepositoryEnvironmentEntry;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::RootRepositoryRouteObservationKey;
    use slug_bzlmod_v2::host_repository_materialization_request;
    use slug_bzlmod_v2::install_registry_io;
    use slug_identity_v2::ApparentRepoName;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochError;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use slug_workspace_v2::path_observation_shards;
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

    fn metadata_call(
        source: &str,
        context: InvocationContext,
    ) -> Result<ModuleExtensionMetadata, String> {
        let module = Module::new();
        let ast = AstModule::parse("test.bzl", source.to_owned(), &Dialect::Standard).unwrap();
        let mut evaluator = Evaluator::new(&module);
        evaluator
            .eval_module(ast, &Globals::standard())
            .map_err(|error| error.to_string())?;
        let function = module.get("f").unwrap();
        let context = module.heap().alloc_simple(context);
        let value = evaluator
            .eval_function(function, &[context], &[])
            .map_err(|error| error.to_string())?;
        returned_metadata(value).map_err(|error| error.to_string())
    }

    #[test]
    fn immutable_lists_and_foreign_tags_fail_closed() {
        let owner = Arc::new(());
        let context = InvocationContext {
            modules: Arc::from([empty_module(&owner)]),
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
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
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
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
                    module
                        .heap()
                        .alloc(AllocImmutableList(tags.iter().cloned())),
                ]
            },
        )
        .unwrap();
        assert_eq!(value, "[\"root\", \"v1\", \"v0\"]");
        for (source, expected) in [
            (
                "def f(modules, tags):\n  return modules[-2]\n",
                "list index out of range",
            ),
            ("def f(modules, tags):\n  return tags[-3]\n", "out of bound"),
        ] {
            let error = call(source, |module| {
                vec![
                    module.heap().alloc_simple(modules.clone()),
                    module
                        .heap()
                        .alloc(AllocImmutableList(tags.iter().cloned())),
                ]
            })
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                tags: Arc::from([
                    tag(&owner, 0, false, &frozen_heap),
                    tag(&owner, 1, true, &frozen_heap),
                ]),
                ..empty_module(&owner)
            }]),
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
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
    fn native_immutable_tag_views_concat_to_fresh_mutable_list() {
        let owner = Arc::new(());
        let frozen_heap = FrozenHeap::new();
        let tags: Arc<[InvocationTag]> = (0..3)
            .map(|index| {
                let mut value = tag(&owner, index, false, &frozen_heap);
                value.class = if index == 2 {
                    "from_specs".into()
                } else {
                    "from_cargo".into()
                };
                value
            })
            .collect::<Vec<_>>()
            .into();
        let context = InvocationContext {
            modules: Arc::from([InvocationModule {
                tag_classes: Arc::from([
                    CompactString::from("from_cargo"),
                    CompactString::from("from_specs"),
                ]),
                tags,
                ..empty_module(&owner)
            }]),
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
            owner,
        };
        let result = call(
            "def f(ctx):\n  tags = ctx.modules[0].tags\n  left = tags.from_cargo\n  right = tags.from_specs\n  joined = left + right\n  joined.append('tail')\n  return [joined[0].value, joined[1].value, joined[2].value, joined[3], len(left), len(right)]\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        )
        .unwrap();
        assert_eq!(result, "[\"v0\", \"v1\", \"v2\", \"tail\", 2, 1]");

        let result = call(
            "def f(ctx):\n  tags = ctx.modules[0].tags\n  return [len(tags.from_specs + tags.from_cargo), len(tags.from_cargo)]\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        )
        .unwrap();
        assert_eq!(result, "[3, 2]");

        let error = call(
            "def f(ctx):\n  tags = ctx.modules[0].tags\n  left = tags.from_cargo\n  left += tags.from_specs\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        )
        .unwrap_err();
        assert!(error.contains("Immutable"), "{error}");

        let error = call(
            "def f(ctx):\n  return ctx.modules[0].tags.from_cargo + ()\n",
            |module| vec![module.heap().alloc_simple(context)],
        )
        .unwrap_err();
        assert!(error.contains("list") && error.contains("tuple"), "{error}");
    }

    #[test]
    fn invocation_tag_count_preflight_covers_entire_legacy_batch() {
        assert!(
            validate_invocation_tag_counts(
                [1, MAX_INVOCATION_TAGS_PER_MODULE],
                MAX_INVOCATION_TAGS_PER_MODULE
            )
            .is_ok()
        );
        let error = validate_invocation_tag_counts(
            [1, MAX_INVOCATION_TAGS_PER_MODULE + 1],
            MAX_INVOCATION_TAGS_PER_MODULE,
        )
        .unwrap_err();
        assert!(error.contains("more than 1048576 tags"), "{error}");
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
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
            owner,
        };
        for name in [
            "facts",
            "is_isolated",
            "wait",
            "download_and_extract",
            "extract",
            "file",
            "getenv",
            "read",
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
        let label = StarlarkLabel::new(CanonicalLabel::parse("@@//:watched").unwrap());
        let error = call(
            "def f(ctx, label):\n  return ctx.watch(label)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(label),
                ]
            },
        )
        .unwrap_err();
        assert!(
            error.contains("only supported for a selected module extension"),
            "{error}"
        );
        for source in [
            "def f(ctx):\n  return ctx.path('cargo-bazel')\n",
            "def f(ctx):\n  return ctx.download(url='file:///artifact', output='cargo-bazel', sha256='0000000000000000000000000000000000000000000000000000000000000000')\n",
        ] {
            let error = call(source, |module| {
                vec![module.heap().alloc_simple(context.clone())]
            })
            .unwrap_err();
            assert!(
                error.contains("only supported for a selected module extension"),
                "{error}"
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
            root_usage: None,
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
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
        assert_eq!(
            call("def f(label):\n  return label.workspace_root\n", |module| {
                vec![module.heap().alloc_simple(label.clone())]
            })
            .unwrap(),
            "\"\""
        );
        assert!(
            call("def f(label):\n  return label.relative\n", |module| {
                vec![module.heap().alloc_simple(label)]
            })
            .is_err()
        );
    }

    #[test]
    fn extension_metadata_complete_constructor_and_default_return_are_retained() {
        let owner = Arc::new(());
        let context = InvocationContext {
            modules: Arc::from([empty_module(&owner)]),
            root_usage: Some(RootModuleExtensionUsage::new(true, true)),
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
            owner,
        };
        let none = metadata_call("def f(ctx):\n  return None\n", context.clone()).unwrap();
        let default = metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata()\n",
            context.clone(),
        )
        .unwrap();
        assert_eq!(none, default);

        let metadata = metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps=('regular',), root_module_direct_dev_deps=['dev'], reproducible=True, facts={'z': (1, 2.5), 'a': {'n': None, 'b': True}})\n",
            context.clone(),
        )
        .unwrap();
        assert!(metadata.reproducible());
        assert!(metadata.root_module_direct_deps().contains("regular"));
        assert!(metadata.root_module_direct_dev_deps().contains("dev"));
        assert_eq!(
            metadata
                .facts()
                .values()
                .keys()
                .map(|value| value.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert!(matches!(
            metadata.facts().values().get("z"),
            Some(FactValue::List(values)) if values.len() == 2
        ));
        let arbitrary = metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata(facts={'n': 123456789012345678901234567890})\n",
            context.clone(),
        )
        .unwrap();
        assert!(matches!(
            arbitrary.facts().values().get("n"),
            Some(FactValue::Number(FactNumber::Integer(value)))
                if value == "123456789012345678901234567890"
        ));
        for source in [
            "def f(ctx):\n  return ctx.extension_metadata(facts={1: 'bad'})\n",
            "def f(ctx):\n  return ctx.extension_metadata(facts={'x': set()})\n",
            "def f(ctx):\n  return ctx.extension_metadata(facts={'x': [[[[[[[0]]]]]]]})\n",
        ] {
            assert!(metadata_call(source, context.clone()).is_err());
        }
        let error = metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps='all', root_module_direct_dev_deps=(), facts={1: 'bad'})\n",
            context.clone(),
        )
        .unwrap_err();
        assert!(error.contains("Facts keys must be strings"), "{error}");
        let float_module = Module::new();
        let non_finite = float_module.heap().alloc(StarlarkFloat(f64::INFINITY));
        assert!(
            metadata_fact_value(non_finite, 6, &mut SmallSet::new())
                .unwrap_err()
                .to_string()
                .contains("finite")
        );

        for source in [
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps='all', root_module_direct_dev_deps=())\n",
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps='all', root_module_direct_dev_deps=None)\n",
        ] {
            assert!(
                metadata_call(source, context.clone())
                    .unwrap_err()
                    .contains("empty list")
            );
        }
        assert!(matches!(
            metadata_call(
                "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps='all', root_module_direct_dev_deps=[])\n",
                context.clone(),
            )
            .unwrap()
            .root_module_direct_deps(),
            ModuleExtensionRepositorySelection::All
        ));
        assert!(metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps=['same'], root_module_direct_dev_deps=['same'])\n",
            context.clone(),
        )
        .unwrap_err()
        .contains("also in root_module_direct_deps"));
        assert!(metadata_call(
            "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps=['dup', 'dup'], root_module_direct_dev_deps=[])\n",
            context.clone(),
        )
        .unwrap_err()
        .contains("duplicate entry"));
        for invalid in ["+canonical", ".leading", "under+plus"] {
            let source = format!(
                "def f(ctx):\n  return ctx.extension_metadata(root_module_direct_deps=['{invalid}'], root_module_direct_dev_deps=[])\n"
            );
            assert!(
                metadata_call(&source, context.clone())
                    .unwrap_err()
                    .contains("invalid user-provided repo name")
            );
        }
        assert!(
            metadata_call("def f(ctx):\n  return 1\n", context)
                .unwrap_err()
                .contains("must return None or extension_metadata")
        );
    }

    #[test]
    fn module_context_root_non_dev_field_uses_retained_root_proxies() {
        let owner = Arc::new(());
        for (usage, expected) in [
            (None, "False"),
            (Some(RootModuleExtensionUsage::new(false, true)), "False"),
            (Some(RootModuleExtensionUsage::new(true, false)), "True"),
        ] {
            let context = InvocationContext {
                modules: Arc::from([empty_module(&owner)]),
                root_usage: usage,
                os: None,
                selected_download: None,
                selected_watch: None,
                selected_path: None,
                owner: owner.clone(),
            };
            assert_eq!(
                call(
                    "def f(ctx):\n  return ctx.root_module_has_non_dev_dependency\n",
                    |module| vec![module.heap().alloc_simple(context)],
                )
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn selected_module_os_exposes_only_declared_membership_and_get() {
        let owner = Arc::new(());
        let context = InvocationContext::new_modules(
            Arc::from([empty_module(&owner)]),
            &owner,
            None,
            Some(ModuleExtensionOs {
                platform: RepositoryPlatform::new("linux", "x86_64"),
                environment: Arc::from([
                    (CompactString::new("ABSENT"), None),
                    (CompactString::new("EMPTY"), Some(Arc::from(""))),
                    (
                        CompactString::new("PRESENT"),
                        Some(Arc::from("secret-marker")),
                    ),
                ]),
            }),
        );
        assert_eq!(
            call(
                "def f(ctx):\n  e=ctx.os.environ\n  return [ctx.os.name, ctx.os.arch, 'ABSENT' in e, 'EMPTY' in e, 'PRESENT' in e, e.get('ABSENT'), e.get('ABSENT', 'fallback'), e.get('EMPTY'), e.get('PRESENT')]\n",
                |module| vec![module.heap().alloc_simple(context.clone())],
            )
            .unwrap(),
            "[\"linux\", \"x86_64\", False, True, True, None, \"fallback\", \"\", \"secret-marker\"]"
        );
        for source in [
            "def f(ctx):\n  return 'UNDECLARED' in ctx.os.environ\n",
            "def f(ctx):\n  return ctx.os.environ.get('UNDECLARED')\n",
        ] {
            let error = call(source, |module| {
                vec![module.heap().alloc_simple(context.clone())]
            })
            .unwrap_err();
            assert!(
                error.contains("undeclared module extension environment name"),
                "{error}"
            );
            assert!(!error.contains("secret-marker"), "{error}");
        }
        for source in [
            "def f(ctx):\n  return ctx.os.environ['PRESENT']\n",
            "def f(ctx):\n  return len(ctx.os.environ)\n",
            "def f(ctx):\n  return ctx.os.environ.keys()\n",
        ] {
            assert!(
                call(source, |module| {
                    vec![module.heap().alloc_simple(context.clone())]
                })
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
            root_usage: Some(RootModuleExtensionUsage::new(true, false)),
            os: None,
            selected_download: None,
            selected_watch: None,
            selected_path: None,
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
        tag_limit: usize,
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
                Ok((preflight, observations)) => invoke_pure_preflight(self.prepared.clone(), preflight, observations, true, &mut events, self.tag_limit),
                Err(terminal) => terminal,
            };
            if self.tag_limit != MAX_INVOCATION_TAGS_PER_MODULE { assert!(events.events().is_empty(), "oversized later module emitted earlier invocation events"); }
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

    #[tokio::test]
    async fn oversized_later_legacy_tag_stops_before_earlier_invocation() {
        let module = "module(name='bazel_tools')\na=use_extension('//:first.bzl','first')\nb=use_extension('//:second.bzl','second')\nb.tag(value='second')\n";
        let first = "def impl(ctx):\n    print('first-invoked')\nfirst=module_extension(implementation=impl)\n";
        let second = "tag=tag_class(attrs={'value':attr.string()})\ndef impl(ctx):\n    print('second-invoked')\nsecond=module_extension(implementation=impl,tag_classes={'tag':tag})\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let mut transaction = repository_rule_sources_transaction(
            &dice,
            module,
            &[("first.bzl", Some(first)), ("second.bzl", Some(second))],
            0,
            None,
        )
        .await;
        let prepared = transaction
            .compute(&HostPreparedModuleExtensionInputsObservationKey::new(
                workspace.dupe(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(prepared)) = prepared else {
            panic!("legacy prepared inputs did not complete");
        };
        let prepared = Arc::new(prepared.result().as_ref().as_ref().unwrap().clone());
        assert_eq!(
            prepared
                .inputs
                .iter()
                .map(|input| input.tags.len())
                .collect::<Vec<_>>(),
            [0, 1]
        );
        let outcome = transaction
            .compute(&ObservePreparedKey {
                workspace,
                prepared,
                observations: PathObservationEpoch::empty(),
                id: 71,
                tag_limit: 0,
            })
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("oversized later legacy module did not fail closed");
        };
        let error = observed.result().as_ref().as_ref().unwrap_err();
        let HostPureModuleExtensionInvocationsError::AfterPrepared {
            request,
            completed,
            current_calls,
            error: HostPureModuleExtensionInvocationError::Invocation(message),
            ..
        } = error
        else {
            panic!("oversized later legacy module had the wrong terminal: {error:?}");
        };
        assert!(request.is_none());
        assert!(completed.is_empty());
        assert!(current_calls.is_empty());
        assert!(message.contains("more than 0 tags"));
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
            let observed_injected = injected.compute(&ObservePreparedKey { workspace: workspace.dupe(), prepared: prepared_value.dupe(), observations: PathObservationEpoch::empty(), id, tag_limit: MAX_INVOCATION_TAGS_PER_MODULE }).await.unwrap();
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
        let source = |local: bool, configure: bool, environment: &str, policy: &str| {
            format!(
                "repo=repository_rule(lambda ctx: None, attrs={{'build_file':attr.label({policy})}}, local={local}, configure={configure}, environ={environment})\n\
                 def impl(ctx):\n    repo(name='generated')\n\
                 ext=module_extension(implementation=impl)\n",
                local = if local { "True" } else { "False" },
                configure = if configure { "True" } else { "False" },
            )
        };
        let a_source = source(true, true, "['B','A','B']", "allow_single_file=True");
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
        assert!(definition.attributes[0].file_admissibility.is_any_file());
        assert!(
            definition.attributes[0]
                .file_admissibility
                .single_artifact()
        );
        assert_eq!(
            definition
                .environment
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["B", "A"]
        );

        for changed_source in [
            source(false, true, "['B','A']", "allow_single_file=True"),
            source(true, false, "['B','A']", "allow_single_file=True"),
            source(true, true, "['B','C']", "allow_single_file=True"),
            source(true, true, "['B','A']", "allow_single_file=False"),
            source(true, true, "['B','A']", "allow_files=['.b','.a']"),
            source(true, true, "['B','A']", "allow_files=['.a','.b']"),
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

    struct SelectedFactorRegistryIo;

    #[async_trait]
    impl RegistryIo for SelectedFactorRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            let bytes: Option<&'static [u8]> = match url.as_str() {
                "https://registry.invalid/modules/dep/1/MODULE.bazel" => {
                    Some(b"module(name='dep', version='1', repo_name='dep_alias')\n")
                }
                "https://registry.invalid/modules/dep/1/source.json" => {
                    Some(br#"{"url":"https://origin.invalid/dep.tgz","integrity":"sha256-test"}"#)
                }
                _ => None,
            };
            Ok(bytes.map_or(RegistryIoOutcome::NotFound, |bytes| {
                RegistryIoOutcome::Found(Arc::from(bytes))
            }))
        }
    }

    fn selected_factor_epoch(module_source: &str, extension_source: &str) -> PathObservationEpoch {
        fn add_lstat(
            rows: &mut Vec<(PathObservationDemand, Arc<PathObservationResult>)>,
            namespace: &PathObservationNamespace,
            path: &str,
            result: PathOperationResult<PathLstat>,
        ) {
            rows.push((
                PathObservationDemand::new(
                    namespace.clone(),
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                Arc::new(PathObservationResult::Lstat(result)),
            ));
        }
        let instance = PathObservationInstanceId::new(77);
        let namespace = PathObservationNamespace::Materialization(instance);
        let mut rows = repository_rule_epoch(module_source, &[], 0)
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .collect::<Vec<_>>();
        let node =
            |kind, id| PathOperationResult::Present(PathLstat::new(kind, id, 1, 1, 1, 0o755));
        add_lstat(
            &mut rows,
            &namespace,
            "/",
            node(PathNodeKind::Directory, 80),
        );
        add_lstat(
            &mut rows,
            &namespace,
            "/registry-dep",
            node(PathNodeKind::Directory, 81),
        );
        add_lstat(
            &mut rows,
            &namespace,
            "/registry-dep/REPO.bazel",
            PathOperationResult::Missing,
        );
        add_lstat(
            &mut rows,
            &namespace,
            "/registry-dep/.bazelignore",
            PathOperationResult::Missing,
        );
        for (index, (path, bytes)) in [
            (
                "/registry-dep/MODULE.bazel",
                "module(name='dep', version='1', repo_name='dep_alias')\n",
            ),
            ("/registry-dep/BUILD.bazel", ""),
            ("/registry-dep/ext.bzl", extension_source),
        ]
        .into_iter()
        .enumerate()
        {
            add_lstat(
                &mut rows,
                &namespace,
                path,
                node(PathNodeKind::RegularFile, 82 + index as i64),
            );
            rows.push((
                PathObservationDemand::new(
                    namespace.clone(),
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                Arc::new(PathObservationResult::FileBytes(
                    PathOperationResult::Present(Arc::from(bytes.as_bytes())),
                )),
            ));
        }
        PathObservationEpoch::from_shared(rows).unwrap()
    }

    fn selected_watch_epoch_named(
        module_source: &str,
        extension_source: &str,
        name: &str,
        content: Option<&[u8]>,
    ) -> PathObservationEpoch {
        let path = NormalizedAbsolutePath::new(format!("{REPO_RULE_WORKSPACE}/{name}")).unwrap();
        let demand = |operation| {
            PathObservationDemand::new(PathObservationNamespace::Host, path.dupe(), operation)
        };
        let lstat = content.map_or(PathOperationResult::Missing, |bytes| {
            PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                bytes.len() as i64,
                1,
                1,
                301,
                0o644,
            ))
        });
        let digest = content.map_or(PathOperationResult::Missing, |bytes| {
            PathOperationResult::Present(
                slug_workspace_v2::FileContentDigest::new(
                    Sha256::digest(bytes).into(),
                    bytes.len() as u64,
                )
                .unwrap(),
            )
        });
        PathObservationEpoch::from_shared(
            selected_factor_epoch(module_source, extension_source)
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain([
                    (
                        demand(PathObservationOperation::Lstat),
                        Arc::new(PathObservationResult::Lstat(lstat)),
                    ),
                    (
                        demand(PathObservationOperation::FileDigest),
                        Arc::new(PathObservationResult::FileDigest(digest)),
                    ),
                ]),
        )
        .unwrap()
    }

    fn selected_watch_epoch(
        module_source: &str,
        extension_source: &str,
        content: Option<&[u8]>,
    ) -> PathObservationEpoch {
        selected_watch_epoch_named(module_source, extension_source, "watch.txt", content)
    }

    fn selected_path_package_epoch(
        module_source: &str,
        extension_source: &str,
        watch: bool,
        package_exists: bool,
    ) -> PathObservationEpoch {
        let base = if watch {
            selected_watch_epoch(module_source, extension_source, Some(b"watched"))
        } else {
            selected_factor_epoch(module_source, extension_source)
        };
        let marker = if package_exists {
            PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                0,
                1,
                1,
                602,
                0o644,
            ))
        } else {
            PathOperationResult::Missing
        };
        let lstat = |relative: &str, result| {
            (
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{REPO_RULE_WORKSPACE}/{relative}"))
                        .unwrap(),
                    PathObservationOperation::Lstat,
                ),
                Arc::new(PathObservationResult::Lstat(result)),
            )
        };
        PathObservationEpoch::from_shared(
            base.observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain([
                    lstat(
                        "pkg",
                        PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::Directory,
                            0,
                            1,
                            1,
                            601,
                            0o755,
                        )),
                    ),
                    lstat("pkg/BUILD.bazel", marker),
                    lstat("pkg/BUILD", PathOperationResult::Missing),
                ]),
        )
        .unwrap()
    }

    fn selected_external_path_absent_package_epoch(
        module_source: &str,
        extension_source: &str,
    ) -> PathObservationEpoch {
        let namespace =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(77));
        let lstat = |relative: &str, result| {
            (
                PathObservationDemand::new(
                    namespace.clone(),
                    NormalizedAbsolutePath::new(format!("/registry-dep/{relative}")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                Arc::new(PathObservationResult::Lstat(result)),
            )
        };
        PathObservationEpoch::from_shared(
            selected_factor_epoch(module_source, extension_source)
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain([
                    lstat(
                        "pkg",
                        PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::Directory,
                            0,
                            1,
                            1,
                            701,
                            0o755,
                        )),
                    ),
                    lstat("pkg/BUILD.bazel", PathOperationResult::Missing),
                    lstat("pkg/BUILD", PathOperationResult::Missing),
                ]),
        )
        .unwrap()
    }

    async fn selected_watch_observations(
        transaction: dice::DiceTransaction,
        epoch: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let mut updater = transaction.into_updater();
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to([(PathObservationEpochKey, epoch)])
            .unwrap();
        updater.commit().await
    }

    fn selected_watch_symlink_epoch(
        module_source: &str,
        extension_source: &str,
        ancestor: bool,
        dangling: bool,
    ) -> PathObservationEpoch {
        let demand = |name: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("{REPO_RULE_WORKSPACE}/{name}")).unwrap(),
                operation,
            )
        };
        let lstat = |kind, size, id| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, size, 1, 1, id, 0o755,
            )))
        };
        let mut rows = selected_factor_epoch(module_source, extension_source)
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .collect::<Vec<_>>();
        let link = if ancestor { "nested" } else { "watch.txt" };
        let target = if ancestor { "actual" } else { "actual.txt" };
        rows.push((
            demand(link, PathObservationOperation::Lstat),
            Arc::new(lstat(PathNodeKind::Symlink, target.len() as i64, 401)),
        ));
        rows.push((
            demand(link, PathObservationOperation::ReadLink),
            Arc::new(PathObservationResult::ReadLink(
                PathOperationResult::Present(Arc::new(PathBuf::from(target))),
            )),
        ));
        if ancestor {
            rows.push((
                demand("actual", PathObservationOperation::Lstat),
                Arc::new(lstat(PathNodeKind::Directory, 0, 402)),
            ));
        }
        rows.push((
            demand(
                if ancestor {
                    "actual/watch.txt"
                } else {
                    "actual.txt"
                },
                PathObservationOperation::Lstat,
            ),
            Arc::new(if dangling {
                PathObservationResult::Lstat(PathOperationResult::Missing)
            } else {
                lstat(PathNodeKind::RegularFile, 1, 403)
            }),
        ));
        PathObservationEpoch::from_shared(rows).unwrap()
    }

    fn selected_watch_directory_epoch(
        module_source: &str,
        extension_source: &str,
    ) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(
            selected_factor_epoch(module_source, extension_source)
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain([(
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("{REPO_RULE_WORKSPACE}/watch.txt"))
                            .unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                        PathLstat::new(PathNodeKind::Directory, 0, 1, 1, 501, 0o755),
                    ))),
                )]),
        )
        .unwrap()
    }

    async fn selected_factor_transaction(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        tracker: Arc<RepositoryRuleActivationTracker>,
        expect_supported: bool,
    ) -> (dice::DiceTransaction, Arc<HostSelectedExtensionOwner>) {
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let transaction =
            repository_rule_sources_transaction(dice, module_source, &[], 0, Some(tracker)).await;
        let epoch = selected_factor_epoch(module_source, extension_source);
        let mut updater = transaction.into_updater();
        updater.changed_to(path_observation_shards(&epoch)).unwrap();
        updater
            .changed_to([(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let route = transaction
            .compute(
                &RootRepositoryRouteObservationKey::for_root_build(
                    workspace.dupe(),
                    ApparentRepoName::new("dep_alias").unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(route)) = route else {
            panic!("selected registry route did not complete: {route:?}")
        };
        let route = route
            .result()
            .as_ref()
            .as_ref()
            .unwrap_or_else(|error| panic!("selected registry route failed: {error}"));
        let HostRepositoryMaterializationDisposition::Request(request) =
            host_repository_materialization_request(&route.source_capability()).unwrap()
        else {
            panic!("selected registry route did not request materialization")
        };
        let mut updater = transaction.into_updater();
        updater
            .changed_to([(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.dupe(),
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Immutable {
                                source_identity: Arc::from("sha256-test"),
                                generation_root: PathBuf::from("/registry-dep"),
                                observation_instance: PathObservationInstanceId::new(77),
                            },
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let mapping = transaction
            .compute(&HostRootRepositoryMappingKey::new(workspace.dupe()))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(mapping) = mapping else {
            panic!("selected mapping returned Need")
        };
        let requested = mapping
            .as_ref()
            .as_ref()
            .unwrap()
            .view()
            .unwrap()
            .mapping()
            .find(|(name, _)| name.as_str() == "proof")
            .unwrap()
            .1
            .clone();
        let demand = transaction
            .compute(&HostSelectedExtensionDemandKey::new(
                workspace.dupe(),
                requested,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(demand) = demand else {
            panic!("selected demand returned Need")
        };
        let owner = demand.as_ref().as_ref().unwrap().owner().clone();
        let inputs = transaction
            .compute(&HostSelectedExtensionOwnerInputsKey::new(
                workspace.dupe(),
                owner.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(inputs) = inputs else {
            panic!("selected owner inputs returned Need")
        };
        let inputs = inputs.as_ref().as_ref().unwrap();
        let route = RootRepositoryRoute::for_selected_extension_definition(
            workspace.dupe(),
            inputs.request().source(),
        )
        .expect("fixture must use the selected external registry route");
        let target =
            RootPackageBzlTarget::parse(inputs.request().parts().0.target().as_str()).unwrap();
        let label = RepositoryBzlLabel::new(
            inputs.request().parts().0.package().package().clone(),
            target,
        )
        .unwrap();
        let bzl = transaction
            .compute(&ExternalBzlModuleEvalKey::new_bzlmod(route, label))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(bzl) = bzl else {
            panic!("selected external Bzl returned Need")
        };
        if expect_supported {
            let projection =
                owner_projection(bzl.as_ref().as_ref().unwrap(), inputs.request()).unwrap();
            assert_eq!(
                projection.definition.environment.as_ref(),
                ["FACTOR", "MISSING", "FACTOR"]
            );
            let mut reordered = projection.clone();
            reordered.definition.environment = Arc::from([
                CompactString::new("MISSING"),
                CompactString::new("FACTOR"),
                CompactString::new("FACTOR"),
            ]);
            assert_ne!(projection, reordered);
            reordered.definition.environment = projection.definition.environment.clone();
            assert_eq!(projection, reordered);
        } else {
            assert_eq!(
                owner_projection(bzl.as_ref().as_ref().unwrap(), inputs.request()).unwrap_err(),
                "selected module extension has unsupported factors"
            );
        }
        (transaction, owner)
    }

    async fn selected_factor_host_inputs(
        transaction: dice::DiceTransaction,
        tracker: Arc<RepositoryRuleActivationTracker>,
        platform: RepositoryPlatform,
        snapshot: RepositoryEnvironmentSnapshot,
        frontier: RepositoryEnvironmentNameFrontier,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        selected_factor_host_inputs_with_download(
            transaction,
            tracker,
            platform,
            snapshot,
            frontier,
            SelectedExtensionDownloadTransaction::empty(workspace),
            None,
        )
        .await
    }

    async fn selected_path_case(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        tracker: Arc<RepositoryRuleActivationTracker>,
    ) -> (
        dice::DiceTransaction,
        HostSelectedExtensionOwnerPureObservationKey,
    ) {
        let (base, owner) =
            selected_factor_transaction(dice, module, extension, tracker.clone(), true).await;
        let transaction = selected_factor_host_inputs(
            base,
            tracker,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
        )
        .await;
        let key = HostSelectedExtensionOwnerPureObservationKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            owner,
        );
        (transaction, key)
    }

    async fn selected_factor_host_inputs_with_download(
        transaction: dice::DiceTransaction,
        tracker: Arc<RepositoryRuleActivationTracker>,
        platform: RepositoryPlatform,
        snapshot: RepositoryEnvironmentSnapshot,
        frontier: RepositoryEnvironmentNameFrontier,
        download: SelectedExtensionDownloadTransaction,
        epoch: Option<(
            Arc<HostSelectedExtensionOwner>,
            SelectedExtensionDownloadEpoch,
        )>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let mut data = UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(RepositoryHostInputTransaction::new(
            snapshot.clone(),
            frontier.clone(),
        ));
        data.data.set(download);
        let mut updater = transaction.into_updater();
        updater
            .changed_to([(RepositoryPlatformKey::new(workspace.dupe()), platform)])
            .unwrap();
        updater
            .changed_to(
                frontier
                    .iter()
                    .map(|name| {
                        (
                            RepositoryEnvironmentCellKey::new(workspace.dupe(), name.clone()),
                            RepositoryEnvironmentCell::observed(snapshot.get(name).cloned()),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        if let Some((owner, epoch)) = epoch {
            updater
                .changed_to([(
                    SelectedExtensionDownloadEpochKey::new(workspace, owner),
                    epoch,
                )])
                .unwrap();
        }
        updater.commit_with_data(data).await
    }

    #[tokio::test]
    async fn selected_external_owner_concatenates_native_immutable_tag_views() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.from_cargo(value='left')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs = {'value': attr.string()})
from_cargo = tag_class(attrs = {'value': attr.string()})
from_specs = tag_class(attrs = {'value': attr.string()})
def impl(ctx):
    for mod in ctx.modules:
        for cfg in mod.tags.from_cargo + mod.tags.from_specs:
            repo(name = 'proof', value = cfg.value)
ext = module_extension(
    implementation = impl,
    tag_classes = {'from_cargo': from_cargo, 'from_specs': from_specs},
    environ = ['FACTOR', 'MISSING', 'FACTOR'],
)
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let inputs = base
            .compute(&HostSelectedExtensionOwnerInputsKey::new(
                workspace.dupe(),
                owner.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(inputs) = inputs else {
            panic!("selected owner inputs did not complete");
        };
        let inputs = inputs.as_ref().as_ref().unwrap();
        let route = RootRepositoryRoute::for_selected_extension_definition(
            workspace.dupe(),
            inputs.request().source(),
        )
        .unwrap();
        let target =
            RootPackageBzlTarget::parse(inputs.request().parts().0.target().as_str()).unwrap();
        let label = RepositoryBzlLabel::new(
            inputs.request().parts().0.package().package().clone(),
            target,
        )
        .unwrap();
        let bzl = base
            .compute(&ExternalBzlModuleEvalKey::new_bzlmod(route, label))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(bzl) = bzl else {
            panic!("selected external Bzl did not complete");
        };
        let projection =
            owner_projection(bzl.as_ref().as_ref().unwrap(), inputs.request()).unwrap();
        let error = owner_invocation_modules(inputs, &projection, &Arc::new(()), 0).unwrap_err();
        assert!(error.contains("more than 0 tags"), "{error}");
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
        )
        .await;
        let key = HostSelectedExtensionOwnerPureObservationKey::new(workspace, owner);
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("selected tag-list concat did not complete");
        };
        let calls = &observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .receipt
            .repository_rule_calls;
        assert_eq!(calls.len(), 1);
        let (_, crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value)) =
            calls[0]
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
        else {
            panic!("selected tag-list concat lost its tag value");
        };
        assert_eq!(value.as_str(), "left");
    }

    #[tokio::test]
    async fn selected_external_owner_tracks_declared_host_inputs_and_fails_closed() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs = {'value': attr.string()})
def impl(ctx):
    print('selected-invoked')
    environ = ctx.os.environ
    value = 'absent'
    if 'FACTOR' in environ:
        value = 'empty' if environ.get('FACTOR') == '' else environ.get('FACTOR')
    repo(name = 'proof', value = ctx.os.name + '|' + ctx.os.arch + '|' + value)
ext = module_extension(implementation = impl, environ = ['FACTOR', 'MISSING', 'FACTOR'])
"#;
        fn snapshot(entries: &[(&str, &str)]) -> RepositoryEnvironmentSnapshot {
            RepositoryEnvironmentSnapshot::from_canonical(
                entries
                    .iter()
                    .map(|(name, value)| RepositoryEnvironmentEntry::new(*name, *value))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        }
        fn value(
            outcome: &<HostSelectedExtensionOwnerPureObservationKey as Key>::Value,
        ) -> CompactString {
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("selected owner did not complete: {outcome:?}")
            };
            let result = observed.result().as_ref().as_ref().unwrap();
            let call = &result.receipt.repository_rule_calls[0];
            let (
                _,
                crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value),
            ) = call
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
            else {
                panic!("repository call did not retain string value")
            };
            value.clone()
        }
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        tracker.take();
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner);
        let empty_frontier = RepositoryEnvironmentNameFrontier::empty();
        let declared_frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot(&[("FACTOR", "A"), ("UNRELATED", "one")]),
            empty_frontier,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold declared environment names must return Need")
        };
        assert_eq!(
            need.repository_environment()
                .unwrap()
                .names()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["FACTOR", "MISSING"]
        );
        let need_rows = tracker.take();
        assert!(
            need_rows
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            need_rows
                .iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("selected-invoked"))
        );

        transaction = selected_factor_host_inputs(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot(&[("FACTOR", "A"), ("UNRELATED", "one")]),
            declared_frontier.clone(),
        )
        .await;
        let a = transaction.compute(&key).await.unwrap();
        assert_eq!(value(&a), "linux|x86_64|A");
        tracker.take();

        transaction = selected_factor_host_inputs(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot(&[("FACTOR", "A"), ("UNRELATED", "two")]),
            declared_frontier.clone(),
        )
        .await;
        assert_eq!(value(&transaction.compute(&key).await.unwrap()), value(&a));
        assert!(tracker.take().iter().any(|row| {
            row.key == key.to_string() && row.kind == ActivationKind::Reused && row.batch.is_none()
        }));

        for (entry, expected) in [
            (Some("B"), "linux|x86_64|B"),
            (Some("A"), "linux|x86_64|A"),
            (None, "linux|x86_64|absent"),
            (Some(""), "linux|x86_64|empty"),
            (None, "linux|x86_64|absent"),
        ] {
            let snapshot = snapshot(&entry.map_or(Vec::new(), |entry| vec![("FACTOR", entry)]));
            transaction = selected_factor_host_inputs(
                transaction,
                tracker.clone(),
                RepositoryPlatform::new("linux", "x86_64"),
                snapshot,
                declared_frontier.clone(),
            )
            .await;
            assert_eq!(value(&transaction.compute(&key).await.unwrap()), expected);
            tracker.take();
        }
        transaction = selected_factor_host_inputs(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("mac os x", "aarch64"),
            snapshot(&[]),
            declared_frontier.clone(),
        )
        .await;
        assert_eq!(
            value(&transaction.compute(&key).await.unwrap()),
            "mac os x|aarch64|absent"
        );
        transaction = selected_factor_host_inputs(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot(&[]),
            declared_frontier.clone(),
        )
        .await;
        assert_eq!(
            value(&transaction.compute(&key).await.unwrap()),
            "linux|x86_64|absent"
        );

        let mut updater = transaction.into_updater();
        updater
            .changed_to([(
                RepositoryEnvironmentCellKey::new(workspace.dupe(), "FACTOR"),
                RepositoryEnvironmentCell::observed(Some(Arc::from("secret-marker"))),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let error = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = error else {
            panic!("mismatched cell must be a complete semantic error")
        };
        let error = observed.result().as_ref().as_ref().unwrap_err();
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains("differs from request"), "{diagnostic}");
        assert!(!diagnostic.contains("secret-marker"), "{diagnostic}");

        let mut updater = transaction.into_updater();
        updater
            .changed_to([(
                RepositoryEnvironmentCellKey::new(workspace, "FACTOR"),
                RepositoryEnvironmentCell::Unauthorized,
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let unauthorized = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = unauthorized else {
            panic!("unauthorized cell must be a complete semantic error")
        };
        let error = observed.result().as_ref().as_ref().unwrap_err();
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains("unauthorized"), "{diagnostic}");
        assert!(!diagnostic.contains("secret-marker"), "{diagnostic}");
    }

    #[tokio::test]
    async fn selected_external_owner_watch_label_need_replay_and_content_invalidation() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:watch.txt')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None)
watch_tag = tag_class(attrs = {'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            print('before-watch')
            ctx.watch(tag.file)
            ctx.watch(tag.file)
    repo(name='proof')
    print('after-watch')
ext = module_extension(implementation=impl, tag_classes={'watch': watch_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        tracker.take();
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner);
        let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold selected watch must return Need");
        };
        assert!(need.path_observations().is_some(), "{need:?}");
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("before-watch"))
        );

        for (content, expected_kind) in [
            (Some(&b"A"[..]), ActivationKind::Evaluated),
            (Some(&b"B"[..]), ActivationKind::Evaluated),
            (Some(&b"A"[..]), ActivationKind::Evaluated),
            (None, ActivationKind::Evaluated),
            (Some(&b"A"[..]), ActivationKind::Evaluated),
        ] {
            transaction = selected_watch_observations(
                transaction,
                selected_watch_epoch(MODULE, EXTENSION, content),
            )
            .await;
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("selected watch replay did not complete: {outcome:?}");
            };
            let result = observed.result().as_ref().as_ref().unwrap();
            assert_eq!(result.receipt.repository_rule_calls.len(), 1);
            assert_eq!(result.receipt.repository_rule_calls[0].name, "proof");
            assert_eq!(
                observed
                    .observations()
                    .observations()
                    .iter()
                    .filter(|(demand, _)| {
                        demand.path().as_path().ends_with("watch.txt")
                            && demand.operation() == PathObservationOperation::FileDigest
                    })
                    .count(),
                usize::from(content.is_some())
            );
            let rows = tracker.take();
            assert!(
                rows.iter()
                    .any(|row| row.key == key.to_string() && row.kind == expected_kind),
                "{rows:?}"
            );
            assert!(
                rows.iter()
                    .flat_map(|row| row.batch.as_ref())
                    .flat_map(EventBatch::events)
                    .any(|event| format!("{event:?}").contains("after-watch"))
            );
            let warm = transaction.compute(&key).await.unwrap();
            assert!(matches!(warm, SourcePreparationOutcome::Complete(Ok(_))));
            assert!(
                tracker
                    .take()
                    .iter()
                    .any(|row| row.key == key.to_string() && row.kind == ActivationKind::Reused)
            );
        }
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_missing_target_and_lexical_child() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='//:missing')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs = {'value': attr.string()})
path_tag = tag_class(attrs = {'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.path:
            path = ctx.path(tag.file)
            repo(name = 'proof', value = str(path.dirname.get_child('bin/cargo')))
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("root Label path must resolve through the existing package: {outcome:?}");
        };
        let result = observed.result().as_ref().as_ref().unwrap();
        let calls = &result.receipt.repository_rule_calls;
        assert_eq!(calls.len(), 1);
        let (_, crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value)) =
            calls[0]
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
        else {
            panic!("derived path was not retained as a string");
        };
        assert_eq!(value.as_str(), format!("{REPO_RULE_WORKSPACE}/bin/cargo"));
        assert!(
            observed
                .observations()
                .observations()
                .iter()
                .all(|(demand, _)| { !demand.path().as_path().ends_with("missing") })
        );
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_source_complete_external_repo() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='@dep_alias//:missing')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs={'value': attr.string()})
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.path:
            p = ctx.path(tag.file)
            repo(name='proof', value=str(p.dirname.get_child('bin/cargo')))
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut transaction, key) = selected_path_case(&dice, MODULE, EXTENSION, tracker).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("source-complete external Label path must complete: {outcome:?}");
        };
        let result = observed.result().as_ref().as_ref().unwrap();
        let calls = &result.receipt.repository_rule_calls;
        assert_eq!(calls.len(), 1);
        let (_, crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value)) =
            calls[0]
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
        else {
            panic!("external Label child was not retained");
        };
        assert_eq!(value.as_str(), "/registry-dep/bin/cargo");
        assert!(
            observed
                .observations()
                .observations()
                .iter()
                .all(|(demand, _)| { !demand.path().as_path().ends_with("missing") })
        );
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_cold_need_replays_without_partial_effects() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:watch.txt')\ne.path(file='//pkg:missing')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None)
watch_tag = tag_class(attrs={'file': attr.label()})
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    print('before-path')
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            ctx.watch(tag.file)
    repo(name='prefix')
    for mod in ctx.modules:
        for tag in mod.tags.path:
            ctx.path(tag.file)
    repo(name='proof')
    print('after-path')
ext = module_extension(implementation=impl, tag_classes={'watch': watch_tag, 'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (transaction, key) =
            selected_path_case(&dice, MODULE, EXTENSION, tracker.clone()).await;
        let mut transaction = selected_watch_observations(
            transaction,
            selected_watch_epoch(MODULE, EXTENSION, Some(b"watched")),
        )
        .await;
        tracker.take();
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold subpackage path must request observations");
        };
        assert!(need.path_observations().is_some(), "{need:?}");
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("before-path"))
        );

        transaction = selected_watch_observations(
            transaction,
            selected_path_package_epoch(MODULE, EXTENSION, true, true),
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("subpackage replay must complete: {outcome:?}");
        };
        let calls = &observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .receipt
            .repository_rule_calls;
        assert_eq!(
            calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            ["prefix", "proof"]
        );
        assert!(
            observed
                .observations()
                .observations()
                .iter()
                .all(|(demand, _)| { !demand.path().as_path().ends_with("pkg/missing") })
        );
        let rows = tracker.take();
        let events = rows
            .iter()
            .filter(|row| row.key == key.to_string())
            .flat_map(|row| row.batch.as_ref())
            .flat_map(EventBatch::events)
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.contains("before-path"))
                .count(),
            1,
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.contains("after-path"))
                .count(),
            1,
            "{events:?}"
        );
        let warm = transaction.compute(&key).await.unwrap();
        assert!(matches!(warm, SourcePreparationOutcome::Complete(Ok(_))));
        assert!(
            tracker
                .take()
                .iter()
                .any(|row| row.key == key.to_string() && row.kind == ActivationKind::Reused)
        );

        transaction = selected_watch_observations(
            transaction,
            selected_path_package_epoch(MODULE, EXTENSION, true, false),
        )
        .await;
        let SourcePreparationOutcome::Complete(Ok(absent)) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("absent package must be a stable refusal");
        };
        let error = absent.result().as_ref().as_ref().unwrap_err();
        assert!(format!("{error:?}").contains("PackageAbsent"), "{error:?}");
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_refuses_generated_owners_before_route() {
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None)
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.path:
            print('before-generated-path')
            repo(name='prefix')
            ctx.path(tag.file)
    repo(name='proof')
def other_impl(ctx):
    repo(name='other')
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
other = module_extension(implementation=other_impl)
"#;
        for second_owner in [false, true] {
            let other = if second_owner {
                "f=use_extension('@dep_alias//:ext.bzl','other')\nuse_repo(f, other='other')\n"
            } else {
                ""
            };
            let base_module = format!(
                "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n{other}"
            );
            let mut builder = Dice::builder();
            install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
            let dice = Arc::new(builder.build(DetectCycles::Enabled));
            let tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let (mut bootstrap, _) =
                selected_factor_transaction(&dice, &base_module, EXTENSION, tracker.clone(), true)
                    .await;
            let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
            let mapping = bootstrap
                .compute(&HostRootRepositoryMappingKey::new(workspace))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(mapping) = mapping else {
                panic!("generated owner mapping must complete");
            };
            let mapped = mapping.as_ref().as_ref().unwrap();
            let name = if second_owner { "other" } else { "proof" };
            let generated = mapped
                .view()
                .unwrap()
                .mapping()
                .find(|(apparent, _)| apparent.as_str() == name)
                .unwrap()
                .1
                .clone();
            let module = format!(
                "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='@@{}//:missing')\nuse_repo(e, proof='proof')\n{other}",
                generated.as_str(),
            );
            let (mut transaction, key) =
                selected_path_case(&dice, &module, EXTENSION, tracker.clone()).await;
            tracker.take();
            tracker.take_dependencies();
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("generated Label must fail closed without a Need: {outcome:?}");
            };
            let error = observed.result().as_ref().as_ref().unwrap_err();
            assert!(
                format!("{error:?}").contains("does not support generated repository Labels"),
                "{second_owner}: {error:?}"
            );
            let rows = tracker.take();
            assert!(
                rows.iter()
                    .filter(|row| row.key == key.to_string())
                    .all(|row| row.batch.is_none())
            );
            assert!(
                rows.iter()
                    .flat_map(|row| row.batch.as_ref())
                    .flat_map(EventBatch::events)
                    .all(|event| !format!("{event:?}").contains("before-generated-path"))
            );
            let dependencies = tracker.take_dependencies();
            assert!(dependencies.iter().any(|(name, children)| {
                name == &key.to_string()
                    && children
                        .iter()
                        .any(|child| child.contains("host-selected-extension-demand:"))
            }));
            assert!(
                dependencies
                    .iter()
                    .flat_map(|(_, children)| children.iter())
                    .all(|child| !child.contains(&format!(
                        "host-canonical-repository-load-route:{}:{}",
                        REPO_RULE_WORKSPACE,
                        generated.as_str()
                    )))
            );
        }
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_tag_set_changes_a_b_a() {
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs={'value': attr.string()})
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.path:
            repo(name='proof', value=str(ctx.path(tag.file)))
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let mut results = Vec::new();
        for target in ["a", "b", "a"] {
            let module = format!(
                "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='//:{target}')\nuse_repo(e, proof='proof')\n"
            );
            let (mut transaction, key) =
                selected_path_case(&dice, &module, EXTENSION, tracker.clone()).await;
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("changed path Label set must complete: {outcome:?}");
            };
            let result = observed.result().as_ref().as_ref().unwrap();
            let (
                _,
                crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value),
            ) = result.receipt.repository_rule_calls[0]
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
            else {
                panic!("path string was not retained");
            };
            results.push(value.clone());
            assert_eq!(value.as_str(), format!("{REPO_RULE_WORKSPACE}/{target}"));
        }
        assert_ne!(results[0], results[1]);
        assert_eq!(results[0], results[2]);
    }

    #[tokio::test]
    async fn selected_external_owner_download_watch_then_path_label_replay_is_clean() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:watch.txt')\ne.path(file='//pkg:missing')\nuse_repo(e, proof='proof')\n";
        const BYTES: &[u8] = b"selected-path-download";
        let checksum = format!("{:x}", Sha256::digest(BYTES));
        let extension = format!(
            r#"
repo = repository_rule(lambda ctx: None)
watch_tag = tag_class(attrs={{'file': attr.label()}})
path_tag = tag_class(attrs={{'file': attr.label()}})
def impl(ctx):
    print('before-download-path')
    output = ctx.path('generator')
    ctx.download(url='file:///selected-path-download', output=output, sha256='{checksum}')
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            ctx.watch(tag.file)
    repo(name='prefix')
    for mod in ctx.modules:
        for tag in mod.tags.path:
            ctx.path(tag.file)
    repo(name='proof')
    print('after-download-path')
ext = module_extension(implementation=impl, tag_classes={{'watch': watch_tag, 'path': path_tag}}, environ=['FACTOR','MISSING','FACTOR'])
"#
        );
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, &extension, tracker.clone(), true).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key =
            HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner.clone());
        let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier.clone(),
        )
        .await;
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold selected download must return Need");
        };
        let request = need
            .selected_extension_downloads()
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        let verified =
            Arc::new(SelectedExtensionDownloadVerified::new(request, Arc::from(BYTES)).unwrap());
        transaction = selected_factor_host_inputs_with_download(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier,
            SelectedExtensionDownloadTransaction::new(
                workspace,
                [owner.clone()],
                [verified.clone()],
            )
            .unwrap(),
            Some((owner, SelectedExtensionDownloadEpoch::Ready(verified))),
        )
        .await;
        transaction = selected_watch_observations(
            transaction,
            selected_watch_epoch(MODULE, &extension, Some(b"watched")),
        )
        .await;
        tracker.take();
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold path package must return Need after prepared download and watch");
        };
        assert!(need.path_observations().is_some(), "{need:?}");
        let rows = tracker.take();
        assert!(
            rows.iter()
                .any(|row| row.key.starts_with("used-selected-extension-download:"))
        );
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("before-download-path"))
        );
        transaction = selected_watch_observations(
            transaction,
            selected_path_package_epoch(MODULE, &extension, true, true),
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("download/watch/path replay must complete: {outcome:?}");
        };
        let calls = &observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .receipt
            .repository_rule_calls;
        assert_eq!(
            calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            ["prefix", "proof"]
        );
        let rows = tracker.take();
        let events = rows
            .iter()
            .filter(|row| row.key == key.to_string())
            .flat_map(|row| row.batch.as_ref())
            .flat_map(EventBatch::events)
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.contains("before-download-path"))
                .count(),
            1,
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.contains("after-download-path"))
                .count(),
            1,
            "{events:?}"
        );
    }

    #[tokio::test]
    async fn selected_external_owner_multiple_label_paths_replay_once_per_new_label() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='//:first')\ne.path(file='//:second')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None, attrs={'value': attr.string()})
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    print('multiple-label-paths')
    values = []
    for mod in ctx.modules:
        for tag in mod.tags.path:
            values.append(str(ctx.path(tag.file)))
    repo(name='proof', value='|'.join(values))
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut transaction, key) =
            selected_path_case(&dice, MODULE, EXTENSION, tracker.clone()).await;
        tracker.take();
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("two root Label paths must complete: {outcome:?}");
        };
        let calls = &observed
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .receipt
            .repository_rule_calls;
        assert_eq!(calls.len(), 1);
        let (_, crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value)) =
            calls[0]
                .kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
        else {
            panic!("two paths were not retained");
        };
        assert_eq!(
            value.as_str(),
            format!("{REPO_RULE_WORKSPACE}/first|{REPO_RULE_WORKSPACE}/second")
        );
        let rows = tracker.take();
        let events = rows
            .iter()
            .filter(|row| row.key == key.to_string())
            .flat_map(|row| row.batch.as_ref())
            .flat_map(EventBatch::events)
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.contains("multiple-label-paths"))
                .count(),
            1,
            "{events:?}"
        );
        assert!(
            observed
                .observations()
                .observations()
                .iter()
                .all(|(demand, _)| {
                    !demand.path().as_path().ends_with("first")
                        && !demand.path().as_path().ends_with("second")
                })
        );
    }

    #[tokio::test]
    async fn selected_external_owner_path_label_external_package_absent_is_observed() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.path(file='@dep_alias//pkg:missing')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None)
path_tag = tag_class(attrs={'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.path:
            print('before-absent-external-package')
            ctx.path(tag.file)
    repo(name='proof')
ext = module_extension(implementation=impl, tag_classes={'path': path_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (mut transaction, key) =
            selected_path_case(&dice, MODULE, EXTENSION, tracker.clone()).await;
        tracker.take();
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold external package must request path observations");
        };
        assert!(need.path_observations().is_some(), "{need:?}");
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        transaction = selected_watch_observations(
            transaction,
            selected_external_path_absent_package_epoch(MODULE, EXTENSION),
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("absent external package must be a stable terminal: {outcome:?}");
        };
        let error = observed.result().as_ref().as_ref().unwrap_err();
        assert!(format!("{error:?}").contains("PackageAbsent"), "{error:?}");
        assert!(
            observed
                .observations()
                .observations()
                .iter()
                .any(|(demand, _)| { demand.path().as_path().ends_with("pkg/BUILD.bazel") })
        );
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_external_owner_watch_set_changes_with_selected_tags() {
        const EXTENSION: &str = r#"
watch_tag = tag_class(attrs = {'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            ctx.watch(tag.file)
ext = module_extension(implementation=impl, tag_classes={'watch': watch_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let mut prior_owner = None;
        for name in ["watch.txt", "other.txt", "watch.txt"] {
            let module = format!(
                "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:{name}')\nuse_repo(e, proof='proof')\n"
            );
            let (base, owner) =
                selected_factor_transaction(&dice, &module, EXTENSION, tracker.clone(), true).await;
            if let Some(previous) = &prior_owner {
                assert_eq!(previous, &owner);
            }
            prior_owner = Some(owner.clone());
            tracker.take();
            let base = selected_factor_host_inputs(
                base,
                tracker.clone(),
                RepositoryPlatform::new("linux", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                RepositoryEnvironmentNameFrontier::from_unsorted([
                    CompactString::new("FACTOR"),
                    CompactString::new("MISSING"),
                ]),
            )
            .await;
            let mut transaction = selected_watch_observations(
                base,
                selected_watch_epoch_named(&module, EXTENSION, name, Some(b"x")),
            )
            .await;
            let key = HostSelectedExtensionOwnerPureObservationKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                owner,
            );
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("changed watch set did not complete: {outcome:?}");
            };
            assert!(observed.result().as_ref().is_ok());
            let digests = observed
                .observations()
                .observations()
                .iter()
                .filter(|(demand, _)| demand.operation() == PathObservationOperation::FileDigest)
                .map(|(demand, _)| {
                    demand
                        .path()
                        .as_path()
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(digests, [name]);
            assert!(
                tracker
                    .take()
                    .iter()
                    .any(|row| row.key == key.to_string() && row.kind == ActivationKind::Evaluated)
            );
        }
    }

    #[tokio::test]
    async fn selected_external_owner_watch_refuses_direct_and_ancestor_symlinks() {
        const EXTENSION: &str = r#"
repo = repository_rule(lambda ctx: None)
watch_tag = tag_class(attrs = {'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            print('before-symlink-watch')
            ctx.watch(tag.file)
    repo(name='proof')
ext = module_extension(implementation=impl, tag_classes={'watch': watch_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        for ancestor in [false, true] {
            for dangling in [false, true] {
                let label = if ancestor {
                    "//:nested/watch.txt"
                } else {
                    "//:watch.txt"
                };
                let module = format!(
                    "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='{label}')\nuse_repo(e, proof='proof')\n"
                );
                let mut builder = Dice::builder();
                install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
                let dice = Arc::new(builder.build(DetectCycles::Enabled));
                let tracker = Arc::new(RepositoryRuleActivationTracker::default());
                let (base, owner) =
                    selected_factor_transaction(&dice, &module, EXTENSION, tracker.clone(), true)
                        .await;
                tracker.take();
                let base = selected_factor_host_inputs(
                    base,
                    tracker.clone(),
                    RepositoryPlatform::new("linux", "x86_64"),
                    RepositoryEnvironmentSnapshot::empty(),
                    RepositoryEnvironmentNameFrontier::from_unsorted([
                        CompactString::new("FACTOR"),
                        CompactString::new("MISSING"),
                    ]),
                )
                .await;
                let mut transaction = selected_watch_observations(
                    base,
                    selected_watch_symlink_epoch(&module, EXTENSION, ancestor, dangling),
                )
                .await;
                let key = HostSelectedExtensionOwnerPureObservationKey::new(
                    NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                    owner,
                );
                let outcome = transaction.compute(&key).await.unwrap();
                let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                    panic!("symlink case returned an unexpected Need: {outcome:?}");
                };
                let error = observed.result().as_ref().as_ref().unwrap_err();
                let diagnostic = format!("{error:?}");
                assert!(
                    diagnostic.contains("does not support symlink paths"),
                    "{ancestor} {dangling}: {diagnostic}"
                );
                let rows = tracker.take();
                assert!(
                    rows.iter()
                        .filter(|row| row.key == key.to_string())
                        .all(|row| row.batch.is_none())
                );
                assert!(
                    rows.iter()
                        .flat_map(|row| row.batch.as_ref())
                        .flat_map(EventBatch::events)
                        .all(|event| !format!("{event:?}").contains("before-symlink-watch"))
                );
            }
        }
    }

    #[tokio::test]
    async fn selected_external_owner_watch_refuses_directory() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:watch.txt')\nuse_repo(e, proof='proof')\n";
        const BYTES: &[u8] = b"selected-watch-download";
        let checksum = format!("{:x}", Sha256::digest(BYTES));
        let extension = format!(
            "watch_tag=tag_class(attrs={{'file':attr.label()}})\ndef impl(ctx):\n  output=ctx.path('generator')\n  ctx.download(url='file:///selected-watch-download',output=output,sha256='{checksum}')\n  for mod in ctx.modules:\n    for tag in mod.tags.watch:\n      ctx.watch(tag.file)\next=module_extension(implementation=impl,tag_classes={{'watch':watch_tag}},environ=['FACTOR','MISSING','FACTOR'])\n"
        );
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, &extension, tracker.clone(), true).await;
        tracker.take();
        let base = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
        )
        .await;
        let mut transaction =
            selected_watch_observations(base, selected_watch_directory_epoch(MODULE, &extension))
                .await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key =
            HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner.clone());
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("selected download must be prepared before the directory watch");
        };
        let request = need
            .selected_extension_downloads()
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        let verified =
            Arc::new(SelectedExtensionDownloadVerified::new(request, Arc::from(BYTES)).unwrap());
        transaction = selected_factor_host_inputs_with_download(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
            SelectedExtensionDownloadTransaction::new(
                workspace.dupe(),
                [owner.clone()],
                [verified.clone()],
            )
            .unwrap(),
            Some((owner, SelectedExtensionDownloadEpoch::Ready(verified))),
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("directory watch returned an unexpected Need: {outcome:?}");
        };
        assert!(
            format!("{:?}", observed.result().as_ref().as_ref().unwrap_err())
                .contains("only regular or missing files")
        );
        let rows = tracker.take();
        assert!(
            rows.iter()
                .any(|row| row.key.starts_with("used-selected-extension-download:")),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_external_owner_keeps_watch_dependency_after_starlark_error() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\ne.watch(file='//:watch.txt')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = r#"
watch_tag = tag_class(attrs = {'file': attr.label()})
def impl(ctx):
    for mod in ctx.modules:
        for tag in mod.tags.watch:
            ctx.watch(tag.file)
    print('watched-before-failure')
    fail('later-failure')
ext = module_extension(implementation=impl, tag_classes={'watch': watch_tag}, environ=['FACTOR','MISSING','FACTOR'])
"#;
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        tracker.take();
        let base = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]),
        )
        .await;
        let key = HostSelectedExtensionOwnerPureObservationKey::new(
            NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
            owner,
        );
        let mut transaction = base;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        for content in [&b"A"[..], &b"B"[..]] {
            transaction = selected_watch_observations(
                transaction,
                selected_watch_epoch(MODULE, EXTENSION, Some(content)),
            )
            .await;
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("post-watch error did not complete: {outcome:?}");
            };
            assert!(
                format!("{:?}", observed.result().as_ref().as_ref().unwrap_err())
                    .contains("later-failure")
            );
            assert!(
                observed
                    .observations()
                    .observations()
                    .iter()
                    .any(|(demand, _)| {
                        demand.path().as_path().ends_with("watch.txt")
                            && demand.operation() == PathObservationOperation::FileDigest
                    })
            );
            let rows = tracker.take();
            assert!(
                rows.iter()
                    .any(|row| row.key == key.to_string() && row.kind == ActivationKind::Evaluated)
            );
            assert!(
                rows.iter()
                    .flat_map(|row| row.batch.as_ref())
                    .flat_map(EventBatch::events)
                    .any(|event| format!("{event:?}").contains("watched-before-failure"))
            );
        }
    }

    #[tokio::test]
    async fn selected_external_owner_rejects_facts_version_before_invocation() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        for factor in ["facts_version=1"] {
            let extension = format!(
                "def impl(ctx):\n    print('selected-invoked')\next = module_extension(implementation=impl, environ=['FACTOR'], {factor})\n"
            );
            let mut builder = Dice::builder();
            install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
            let dice = Arc::new(builder.build(DetectCycles::Enabled));
            let tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let (transaction, owner) =
                selected_factor_transaction(&dice, MODULE, &extension, tracker.clone(), false)
                    .await;
            tracker.take();
            let mut transaction = selected_factor_host_inputs(
                transaction,
                tracker.clone(),
                RepositoryPlatform::new("linux", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                RepositoryEnvironmentNameFrontier::empty(),
            )
            .await;
            let key = HostSelectedExtensionOwnerPureObservationKey::new(
                NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap(),
                owner,
            );
            let result = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(observed)) = result else {
                panic!("{factor} did not produce a selected-owner terminal: {result:?}")
            };
            let error = observed.result().as_ref().as_ref().unwrap_err();
            assert!(
                format!("{error:?}").contains("unsupported factors"),
                "{factor}: {error:?}"
            );
            let rows = tracker.take();
            assert!(
                rows.iter().any(|row| row.key == key.to_string()),
                "{factor}: selected owner did not activate"
            );
            assert!(
                rows.iter()
                    .filter(|row| row.key == key.to_string())
                    .all(|row| row.batch.is_none()),
                "{factor}: selected owner published invocation events"
            );
            assert!(
                rows.iter()
                    .flat_map(|row| row.batch.as_ref())
                    .flat_map(EventBatch::events)
                    .all(|event| !format!("{event:?}").contains("selected-invoked")),
                "{factor}: selected implementation ran"
            );
        }
    }

    #[tokio::test]
    async fn selected_external_owner_conditional_host_factors_preserve_structural_identity() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        fn value(
            outcome: &<HostSelectedExtensionOwnerPureObservationKey as Key>::Value,
        ) -> &HostSelectedExtensionOwnerPureResult {
            let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
                panic!("selected factor owner did not complete: {outcome:?}")
            };
            observed.result().as_ref().as_ref().unwrap()
        }
        for (flags, select_os, select_arch) in [
            ("", false, false),
            (", os_dependent=True", true, false),
            (", arch_dependent=True", false, true),
            (", os_dependent=True, arch_dependent=True", true, true),
        ] {
            let extension = format!(
                "repo = repository_rule(lambda ctx: None)\ndef impl(ctx):\n    repo(name='proof')\next = module_extension(implementation=impl, environ=['FACTOR', 'MISSING', 'FACTOR']{flags})\n"
            );
            let mut builder = Dice::builder();
            install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
            let dice = Arc::new(builder.build(DetectCycles::Enabled));
            let tracker = Arc::new(RepositoryRuleActivationTracker::default());
            let (base, owner) =
                selected_factor_transaction(&dice, MODULE, &extension, tracker.clone(), true).await;
            let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
            let key =
                HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner.clone());
            let legacy_key =
                HostSelectedExtensionOwnerPureKey::new(workspace.dupe(), owner.clone());
            let certificate_key = crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey::new(workspace, owner);
            let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
                CompactString::new("FACTOR"),
                CompactString::new("MISSING"),
            ]);
            let mut transaction = selected_factor_host_inputs(
                base,
                tracker.clone(),
                RepositoryPlatform::new("linux", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                frontier.clone(),
            )
            .await;
            let a = transaction.compute(&key).await.unwrap();
            assert_eq!(
                value(&a).factors.operating_system.as_deref(),
                select_os.then_some("linux")
            );
            assert_eq!(
                value(&a).factors.architecture.as_deref(),
                select_arch.then_some("x86_64")
            );
            let calls = value(&a).receipt.repository_rule_calls.clone();
            assert_eq!(calls.len(), 1);
            let SourcePreparationOutcome::Complete(legacy_a) =
                transaction.compute(&legacy_key).await.unwrap()
            else {
                panic!("legacy selected owner returned Need")
            };
            assert_eq!(
                legacy_a.as_ref().as_ref().unwrap().factors,
                value(&a).factors
            );
            let certificate_a = transaction.compute(&certificate_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(certificate_a_value)) = &certificate_a else {
                panic!("selected owner certificate did not complete: {certificate_a:?}")
            };
            let generated_a = certificate_a_value
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .iter()
                .map(|(canonical, _, generated, mapping)| {
                    (
                        canonical.as_str().to_owned(),
                        generated.to_owned(),
                        mapping.context_repo().as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(generated_a.len(), 1);

            transaction = selected_factor_host_inputs(
                transaction,
                tracker.clone(),
                RepositoryPlatform::new("mac os x", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                frontier.clone(),
            )
            .await;
            let changed_os = transaction.compute(&key).await.unwrap();
            assert_eq!(value(&changed_os).receipt.repository_rule_calls, calls);
            let certificate_changed_os = transaction.compute(&certificate_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(certificate_changed_os_value)) =
                &certificate_changed_os
            else {
                panic!("selected owner certificate changed shape: {certificate_changed_os:?}")
            };
            let generated_changed_os = certificate_changed_os_value
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .iter()
                .map(|(canonical, _, generated, mapping)| {
                    (
                        canonical.as_str().to_owned(),
                        generated.to_owned(),
                        mapping.context_repo().as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(generated_a, generated_changed_os);
            assert_eq!(
                HostSelectedExtensionOwnerPureObservationKey::equality(&a, &changed_os),
                !select_os,
                "OS participation for {flags}"
            );
            assert_eq!(
                crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey::equality(&certificate_a, &certificate_changed_os),
                !select_os,
                "certificate OS participation for {flags}"
            );

            transaction = selected_factor_host_inputs(
                transaction,
                tracker.clone(),
                RepositoryPlatform::new("linux", "aarch64"),
                RepositoryEnvironmentSnapshot::empty(),
                frontier.clone(),
            )
            .await;
            let changed_arch = transaction.compute(&key).await.unwrap();
            assert_eq!(value(&changed_arch).receipt.repository_rule_calls, calls);
            assert_eq!(
                HostSelectedExtensionOwnerPureObservationKey::equality(&a, &changed_arch),
                !select_arch,
                "architecture participation for {flags}"
            );

            transaction = selected_factor_host_inputs(
                transaction,
                tracker,
                RepositoryPlatform::new("linux", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                frontier,
            )
            .await;
            let restored = transaction.compute(&key).await.unwrap();
            assert!(HostSelectedExtensionOwnerPureObservationKey::equality(
                &a, &restored
            ));
            if flags.is_empty() {
                let with_os = "repo = repository_rule(lambda ctx: None)\ndef impl(ctx):\n    repo(name='proof')\next = module_extension(implementation=impl, environ=['FACTOR', 'MISSING', 'FACTOR'], os_dependent=True)\n";
                let changed = selected_factor_epoch(MODULE, with_os);
                let mut updater = transaction.into_updater();
                updater
                    .changed_to(path_observation_shards(&changed))
                    .unwrap();
                updater
                    .changed_to([(PathObservationEpochKey, changed)])
                    .unwrap();
                transaction = updater.commit().await;
                let changed_declaration = transaction.compute(&key).await.unwrap();
                assert_eq!(
                    value(&changed_declaration)
                        .factors
                        .operating_system
                        .as_deref(),
                    Some("linux")
                );
                assert!(!HostSelectedExtensionOwnerPureObservationKey::equality(
                    &a,
                    &changed_declaration
                ));

                let original = selected_factor_epoch(MODULE, &extension);
                let mut updater = transaction.into_updater();
                updater
                    .changed_to(path_observation_shards(&original))
                    .unwrap();
                updater
                    .changed_to([(PathObservationEpochKey, original)])
                    .unwrap();
                transaction = updater.commit().await;
                let source_restored = transaction.compute(&key).await.unwrap();
                assert!(HostSelectedExtensionOwnerPureObservationKey::equality(
                    &a,
                    &source_restored
                ));
            }
        }
    }

    #[tokio::test]
    async fn selected_external_owner_retains_factors_on_identical_error_and_observation_terminal() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = "def impl(ctx):\n    fail('same failure')\next = module_extension(implementation=impl, environ=['FACTOR', 'MISSING', 'FACTOR'], os_dependent=True, arch_dependent=True)\n";
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker.clone(), true).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key = HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner);
        let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier.clone(),
        )
        .await;
        let a = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = &a else {
            panic!("selected failure did not complete: {a:?}")
        };
        let HostSelectedExtensionOwnerPureError::Factored {
            factors: a_factors,
            error,
        } = observed.result().as_ref().as_ref().unwrap_err()
        else {
            panic!("selected failure lost its factors")
        };
        assert_eq!(a_factors.operating_system.as_deref(), Some("linux"));
        assert_eq!(a_factors.architecture.as_deref(), Some("x86_64"));
        assert!(format!("{error:?}").contains("same failure"));
        let inputs = match error.as_ref() {
            HostSelectedExtensionOwnerPureError::AfterInputs { inputs, .. } => inputs.clone(),
            _ => panic!("selected failure lost owner inputs"),
        };
        transaction = selected_factor_host_inputs(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("mac os x", "aarch64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier.clone(),
        )
        .await;
        let b = transaction.compute(&key).await.unwrap();
        assert!(!HostSelectedExtensionOwnerPureObservationKey::equality(
            &a, &b
        ));
        transaction = selected_factor_host_inputs(
            transaction,
            tracker,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier,
        )
        .await;
        let restored = transaction.compute(&key).await.unwrap();
        assert!(HostSelectedExtensionOwnerPureObservationKey::equality(
            &a, &restored
        ));

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            workspace,
            PathObservationOperation::FileBytes,
        );
        let lower = ObservedPathFrontierError::from(PathObservationEpochError::OperationMismatch {
            demand,
            result_operation: PathObservationOperation::Lstat,
        });
        let outer = || -> OwnerPureDriver {
            SourcePreparationOutcome::Complete(Err(
                HostSelectedExtensionOwnerPureObservationError::Merge {
                    inputs: inputs.clone(),
                    first: None,
                    error: lower.clone(),
                },
            ))
        };
        fn observed_outer(
            terminal: OwnerPureDriver,
        ) -> <HostSelectedExtensionOwnerPureObservationKey as Key>::Value {
            let SourcePreparationOutcome::Complete(Err(error)) = terminal else {
                panic!("expected a complete observed outer terminal")
            };
            SourcePreparationOutcome::Complete(Err(error))
        }
        let factored_a = observed_outer(factor_owner_terminal(a_factors, outer()));
        let b_factors = Arc::new(SelectedExtensionEvalFactors {
            operating_system: Some("mac os x".into()),
            architecture: Some("aarch64".into()),
        });
        let factored_b = observed_outer(factor_owner_terminal(&b_factors, outer()));
        let factored_restored = observed_outer(factor_owner_terminal(a_factors, outer()));
        assert!(!HostSelectedExtensionOwnerPureObservationKey::equality(
            &factored_a,
            &factored_b
        ));
        assert!(HostSelectedExtensionOwnerPureObservationKey::equality(
            &factored_a,
            &factored_restored
        ));
        let SourcePreparationOutcome::Complete(Err(
            HostSelectedExtensionOwnerPureObservationError::Factored { factors, error },
        )) = factored_a
        else {
            panic!("outer observation error lost selected factors")
        };
        assert_eq!(factors.as_ref(), a_factors.as_ref());
        assert!(matches!(
            error.as_ref(),
            HostSelectedExtensionOwnerPureObservationError::Merge { .. }
        ));
    }

    #[tokio::test]
    async fn selected_external_owner_download_need_replay_and_used_request() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        const BYTES: &[u8] = b"selected-generator-fixture\n";
        let checksum = format!("{:x}", Sha256::digest(BYTES));
        let extension = format!(
            r#"
repo = repository_rule(lambda ctx: None, attrs = {{'value': attr.string()}})
def impl(ctx):
    output = ctx.path('cargo-bazel')
    print('before-selected-download')
    result = ctx.download(url='file:///selected-generator-fixture', output=output, executable=True, sha256='{checksum}')
    repo(name='proof', value=result.sha256 + '|' + result.integrity + '|' + str(output))
    print('after-selected-download')
ext = module_extension(implementation=impl, environ=['FACTOR', 'MISSING', 'FACTOR'])
"#
        );
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (base, owner) =
            selected_factor_transaction(&dice, MODULE, &extension, tracker.clone(), true).await;
        tracker.take();
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let key =
            HostSelectedExtensionOwnerPureObservationKey::new(workspace.dupe(), owner.clone());
        let frontier = RepositoryEnvironmentNameFrontier::from_unsorted([
            CompactString::new("FACTOR"),
            CompactString::new("MISSING"),
        ]);
        let mut transaction = selected_factor_host_inputs(
            base,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier.clone(),
        )
        .await;
        let SourcePreparationOutcome::Need(need) = transaction.compute(&key).await.unwrap() else {
            panic!("cold selected download must return Need");
        };
        let requests = need.selected_extension_downloads();
        assert_eq!(requests.len(), 1);
        let request = requests.iter().next().unwrap().1.clone();
        assert_eq!(request.owner(), &owner);
        assert_eq!(request.url(), "file:///selected-generator-fixture");
        assert_eq!(request.output().as_path().to_str(), Some("cargo-bazel"));
        assert_eq!(request.sha256_hex(), checksum);
        assert!(request.executable());
        let rows = tracker.take();
        assert!(
            rows.iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .all(|event| !format!("{event:?}").contains("selected-download"))
        );

        let verified = Arc::new(
            SelectedExtensionDownloadVerified::new(request.clone(), Arc::from(BYTES)).unwrap(),
        );
        transaction = selected_factor_host_inputs_with_download(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier.clone(),
            SelectedExtensionDownloadTransaction::new(
                workspace.dupe(),
                [owner.clone()],
                [verified.clone()],
            )
            .unwrap(),
            Some((
                owner.clone(),
                SelectedExtensionDownloadEpoch::Ready(verified.clone()),
            )),
        )
        .await;
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("selected download replay did not complete");
        };
        let result = observed.result().as_ref().as_ref().unwrap();
        let call = &result.receipt.repository_rule_calls[0];
        let (_, crate::module_extension_repository_rule::RepositoryRuleCallValue::String(value)) =
            call.kwargs
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
        else {
            panic!("selected repository call lost download result");
        };
        assert_eq!(
            value.as_str(),
            format!(
                "{}|{}|cargo-bazel",
                request.sha256_hex(),
                request.sha256_sri()
            )
        );
        let rows = tracker.take();
        assert!(
            rows.iter()
                .any(|row| row.key.starts_with("used-selected-extension-download:"))
        );
        assert!(
            rows.iter()
                .flat_map(|row| row.batch.as_ref())
                .flat_map(EventBatch::events)
                .any(|event| format!("{event:?}").contains("after-selected-download"))
        );
        let warm = transaction.compute(&key).await.unwrap();
        assert!(matches!(warm, SourcePreparationOutcome::Complete(Ok(_))));
        assert!(
            tracker
                .take()
                .iter()
                .any(|row| row.key == key.to_string() && row.kind == ActivationKind::Reused)
        );

        transaction = selected_factor_host_inputs_with_download(
            transaction,
            tracker.clone(),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            frontier,
            SelectedExtensionDownloadTransaction::new(workspace, [owner.clone()], [verified])
                .unwrap(),
            Some((owner, SelectedExtensionDownloadEpoch::AuthorizedEmpty)),
        )
        .await;
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("mismatched selected download cell must be a complete terminal");
        };
        let diagnostic = format!("{:?}", observed.result().as_ref().as_ref().unwrap_err());
        assert!(diagnostic.contains("differs from request"), "{diagnostic}");
        assert!(
            !diagnostic.contains("selected-generator-fixture"),
            "{diagnostic}"
        );
        assert!(
            tracker
                .take()
                .iter()
                .filter(|row| row.key == key.to_string())
                .all(|row| row.batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_module_path_and_download_reject_unmodeled_arguments() {
        const MODULE: &str = "module(name='bazel_tools')\nbazel_dep(name='dep', version='1', repo_name='dep_alias')\ne=use_extension('@dep_alias//:ext.bzl','ext')\nuse_repo(e, proof='proof')\n";
        const EXTENSION: &str = "ext=module_extension(implementation=lambda ctx: None, environ=['FACTOR','MISSING','FACTOR'])\n";
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, Arc::new(SelectedFactorRegistryIo));
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(RepositoryRuleActivationTracker::default());
        let (_, selected_owner) =
            selected_factor_transaction(&dice, MODULE, EXTENSION, tracker, true).await;
        let workspace = NormalizedAbsolutePath::new(REPO_RULE_WORKSPACE).unwrap();
        let owner = Arc::new(());
        let scratch = Arc::new(SelectedDownloadInvocation::new(
            workspace.dupe(),
            selected_owner.clone(),
            None,
        ));
        let context =
            InvocationContext::new_modules(Arc::from([empty_module(&owner)]), &owner, None, None)
                .with_selected_download(scratch.clone());
        assert_eq!(
            call(
                "def f(ctx):\n  return str(ctx.path('cargo-bazel'))\n",
                |module| vec![module.heap().alloc_simple(context.clone())]
            )
            .unwrap(),
            "\"cargo-bazel\""
        );
        for path in [
            "",
            "../escape",
            "/absolute",
            "a/../b",
            "a//b",
            "a\\b",
            "a/./b",
        ] {
            let source = format!("def f(ctx):\n  return ctx.path({path:?})\n");
            let error = call(&source, |module| {
                vec![module.heap().alloc_simple(context.clone())]
            })
            .unwrap_err();
            assert!(
                error.contains("normalized relative string"),
                "{path:?}: {error}"
            );
        }
        for source in [
            "def f(ctx):\n  return ctx.download(url='file:///x', output='x', sha256='00' * 32)\n",
            "def f(ctx):\n  return ctx.download(url=['file:///x'], output=ctx.path('x'), sha256='00' * 32)\n",
            "def f(ctx):\n  return ctx.download(url='file:///x', output=ctx.path('x'), sha256='bad')\n",
            "def f(ctx):\n  return ctx.download(url='file:///x', output=ctx.path('x'), sha256='00' * 32, allow_fail=True)\n",
            "def f(ctx):\n  return ctx.download(url='file:///x', output=ctx.path('x'))\n",
        ] {
            assert!(
                call(source, |module| vec![
                    module.heap().alloc_simple(context.clone())
                ])
                .is_err(),
                "{source}"
            );
        }
        let other_owner = Arc::new(());
        let other = InvocationContext::new_modules(
            Arc::from([empty_module(&other_owner)]),
            &other_owner,
            None,
            None,
        )
        .with_selected_download(scratch.clone());
        let foreign = call(
            "def f(ctx, other):\n  return ctx.download(url='file:///x', output=other.path('x'), sha256='00' * 32)\n",
            |module| vec![module.heap().alloc_simple(context.clone()), module.heap().alloc_simple(other)],
        ).unwrap_err();
        assert!(foreign.contains("another module_ctx"), "{foreign}");
        assert!(scratch.need().unwrap().is_none());

        let checksum = format!("{:x}", Sha256::digest(b"x"));
        let request = Arc::new(
            SelectedExtensionDownloadRequest::new(
                workspace.dupe(),
                selected_owner.clone(),
                selected_scratch_relative("first").unwrap(),
                "file:///x",
                selected_download_sha256(&checksum).unwrap(),
                true,
            )
            .unwrap(),
        );
        let ready = Arc::new(
            SelectedExtensionDownloadVerified::new(request.clone(), Arc::from(&b"x"[..])).unwrap(),
        );
        let completed = Arc::new(SelectedDownloadInvocation::new(
            workspace,
            selected_owner,
            Some(ready),
        ));
        let context =
            InvocationContext::new_modules(Arc::from([empty_module(&owner)]), &owner, None, None)
                .with_selected_download(completed.clone());
        let source = format!(
            "def f(ctx):\n  ctx.download(url='file:///x', output=ctx.path('first'), sha256='{checksum}', executable=True)\n  return ctx.download(url='file:///x', output=ctx.path('second'), sha256='{checksum}', executable=True)\n"
        );
        let error = call(&source, |module| vec![module.heap().alloc_simple(context)]).unwrap_err();
        assert!(
            error.contains("multiple distinct module extension downloads"),
            "{error}"
        );
        assert_eq!(completed.used().unwrap(), Some(request));
        assert!(completed.need().unwrap().is_none());
    }

    #[test]
    fn selected_module_watch_rejects_unmodeled_arguments_and_caps_distinct_labels() {
        let owner = Arc::new(());
        let watch = Arc::new(SelectedWatchInvocation::new());
        let context =
            InvocationContext::new_modules(Arc::from([empty_module(&owner)]), &owner, None, None)
                .with_selected_watch(watch.clone());
        let root = StarlarkLabel::new(CanonicalLabel::parse("@@//:watch.txt").unwrap());
        assert_eq!(
            call(
                "def f(ctx, label):\n  return ctx.watch(label)\n",
                |module| vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(root.clone()),
                ]
            )
            .unwrap(),
            "None"
        );
        assert_eq!(watch.called().unwrap().len(), 1);
        let error = call(
            "def f(ctx):\n  return ctx.watch('//:watch.txt')\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        )
        .unwrap_err();
        assert!(error.contains("requires a Label"), "{error}");
        let scratch_path = SelectedModuleScratchPath {
            relative: selected_scratch_relative("watch.txt").unwrap(),
            owner: owner.clone(),
        };
        let error = call("def f(ctx, path):\n  return ctx.watch(path)\n", |module| {
            vec![
                module.heap().alloc_simple(context.clone()),
                module.heap().alloc_simple(scratch_path),
            ]
        })
        .unwrap_err();
        assert!(error.contains("requires a Label"), "{error}");
        let external = StarlarkLabel::new(CanonicalLabel::parse("@@dep+//:watch.txt").unwrap());
        let error = call(
            "def f(ctx, label):\n  return ctx.watch(label)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(external),
                ]
            },
        )
        .unwrap_err();
        assert!(error.contains("root repository Labels"), "{error}");
        assert_eq!(watch.called().unwrap().len(), 1);
        for index in 1..MAX_SELECTED_WATCHES {
            watch
                .call(&CanonicalLabel::parse(&format!("@@//:watch{index}.txt")).unwrap())
                .unwrap();
        }
        watch.call(root.canonical()).unwrap();
        let error = watch
            .call(&CanonicalLabel::parse("@@//:overflow.txt").unwrap())
            .unwrap_err();
        assert!(
            error.to_string().contains("at most 8 distinct Labels"),
            "{error}"
        );
        assert_eq!(watch.called().unwrap().len(), MAX_SELECTED_WATCHES);
    }

    #[test]
    fn selected_module_label_path_value_refuses_unmodeled_operations_and_caps_labels() {
        let owner = Arc::new(());
        let label = CanonicalLabel::parse("@@//:missing").unwrap();
        let physical = HostRepositoryLabelPathValue::new(
            NormalizedAbsolutePath::new("/fixture/missing").unwrap(),
            PathObservationNamespace::Host,
        )
        .unwrap();
        let path = Arc::new(SelectedPathInvocation::new(vec![(label.clone(), physical)]));
        let context =
            InvocationContext::new_modules(Arc::from([empty_module(&owner)]), &owner, None, None)
                .with_selected_path(path.clone());
        let starlark_label = StarlarkLabel::new(label.clone());
        assert_eq!(
            call(
                "def f(ctx, label):\n  p=ctx.path(label)\n  return [str(p), str(p.dirname), str(p.dirname.get_child('bin/cargo'))]\n",
                |module| vec![module.heap().alloc_simple(context.clone()), module.heap().alloc_simple(starlark_label.clone())],
            )
            .unwrap(),
            "[\"/fixture/missing\", \"/fixture\", \"/fixture/bin/cargo\"]"
        );
        for source in [
            "def f(ctx, label):\n  return ctx.path(ctx.path(label))\n",
            "def f(ctx, label):\n  return ctx.path(label).exists\n",
            "def f(ctx, label):\n  return ctx.path(label).is_dir\n",
            "def f(ctx, label):\n  return ctx.path(label).realpath\n",
            "def f(ctx, label):\n  return ctx.path(label).get_child()\n",
            "def f(ctx, label):\n  return ctx.path(label).get_child('a', 'b')\n",
            "def f(ctx, label):\n  return ctx.path(label).get_child('../escape')\n",
        ] {
            assert!(
                call(source, |module| vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(starlark_label.clone())
                ])
                .is_err(),
                "{source}"
            );
        }
        assert!(path.pending().unwrap().is_none());
        let ready = (0..MAX_SELECTED_PATH_LABELS)
            .map(|index| {
                (
                    CanonicalLabel::parse(&format!("@@//:ready{index}")).unwrap(),
                    HostRepositoryLabelPathValue::new(
                        NormalizedAbsolutePath::new(format!("/fixture/ready{index}")).unwrap(),
                        PathObservationNamespace::Host,
                    )
                    .unwrap(),
                )
            })
            .collect();
        let capped = SelectedPathInvocation::new(ready);
        let error = capped
            .call(&CanonicalLabel::parse("@@//:overflow").unwrap())
            .unwrap_err();
        assert!(
            error.to_string().contains("at most 8 distinct Labels"),
            "{error}"
        );
        assert!(capped.pending().unwrap().is_none());
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
