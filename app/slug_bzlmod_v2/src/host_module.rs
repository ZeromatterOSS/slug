/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the Host root-module activation packet.

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::BuiltinBazelToolsSnapshot;
use crate::EvaluatedRootModule;
use crate::GeneratedRepositoryFileEffectPlan;
use crate::HostCanonicalSelectedModuleDefinition;
use crate::HostCanonicalSelectedModuleDefinitionError;
use crate::HostCanonicalSelectedModuleDefinitionErrorDisposition;
use crate::HostCanonicalSelectedModuleDefinitionKey;
use crate::HostCanonicalSelectedModuleDefinitionObservationError;
use crate::HostCanonicalSelectedModuleDefinitionObservationKey;
use crate::HostCanonicalSelectedModuleIdentity;
use crate::HostCanonicalSelectedModuleKind;
use crate::HostRootRepositoryMappingError;
use crate::HostRootRepositoryMappingKey;
use crate::HostRootRepositoryMappingObservationError;
use crate::HostRootRepositoryMappingObservationKey;
use crate::HostSelectedExtensionDefinitionSource;
use crate::HostSelectedObservationFrontier;
use crate::LogicalModuleFileId;
use crate::LogicalSpan;
use crate::NonrootIncludeRequest;
use crate::OverrideAttributeKey;
use crate::OverrideAttributeValue;
use crate::RepoSpec;
use crate::RootModuleBootstrapRequest;
use crate::RootModuleOverride;
use crate::RootModuleOverrides;
use crate::RootModuleRegistrations;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationOutcome;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileBytesObservationKey;
use crate::host_file::HostFileError;
use crate::host_include::HostRootIncludeError;
use crate::host_include::HostRootIncludePackageFailure as Failure;
use crate::host_include::preflight_root_include_horizon;
use crate::host_include::preflight_root_include_horizon_observed;
use crate::host_package::HostRootPackageLookupError;
use crate::module_eval::RootExtensionUsage;
use crate::module_eval::RootModuleSourceFile;
use crate::module_eval::evaluate_root_module_closure_with_events;
use crate::module_eval::host_file_semantic_error;
use crate::module_eval::root_module_ignore_dev_dependency;
use crate::module_eval::validate_root_module_source;
use crate::selected_repo_spec::HostSelectedBzlLoadSource;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRootModuleFileValue {
    pub(crate) module: EvaluatedRootModule,
    pub(crate) overrides: RootModuleOverrides,
    pub(crate) module_file_paths: Arc<[PathBuf]>,
    pub(crate) extension_usages: Arc<[RootExtensionUsage]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootModuleFileError {
    CommandPolicy {
        message: CompactString,
    },
    RootFile {
        error: HostFileError,
    },
    RootValidation {
        logical_id: LogicalModuleFileId,
        message: CompactString,
    },
    IncludePreflight {
        error: HostRootIncludeError,
    },
    IncludeMissing {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
    },
    IncludeFile {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
        error: HostFileError,
    },
    IncludeValidation {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    IncludeCycle {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
    },
    Evaluation {
        message: CompactString,
        include_occurrences: Arc<[NonrootIncludeRequest]>,
    },
}

impl fmt::Display for HostRootModuleFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostRootModuleFileError {}

pub(crate) fn package_error(raw_label: &str, failure: &Failure) -> CompactString {
    CompactString::new(match failure {
        Failure::NoBuildFile => {
            format!("root MODULE include package has no BUILD file: {raw_label}")
        }
        Failure::Deleted => format!("root MODULE include package is deleted: {raw_label}"),
        Failure::InvalidPackageName { message } => {
            format!("root MODULE include package name is invalid: {raw_label}: {message}")
        }
        Failure::Operational(error) => match error {
            HostRootPackageLookupError::PolicyInput(error) => {
                format!("root MODULE include package policy failed: {raw_label}: {error}")
            }
            HostRootPackageLookupError::RepositoryIgnore(_) => {
                format!("root MODULE include repository-ignore failed: {raw_label}")
            }
            HostRootPackageLookupError::Resolution {
                logical_path,
                error,
            } => {
                let class = match error {
                    PathResolutionError::Observation { .. } => "observation failed",
                    PathResolutionError::InconsistentState { .. } => "changed during resolution",
                    PathResolutionError::Cycle { .. } => "has a symlink cycle",
                    PathResolutionError::InfiniteExpansion { .. } => {
                        "has infinite symlink expansion"
                    }
                };
                format!(
                    "root MODULE include package marker {} {class}: {raw_label}",
                    logical_path.as_path().display()
                )
            }
        },
    })
}

impl HostRootModuleFileError {
    pub(crate) fn semantic_message(&self) -> CompactString {
        match self {
            Self::CommandPolicy { message }
            | Self::RootValidation { message, .. }
            | Self::IncludeValidation { message, .. }
            | Self::Evaluation { message, .. } => message.clone(),
            Self::RootFile { error } | Self::IncludeFile { error, .. } => {
                host_file_semantic_error(error)
            }
            Self::IncludePreflight { error } => match error {
                HostRootIncludeError::BadLabel {
                    raw_label, message, ..
                } => CompactString::new(format!(
                    "invalid root MODULE include {raw_label}: {message}"
                )),
                HostRootIncludeError::Package {
                    raw_label, failure, ..
                } => package_error(raw_label, failure),
            },
            Self::IncludeMissing { logical_path, .. } => CompactString::new(format!(
                "workspace file is absent: {}",
                logical_path.as_path().display()
            )),
            Self::IncludeCycle { logical_path, .. } => CompactString::new(format!(
                "root MODULE include cycle at {}",
                logical_path.as_path().display()
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRootModuleFileKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRootModuleFileKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRootModuleFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-root-module-file:{}", self.workspace)
    }
}

type HostRootModuleFileCarrier = Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>>;
type HostRootModuleFileOutcome = SourcePreparationOutcome<HostRootModuleFileCarrier>;

fn path_need(need: NeedPathObservations) -> HostRootModuleFileOutcome {
    SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
}

fn terminal_error(error: HostRootModuleFileError) -> HostRootModuleFileOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

fn evaluate_root_module_terminal(
    ignore_dev_dependency: bool,
    files: Vec<RootModuleSourceFile>,
    include_indices: SmallMap<CompactString, usize>,
    module_file_paths: Arc<[PathBuf]>,
    evaluation_occurrences: Vec<NonrootIncludeRequest>,
    capture_events: bool,
) -> (HostRootModuleFileCarrier, Option<EventBatch>) {
    let (evaluation, captured) = evaluate_root_module_closure_with_events(
        ignore_dev_dependency,
        files,
        include_indices,
        module_file_paths,
        capture_events,
    );
    let result = evaluation
        .map(|evaluation| HostRootModuleFileValue {
            module: evaluation.module,
            overrides: evaluation.overrides,
            module_file_paths: evaluation.module_file_paths,
            extension_usages: evaluation.extension_usages,
        })
        .map_err(|message| HostRootModuleFileError::Evaluation {
            message,
            include_occurrences: evaluation_occurrences.into(),
        });
    (Arc::new(result), captured)
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host root-module DICE invariant failed: {error:?}"))
}

fn root_logical_id(path: &NormalizedAbsolutePath) -> LogicalModuleFileId {
    LogicalModuleFileId::new(path.as_path().display().to_string())
}

fn include_relative_path(
    package: &slug_identity_v2::PackagePath,
    target: &slug_identity_v2::TargetName,
) -> PathBuf {
    PathBuf::from(package.as_str()).join(target.as_str())
}

struct RootModuleIncludeAncestry {
    logical_path: NormalizedAbsolutePath,
    parent: Option<Arc<Self>>,
}

impl RootModuleIncludeAncestry {
    fn contains(&self, logical_path: &NormalizedAbsolutePath) -> bool {
        let mut current = Some(self);
        while let Some(ancestry) = current {
            if ancestry.logical_path == *logical_path {
                return true;
            }
            current = ancestry.parent.as_deref();
        }
        false
    }
}

struct PendingRootModuleInclude {
    request: NonrootIncludeRequest,
    ancestry: Arc<RootModuleIncludeAncestry>,
}

#[async_trait]
impl Key for HostRootModuleFileKey {
    type Value = HostRootModuleFileOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let observed = drive_root_module(
            ctx,
            &self.workspace,
            RootModuleFrontierMode::Legacy,
            capture_events,
            &mut event_batch,
        )
        .await;
        let outcome = match observed {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                SourcePreparationOutcome::Complete(observed.result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy Host root-module driver produced frontier error: {error}")
            }
        };
        if capture_events && outcome.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("Host root-module key stores exactly one event batch");
        }
        outcome
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRootModuleFile {
    result: HostRootModuleFileCarrier,
    observations: PathObservationEpoch,
}

impl ObservedHostRootModuleFile {
    pub(crate) fn result(&self) -> &Result<HostRootModuleFileValue, HostRootModuleFileError> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRootModuleFileObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRootModuleFileObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRootModuleFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bzlmod-observed-host-root-module-file:{}",
            self.workspace
        )
    }
}

type ObservedHostRootModuleFileOutcome =
    SourcePreparationOutcome<Result<ObservedHostRootModuleFile, ObservedPathFrontierError>>;
type RootModuleHostFileProjection = (Result<HostFileBytes, HostFileError>, PathObservationEpoch);
type RootModulePreflightProjection = (
    Arc<Result<crate::host_include::HostRootIncludeHorizon, HostRootIncludeError>>,
    PathObservationEpoch,
);

#[derive(Clone, Copy)]
enum RootModuleFrontierMode {
    Legacy,
    Observed,
}

fn observed_path_need(need: NeedPathObservations) -> ObservedHostRootModuleFileOutcome {
    SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
}

fn observed_complete(
    result: HostRootModuleFileCarrier,
    observations: PathObservationEpoch,
) -> ObservedHostRootModuleFileOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedHostRootModuleFile {
        result,
        observations,
    }))
}

fn observed_error(
    error: HostRootModuleFileError,
    observations: PathObservationEpoch,
) -> ObservedHostRootModuleFileOutcome {
    observed_complete(Arc::new(Err(error)), observations)
}

fn observed_outer(error: ObservedPathFrontierError) -> ObservedHostRootModuleFileOutcome {
    SourcePreparationOutcome::Complete(Err(error))
}

fn union_observations(
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

fn merge_observations(
    mode: RootModuleFrontierMode,
    left: PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    match mode {
        RootModuleFrontierMode::Legacy => Ok(left),
        RootModuleFrontierMode::Observed => union_observations(&left, right),
    }
}

async fn compute_root_module_host_file(
    ctx: &mut DiceComputations<'_>,
    logical_path: NormalizedAbsolutePath,
    mode: RootModuleFrontierMode,
) -> PathOutcome<Result<RootModuleHostFileProjection, ObservedPathFrontierError>> {
    match mode {
        RootModuleFrontierMode::Legacy => {
            match dice_invariant(ctx.compute(&HostFileBytesKey::new(logical_path)).await) {
                PathOutcome::Need(need) => PathOutcome::Need(need),
                PathOutcome::Complete(result) => {
                    PathOutcome::Complete(Ok((result, PathObservationEpoch::empty())))
                }
            }
        }
        RootModuleFrontierMode::Observed => match dice_invariant(
            ctx.compute(&HostFileBytesObservationKey::new(logical_path))
                .await,
        ) {
            PathOutcome::Need(need) => PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => PathOutcome::Complete(Err(error)),
            PathOutcome::Complete(Ok(file)) => {
                PathOutcome::Complete(Ok((file.result().clone(), file.observations().dupe())))
            }
        },
    }
}

async fn compute_root_module_preflight(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requests: &[NonrootIncludeRequest],
    mode: RootModuleFrontierMode,
) -> PathOutcome<Result<RootModulePreflightProjection, ObservedPathFrontierError>> {
    match mode {
        RootModuleFrontierMode::Legacy => {
            match preflight_root_include_horizon(ctx, workspace, requests).await {
                PathOutcome::Need(need) => PathOutcome::Need(need),
                PathOutcome::Complete(value) => {
                    PathOutcome::Complete(Ok((value, PathObservationEpoch::empty())))
                }
            }
        }
        RootModuleFrontierMode::Observed => {
            preflight_root_include_horizon_observed(ctx, workspace, requests).await
        }
    }
}

async fn drive_root_module(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    mode: RootModuleFrontierMode,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> ObservedHostRootModuleFileOutcome {
    let ignore_dev_dependency =
        match root_module_ignore_dev_dependency(ctx, workspace.as_path()).await {
            Ok(value) => value,
            Err(message) => {
                return observed_error(
                    HostRootModuleFileError::CommandPolicy { message },
                    PathObservationEpoch::empty(),
                );
            }
        };
    let root_path = NormalizedAbsolutePath::new(workspace.as_path().join("MODULE.bazel"))
        .expect("joining the root MODULE basename remains normalized absolute");
    let (root, mut observations) =
        match compute_root_module_host_file(ctx, root_path.dupe(), mode).await {
            PathOutcome::Need(need) => return observed_path_need(need),
            PathOutcome::Complete(Err(error)) => return observed_outer(error),
            PathOutcome::Complete(Ok(root)) => root,
        };
    let root_bytes = match root {
        Err(error) => {
            return observed_error(HostRootModuleFileError::RootFile { error }, observations);
        }
        Ok(HostFileBytes::Missing) => {
            return SourcePreparationOutcome::Need(SourcePreparationNeeds::root_module_bootstrap(
                RootModuleBootstrapRequest {
                    workspace: workspace.dupe(),
                },
            ));
        }
        Ok(HostFileBytes::Present(bytes)) => bytes,
    };
    let root_id = root_logical_id(&root_path);
    let root_inspection = match validate_root_module_source(root_id.clone(), root_bytes.as_ref()) {
        Ok(inspection) => inspection,
        Err(message) => {
            return observed_error(
                HostRootModuleFileError::RootValidation {
                    logical_id: root_id,
                    message,
                },
                observations,
            );
        }
    };
    let root_source = Arc::new(
        std::str::from_utf8(root_bytes.as_ref())
            .expect("successful validation established UTF-8")
            .to_owned(),
    );
    let root_ancestry = Arc::new(RootModuleIncludeAncestry {
        logical_path: root_path.dupe(),
        parent: None,
    });
    let mut horizon = root_inspection
        .includes
        .iter()
        .cloned()
        .map(|request| PendingRootModuleInclude {
            request,
            ancestry: root_ancestry.dupe(),
        })
        .collect::<Vec<_>>();
    let mut files = vec![RootModuleSourceFile {
        path: root_path.as_path().to_path_buf(),
        source: root_source,
        _inspection: root_inspection,
    }];
    let mut include_indices = SmallMap::new();
    let mut module_file_paths = vec![PathBuf::from("MODULE.bazel")];
    let mut evaluation_occurrences = Vec::new();

    while !horizon.is_empty() {
        let requests = horizon
            .iter()
            .map(|pending| pending.request.clone())
            .collect::<Vec<_>>();
        let (preflight, preflight_observations) =
            match compute_root_module_preflight(ctx, workspace, &requests, mode).await {
                PathOutcome::Need(need) => return observed_path_need(need),
                PathOutcome::Complete(Err(error)) => return observed_outer(error),
                PathOutcome::Complete(Ok(value)) => value,
            };
        observations = match merge_observations(mode, observations, &preflight_observations) {
            Ok(observations) => observations,
            Err(error) => return observed_outer(error),
        };
        let preflight = match preflight.as_ref() {
            Ok(preflight) => preflight,
            Err(error) => {
                return observed_error(
                    HostRootModuleFileError::IncludePreflight {
                        error: error.clone(),
                    },
                    observations,
                );
            }
        };

        let mut unique_paths = SmallSet::with_capacity(preflight.includes().len());
        for include in preflight.includes() {
            unique_paths.insert(include.logical_path().dupe());
        }
        let computed = ctx
            .compute_join(unique_paths, |ctx, logical_path| {
                Box::pin(async move {
                    let outcome =
                        compute_root_module_host_file(ctx, logical_path.dupe(), mode).await;
                    (logical_path, outcome)
                })
            })
            .await;
        let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
        let all_need: Option<NeedPathObservations> =
            outcomes
                .values()
                .fold(None, |current, outcome| match outcome {
                    PathOutcome::Need(incoming) => Some(match current {
                        Some(current) => current.union(incoming),
                        None => incoming.dupe(),
                    }),
                    PathOutcome::Complete(_) => current,
                });

        let mut next_horizon = Vec::new();
        for (include, pending) in preflight.includes().iter().zip(&horizon) {
            let request = include.include();
            let logical_path = include.logical_path();
            let (file, file_observations) = match outcomes
                .get(logical_path)
                .expect("every selected logical include path was computed")
            {
                PathOutcome::Need(_) => {
                    return observed_path_need(
                        all_need.expect("the current occurrence contributed a Need"),
                    );
                }
                PathOutcome::Complete(Err(error)) => return observed_outer(error.dupe()),
                PathOutcome::Complete(Ok(file)) => file,
            };
            observations = match merge_observations(mode, observations, file_observations) {
                Ok(observations) => observations,
                Err(error) => return observed_outer(error),
            };
            let bytes = match file {
                Ok(HostFileBytes::Missing) => {
                    return observed_error(
                        HostRootModuleFileError::IncludeMissing {
                            raw_label: CompactString::new(request.raw_label()),
                            location: request.location().clone(),
                            logical_path: logical_path.dupe(),
                        },
                        observations,
                    );
                }
                Err(error) => {
                    return observed_error(
                        HostRootModuleFileError::IncludeFile {
                            raw_label: CompactString::new(request.raw_label()),
                            location: request.location().clone(),
                            logical_path: logical_path.dupe(),
                            error: error.clone(),
                        },
                        observations,
                    );
                }
                Ok(HostFileBytes::Present(bytes)) => bytes.dupe(),
            };
            let relative_path =
                include_relative_path(request.package().package(), request.target());
            let logical_id = LogicalModuleFileId::new(logical_path.as_path().display().to_string());
            let inspection = match validate_root_module_source(logical_id, bytes.as_ref()) {
                Ok(inspection) => inspection,
                Err(message) => {
                    return observed_error(
                        HostRootModuleFileError::IncludeValidation {
                            raw_label: CompactString::new(request.raw_label()),
                            location: request.location().clone(),
                            logical_path: logical_path.dupe(),
                            message,
                        },
                        observations,
                    );
                }
            };
            if pending.ancestry.contains(logical_path) {
                return observed_error(
                    HostRootModuleFileError::IncludeCycle {
                        raw_label: CompactString::new(request.raw_label()),
                        location: request.location().clone(),
                        logical_path: logical_path.dupe(),
                    },
                    observations,
                );
            }
            let ancestry = Arc::new(RootModuleIncludeAncestry {
                logical_path: logical_path.dupe(),
                parent: Some(pending.ancestry.dupe()),
            });
            let source = Arc::new(
                std::str::from_utf8(bytes.as_ref())
                    .expect("successful validation established UTF-8")
                    .to_owned(),
            );
            next_horizon.extend(inspection.includes.iter().cloned().map(|request| {
                PendingRootModuleInclude {
                    request,
                    ancestry: ancestry.dupe(),
                }
            }));
            let index = files.len();
            include_indices.insert(CompactString::new(request.raw_label()), index);
            module_file_paths.push(relative_path);
            evaluation_occurrences.push(NonrootIncludeRequest {
                path: CompactString::new(request.raw_label()),
                location: request.location().clone(),
            });
            files.push(RootModuleSourceFile {
                path: logical_path.as_path().to_path_buf(),
                source,
                _inspection: inspection,
            });
        }
        horizon = next_horizon;
    }

    module_file_paths.sort();
    module_file_paths.dedup();
    let (carrier, captured) = evaluate_root_module_terminal(
        ignore_dev_dependency,
        files,
        include_indices,
        module_file_paths.into(),
        evaluation_occurrences,
        capture_events,
    );
    *event_batch = captured;
    observed_complete(carrier, observations)
}

#[async_trait]
impl Key for HostRootModuleFileObservationKey {
    type Value = ObservedHostRootModuleFileOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let outcome = drive_root_module(
            ctx,
            &self.workspace,
            RootModuleFrontierMode::Observed,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && matches!(&outcome, SourcePreparationOutcome::Complete(Ok(_))) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("observed Host root-module key stores exactly one event batch");
        }
        outcome
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLoadingAnchor {
    carrier: HostRootModuleFileCarrier,
}

impl fmt::Debug for RootModuleLoadingAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootModuleLoadingAnchor")
    }
}

impl RootModuleLoadingAnchor {
    pub fn registrations(&self) -> &RootModuleRegistrations {
        &self
            .carrier
            .as_ref()
            .as_ref()
            .expect("root module loading anchor retains a successful carrier")
            .module
            .registrations
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLoadingAnchorError {
    carrier: HostRootModuleFileCarrier,
}

impl fmt::Debug for RootModuleLoadingAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootModuleLoadingAnchorError")
    }
}

impl fmt::Display for RootModuleLoadingAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.carrier.as_ref() {
            Err(error) => fmt::Display::fmt(error, f),
            Ok(_) => unreachable!("root-loading anchor error retains an error carrier"),
        }
    }
}

impl std::error::Error for RootModuleLoadingAnchorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.carrier.as_ref() {
            Err(error) => std::error::Error::source(error),
            Ok(_) => unreachable!("root-loading anchor error retains an error carrier"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootModuleLoadingAnchorKey {
    workspace: NormalizedAbsolutePath,
}

impl RootModuleLoadingAnchorKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RootModuleLoadingAnchorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-loading-anchor:{}", self.workspace)
    }
}

#[async_trait]
impl Key for RootModuleLoadingAnchorKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>>,
    >;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        dice_invariant(
            ctx.compute(&HostRootModuleFileKey::new(self.workspace.dupe()))
                .await,
        )
        .map(|carrier| {
            Arc::new(if carrier.is_ok() {
                Ok(RootModuleLoadingAnchor { carrier })
            } else {
                Err(RootModuleLoadingAnchorError { carrier })
            })
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedRootModuleLoadingAnchor {
    result: Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>,
    observations: PathObservationEpoch,
}

impl ObservedRootModuleLoadingAnchor {
    #[doc(hidden)]
    pub fn result(&self) -> &Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError> {
        &self.result
    }

    #[doc(hidden)]
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootModuleLoadingAnchorObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl RootModuleLoadingAnchorObservationKey {
    #[doc(hidden)]
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RootModuleLoadingAnchorObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-root-module-loading-anchor:{}", self.workspace)
    }
}

type ObservedRootModuleLoadingAnchorOutcome =
    SourcePreparationOutcome<Result<ObservedRootModuleLoadingAnchor, ObservedPathFrontierError>>;

fn project_observed_root_module_loading_anchor(
    outcome: ObservedHostRootModuleFileOutcome,
) -> ObservedRootModuleLoadingAnchorOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(error))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            let ObservedHostRootModuleFile {
                result: carrier,
                observations,
            } = observed;
            let result = if carrier.is_ok() {
                Ok(RootModuleLoadingAnchor { carrier })
            } else {
                Err(RootModuleLoadingAnchorError { carrier })
            };
            SourcePreparationOutcome::Complete(Ok(ObservedRootModuleLoadingAnchor {
                result,
                observations,
            }))
        }
    }
}

#[async_trait]
impl Key for RootModuleLoadingAnchorObservationKey {
    type Value = ObservedRootModuleLoadingAnchorOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        project_observed_root_module_loading_anchor(dice_invariant(
            ctx.compute(&HostRootModuleFileObservationKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, Allocative)]
#[doc(hidden)]
pub struct SelectedRegistryRepositoryRoute {
    definition: HostCanonicalSelectedModuleDefinition,
    repo_spec: RepoSpec,
    local_path_policy: crate::HostRepositoryLocalPathPolicy,
    mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
}

impl PartialEq for SelectedRegistryRepositoryRoute {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition
            && self.repo_spec == other.repo_spec
            && self.local_path_policy == other.local_path_policy
            && self.mapping == other.mapping
    }
}

impl Eq for SelectedRegistryRepositoryRoute {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RootRepositorySource {
    DirectLocal(RepoSpec),
    BuiltinBazelTools(BuiltinBazelToolsRouteIdentity),
    SelectedRegistry(SelectedRegistryRepositoryRoute),
    /// Extension-generated repository routed from core's accepted private
    /// generated-definition view. Constructed only through
    /// [`RootRepositoryRoute::for_generated_repo_spec`], which enforces the
    /// nonroot/non-bazel_tools/LocalUnsupported invariant.
    Generated {
        repo_spec: RepoSpec,
        local_path_policy: crate::HostRepositoryLocalPathPolicy,
        plan: GeneratedRepositoryFileEffectPlan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum HostRepositoryMaterializationFlavor {
    Classified,
    GeneratedFileEffects(GeneratedRepositoryFileEffectPlan),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositorySourceCapabilitySource {
    Builtin(BuiltinBazelToolsRouteIdentity),
    RepoSpec {
        repo_spec: Arc<RepoSpec>,
        local_path_policy: crate::HostRepositoryLocalPathPolicy,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceCapability {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    canonical_repo: CanonicalRepoName,
    source: HostRepositorySourceCapabilitySource,
    materialization_flavor: HostRepositoryMaterializationFlavor,
}

impl HostRepositorySourceCapability {
    #[doc(hidden)]
    pub fn from_repo_spec(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        canonical_repo: CanonicalRepoName,
        repo_spec: &RepoSpec,
        local_path_policy: crate::HostRepositoryLocalPathPolicy,
    ) -> Option<Self> {
        (!apparent_repo.is_root()
            && !canonical_repo.is_root()
            && canonical_repo.as_str() != "bazel_tools")
            .then(|| Self {
                workspace,
                apparent_repo,
                canonical_repo,
                source: HostRepositorySourceCapabilitySource::RepoSpec {
                    repo_spec: Arc::new(repo_spec.clone()),
                    local_path_policy,
                },
                materialization_flavor: HostRepositoryMaterializationFlavor::Classified,
            })
    }

    fn from_generated_repo_spec(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        canonical_repo: CanonicalRepoName,
        repo_spec: &RepoSpec,
        local_path_policy: crate::HostRepositoryLocalPathPolicy,
        plan: &GeneratedRepositoryFileEffectPlan,
    ) -> Option<Self> {
        Self::from_repo_spec(
            workspace,
            apparent_repo,
            canonical_repo,
            repo_spec,
            local_path_policy,
        )
        .map(|mut capability| {
            capability.materialization_flavor =
                HostRepositoryMaterializationFlavor::GeneratedFileEffects(plan.clone());
            capability
        })
    }

    #[doc(hidden)]
    pub fn builtin(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        canonical_repo: CanonicalRepoName,
        identity: BuiltinBazelToolsRouteIdentity,
    ) -> Option<Self> {
        (apparent_repo.as_str() == "bazel_tools" && canonical_repo.as_str() == "bazel_tools")
            .then_some(Self {
                workspace,
                apparent_repo,
                canonical_repo,
                source: HostRepositorySourceCapabilitySource::Builtin(identity),
                materialization_flavor: HostRepositoryMaterializationFlavor::Classified,
            })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn apparent_repo(&self) -> &ApparentRepoName {
        &self.apparent_repo
    }
    pub fn canonical_repo(&self) -> &CanonicalRepoName {
        &self.canonical_repo
    }
    pub fn source(&self) -> &HostRepositorySourceCapabilitySource {
        &self.source
    }
    pub fn repo_spec(&self) -> Option<&RepoSpec> {
        match &self.source {
            HostRepositorySourceCapabilitySource::Builtin(_) => None,
            HostRepositorySourceCapabilitySource::RepoSpec { repo_spec, .. } => Some(repo_spec),
        }
    }
    pub fn local_path_policy(&self) -> Option<crate::HostRepositoryLocalPathPolicy> {
        match &self.source {
            HostRepositorySourceCapabilitySource::Builtin(_) => None,
            HostRepositorySourceCapabilitySource::RepoSpec {
                local_path_policy, ..
            } => Some(*local_path_policy),
        }
    }

    pub(crate) fn generated_file_effect_plan(&self) -> Option<&GeneratedRepositoryFileEffectPlan> {
        match &self.materialization_flavor {
            HostRepositoryMaterializationFlavor::Classified => None,
            HostRepositoryMaterializationFlavor::GeneratedFileEffects(plan) => Some(plan),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RootRepositoryRoute {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    module_name: CompactString,
    canonical_repo: CanonicalRepoName,
    source: RootRepositorySource,
}

fn hash_override_attribute_value<H: Hasher>(value: &OverrideAttributeValue, state: &mut H) {
    std::mem::discriminant(value).hash(state);
    match value {
        OverrideAttributeValue::None => {}
        OverrideAttributeValue::Bool(value) => value.hash(state),
        OverrideAttributeValue::Int(value) => value.hash(state),
        OverrideAttributeValue::String(value) => value.hash(state),
        OverrideAttributeValue::Label(value) => value.hash(state),
        OverrideAttributeValue::Iterable(values) => {
            values.len().hash(state);
            for value in values.iter() {
                hash_override_attribute_value(value, state);
            }
        }
        OverrideAttributeValue::Map(values) => {
            hash_override_attribute_map(values, state);
        }
    }
}

fn hash_override_attribute_map<H: Hasher>(
    values: &SmallMap<OverrideAttributeKey, OverrideAttributeValue>,
    state: &mut H,
) {
    let mut entry_hashes = values
        .iter()
        .map(|(key, value)| {
            let mut entry = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut entry);
            hash_override_attribute_value(value, &mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    entry_hashes.sort_unstable();
    entry_hashes.hash(state);
}

fn hash_repo_spec<H: Hasher>(spec: &RepoSpec, state: &mut H) {
    spec.rule_id.bzl_file.hash(state);
    spec.rule_id.rule_name.hash(state);
    let mut entry_hashes = spec
        .attributes
        .iter()
        .map(|(key, value)| {
            let mut entry = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut entry);
            hash_override_attribute_value(value, &mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    entry_hashes.sort_unstable();
    entry_hashes.hash(state);
}

impl Hash for SelectedRegistryRepositoryRoute {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_repo_spec(&self.repo_spec, state);
        self.local_path_policy.hash(state);
        self.mapping.hash(state);
    }
}

impl Hash for RootRepositorySource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::DirectLocal(spec) => hash_repo_spec(spec, state),
            Self::BuiltinBazelTools(identity) => identity.hash(state),
            Self::SelectedRegistry(route) => route.hash(state),
            Self::Generated {
                repo_spec,
                local_path_policy,
                plan,
            } => {
                hash_repo_spec(repo_spec, state);
                local_path_policy.hash(state);
                plan.hash(state);
            }
        }
    }
}

impl Hash for HostRepositorySourceCapabilitySource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Builtin(identity) => identity.hash(state),
            Self::RepoSpec {
                repo_spec,
                local_path_policy,
            } => {
                hash_repo_spec(repo_spec, state);
                local_path_policy.hash(state);
            }
        }
    }
}

impl Hash for HostRepositorySourceCapability {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.apparent_repo.hash(state);
        self.canonical_repo.hash(state);
        self.source.hash(state);
        self.materialization_flavor.hash(state);
    }
}

impl Hash for RootRepositoryRoute {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.apparent_repo.hash(state);
        self.module_name.hash(state);
        self.canonical_repo.hash(state);
        self.source.hash(state);
    }
}

impl fmt::Debug for RootRepositoryRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootRepositoryRoute")
            .field("apparent_repo", &self.apparent_repo)
            .field("canonical_repo", &self.canonical_repo)
            .finish_non_exhaustive()
    }
}

impl RootRepositoryRoute {
    #[doc(hidden)]
    pub fn source_capability(&self) -> HostRepositorySourceCapability {
        match &self.source {
            RootRepositorySource::DirectLocal(spec) => {
                HostRepositorySourceCapability::from_repo_spec(
                    self.workspace.dupe(),
                    self.apparent_repo.clone(),
                    self.canonical_repo.clone(),
                    spec,
                    crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
                )
            }
            RootRepositorySource::Generated {
                repo_spec,
                local_path_policy,
                plan,
            } => HostRepositorySourceCapability::from_generated_repo_spec(
                self.workspace.dupe(),
                self.apparent_repo.clone(),
                self.canonical_repo.clone(),
                repo_spec,
                *local_path_policy,
                plan,
            ),
            RootRepositorySource::SelectedRegistry(route) => {
                HostRepositorySourceCapability::from_repo_spec(
                    self.workspace.dupe(),
                    self.apparent_repo.clone(),
                    self.canonical_repo.clone(),
                    &route.repo_spec,
                    route.local_path_policy,
                )
            }
            RootRepositorySource::BuiltinBazelTools(identity) => {
                HostRepositorySourceCapability::builtin(
                    self.workspace.dupe(),
                    self.apparent_repo.clone(),
                    self.canonical_repo.clone(),
                    identity.clone(),
                )
            }
        }
        .expect("a RootRepositoryRoute has a valid source-capability polarity")
    }

    pub fn apparent_repo(&self) -> &ApparentRepoName {
        &self.apparent_repo
    }

    pub fn canonical_repo(&self) -> &CanonicalRepoName {
        &self.canonical_repo
    }

    pub fn module_name(&self) -> &str {
        self.module_name.as_str()
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn source(&self) -> &RootRepositorySource {
        &self.source
    }

    /// Immutable selected mapping used while evaluating this route's `.bzl`
    /// modules. Other route families remain fail-closed until admitted.
    #[doc(hidden)]
    pub fn bzl_repository_mapping(&self) -> Arc<[(ApparentRepoName, CanonicalRepoName)]> {
        match &self.source {
            RootRepositorySource::SelectedRegistry(route) => route.mapping.clone(),
            _ => Arc::from([]),
        }
    }

    pub fn is_builtin_bazel_tools(&self) -> bool {
        matches!(self.source, RootRepositorySource::BuiltinBazelTools(_))
    }

    #[doc(hidden)]
    pub fn for_selected_extension_definition(
        workspace: NormalizedAbsolutePath,
        source: &HostSelectedExtensionDefinitionSource,
    ) -> Option<Self> {
        let HostSelectedExtensionDefinitionSource::Selected {
            definition,
            apparent_repo,
        } = source
        else {
            return None;
        };
        let view = definition.view();
        if view.kind() != HostCanonicalSelectedModuleKind::SelectedRegistry {
            return None;
        }
        let repo_spec = view.repo_spec()?.clone();
        let local_path_policy = view.local_path_policy()?;
        let HostCanonicalSelectedModuleIdentity::Module { name, .. } = view.identity() else {
            return None;
        };
        let canonical_repo = view.canonical_repo().clone();
        let mapping = view
            .mapping()
            .map(|(name, target)| (name.clone(), target.clone()))
            .collect();
        Some(Self {
            workspace,
            apparent_repo: apparent_repo.clone(),
            module_name: CompactString::new(name),
            canonical_repo,
            source: RootRepositorySource::SelectedRegistry(SelectedRegistryRepositoryRoute {
                definition: definition.clone(),
                repo_spec,
                local_path_policy,
                mapping,
            }),
        })
    }

    #[doc(hidden)]
    pub fn selected_bzl_load_route(&self, apparent: &ApparentRepoName) -> Option<Self> {
        let RootRepositorySource::SelectedRegistry(route) = &self.source else {
            return None;
        };
        match route.definition.mapped_bzl_load(apparent)? {
            HostSelectedBzlLoadSource::Selected(definition) => {
                Self::for_selected_extension_definition(
                    self.workspace.dupe(),
                    &HostSelectedExtensionDefinitionSource::Selected {
                        definition,
                        apparent_repo: apparent.clone(),
                    },
                )
            }
            HostSelectedBzlLoadSource::Builtin => Some(Self {
                workspace: self.workspace.dupe(),
                apparent_repo: apparent.clone(),
                module_name: CompactString::new("bazel_tools"),
                canonical_repo: CanonicalRepoName::new("bazel_tools").ok()?,
                source: RootRepositorySource::BuiltinBazelTools(
                    BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
                ),
            }),
        }
    }

    /// Constructs the route for an extension-generated repository from core's
    /// accepted generated-definition view. The module name is the apparent
    /// name's owning extension repository segment; polarity must be
    /// `LocalUnsupported`, matching the private core view invariant exactly.
    #[doc(hidden)]
    pub fn for_generated_repo_spec(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
        local_path_policy: crate::HostRepositoryLocalPathPolicy,
        plan: GeneratedRepositoryFileEffectPlan,
    ) -> Option<Self> {
        if apparent_repo.is_root()
            || canonical_repo.is_root()
            || canonical_repo.as_str() == "bazel_tools"
            || local_path_policy != crate::HostRepositoryLocalPathPolicy::LocalUnsupported
        {
            return None;
        }
        Some(Self {
            workspace,
            module_name: CompactString::new(apparent_repo.as_str()),
            apparent_repo,
            canonical_repo,
            source: RootRepositorySource::Generated {
                repo_spec,
                local_path_policy,
                plan,
            },
        })
    }

    pub(crate) fn repo_spec(&self) -> &RepoSpec {
        match &self.source {
            RootRepositorySource::DirectLocal(spec) => spec,
            RootRepositorySource::SelectedRegistry(route) => &route.repo_spec,
            RootRepositorySource::Generated { repo_spec, .. } => repo_spec,
            RootRepositorySource::BuiltinBazelTools(_) => {
                panic!("built-in bazel_tools has no RepoSpec")
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        module_name: CompactString,
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
    ) -> Self {
        Self {
            workspace,
            apparent_repo,
            module_name,
            canonical_repo,
            source: RootRepositorySource::DirectLocal(repo_spec),
        }
    }

    #[cfg(test)]
    pub(crate) fn builtin_for_test(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            apparent_repo: ApparentRepoName::new("bazel_tools")
                .expect("built-in apparent name is valid"),
            module_name: CompactString::new("bazel_tools"),
            canonical_repo: CanonicalRepoName::new("bazel_tools")
                .expect("built-in canonical name is valid"),
            source: RootRepositorySource::BuiltinBazelTools(
                BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootRepositoryRouteErrorKind {
    Root(HostRootModuleFileError),
    Unknown {
        apparent_repo: ApparentRepoName,
    },
    Unsupported {
        apparent_repo: ApparentRepoName,
        module_name: CompactString,
    },
    SelectedMapping(HostRootRepositoryMappingError),
    SelectedDefinition(HostCanonicalSelectedModuleDefinitionError),
    SelectedInfrastructure(Arc<str>),
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RootRepositoryRouteError {
    kind: RootRepositoryRouteErrorKind,
}

impl RootRepositoryRouteError {
    /// Distinguishes the two nonroot route failure kinds so core's external
    /// build branch can fall back to the generated-repository bridge while
    /// preserving every other diagnostic byte-exactly.
    #[doc(hidden)]
    pub fn is_generated_route_fallback(&self) -> bool {
        matches!(
            self.kind,
            RootRepositoryRouteErrorKind::Unknown { .. }
                | RootRepositoryRouteErrorKind::Unsupported { .. }
        )
    }
}

impl fmt::Debug for RootRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootRepositoryRouteError")
    }
}

impl fmt::Display for RootRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RootRepositoryRouteErrorKind::Root(error) => error.fmt(f),
            RootRepositoryRouteErrorKind::Unknown { apparent_repo } => write!(
                f,
                "no such package '@@[unknown repo '{}' requested from @@]//': The repository '@@[unknown repo '{}' requested from @@]' could not be resolved: No repository visible as '@{}' from main repository",
                apparent_repo.as_str(),
                apparent_repo.as_str(),
                apparent_repo.as_str(),
            ),
            RootRepositoryRouteErrorKind::Unsupported {
                apparent_repo,
                module_name,
            } => write!(
                f,
                "external repository '@{}' from module '{}' is not a direct local_path_override",
                apparent_repo.as_str(),
                module_name,
            ),
            RootRepositoryRouteErrorKind::SelectedMapping(error) => error.fmt(f),
            RootRepositoryRouteErrorKind::SelectedDefinition(error) => error.fmt(f),
            RootRepositoryRouteErrorKind::SelectedInfrastructure(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for RootRepositoryRouteError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootRepositoryRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    admission: RootRepositoryRouteAdmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
enum RootRepositoryRouteAdmission {
    Ordinary,
    RootBuild,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedRootRepositoryRoute {
    result: Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>,
    observations: PathObservationEpoch,
}

impl ObservedRootRepositoryRoute {
    #[doc(hidden)]
    pub fn result(&self) -> &Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>> {
        &self.result
    }

    #[doc(hidden)]
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootRepositoryRouteObservationKey(RootRepositoryRouteKey);

/// The selected-only route projection distinguishes retryable path frontiers
/// from infrastructure failures. Ordinary consumers are intentionally limited
/// to `Path` through [`Self::ordinary_path`].
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RootRepositoryRouteObservationError {
    Path(ObservedPathFrontierError),
    Mapping(HostRootRepositoryMappingObservationError),
    Definition(HostCanonicalSelectedModuleDefinitionObservationError),
    Infrastructure(Arc<str>),
}

impl RootRepositoryRouteObservationError {
    #[doc(hidden)]
    pub fn selected_frontier(self) -> HostSelectedObservationFrontier {
        match self {
            Self::Path(error) => HostSelectedObservationFrontier::Path(error),
            Self::Mapping(error) => error.selected_frontier(),
            Self::Definition(error) => error.selected_frontier(),
            Self::Infrastructure(message) => {
                HostSelectedObservationFrontier::Infrastructure(message)
            }
        }
    }

    #[doc(hidden)]
    pub fn ordinary_path(self) -> ObservedPathFrontierError {
        match self {
            Self::Path(error) => error,
            Self::Mapping(_) | Self::Definition(_) | Self::Infrastructure(_) => {
                panic!("ordinary root repository route reached selected admission")
            }
        }
    }
}

impl RootRepositoryRouteKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        if apparent_repo.is_root() {
            return Err("external repository route requires a nonroot apparent name".to_owned());
        }
        Ok(Self {
            workspace,
            apparent_repo,
            admission: RootRepositoryRouteAdmission::Ordinary,
        })
    }

    #[doc(hidden)]
    pub fn for_root_build(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        let mut key = Self::new(workspace, apparent_repo)?;
        key.admission = RootRepositoryRouteAdmission::RootBuild;
        Ok(key)
    }
}

impl RootRepositoryRouteObservationKey {
    #[doc(hidden)]
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        RootRepositoryRouteKey::new(workspace, apparent_repo).map(Self)
    }

    #[doc(hidden)]
    pub fn for_root_build(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        RootRepositoryRouteKey::for_root_build(workspace, apparent_repo).map(Self)
    }
}

impl fmt::Display for RootRepositoryRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:@{}",
            match self.admission {
                RootRepositoryRouteAdmission::Ordinary => "root-repository-route",
                RootRepositoryRouteAdmission::RootBuild => "root-build-repository-route",
            },
            self.workspace,
            self.apparent_repo.as_str()
        )
    }
}

impl fmt::Display for RootRepositoryRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

fn is_local_path_override(spec: &RepoSpec) -> bool {
    let local_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
        .expect("pinned local repository label is canonical");
    spec.rule_id.bzl_file == local_bzl && spec.rule_id.rule_name == "local_repository"
}

fn project_root_repository_route(
    key: &RootRepositoryRouteKey,
    carrier: &HostRootModuleFileCarrier,
) -> Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>> {
    let value = match carrier.as_ref() {
        Err(error) => Err(RootRepositoryRouteError {
            kind: RootRepositoryRouteErrorKind::Root(error.clone()),
        }),
        Ok(root) => {
            if key.apparent_repo.as_str() == "bazel_tools" {
                return Arc::new(Ok(RootRepositoryRoute {
                    workspace: key.workspace.dupe(),
                    apparent_repo: key.apparent_repo.clone(),
                    module_name: CompactString::new("bazel_tools"),
                    canonical_repo: CanonicalRepoName::new("bazel_tools")
                        .expect("built-in repository name is canonical"),
                    source: RootRepositorySource::BuiltinBazelTools(
                        BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
                    ),
                }));
            }
            let dependency = root.module.dependencies.iter().find(|dependency| {
                !dependency.nodep
                    && dependency
                        .repo_name
                        .as_deref()
                        .unwrap_or(dependency.name.as_str())
                        == key.apparent_repo.as_str()
            });
            match dependency {
                None => Err(RootRepositoryRouteError {
                    kind: RootRepositoryRouteErrorKind::Unknown {
                        apparent_repo: key.apparent_repo.clone(),
                    },
                }),
                Some(dependency) => {
                    let repo_spec = match root.overrides.get(dependency.name.as_str()) {
                        Some(RootModuleOverride::NonRegistry(spec))
                            if is_local_path_override(spec) =>
                        {
                            spec.clone()
                        }
                        _ => {
                            return Arc::new(Err(RootRepositoryRouteError {
                                kind: RootRepositoryRouteErrorKind::Unsupported {
                                    apparent_repo: key.apparent_repo.clone(),
                                    module_name: dependency.name.clone(),
                                },
                            }));
                        }
                    };
                    let canonical_repo = CanonicalRepoName::new(format!("{}+", dependency.name))
                        .expect("validated module name forms a canonical repository");
                    Ok(RootRepositoryRoute {
                        workspace: key.workspace.dupe(),
                        apparent_repo: key.apparent_repo.clone(),
                        module_name: dependency.name.clone(),
                        canonical_repo,
                        source: RootRepositorySource::DirectLocal(repo_spec),
                    })
                }
            }
        }
    };
    Arc::new(value)
}

fn original_unsupported(
    value: &Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>,
) -> Option<RootRepositoryRouteError> {
    match value.as_ref() {
        Err(error) if matches!(error.kind, RootRepositoryRouteErrorKind::Unsupported { .. }) => {
            Some(error.clone())
        }
        _ => None,
    }
}

fn selected_route(
    key: &RootRepositoryRouteKey,
    definition: HostCanonicalSelectedModuleDefinition,
) -> Option<RootRepositoryRoute> {
    let source = HostSelectedExtensionDefinitionSource::Selected {
        definition,
        apparent_repo: key.apparent_repo.clone(),
    };
    RootRepositoryRoute::for_selected_extension_definition(key.workspace.dupe(), &source)
}

fn selected_definition_for_root_mapping(
    key: &RootRepositoryRouteKey,
    mapping: &crate::HostRootRepositoryMapping,
) -> Option<CanonicalRepoName> {
    mapping.view()?.mapping().find_map(|(apparent, canonical)| {
        (apparent == &key.apparent_repo).then(|| canonical.clone())
    })
}

async fn admit_selected_root_repository_route(
    ctx: &mut DiceComputations<'_>,
    key: &RootRepositoryRouteKey,
    original: Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>,
) -> SourcePreparationOutcome<Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>> {
    let Some(fallback) = original_unsupported(&original) else {
        return SourcePreparationOutcome::Complete(original);
    };
    let mapping = match ctx
        .compute(&HostRootRepositoryMappingKey::new(key.workspace.dupe()))
        .await
    {
        Err(error) => {
            return SourcePreparationOutcome::Complete(Arc::new(Err(RootRepositoryRouteError {
                kind: RootRepositoryRouteErrorKind::SelectedInfrastructure(Arc::from(
                    error.to_string(),
                )),
            })));
        }
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
    };
    let mapping = match mapping.as_ref() {
        Ok(mapping) => mapping,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Arc::new(Err(RootRepositoryRouteError {
                kind: RootRepositoryRouteErrorKind::SelectedMapping(error.clone()),
            })));
        }
    };
    let Some(canonical) = selected_definition_for_root_mapping(key, mapping) else {
        return SourcePreparationOutcome::Complete(Arc::new(Err(fallback)));
    };
    let definition = match ctx
        .compute(&HostCanonicalSelectedModuleDefinitionKey::new(
            key.workspace.dupe(),
            canonical,
        ))
        .await
    {
        Err(error) => {
            return SourcePreparationOutcome::Complete(Arc::new(Err(RootRepositoryRouteError {
                kind: RootRepositoryRouteErrorKind::SelectedInfrastructure(Arc::from(
                    error.to_string(),
                )),
            })));
        }
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
    };
    match definition.as_ref() {
        Ok(definition) => match selected_route(key, definition.clone()) {
            Some(route) => SourcePreparationOutcome::Complete(Arc::new(Ok(route))),
            None => SourcePreparationOutcome::Complete(Arc::new(Err(fallback))),
        },
        Err(error)
            if error.disposition()
                == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing =>
        {
            SourcePreparationOutcome::Complete(Arc::new(Err(fallback)))
        }
        Err(error) => SourcePreparationOutcome::Complete(Arc::new(Err(RootRepositoryRouteError {
            kind: RootRepositoryRouteErrorKind::SelectedDefinition(error.clone()),
        }))),
    }
}

type ObservedRootRepositoryRouteOutcome = SourcePreparationOutcome<
    Result<ObservedRootRepositoryRoute, RootRepositoryRouteObservationError>,
>;

fn project_observed_root_repository_route(
    key: &RootRepositoryRouteKey,
    outcome: ObservedHostRootModuleFileOutcome,
) -> ObservedRootRepositoryRouteOutcome {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(
            RootRepositoryRouteObservationError::Path(error),
        )),
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            let result = project_root_repository_route(key, &observed.result);
            SourcePreparationOutcome::Complete(Ok(ObservedRootRepositoryRoute {
                result,
                observations: observed.observations,
            }))
        }
    }
}

#[async_trait]
impl Key for RootRepositoryRouteKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let carrier = match dice_invariant(
            ctx.compute(&HostRootModuleFileKey::new(self.workspace.dupe()))
                .await,
        ) {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(carrier) => carrier,
        };
        let original = project_root_repository_route(self, &carrier);
        match self.admission {
            RootRepositoryRouteAdmission::Ordinary => SourcePreparationOutcome::Complete(original),
            RootRepositoryRouteAdmission::RootBuild => {
                admit_selected_root_repository_route(ctx, self, original).await
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
impl Key for RootRepositoryRouteObservationKey {
    type Value = ObservedRootRepositoryRouteOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let root = project_observed_root_repository_route(
            &self.0,
            dice_invariant(
                ctx.compute(&HostRootModuleFileObservationKey::new(
                    self.0.workspace.dupe(),
                ))
                .await,
            ),
        );
        let SourcePreparationOutcome::Complete(Ok(observed)) = root else {
            return root;
        };
        if self.0.admission == RootRepositoryRouteAdmission::Ordinary
            || original_unsupported(observed.result()).is_none()
        {
            return SourcePreparationOutcome::Complete(Ok(observed));
        }
        let fallback = original_unsupported(observed.result()).expect("checked above");
        let mut observations = observed.observations().dupe();
        let mapping = match ctx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                self.0.workspace.dupe(),
            ))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Infrastructure(Arc::from(
                        error.to_string(),
                    )),
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Mapping(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
        };
        observations = match PathObservationEpoch::from_shared(
            observations
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain(
                    mapping
                        .observations()
                        .observations()
                        .iter()
                        .map(|(demand, result)| (demand.dupe(), result.dupe())),
                ),
        ) {
            Ok(observations) => observations,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Path(error.into()),
                ));
            }
        };
        let mapping = match mapping.result().as_ref() {
            Ok(mapping) => mapping,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok(ObservedRootRepositoryRoute {
                    result: Arc::new(Err(RootRepositoryRouteError {
                        kind: RootRepositoryRouteErrorKind::SelectedMapping(error.clone()),
                    })),
                    observations,
                }));
            }
        };
        let Some(canonical) = selected_definition_for_root_mapping(&self.0, mapping) else {
            return SourcePreparationOutcome::Complete(Ok(ObservedRootRepositoryRoute {
                result: Arc::new(Err(fallback)),
                observations,
            }));
        };
        let definition = match ctx
            .compute(&HostCanonicalSelectedModuleDefinitionObservationKey::new(
                self.0.workspace.dupe(),
                canonical,
            ))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Infrastructure(Arc::from(
                        error.to_string(),
                    )),
                ));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Definition(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
        };
        observations = match PathObservationEpoch::from_shared(
            observations
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain(
                    definition
                        .observations()
                        .observations()
                        .iter()
                        .map(|(demand, result)| (demand.dupe(), result.dupe())),
                ),
        ) {
            Ok(observations) => observations,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootRepositoryRouteObservationError::Path(error.into()),
                ));
            }
        };
        let result = match definition.result().as_ref() {
            Ok(definition) => selected_route(&self.0, definition.clone()).ok_or(fallback),
            Err(error)
                if error.disposition()
                    == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing =>
            {
                Err(fallback)
            }
            Err(error) => Err(RootRepositoryRouteError {
                kind: RootRepositoryRouteErrorKind::SelectedDefinition(error.clone()),
            }),
        };
        SourcePreparationOutcome::Complete(Ok(ObservedRootRepositoryRoute {
            result: Arc::new(result),
            observations,
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::error::Error as _;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use compact_str::CompactString;
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
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::ObservedPathFrontierError;
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
    use slug_workspace_v2::WorkspaceSnapshot;
    use slug_workspace_v2::WorkspaceSnapshotKey;
    use starlark_map::small_map::SmallMap;
    use starlark_map::sorted_map::SortedMap;

    use super::HostRootModuleFileError;
    use super::HostRootModuleFileKey;
    use super::HostRootModuleFileObservationKey;
    use super::HostRootModuleFileValue;
    use super::ObservedHostRootModuleFile;
    use super::ObservedRootRepositoryRoute;
    use super::RootModuleLoadingAnchor;
    use super::RootModuleLoadingAnchorError;
    use super::RootModuleLoadingAnchorKey;
    use super::RootModuleLoadingAnchorObservationKey;
    use super::RootRepositoryRoute;
    use super::RootRepositoryRouteError;
    use super::RootRepositoryRouteErrorKind;
    use super::RootRepositoryRouteKey;
    use super::RootRepositoryRouteObservationError;
    use super::RootRepositoryRouteObservationKey;
    use super::RootRepositorySource;
    use super::project_observed_root_module_loading_anchor;
    use super::project_observed_root_repository_route;
    use crate::BzlmodCommandPolicyKey;
    use crate::BzlmodEnvironmentPolicyKey;
    use crate::EvaluatedRootModule;
    use crate::LockfileMode;
    use crate::RootModuleOverrides;
    use crate::RootPackagePolicyInputs;
    use crate::SourcePreparationNeeds;
    use crate::SourcePreparationOutcome;
    use crate::host_file::HostFileBytesKey;
    use crate::host_file::HostFileBytesObservationKey;
    use crate::host_package::HostRootPackageLookupKey;
    use crate::host_package::HostRootPackageLookupObservationKey;
    use crate::inject_root_module_request_inputs;
    use crate::inject_root_package_policy_inputs;
    use crate::module_eval::RootModuleFilesKey;
    use crate::module_eval::RootModuleFilesObservationKey;
    use crate::module_eval::clear_validated_root_module_logical_ids;
    use crate::module_eval::take_validated_root_module_logical_ids;
    use crate::repo_file::HostRepoFileKey;

    #[test]
    fn generated_route_fallback_classifier_is_limited_to_nonroot_route_errors() {
        let apparent_repo = ApparentRepoName::new("extension_repo").unwrap();
        let unknown = RootRepositoryRouteError {
            kind: RootRepositoryRouteErrorKind::Unknown {
                apparent_repo: apparent_repo.clone(),
            },
        };
        let unsupported = RootRepositoryRouteError {
            kind: RootRepositoryRouteErrorKind::Unsupported {
                apparent_repo,
                module_name: CompactString::new("extension_module"),
            },
        };
        let root = RootRepositoryRouteError {
            kind: RootRepositoryRouteErrorKind::Root(HostRootModuleFileError::CommandPolicy {
                message: CompactString::new("root route"),
            }),
        };

        assert!(unknown.is_generated_route_fallback());
        assert!(unsupported.is_generated_route_fallback());
        assert!(!root.is_generated_route_fallback());
    }

    fn workspace() -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new("/workspace").unwrap()
    }

    fn external_source_capability_snapshot(
        route: &crate::RootRepositoryRoute,
    ) -> (
        String,
        String,
        bool,
        Option<crate::HostRepositoryLocalPathPolicy>,
    ) {
        let capability: crate::HostRepositorySourceCapability = route.source_capability();
        (
            capability.apparent_repo().as_str().to_owned(),
            capability.canonical_repo().as_str().to_owned(),
            matches!(
                capability.source(),
                crate::HostRepositorySourceCapabilitySource::RepoSpec { .. }
            ),
            capability.local_path_policy(),
        )
    }

    fn empty_value() -> HostRootModuleFileValue {
        HostRootModuleFileValue {
            module: EvaluatedRootModule {
                header: None,
                dependencies: [].into(),
                registrations: Default::default(),
            },
            overrides: RootModuleOverrides::default(),
            module_file_paths: ["MODULE.bazel".into()].into(),
            extension_usages: Arc::from([]),
        }
    }

    #[derive(Default)]
    struct EpochBuilder {
        entries: SmallMap<PathObservationDemand, PathObservationResult>,
    }

    impl EpochBuilder {
        fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        }

        fn directory(&mut self, path: &str, variant: i64) {
            self.node(path, PathNodeKind::Directory, variant);
        }

        fn node(&mut self, path: &str, kind: PathNodeKind, variant: i64) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    kind, variant, variant, variant, variant, 0o755,
                ))),
            );
        }

        fn missing(&mut self, path: &str) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            );
        }

        fn lstat_error(&mut self, path: &str) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Error(
                    slug_workspace_v2::PathObservationError::Io {
                        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                        raw_os_error: Some(13),
                    },
                )),
            );
        }

        fn file(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
            self.node(path, PathNodeKind::RegularFile, variant);
            self.file_kind(path, source, variant, PathNodeKind::RegularFile);
        }

        fn special(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
            self.node(path, PathNodeKind::SpecialFile, variant);
            self.file_kind(path, source, variant, PathNodeKind::SpecialFile);
        }

        fn file_kind(
            &mut self,
            path: &str,
            source: impl AsRef<[u8]>,
            _variant: i64,
            _kind: PathNodeKind,
        ) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    source.as_ref(),
                ))),
            );
        }

        fn root(source: impl AsRef<[u8]>, variant: i64) -> Self {
            let mut builder = Self::default();
            builder.directory("/", variant);
            builder.directory("/workspace", variant);
            builder.file("/workspace/MODULE.bazel", source, variant);
            builder
        }

        fn repository_policy(&mut self, roots: &[&str], variant: i64) {
            self.missing("/workspace/REPO.bazel");
            for root in roots {
                self.directory(root, variant);
                self.missing(&format!("{root}/.bazelignore"));
            }
        }

        fn package(&mut self, root: &str, package: &str, variant: i64) {
            let mut current = PathBuf::from(root);
            for component in package.split('/') {
                current.push(component);
                self.directory(current.to_str().unwrap(), variant);
            }
            self.node(
                &format!("{root}/{package}/BUILD.bazel"),
                PathNodeKind::RegularFile,
                variant,
            );
        }

        fn build(self) -> PathObservationEpoch {
            PathObservationEpoch::new(self.entries).unwrap()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TrackedBatch {
        key: String,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct EventTracker {
        entries: Mutex<Vec<TrackedBatch>>,
        anchor_dependencies: Mutex<Vec<Vec<String>>>,
    }

    impl EventTracker {
        fn take(&self) -> Vec<TrackedBatch> {
            std::mem::take(&mut *self.entries.lock().unwrap())
        }

        fn take_anchor_dependencies(&self) -> Vec<Vec<String>> {
            std::mem::take(&mut *self.anchor_dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for EventTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<RootModuleLoadingAnchorKey>().is_some()
                || key
                    .downcast_ref::<RootModuleLoadingAnchorObservationKey>()
                    .is_some()
                || key.downcast_ref::<RootModuleFilesKey>().is_some()
                || key
                    .downcast_ref::<RootModuleFilesObservationKey>()
                    .is_some()
            {
                self.anchor_dependencies
                    .lock()
                    .unwrap()
                    .push(deps.map(ToString::to_string).collect());
            }
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<HostRootModuleFileKey>().is_none()
                && key
                    .downcast_ref::<HostRootModuleFileObservationKey>()
                    .is_none()
                && key.downcast_ref::<RootModuleLoadingAnchorKey>().is_none()
                && key
                    .downcast_ref::<RootModuleLoadingAnchorObservationKey>()
                    .is_none()
                && key.downcast_ref::<RootModuleFilesKey>().is_none()
                && key
                    .downcast_ref::<RootModuleFilesObservationKey>()
                    .is_none()
                && key.downcast_ref::<RootRepositoryRouteKey>().is_none()
                && key
                    .downcast_ref::<RootRepositoryRouteObservationKey>()
                    .is_none()
                && key.downcast_ref::<HostRepoFileKey>().is_none()
                && key.downcast_ref::<HostFileBytesKey>().is_none()
                && key.downcast_ref::<HostFileBytesObservationKey>().is_none()
                && key.downcast_ref::<HostRootPackageLookupKey>().is_none()
                && key
                    .downcast_ref::<HostRootPackageLookupObservationKey>()
                    .is_none()
            {
                return;
            }
            self.entries.lock().unwrap().push(TrackedBatch {
                key: key.to_string(),
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    #[derive(Default)]
    struct DependencyTracker {
        dependencies: Mutex<Vec<String>>,
    }

    impl DependencyTracker {
        fn take(&self) -> Vec<String> {
            std::mem::take(&mut *self.dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for DependencyTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<HostRootModuleFileKey>().is_some() {
                self.dependencies
                    .lock()
                    .unwrap()
                    .extend(deps.map(ToString::to_string));
            }
        }
    }

    fn policy(roots: &[&str]) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            workspace(),
            roots
                .iter()
                .map(|root| NormalizedAbsolutePath::new(*root).unwrap())
                .collect::<Vec<_>>(),
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap()
    }

    fn snapshot(lockfile: Option<&str>) -> Arc<WorkspaceSnapshot> {
        let files = lockfile
            .into_iter()
            .map(|source| {
                (
                    PathBuf::from("/workspace/MODULE.bazel.lock"),
                    WorkspaceFileValue::Present(Arc::new(source.to_owned())),
                )
            })
            .collect::<SortedMap<_, _>>();
        Arc::new(WorkspaceSnapshot {
            files: Arc::new(files),
        })
    }

    async fn observed(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        roots: &[&str],
        capture_events: bool,
        tracker: Option<Arc<EventTracker>>,
        environment: Option<&str>,
        lockfile_mode: LockfileMode,
        lockfile: Option<&str>,
    ) -> super::HostRootModuleFileOutcome {
        let mut user_data = UserComputationData {
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture_events {
            user_data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(lockfile),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(roots)).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(environment).unwrap(),
            lockfile_mode,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootModuleFileKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_frontier(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        roots: &[&str],
        capture_events: bool,
        tracker: Option<Arc<EventTracker>>,
        inject_request: bool,
    ) -> <HostRootModuleFileObservationKey as Key>::Value {
        let mut user_data = UserComputationData {
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture_events {
            user_data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(None),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(roots)).unwrap();
        if inject_request {
            inject_root_module_request_inputs(
                &mut updater,
                workspace().as_path(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .unwrap();
        }
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootModuleFileObservationKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_root_files(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: &Arc<EventTracker>,
    ) -> <RootModuleFilesObservationKey as Key>::Value {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(None),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater
            .commit()
            .await
            .compute(&RootModuleFilesObservationKey::new(workspace()))
            .await
            .unwrap()
    }

    fn complete_frontier(
        outcome: &<HostRootModuleFileObservationKey as Key>::Value,
    ) -> &ObservedHostRootModuleFile {
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("observed root module did not complete with a frontier: {outcome:?}");
        };
        observed
    }

    async fn observed_anchor(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: &Arc<EventTracker>,
    ) -> SourcePreparationOutcome<Arc<Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>>>
    {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(None),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&RootModuleLoadingAnchorKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_frontier_anchor(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: &Arc<EventTracker>,
    ) -> <RootModuleLoadingAnchorObservationKey as Key>::Value {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(None),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater
            .commit()
            .await
            .compute(&RootModuleLoadingAnchorObservationKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_route(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        apparent_repo: &str,
    ) -> <RootRepositoryRouteKey as Key>::Value {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(
                &RootRepositoryRouteKey::new(
                    workspace(),
                    ApparentRepoName::new(apparent_repo).unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap()
    }

    fn observed_route_key(apparent_repo: &str) -> RootRepositoryRouteObservationKey {
        let apparent_repo = ApparentRepoName::new(apparent_repo).unwrap();
        RootRepositoryRouteObservationKey::new(workspace(), apparent_repo).unwrap()
    }

    async fn observed_route_transaction(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: Option<Arc<EventTracker>>,
    ) -> dice::DiceTransaction {
        let mut user_data = UserComputationData {
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater.commit().await
    }

    async fn observed_route_frontier(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        apparent_repo: &str,
        tracker: Option<Arc<EventTracker>>,
        compute_anchor: bool,
    ) -> <RootRepositoryRouteObservationKey as Key>::Value {
        let mut transaction = observed_route_transaction(dice, epoch, tracker).await;
        if compute_anchor {
            transaction
                .compute(&RootModuleLoadingAnchorObservationKey::new(workspace()))
                .await
                .unwrap();
        }
        transaction
            .compute(&observed_route_key(apparent_repo))
            .await
            .unwrap()
    }

    fn complete_observed_route(
        outcome: &<RootRepositoryRouteObservationKey as Key>::Value,
    ) -> &ObservedRootRepositoryRoute {
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("observed route did not complete: {outcome:?}");
        };
        observed
    }

    fn complete_value(outcome: &super::HostRootModuleFileOutcome) -> &HostRootModuleFileValue {
        match outcome {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap(),
            SourcePreparationOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
        }
    }

    fn event_texts(batch: &EventBatch) -> Vec<&str> {
        batch
            .events()
            .iter()
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
            })
            .collect()
    }

    #[test]
    fn loading_anchor_identity_equality_and_opacity_are_exact() {
        let key = RootModuleLoadingAnchorKey::new(workspace());
        assert_eq!(key, RootModuleLoadingAnchorKey::new(workspace()));
        let other_workspace = NormalizedAbsolutePath::new("/other-workspace").unwrap();
        assert_ne!(key, RootModuleLoadingAnchorKey::new(other_workspace));
        assert_eq!(key.to_string(), "root-module-loading-anchor:\"/workspace\"");

        let success = |value| RootModuleLoadingAnchor {
            carrier: Arc::new(Ok(value)),
        };
        let success_a = success(empty_value());
        assert_eq!(format!("{success_a:?}"), "RootModuleLoadingAnchor");
        let equal_success_a = SourcePreparationOutcome::Complete(Arc::new(Ok(success_a)));
        let equal_success_b =
            SourcePreparationOutcome::Complete(Arc::new(Ok(success(empty_value()))));
        assert!(RootModuleLoadingAnchorKey::equality(
            &equal_success_a,
            &equal_success_b
        ));
        assert!(RootModuleLoadingAnchorKey::validity(&equal_success_a));
        let mut different_value = empty_value();
        different_value.module_file_paths = Arc::from([PathBuf::from("different")]);
        let unequal_success =
            SourcePreparationOutcome::Complete(Arc::new(Ok(success(different_value))));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &equal_success_a,
            &unequal_success
        ));
        let private_error = HostRootModuleFileError::CommandPolicy {
            message: "PRIVATE_DIAGNOSTIC".into(),
        };
        let expected_display = private_error.to_string();
        let error = |message: &str| RootModuleLoadingAnchorError {
            carrier: Arc::new(Err(HostRootModuleFileError::CommandPolicy {
                message: message.into(),
            })),
        };
        let error_a = error("PRIVATE_DIAGNOSTIC");
        assert_eq!(format!("{error_a:?}"), "RootModuleLoadingAnchorError");
        assert!(!format!("{error_a:?}").contains("PRIVATE_DIAGNOSTIC"));
        assert_eq!(error_a.to_string(), expected_display);
        assert!(error_a.source().is_none());
        let equal_error_a = SourcePreparationOutcome::Complete(Arc::new(Err(error_a)));
        let equal_error_b =
            SourcePreparationOutcome::Complete(Arc::new(Err(error("PRIVATE_DIAGNOSTIC"))));
        assert!(RootModuleLoadingAnchorKey::equality(
            &equal_error_a,
            &equal_error_b
        ));
        assert!(RootModuleLoadingAnchorKey::validity(&equal_error_a));
        let unequal_error =
            SourcePreparationOutcome::Complete(Arc::new(Err(error("DIFFERENT_DIAGNOSTIC"))));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &equal_error_a,
            &unequal_error
        ));
    }

    #[tokio::test]
    async fn repository_route_maps_direct_alias_and_rejects_unknown_without_legacy_graph() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let source = "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n";
        let route = observed_route(&dice, EpochBuilder::root(source, 1).build(), "dep_alias").await;
        let SourcePreparationOutcome::Complete(route) = route else {
            panic!("complete root module returned Need");
        };
        let route = route.as_ref().as_ref().unwrap();
        assert_eq!(route.apparent_repo().as_str(), "dep_alias");
        assert_eq!(route.module_name(), "dep");
        assert_eq!(route.canonical_repo().as_str(), "dep+");
        assert_eq!(route.workspace(), &workspace());
        let hash = |route: &super::RootRepositoryRoute| {
            let mut state = DefaultHasher::new();
            route.hash(&mut state);
            state.finish()
        };
        let mut changed_spec = route.clone();
        let RootRepositorySource::DirectLocal(spec) = &mut changed_spec.source else {
            panic!("test route is direct local");
        };
        Arc::make_mut(&mut spec.attributes).insert(
            "path".into(),
            crate::OverrideAttributeValue::String("other-dep".into()),
        );
        assert_ne!(route, &changed_spec);
        assert_ne!(hash(route), hash(&changed_spec));

        let capability = route.source_capability();
        assert_eq!(capability.workspace(), route.workspace());
        assert_eq!(capability.apparent_repo(), route.apparent_repo());
        assert_eq!(capability.canonical_repo(), route.canonical_repo());
        let crate::HostRepositorySourceCapabilitySource::RepoSpec {
            repo_spec: spec,
            local_path_policy,
        } = capability.source()
        else {
            panic!("direct-local route must project its RepoSpec");
        };
        assert_eq!(spec.as_ref(), route.repo_spec());
        assert_eq!(capability.repo_spec(), Some(route.repo_spec()));
        assert_eq!(
            *local_path_policy,
            crate::HostRepositoryLocalPathPolicy::WorkspaceRelative
        );
        let cloned = capability.clone();
        let crate::HostRepositorySourceCapabilitySource::RepoSpec {
            repo_spec: cloned_spec,
            ..
        } = cloned.source()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(spec, cloned_spec));
        let capability_hash = |value: &crate::HostRepositorySourceCapability| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        assert_eq!(capability_hash(&capability), capability_hash(&cloned));
        assert_eq!(
            external_source_capability_snapshot(route),
            (
                "dep_alias".into(),
                "dep+".into(),
                true,
                Some(crate::HostRepositoryLocalPathPolicy::WorkspaceRelative)
            )
        );

        let builtin =
            observed_route(&dice, EpochBuilder::root(source, 1).build(), "bazel_tools").await;
        let SourcePreparationOutcome::Complete(builtin) = builtin else {
            panic!("complete root module returned Need");
        };
        let builtin = builtin.as_ref().as_ref().unwrap();
        assert_eq!(builtin.canonical_repo().as_str(), "bazel_tools");
        assert_eq!(builtin.module_name(), "bazel_tools");
        let RootRepositorySource::BuiltinBazelTools(identity) = builtin.source() else {
            panic!("reserved route must use built-in source identity");
        };
        assert_eq!(
            identity.snapshot(),
            crate::BuiltinBazelToolsSnapshot::Bazel9_2
        );
        let builtin_capability = builtin.source_capability();
        assert!(matches!(
            builtin_capability.source(),
            crate::HostRepositorySourceCapabilitySource::Builtin(projected)
                if projected == identity
        ));
        assert_eq!(builtin_capability.apparent_repo().as_str(), "bazel_tools");
        assert_eq!(builtin_capability.canonical_repo().as_str(), "bazel_tools");
        for (apparent, canonical) in [
            (ApparentRepoName::root(), route.canonical_repo().clone()),
            (route.apparent_repo().clone(), CanonicalRepoName::root()),
            (
                route.apparent_repo().clone(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ),
        ] {
            assert!(
                crate::HostRepositorySourceCapability::from_repo_spec(
                    workspace(),
                    apparent,
                    canonical,
                    route.repo_spec(),
                    crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
                )
                .is_none()
            );
        }
        let apparent_builtin_spec = crate::HostRepositorySourceCapability::from_repo_spec(
            workspace(),
            ApparentRepoName::new("bazel_tools").unwrap(),
            route.canonical_repo().clone(),
            route.repo_spec(),
            crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
        )
        .unwrap();
        assert!(matches!(
            apparent_builtin_spec.source(),
            crate::HostRepositorySourceCapabilitySource::RepoSpec { .. }
        ));
        for (apparent, canonical) in [
            (
                ApparentRepoName::root(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ),
            (
                ApparentRepoName::new("bazel_tools").unwrap(),
                CanonicalRepoName::root(),
            ),
            (
                route.apparent_repo().clone(),
                CanonicalRepoName::new("bazel_tools").unwrap(),
            ),
            (
                ApparentRepoName::new("bazel_tools").unwrap(),
                route.canonical_repo().clone(),
            ),
        ] {
            assert!(
                crate::HostRepositorySourceCapability::builtin(
                    workspace(),
                    apparent,
                    canonical,
                    identity.clone(),
                )
                .is_none()
            );
        }

        let variants = [
            crate::HostRepositorySourceCapability::from_repo_spec(
                NormalizedAbsolutePath::new("/other-workspace").unwrap(),
                route.apparent_repo().clone(),
                route.canonical_repo().clone(),
                route.repo_spec(),
                crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
            )
            .unwrap(),
            crate::HostRepositorySourceCapability::from_repo_spec(
                workspace(),
                ApparentRepoName::new("other").unwrap(),
                route.canonical_repo().clone(),
                route.repo_spec(),
                crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
            )
            .unwrap(),
            crate::HostRepositorySourceCapability::from_repo_spec(
                workspace(),
                route.apparent_repo().clone(),
                CanonicalRepoName::new("other+").unwrap(),
                route.repo_spec(),
                crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
            )
            .unwrap(),
            crate::HostRepositorySourceCapability::from_repo_spec(
                workspace(),
                route.apparent_repo().clone(),
                route.canonical_repo().clone(),
                changed_spec.repo_spec(),
                crate::HostRepositoryLocalPathPolicy::WorkspaceRelative,
            )
            .unwrap(),
            crate::HostRepositorySourceCapability::from_repo_spec(
                workspace(),
                route.apparent_repo().clone(),
                route.canonical_repo().clone(),
                route.repo_spec(),
                crate::HostRepositoryLocalPathPolicy::CommandAbsolute,
            )
            .unwrap(),
            apparent_builtin_spec,
            builtin_capability.clone(),
        ];
        for variant in variants {
            assert_ne!(capability, variant);
            assert_ne!(capability_hash(&capability), capability_hash(&variant));
            assert_eq!(capability, route.source_capability());
            assert_eq!(
                capability_hash(&capability),
                capability_hash(&route.source_capability())
            );
        }
        assert_eq!(
            hex::encode(identity.manifest_sha256()),
            "f999235edbaf1c8c0a46c4ac8a1e370f8f1eb6ea122c23905dc34ee8890e3a0a"
        );

        let unknown = observed_route(&dice, EpochBuilder::root(source, 1).build(), "missing").await;
        let SourcePreparationOutcome::Complete(unknown) = unknown else {
            panic!("complete root module returned Need");
        };
        assert_eq!(
            unknown.as_ref().as_ref().unwrap_err().to_string(),
            "no such package '@@[unknown repo 'missing' requested from @@]//': The repository '@@[unknown repo 'missing' requested from @@]' could not be resolved: No repository visible as '@missing' from main repository"
        );

        let nodep = observed_route(
            &dice,
            EpochBuilder::root(
                "module(name = \"root\")\nbazel_dep(name = \"nodep_dep\", version = \"1.0.0\", repo_name = None)\nlocal_path_override(module_name = \"nodep_dep\", path = \"dep\")\n",
                2,
            )
            .build(),
            "nodep_dep",
        )
        .await;
        let SourcePreparationOutcome::Complete(nodep) = nodep else {
            panic!("complete root module returned Need");
        };
        assert!(nodep.as_ref().is_err());

        let unsupported = observed_route(
            &dice,
            EpochBuilder::root(
                "module(name = \"root\")\nbazel_dep(name = \"registry_dep\", version = \"1.0.0\")\n",
                3,
            )
            .build(),
            "registry_dep",
        )
        .await;
        let SourcePreparationOutcome::Complete(unsupported) = unsupported else {
            panic!("complete root module returned Need");
        };
        assert!(
            unsupported
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("is not a direct local_path_override")
        );
    }

    #[test]
    fn root_build_route_admission_discriminates_key_hash_and_display() {
        let apparent = ApparentRepoName::new("dep").unwrap();
        let ordinary = RootRepositoryRouteKey::new(workspace(), apparent.clone()).unwrap();
        let admitted =
            RootRepositoryRouteKey::for_root_build(workspace(), apparent.clone()).unwrap();
        let hash = |key: &RootRepositoryRouteKey| {
            let mut state = DefaultHasher::new();
            key.hash(&mut state);
            state.finish()
        };
        assert_ne!(ordinary, admitted);
        assert_ne!(hash(&ordinary), hash(&admitted));
        assert_eq!(
            ordinary.to_string(),
            "root-repository-route:\"/workspace\":@dep"
        );
        assert_eq!(
            admitted.to_string(),
            "root-build-repository-route:\"/workspace\":@dep"
        );

        let ordinary_observed =
            RootRepositoryRouteObservationKey::new(workspace(), apparent.clone()).unwrap();
        let admitted_observed =
            RootRepositoryRouteObservationKey::for_root_build(workspace(), apparent).unwrap();
        assert_ne!(ordinary_observed, admitted_observed);
        assert_eq!(
            ordinary_observed.to_string(),
            "observed-root-repository-route:\"/workspace\":@dep"
        );
        assert_eq!(
            admitted_observed.to_string(),
            "observed-root-build-repository-route:\"/workspace\":@dep"
        );
    }

    #[test]
    fn observed_route_projection_preserves_arcs_and_terminal_polarity() {
        let key =
            RootRepositoryRouteKey::new(workspace(), ApparentRepoName::new("bazel_tools").unwrap())
                .unwrap();
        let demand = EpochBuilder::demand("/", PathObservationOperation::Lstat);
        let shared = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), shared.dupe())]).unwrap();
        let projected = project_observed_root_repository_route(
            &key,
            SourcePreparationOutcome::Complete(Ok(ObservedHostRootModuleFile {
                result: Arc::new(Ok(empty_value())),
                observations: epoch,
            })),
        );
        let SourcePreparationOutcome::Complete(Ok(projected)) = projected else {
            panic!("observed route projection must complete");
        };
        assert!(
            projected
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .is_builtin_bazel_tools()
        );
        assert!(Arc::ptr_eq(
            projected.observations().get(&demand).unwrap(),
            &shared
        ));
        let held = projected.result().dupe();
        let cloned = projected.dupe();
        assert!(Arc::ptr_eq(&held, cloned.result()));

        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(demand.dupe()),
        ));
        assert!(matches!(
            project_observed_root_repository_route(&key, need),
            SourcePreparationOutcome::Need(_)
        ));
        let outer =
            ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(demand));
        let projected = project_observed_root_repository_route(
            &key,
            SourcePreparationOutcome::Complete(Err(outer.dupe())),
        );
        let SourcePreparationOutcome::Complete(Err(projected)) = projected else {
            panic!("observed route outer error must remain outer");
        };
        assert_eq!(projected, RootRepositoryRouteObservationError::Path(outer));
    }

    #[tokio::test]
    async fn observed_route_reuses_module_family_events_and_recovers_across_edits() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let source = |name: &str| {
            format!(
                "print('ROUTE_{name}')\nmodule(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"{name}\")\n"
            )
        };
        let a = EpochBuilder::root(source("a"), 41).build();
        let mut cancelled = observed_route_transaction(&dice, a.dupe(), Some(tracker.dupe())).await;
        let key = observed_route_key("dep");
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        drop(cancelled);
        assert!(tracker.take().is_empty());
        let first =
            observed_route_frontier(&dice, a.dupe(), "dep", Some(tracker.dupe()), true).await;
        assert!(RootRepositoryRouteObservationKey::validity(&first));
        let first_route = complete_observed_route(&first);
        for (demand, expected) in a.observations() {
            assert!(Arc::ptr_eq(
                first_route.observations().get(demand).unwrap(),
                expected
            ));
        }
        let entries = tracker.take();
        let module_entries = entries.iter().filter(|entry| {
            entry
                .key
                .starts_with("bzlmod-observed-host-root-module-file:")
                && entry.batch.is_some()
        });
        assert_eq!(module_entries.clone().count(), 1);
        assert!(entries.iter().any(|entry| {
            entry
                .key
                .starts_with("observed-root-module-loading-anchor:")
        }));
        let module_entry = module_entries.into_iter().next().unwrap();
        assert_eq!(
            event_texts(module_entry.batch.as_ref().unwrap()),
            ["ROUTE_a"]
        );
        assert!(entries.iter().all(|entry| {
            !entry.key.starts_with("root-repository-route:")
                && !entry.key.starts_with("host-root-module-file:")
        }));

        let warm =
            observed_route_frontier(&dice, a.dupe(), "dep", Some(tracker.dupe()), true).await;
        assert!(RootRepositoryRouteObservationKey::equality(&first, &warm));
        assert!(Arc::ptr_eq(
            complete_observed_route(&first).result(),
            complete_observed_route(&warm).result()
        ));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));

        let changed = observed_route_frontier(
            &dice,
            EpochBuilder::root(source("b"), 42).build(),
            "dep",
            Some(tracker.dupe()),
            false,
        )
        .await;
        assert!(!RootRepositoryRouteObservationKey::equality(
            &first, &changed
        ));
        tracker.take();
        let restored = observed_route_frontier(&dice, a, "dep", Some(tracker.dupe()), false).await;
        assert!(RootRepositoryRouteObservationKey::equality(
            &first, &restored
        ));

        let need = observed_route_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            PathObservationEpoch::empty(),
            "dep",
            None,
            false,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!RootRepositoryRouteObservationKey::validity(&need));
        assert!(!RootRepositoryRouteObservationKey::equality(&need, &need));

        for (source, apparent, message) in [
            (
                "module(name = \"root\")\n",
                "missing",
                "could not be resolved",
            ),
            (
                "module(name = \"root\")\nbazel_dep(name = \"registry_dep\", version = \"1.0.0\")\n",
                "registry_dep",
                "is not a direct local_path_override",
            ),
            ("this is not valid module syntax", "missing", "syntax"),
        ] {
            let outcome = observed_route_frontier(
                &Dice::builder().build(DetectCycles::Enabled),
                EpochBuilder::root(source, 51).build(),
                apparent,
                None,
                false,
            )
            .await;
            let route = complete_observed_route(&outcome);
            let error = route.result().as_ref().as_ref().unwrap_err();
            assert!(error.to_string().contains(message));
        }
    }

    #[tokio::test]
    async fn builtin_route_root_edit_restore_keeps_immutable_source_identity() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let route = |value: <RootRepositoryRouteKey as Key>::Value| {
            let SourcePreparationOutcome::Complete(value) = value else {
                panic!("complete root module returned Need");
            };
            value.as_ref().as_ref().unwrap().clone()
        };
        let first = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"first\")\n", 1).build(),
                "bazel_tools",
            )
            .await,
        );
        let middle = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"middle\")\n", 2).build(),
                "bazel_tools",
            )
            .await,
        );
        let restored = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"first\")\n", 3).build(),
                "bazel_tools",
            )
            .await,
        );
        assert_eq!(first.source(), middle.source());
        assert_eq!(first, restored);

        let other_workspace = RootRepositoryRoute::builtin_for_test(
            NormalizedAbsolutePath::new("/other-workspace").unwrap(),
        );
        assert_ne!(first, other_workspace);
        assert_eq!(first.source(), other_workspace.source());
    }

    #[tokio::test]
    async fn loading_anchor_retained_dice_lifecycle_and_event_closure_are_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let source = |reversed: bool| {
            if reversed {
                "print('ANCHOR_EVENT')\nregister_execution_platforms('//:second', '//:first')\n"
            } else {
                "print('ANCHOR_EVENT')\nregister_execution_platforms('//:first', '//:second')\n"
            }
        };

        let first_path = observed_anchor(&dice, EpochBuilder::default().build(), &tracker).await;
        let SourcePreparationOutcome::Need(first_path_need) = &first_path else {
            panic!("expected first-path Need");
        };
        assert_eq!(
            first_path_need.path_observations().unwrap().demands()[0]
                .path()
                .as_path(),
            std::path::Path::new("/")
        );
        assert!(first_path_need.root_module_bootstrap_request().is_none());
        assert!(first_path_need.repository_materializations().is_empty());
        assert!(!RootModuleLoadingAnchorKey::validity(&first_path));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &first_path,
            &first_path
        ));
        assert!(tracker.take().iter().all(|entry| {
            (!entry.key.starts_with("root-module-loading-anchor:")
                && !entry.key.starts_with("host-root-module-file:"))
                || entry.batch.is_none()
        }));
        tracker.take_anchor_dependencies();

        let mut missing = EpochBuilder::default();
        missing.directory("/", 1);
        missing.directory("/workspace", 1);
        missing.missing("/workspace/MODULE.bazel");
        let bootstrap = observed_anchor(&dice, missing.build(), &tracker).await;
        let SourcePreparationOutcome::Need(bootstrap_need) = &bootstrap else {
            panic!("expected root-bootstrap Need");
        };
        assert!(bootstrap_need.path_observations().is_none());
        assert!(bootstrap_need.root_module_bootstrap_request().is_some());
        assert!(bootstrap_need.repository_materializations().is_empty());
        assert!(tracker.take().iter().all(|entry| {
            (!entry.key.starts_with("root-module-loading-anchor:")
                && !entry.key.starts_with("host-root-module-file:"))
                || entry.batch.is_none()
        }));
        tracker.take_anchor_dependencies();

        let success = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 2).build(),
            &tracker,
        )
        .await;
        assert!(matches!(success, SourcePreparationOutcome::Complete(ref value) if value.is_ok()));
        let SourcePreparationOutcome::Complete(success_value) = &success else {
            unreachable!()
        };
        assert_eq!(
            success_value
                .as_ref()
                .as_ref()
                .unwrap()
                .registrations()
                .execution_platforms()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["//:first", "//:second"]
        );
        let success_events = tracker.take();
        let wrapper = success_events
            .iter()
            .find(|entry| entry.key.starts_with("root-module-loading-anchor:"))
            .unwrap();
        assert!(wrapper.batch.is_none());
        let producer = success_events
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .unwrap();
        assert_eq!(
            event_texts(producer.batch.as_ref().unwrap()),
            ["ANCHOR_EVENT"]
        );
        assert_eq!(
            tracker.take_anchor_dependencies(),
            vec![vec![String::from("host-root-module-file:\"/workspace\"")]]
        );

        let reversed =
            observed_anchor(&dice, EpochBuilder::root(source(true), 3).build(), &tracker).await;
        assert!(!RootModuleLoadingAnchorKey::equality(&success, &reversed));
        let SourcePreparationOutcome::Complete(reversed_value) = &reversed else {
            unreachable!()
        };
        assert_eq!(
            reversed_value
                .as_ref()
                .as_ref()
                .unwrap()
                .registrations()
                .execution_platforms()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["//:second", "//:first"]
        );
        tracker.take();
        tracker.take_anchor_dependencies();

        let error = observed_anchor(
            &dice,
            EpochBuilder::root("unknown_identifier\n", 4).build(),
            &tracker,
        )
        .await;
        assert!(matches!(error, SourcePreparationOutcome::Complete(ref value) if value.is_err()));
        tracker.take();
        tracker.take_anchor_dependencies();

        let restored = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 5).build(),
            &tracker,
        )
        .await;
        assert!(RootModuleLoadingAnchorKey::equality(&success, &restored));
        tracker.take();
        tracker.take_anchor_dependencies();

        let warm = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 5).build(),
            &tracker,
        )
        .await;
        assert!(RootModuleLoadingAnchorKey::equality(&restored, &warm));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    }

    #[test]
    fn key_is_workspace_identified_and_equality_is_complete_only() {
        assert_eq!(
            HostRootModuleFileKey::new(workspace()),
            HostRootModuleFileKey::new(workspace())
        );

        let complete = SourcePreparationOutcome::Complete(std::sync::Arc::new(Ok(empty_value())));
        assert!(HostRootModuleFileKey::equality(&complete, &complete));
        assert!(HostRootModuleFileKey::validity(&complete));

        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(slug_workspace_v2::PathObservationDemand::new(
                slug_workspace_v2::PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
                slug_workspace_v2::PathObservationOperation::Lstat,
            )),
        ));
        assert!(!HostRootModuleFileKey::equality(&need, &need));
        assert!(!HostRootModuleFileKey::validity(&need));
    }

    #[tokio::test]
    async fn missing_root_is_the_sole_bootstrap_need() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut epoch = EpochBuilder::default();
        epoch.directory("/", 1);
        epoch.directory("/workspace", 1);
        epoch.missing("/workspace/MODULE.bazel");
        let outcome = observed(
            &dice,
            epoch.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = outcome else {
            panic!("expected bootstrap Need");
        };
        assert_eq!(
            need.root_module_bootstrap_request()
                .unwrap()
                .module_path()
                .as_path(),
            std::path::Path::new("/workspace/MODULE.bazel")
        );
        assert!(need.path_observations().is_none());
        assert!(need.repository_materializations().is_empty());
    }

    #[tokio::test]
    async fn root_observation_needs_accumulate_and_root_lifecycle_recovers() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let stages = [
            EpochBuilder::default(),
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch
            },
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch.directory("/workspace", 1);
                epoch
            },
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch.directory("/workspace", 1);
                epoch.node("/workspace/MODULE.bazel", PathNodeKind::RegularFile, 1);
                epoch
            },
        ];
        let expected = [
            ("/", PathObservationOperation::Lstat),
            ("/workspace", PathObservationOperation::Lstat),
            ("/workspace/MODULE.bazel", PathObservationOperation::Lstat),
            (
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
        ];
        for (epoch, (path, operation)) in stages.into_iter().zip(expected) {
            let outcome = observed(
                &dice,
                epoch.build(),
                &["/workspace"],
                false,
                None,
                None,
                LockfileMode::Update,
                None,
            )
            .await;
            let SourcePreparationOutcome::Need(need) = outcome else {
                panic!("expected cumulative path Need");
            };
            let demands = need.path_observations().unwrap().demands();
            assert_eq!(demands.len(), 1);
            assert_eq!(demands[0].path().as_path(), std::path::Path::new(path));
            assert_eq!(demands[0].operation(), operation);
        }

        let a = observed(
            &dice,
            EpochBuilder::root("module(name='a')\n", 2).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a).module.header.as_ref().unwrap().name, "a");
        let b = observed(
            &dice,
            EpochBuilder::root("module(name='b')\n", 3).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&b).module.header.as_ref().unwrap().name, "b");

        let mut deleted = EpochBuilder::default();
        deleted.directory("/", 4);
        deleted.directory("/workspace", 4);
        deleted.missing("/workspace/MODULE.bazel");
        let deleted = observed(
            &dice,
            deleted.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Need(need)
                if need.root_module_bootstrap_request().is_some()
        ));

        let restored = observed(
            &dice,
            EpochBuilder::root("module(name='a')\n", 5).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&restored));
    }

    #[tokio::test]
    async fn root_and_next_horizon_prepare_failures_block_later_dependencies() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let root_failure =
            EpochBuilder::root("include('//pkg:a.MODULE.bazel')\nunknown_identifier\n", 1);
        let outcome = observed(
            &dice,
            root_failure.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::RootValidation { message, .. })
                        if message.contains("unknown_identifier")
                )
        ));
        assert!(tracker.take().iter().any(|entry| {
            entry.key.starts_with("host-root-module-file:")
                && entry
                    .batch
                    .as_ref()
                    .is_some_and(|batch| batch.events().is_empty())
        }));

        let mut horizon_failure = EpochBuilder::root("include('//pkg:a.MODULE.bazel')\n", 2);
        horizon_failure.repository_policy(&["/workspace"], 2);
        horizon_failure.package("/workspace", "pkg", 2);
        horizon_failure.file(
            "/workspace/pkg/a.MODULE.bazel",
            "include('//next:n.MODULE.bazel')\nunknown_identifier\n",
            2,
        );
        let outcome = observed(
            &dice,
            horizon_failure.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation {
                        raw_label,
                        location,
                        message,
                        ..
                    }) if raw_label == "//pkg:a.MODULE.bazel"
                        && location.start_line == 1
                        && message.contains("unknown_identifier")
                )
        ));

        let tracker = Arc::new(EventTracker::default());
        let mut full_closure_failure = EpochBuilder::root(
            "\
print('ROOT_PREFIX')
fail('earlier runtime failure')
include('//pkg:bad.MODULE.bazel')
",
            3,
        );
        full_closure_failure.repository_policy(&["/workspace"], 3);
        full_closure_failure.package("/workspace", "pkg", 3);
        full_closure_failure.file("/workspace/pkg/bad.MODULE.bazel", "unknown_identifier\n", 3);
        let outcome = observed(
            &dice,
            full_closure_failure.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation { raw_label, .. })
                        if raw_label == "//pkg:bad.MODULE.bazel"
                )
        ));
        let root_batch = tracker
            .take()
            .into_iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch)
            .unwrap();
        assert!(root_batch.events().is_empty());
    }

    #[tokio::test]
    async fn first_source_utf8_prepare_and_evaluation_failures_retain_include_context() {
        let root = "\
print('ROOT_PREFIX')
include('//a:a.MODULE.bazel')
include('//b:b.MODULE.bazel')
";
        let base = |variant| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.repository_policy(&["/workspace"], variant);
            epoch.package("/workspace", "a", variant);
            epoch.package("/workspace", "b", variant);
            epoch
        };

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut utf8 = base(1);
        utf8.file("/workspace/a/a.MODULE.bazel", [0xff], 1);
        let outcome = observed(
            &dice,
            utf8.build(),
            &["/workspace"],
            true,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation {
                        raw_label,
                        location,
                        message,
                        ..
                    }) if raw_label == "//a:a.MODULE.bazel"
                        && location.start_line == 2
                        && message.contains("UTF-8")
                )
        ));

        let tracker = Arc::new(EventTracker::default());
        let mut evaluation = base(2);
        evaluation.file(
            "/workspace/a/a.MODULE.bazel",
            "print('A_PREFIX')\nfail('A_FAIL')\n",
            2,
        );
        evaluation.file(
            "/workspace/b/b.MODULE.bazel",
            "print('B_MUST_NOT_RUN')\nfail('B_FAIL')\n",
            2,
        );
        let outcome = observed(
            &dice,
            evaluation.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Complete(value) = &outcome else {
            panic!("expected terminal evaluation failure");
        };
        let HostRootModuleFileError::Evaluation {
            message,
            include_occurrences,
        } = value.as_ref().as_ref().unwrap_err()
        else {
            panic!("expected evaluation failure: {value:?}");
        };
        assert!(message.contains("A_FAIL"), "{message}");
        assert_eq!(include_occurrences[0].path, "//a:a.MODULE.bazel");
        assert_eq!(include_occurrences[0].location.start_line, 2);
        assert_eq!(include_occurrences[1].path, "//b:b.MODULE.bazel");
        assert_eq!(include_occurrences[1].location.start_line, 3);
        let root_batch = tracker
            .take()
            .into_iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch)
            .unwrap();
        assert_eq!(event_texts(&root_batch), ["ROOT_PREFIX", "A_PREFIX"]);
    }

    #[tokio::test]
    async fn grouped_byte_needs_union_and_obey_source_order_terminal_precedence() {
        let root = "include('//a:a.MODULE.bazel')\ninclude('//b:b.MODULE.bazel')\n";
        let base = |variant| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.repository_policy(&["/workspace"], variant);
            epoch.package("/workspace", "a", variant);
            epoch.package("/workspace", "b", variant);
            epoch
        };

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let package_tracker = Arc::new(EventTracker::default());
        let mut package_barrier = EpochBuilder::root(root, 0);
        package_barrier.repository_policy(&["/workspace"], 0);
        package_barrier.directory("/workspace/a", 0);
        package_barrier.missing("/workspace/a/BUILD.bazel");
        package_barrier.missing("/workspace/a/BUILD");
        let outcome = observed(
            &dice,
            package_barrier.build(),
            &["/workspace"],
            false,
            Some(package_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludePreflight { error })
                        if matches!(
                            error,
                            crate::host_include::HostRootIncludeError::Package {
                                raw_label,
                                ..
                            } if raw_label == "//a:a.MODULE.bazel"
                        )
                )
        ));
        let package_activations = package_tracker.take();
        assert!(package_activations.iter().all(|entry| {
            !entry.key.contains("/workspace/a/a.MODULE.bazel")
                && !entry.key.contains("/workspace/b/b.MODULE.bazel")
        }));
        assert!(
            package_activations
                .iter()
                .any(|entry| { entry.key.contains("/workspace/MODULE.bazel") })
        );

        let outcome = observed(
            &dice,
            base(1).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = outcome else {
            panic!("expected grouped byte Need");
        };
        let demands = need.path_observations().unwrap().demands();
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/a/a.MODULE.bazel")
        }));
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/b/b.MODULE.bazel")
        }));

        let mut terminal_first = base(2);
        terminal_first.missing("/workspace/a/a.MODULE.bazel");
        let outcome = observed(
            &dice,
            terminal_first.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeMissing { raw_label, .. })
                        if raw_label == "//a:a.MODULE.bazel"
                )
        ));

        let mut need_first = base(3);
        need_first.missing("/workspace/b/b.MODULE.bazel");
        let outcome = observed(
            &dice,
            need_first.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
    }

    #[tokio::test]
    async fn occurrence_execution_and_path_set_collapse_are_distinct_from_dependency_dedupe() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let mut epoch = EpochBuilder::root(
            "\
include('//pkg:sub/x.MODULE.bazel')
include('//pkg/sub:x.MODULE.bazel')
include('//pkg:sub/x.MODULE.bazel')
",
            1,
        );
        epoch.repository_policy(&["/workspace"], 1);
        epoch.package("/workspace", "pkg", 1);
        epoch.package("/workspace", "pkg/sub", 1);
        epoch.package("/workspace", "nested", 1);
        epoch.file(
            "/workspace/pkg/sub/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nprint('X')\n",
            1,
        );
        epoch.file("/workspace/nested/n.MODULE.bazel", "print('NESTED')\n", 1);
        clear_validated_root_module_logical_ids();
        let outcome = observed(
            &dice,
            epoch.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(
            take_validated_root_module_logical_ids()
                .iter()
                .map(|logical_id| logical_id.0.as_str())
                .collect::<Vec<_>>(),
            [
                "/workspace/MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
            ],
            "compile validation preserves every include occurrence and horizon order"
        );
        let value = complete_value(&outcome);
        assert_eq!(
            value.module_file_paths.as_ref(),
            [
                PathBuf::from("MODULE.bazel"),
                PathBuf::from("nested/n.MODULE.bazel"),
                PathBuf::from("pkg/sub/x.MODULE.bazel")
            ]
        );
        let entries = tracker.take();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.key.contains("pkg/sub/x.MODULE.bazel"))
                .count(),
            1,
            "one grouped HostFileBytesKey owns all three include occurrences"
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.key.contains("nested/n.MODULE.bazel"))
                .count(),
            1,
            "the duplicated nested horizon also shares one HostFileBytesKey"
        );
        let root = entries
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(
            event_texts(root),
            ["NESTED", "X", "NESTED", "X", "NESTED", "X"]
        );

        let mut reordered = EpochBuilder::root(
            "\
include('//pkg/sub:x.MODULE.bazel')
include('//pkg:sub/x.MODULE.bazel')
include('//pkg/sub:x.MODULE.bazel')
",
            2,
        );
        reordered.repository_policy(&["/workspace"], 2);
        reordered.package("/workspace", "pkg", 2);
        reordered.package("/workspace", "pkg/sub", 2);
        reordered.package("/workspace", "nested", 2);
        reordered.file(
            "/workspace/pkg/sub/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nprint('X')\n",
            2,
        );
        reordered.file("/workspace/nested/n.MODULE.bazel", "print('NESTED')\n", 2);
        let reordered = observed(
            &dice,
            reordered.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&outcome), complete_value(&reordered));

        let marker_off_tracker = Arc::new(EventTracker::default());
        let mut epoch = EpochBuilder::root("print('DIRECT')\n", 3);
        epoch.repository_policy(&["/workspace"], 3);
        let outcome = observed(
            &Dice::builder().build(DetectCycles::Enabled),
            epoch.build(),
            &["/workspace"],
            false,
            Some(marker_off_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&outcome);
        assert!(
            marker_off_tracker
                .take()
                .iter()
                .all(|entry| entry.batch.is_none())
        );
    }

    #[tokio::test]
    async fn repo_child_and_root_include_events_keep_separate_membership_across_need_retry() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let scenario = |include_bytes: bool, variant: i64| {
            let mut epoch = EpochBuilder::root(
                "\
print('ROOT_BEFORE')
include('//pkg:x.MODULE.bazel')
print('ROOT_AFTER')
",
                variant,
            );
            epoch.directory("/workspace/pkg", variant);
            epoch.file(
                "/workspace/REPO.bazel",
                "print('REPO')\nignore_directories([])\n",
                variant,
            );
            epoch.missing("/workspace/.bazelignore");
            epoch.package("/workspace", "pkg", variant);
            if include_bytes {
                epoch.file(
                    "/workspace/pkg/x.MODULE.bazel",
                    "print('INCLUDE')\n",
                    variant,
                );
            }
            epoch
        };

        let need = observed(
            &dice,
            scenario(false, 1).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let first_attempt = tracker.take();
        let repo_batch = first_attempt
            .iter()
            .find(|entry| entry.key.starts_with("host-repo-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(event_texts(repo_batch), ["REPO"]);
        assert!(first_attempt.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:") || entry.batch.is_none()
        }));

        let complete = observed(
            &dice,
            scenario(true, 2).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&complete);
        let retry = tracker.take();
        let root_batch = retry
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(
            event_texts(root_batch),
            ["ROOT_BEFORE", "INCLUDE", "ROOT_AFTER"]
        );
        assert!(retry.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:")
                || entry
                    .batch
                    .as_ref()
                    .is_some_and(|batch| !event_texts(batch).contains(&"REPO"))
        }));

        let warm = observed(
            &dice,
            scenario(true, 2).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&warm);
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));

        let terminal_dice = Dice::builder().build(DetectCycles::Enabled);
        let terminal_tracker = Arc::new(EventTracker::default());
        let mut terminal = EpochBuilder::root("include('//missing:x.MODULE.bazel')\n", 3);
        terminal.file(
            "/workspace/REPO.bazel",
            "print('REPO_TERMINAL')\nignore_directories([])\n",
            3,
        );
        terminal.missing("/workspace/.bazelignore");
        terminal.directory("/workspace/missing", 3);
        terminal.missing("/workspace/missing/BUILD.bazel");
        terminal.missing("/workspace/missing/BUILD");
        let outcome = observed(
            &terminal_dice,
            terminal.build(),
            &["/workspace"],
            true,
            Some(terminal_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludePreflight { .. })
                )
        ));
        let terminal_events = terminal_tracker.take();
        let repo_batch = terminal_events
            .iter()
            .find(|entry| entry.key.starts_with("host-repo-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(event_texts(repo_batch), ["REPO_TERMINAL"]);
        let root_batch = terminal_events
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert!(root_batch.events().is_empty());
        assert!(
            terminal_events
                .iter()
                .all(|entry| !entry.key.contains("/workspace/missing/x.MODULE.bazel"))
        );
    }

    #[tokio::test]
    async fn alternate_special_include_and_root_include_lifecycle_recover_on_retained_dice() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = "include('//pkg:x.MODULE.bazel')\n";
        let scenario = |source: Option<&str>, variant: i64| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.directory("/alternate", variant);
            epoch.repository_policy(&["/alternate"], variant);
            epoch.package("/alternate", "pkg", variant);
            match source {
                Some(source) => {
                    epoch.special("/alternate/pkg/x.MODULE.bazel", source, variant);
                }
                None => epoch.missing("/alternate/pkg/x.MODULE.bazel"),
            }
            epoch
        };
        let a = observed(
            &dice,
            scenario(Some("bazel_dep(name='a', version='1.0')\n"), 1).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a).module.dependencies[0].name.as_str(), "a");
        assert_eq!(
            complete_value(&a).module_file_paths.as_ref(),
            [
                PathBuf::from("MODULE.bazel"),
                PathBuf::from("pkg/x.MODULE.bazel")
            ]
        );

        let mut workspace_selected = EpochBuilder::root(root, 10);
        workspace_selected.repository_policy(&["/workspace"], 10);
        workspace_selected.package("/workspace", "pkg", 10);
        workspace_selected.file(
            "/workspace/pkg/x.MODULE.bazel",
            "bazel_dep(name='a', version='1.0')\n",
            10,
        );
        let workspace_dice = Dice::builder().build(DetectCycles::Enabled);
        let workspace_selected = observed(
            &workspace_dice,
            workspace_selected.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&workspace_selected));

        let b = observed(
            &dice,
            scenario(Some("bazel_dep(name='b', version='2.0')\n"), 2).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&b).module.dependencies[0].name.as_str(), "b");

        let deleted = observed(
            &dice,
            scenario(None, 3).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeMissing { .. })
                )
        ));

        let restored = observed(
            &dice,
            scenario(Some("bazel_dep(name='a', version='1.0')\n"), 4).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&restored));

        let mut invalid = EpochBuilder::root(root, 5);
        invalid.directory("/alternate", 5);
        invalid.repository_policy(&["/alternate"], 5);
        invalid.package("/alternate", "pkg", 5);
        invalid.special(
            "/alternate/pkg/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nunknown_identifier\n",
            5,
        );
        let invalid = observed(
            &dice,
            invalid.build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        match invalid {
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Err(HostRootModuleFileError::IncludeValidation {
                    raw_label,
                    logical_path,
                    message,
                    ..
                }) => {
                    assert_eq!(raw_label.as_str(), "//pkg:x.MODULE.bazel");
                    assert_eq!(
                        logical_path.as_path(),
                        std::path::Path::new("/alternate/pkg/x.MODULE.bazel")
                    );
                    assert!(message.contains("/alternate/pkg/x.MODULE.bazel"));
                    assert!(message.contains("unknown_identifier"));
                }
                other => panic!("expected alternate-root include validation error, got {other:?}"),
            },
            SourcePreparationOutcome::Need(need) => {
                panic!("validation must precede nested include preflight, got {need:?}")
            }
        }
    }

    #[tokio::test]
    async fn lockfile_environment_and_mode_changes_are_not_root_dependencies() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(DependencyTracker::default());
        let epoch = || {
            let mut epoch = EpochBuilder::root("module(name='root')\n", 1);
            epoch.repository_policy(&["/workspace"], 1);
            epoch.build()
        };
        let a = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            Some("{\"lockFileVersion\":28,\"facts\":{\"a\":{}}}\n"),
        )
        .await;
        let b = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            Some("all"),
            LockfileMode::Off,
            Some("{\"lockFileVersion\":28,\"facts\":{\"b\":{}}}\n"),
        )
        .await;
        let restored = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Error,
            Some("{\"lockFileVersion\":28,\"facts\":{\"a\":{}}}\n"),
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&b));
        assert_eq!(complete_value(&a), complete_value(&restored));

        let dependency_dice = Dice::builder().build(DetectCycles::Enabled);
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut updater = dependency_dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch())])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            LockfileMode::Refresh,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootModuleFileKey::new(workspace()))
            .await
            .unwrap();
        let dependencies = tracker.take();
        assert!(
            dependencies
                .iter()
                .any(|dependency| dependency.starts_with("bzlmod-host-file-bytes:")),
            "the tracker observed the Host root key's direct dependency set"
        );
        assert!(dependencies.iter().all(|dependency| {
            !dependency.starts_with("visible-lockfile:")
                && !dependency.starts_with("root-module-environment-policy:")
                && !dependency.starts_with("root-module-lockfile-mode:")
                && !dependency.starts_with("root-module-graph:")
        }));
    }
    #[tokio::test]
    async fn active_ancestry_cycles_are_typed_at_alias_back_edges() {
        struct Scenario {
            root: &'static str,
            packages: &'static [&'static str],
            files: &'static [(&'static str, &'static str)],
            expected_label: &'static str,
            expected_location_file: &'static str,
            expected_path: &'static str,
        }

        let scenarios = [
            Scenario {
                root: "include('//pkg:sub/x.MODULE.bazel')\n",
                packages: &["pkg", "pkg/sub"],
                files: &[(
                    "/workspace/pkg/sub/x.MODULE.bazel",
                    "include('//pkg/sub:x.MODULE.bazel')\n",
                )],
                expected_label: "//pkg/sub:x.MODULE.bazel",
                expected_location_file: "/workspace/pkg/sub/x.MODULE.bazel",
                expected_path: "/workspace/pkg/sub/x.MODULE.bazel",
            },
            Scenario {
                root: "include('//a:a.MODULE.bazel')\n",
                packages: &["a", "b"],
                files: &[
                    (
                        "/workspace/a/a.MODULE.bazel",
                        "include('//b:b.MODULE.bazel')\n",
                    ),
                    (
                        "/workspace/b/b.MODULE.bazel",
                        "include('//a:a.MODULE.bazel')\n",
                    ),
                ],
                expected_label: "//a:a.MODULE.bazel",
                expected_location_file: "/workspace/b/b.MODULE.bazel",
                expected_path: "/workspace/a/a.MODULE.bazel",
            },
        ];

        for (index, scenario) in scenarios.into_iter().enumerate() {
            let variant = i64::try_from(index).unwrap() + 1;
            let mut epoch = EpochBuilder::root(scenario.root, variant);
            epoch.repository_policy(&["/workspace"], variant);
            for package in scenario.packages {
                epoch.package("/workspace", package, variant);
            }
            for (path, source) in scenario.files {
                epoch.file(path, source, variant);
            }
            let tracker = Arc::new(EventTracker::default());
            let outcome = observed(
                &Dice::builder().build(DetectCycles::Enabled),
                epoch.build(),
                &["/workspace"],
                true,
                Some(tracker.dupe()),
                None,
                LockfileMode::Update,
                None,
            )
            .await;
            let SourcePreparationOutcome::Complete(value) = &outcome else {
                panic!("a finite include cycle must complete");
            };
            let Err(HostRootModuleFileError::IncludeCycle {
                raw_label,
                location,
                logical_path,
            }) = value.as_ref()
            else {
                panic!("expected typed include cycle, got {value:?}");
            };
            assert_eq!(raw_label.as_str(), scenario.expected_label);
            assert_eq!(location.file.0.as_str(), scenario.expected_location_file);
            assert_eq!((location.start_line, location.start_column), (1, 1));
            assert_eq!(
                logical_path.as_path(),
                std::path::Path::new(scenario.expected_path)
            );
            let entries = tracker.take();
            let parent = entries
                .iter()
                .find(|entry| entry.key.starts_with("host-root-module-file:"))
                .expect("the completed parent activation is recorded");
            assert!(matches!(
                parent.batch.as_ref(),
                Some(batch) if batch.events().is_empty()
            ));
        }
    }

    #[tokio::test]
    async fn include_cycle_need_error_and_recovery_order_are_stable() {
        let self_cycle = |source: &str, variant: i64| {
            let mut epoch = EpochBuilder::root("include('//pkg:x.MODULE.bazel')\n", variant);
            epoch.repository_policy(&["/workspace"], variant);
            epoch.package("/workspace", "pkg", variant);
            epoch.file("/workspace/pkg/x.MODULE.bazel", source, variant);
            epoch.build()
        };
        let cycle_source = "include('//pkg:x.MODULE.bazel')\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cycle_a = self_cycle(cycle_source, 10);
        let first = observed(
            &dice,
            cycle_a.dupe(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let warm = observed(
            &dice,
            cycle_a,
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let acyclic = observed(
            &dice,
            self_cycle("print('OK')\n", 11),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let restored = observed(
            &dice,
            self_cycle(cycle_source, 12),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(HostRootModuleFileKey::equality(&first, &warm));
        assert!(!HostRootModuleFileKey::equality(&first, &acyclic));
        assert!(HostRootModuleFileKey::equality(&first, &restored));

        let mut earlier_error = EpochBuilder::root("include('//pkg:x.MODULE.bazel')\n", 20);
        earlier_error.repository_policy(&["/workspace"], 20);
        earlier_error.package("/workspace", "pkg", 20);
        earlier_error.directory("/workspace/missing", 20);
        earlier_error.missing("/workspace/missing/BUILD.bazel");
        earlier_error.missing("/workspace/missing/BUILD");
        earlier_error.file(
            "/workspace/pkg/x.MODULE.bazel",
            "include('//missing:y.MODULE.bazel')\ninclude('//pkg:x.MODULE.bazel')\n",
            20,
        );
        let outcome = observed(
            &Dice::builder().build(DetectCycles::Enabled),
            earlier_error.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludePreflight {
                        error: crate::host_include::HostRootIncludeError::Package {
                            raw_label,
                            ..
                        }
                    }) if raw_label == "//missing:y.MODULE.bazel"
                )
        ));

        let mut later_need = EpochBuilder::root("include('//pkg:x.MODULE.bazel')\n", 21);
        later_need.repository_policy(&["/workspace"], 21);
        later_need.package("/workspace", "pkg", 21);
        later_need.file(
            "/workspace/pkg/x.MODULE.bazel",
            "include('//pkg:x.MODULE.bazel')\ninclude('//need:y.MODULE.bazel')\n",
            21,
        );
        let tracker = Arc::new(EventTracker::default());
        let outcome = observed(
            &Dice::builder().build(DetectCycles::Enabled),
            later_need.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
        assert!(tracker.take().iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:") || entry.batch.is_none()
        }));
    }
    #[tokio::test]
    async fn observed_frontier_matches_legacy_events_and_retains_exact_input_arcs() {
        let mut script = EpochBuilder::root(
            "print('ROOT')
include('//pkg:x.MODULE.bazel')
",
            1,
        );
        script.repository_policy(&["/workspace"], 1);
        script.package("/workspace", "pkg", 1);
        script.file(
            "/workspace/pkg/x.MODULE.bazel",
            "print('CHILD')
",
            1,
        );
        let injected = script.build();

        let legacy_tracker = Arc::new(EventTracker::default());
        let legacy = observed(
            &Dice::builder().build(DetectCycles::Enabled),
            injected.dupe(),
            &["/workspace"],
            true,
            Some(legacy_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Complete(legacy_result) = &legacy else {
            panic!("complete legacy script returned Need");
        };

        let frontier_tracker = Arc::new(EventTracker::default());
        let frontier = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            injected.dupe(),
            &["/workspace"],
            true,
            Some(frontier_tracker.dupe()),
            true,
        )
        .await;
        assert!(HostRootModuleFileObservationKey::validity(&frontier));
        assert!(HostRootModuleFileObservationKey::equality(
            &frontier, &frontier
        ));
        let frontier_value = complete_frontier(&frontier);
        assert_eq!(frontier_value.result(), legacy_result.as_ref());
        for (demand, expected) in injected.observations() {
            assert!(Arc::ptr_eq(
                expected,
                frontier_value
                    .observations()
                    .get(demand)
                    .expect("every exact Host input is retained")
            ));
        }

        let legacy_entries = legacy_tracker.take();
        let legacy_batch = legacy_entries
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        let frontier_entries = frontier_tracker.take();
        let frontier_batch = frontier_entries
            .iter()
            .find(|entry| {
                entry
                    .key
                    .starts_with("bzlmod-observed-host-root-module-file:")
            })
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(event_texts(frontier_batch), event_texts(legacy_batch));
        assert_eq!(event_texts(frontier_batch), ["ROOT", "CHILD"]);
        assert!(frontier_entries.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:")
                && !entry.key.starts_with("bzlmod-host-file-bytes:")
                && !entry.key.starts_with("host-root-package-lookup:")
                && !entry.key.starts_with("host-repo-file:")
        }));
    }

    #[tokio::test]
    async fn observed_frontier_root_terminals_keep_exact_completion_polarity() {
        let policy_tracker = Arc::new(EventTracker::default());
        let policy = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            PathObservationEpoch::empty(),
            &["/workspace"],
            true,
            Some(policy_tracker.dupe()),
            false,
        )
        .await;
        let policy = complete_frontier(&policy);
        assert!(matches!(
            policy.result(),
            Err(HostRootModuleFileError::CommandPolicy { .. })
        ));
        assert!(policy.observations().observations().is_empty());
        assert!(policy_tracker.take().iter().any(|entry| {
            entry
                .key
                .starts_with("bzlmod-observed-host-root-module-file:")
                && matches!(entry.batch.as_ref(), Some(batch) if batch.events().is_empty())
        }));

        let mut denied = EpochBuilder::default();
        denied.directory("/", 1);
        denied.directory("/workspace", 1);
        denied.lstat_error("/workspace/MODULE.bazel");
        let denied = denied.build();
        let outcome = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            denied.dupe(),
            &["/workspace"],
            false,
            None,
            true,
        )
        .await;
        let retained = complete_frontier(&outcome);
        assert!(matches!(
            retained.result(),
            Err(HostRootModuleFileError::RootFile { .. })
        ));
        for (demand, expected) in denied.observations() {
            assert!(Arc::ptr_eq(
                expected,
                retained.observations().get(demand).unwrap()
            ));
        }

        let invalid = EpochBuilder::root(
            "unknown_identifier
",
            2,
        )
        .build();
        let outcome = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            invalid.dupe(),
            &["/workspace"],
            false,
            None,
            true,
        )
        .await;
        let retained = complete_frontier(&outcome);
        assert!(matches!(
            retained.result(),
            Err(HostRootModuleFileError::RootValidation { .. })
        ));
        assert_eq!(
            retained.observations().observations().len(),
            invalid.observations().len()
        );

        let mut missing = EpochBuilder::default();
        missing.directory("/", 3);
        missing.directory("/workspace", 3);
        missing.missing("/workspace/MODULE.bazel");
        let tracker = Arc::new(EventTracker::default());
        let need = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            missing.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            true,
        )
        .await;
        assert!(matches!(
            need,
            SourcePreparationOutcome::Need(ref needs)
                if needs.root_module_bootstrap_request().is_some()
        ));
        assert!(!HostRootModuleFileObservationKey::validity(&need));
        assert!(!HostRootModuleFileObservationKey::equality(&need, &need));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    }

    #[tokio::test]
    async fn observed_frontier_excludes_speculative_later_files_and_groups_need() {
        let root = "include('//a:a.MODULE.bazel')
include('//b:b.MODULE.bazel')
";
        let base = |variant| {
            let mut script = EpochBuilder::root(root, variant);
            script.repository_policy(&["/workspace"], variant);
            script.package("/workspace", "a", variant);
            script.package("/workspace", "b", variant);
            script
        };

        let mut terminal = base(1);
        terminal.missing("/workspace/a/a.MODULE.bazel");
        terminal.file(
            "/workspace/b/b.MODULE.bazel",
            "print('LATE')
",
            1,
        );
        let terminal = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            terminal.build(),
            &["/workspace"],
            true,
            None,
            true,
        )
        .await;
        let terminal = complete_frontier(&terminal);
        assert!(matches!(
            terminal.result(),
            Err(HostRootModuleFileError::IncludeMissing { raw_label, .. })
                if raw_label == "//a:a.MODULE.bazel"
        ));
        assert!(terminal.observations().observations().keys().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/a/a.MODULE.bazel")
        }));
        assert!(terminal.observations().observations().keys().all(|demand| {
            demand.path().as_path() != std::path::Path::new("/workspace/b/b.MODULE.bazel")
        }));

        let tracker = Arc::new(EventTracker::default());
        let need = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            base(2).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            true,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = need else {
            panic!("unobserved include files must return Need");
        };
        let demands = need.path_observations().unwrap().demands();
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/a/a.MODULE.bazel")
        }));
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/b/b.MODULE.bazel")
        }));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    }

    #[tokio::test]
    async fn observed_frontier_cycles_and_a_b_a_restore_structural_identity() {
        let cycle = |variant| {
            let mut script = EpochBuilder::root(
                "include('//pkg:x.MODULE.bazel')
",
                variant,
            );
            script.repository_policy(&["/workspace"], variant);
            script.package("/workspace", "pkg", variant);
            script.file(
                "/workspace/pkg/x.MODULE.bazel",
                "include('//pkg:x.MODULE.bazel')
",
                variant,
            );
            script.build()
        };
        let a = cycle(10);
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let first = observed_frontier(&dice, a.dupe(), &["/workspace"], false, None, true).await;
        let warm = observed_frontier(&dice, a.dupe(), &["/workspace"], false, None, true).await;
        let changed = observed_frontier(
            &dice,
            EpochBuilder::root(
                "module(name='changed')
",
                11,
            )
            .build(),
            &["/workspace"],
            false,
            None,
            true,
        )
        .await;
        let restored = observed_frontier(&dice, a, &["/workspace"], false, None, true).await;
        assert!(matches!(
            complete_frontier(&first).result(),
            Err(HostRootModuleFileError::IncludeCycle { logical_path, .. })
                if logical_path.as_path()
                    == std::path::Path::new("/workspace/pkg/x.MODULE.bazel")
        ));
        assert!(HostRootModuleFileObservationKey::equality(&first, &warm));
        assert!(!HostRootModuleFileObservationKey::equality(
            &first, &changed
        ));
        assert!(HostRootModuleFileObservationKey::equality(
            &first, &restored
        ));

        let mut indirect = EpochBuilder::root(
            "include('//a:a.MODULE.bazel')
",
            20,
        );
        indirect.repository_policy(&["/workspace"], 20);
        indirect.package("/workspace", "a", 20);
        indirect.package("/workspace", "b", 20);
        indirect.file(
            "/workspace/a/a.MODULE.bazel",
            "include('//b:b.MODULE.bazel')
",
            20,
        );
        indirect.file(
            "/workspace/b/b.MODULE.bazel",
            "include('//a:a.MODULE.bazel')
",
            20,
        );
        let indirect = observed_frontier(
            &Dice::builder().build(DetectCycles::Enabled),
            indirect.build(),
            &["/workspace"],
            false,
            None,
            true,
        )
        .await;
        let indirect = complete_frontier(&indirect);
        assert!(matches!(
            indirect.result(),
            Err(HostRootModuleFileError::IncludeCycle { logical_path, .. })
                if logical_path.as_path()
                    == std::path::Path::new("/workspace/a/a.MODULE.bazel")
        ));
        for path in ["/workspace/a/a.MODULE.bazel", "/workspace/b/b.MODULE.bazel"] {
            assert!(indirect.observations().observations().keys().any(|demand| {
                demand.path().as_path() == std::path::Path::new(path)
                    && demand.operation() == PathObservationOperation::FileBytes
            }));
        }
    }
    #[test]
    fn observed_anchor_projection_preserves_arcs_and_outer_error_polarity() {
        let demand = EpochBuilder::demand("/", PathObservationOperation::Lstat);
        let shared = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), shared.dupe())]).unwrap();
        let success: Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>> =
            Arc::new(Ok(empty_value()));
        let projected = project_observed_root_module_loading_anchor(
            SourcePreparationOutcome::Complete(Ok(ObservedHostRootModuleFile {
                result: success.dupe(),
                observations: epoch,
            })),
        );
        let SourcePreparationOutcome::Complete(Ok(projected)) = projected else {
            panic!("semantic success must produce an observed anchor");
        };
        let anchor = projected.result().as_ref().unwrap();
        assert!(Arc::ptr_eq(&anchor.carrier, &success));
        assert!(Arc::ptr_eq(
            projected.observations().get(&demand).unwrap(),
            &shared
        ));

        let failure: Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>> =
            Arc::new(Err(HostRootModuleFileError::CommandPolicy {
                message: "policy".into(),
            }));
        let projected = project_observed_root_module_loading_anchor(
            SourcePreparationOutcome::Complete(Ok(ObservedHostRootModuleFile {
                result: failure.dupe(),
                observations: PathObservationEpoch::empty(),
            })),
        );
        let SourcePreparationOutcome::Complete(Ok(projected)) = projected else {
            panic!("semantic failure must produce an observed anchor error");
        };
        assert!(Arc::ptr_eq(
            &projected.result().as_ref().unwrap_err().carrier,
            &failure
        ));

        let outer =
            ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(demand));
        let projected = project_observed_root_module_loading_anchor(
            SourcePreparationOutcome::Complete(Err(outer.dupe())),
        );
        let SourcePreparationOutcome::Complete(Err(projected)) = projected else {
            panic!("outer frontier error must remain outer");
        };
        assert_eq!(projected, outer);
        let projected = SourcePreparationOutcome::Complete(Err(projected));
        assert!(RootModuleLoadingAnchorObservationKey::validity(&projected));
        assert_eq!(
            RootModuleLoadingAnchorObservationKey::new(workspace()).to_string(),
            "observed-root-module-loading-anchor:\"/workspace\""
        );
    }

    #[tokio::test]
    async fn observed_anchor_retains_frontier_events_and_a_b_a_without_legacy_activation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let source = |reversed: bool| {
            if reversed {
                "print('OBSERVED_ANCHOR')\nregister_execution_platforms('//:second', '//:first')\n"
            } else {
                "print('OBSERVED_ANCHOR')\nregister_execution_platforms('//:first', '//:second')\n"
            }
        };
        let a = EpochBuilder::root(source(false), 31).build();
        let first = observed_frontier_anchor(&dice, a.dupe(), &tracker).await;
        assert!(RootModuleLoadingAnchorObservationKey::validity(&first));
        let SourcePreparationOutcome::Complete(Ok(frontier)) = &first else {
            panic!("observed anchor must complete");
        };
        assert_eq!(
            frontier
                .result()
                .as_ref()
                .unwrap()
                .registrations()
                .execution_platforms()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["//:first", "//:second"]
        );
        for (demand, expected) in a.observations() {
            assert!(Arc::ptr_eq(
                frontier.observations().get(demand).unwrap(),
                expected
            ));
        }
        let entries = tracker.take();
        assert!(entries.iter().any(|entry| {
            entry
                .key
                .starts_with("observed-root-module-loading-anchor:")
                && entry.batch.is_none()
        }));
        assert!(entries.iter().any(|entry| {
            entry
                .key
                .starts_with("bzlmod-observed-host-root-module-file:")
                && matches!(
                    entry.batch.as_ref().map(event_texts),
                    Some(texts) if texts == ["OBSERVED_ANCHOR"]
                )
        }));
        assert!(entries.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:")
                && !entry.key.starts_with("root-module-loading-anchor:")
        }));
        assert_eq!(
            tracker.take_anchor_dependencies(),
            vec![vec![String::from(
                "bzlmod-observed-host-root-module-file:\"/workspace\""
            )]]
        );

        let warm = observed_frontier_anchor(&dice, a.dupe(), &tracker).await;
        assert!(RootModuleLoadingAnchorObservationKey::equality(
            &first, &warm
        ));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
        tracker.take_anchor_dependencies();

        let changed = observed_frontier_anchor(
            &dice,
            EpochBuilder::root(source(true), 32).build(),
            &tracker,
        )
        .await;
        assert!(!RootModuleLoadingAnchorObservationKey::equality(
            &first, &changed
        ));
        tracker.take();
        tracker.take_anchor_dependencies();

        let restored = observed_frontier_anchor(&dice, a, &tracker).await;
        assert!(RootModuleLoadingAnchorObservationKey::equality(
            &first, &restored
        ));
    }

    #[tokio::test]
    async fn observed_anchor_need_is_invalid_and_retains_no_parent_event() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let path_need =
            observed_frontier_anchor(&dice, PathObservationEpoch::empty(), &tracker).await;
        assert!(matches!(path_need, SourcePreparationOutcome::Need(_)));
        assert!(!RootModuleLoadingAnchorObservationKey::validity(&path_need));
        assert!(!RootModuleLoadingAnchorObservationKey::equality(
            &path_need, &path_need
        ));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
        tracker.take_anchor_dependencies();

        let mut missing = EpochBuilder::default();
        missing.directory("/", 1);
        missing.directory("/workspace", 1);
        missing.missing("/workspace/MODULE.bazel");
        let bootstrap = observed_frontier_anchor(&dice, missing.build(), &tracker).await;
        let SourcePreparationOutcome::Need(bootstrap) = bootstrap else {
            panic!("missing root module must request bootstrap");
        };
        assert!(bootstrap.root_module_bootstrap_request().is_some());
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    }

    #[tokio::test]
    async fn root_files_parent_is_eventless_family_isolated_and_restores_a_b_a() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let source = |value: &str| {
            format!(
                "module(name='root')\nprint('{value}')\np=use_extension('//:{value}.bzl','{value}')\nuse_repo(p, rust_toolchains='rust_toolchains')"
            )
        };
        let epoch = |value: &str, variant| {
            let mut epoch = EpochBuilder::root(source(value), variant);
            epoch.missing("/workspace/MODULE.bazel.lock");
            epoch.build()
        };

        let first = observed_root_files(&dice, epoch("a", 1), &tracker).await;
        let SourcePreparationOutcome::Complete(Ok(first)) = first else {
            panic!("root files did not complete: {first:?}");
        };
        let held = first.result().dupe();
        let dependencies = tracker.take_anchor_dependencies();
        assert!(dependencies.iter().any(|row| {
            row.iter()
                .any(|key| key.starts_with("bzlmod-observed-host-root-module-file:"))
                && row
                    .iter()
                    .any(|key| key.starts_with("bzlmod-observed-host-file-bytes:"))
                && row.iter().all(|key| {
                    !key.starts_with("root-module-evaluation:")
                        && !key.starts_with("visible-lockfile:")
                })
        }));
        let cold = tracker.take();
        assert!(cold.iter().any(|entry| {
            entry
                .key
                .starts_with("bzlmod-observed-host-root-module-file:")
                && entry
                    .batch
                    .as_ref()
                    .is_some_and(|batch| !batch.events().is_empty())
        }));
        assert!(cold.iter().any(|entry| {
            entry.key.starts_with("observed-root-module-files:") && entry.batch.is_none()
        }));

        observed_root_files(&dice, epoch("a", 1), &tracker).await;
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
        tracker.take_anchor_dependencies();

        let changed = observed_root_files(&dice, epoch("b", 2), &tracker).await;
        assert!(!RootModuleFilesObservationKey::equality(
            &SourcePreparationOutcome::Complete(Ok(first.clone())),
            &changed
        ));
        tracker.take();
        tracker.take_anchor_dependencies();

        let restored = observed_root_files(&dice, epoch("a", 3), &tracker).await;
        let SourcePreparationOutcome::Complete(Ok(restored)) = &restored else {
            panic!("restored root files did not complete: {restored:?}");
        };
        assert_eq!(held.as_ref(), restored.result().as_ref());
        assert_eq!(
            held.as_ref().as_ref().unwrap().extension_usages[0].extension_name,
            "a"
        );
    }
}
