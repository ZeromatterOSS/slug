/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputs;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputsError;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputsKey;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputsObservationError;
use slug_bzlmod_v2::HostSelectedInnateRepositoryOwnerInputsObservationKey;
use slug_bzlmod_v2::NonrootAttributeKey;
use slug_bzlmod_v2::NonrootAttributeValue;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostCanonicalRepositoryLoadRouteError;
use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::bzl_module::ExternalBzlModuleError;
use crate::bzl_module::ExternalBzlModuleEvalKey;
use crate::bzl_module::ExternalBzlModuleObservationKey;
use crate::bzl_module::FrozenBzlModule;
use crate::bzl_module::HostBzlModuleError;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostBzlModuleObservationKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::bzl_module::RepositoryBzlLabel;
use crate::module_extension_repository_rule::FrozenRepositoryRuleDefinition;
use crate::module_extension_repository_rule::RepositoryRuleCallFrame;
use crate::module_extension_repository_rule::RepositoryRuleCallKey;
use crate::module_extension_repository_rule::RepositoryRuleCallRecord;
use crate::module_extension_repository_rule::RepositoryRuleCallSpan;
use crate::module_extension_repository_rule::RepositoryRuleCallValue;
use crate::module_extension_repository_rule::RepositoryRuleDefinitionProjection;

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostPureInnateRepositoryOwnerResult { pub(crate) inputs: Arc<HostSelectedInnateRepositoryOwnerInputs>, pub(crate) repository_rule_calls: Arc<[RepositoryRuleCallRecord]> }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPureInnateRepositoryOwnerError { Inputs(HostSelectedInnateRepositoryOwnerInputsError), Compute(CompactString), Label(CompactString), LoadRoute(Arc<HostCanonicalRepositoryLoadRouteError>), RootBzl(HostBzlModuleError), ExternalBzl(ExternalBzlModuleError), Export(CompactString), Drift, Call(CompactString) }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostPureInnateRepositoryOwnerKey { workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner> }
#[rustfmt::skip]
impl HostPureInnateRepositoryOwnerKey { pub(crate) fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self { workspace, owner } } }
#[rustfmt::skip]
impl fmt::Display for HostPureInnateRepositoryOwnerKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "host-pure-innate-repository-owner:{}:{:?}", self.workspace, self.owner) } }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostPureInnateRepositoryOwnerObservationKey(HostPureInnateRepositoryOwnerKey);
#[rustfmt::skip]
impl HostPureInnateRepositoryOwnerObservationKey { pub(crate) fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self { Self(HostPureInnateRepositoryOwnerKey::new(workspace, owner)) } }
#[rustfmt::skip]
impl fmt::Display for HostPureInnateRepositoryOwnerObservationKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) } }

type PureResult =
    Arc<Result<HostPureInnateRepositoryOwnerResult, HostPureInnateRepositoryOwnerError>>;

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostPureInnateRepositoryOwner { result: PureResult, observations: PathObservationEpoch }
#[rustfmt::skip]
impl ObservedHostPureInnateRepositoryOwner { pub(crate) fn result(&self) -> &PureResult { &self.result } pub(crate) fn observations(&self) -> &PathObservationEpoch { &self.observations } }
#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostPureInnateRepositoryOwnerObservationError { Inputs(HostSelectedInnateRepositoryOwnerInputsObservationError), LoadRoute(Arc<HostCanonicalRepositoryLoadRouteObservationError>), Path(ObservedPathFrontierError) }

#[derive(Clone, Copy)]
enum Mode {
    Legacy,
    Observed,
}

type Driver = SourcePreparationOutcome<
    Result<(PureResult, PathObservationEpoch), HostPureInnateRepositoryOwnerObservationError>,
>;

#[rustfmt::skip]
fn complete(value: Result<HostPureInnateRepositoryOwnerResult, HostPureInnateRepositoryOwnerError>, observations: PathObservationEpoch) -> Driver { SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations))) }

#[rustfmt::skip]
fn merge_observations(left: &PathObservationEpoch, right: &PathObservationEpoch) -> Result<PathObservationEpoch, HostPureInnateRepositoryOwnerObservationError> {
    PathObservationEpoch::from_shared(left.observations().iter().chain(right.observations().iter()).map(|(demand, result)| (demand.dupe(), result.dupe()))).map_err(|error| HostPureInnateRepositoryOwnerObservationError::Path(ObservedPathFrontierError::from(error)))
}

#[rustfmt::skip]
async fn load_inputs(ctx: &mut DiceComputations<'_>, key: &HostPureInnateRepositoryOwnerKey, mode: Mode) -> Result<(Arc<HostSelectedInnateRepositoryOwnerInputs>, PathObservationEpoch), Driver> {
    let (result, observations) = match mode {
        Mode::Legacy => match ctx.compute(&HostSelectedInnateRepositoryOwnerInputsKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => return Err(complete(Err(HostPureInnateRepositoryOwnerError::Compute(error.to_string().into())), PathObservationEpoch::empty())),
        },
        Mode::Observed => match ctx.compute(&HostSelectedInnateRepositoryOwnerInputsObservationKey::new(key.workspace.dupe(), key.owner.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(HostPureInnateRepositoryOwnerObservationError::Inputs(error)))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => (value.result().dupe(), value.observations().dupe()),
            Err(error) => return Err(complete(Err(HostPureInnateRepositoryOwnerError::Compute(error.to_string().into())), PathObservationEpoch::empty())),
        },
    };
    match result.as_ref() {
        Ok(value) => Ok((Arc::new(value.clone()), observations)),
        Err(error) => Err(complete(Err(HostPureInnateRepositoryOwnerError::Inputs(error.clone())), observations)),
    }
}

async fn load_bzl(
    ctx: &mut DiceComputations<'_>,
    key: &HostPureInnateRepositoryOwnerKey,
    inputs: &HostSelectedInnateRepositoryOwnerInputs,
    observations: PathObservationEpoch,
    mode: Mode,
) -> Result<(FrozenBzlModule, PathObservationEpoch), Driver> {
    let (label, _, _, _) = inputs.definition_parts();
    let target = match RootPackageBzlTarget::parse(label.target().as_str()) {
        Ok(value) => value,
        Err(error) => {
            return Err(complete(
                Err(HostPureInnateRepositoryOwnerError::Label(
                    error.to_string().into(),
                )),
                observations,
            ));
        }
    };
    if label.package().repo().is_root() {
        let root = HostRootBzlLabel::new(label.package().package().clone(), target);
        let (result, incoming) = match mode {
            Mode::Legacy => match ctx
                .compute(&HostBzlModuleEvalKey::new_bzlmod(
                    key.workspace.dupe(),
                    root,
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return Err(SourcePreparationOutcome::Need(need));
                }
                Ok(SourcePreparationOutcome::Complete(value)) => {
                    (value.as_ref().clone(), PathObservationEpoch::empty())
                }
                Err(error) => {
                    return Err(complete(
                        Err(HostPureInnateRepositoryOwnerError::Compute(
                            error.to_string().into(),
                        )),
                        observations,
                    ));
                }
            },
            Mode::Observed => match ctx
                .compute(&HostBzlModuleObservationKey::new_bzlmod(
                    key.workspace.dupe(),
                    root,
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return Err(SourcePreparationOutcome::Need(need));
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return Err(SourcePreparationOutcome::Complete(Err(
                        HostPureInnateRepositoryOwnerObservationError::Path(error),
                    )));
                }
                Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                    (value.result().clone(), value.observations().dupe())
                }
                Err(error) => {
                    return Err(complete(
                        Err(HostPureInnateRepositoryOwnerError::Compute(
                            error.to_string().into(),
                        )),
                        observations,
                    ));
                }
            },
        };
        let observations = merge_observations(&observations, &incoming)
            .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
        return result
            .map(|value| (value, observations.dupe()))
            .map_err(|error| {
                complete(
                    Err(HostPureInnateRepositoryOwnerError::RootBzl(error)),
                    observations,
                )
            });
    }
    let canonical_repo = label.package().repo().clone();
    let (route, incoming) = match mode {
        Mode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                key.workspace.dupe(),
                canonical_repo,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => {
                return Err(complete(
                    Err(HostPureInnateRepositoryOwnerError::Compute(
                        error.to_string().into(),
                    )),
                    observations,
                ));
            }
        },
        Mode::Observed => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                key.workspace.dupe(),
                canonical_repo,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostPureInnateRepositoryOwnerObservationError::LoadRoute(Arc::new(error)),
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                (value.result().dupe(), value.observations().dupe())
            }
            Err(error) => {
                return Err(complete(
                    Err(HostPureInnateRepositoryOwnerError::Compute(
                        error.to_string().into(),
                    )),
                    observations,
                ));
            }
        },
    };
    let observations = merge_observations(&observations, &incoming)
        .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
    let route = match route.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return Err(complete(
                Err(HostPureInnateRepositoryOwnerError::LoadRoute(Arc::new(
                    error.clone(),
                ))),
                observations,
            ));
        }
    };
    let repository_label = match RepositoryBzlLabel::new(label.package().package().clone(), target)
    {
        Ok(value) => value,
        Err(error) => {
            return Err(complete(
                Err(HostPureInnateRepositoryOwnerError::Label(
                    error.to_string().into(),
                )),
                observations,
            ));
        }
    };
    let (result, incoming) = match mode {
        Mode::Legacy => match ctx
            .compute(&ExternalBzlModuleEvalKey::new_canonical_bzlmod(
                route.input().clone(),
                repository_label,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(value)) => {
                (value.as_ref().clone(), PathObservationEpoch::empty())
            }
            Err(error) => {
                return Err(complete(
                    Err(HostPureInnateRepositoryOwnerError::Compute(
                        error.to_string().into(),
                    )),
                    observations,
                ));
            }
        },
        Mode::Observed => match ctx
            .compute(&ExternalBzlModuleObservationKey::new_canonical_bzlmod(
                route.input().clone(),
                repository_label,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostPureInnateRepositoryOwnerObservationError::Path(error),
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                (value.result().as_ref().clone(), value.observations().dupe())
            }
            Err(error) => {
                return Err(complete(
                    Err(HostPureInnateRepositoryOwnerError::Compute(
                        error.to_string().into(),
                    )),
                    observations,
                ));
            }
        },
    };
    let observations = merge_observations(&observations, &incoming)
        .map_err(|error| SourcePreparationOutcome::Complete(Err(error)))?;
    result
        .map(|value| (value, observations.dupe()))
        .map_err(|error| {
            complete(
                Err(HostPureInnateRepositoryOwnerError::ExternalBzl(error)),
                observations,
            )
        })
}

fn projection(
    module: &FrozenBzlModule,
    inputs: &HostSelectedInnateRepositoryOwnerInputs,
) -> Result<RepositoryRuleDefinitionProjection, HostPureInnateRepositoryOwnerError> {
    let (label, name, _, _) = inputs.definition_parts();
    let value = module
        .module
        .get(name)
        .map_err(|error| HostPureInnateRepositoryOwnerError::Export(error.to_string().into()))?;
    let definition = value
        .downcast::<FrozenRepositoryRuleDefinition>()
        .map_err(|_| {
            HostPureInnateRepositoryOwnerError::Export(
                "selected innate export is not repository_rule".into(),
            )
        })?;
    let projection = definition.projection().ok_or_else(|| {
        HostPureInnateRepositoryOwnerError::Export(
            "selected innate repository_rule is not exported".into(),
        )
    })?;
    if &projection.defining_label != label || projection.exported_name != name {
        return Err(HostPureInnateRepositoryOwnerError::Export(
            "selected innate repository_rule projection differs from request".into(),
        ));
    }
    Ok(projection)
}

fn stable_projection(
    first: &RepositoryRuleDefinitionProjection,
    second: &RepositoryRuleDefinitionProjection,
) -> Result<(), HostPureInnateRepositoryOwnerError> {
    (first == second)
        .then_some(())
        .ok_or(HostPureInnateRepositoryOwnerError::Drift)
}

fn call_span(span: &slug_bzlmod_v2::LogicalSpan) -> RepositoryRuleCallSpan {
    RepositoryRuleCallSpan {
        file: span.file.0.clone(),
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

fn call_value(value: &NonrootAttributeValue) -> Result<RepositoryRuleCallValue, CompactString> {
    match value {
        NonrootAttributeValue::None => Ok(RepositoryRuleCallValue::None),
        NonrootAttributeValue::Bool(value) => Ok(RepositoryRuleCallValue::Bool(*value)),
        NonrootAttributeValue::Int(value) => value
            .as_i32()
            .map(RepositoryRuleCallValue::Int)
            .ok_or_else(|| "repository-rule integer is outside i32".into()),
        NonrootAttributeValue::String(value) => Ok(RepositoryRuleCallValue::String(value.clone())),
        NonrootAttributeValue::Label(value) => CanonicalLabel::parse(value)
            .map(RepositoryRuleCallValue::Label)
            .map_err(|error| {
                format!("invalid retained repository-rule Label '{value}': {error}").into()
            }),
        NonrootAttributeValue::List(values) | NonrootAttributeValue::Tuple(values) => values
            .iter()
            .map(call_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| RepositoryRuleCallValue::Sequence(values.into())),
        NonrootAttributeValue::Dict(values) => values
            .iter()
            .map(|(key, value)| Ok((call_key(key)?, call_value(value)?)))
            .collect::<Result<Vec<_>, CompactString>>()
            .map(|values| RepositoryRuleCallValue::Map(values.into())),
        NonrootAttributeValue::Float314
        | NonrootAttributeValue::BuiltinPrint
        | NonrootAttributeValue::ExtensionProxy
        | NonrootAttributeValue::SelfList => {
            Err("unsupported innate repository-rule attribute value".into())
        }
    }
}

#[cfg(test)]
pub(crate) fn call_value_for_test(
    value: &NonrootAttributeValue,
) -> Result<RepositoryRuleCallValue, CompactString> {
    call_value(value)
}

fn call_key(value: &NonrootAttributeKey) -> Result<RepositoryRuleCallKey, CompactString> {
    match value {
        NonrootAttributeKey::String(value) => Ok(RepositoryRuleCallKey::String(value.clone())),
        NonrootAttributeKey::Label(value) => CanonicalLabel::parse(value)
            .map(RepositoryRuleCallKey::Label)
            .map_err(|error| {
                format!("invalid retained repository-rule Label key '{value}': {error}").into()
            }),
        NonrootAttributeKey::DeferredFloat314 => {
            Err("unsupported innate repository-rule dictionary key".into())
        }
    }
}

fn calls(
    inputs: &HostSelectedInnateRepositoryOwnerInputs,
    definition: &RepositoryRuleDefinitionProjection,
) -> Result<Arc<[RepositoryRuleCallRecord]>, HostPureInnateRepositoryOwnerError> {
    inputs
        .tags()
        .iter()
        .map(|tag| {
            if tag.tag_class != "repo" {
                return Err(HostPureInnateRepositoryOwnerError::Call(
                    "innate repository-rule tag class is not repo".into(),
                ));
            }
            let name = tag
                .attributes
                .get("name")
                .and_then(|value| match value {
                    NonrootAttributeValue::String(value) => Some(value.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    HostPureInnateRepositoryOwnerError::Call(
                        "innate repository-rule call lacks a string name".into(),
                    )
                })?;
            let kwargs = tag
                .attributes
                .iter()
                .map(|(name, value)| {
                    call_value(value)
                        .map(|value| (name.clone(), value))
                        .map_err(HostPureInnateRepositoryOwnerError::Call)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let caller = call_span(&tag.location);
            Ok(RepositoryRuleCallRecord {
                definition: definition.clone(),
                name,
                kwargs: kwargs.into(),
                caller: caller.clone(),
                stack: Arc::new([RepositoryRuleCallFrame {
                    function: "<toplevel>".into(),
                    location: Some(caller),
                }]),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Into::into)
}

async fn compute_pure(
    ctx: &mut DiceComputations<'_>,
    key: &HostPureInnateRepositoryOwnerKey,
    mode: Mode,
) -> Driver {
    let (inputs, observations) = match load_inputs(ctx, key, mode).await {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    let (first_module, observations) = match load_bzl(ctx, key, &inputs, observations, mode).await {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    let first = match projection(&first_module, &inputs) {
        Ok(value) => value,
        Err(error) => return complete(Err(error), observations),
    };
    drop(first_module);
    let (module, observations) = match load_bzl(ctx, key, &inputs, observations, mode).await {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    let second = match projection(&module, &inputs) {
        Ok(value) => value,
        Err(error) => return complete(Err(error), observations),
    };
    if let Err(error) = stable_projection(&first, &second) {
        return complete(Err(error), observations);
    }
    match calls(&inputs, &second) {
        Ok(repository_rule_calls) => complete(
            Ok(HostPureInnateRepositoryOwnerResult {
                inputs,
                repository_rule_calls,
            }),
            observations,
        ),
        Err(error) => complete(Err(error), observations),
    }
}

#[async_trait]
#[rustfmt::skip]
impl Key for HostPureInnateRepositoryOwnerKey {
    type Value = SourcePreparationOutcome<PureResult>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value { match compute_pure(ctx, self, Mode::Legacy).await { SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need), SourcePreparationOutcome::Complete(Ok((result, _))) => SourcePreparationOutcome::Complete(result), SourcePreparationOutcome::Complete(Err(_)) => unreachable!() } }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool { x.complete_eq(y) }
    fn validity(value: &Self::Value) -> bool { value.is_complete() }
}

#[async_trait]
#[rustfmt::skip]
impl Key for HostPureInnateRepositoryOwnerObservationKey {
    type Value = SourcePreparationOutcome<Result<ObservedHostPureInnateRepositoryOwner, HostPureInnateRepositoryOwnerObservationError>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value { match compute_pure(ctx, &self.0, Mode::Observed).await { SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need), SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(error)), SourcePreparationOutcome::Complete(Ok((result, observations))) => SourcePreparationOutcome::Complete(Ok(ObservedHostPureInnateRepositoryOwner { result, observations })) } }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool { x.complete_eq(y) }
    fn validity(value: &Self::Value) -> bool { value.is_complete() }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
    use slug_bzlmod_v2::HostSelectedExtensionOwnerKind;
    use slug_bzlmod_v2::OverrideAttributeValue;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::PathDirectoryEntries;
    use slug_workspace_v2::PathDirectoryEntry;
    use slug_workspace_v2::PathDirectoryEntryKind;
    use slug_workspace_v2::PathDirectoryName;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use starlark_map::small_map::SmallMap;

    use super::*;
    use crate::HostSelectedExtensionOwnerCertificateObservationKey;
    use crate::canonical_repository_route_tests::tests::EXTENSION_A;
    use crate::canonical_repository_route_tests::tests::MODULE;
    use crate::canonical_repository_route_tests::tests::WORKSPACE;
    use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
    use crate::canonical_repository_route_tests::tests::builtin_graph_module;
    use crate::canonical_repository_route_tests::tests::transaction;

    async fn selected_owner(
        tx: &mut dice::DiceTransaction,
        workspace: &NormalizedAbsolutePath,
        requested: &str,
    ) -> Arc<HostSelectedExtensionOwner> {
        let demand = tx
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                workspace.dupe(),
                CanonicalRepoName::new(requested).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(demand)) = demand else {
            panic!("innate demand must complete: {demand:?}");
        };
        demand.result().as_ref().as_ref().unwrap().owner().clone()
    }

    fn platform_materialization(
        workspace: &NormalizedAbsolutePath,
        canonical_repo: &str,
    ) -> RepositoryMaterializationEpochEntry {
        RepositoryMaterializationEpochEntry {
            request: Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.dupe(),
                    canonical_repo: CanonicalRepoName::new(canonical_repo).unwrap(),
                },
                repo_spec: RepoSpec {
                    rule_id: RepoRuleId {
                        bzl_file: CanonicalLabel::parse(
                            "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        )
                        .unwrap(),
                        rule_name: "local_repository".into(),
                    },
                    attributes: Arc::new(SmallMap::from_iter([(
                        CompactString::from("path"),
                        OverrideAttributeValue::String("platforms".into()),
                    )])),
                },
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new(format!("{WORKSPACE}/platforms"))
                        .unwrap(),
                },
            }),
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Local,
            ),
        }
    }

    fn fixture_result(
        demand: &PathObservationDemand,
        instance: PathObservationInstanceId,
        logical_root: &Path,
        fixture_root: &Path,
    ) -> PathObservationResult {
        assert_eq!(
            demand.namespace(),
            PathObservationNamespace::Materialization(instance)
        );
        let relative = demand
            .path()
            .as_path()
            .strip_prefix(logical_root)
            .unwrap_or(Path::new(""));
        let actual = fixture_root.join(relative);
        let metadata = std::fs::symlink_metadata(&actual).ok();
        match demand.operation() {
            PathObservationOperation::Lstat => {
                let value = metadata.map(|metadata| {
                    let kind = if metadata.file_type().is_dir() {
                        PathNodeKind::Directory
                    } else if metadata.file_type().is_symlink() {
                        PathNodeKind::Symlink
                    } else {
                        PathNodeKind::RegularFile
                    };
                    PathLstat::new(
                        kind,
                        900 + relative.components().count() as i64,
                        metadata.len() as i64,
                        1,
                        1,
                        if kind == PathNodeKind::Directory {
                            0o755
                        } else {
                            0o644
                        },
                    )
                });
                PathObservationResult::Lstat(
                    value.map_or(PathOperationResult::Missing, PathOperationResult::Present),
                )
            }
            PathObservationOperation::FileBytes => PathObservationResult::FileBytes(
                std::fs::read(actual)
                    .ok()
                    .map(Arc::<[u8]>::from)
                    .map_or(PathOperationResult::Missing, PathOperationResult::Present),
            ),
            PathObservationOperation::DirectoryEntries => {
                let value = std::fs::read_dir(actual).ok().map(|entries| {
                    PathDirectoryEntries::new(entries.map(|entry| {
                        let entry = entry.unwrap();
                        let kind = entry
                            .file_type()
                            .ok()
                            .map(|kind| {
                                if kind.is_dir() {
                                    PathDirectoryEntryKind::Directory
                                } else if kind.is_file() {
                                    PathDirectoryEntryKind::File
                                } else if kind.is_symlink() {
                                    PathDirectoryEntryKind::Symlink
                                } else {
                                    PathDirectoryEntryKind::Unknown
                                }
                            })
                            .unwrap_or(PathDirectoryEntryKind::Unknown);
                        PathDirectoryEntry::new(
                            PathDirectoryName::new(entry.file_name().to_str().unwrap()).unwrap(),
                            kind,
                        )
                    }))
                });
                PathObservationResult::DirectoryEntries(
                    value.map_or(PathOperationResult::Missing, PathOperationResult::Present),
                )
            }
            PathObservationOperation::ReadLink => PathObservationResult::ReadLink(
                std::fs::read_link(actual)
                    .ok()
                    .map(Arc::new)
                    .map_or(PathOperationResult::Missing, PathOperationResult::Present),
            ),
            operation => panic!("unexpected fixture operation: {operation:?}"),
        }
    }

    async fn exact_rules_cc_certificate(
        module: &str,
        requested: &str,
    ) -> Vec<(CanonicalRepoName, RepoSpec, CompactString)> {
        let dice = builtin_graph_dice();
        let mut tx = transaction(&dice, module, "", false, None).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new(requested).unwrap();
        let demand = tx
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                workspace.clone(),
                requested.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(demand)) = demand else {
            panic!("external innate demand must complete");
        };
        let demand = demand.result().as_ref().as_ref().unwrap();
        assert_eq!(demand.requested(), &requested);
        assert_eq!(
            demand.owner().kind(),
            HostSelectedExtensionOwnerKind::InnateRepositoryRule
        );
        let key = HostSelectedExtensionOwnerCertificateObservationKey::new(
            workspace.clone(),
            demand.owner().clone(),
        );
        let SourcePreparationOutcome::Need(need) = tx.compute(&key).await.unwrap() else {
            panic!("external innate must first request rules_cc materialization");
        };
        assert_eq!(need.repository_materializations().len(), 1);
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_eq!(request.id.canonical_repo.as_str(), "rules_cc+");
        let logical_root = PathBuf::from("/innate-rules-cc");
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/v2_oracle/fixtures/nonroot-module-extension-semantics/workspace/registry/modules/rules_cc/0.2.17",
        );
        let instance = PathObservationInstanceId::new(901);
        let global = tx.compute(&PathObservationEpochKey).await.unwrap();
        let mut observations = global
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .collect::<Vec<_>>();
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.dupe(),
                    [
                        platform_materialization(&workspace, "platforms+"),
                        platform_materialization(&workspace, "platforms"),
                        RepositoryMaterializationEpochEntry {
                            request,
                            result: RepositoryMaterializationResult::Success(
                                RepositoryMaterializationSuccess::Immutable {
                                    source_identity: Arc::from("rules-cc-fixture"),
                                    generation_root: logical_root.clone(),
                                    observation_instance: instance,
                                },
                            ),
                        },
                    ],
                )
                .unwrap(),
            )])
            .unwrap();
        tx = updater.commit().await;
        for _ in 0..16 {
            match tx.compute(&key).await.unwrap() {
                SourcePreparationOutcome::Complete(Ok(certificate)) => {
                    let certificate = certificate.result().as_ref().as_ref().unwrap();
                    return certificate
                        .iter()
                        .map(|(canonical, spec, name, _)| {
                            (canonical.clone(), spec.clone(), CompactString::new(name))
                        })
                        .collect();
                }
                SourcePreparationOutcome::Need(need) => {
                    assert!(need.repository_materializations().is_empty(), "{need:?}");
                    let paths = need.path_observations().expect("path retry");
                    for demand in paths.demands() {
                        observations.retain(|(current, _)| current != demand);
                        observations.push((
                            demand.dupe(),
                            Arc::new(fixture_result(
                                demand,
                                instance,
                                &logical_root,
                                &fixture_root,
                            )),
                        ));
                    }
                    let epoch =
                        PathObservationEpoch::from_shared(observations.iter().cloned()).unwrap();
                    let mut updater = tx.into_updater();
                    updater
                        .changed_to(vec![(PathObservationEpochKey, epoch)])
                        .unwrap();
                    tx = updater.commit().await;
                }
                terminal => panic!("external innate certificate failed: {terminal:?}"),
            }
        }
        panic!("external innate certificate did not converge");
    }

    #[tokio::test]
    async fn exact_builtin_winsdk_owner_authenticates_through_canonical_dependency() {
        let module = builtin_graph_module();
        let requested = "bazel_tools+winsdk_configure+local_config_winsdk";
        let rows = exact_rules_cc_certificate(&module, requested).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0.as_str(), requested);
        assert_eq!(rows[0].1.rule_id.rule_name, "winsdk_configure");
    }

    #[tokio::test]
    async fn external_innate_rule_keeps_one_canonical_rules_cc_definition_label() {
        let mut module = builtin_graph_module();
        module.push_str("\nrepo=use_repo_rule('@rules_cc//cc/private/toolchain:cc_configure.bzl','cc_configure')\nrepo(name='out')\n");
        let rows = exact_rules_cc_certificate(&module, "+cc_configure+out").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].1.rule_id.bzl_file,
            CanonicalLabel::parse("@@rules_cc+//cc/private/toolchain:cc_configure.bzl").unwrap()
        );
        assert_eq!(rows[0].1.rule_id.rule_name, "cc_configure");
    }

    #[tokio::test]
    async fn root_innate_rule_authenticates_and_instantiates_retained_call() {
        let dice = builtin_graph_dice();
        let mut module = builtin_graph_module();
        module.push_str(
            "\nrepo=use_repo_rule('//:ext.bzl','repo')\nrepo(name='out', target=':dep')\n",
        );
        let definition = "repo=repository_rule(lambda ctx: None, attrs={'target':attr.label()})\n";
        let mut tx = transaction(&dice, &module, definition, true, None).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let requested = CanonicalRepoName::new("+repo+out").unwrap();
        let demand = tx
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                workspace.clone(),
                requested.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(demand)) = demand else {
            panic!("root innate demand must complete");
        };
        let owner = demand.result().as_ref().as_ref().unwrap().owner().clone();
        let pure = tx
            .compute(&HostPureInnateRepositoryOwnerObservationKey::new(
                workspace.dupe(),
                owner.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(pure)) = pure else {
            panic!("root innate projection must complete: {pure:?}");
        };
        let first = &pure
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .repository_rule_calls[0]
            .definition;
        let mut changed = first.clone();
        changed.configure = !changed.configure;
        assert_eq!(
            stable_projection(first, &changed),
            Err(HostPureInnateRepositoryOwnerError::Drift)
        );
        let certificate = tx
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                workspace, owner,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
            panic!("root innate certificate must complete: {certificate:?}");
        };
        let certificate = certificate.result().as_ref().as_ref().unwrap();
        let rows = certificate.iter().collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, &requested);
        assert_eq!(rows[0].1.rule_id.rule_name, "repo");
        assert_eq!(
            rows[0].1.attributes.get("target"),
            Some(&slug_bzlmod_v2::OverrideAttributeValue::Label(
                slug_identity_v2::CanonicalLabel::parse("@@//:dep").unwrap()
            ))
        );
    }

    #[tokio::test]
    async fn root_innate_calls_keep_order_and_the_admitted_value_matrix() {
        let dice = builtin_graph_dice();
        let mut module = builtin_graph_module();
        module.push_str(
            "\nrepo=use_repo_rule('//:ext.bzl','repo')\nrepo(name='first', s=None, b=True, i=7, l=':dep')\nrepo(name='second', s='two', b=False, i=-3, l='//:other')\n",
        );
        let definition = "repo=repository_rule(lambda ctx: None, attrs={'s':attr.string(), 'b':attr.bool(), 'i':attr.int(), 'l':attr.label()})\n";
        let mut tx = transaction(&dice, &module, definition, true, None).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let owner = selected_owner(&mut tx, &workspace, "+repo+first").await;
        let certificate = tx
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                workspace, owner,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
            panic!("innate certificate must complete: {certificate:?}");
        };
        let rows = certificate
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter().map(|row| row.2).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert!(!rows[0].1.attributes.contains_key("s"));
        assert_eq!(
            rows[0].1.attributes.get("b"),
            Some(&slug_bzlmod_v2::OverrideAttributeValue::Bool(true))
        );
        assert_eq!(
            rows[0].1.attributes.get("i"),
            Some(&slug_bzlmod_v2::OverrideAttributeValue::Int(7))
        );
        assert_eq!(
            rows[1].1.attributes.get("s"),
            Some(&slug_bzlmod_v2::OverrideAttributeValue::String(
                "two".into()
            ))
        );
        assert_eq!(
            rows[1].1.attributes.get("l"),
            Some(&slug_bzlmod_v2::OverrideAttributeValue::Label(
                slug_identity_v2::CanonicalLabel::parse("@@//:other").unwrap()
            ))
        );
    }

    #[tokio::test]
    async fn innate_owner_fails_closed_for_wrong_export_kind_and_unsupported_value() {
        for (definition, export, call, expected) in [
            (
                "other=repository_rule(lambda ctx: None)\n",
                "repo",
                "repo(name='out')",
                "export",
            ),
            (
                "repo=module_extension(implementation=lambda ctx: None)\n",
                "repo",
                "repo(name='out')",
                "export",
            ),
            (
                "other=repository_rule(lambda ctx: None)\nrepo=other\n",
                "repo",
                "repo(name='out')",
                "export",
            ),
            (
                "repo=repository_rule(lambda ctx: None, attrs={'value':attr.string()})\n",
                "repo",
                "repo(name='out', value=['unsupported'])",
                "captured",
            ),
        ] {
            let dice = builtin_graph_dice();
            let mut module = builtin_graph_module();
            module.push_str(&format!(
                "\nrepo=use_repo_rule('//:ext.bzl','{export}')\n{call}\n"
            ));
            let mut tx = transaction(&dice, &module, definition, true, None).await;
            let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
            let owner = selected_owner(&mut tx, &workspace, "+repo+out").await;
            let result = tx
                .compute(&HostPureInnateRepositoryOwnerObservationKey::new(
                    workspace, owner,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(result)) = result else {
                panic!("innate failure must be terminal: {result:?}");
            };
            let accepted = match (expected, result.result().as_ref()) {
                ("export", Err(HostPureInnateRepositoryOwnerError::Export(_))) => true,
                ("captured", Ok(value)) => matches!(
                    value.repository_rule_calls[0].kwargs[0].1,
                    RepositoryRuleCallValue::Sequence(_)
                ),
                _ => false,
            };
            assert!(accepted, "unexpected {expected} result: {result:?}");
        }
    }

    #[tokio::test]
    async fn ordinary_certificate_output_is_unchanged_by_innate_dispatch() {
        let dice = builtin_graph_dice();
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let owner = selected_owner(&mut tx, &workspace, "+ext+first").await;
        assert_eq!(
            owner.kind(),
            HostSelectedExtensionOwnerKind::ModuleExtension
        );
        let certificate = tx
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                workspace, owner,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
            panic!("ordinary certificate must complete: {certificate:?}");
        };
        let rows = certificate
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(rows[0].2, "first");
        assert_eq!(
            rows[0].1.attributes.get("value"),
            Some(&OverrideAttributeValue::String("one".into()))
        );
        assert_eq!(
            rows[0].1.attributes.get("target"),
            Some(&OverrideAttributeValue::Label(
                CanonicalLabel::parse("@@//:local").unwrap()
            ))
        );
    }

    #[tokio::test]
    async fn root_innate_observed_create_edit_delete_recreate_cuts_off_structurally() {
        let dice = builtin_graph_dice();
        let mut module = builtin_graph_module();
        module.push_str("\nrepo=use_repo_rule('//:ext.bzl','repo')\nrepo(name='out')\n");
        let definitions = [
            (
                "repo=repository_rule(lambda ctx: None, attrs={'value':attr.string(default='A')})\n",
                true,
            ),
            (
                "repo=repository_rule(lambda ctx: None, attrs={'value':attr.string(default='B')})\n",
                true,
            ),
            ("", false),
            (
                "repo=repository_rule(lambda ctx: None, attrs={'value':attr.string(default='A')})\n",
                true,
            ),
        ];
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut held = Vec::new();
        for (definition, present) in definitions {
            let mut tx = transaction(&dice, &module, definition, present, None).await;
            let owner = selected_owner(&mut tx, &workspace, "+repo+out").await;
            let outcome = tx
                .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                    workspace.dupe(),
                    owner,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(certificate)) = outcome else {
                panic!("lifecycle result must be terminal: {outcome:?}");
            };
            held.push(certificate.result().clone());
        }
        assert_ne!(held[0], held[1]);
        assert!(held[2].is_err());
        assert_eq!(held[0], held[3]);
    }
}
