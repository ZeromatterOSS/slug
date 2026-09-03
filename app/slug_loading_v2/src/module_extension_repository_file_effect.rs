/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError;
use slug_bzlmod_v2::HostRepositoryLabelPathError;
use slug_bzlmod_v2::HostRepositoryLabelPathKey;
use slug_bzlmod_v2::HostRepositoryLabelPathObservationKey;
use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::HostRepositorySourceFileValue;
use slug_bzlmod_v2::HostRepositorySourceObservation;
use slug_bzlmod_v2::HostRepositorySourceReadKey;
use slug_bzlmod_v2::HostRepositorySourceReadObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
use slug_bzlmod_v2::ObservedHostRepositoryLabelPath;
use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
use slug_bzlmod_v2::RepositoryHostInputTransaction;
use slug_bzlmod_v2::RepositoryLabelPathAddress;
use slug_bzlmod_v2::RepositoryPlatformKey;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_relative_path;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark::PrintHandler;
use starlark::PrintLocation;

use crate::HostCanonicalRepositoryLoadRouteError;
use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::bzl_module::ExternalBzlModuleEvalKey;
use crate::bzl_module::ExternalBzlModuleObservationKey;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostBzlModuleObservationKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::bzl_module::RepositoryBzlLabel;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepository;
use crate::module_extension_repository_rule::FrozenRepositoryRuleDefinition;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificate;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateError;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateKey;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationError;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey;
use crate::repository_rule_context::PreparedRepositoryLabelPaths;
use crate::repository_rule_context::PreparedRepositoryTemplateSources;
use crate::repository_rule_context::RepositoryRuleHostObservation;
use crate::repository_rule_context::RepositoryRuleInvocationError;
use crate::repository_rule_context::RepositoryRuleInvocationInput;
use crate::repository_rule_context::invoke_repository_rule;

const MAX_REPOSITORY_LABEL_PATHS: usize = 256;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedRepositoryFileEffect {
    certificate: Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    host: RepositoryRuleHostObservation,
    plan: GeneratedRepositoryFileEffectPlan,
}

impl HostSelectedRepositoryFileEffect {
    pub fn plan(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.plan
    }

    pub fn host(&self) -> &RepositoryRuleHostObservation {
        &self.host
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedRepositoryFileEffectHostBzlError(CompactString);

impl HostSelectedRepositoryFileEffectHostBzlError {
    fn new(error: impl fmt::Display) -> Self {
        Self(error.to_string().into())
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostSelectedRepositoryFileEffectError {
    Certificate(HostSelectedExtensionOwnerCertificateError),
    Compute(CompactString),
    MissingOrdinal {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
    },
    UnsupportedDefiningLabel {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        label: CanonicalLabel,
    },
    HostBzl {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: HostSelectedRepositoryFileEffectHostBzlError,
    },
    Projection {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        message: CompactString,
    },
    HostInput {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        message: CompactString,
    },
    Path {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: GeneratedRepositoryFileEffectPlanError,
    },
    LabelPathRoute {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        address: RepositoryLabelPathAddress,
        error: Arc<HostCanonicalRepositoryLoadRouteError>,
    },
    LabelPath {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        address: RepositoryLabelPathAddress,
        error: HostRepositoryLabelPathError,
    },
    Invocation {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        message: CompactString,
    },
    Result {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        type_name: CompactString,
    },
}

impl fmt::Display for HostSelectedRepositoryFileEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostSelectedRepositoryFileEffectError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedRepositoryFileEffectKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
}

impl HostSelectedRepositoryFileEffectKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
    ) -> Self {
        Self {
            workspace,
            owner,
            ordinal,
        }
    }
}

impl fmt::Display for HostSelectedRepositoryFileEffectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-repository-file-effect:{}:{:?}:{}",
            self.workspace, self.owner, self.ordinal
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedRepositoryFileEffectObservationKey(HostSelectedRepositoryFileEffectKey);

impl HostSelectedRepositoryFileEffectObservationKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
    ) -> Self {
        Self(HostSelectedRepositoryFileEffectKey::new(
            workspace, owner, ordinal,
        ))
    }
}

impl fmt::Display for HostSelectedRepositoryFileEffectObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type EffectResult =
    Arc<Result<HostSelectedRepositoryFileEffect, HostSelectedRepositoryFileEffectError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedRepositoryFileEffect {
    result: EffectResult,
    observations: PathObservationEpoch,
}

impl ObservedHostSelectedRepositoryFileEffect {
    pub fn result(&self) -> &EffectResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostSelectedRepositoryFileEffectObservationError {
    Certificate(HostSelectedExtensionOwnerCertificateObservationError),
    CanonicalRoute {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: Arc<HostCanonicalRepositoryLoadRouteObservationError>,
    },
    HostBzl {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: ObservedPathFrontierError,
    },
    Merge {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: ObservedPathFrontierError,
    },
}

impl HostSelectedRepositoryFileEffectObservationError {
    #[doc(hidden)]
    pub fn selected_frontier(&self) -> slug_bzlmod_v2::HostSelectedObservationFrontier {
        match self {
            Self::Certificate(error) => error.selected_frontier(),
            Self::CanonicalRoute { error, .. } => error.selected_frontier(),
            Self::HostBzl { error, .. } | Self::Merge { error, .. } => {
                slug_bzlmod_v2::HostSelectedObservationFrontier::Path(error.clone())
            }
        }
    }
}

#[derive(Clone, Copy)]
enum EffectMode {
    Legacy,
    Observed,
}

type EffectDriver = SourcePreparationOutcome<
    Result<(EffectResult, PathObservationEpoch), HostSelectedRepositoryFileEffectObservationError>,
>;

#[rustfmt::skip]
fn complete_effect_error(error: HostSelectedRepositoryFileEffectError, observations: PathObservationEpoch) -> EffectDriver { SourcePreparationOutcome::Complete(Ok((Arc::new(Err(error)), observations))) }

fn merge_definition_observations(
    mode: EffectMode,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    current: PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, EffectDriver> {
    if matches!(mode, EffectMode::Legacy) {
        return Ok(current);
    }
    merge_observations(&current, incoming).map_err(|error| {
        SourcePreparationOutcome::Complete(Err(
            HostSelectedRepositoryFileEffectObservationError::Merge {
                certificate: certificate.clone(),
                ordinal,
                error,
            },
        ))
    })
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

fn merge_observations(
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

enum RepositoryDefinitionLabel {
    Root(HostRootBzlLabel),
    Canonical {
        repo: slug_identity_v2::CanonicalRepoName,
        label: RepositoryBzlLabel,
    },
}

fn definition_label(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    repository: &HostInstantiatedModuleExtensionRepository,
) -> Result<RepositoryDefinitionLabel, HostSelectedRepositoryFileEffectError> {
    let label = &repository.call().definition.defining_label;
    let target = RootPackageBzlTarget::parse(label.target().as_str()).map_err(|_| {
        HostSelectedRepositoryFileEffectError::UnsupportedDefiningLabel {
            certificate: certificate.clone(),
            ordinal,
            label: label.clone(),
        }
    })?;
    if label.package().repo().is_root() {
        Ok(RepositoryDefinitionLabel::Root(HostRootBzlLabel::new(
            label.package().package().clone(),
            target,
        )))
    } else {
        let external =
            RepositoryBzlLabel::new(label.package().package().clone(), target).map_err(|_| {
                HostSelectedRepositoryFileEffectError::UnsupportedDefiningLabel {
                    certificate: certificate.clone(),
                    ordinal,
                    label: label.clone(),
                }
            })?;
        Ok(RepositoryDefinitionLabel::Canonical {
            repo: label.package().repo().clone(),
            label: external,
        })
    }
}

fn authenticate_rule(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    repository: &HostInstantiatedModuleExtensionRepository,
    module: &crate::bzl_module::FrozenBzlModule,
) -> Result<starlark::values::FrozenValue, HostSelectedRepositoryFileEffectError> {
    let call = repository.call();
    let value = module
        .module
        .get_any_visibility(&call.definition.exported_name)
        .map(|(value, _visibility)| value)
        .map_err(|error| HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: error.to_string().into(),
        })?;
    let rule = value
        .downcast::<FrozenRepositoryRuleDefinition>()
        .map_err(|_| HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "selected export is not repository_rule".into(),
        })?;
    let Some(projection) = rule.projection() else {
        return Err(HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "selected repository_rule is not exported".into(),
        });
    };
    if projection != call.definition {
        return Err(HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "reacquired repository_rule projection differs".into(),
        });
    }
    Ok(rule.implementation())
}

fn host_input_error(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    message: impl Into<CompactString>,
) -> HostSelectedRepositoryFileEffectError {
    HostSelectedRepositoryFileEffectError::HostInput {
        certificate: certificate.clone(),
        ordinal,
        message: message.into(),
    }
}

fn environment_need(
    workspace: NormalizedAbsolutePath,
    names: impl IntoIterator<Item = CompactString>,
) -> SourcePreparationNeeds {
    let names = RepositoryEnvironmentNameFrontier::from_unsorted(names);
    SourcePreparationNeeds::environment(
        NeedRepositoryEnvironmentNames::new(workspace, names)
            .expect("an environment retry contains at least one name"),
    )
}

async fn verified_environment(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    transaction: &RepositoryHostInputTransaction,
    names: impl IntoIterator<Item = CompactString>,
) -> Result<Vec<(CompactString, Option<Arc<str>>)>, CompactString> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();
    names.dedup();
    let mut observed = Vec::with_capacity(names.len());
    for name in names {
        let cell = ctx
            .compute(&RepositoryEnvironmentCellKey::new(
                workspace.dupe(),
                name.clone(),
            ))
            .await
            .map_err(|error| CompactString::new(error.to_string()))?;
        let Some(value) = cell.value() else {
            return Err(format!("repository environment name {name:?} is unauthorized").into());
        };
        let expected = transaction.snapshot().get(&name).cloned();
        if value != &expected {
            return Err(
                format!("repository environment name {name:?} differs from request").into(),
            );
        }
        observed.push((name, expected));
    }
    Ok(observed)
}

type DefinitionModuleResult =
    Result<crate::bzl_module::FrozenBzlModule, HostSelectedRepositoryFileEffectHostBzlError>;

#[rustfmt::skip]
async fn load_definition_module(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    label: RepositoryDefinitionLabel,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<(DefinitionModuleResult, PathObservationEpoch), EffectDriver> {
    let complete_compute_error = |message: String, observations: PathObservationEpoch| {
        complete_effect_error(
            HostSelectedRepositoryFileEffectError::Compute(message.into()),
            observations,
        )
    };
    match (label, mode) {
        (RepositoryDefinitionLabel::Root(label), EffectMode::Legacy) => match ctx.compute(&HostBzlModuleEvalKey::new_bzlmod(key.workspace.dupe(), label)).await {
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(value)) => Ok((value.as_ref().clone().map_err(HostSelectedRepositoryFileEffectHostBzlError::new), observations)),
            Err(error) => Err(complete_compute_error(error.to_string(), observations)),
        },
        (RepositoryDefinitionLabel::Root(label), EffectMode::Observed) => match ctx.compute(&HostBzlModuleObservationKey::new_bzlmod(key.workspace.dupe(), label)).await {
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => Err(SourcePreparationOutcome::Complete(Err(HostSelectedRepositoryFileEffectObservationError::HostBzl { certificate: certificate.clone(), ordinal: key.ordinal, error }))),
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                observations = merge_definition_observations(mode, certificate, key.ordinal, observations, value.observations())?;
                Ok((value.result().clone().map_err(HostSelectedRepositoryFileEffectHostBzlError::new), observations))
            }
            Err(error) => Err(complete_compute_error(error.to_string(), observations)),
        },
        (RepositoryDefinitionLabel::Canonical { repo, label }, EffectMode::Legacy) => {
            let input = match ctx.compute(&HostCanonicalRepositoryLoadRouteKey::new(key.workspace.dupe(), repo)).await {
                Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
                Ok(SourcePreparationOutcome::Complete(route)) => route.as_ref().as_ref().map(|route| route.input().clone()).map_err(HostSelectedRepositoryFileEffectHostBzlError::new),
                Err(error) => return Err(complete_compute_error(error.to_string(), observations)),
            };
            let input = match input { Ok(input) => input, Err(error) => return Ok((Err(error), observations)) };
            match ctx.compute(&ExternalBzlModuleEvalKey::new_canonical_bzlmod(input, label)).await {
                Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
                Ok(SourcePreparationOutcome::Complete(value)) => Ok((value.as_ref().clone().map_err(HostSelectedRepositoryFileEffectHostBzlError::new), observations)),
                Err(error) => Err(complete_compute_error(error.to_string(), observations)),
            }
        }
        (RepositoryDefinitionLabel::Canonical { repo, label }, EffectMode::Observed) => {
            let input = match ctx.compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(key.workspace.dupe(), repo)).await {
                Ok(SourcePreparationOutcome::Need(need)) => return Err(SourcePreparationOutcome::Need(need)),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => return Err(SourcePreparationOutcome::Complete(Err(HostSelectedRepositoryFileEffectObservationError::CanonicalRoute { certificate: certificate.clone(), ordinal: key.ordinal, error: Arc::new(error) }))),
                Ok(SourcePreparationOutcome::Complete(Ok(route))) => {
                    observations = merge_definition_observations(mode, certificate, key.ordinal, observations, route.observations())?;
                    route.result().as_ref().as_ref().map(|route| route.input().clone()).map_err(HostSelectedRepositoryFileEffectHostBzlError::new)
                }
                Err(error) => return Err(complete_compute_error(error.to_string(), observations)),
            };
            let input = match input { Ok(input) => input, Err(error) => return Ok((Err(error), observations)) };
            match ctx.compute(&ExternalBzlModuleObservationKey::new_canonical_bzlmod(input, label)).await {
                Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
                Ok(SourcePreparationOutcome::Complete(Err(error))) => Err(SourcePreparationOutcome::Complete(Err(HostSelectedRepositoryFileEffectObservationError::HostBzl { certificate: certificate.clone(), ordinal: key.ordinal, error }))),
                Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                    observations = merge_definition_observations(mode, certificate, key.ordinal, observations, value.observations())?;
                    Ok((value.result().as_ref().clone().map_err(HostSelectedRepositoryFileEffectHostBzlError::new), observations))
                }
                Err(error) => Err(complete_compute_error(error.to_string(), observations)),
            }
        }
    }
}

type RepositoryLabelPathOutcome = SourcePreparationOutcome<
    Arc<Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>>,
>;
type RepositoryLabelPathObservationOutcome =
    SourcePreparationOutcome<Result<ObservedHostRepositoryLabelPath, ObservedPathFrontierError>>;

async fn compute_repository_label_path(
    ctx: &mut DiceComputations<'_>,
    key: HostRepositoryLabelPathKey,
) -> Result<RepositoryLabelPathOutcome, CompactString> {
    ctx.compute(&key)
        .await
        .map_err(|error| error.to_string().into())
}

async fn compute_repository_label_path_observed(
    ctx: &mut DiceComputations<'_>,
    key: HostRepositoryLabelPathObservationKey,
) -> Result<RepositoryLabelPathObservationOutcome, CompactString> {
    ctx.compute(&key)
        .await
        .map_err(|error| error.to_string().into())
}

fn complete_label_path_error(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    address: RepositoryLabelPathAddress,
    error: HostRepositoryLabelPathError,
    observations: PathObservationEpoch,
) -> EffectDriver {
    complete_effect_error(
        HostSelectedRepositoryFileEffectError::LabelPath {
            certificate: certificate.clone(),
            ordinal,
            address,
            error,
        },
        observations,
    )
}

async fn legacy_repository_label_path_route(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: &RepositoryLabelPathAddress,
    observations: PathObservationEpoch,
) -> Result<HostRepositorySourceRoute, EffectDriver> {
    let route = match ctx
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            key.workspace.dupe(),
            address.repo().clone(),
        ))
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        Ok(SourcePreparationOutcome::Complete(route)) => route,
        Err(error) => {
            return Err(complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                observations,
            ));
        }
    };
    match route.as_ref() {
        Ok(route) => Ok(HostRepositorySourceRoute::canonical(route.input().clone())),
        Err(error) => Err(complete_effect_error(
            HostSelectedRepositoryFileEffectError::LabelPathRoute {
                certificate: certificate.clone(),
                ordinal: key.ordinal,
                address: address.clone(),
                error: Arc::new(error.clone()),
            },
            observations,
        )),
    }
}

async fn observed_repository_label_path_route(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: &RepositoryLabelPathAddress,
    mut observations: PathObservationEpoch,
) -> Result<(HostRepositorySourceRoute, PathObservationEpoch), EffectDriver> {
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
                HostSelectedRepositoryFileEffectObservationError::CanonicalRoute {
                    certificate: certificate.clone(),
                    ordinal: key.ordinal,
                    error: Arc::new(error),
                },
            )));
        }
        Ok(SourcePreparationOutcome::Complete(Ok(route))) => route,
        Err(error) => {
            return Err(complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                observations,
            ));
        }
    };
    observations = merge_definition_observations(
        EffectMode::Observed,
        certificate,
        key.ordinal,
        observations,
        route.observations(),
    )?;
    match route.result().as_ref() {
        Ok(route) => Ok((
            HostRepositorySourceRoute::canonical(route.input().clone()),
            observations,
        )),
        Err(error) => Err(complete_effect_error(
            HostSelectedRepositoryFileEffectError::LabelPathRoute {
                certificate: certificate.clone(),
                ordinal: key.ordinal,
                address: address.clone(),
                error: Arc::new(error.clone()),
            },
            observations,
        )),
    }
}

async fn resolve_legacy_repository_label_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: RepositoryLabelPathAddress,
    route: Option<HostRepositorySourceRoute>,
    observations: PathObservationEpoch,
) -> Result<(HostRepositoryLabelPathValue, PathObservationEpoch), EffectDriver> {
    let path_key = match route {
        None => HostRepositoryLabelPathKey::new_root(key.workspace.dupe(), address.clone()),
        Some(route) => HostRepositoryLabelPathKey::new_external(route, address.clone()),
    }
    .map_err(|error| {
        complete_label_path_error(
            certificate,
            key.ordinal,
            address.clone(),
            error,
            observations.dupe(),
        )
    })?;
    match compute_repository_label_path(ctx, path_key).await {
        Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
        Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(value) => Ok((value.clone(), observations)),
            Err(error) => Err(complete_label_path_error(
                certificate,
                key.ordinal,
                address,
                error.clone(),
                observations,
            )),
        },
        Err(message) => Err(complete_effect_error(
            HostSelectedRepositoryFileEffectError::Compute(message),
            observations,
        )),
    }
}

async fn resolve_observed_repository_label_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: RepositoryLabelPathAddress,
    route: Option<HostRepositorySourceRoute>,
    mut observations: PathObservationEpoch,
) -> Result<(HostRepositoryLabelPathValue, PathObservationEpoch), EffectDriver> {
    let path_key = match route {
        None => {
            HostRepositoryLabelPathObservationKey::new_root(key.workspace.dupe(), address.clone())
        }
        Some(route) => HostRepositoryLabelPathObservationKey::new_external(route, address.clone()),
    }
    .map_err(|error| {
        complete_label_path_error(
            certificate,
            key.ordinal,
            address.clone(),
            error,
            observations.dupe(),
        )
    })?;
    let observed = match compute_repository_label_path_observed(ctx, path_key).await {
        Ok(SourcePreparationOutcome::Need(need)) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        Ok(SourcePreparationOutcome::Complete(Err(error))) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedRepositoryFileEffectObservationError::Merge {
                    certificate: certificate.clone(),
                    ordinal: key.ordinal,
                    error,
                },
            )));
        }
        Ok(SourcePreparationOutcome::Complete(Ok(value))) => value,
        Err(message) => {
            return Err(complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(message),
                observations,
            ));
        }
    };
    observations = merge_definition_observations(
        EffectMode::Observed,
        certificate,
        key.ordinal,
        observations,
        observed.observations(),
    )?;
    match observed.result().as_ref() {
        Ok(value) => Ok((value.clone(), observations)),
        Err(error) => Err(complete_label_path_error(
            certificate,
            key.ordinal,
            address,
            error.clone(),
            observations,
        )),
    }
}

async fn resolve_repository_label_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: RepositoryLabelPathAddress,
    mode: EffectMode,
    observations: PathObservationEpoch,
) -> Result<(HostRepositoryLabelPathValue, PathObservationEpoch), EffectDriver> {
    let (route, observations) = if address.repo().is_root() {
        (None, observations)
    } else {
        match mode {
            EffectMode::Legacy => (
                Some(
                    legacy_repository_label_path_route(
                        ctx,
                        key,
                        certificate,
                        &address,
                        observations.dupe(),
                    )
                    .await?,
                ),
                observations,
            ),
            EffectMode::Observed => {
                let (route, observations) = observed_repository_label_path_route(
                    ctx,
                    key,
                    certificate,
                    &address,
                    observations,
                )
                .await?;
                (Some(route), observations)
            }
        }
    };

    match mode {
        EffectMode::Legacy => {
            resolve_legacy_repository_label_path(
                ctx,
                key,
                certificate,
                address,
                route,
                observations,
            )
            .await
        }
        EffectMode::Observed => {
            resolve_observed_repository_label_path(
                ctx,
                key,
                certificate,
                address,
                route,
                observations,
            )
            .await
        }
    }
}

fn template_source_error(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    message: impl Into<CompactString>,
    observations: PathObservationEpoch,
) -> EffectDriver {
    complete_effect_error(
        HostSelectedRepositoryFileEffectError::Invocation {
            certificate: certificate.clone(),
            ordinal,
            message: message.into(),
        },
        observations,
    )
}

fn template_relative_path(
    address: &RepositoryLabelPathAddress,
) -> Result<slug_bzlmod_v2::HostRepositoryRelativePath, CompactString> {
    host_repository_relative_path(
        std::path::PathBuf::from(address.package().as_str()).join(address.target().as_str()),
    )
    .map_err(|error| error.to_string().into())
}

fn template_source_bytes(
    source: &HostRepositorySourceFileValue,
) -> Result<Arc<[u8]>, CompactString> {
    match source {
        HostRepositorySourceFileValue::Present { bytes, .. } => Ok(bytes.dupe()),
        HostRepositorySourceFileValue::Absent => {
            Err("repository_ctx.template source is absent".into())
        }
    }
}

async fn read_legacy_template_source(
    ctx: &mut DiceComputations<'_>,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    observations: PathObservationEpoch,
    route: HostRepositorySourceRoute,
    relative: slug_bzlmod_v2::HostRepositoryRelativePath,
) -> Result<Arc<[u8]>, EffectDriver> {
    match route.source_read_key(relative) {
        HostRepositorySourceReadKey::RootRequest(_) => {
            unreachable!("template sources are canonical external routes")
        }
        HostRepositorySourceReadKey::Observation(source_key) => {
            match ctx.compute(&source_key).await {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    Err(SourcePreparationOutcome::Need(need))
                }
                Ok(SourcePreparationOutcome::Complete(result)) => match result.as_ref() {
                    Ok(HostRepositorySourceObservation::Request(source)) => {
                        template_source_bytes(source).map_err(|error| {
                            template_source_error(certificate, ordinal, error, observations)
                        })
                    }
                    Ok(HostRepositorySourceObservation::Builtin(_)) => Err(template_source_error(
                        certificate,
                        ordinal,
                        "repository_ctx.template built-in source is unsupported",
                        observations,
                    )),
                    Err(error) => Err(template_source_error(
                        certificate,
                        ordinal,
                        error.to_string(),
                        observations,
                    )),
                },
                Err(error) => Err(template_source_error(
                    certificate,
                    ordinal,
                    error.to_string(),
                    observations,
                )),
            }
        }
    }
}

async fn read_observed_template_source(
    ctx: &mut DiceComputations<'_>,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    observations: PathObservationEpoch,
    route: HostRepositorySourceRoute,
    relative: slug_bzlmod_v2::HostRepositoryRelativePath,
) -> Result<(Arc<[u8]>, PathObservationEpoch), EffectDriver> {
    match route.source_read_observation_key(relative) {
        HostRepositorySourceReadObservationKey::RootRequest(_) => {
            unreachable!("template sources are canonical external routes")
        }
        HostRepositorySourceReadObservationKey::Observation(source_key) => match ctx
            .compute(&source_key)
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedRepositoryFileEffectObservationError::Merge {
                        certificate: certificate.clone(),
                        ordinal,
                        error,
                    },
                )))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                let observations = merge_definition_observations(
                    EffectMode::Observed,
                    certificate,
                    ordinal,
                    observations,
                    observed.observations(),
                )?;
                match observed.result().as_ref() {
                    Ok(HostRepositorySourceObservation::Request(source)) => {
                        template_source_bytes(source)
                            .map(|bytes| (bytes, observations.dupe()))
                            .map_err(|error| {
                                template_source_error(certificate, ordinal, error, observations)
                            })
                    }
                    Ok(HostRepositorySourceObservation::Builtin(_)) => Err(template_source_error(
                        certificate,
                        ordinal,
                        "repository_ctx.template built-in source is unsupported",
                        observations,
                    )),
                    Err(error) => Err(template_source_error(
                        certificate,
                        ordinal,
                        error.to_string(),
                        observations,
                    )),
                }
            }
            Err(error) => Err(template_source_error(
                certificate,
                ordinal,
                error.to_string(),
                observations,
            )),
        },
    }
}

async fn resolve_template_source(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: &RepositoryLabelPathAddress,
    mode: EffectMode,
    observations: PathObservationEpoch,
) -> Result<(Arc<[u8]>, PathObservationEpoch), EffectDriver> {
    if address.repo().is_root() {
        return Err(template_source_error(
            certificate,
            key.ordinal,
            "repository_ctx.template root source is unsupported",
            observations,
        ));
    }
    let relative = template_relative_path(address).map_err(|error| {
        template_source_error(certificate, key.ordinal, error, observations.dupe())
    })?;
    match mode {
        EffectMode::Legacy => {
            let route = legacy_repository_label_path_route(
                ctx,
                key,
                certificate,
                address,
                observations.dupe(),
            )
            .await?;
            read_legacy_template_source(
                ctx,
                certificate,
                key.ordinal,
                observations.clone(),
                route,
                relative,
            )
            .await
            .map(|bytes| (bytes, observations))
        }
        EffectMode::Observed => {
            let (route, observations) =
                observed_repository_label_path_route(ctx, key, certificate, address, observations)
                    .await?;
            let (bytes, incoming) = read_observed_template_source(
                ctx,
                certificate,
                key.ordinal,
                observations.dupe(),
                route,
                relative,
            )
            .await?;
            Ok((bytes, incoming))
        }
    }
}

fn terminal_repository_rule_invocation_error(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    error: RepositoryRuleInvocationError,
) -> HostSelectedRepositoryFileEffectError {
    match error {
        RepositoryRuleInvocationError::PathArgument => {
            HostSelectedRepositoryFileEffectError::Invocation {
                certificate: certificate.clone(),
                ordinal,
                message: "repository_ctx.file path must be a string".into(),
            }
        }
        RepositoryRuleInvocationError::LabelPathArgument => {
            HostSelectedRepositoryFileEffectError::Invocation {
                certificate: certificate.clone(),
                ordinal,
                message: "repository_ctx.path argument must be a Label".into(),
            }
        }
        RepositoryRuleInvocationError::TemplateDestinationArgument
        | RepositoryRuleInvocationError::TemplateSourceArgument
        | RepositoryRuleInvocationError::TemplateSubstitutions
        | RepositoryRuleInvocationError::TemplateLimit => {
            HostSelectedRepositoryFileEffectError::Invocation {
                certificate: certificate.clone(),
                ordinal,
                message: "repository_ctx.template invocation is unsupported".into(),
            }
        }
        RepositoryRuleInvocationError::Plan(error) => HostSelectedRepositoryFileEffectError::Path {
            certificate: certificate.clone(),
            ordinal,
            error,
        },
        RepositoryRuleInvocationError::Evaluation(message) => {
            HostSelectedRepositoryFileEffectError::Invocation {
                certificate: certificate.clone(),
                ordinal,
                message,
            }
        }
        RepositoryRuleInvocationError::Result(type_name) => {
            HostSelectedRepositoryFileEffectError::Result {
                certificate: certificate.clone(),
                ordinal,
                type_name,
            }
        }
        RepositoryRuleInvocationError::LabelPathNeed(_)
        | RepositoryRuleInvocationError::TemplateSourceNeed(_) => unreachable!(),
    }
}

async fn invoke_repository_rule_with_label_paths(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    module: &crate::bzl_module::FrozenBzlModule,
    implementation: starlark::values::FrozenValue,
    input: &RepositoryRuleInvocationInput,
    platform: &slug_bzlmod_v2::RepositoryPlatform,
    transaction: &RepositoryHostInputTransaction,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
    capture_enabled: bool,
) -> Result<
    (
        crate::repository_rule_context::RepositoryRuleInvocation,
        Option<InvocationPrintCapture>,
        PathObservationEpoch,
    ),
    EffectDriver,
> {
    let mut prepared_paths = PreparedRepositoryLabelPaths::new();
    let mut prepared_templates = PreparedRepositoryTemplateSources::new();
    loop {
        let capture = capture_enabled.then(InvocationPrintCapture::default);
        match invoke_repository_rule(
            implementation,
            &module.manifest,
            &prepared_paths,
            &prepared_templates,
            input.clone(),
            platform.clone(),
            transaction.snapshot().dupe(),
            capture.as_ref().map(|capture| capture as &dyn PrintHandler),
        ) {
            Ok(invocation) => return Ok((invocation, capture, observations)),
            Err(RepositoryRuleInvocationError::LabelPathNeed(address)) => {
                if prepared_paths.contains_key(&address)
                    || prepared_paths.len() == MAX_REPOSITORY_LABEL_PATHS
                {
                    let message = if prepared_paths.contains_key(&address) {
                        "repository_ctx.path repeated an already prepared Label path".into()
                    } else {
                        format!(
                            "repository_ctx.path exceeds the per-invocation limit of {MAX_REPOSITORY_LABEL_PATHS} distinct Labels"
                        )
                        .into()
                    };
                    return Err(complete_effect_error(
                        HostSelectedRepositoryFileEffectError::Invocation {
                            certificate: certificate.clone(),
                            ordinal: key.ordinal,
                            message,
                        },
                        observations,
                    ));
                }
                let (value, next_observations) = resolve_repository_label_path(
                    ctx,
                    key,
                    certificate,
                    address.clone(),
                    mode,
                    observations,
                )
                .await?;
                observations = next_observations;
                prepared_paths.insert(address, value);
            }
            Err(RepositoryRuleInvocationError::TemplateSourceNeed(address)) => {
                if prepared_templates.contains_key(&address)
                    || prepared_templates.len() == MAX_REPOSITORY_LABEL_PATHS
                {
                    return Err(template_source_error(
                        certificate,
                        key.ordinal,
                        "repository_ctx.template repeated or exceeded its source limit",
                        observations,
                    ));
                }
                let (bytes, next_observations) =
                    resolve_template_source(ctx, key, certificate, &address, mode, observations)
                        .await?;
                observations = next_observations;
                prepared_templates.insert(address, bytes);
            }
            Err(error) => {
                if let Some(capture) = capture {
                    ctx.store_evaluation_data(capture.into_batch())
                        .expect("repository-file invocation stores one local Complete event batch");
                }
                return Err(complete_effect_error(
                    terminal_repository_rule_invocation_error(certificate, key.ordinal, error),
                    observations,
                ));
            }
        }
    }
}

async fn compute_effect(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    mode: EffectMode,
) -> EffectDriver {
    let (certificate, observations) = match mode {
        EffectMode::Legacy => match ctx
            .compute(&HostSelectedExtensionOwnerCertificateKey::new(
                key.workspace.dupe(),
                key.owner.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => {
                return complete_effect_error(
                    HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                    PathObservationEpoch::empty(),
                );
            }
        },
        EffectMode::Observed => match ctx
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                key.workspace.dupe(),
                key.owner.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostSelectedRepositoryFileEffectObservationError::Certificate(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                (value.result().dupe(), value.observations().dupe())
            }
            Err(error) => {
                return complete_effect_error(
                    HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                    PathObservationEpoch::empty(),
                );
            }
        },
    };
    let certificate = match certificate.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => {
            return complete_effect_error(
                HostSelectedRepositoryFileEffectError::Certificate(error.clone()),
                observations,
            );
        }
    };
    let Some(repository) = certificate.repository(key.ordinal) else {
        return complete_effect_error(
            HostSelectedRepositoryFileEffectError::MissingOrdinal {
                certificate,
                ordinal: key.ordinal,
            },
            observations,
        );
    };
    let repository = repository.clone();
    let label = match definition_label(&certificate, key.ordinal, &repository) {
        Ok(label) => label,
        Err(error) => return complete_effect_error(error, observations),
    };
    let (module, observations) =
        match load_definition_module(ctx, key, &certificate, label, mode, observations).await {
            Ok(value) => value,
            Err(terminal) => return terminal,
        };
    let module = match module {
        Ok(module) => module,
        Err(error) => {
            return complete_effect_error(
                HostSelectedRepositoryFileEffectError::HostBzl {
                    certificate: certificate.clone(),
                    ordinal: key.ordinal,
                    error,
                },
                observations,
            );
        }
    };
    let implementation = match authenticate_rule(&certificate, key.ordinal, &repository, &module) {
        Ok(value) => value,
        Err(error) => return complete_effect_error(error, observations),
    };
    let (canonical_name, spec) = repository.spec_parts();
    let input = match RepositoryRuleInvocationInput::new(
        canonical_name.as_str().into(),
        Some(repository.generated_name().into()),
        spec.attributes.clone(),
        repository.call().definition.attributes.clone(),
    ) {
        Ok(input) => input,
        Err(message) => {
            return complete_effect_error(
                HostSelectedRepositoryFileEffectError::Projection {
                    certificate: certificate.clone(),
                    ordinal: key.ordinal,
                    message,
                },
                observations,
            );
        }
    };
    let transaction = match ctx
        .per_transaction_data()
        .data
        .get::<RepositoryHostInputTransaction>()
    {
        Ok(transaction) => transaction.clone(),
        Err(error) => {
            return complete_effect_error(
                host_input_error(&certificate, key.ordinal, error.to_string()),
                observations,
            );
        }
    };
    let declared = repository
        .call()
        .definition
        .environment
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let unknown = declared
        .iter()
        .filter(|name| !transaction.frontier().contains(name))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return SourcePreparationOutcome::Need(environment_need(key.workspace.dupe(), unknown));
    }
    let platform = match ctx
        .compute(&RepositoryPlatformKey::new(key.workspace.dupe()))
        .await
    {
        Ok(platform) => platform,
        Err(error) => {
            return complete_effect_error(
                host_input_error(&certificate, key.ordinal, error.to_string()),
                observations,
            );
        }
    };
    if platform.os_name().eq_ignore_ascii_case("windows") {
        return complete_effect_error(
            host_input_error(
                &certificate,
                key.ordinal,
                "Windows repository-rule execution is unsupported",
            ),
            observations,
        );
    }
    let declared_observed =
        match verified_environment(ctx, &key.workspace, &transaction, declared.iter().cloned())
            .await
        {
            Ok(observed) => observed,
            Err(message) => {
                return complete_effect_error(
                    host_input_error(&certificate, key.ordinal, message),
                    observations,
                );
            }
        };
    let capture_enabled = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok();
    let (invocation, capture, observations) = match invoke_repository_rule_with_label_paths(
        ctx,
        key,
        &certificate,
        &module,
        implementation,
        &input,
        &platform,
        &transaction,
        mode,
        observations,
        capture_enabled,
    )
    .await
    {
        Ok(value) => value,
        Err(terminal) => return terminal,
    };
    let unknown = invocation
        .dynamic_environment()
        .iter()
        .filter(|name| !transaction.frontier().contains(name))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return SourcePreparationOutcome::Need(environment_need(key.workspace.dupe(), unknown));
    }
    let dynamic_observed = match verified_environment(
        ctx,
        &key.workspace,
        &transaction,
        invocation.dynamic_environment().iter().cloned(),
    )
    .await
    {
        Ok(observed) => observed,
        Err(message) => {
            return complete_effect_error(
                host_input_error(&certificate, key.ordinal, message),
                observations,
            );
        }
    };
    if let Some(capture) = capture {
        ctx.store_evaluation_data(capture.into_batch())
            .expect("repository-file invocation stores one local Complete event batch");
    }
    let result = Ok(HostSelectedRepositoryFileEffect {
        certificate,
        ordinal: key.ordinal,
        host: RepositoryRuleHostObservation::new(
            platform,
            declared_observed.into_iter().chain(dynamic_observed),
        ),
        plan: invocation.into_plan(),
    });
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

#[async_trait]
impl Key for HostSelectedRepositoryFileEffectKey {
    type Value = SourcePreparationOutcome<EffectResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_effect(ctx, self, EffectMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy repository-file effect has no observed outer")
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
impl Key for HostSelectedRepositoryFileEffectObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedRepositoryFileEffect,
            HostSelectedRepositoryFileEffectObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_effect(ctx, &self.0, EffectMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedRepositoryFileEffect {
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
    use slug_bzlmod_v2::OverrideAttributeValue;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryEnvironmentCell;
    use slug_bzlmod_v2::RepositoryEnvironmentEntry;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPlatform;
    use slug_bzlmod_v2::SourcePreparationNeeds;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathDirectoryEntries;
    use slug_workspace_v2::PathDirectoryEntry;
    use slug_workspace_v2::PathDirectoryEntryKind;
    use slug_workspace_v2::PathDirectoryName;
    use slug_workspace_v2::PathIoErrorKind;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationError;
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use starlark_map::small_map::SmallMap;

    use super::*;
    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    use crate::module_extension_repository_instantiation::tests::transaction_untracked as base_transaction_untracked;
    use crate::module_extension_repository_instantiation::tests::transaction_with_tracker as base_transaction_with_tracker;

    async fn with_host_inputs_for(
        transaction: dice::DiceTransaction,
        workspace: NormalizedAbsolutePath,
        tracker: Option<Arc<dyn ActivationTracker>>,
        platform: RepositoryPlatform,
        snapshot: RepositoryEnvironmentSnapshot,
        frontier: RepositoryEnvironmentNameFrontier,
    ) -> dice::DiceTransaction {
        let mut data = UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data.data.set(RepositoryHostInputTransaction::new(
            snapshot.clone(),
            frontier.clone(),
        ));
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                RepositoryPlatformKey::new(workspace.dupe()),
                platform,
            )])
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
        updater.commit_with_data(data).await
    }

    async fn with_host_inputs(
        transaction: dice::DiceTransaction,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        with_host_inputs_for(
            transaction,
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            tracker,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::empty(),
        )
        .await
    }

    async fn transaction_untracked(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
    ) -> dice::DiceTransaction {
        with_host_inputs(
            base_transaction_untracked(dice, module_source, extension_source, extension_present)
                .await,
            None,
        )
        .await
    }

    async fn transaction_with_tracker(
        dice: &Arc<Dice>,
        module_source: &str,
        extension_source: &str,
        extension_present: bool,
        tracker: Arc<dyn ActivationTracker>,
    ) -> dice::DiceTransaction {
        with_host_inputs(
            base_transaction_with_tracker(
                dice,
                module_source,
                extension_source,
                extension_present,
                tracker.clone(),
            )
            .await,
            Some(tracker),
        )
        .await
    }

    fn environment_snapshot(entries: &[(&str, &str)]) -> RepositoryEnvironmentSnapshot {
        RepositoryEnvironmentSnapshot::from_canonical(
            entries
                .iter()
                .map(|(name, value)| RepositoryEnvironmentEntry::new(*name, *value))
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn environment_frontier(names: &[&str]) -> RepositoryEnvironmentNameFrontier {
        RepositoryEnvironmentNameFrontier::from_unsorted(
            names.iter().map(|name| CompactString::new(*name)),
        )
    }

    const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, generated='first')\n";
    const EXTENSION: &str = r#"
def write(ctx):
    print('repository-file-effect')
    ctx.file('BUILD.bazel', content='exports_files([\"generated.txt\"])\\n')
    ctx.file('generated.txt', 'from-rule', executable=False)
_repo=repository_rule(implementation=write)
def impl(ctx):
    _repo(name='first')
_ext=module_extension(implementation=impl)
ext=_ext
"#;
    const MSVC_ENVVARS_FIXTURE: &str = r#"MSVC_ENVVARS = [
    "BAZEL_VC",
    "BAZEL_VC_FULL_VERSION",
    "BAZEL_VS",
    "BAZEL_WINSDK_FULL_VERSION",
    "VS90COMNTOOLS",
    "VS100COMNTOOLS",
    "VS110COMNTOOLS",
    "VS120COMNTOOLS",
    "VS140COMNTOOLS",
    "VS150COMNTOOLS",
    "VS160COMNTOOLS",
    "TMP",
    "TEMP",
]

def find_vc_path(*args, **kwargs):
    return None

def setup_vc_env_vars(*args, **kwargs):
    return {}
"#;
    const TEMPLATE_RULE_FIXTURE: &str = r#"def _cc_configure_impl(ctx):
    source = ctx.path(Label("//cc/private/toolchain:BUILD.toolchains.tpl"))
    print("before-template")
    ctx.template("BUILD", source, {"%{name}": "%{cpu}", "%{cpu}": "k8"})
    ctx.template("COPY", source, {"%{name}": "arm"}, executable = False)

cc_configure = repository_rule(implementation = _cc_configure_impl)
"#;
    const TEMPLATE_PATH: &str = "cc/private/toolchain/BUILD.toolchains.tpl";
    const TEMPLATE_A: &[u8] = b"toolchain=%{name}\nraw=\xff\n";

    #[derive(Default)]
    struct EffectTracker(Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>);

    impl EffectTracker {
        fn take(&self) -> Vec<(String, ActivationKind, Option<EventBatch>)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl ActivationTracker for EffectTracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.0.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
        }
    }

    async fn owner_named(
        transaction: &mut dice::DiceTransaction,
        workspace: NormalizedAbsolutePath,
        requested: &str,
    ) -> Arc<HostSelectedExtensionOwner> {
        let requested = CanonicalRepoName::new(requested).unwrap();
        let demand = transaction
            .compute(&HostSelectedExtensionDemandKey::new(workspace, requested))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(demand) = demand else {
            panic!("selected demand must complete")
        };
        demand.as_ref().as_ref().unwrap().owner().clone()
    }

    async fn owner(transaction: &mut dice::DiceTransaction) -> Arc<HostSelectedExtensionOwner> {
        owner_named(
            transaction,
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            "+ext+first",
        )
        .await
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
                    logical_root: NormalizedAbsolutePath::new(format!(
                        "{}/platforms",
                        workspace.as_path().display()
                    ))
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
                        950 + relative.components().count() as i64,
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
            PathObservationOperation::FileBytes => {
                let value = if relative == Path::new("cc/toolchains/toolchain_config_utils.bzl") {
                    Some(Arc::<[u8]>::from(MSVC_ENVVARS_FIXTURE.as_bytes()))
                } else {
                    std::fs::read(actual).ok().map(Arc::<[u8]>::from)
                };
                PathObservationResult::FileBytes(
                    value.map_or(PathOperationResult::Missing, PathOperationResult::Present),
                )
            }
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

    fn fixture_result_with_template(
        demand: &PathObservationDemand,
        instance: PathObservationInstanceId,
        logical_root: &Path,
        fixture_root: &Path,
        template: &[u8],
    ) -> PathObservationResult {
        let relative = demand
            .path()
            .as_path()
            .strip_prefix(logical_root)
            .unwrap_or(Path::new(""));
        if relative == Path::new(TEMPLATE_PATH) {
            return match demand.operation() {
                PathObservationOperation::Lstat => {
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        999,
                        template.len() as i64,
                        1,
                        1,
                        0o644,
                    )))
                }
                PathObservationOperation::FileBytes => PathObservationResult::FileBytes(
                    PathOperationResult::Present(Arc::<[u8]>::from(template)),
                ),
                operation => panic!("unexpected template operation: {operation:?}"),
            };
        }
        if relative == Path::new("cc/private/toolchain/cc_configure.bzl")
            && demand.operation() == PathObservationOperation::FileBytes
        {
            return PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                TEMPLATE_RULE_FIXTURE.as_bytes(),
            )));
        }
        fixture_result(demand, instance, logical_root, fixture_root)
    }

    fn replace_template_observations(
        epoch: &PathObservationEpoch,
        bytes: &[u8],
    ) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(epoch.observations().iter().map(|(demand, result)| {
            let replacement = if demand.path().as_path().ends_with(TEMPLATE_PATH) {
                match demand.operation() {
                    PathObservationOperation::Lstat => {
                        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                            PathNodeKind::RegularFile,
                            999,
                            bytes.len() as i64,
                            1,
                            1,
                            0o644,
                        )))
                    }
                    PathObservationOperation::FileBytes => PathObservationResult::FileBytes(
                        PathOperationResult::Present(Arc::<[u8]>::from(bytes)),
                    ),
                    operation => panic!("unexpected template operation: {operation:?}"),
                }
            } else {
                return (demand.dupe(), result.dupe());
            };
            (demand.dupe(), Arc::new(replacement))
        }))
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum TemplateSourceFailure {
        Missing,
        Directory,
        Unreadable,
    }

    #[rustfmt::skip]
    fn failing_template_observations(epoch: &PathObservationEpoch, failure: TemplateSourceFailure) -> PathObservationEpoch {
        PathObservationEpoch::from_shared(epoch.observations().iter().map(|(demand, result)| {
            let replacement = match (failure, demand.operation(), demand.path().as_path().ends_with(TEMPLATE_PATH)) {
                (TemplateSourceFailure::Missing, PathObservationOperation::Lstat, true) => Some(PathObservationResult::Lstat(PathOperationResult::Missing)),
                (TemplateSourceFailure::Directory, PathObservationOperation::Lstat, true) => Some(PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(PathNodeKind::Directory, 0, 1, 1, 999, 0o755)))),
                (TemplateSourceFailure::Unreadable, PathObservationOperation::FileBytes, true) => Some(PathObservationResult::FileBytes(PathOperationResult::Error(PathObservationError::Io { kind: PathIoErrorKind::PermissionDenied, raw_os_error: None }))),
                _ => None,
            };
            (demand.dupe(), replacement.map_or_else(|| result.dupe(), Arc::new))
        }))
        .unwrap()
    }

    async fn update_template_need(
        transaction: dice::DiceTransaction,
        workspace: &NormalizedAbsolutePath,
        need: &SourcePreparationNeeds,
        observations: &mut Vec<(PathObservationDemand, Arc<PathObservationResult>)>,
        logical_root: &Path,
        fixture_root: &Path,
        instance: PathObservationInstanceId,
    ) -> dice::DiceTransaction {
        let mut updater = transaction.into_updater();
        if let Some(request) = need.repository_materializations().values().next() {
            assert_eq!(need.repository_materializations().len(), 1);
            assert_eq!(request.id.canonical_repo.as_str(), "rules_cc+");
            updater
                .changed_to(vec![(
                    RepositoryMaterializationResultEpochKey {
                        workspace: workspace.dupe(),
                    },
                    RepositoryMaterializationResultEpoch::new(
                        workspace.dupe(),
                        [
                            platform_materialization(workspace, "platforms+"),
                            platform_materialization(workspace, "platforms"),
                            RepositoryMaterializationEpochEntry {
                                request: request.clone(),
                                result: RepositoryMaterializationResult::Success(
                                    RepositoryMaterializationSuccess::Immutable {
                                        source_identity: Arc::from("template-rules-cc-fixture"),
                                        generation_root: logical_root.to_owned(),
                                        observation_instance: instance,
                                    },
                                ),
                            },
                        ],
                    )
                    .unwrap(),
                )])
                .unwrap();
        } else {
            for demand in need
                .path_observations()
                .expect("template path retry")
                .demands()
            {
                observations.retain(|(current, _)| current != demand);
                observations.push((
                    demand.dupe(),
                    Arc::new(fixture_result_with_template(
                        demand,
                        instance,
                        logical_root,
                        fixture_root,
                        TEMPLATE_A,
                    )),
                ));
            }
            updater
                .changed_to(vec![(
                    PathObservationEpochKey,
                    PathObservationEpoch::from_shared(observations.iter().cloned()).unwrap(),
                )])
                .unwrap();
        }
        updater.commit().await
    }

    async fn converge_observed_template(
        mut transaction: dice::DiceTransaction,
        key: &HostSelectedRepositoryFileEffectObservationKey,
        workspace: &NormalizedAbsolutePath,
        logical_root: &Path,
        fixture_root: &Path,
        instance: PathObservationInstanceId,
    ) -> (
        dice::DiceTransaction,
        ObservedHostSelectedRepositoryFileEffect,
    ) {
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let mut observations = global
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .collect::<Vec<_>>();
        for _ in 0..24 {
            match transaction.compute(key).await.unwrap() {
                SourcePreparationOutcome::Complete(Ok(value)) => return (transaction, value),
                SourcePreparationOutcome::Need(need) => {
                    assert!(need.repository_environment().is_none(), "{need:?}");
                    transaction = update_template_need(
                        transaction,
                        workspace,
                        &need,
                        &mut observations,
                        logical_root,
                        fixture_root,
                        instance,
                    )
                    .await;
                }
                terminal => panic!("template effect failed: {terminal:?}"),
            }
        }
        panic!("template effect did not converge")
    }

    #[tokio::test]
    async fn repository_label_path_retries_without_publishing_partial_attempts() {
        const PATH_EXTENSION: &str = r#"
def write(ctx):
    print("before-path")
    first = ctx.path(Label("//:missing-first"))
    print("between-paths")
    second = ctx.path(Label("//:missing-second"))
    print("after-path")
    ctx.file("resolved", "%s|%s|%s" % (first, second, ctx.path(Label("//:missing-first"))), executable = False)
repo = repository_rule(implementation = write)
def impl(ctx):
    repo(name = "first")
ext = module_extension(implementation = impl)
"#;
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(EffectTracker::default());
        let mut transaction =
            transaction_with_tracker(&dice, MODULE, PATH_EXTENSION, true, tracker.clone()).await;
        let owner = owner(&mut transaction).await;
        tracker.take();

        let observed_key =
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), owner.clone(), 0);
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&observed_key).await.unwrap()
        else {
            panic!("observed Label path must complete")
        };
        let effect = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(
            effect.plan().effects()[0].content(),
            format!(
                "{WORKSPACE}/missing-first|{WORKSPACE}/missing-second|{WORKSPACE}/missing-first"
            )
            .as_bytes()
        );
        assert!(
            observed
                .observations()
                .observations()
                .keys()
                .all(|demand| !demand.path().as_path().ends_with("missing-first")
                    && !demand.path().as_path().ends_with("missing-second"))
        );
        let events = tracker
            .take()
            .into_iter()
            .find_map(|(name, _, batch)| {
                (name == observed_key.to_string())
                    .then_some(batch)
                    .flatten()
            })
            .unwrap();
        assert_eq!(
            events
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["before-path", "between-paths", "after-path"]
        );
        let warm = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(warm)) = warm else {
            panic!("warm observed Label path must complete")
        };
        assert!(Arc::ptr_eq(observed.result(), warm.result()));
        assert!(tracker.take().iter().any(|(name, kind, batch)| {
            name == &observed_key.to_string() && *kind == ActivationKind::Reused && batch.is_none()
        }));

        let SourcePreparationOutcome::Complete(legacy) = transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace, owner, 0,
            ))
            .await
            .unwrap()
        else {
            panic!("legacy Label path must complete")
        };
        assert_eq!(legacy.as_ref(), observed.result().as_ref());
    }

    #[tokio::test]
    async fn repository_label_path_enforces_distinct_address_cap() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        for count in [MAX_REPOSITORY_LABEL_PATHS, MAX_REPOSITORY_LABEL_PATHS + 1] {
            let extension = format!(
                r#"
def write(ctx):
    paths = [ctx.path(Label("//:missing-%s" % i)) for i in range({count})]
    ctx.file("resolved", str(len(paths)), executable = False)
repo = repository_rule(implementation = write)
def impl(ctx):
    repo(name = "first")
ext = module_extension(implementation = impl)
"#
            );
            let mut transaction = transaction_untracked(&dice, MODULE, &extension, true).await;
            let owner = owner(&mut transaction).await;
            let SourcePreparationOutcome::Complete(result) = transaction
                .compute(&HostSelectedRepositoryFileEffectKey::new(
                    workspace.dupe(),
                    owner,
                    0,
                ))
                .await
                .unwrap()
            else {
                panic!("cap outcome must be terminal")
            };
            if count == MAX_REPOSITORY_LABEL_PATHS {
                assert_eq!(
                    result.as_ref().as_ref().unwrap().plan().effects()[0].content(),
                    MAX_REPOSITORY_LABEL_PATHS.to_string().as_bytes()
                );
            } else {
                assert!(matches!(
                    result.as_ref(),
                    Err(HostSelectedRepositoryFileEffectError::Invocation { message, .. })
                        if message.contains("per-invocation limit")
                ));
            }
        }
    }

    struct TemplateEffectFixture {
        transaction: dice::DiceTransaction,
        first: ObservedHostSelectedRepositoryFileEffect,
        key: HostSelectedRepositoryFileEffectObservationKey,
        owner: Arc<HostSelectedExtensionOwner>,
        workspace: NormalizedAbsolutePath,
        tracker: Arc<EffectTracker>,
        instance: PathObservationInstanceId,
    }

    async fn template_effect_fixture() -> TemplateEffectFixture {
        use crate::canonical_repository_route_tests::tests::WORKSPACE as BUILTIN_WORKSPACE;
        use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
        use crate::canonical_repository_route_tests::tests::builtin_graph_module;
        use crate::canonical_repository_route_tests::tests::transaction as builtin_transaction;

        let dice = builtin_graph_dice();
        let workspace = NormalizedAbsolutePath::new(BUILTIN_WORKSPACE).unwrap();
        let mut module = builtin_graph_module();
        module.push_str("\nrepo=use_repo_rule('@rules_cc//cc/private/toolchain:cc_configure.bzl','cc_configure')\nrepo(name='out')\n");
        let tracker = Arc::new(EffectTracker::default());
        let transaction =
            builtin_transaction(&dice, &module, "", false, Some(tracker.clone())).await;
        let mut transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            Some(tracker.clone()),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::empty(),
        )
        .await;
        let owner = owner_named(&mut transaction, workspace.dupe(), "+cc_configure+out").await;
        tracker.take();
        let key =
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), owner.clone(), 0);
        let logical_root = PathBuf::from("/template-rules-cc");
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/v2_oracle/fixtures/nonroot-module-extension-semantics/workspace/registry/modules/rules_cc/0.2.17",
        );
        let instance = PathObservationInstanceId::new(952);
        let (transaction, first) = converge_observed_template(
            transaction,
            &key,
            &workspace,
            &logical_root,
            &fixture_root,
            instance,
        )
        .await;
        TemplateEffectFixture {
            transaction,
            first,
            key,
            owner,
            workspace,
            tracker,
            instance,
        }
    }

    fn assert_template_result(fixture: &TemplateEffectFixture) {
        let plan = fixture.first.result().as_ref().as_ref().unwrap().plan();
        assert_eq!(plan.effects()[0].content(), b"toolchain=k8\nraw=\xff\n");
        assert!(plan.effects()[0].executable());
        assert_eq!(plan.effects()[1].content(), b"toolchain=arm\nraw=\xff\n");
        assert!(!plan.effects()[1].executable());
        let template_demands = fixture
            .first
            .observations()
            .observations()
            .keys()
            .filter(|demand| demand.path().as_path().ends_with(TEMPLATE_PATH))
            .collect::<Vec<_>>();
        assert_eq!(template_demands.len(), 2);
        assert!(template_demands.iter().all(|demand| demand.namespace()
            == PathObservationNamespace::Materialization(fixture.instance)));
        let prints = fixture
            .tracker
            .take()
            .into_iter()
            .flat_map(|(_, _, batch)| {
                batch.into_iter().flat_map(|batch| {
                    batch
                        .events()
                        .iter()
                        .filter_map(|event| match event {
                            EvaluationEvent::StarlarkPrint { text, .. } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(prints, [CompactString::new("before-template")]);
    }

    #[rustfmt::skip]
    async fn assert_template_reuse_and_a_b_a(mut fixture: TemplateEffectFixture) {
        let SourcePreparationOutcome::Complete(Ok(warm)) =
            fixture.transaction.compute(&fixture.key).await.unwrap()
        else {
            panic!("warm template must complete")
        };
        assert!(Arc::ptr_eq(fixture.first.result(), warm.result()));
        let SourcePreparationOutcome::Complete(legacy) = fixture
            .transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                fixture.workspace.dupe(),
                fixture.owner,
                0,
            ))
            .await
            .unwrap()
        else {
            panic!("legacy template must complete")
        };
        assert_eq!(legacy.as_ref(), fixture.first.result().as_ref());
        let baseline = fixture
            .transaction
            .compute(&PathObservationEpochKey)
            .await
            .unwrap();
        let changed = replace_template_observations(&baseline, b"variant==%{name}\nraw=\xff\n");
        let mut updater = fixture.transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, changed)])
            .unwrap();
        fixture.transaction = updater.commit().await;
        let SourcePreparationOutcome::Complete(Ok(second)) =
            fixture.transaction.compute(&fixture.key).await.unwrap()
        else {
            panic!("changed template must complete")
        };
        assert_eq!(
            second.result().as_ref().as_ref().unwrap().plan().effects()[0].content(),
            b"variant==k8\nraw=\xff\n"
        );
        for failure in [TemplateSourceFailure::Missing, TemplateSourceFailure::Directory, TemplateSourceFailure::Unreadable] {
            let mut updater = fixture.transaction.into_updater();
            updater.changed_to(vec![(PathObservationEpochKey, failing_template_observations(&baseline, failure))]).unwrap();
            fixture.transaction = updater.commit().await;
            let SourcePreparationOutcome::Complete(Ok(failed)) = fixture.transaction.compute(&fixture.key).await.unwrap() else { panic!("terminal template source failure must complete") };
            assert!(failed.result().is_err(), "source failure published an effect");
            assert!(failed.observations().observations().keys().any(|demand| demand.path().as_path().ends_with(TEMPLATE_PATH)));
        }
        let mut updater = fixture.transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, baseline)])
            .unwrap();
        fixture.transaction = updater.commit().await;
        let SourcePreparationOutcome::Complete(Ok(restored)) =
            fixture.transaction.compute(&fixture.key).await.unwrap()
        else {
            panic!("restored template must complete")
        };
        assert_eq!(fixture.first.result(), restored.result());
    }

    #[tokio::test]
    async fn repository_template_routes_bytes_retries_and_restores_a_b_a() {
        let fixture = template_effect_fixture().await;
        assert_template_result(&fixture);
        assert_template_reuse_and_a_b_a(fixture).await;
    }

    #[tokio::test]
    async fn exact_builtin_winsdk_reaches_generic_environment_retry() {
        use crate::canonical_repository_route_tests::tests::WORKSPACE as BUILTIN_WORKSPACE;
        use crate::canonical_repository_route_tests::tests::builtin_graph_dice;
        use crate::canonical_repository_route_tests::tests::builtin_graph_module;
        use crate::canonical_repository_route_tests::tests::transaction as builtin_transaction;

        let dice = builtin_graph_dice();
        let workspace = NormalizedAbsolutePath::new(BUILTIN_WORKSPACE).unwrap();
        let module = builtin_graph_module();
        let tracker = Arc::new(EffectTracker::default());
        let transaction =
            builtin_transaction(&dice, &module, "", false, Some(tracker.clone())).await;
        let mut transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            Some(tracker.clone()),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            RepositoryEnvironmentNameFrontier::empty(),
        )
        .await;
        tracker.take();
        let owner = owner_named(
            &mut transaction,
            workspace.dupe(),
            "bazel_tools+winsdk_configure+local_config_winsdk",
        )
        .await;
        let key = HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), owner, 0);
        let logical_root = PathBuf::from("/effect-rules-cc");
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/v2_oracle/fixtures/nonroot-module-extension-semantics/workspace/registry/modules/rules_cc/0.2.17",
        );
        let instance = PathObservationInstanceId::new(951);
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let mut observations = global
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .collect::<Vec<_>>();
        let mut environment_names = None;
        for _ in 0..16 {
            let outcome = transaction.compute(&key).await.unwrap();
            let SourcePreparationOutcome::Need(need) = outcome else {
                panic!("cold winsdk inputs must retry: {outcome:?}")
            };
            if let Some(environment) = need.repository_environment() {
                environment_names = Some(environment.names().clone());
                break;
            }
            if let Some(request) = need.repository_materializations().values().next() {
                assert_eq!(need.repository_materializations().len(), 1);
                assert_eq!(request.id.canonical_repo.as_str(), "rules_cc+");
                let mut updater = transaction.into_updater();
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
                                    request: request.clone(),
                                    result: RepositoryMaterializationResult::Success(
                                        RepositoryMaterializationSuccess::Immutable {
                                            source_identity: Arc::from("effect-rules-cc-fixture"),
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
                transaction = updater.commit().await;
            } else {
                let paths = need
                    .path_observations()
                    .expect("canonical source path retry");
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
                let mut updater = transaction.into_updater();
                updater
                    .changed_to(vec![(PathObservationEpochKey, epoch)])
                    .unwrap();
                transaction = updater.commit().await;
            }
            transaction = with_host_inputs_for(
                transaction,
                workspace.dupe(),
                Some(tracker.clone()),
                RepositoryPlatform::new("linux", "x86_64"),
                RepositoryEnvironmentSnapshot::empty(),
                RepositoryEnvironmentNameFrontier::empty(),
            )
            .await;
        }
        let names = environment_names.expect("winsdk source inputs must converge");
        assert_eq!(
            names.iter().map(CompactString::as_str).collect::<Vec<_>>(),
            [
                "BAZEL_VC",
                "BAZEL_VC_FULL_VERSION",
                "BAZEL_VS",
                "BAZEL_WINSDK_FULL_VERSION",
                "TEMP",
                "TMP",
                "VS100COMNTOOLS",
                "VS110COMNTOOLS",
                "VS120COMNTOOLS",
                "VS140COMNTOOLS",
                "VS150COMNTOOLS",
                "VS160COMNTOOLS",
                "VS90COMNTOOLS",
            ]
        );
        let mut transaction = with_host_inputs_for(
            transaction,
            workspace,
            Some(tracker.clone()),
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            names,
        )
        .await;
        let SourcePreparationOutcome::Complete(value) = transaction.compute(&key).await.unwrap()
        else {
            panic!("authenticated winsdk must complete after retry")
        };
        let effect = value.as_ref().as_ref().unwrap();
        assert_eq!(effect.plan().effects().len(), 2);
        assert_eq!(effect.plan().effects()[0].path(), "BUILD");
        assert_eq!(effect.plan().effects()[0].content(), b"");
        assert!(!effect.plan().effects()[0].executable());
        assert_eq!(effect.plan().effects()[1].path(), "toolchains.bzl");
        assert_eq!(
            effect.plan().effects()[1].content(),
            b"# Auto-generated by winsdk_configure.bzl\n\ndef register_local_rc_exe_toolchains():\n    pass\n"
        );
        assert!(!effect.plan().effects()[1].executable());
        let activations = tracker.take();
        assert!(
            activations
                .iter()
                .any(|(name, _, _)| { name.starts_with("host-canonical-repository-load-route:") })
        );
        assert!(activations.iter().all(|(name, _, _)| {
            !name.starts_with("observed-host-canonical-repository-load-route:")
                && !name.starts_with("observed-external-bzl-module:")
        }));
    }

    #[tokio::test]
    async fn repository_host_dependencies_retry_invalidate_and_fail_closed() {
        const HOST_EXTENSION: &str = r#"
def write(ctx):
    ctx.getenv("DYNAMIC_ABSENT")
    ctx.getenv("DYNAMIC_ABSENT", "fallback")
    ctx.getenv("DYNAMIC_PRESENT")
    ctx.getenv("FLIP")
    ctx.file("constant", "same", executable = False)
repo = repository_rule(
    implementation = write,
    environ = ["PRESENT", "MISSING", "EMPTY"],
)
def impl(ctx):
    repo(name = "first")
ext = module_extension(implementation = impl)
"#;
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let base = base_transaction_untracked(&dice, MODULE, HOST_EXTENSION, true).await;
        let snapshot_a = environment_snapshot(&[
            ("DYNAMIC_PRESENT", "dynamic"),
            ("EMPTY", ""),
            ("PRESENT", "declared"),
            ("UNRELATED", "one"),
        ]);
        let mut transaction = with_host_inputs_for(
            base,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot_a.clone(),
            RepositoryEnvironmentNameFrontier::empty(),
        )
        .await;
        let selected_owner = owner(&mut transaction).await;
        let key = HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), selected_owner, 0);

        let SourcePreparationOutcome::Need(declared_need) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("declared cold names must retry before invocation")
        };
        assert_eq!(
            declared_need
                .repository_environment()
                .unwrap()
                .names()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["EMPTY", "MISSING", "PRESENT"]
        );
        let declared = environment_frontier(&["EMPTY", "MISSING", "PRESENT"]);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot_a.clone(),
            declared,
        )
        .await;
        let SourcePreparationOutcome::Need(dynamic_need) = transaction.compute(&key).await.unwrap()
        else {
            panic!("staged dynamic reads must retry before publication")
        };
        assert_eq!(
            dynamic_need
                .repository_environment()
                .unwrap()
                .names()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["DYNAMIC_ABSENT", "DYNAMIC_PRESENT", "FLIP"]
        );
        let all = environment_frontier(&[
            "EMPTY",
            "MISSING",
            "PRESENT",
            "DYNAMIC_ABSENT",
            "DYNAMIC_PRESENT",
            "FLIP",
        ]);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot_a.clone(),
            all.clone(),
        )
        .await;
        let SourcePreparationOutcome::Complete(first) = transaction.compute(&key).await.unwrap()
        else {
            panic!("authorized Host inputs must publish")
        };
        let first_effect = first.as_ref().as_ref().unwrap();
        assert_eq!(first_effect.plan().effects()[0].content(), b"same");
        assert_eq!(
            first_effect
                .host()
                .environment()
                .map(|(name, value)| (name, value.map(|value| value.as_ref())))
                .collect::<Vec<_>>(),
            [
                ("DYNAMIC_ABSENT", None),
                ("DYNAMIC_PRESENT", Some("dynamic")),
                ("EMPTY", Some("")),
                ("FLIP", None),
                ("MISSING", None),
                ("PRESENT", Some("declared")),
            ]
        );

        let unrelated = environment_snapshot(&[
            ("DYNAMIC_PRESENT", "dynamic"),
            ("EMPTY", ""),
            ("PRESENT", "declared"),
            ("UNRELATED", "two"),
        ]);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            unrelated,
            all.clone(),
        )
        .await;
        let SourcePreparationOutcome::Complete(reused) = transaction.compute(&key).await.unwrap()
        else {
            panic!("unrelated Host name must remain complete")
        };
        assert!(Arc::ptr_eq(&first, &reused));

        let snapshot_b = environment_snapshot(&[
            ("DYNAMIC_PRESENT", "dynamic"),
            ("EMPTY", ""),
            ("FLIP", "present"),
            ("PRESENT", "declared"),
        ]);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot_b,
            all.clone(),
        )
        .await;
        let SourcePreparationOutcome::Complete(second) = transaction.compute(&key).await.unwrap()
        else {
            panic!("relevant Host change must recompute")
        };
        assert_eq!(
            first_effect.plan(),
            second.as_ref().as_ref().unwrap().plan()
        );
        assert_ne!(first, second);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot_a.clone(),
            all.clone(),
        )
        .await;
        let SourcePreparationOutcome::Complete(third) = transaction.compute(&key).await.unwrap()
        else {
            panic!("A/B/A Host identity must complete")
        };
        assert_eq!(first, third);

        let declared_changed = environment_snapshot(&[
            ("DYNAMIC_PRESENT", "dynamic"),
            ("EMPTY", ""),
            ("PRESENT", "changed"),
        ]);
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("linux", "x86_64"),
            declared_changed,
            all.clone(),
        )
        .await;
        let SourcePreparationOutcome::Complete(declared_effect) =
            transaction.compute(&key).await.unwrap()
        else {
            panic!("declared present change must recompute")
        };
        assert_eq!(
            first_effect.plan(),
            declared_effect.as_ref().as_ref().unwrap().plan()
        );
        assert_ne!(first, declared_effect);

        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![
                (
                    RepositoryEnvironmentCellKey::new(workspace.dupe(), "DYNAMIC_ABSENT"),
                    RepositoryEnvironmentCell::Unauthorized,
                ),
                (
                    RepositoryEnvironmentCellKey::new(workspace.dupe(), "DYNAMIC_PRESENT"),
                    RepositoryEnvironmentCell::Unauthorized,
                ),
            ])
            .unwrap();
        transaction = updater.commit().await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostSelectedRepositoryFileEffectError::HostInput { .. }))
        ));
        transaction = with_host_inputs_for(
            transaction,
            workspace.dupe(),
            None,
            RepositoryPlatform::new("windows", "x86_64"),
            snapshot_a,
            all,
        )
        .await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostSelectedRepositoryFileEffectError::HostInput { .. }))
        ));
    }

    fn observed_carrier(
        value: &<HostSelectedRepositoryFileEffectObservationKey as Key>::Value,
    ) -> &ObservedHostSelectedRepositoryFileEffect {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed repository-file carrier: {value:?}"),
        }
    }

    #[tokio::test]
    async fn selected_repository_file_effect_is_owner_ordinal_scoped_and_observed() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, MODULE, EXTENSION, true).await;
        let owner = owner(&mut transaction).await;
        let legacy_key =
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), owner.clone(), 0);
        let observed_key =
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), owner.clone(), 0);
        let legacy = transaction.compute(&legacy_key).await.unwrap();
        let observed = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_result) = &legacy else {
            panic!("legacy file effect must complete")
        };
        let carrier = observed_carrier(&observed);
        assert_eq!(legacy_result, carrier.result());
        assert!(HostSelectedRepositoryFileEffectKey::equality(
            &legacy, &legacy
        ));
        assert!(HostSelectedRepositoryFileEffectKey::validity(&legacy));
        assert!(HostSelectedRepositoryFileEffectObservationKey::equality(
            &observed, &observed
        ));
        assert!(HostSelectedRepositoryFileEffectObservationKey::validity(
            &observed
        ));
        let effect = legacy_result.as_ref().as_ref().unwrap();
        assert_eq!(effect.plan().effects().len(), 2);
        assert_eq!(effect.plan().effects()[0].path(), "BUILD.bazel");
        assert_eq!(effect.plan().effects()[1].path(), "generated.txt");
        assert!(!effect.plan().effects()[1].executable());

        let certificate = transaction
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                workspace.dupe(),
                owner.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
            panic!("observed certificate must complete")
        };
        let certificate = certificate.dupe();
        let certificate_value = Arc::new(certificate.result().as_ref().as_ref().unwrap().clone());
        let repository = certificate_value.repository(0).unwrap();
        let RepositoryDefinitionLabel::Root(label) =
            definition_label(&certificate_value, 0, repository).unwrap()
        else {
            panic!("fixture definition must be root")
        };
        let child = transaction
            .compute(&HostBzlModuleObservationKey::new(workspace.dupe(), label))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("observed HostBzl must complete")
        };
        assert_eq!(
            carrier.observations(),
            &merge_observations(certificate.observations(), child.observations()).unwrap()
        );
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        for (demand, result) in carrier.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }

        let mismatch_source = EXTENSION.replace(
            "_repo=repository_rule(implementation=write)",
            "_repo=repository_rule(implementation=write, environ=['MISMATCH'])",
        );
        let mut mismatch = transaction_untracked(&dice, MODULE, &mismatch_source, true).await;
        let RepositoryDefinitionLabel::Root(label) =
            definition_label(&certificate_value, 0, repository).unwrap()
        else {
            unreachable!()
        };
        let mismatch_module = mismatch
            .compute(&HostBzlModuleObservationKey::new(workspace.dupe(), label))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(mismatch_module)) = mismatch_module else {
            panic!("mismatched module must load")
        };
        assert!(matches!(
            authenticate_rule(
                &certificate_value,
                0,
                repository,
                mismatch_module.result().as_ref().unwrap()
            ),
            Err(HostSelectedRepositoryFileEffectError::Projection { .. })
        ));

        let missing = transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace, owner, 1,
            ))
            .await
            .unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostSelectedRepositoryFileEffectError::MissingOrdinal { ordinal: 1, .. }))
        ));
    }

    #[tokio::test]
    async fn selected_repository_file_effect_owns_cold_warm_events_and_recovers_after_cancel() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(EffectTracker::default());
        let mut transaction =
            transaction_with_tracker(&dice, MODULE, EXTENSION, true, tracker.clone()).await;
        let selected_owner = owner(&mut transaction).await;
        tracker.take();
        let key = HostSelectedRepositoryFileEffectObservationKey::new(
            workspace.dupe(),
            selected_owner,
            0,
        );
        let first = observed_carrier(&transaction.compute(&key).await.unwrap()).dupe();
        let cold = tracker.take();
        let rows = cold
            .iter()
            .filter(|(name, _, _)| name == &key.to_string())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, ActivationKind::Evaluated);
        assert_eq!(
            rows[0]
                .2
                .as_ref()
                .unwrap()
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["repository-file-effect"]
        );
        let second = observed_carrier(&transaction.compute(&key).await.unwrap()).dupe();
        assert!(Arc::ptr_eq(first.result(), second.result()));
        assert!(tracker.take().iter().any(|(name, kind, batch)| {
            name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()
        }));

        let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancelled_tracker = Arc::new(EffectTracker::default());
        let mut cancelled = transaction_with_tracker(
            &cancelled_dice,
            MODULE,
            EXTENSION,
            true,
            cancelled_tracker.clone(),
        )
        .await;
        let cancelled_owner = owner(&mut cancelled).await;
        cancelled_tracker.take();
        let cancelled_key = HostSelectedRepositoryFileEffectObservationKey::new(
            workspace.dupe(),
            cancelled_owner,
            0,
        );
        let mut future = Box::pin(cancelled.compute(&cancelled_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            cancelled_tracker
                .take()
                .iter()
                .all(|(name, _, _)| name != &cancelled_key.to_string())
        );
        let mut recovery = transaction_untracked(&cancelled_dice, MODULE, EXTENSION, true).await;
        let recovered_owner = owner(&mut recovery).await;
        let recovered = observed_carrier(
            &recovery
                .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                    workspace,
                    recovered_owner,
                    0,
                ))
                .await
                .unwrap(),
        )
        .dupe();
        assert_eq!(recovered.result(), first.result());
    }

    #[tokio::test]
    async fn selected_repository_file_effect_reloads_implementation_and_is_complete_only() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let replacement = EXTENSION.replace("from-rule", "from-reloaded-rule");
        let mut values = Vec::new();
        for extension in [EXTENSION, replacement.as_str(), EXTENSION] {
            let mut transaction = transaction_untracked(&dice, MODULE, extension, true).await;
            let selected_owner = owner(&mut transaction).await;
            let value = transaction
                .compute(&HostSelectedRepositoryFileEffectKey::new(
                    workspace.dupe(),
                    selected_owner,
                    0,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(value) = value else {
                panic!("reloaded file effect must complete")
            };
            values.push(value);
        }
        assert_eq!(
            values[0].as_ref().as_ref().unwrap().plan().effects()[1].content(),
            b"from-rule"
        );
        assert_eq!(
            values[1].as_ref().as_ref().unwrap().plan().effects()[1].content(),
            b"from-reloaded-rule"
        );
        assert_eq!(values[0], values[2]);
        assert_ne!(values[0], values[1]);

        let need: <HostSelectedRepositoryFileEffectKey as Key>::Value =
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
                NeedPathObservations::singleton(PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/repository-file-effect-need").unwrap(),
                    PathObservationOperation::Lstat,
                )),
            ));
        assert!(!HostSelectedRepositoryFileEffectKey::validity(&need));
        assert!(!HostSelectedRepositoryFileEffectObservationKey::validity(
            &SourcePreparationOutcome::Need(match need {
                SourcePreparationOutcome::Need(need) => need,
                SourcePreparationOutcome::Complete(_) => unreachable!(),
            }),
        ));
    }

    #[tokio::test]
    async fn repository_context_attributes_restore_warm_effects_for_ordinary_and_innate_owners() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let definition = r#"
def write(ctx):
    ctx.file('generated.txt', repr([ctx.name, ctx.original_name, ctx.attr.s, ctx.attr.words, ctx.attr.groups, ctx.attr.default]))
repo=repository_rule(implementation=write, attrs={'s':attr.string(), 'words':attr.string_list(), 'groups':attr.string_list_dict(), 'default':attr.string(default='d')})
"#;
        let ordinary = |s: &str, words: &str, groups: &str| {
            format!(
                "{definition}def impl(ctx):\n    repo(name='first', s='{s}', words={words}, groups={groups})\next=module_extension(implementation=impl)\n"
            )
        };
        let innate_module = |s: &str, words: &str, groups: &str| {
            format!(
                "module(name='bazel_tools')\nrepo=use_repo_rule('//:ext.bzl','repo')\nrepo(name='first', s='{s}', words={words}, groups={groups})\n"
            )
        };
        for (module, requested) in [
            (MODULE.to_owned(), "+ext+first"),
            (
                innate_module("one", "['a']", "{'z':['a'], 'a':['b']}"),
                "+repo+first",
            ),
        ] {
            let extensions = if requested == "+ext+first" {
                [
                    ordinary("one", "['a']", "{'z':['a'], 'a':['b']}"),
                    ordinary("two", "['b']", "{'a':['b'], 'z':['a']}"),
                    ordinary("one", "['a']", "{'z':['a'], 'a':['b']}"),
                ]
            } else {
                [
                    innate_module("one", "['a']", "{'z':['a'], 'a':['b']}"),
                    innate_module("two", "['b']", "{'a':['b'], 'z':['a']}"),
                    innate_module("one", "['a']", "{'z':['a'], 'a':['b']}"),
                ]
            };
            let mut values = Vec::new();
            for extension in extensions.iter() {
                let (module_source, extension_source) = if requested == "+ext+first" {
                    (module.as_str(), extension.as_str())
                } else {
                    (extension.as_str(), definition)
                };
                let mut tx =
                    transaction_untracked(&dice, module_source, extension_source, true).await;
                let owner = owner_named(&mut tx, workspace.dupe(), requested).await;
                let SourcePreparationOutcome::Complete(value) = tx
                    .compute(&HostSelectedRepositoryFileEffectKey::new(
                        workspace.dupe(),
                        owner,
                        0,
                    ))
                    .await
                    .unwrap()
                else {
                    panic!("effect must complete")
                };
                values.push(value);
            }
            let first = std::str::from_utf8(
                values[0].as_ref().as_ref().unwrap().plan().effects()[0].content(),
            )
            .unwrap();
            let changed = std::str::from_utf8(
                values[1].as_ref().as_ref().unwrap().plan().effects()[0].content(),
            )
            .unwrap();
            assert!(
                first.contains(r#", "one", ["a"],"#) && changed.contains(r#", "two", ["b"],"#),
                "{requested}: scalar and list values must both change"
            );
            assert!(
                first.contains(r#"{"z": ["a"], "a": ["b"]}"#)
                    && changed.contains(r#"{"a": ["b"], "z": ["a"]}"#),
                "{requested}: nested map order must change without sorting"
            );
            assert_ne!(
                first, changed,
                "{requested}: scalar, collection, and nested-order transition"
            );
            assert_eq!(
                values[0], values[2],
                "{requested}: A/B/A restores effect identity"
            );
        }
    }

    #[tokio::test]
    async fn repository_context_defaults_names_and_failures_cover_both_owner_kinds() {
        fn definition(default: &str, fail_after_file: bool) -> String {
            let body = if fail_after_file {
                "    ctx.file('staged')\n    return ctx.attr.missing\n"
            } else {
                "    ctx.file('generated.txt', repr([ctx.name, ctx.original_name, ctx.attr.value]))\n"
            };
            let descriptor = if default == "B" {
                "attr.string_list(default=['B'])".to_owned()
            } else {
                format!("attr.string(default='{default}')")
            };
            format!(
                "def write(ctx):\n{body}repo=repository_rule(implementation=write, attrs={{'value':{descriptor}}})\n"
            )
        }

        fn ordinary(definition: &str, name: &str) -> String {
            format!(
                "{definition}def impl(ctx):\n    repo(name='{name}')\next=module_extension(implementation=impl)\n"
            )
        }

        fn innate(name: &str) -> String {
            format!(
                "module(name='bazel_tools')\nrepo=use_repo_rule('//:ext.bzl','repo')\nrepo(name='{name}')\n"
            )
        }

        fn ordinary_module(name: &str) -> String {
            format!(
                "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, generated='{name}')\n"
            )
        }

        async fn effect(
            dice: &Arc<Dice>,
            workspace: &NormalizedAbsolutePath,
            module: &str,
            extension: &str,
            requested: &str,
        ) -> EffectResult {
            let mut transaction = transaction_untracked(dice, module, extension, true).await;
            let owner = owner_named(&mut transaction, workspace.dupe(), requested).await;
            let SourcePreparationOutcome::Complete(value) = transaction
                .compute(&HostSelectedRepositoryFileEffectKey::new(
                    workspace.dupe(),
                    owner,
                    0,
                ))
                .await
                .unwrap()
            else {
                panic!("repository context effect must complete")
            };
            value
        }

        fn content(value: &EffectResult) -> &[u8] {
            value.as_ref().as_ref().unwrap().plan().effects()[0].content()
        }

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        for innate_owner in [false, true] {
            let mut defaults = Vec::new();
            for default in ["A", "B", "A"] {
                let definition = definition(default, false);
                let (module, extension, requested) = if innate_owner {
                    (innate("first"), definition, "+repo+first")
                } else {
                    (
                        MODULE.to_owned(),
                        ordinary(&definition, "first"),
                        "+ext+first",
                    )
                };
                defaults.push(effect(&dice, &workspace, &module, &extension, requested).await);
            }
            let canonical = if innate_owner {
                "+repo+first"
            } else {
                "+ext+first"
            };
            assert_eq!(
                content(&defaults[0]),
                format!(r#"["{canonical}", "first", "A"]"#).as_bytes()
            );
            assert_ne!(content(&defaults[0]), content(&defaults[1]));
            assert!(
                std::str::from_utf8(content(&defaults[1]))
                    .unwrap()
                    .contains(r#"["B"]"#)
            );
            assert_eq!(
                defaults[0], defaults[2],
                "declaration kind/default A/B/A must restore"
            );

            let mut names = Vec::new();
            for name in ["first", "second", "first"] {
                let definition = definition("A", false);
                let requested = if innate_owner {
                    format!("+repo+{name}")
                } else {
                    format!("+ext+{name}")
                };
                let (module, extension) = if innate_owner {
                    (innate(name), definition)
                } else {
                    (ordinary_module(name), ordinary(&definition, name))
                };
                names.push(effect(&dice, &workspace, &module, &extension, &requested).await);
            }
            let second_canonical = if innate_owner {
                "+repo+second"
            } else {
                "+ext+second"
            };
            assert_eq!(
                content(&names[1]),
                format!(r#"["{second_canonical}", "second", "A"]"#).as_bytes(),
                "canonical and generated/original names must both change"
            );
            assert_eq!(names[0], names[2], "name A/B/A must restore");

            let failure_definition = definition("A", true);
            let (module, extension, requested) = if innate_owner {
                (innate("first"), failure_definition, "+repo+first")
            } else {
                (
                    MODULE.to_owned(),
                    ordinary(&failure_definition, "first"),
                    "+ext+first",
                )
            };
            let mut transaction = transaction_untracked(&dice, &module, &extension, true).await;
            let owner = owner_named(&mut transaction, workspace.dupe(), requested).await;
            let SourcePreparationOutcome::Complete(certificate) = transaction
                .compute(&HostSelectedExtensionOwnerCertificateKey::new(
                    workspace.dupe(),
                    owner.clone(),
                ))
                .await
                .unwrap()
            else {
                panic!("owner certificate must complete")
            };
            let repository = certificate
                .as_ref()
                .as_ref()
                .unwrap()
                .repository(0)
                .unwrap();
            let (canonical, spec) = repository.spec_parts();
            let mut malformed = spec.attributes.as_ref().clone();
            malformed.insert("value".into(), OverrideAttributeValue::Int(1));
            assert!(
                RepositoryRuleInvocationInput::new(
                    canonical.as_str().into(),
                    Some(repository.generated_name().into()),
                    Arc::new(malformed),
                    repository.call().definition.attributes.clone(),
                )
                .is_err()
            );
            let SourcePreparationOutcome::Complete(failed) = transaction
                .compute(&HostSelectedRepositoryFileEffectKey::new(
                    workspace.dupe(),
                    owner,
                    0,
                ))
                .await
                .unwrap()
            else {
                panic!("failing effect must complete")
            };
            assert!(matches!(
                failed.as_ref(),
                Err(HostSelectedRepositoryFileEffectError::Invocation { .. })
            ));
        }
    }

    #[tokio::test]
    async fn selected_repository_file_effect_forwards_real_observed_need_without_batch() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(EffectTracker::default());
        let mut transaction =
            transaction_with_tracker(&dice, MODULE, EXTENSION, true, tracker.clone()).await;
        let selected_owner = owner(&mut transaction).await;
        let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let ext = NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap();
        let epoch = PathObservationEpoch::new(
            epoch
                .observations()
                .iter()
                .filter(|(demand, _)| demand.path() != &ext)
                .map(|(demand, result)| (demand.dupe(), result.as_ref().clone())),
        )
        .unwrap();
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        tracker.take();
        let legacy =
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), selected_owner.clone(), 0);
        let key = HostSelectedRepositoryFileEffectObservationKey::new(workspace, selected_owner, 0);
        assert!(matches!(
            transaction.compute(&legacy).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(
            tracker
                .take()
                .iter()
                .filter(|(name, _, _)| name == &legacy.to_string() || name == &key.to_string())
                .all(|(_, _, batch)| batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_executes_selected_ordinal_not_failing_sibling() {
        let extension = r#"
def selected(ctx):
    ctx.file('selected', 'ok')
def sibling(ctx):
    fail('sibling repository rule must not execute')
first=repository_rule(implementation=selected)
second=repository_rule(implementation=sibling)
def impl(ctx):
    first(name='first')
    second(name='second')
ext=module_extension(implementation=impl)
"#;
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, MODULE, extension, true).await;
        let selected_owner = owner(&mut transaction).await;
        let value = transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace,
                selected_owner,
                0,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("selected ordinal must complete")
        };
        assert_eq!(
            value.as_ref().as_ref().unwrap().plan().effects()[0].path(),
            "selected"
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_preserves_semantic_failures_in_observed_carrier() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let semantic_extension = EXTENSION.replace(
            "ctx.file('BUILD.bazel', content='exports_files([\\\"generated.txt\\\"])\\\\n')",
            "fail('repository-rule semantic failure')",
        );
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut semantic = transaction_untracked(&dice, MODULE, &semantic_extension, true).await;
        let semantic_owner = owner(&mut semantic).await;
        let legacy = semantic
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace.dupe(),
                semantic_owner.clone(),
                0,
            ))
            .await
            .unwrap();
        let observed = semantic
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                workspace.dupe(),
                semantic_owner,
                0,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("semantic legacy failure must complete")
        };
        let observed = observed_carrier(&observed);
        assert_eq!(legacy, *observed.result());
        assert!(matches!(
            legacy.as_ref(),
            Err(HostSelectedRepositoryFileEffectError::Invocation { .. })
        ));

        let tracker = Arc::new(EffectTracker::default());
        let event_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut observed_transaction = transaction_with_tracker(
            &event_dice,
            MODULE,
            &semantic_extension,
            true,
            tracker.clone(),
        )
        .await;
        let observed_owner = owner(&mut observed_transaction).await;
        tracker.take();
        let key = HostSelectedRepositoryFileEffectObservationKey::new(workspace, observed_owner, 0);
        let _ = observed_transaction.compute(&key).await.unwrap();
        let batches = tracker
            .take()
            .into_iter()
            .filter(|(name, _, _)| name == &key.to_string())
            .filter_map(|(_, _, batch)| batch)
            .collect::<Vec<_>>();
        assert_eq!(batches.len(), 1);
        assert_eq!(
            batches[0]
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["repository-file-effect"]
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_keys_distinguish_owner_and_ordinal() {
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nf=use_extension('//:ext.bzl','other')\nuse_repo(e, first='first')\nuse_repo(f, second='first')\n";
        let extension = format!("{EXTENSION}\nother=module_extension(implementation=impl)\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, module, &extension, true).await;
        let first = owner(&mut transaction).await;
        let demand = transaction
            .compute(&HostSelectedExtensionDemandKey::new(
                workspace.dupe(),
                CanonicalRepoName::new("+other+first").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(demand) = demand else {
            panic!("second selected demand must complete")
        };
        let second = demand.as_ref().as_ref().unwrap().owner().clone();
        assert_ne!(
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), first.clone(), 0),
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), first.clone(), 1)
        );
        assert_ne!(
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), second, 0),
            HostSelectedRepositoryFileEffectObservationKey::new(workspace, first, 0)
        );
    }

    #[test]
    fn selected_repository_file_effect_has_exact_authentication_and_retained_shape() {
        let source = include_str!("module_extension_repository_file_effect.rs");
        let production = &source[..source.find("\n#[cfg(test)]").unwrap()];
        for shape in [
            "get_any_visibility(&call.definition.exported_name)",
            "downcast::<FrozenRepositoryRuleDefinition>()",
            "if projection != call.definition",
            "Ok(rule.implementation())",
            "SourcePreparationOutcome::Complete(Err(error))",
            "RepositoryEnvironmentCellKey::new(",
            "RepositoryPlatformKey::new(",
            "invoke_repository_rule(",
            "HostCanonicalRepositoryLoadRouteKey::new(",
            "HostRepositoryLabelPathObservationKey::new_",
            "PreparedRepositoryLabelPaths::new()",
            "MAX_REPOSITORY_LABEL_PATHS",
            "ExternalBzlModuleEvalKey::new_canonical_bzlmod(",
            "HostSelectedRepositoryFileEffectObservationError::CanonicalRoute {",
            "HostSelectedRepositoryFileEffectObservationError::Certificate(error)",
            "HostSelectedRepositoryFileEffectObservationError::HostBzl {",
            "HostSelectedRepositoryFileEffectObservationError::Merge {",
        ] {
            assert!(
                production.contains(shape),
                "missing producer shape: {shape}"
            );
        }
        let context = include_str!("repository_rule_context.rs");
        for shape in [
            "let Some(path) = path.unpack_str()",
            "#[starlark_value(type = \"repository_ctx\")]",
            "#[starlark_value(type = \"repository_os\")]",
            "fn getenv<'v>(",
            "fn path<'v>(",
            "RepositoryStarlarkPath",
            "AllocDict(self.snapshot.iter()",
        ] {
            assert!(context.contains(shape), "missing context shape: {shape}");
        }
        let start = production
            .find("pub struct HostSelectedRepositoryFileEffect {")
            .unwrap();
        let end = production
            .find("\n#[derive(Clone, Copy)]\nenum EffectMode")
            .unwrap();
        let retained = &production[start..end];
        for absent in [
            "FrozenModule",
            "FrozenValue",
            "Heap",
            "Evaluator",
            "RepositoryFileContext",
            "std::fs",
            "I/O",
        ] {
            assert!(
                !retained.contains(absent),
                "retained shape contains {absent}"
            );
        }
    }
}
