/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::Demand;
use dice::DiceComputations;
use dice::DiceDataBuilder;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetName;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::PathResult;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::BuiltinBazelToolsSnapshot;
use crate::EvaluatedNonrootModule;
use crate::ModuleKey;
use crate::NonrootModuleKey;
use crate::OverrideAttributeValue;
use crate::RegistryBaseUrl;
use crate::RegistryFileError;
use crate::RegistryFileKey;
use crate::RegistryFileUrl;
use crate::RegistryFileValue;
use crate::RegistryPolicyKey;
use crate::RepoSpec;
use crate::RootModuleBootstrapRequest;
use crate::RootModuleFilesKey;
use crate::RootModuleOverride;
use crate::RootRepositoryRoute;
use crate::apply_unified_patch;
use crate::builtin_repository::BuiltinBazelToolsModuleError;
use crate::builtin_repository::BuiltinBazelToolsModuleKey;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupError;
use crate::host_package::ExternalRepositoryPackageLookupKey;
use crate::host_package::HostBuildFileName;
use crate::host_package::invalid_package_name;
use crate::module_eval::DirectNonregistryEvaluationError;
use crate::module_eval::DirectNonregistryIncludeFile;
use crate::module_eval::NonrootIncludeRequest;
use crate::module_eval::NonrootModuleFileInspection;
use crate::module_eval::evaluate_direct_nonregistry_module_closure_with_events;
use crate::module_eval::parse_root_include;
use crate::module_eval::validate_root_module_source;
use crate::package_policy::CanonicalDeletedPackagesProjectionKey;
use crate::registry_module_file_url;
use crate::repository_ignore::HostNonregistryRepositoryIgnoreKey;
use crate::repository_ignore::HostRepositoryIgnoreError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
}

/// A bzlmod computation either completed or needs outside-DICE observations
/// and/or repository materialization. Needs are deliberately transient: they
/// are neither valid nor equal at DICE boundaries.
#[derive(Debug, Clone, Allocative, Dupe)]
pub enum SourcePreparationOutcome<T> {
    Complete(T),
    Need(SourcePreparationNeeds),
}

pub type SourcePreparationResult<T, E> = SourcePreparationOutcome<Result<T, E>>;

impl<T> SourcePreparationOutcome<T> {
    fn path_need(need: NeedPathObservations) -> Self {
        Self::Need(SourcePreparationNeeds::path(need))
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    pub fn complete_eq(&self, other: &Self) -> bool
    where
        T: PartialEq,
    {
        matches!((self, other), (Self::Complete(left), Self::Complete(right)) if left == right)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> SourcePreparationOutcome<U> {
        match self {
            Self::Complete(value) => SourcePreparationOutcome::Complete(f(value)),
            Self::Need(need) => SourcePreparationOutcome::Need(need),
        }
    }
}

fn source_outcome_from_path<T>(outcome: PathOutcome<T>) -> SourcePreparationOutcome<T> {
    match outcome {
        PathOutcome::Complete(value) => SourcePreparationOutcome::Complete(value),
        PathOutcome::Need(need) => SourcePreparationOutcome::path_need(need),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SourcePreparationNeeds {
    root_module_bootstrap: Option<RootModuleBootstrapRequest>,
    path_observations: Option<NeedPathObservations>,
    repository_materializations:
        Arc<SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum SourcePreparationNeedsError {
    ConflictingRootModuleBootstrap,
    ConflictingRepositoryRequest { canonical_repo: CanonicalRepoName },
}

impl SourcePreparationNeeds {
    pub fn root_module_bootstrap(request: RootModuleBootstrapRequest) -> Self {
        Self {
            root_module_bootstrap: Some(request),
            path_observations: None,
            repository_materializations: Arc::new(SmallMap::new()),
        }
    }

    pub fn path(need: NeedPathObservations) -> Self {
        Self {
            root_module_bootstrap: None,
            path_observations: Some(need),
            repository_materializations: Arc::new(SmallMap::new()),
        }
    }

    pub fn repository(request: RepositoryMaterializationRequest) -> Self {
        let mut repository_materializations = SmallMap::new();
        repository_materializations.insert(request.id.clone(), Arc::new(request));
        Self {
            root_module_bootstrap: None,
            path_observations: None,
            repository_materializations: Arc::new(repository_materializations),
        }
    }

    pub fn root_module_bootstrap_request(&self) -> Option<&RootModuleBootstrapRequest> {
        self.root_module_bootstrap.as_ref()
    }

    pub fn path_observations(&self) -> Option<&NeedPathObservations> {
        self.path_observations.as_ref()
    }

    pub fn repository_materializations(
        &self,
    ) -> &SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>> {
        &self.repository_materializations
    }

    pub fn try_union(&self, other: &Self) -> Result<Self, SourcePreparationNeedsError> {
        let path_observations = match (&self.path_observations, &other.path_observations) {
            (Some(left), Some(right)) => Some(left.union(right)),
            (Some(need), None) | (None, Some(need)) => Some(need.dupe()),
            (None, None) => None,
        };
        let mut requests = (*self.repository_materializations).clone();
        for (request_id, request) in other.repository_materializations.iter() {
            match requests.get(request_id) {
                Some(existing) if existing == request => {}
                Some(_) => {
                    return Err(SourcePreparationNeedsError::ConflictingRepositoryRequest {
                        canonical_repo: request_id.canonical_repo.clone(),
                    });
                }
                None => {
                    requests.insert(request_id.clone(), request.dupe());
                }
            }
        }
        let root_module_bootstrap =
            match (&self.root_module_bootstrap, &other.root_module_bootstrap) {
                (Some(left), Some(right)) if left == right => Some(left.dupe()),
                (Some(_), Some(_)) => {
                    return Err(SourcePreparationNeedsError::ConflictingRootModuleBootstrap);
                }
                (Some(request), None) | (None, Some(request)) => Some(request.dupe()),
                (None, None) => None,
            };
        debug_assert!(
            root_module_bootstrap.is_some() || path_observations.is_some() || !requests.is_empty()
        );
        Ok(Self {
            root_module_bootstrap,
            path_observations,
            repository_materializations: Arc::new(requests),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationRequestId {
    pub workspace: NormalizedAbsolutePath,
    pub canonical_repo: CanonicalRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationKind {
    Local {
        logical_root: NormalizedAbsolutePath,
    },
    Immutable,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryMaterializationRequest {
    pub id: RepositoryMaterializationRequestId,
    pub repo_spec: RepoSpec,
    pub kind: RepositoryMaterializationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationSuccess {
    Local,
    Immutable {
        source_identity: Arc<str>,
        generation_root: PathBuf,
        observation_instance: PathObservationInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationResult {
    Success(RepositoryMaterializationSuccess),
    SpecError(CompactString),
    TransportError {
        generation: RepositoryMaterializationGeneration,
        message: CompactString,
    },
    MaterializationError {
        generation: RepositoryMaterializationGeneration,
        message: CompactString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryMaterializationEpochEntry {
    pub request: Arc<RepositoryMaterializationRequest>,
    pub result: RepositoryMaterializationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryMaterializationResultEpoch {
    workspace: NormalizedAbsolutePath,
    entries: Arc<SmallMap<CanonicalRepoName, RepositoryMaterializationEpochEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationResultEpochError {
    WrongWorkspace { canonical_repo: CanonicalRepoName },
    DuplicateRepository { canonical_repo: CanonicalRepoName },
    ConflictingRepositoryRequest { canonical_repo: CanonicalRepoName },
    SuccessKindMismatch { canonical_repo: CanonicalRepoName },
}

impl RepositoryMaterializationResultEpoch {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        entries: impl IntoIterator<Item = RepositoryMaterializationEpochEntry>,
    ) -> Result<Self, RepositoryMaterializationResultEpochError> {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.request
                .id
                .canonical_repo
                .cmp(&right.request.id.canonical_repo)
        });
        let mut result: SmallMap<CanonicalRepoName, RepositoryMaterializationEpochEntry> =
            SmallMap::with_capacity(entries.len());
        for entry in entries {
            let canonical_repo = entry.request.id.canonical_repo.clone();
            if entry.request.id.workspace != workspace {
                return Err(RepositoryMaterializationResultEpochError::WrongWorkspace {
                    canonical_repo,
                });
            }
            if !result_kind_matches(&entry.request.kind, &entry.result) {
                return Err(
                    RepositoryMaterializationResultEpochError::SuccessKindMismatch {
                        canonical_repo,
                    },
                );
            }
            if let Some(existing) = result.get(&canonical_repo) {
                return Err(if existing.request == entry.request {
                    RepositoryMaterializationResultEpochError::DuplicateRepository {
                        canonical_repo,
                    }
                } else {
                    RepositoryMaterializationResultEpochError::ConflictingRepositoryRequest {
                        canonical_repo,
                    }
                });
            }
            result.insert(canonical_repo, entry);
        }
        Ok(Self {
            workspace,
            entries: Arc::new(result),
        })
    }

    fn get(
        &self,
        request: &RepositoryMaterializationRequest,
    ) -> Option<&RepositoryMaterializationEpochEntry> {
        (self.workspace == request.id.workspace)
            .then(|| self.entries.get(&request.id.canonical_repo))
            .flatten()
    }
}

fn result_kind_matches(
    kind: &RepositoryMaterializationKind,
    result: &RepositoryMaterializationResult,
) -> bool {
    matches!(
        (kind, result),
        (_, RepositoryMaterializationResult::SpecError(_))
            | (_, RepositoryMaterializationResult::TransportError { .. })
            | (
                _,
                RepositoryMaterializationResult::MaterializationError { .. }
            )
            | (
                RepositoryMaterializationKind::Local { .. },
                RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local)
            )
            | (
                RepositoryMaterializationKind::Immutable,
                RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable { .. }
                )
            )
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationResultEpochKey {
    pub workspace: NormalizedAbsolutePath,
}

impl fmt::Display for RepositoryMaterializationResultEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-materialization-results:{}", self.workspace)
    }
}

impl InjectedKey for RepositoryMaterializationResultEpochKey {
    type Value = RepositoryMaterializationResultEpoch;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// Prepares one module's raw MODULE.bazel bytes. `version` is already the
/// effective version chosen by the upstream owner; this key never resolves or
/// rewrites it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleSourcePreparationKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
    pub version: CompactString,
}

impl fmt::Display for ModuleSourcePreparationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "module-source-preparation:{}@{}",
            self.module_name, self.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleSourcePreparation {
    NonRegistry {
        bytes: Arc<[u8]>,
    },
    Registry {
        bytes: Arc<[u8]>,
        selected_registry: RegistryBaseUrl,
        module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistryModuleFileAttempt {
    pub url: RegistryFileUrl,
    pub sha256: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleSourcePreparationError {
    RootModuleFiles(CompactString),
    RegistryPolicy(RegistryFileError),
    RegistryFileCompute {
        url: RegistryFileUrl,
        prior_not_found_attempts: Arc<[RegistryModuleFileAttempt]>,
        message: CompactString,
    },
    RegistryFile {
        url: RegistryFileUrl,
        prior_not_found_attempts: Arc<[RegistryModuleFileAttempt]>,
        error: RegistryFileError,
    },
    RegistryPolicyCompute(CompactString),
    Source(RepositorySourceFileError),
    SourceCompute(Arc<str>),
    InvalidPatchPath {
        path: PathBuf,
    },
    PatchMissing {
        logical_path: NormalizedAbsolutePath,
    },
    PatchWrongKind {
        logical_path: NormalizedAbsolutePath,
        actual: PathNodeKind,
    },
    PatchResolution(PathResolutionError),
    PatchResolutionCompute {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    PatchFileObservation {
        demand: PathObservationDemand,
        error: PathObservationError,
    },
    PatchFileInconsistentState {
        demand: PathObservationDemand,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    PatchFileCompute {
        demand: PathObservationDemand,
        message: CompactString,
    },
    Patch(CompactString),
    MissingVersion,
    ModuleNotFound {
        module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    },
}
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostDiscoveredModuleProvenance {
    BuiltinBazelTools {
        route_identity: BuiltinBazelToolsRouteIdentity,
        module_sha256: [u8; 32],
    },
    Registry {
        selected_registry: RegistryBaseUrl,
        module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostDiscoveredModule {
    pub(crate) module: EvaluatedNonrootModule,
    pub(crate) provenance: HostDiscoveredModuleProvenance,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostDiscoveredModuleError {
    RootModuleFiles(CompactString),
    ExplicitBuiltinOverride,
    InvalidBuiltinVersion { version: CompactString },
    MissingVersion { module_name: CompactString },
    SourcePreparationCompute(Arc<str>),
    SourcePreparation(ModuleSourcePreparationError),
    NonRegistryUnsupported { module_name: CompactString },
    BuiltinCompute(Arc<str>),
    Builtin(BuiltinBazelToolsModuleError),
    Evaluation(DirectNonregistryEvaluationError),
}

impl fmt::Display for HostDiscoveredModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootModuleFiles(message) => write!(f, "root MODULE files failed: {message}"),
            Self::ExplicitBuiltinOverride => {
                f.write_str("explicit bazel_tools override is not supported by Host discovery")
            }
            Self::InvalidBuiltinVersion { version } => {
                write!(
                    f,
                    "built-in bazel_tools requires the empty version, got {version}"
                )
            }
            Self::MissingVersion { module_name } => {
                write!(f, "registry module {module_name} requires a version")
            }
            Self::SourcePreparationCompute(message) => {
                write!(f, "module source preparation failed to compute: {message}")
            }
            Self::SourcePreparation(error) => write!(f, "{error:?}"),
            Self::NonRegistryUnsupported { module_name } => {
                write!(
                    f,
                    "nonregistry discovery is not supported for {module_name}"
                )
            }
            Self::BuiltinCompute(message) => {
                write!(f, "built-in MODULE failed to compute: {message}")
            }
            Self::Builtin(error) => error.fmt(f),
            Self::Evaluation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for HostDiscoveredModuleError {}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostDiscoveredModuleKey {
    workspace: NormalizedAbsolutePath,
    module: NonrootModuleKey,
}

#[allow(dead_code)]
impl HostDiscoveredModuleKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, module: NonrootModuleKey) -> Self {
        Self { workspace, module }
    }
}

impl fmt::Display for HostDiscoveredModuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-discovered-module:{}:{}@{}",
            self.workspace, self.module.name, self.module.version
        )
    }
}

fn host_discovered_module_error(
    error: HostDiscoveredModuleError,
) -> <HostDiscoveredModuleKey as Key>::Value {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

#[async_trait]
impl Key for HostDiscoveredModuleKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<HostDiscoveredModule, HostDiscoveredModuleError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.as_path().to_path_buf(),
            })
            .await
        {
            Ok(root) => root,
            Err(error) => {
                return host_discovered_module_error(HostDiscoveredModuleError::RootModuleFiles(
                    error.to_string().into(),
                ));
            }
        };
        let root = match root.as_ref() {
            Ok(root) => root,
            Err(error) => {
                return host_discovered_module_error(HostDiscoveredModuleError::RootModuleFiles(
                    error.clone(),
                ));
            }
        };
        let override_ = root.overrides.get(self.module.name.as_str());
        if self.module.name == "bazel_tools" {
            if override_.is_some() {
                return host_discovered_module_error(
                    HostDiscoveredModuleError::ExplicitBuiltinOverride,
                );
            }
            if !self.module.version.is_empty() {
                return host_discovered_module_error(
                    HostDiscoveredModuleError::InvalidBuiltinVersion {
                        version: self.module.version.clone(),
                    },
                );
            }
            let value = match ctx
                .compute(&BuiltinBazelToolsModuleKey::new(
                    BuiltinBazelToolsSnapshot::CURRENT,
                ))
                .await
            {
                Ok(value) => value,
                Err(error) => {
                    return host_discovered_module_error(
                        HostDiscoveredModuleError::BuiltinCompute(Arc::from(error.to_string())),
                    );
                }
            };
            return SourcePreparationOutcome::Complete(Arc::new(match value.as_ref() {
                Ok(value) => Ok(HostDiscoveredModule {
                    module: value.module.clone(),
                    provenance: HostDiscoveredModuleProvenance::BuiltinBazelTools {
                        route_identity: value.route_identity.clone(),
                        module_sha256: value.module_sha256,
                    },
                }),
                Err(error) => Err(HostDiscoveredModuleError::Builtin(error.clone())),
            }));
        }
        if self.module.version.is_empty() {
            return host_discovered_module_error(HostDiscoveredModuleError::MissingVersion {
                module_name: self.module.name.clone(),
            });
        }
        if matches!(override_, Some(RootModuleOverride::NonRegistry(_))) {
            return host_discovered_module_error(
                HostDiscoveredModuleError::NonRegistryUnsupported {
                    module_name: self.module.name.clone(),
                },
            );
        }
        let preparation = match ctx
            .compute(&ModuleSourcePreparationKey {
                workspace: self.workspace.as_path().to_path_buf(),
                module_name: self.module.name.clone(),
                version: self.module.version.clone(),
            })
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => value,
            Err(error) => {
                return host_discovered_module_error(
                    HostDiscoveredModuleError::SourcePreparationCompute(Arc::from(
                        error.to_string(),
                    )),
                );
            }
        };
        let preparation = match preparation.as_ref() {
            Ok(preparation) => preparation,
            Err(error) => {
                return host_discovered_module_error(HostDiscoveredModuleError::SourcePreparation(
                    error.clone(),
                ));
            }
        };
        let ModuleSourcePreparation::Registry {
            bytes,
            selected_registry,
            module_file_attempts,
        } = preparation
        else {
            return host_discovered_module_error(
                HostDiscoveredModuleError::NonRegistryUnsupported {
                    module_name: self.module.name.clone(),
                },
            );
        };
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let logical_id = crate::LogicalModuleFileId::new(format!(
            "{}modules/{}/{}/MODULE.bazel",
            selected_registry.as_str(),
            self.module.name,
            self.module.version
        ));
        let (module, events) = evaluate_direct_nonregistry_module_closure_with_events(
            self.module.clone(),
            logical_id,
            bytes.as_ref(),
            &[],
            capture_events,
        );
        let value = match module {
            Ok(module) => Ok(HostDiscoveredModule {
                module,
                provenance: HostDiscoveredModuleProvenance::Registry {
                    selected_registry: selected_registry.clone(),
                    module_file_attempts: module_file_attempts.clone(),
                },
            }),
            Err(error) => Err(HostDiscoveredModuleError::Evaluation(error)),
        };
        if capture_events {
            ctx.store_evaluation_data(events.unwrap_or_else(EventBatch::empty))
                .expect("Host discovered MODULE stores exactly one event batch");
        }
        SourcePreparationOutcome::Complete(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl fmt::Display for RepositoryMaterializationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-materialization:{}", self.module_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceScope {
    pub workspace: NormalizedAbsolutePath,
    pub module_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceFileKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
    pub repo_relative_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRepositoryPathKey {
    route: RootRepositoryRoute,
    repo_relative_path: PathBuf,
}

impl HostRepositoryPathKey {
    pub(crate) fn new(route: RootRepositoryRoute, repo_relative_path: PathBuf) -> Self {
        Self {
            route,
            repo_relative_path,
        }
    }
}

impl Hash for HostRepositoryPathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.repo_relative_path.hash(state);
    }
}

impl fmt::Display for HostRepositoryPathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-repository-path:{}:{}",
            self.route.canonical_repo(),
            self.repo_relative_path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostRepositoryPathValue(ResolvedPath);

impl HostRepositoryPathValue {
    pub(crate) fn resolved(&self) -> &ResolvedPath {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceFileKey {
    route: RootRepositoryRoute,
    repo_relative_path: PathBuf,
}

impl HostRepositorySourceFileKey {
    pub fn new(route: RootRepositoryRoute, repo_relative_path: PathBuf) -> Self {
        Self {
            route,
            repo_relative_path,
        }
    }
}

impl Hash for HostRepositorySourceFileKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.repo_relative_path.hash(state);
    }
}

impl fmt::Display for HostRepositorySourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-repository-source-file:{}:{}",
            self.route.canonical_repo(),
            self.repo_relative_path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModuleFileKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: slug_identity_v2::ApparentRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalModuleFile(RootRepositoryRoute, HostRepositorySourceFileValue);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleFileError {
    RouteCompute(Arc<str>),
    Route(crate::RootRepositoryRouteError),
    SourceCompute(Arc<str>),
    Source(RepositorySourceFileError),
}

impl DirectLocalModuleFileKey {
    fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: slug_identity_v2::ApparentRepoName,
    ) -> Result<Self, String> {
        (!apparent_repo.is_root())
            .then_some(Self {
                workspace,
                apparent_repo,
            })
            .ok_or_else(|| "direct local module file requires a nonroot apparent name".to_owned())
    }
}

impl fmt::Display for DirectLocalModuleFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-module-file:")?;
        self.workspace.fmt(f)?;
        write!(f, ":@{}", self.apparent_repo.as_str())
    }
}

#[async_trait]
impl Key for DirectLocalModuleFileKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<DirectLocalModuleFile, DirectLocalModuleFileError>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let route_key =
            crate::RootRepositoryRouteKey::new(self.workspace.dupe(), self.apparent_repo.clone())
                .expect("direct key rejects root names");
        let route = match ctx.compute(&route_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(route)) => route,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalModuleFileError::RouteCompute(Arc::from(error.to_string())),
                )));
            }
        };
        let route = match route.as_ref() {
            Ok(route) => route.clone(),
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalModuleFileError::Route(error.clone()),
                )));
            }
        };
        match ctx
            .compute(&HostRepositorySourceFileKey::new(
                route.clone(),
                PathBuf::from("MODULE.bazel"),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Ok(source))) => {
                SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleFile(
                    route, source,
                ))))
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalModuleFileError::Source(error),
                )))
            }
            Err(error) => SourcePreparationOutcome::Complete(Arc::new(Err(
                DirectLocalModuleFileError::SourceCompute(Arc::from(error.to_string())),
            ))),
        }
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModuleInspectionKey(NormalizedAbsolutePath, ApparentRepoName);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalModuleInspection(DirectLocalModuleFile, Option<NonrootModuleFileInspection>);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleInspectionError {
    InputCompute(Arc<str>),
    Input(DirectLocalModuleFileError),
    Inspection(NormalizedAbsolutePath, Arc<str>),
}

impl fmt::Display for DirectLocalModuleInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DirectLocalModuleInspectionError {}

impl DirectLocalModuleInspectionKey {
    fn new(workspace: NormalizedAbsolutePath, apparent_repo: ApparentRepoName) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self(workspace, apparent_repo))
    }
}

impl fmt::Display for DirectLocalModuleInspectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-module-inspection:")?;
        self.0.fmt(f)?;
        write!(f, ":@{}", self.1.as_str())
    }
}

#[async_trait]
impl Key for DirectLocalModuleInspectionKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalModuleInspection, DirectLocalModuleInspectionError>>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let input = match ctx
            .compute(
                &DirectLocalModuleFileKey::new(self.0.dupe(), self.1.clone())
                    .expect("direct inspection key rejects root names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(input)) => input,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalModuleInspectionError::InputCompute(Arc::from(error.to_string())),
                )));
            }
        };
        let input = match input.as_ref() {
            Ok(input) => input.clone(),
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalModuleInspectionError::Input(error.clone()),
                )));
            }
        };
        let inspection = match &input.1 {
            HostRepositorySourceFileValue::Absent => None,
            HostRepositorySourceFileValue::Present {
                bytes,
                logical_path,
            } => match crate::inspect_nonroot_module_file(
                crate::LogicalModuleFileId::new(logical_path.as_path().display().to_string()),
                bytes,
            ) {
                Ok(inspection) => Some(inspection),
                Err(error) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(
                        DirectLocalModuleInspectionError::Inspection(
                            logical_path.dupe(),
                            Arc::from(error.to_string()),
                        ),
                    )));
                }
            },
        };
        SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleInspection(
            input, inspection,
        ))))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalIncludePackageHorizonKey(NormalizedAbsolutePath, ApparentRepoName);
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalIncludePackageHorizon {
    route: RootRepositoryRoute,
    occurrences: Arc<[DirectLocalIncludePackageOccurrence]>,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalIncludePackageOccurrence {
    package: PackageIdentifier,
    target: TargetName,
    raw_label: CompactString,
    location: crate::LogicalSpan,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalIncludePackageHorizonError {
    InspectionCompute {
        message: Arc<str>,
    },
    Inspection(DirectLocalModuleInspectionError),
    BadLabel {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        message: CompactString,
    },
    Package {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        package: PackageIdentifier,
        failure: DirectLocalIncludePackageFailure,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalIncludePackageFailure {
    InvalidPackageName { message: Arc<str> },
    Deleted,
    NoBuildFile,
    Lookup(ExternalRepositoryPackageLookupError),
    LookupCompute { message: Arc<str> },
}
impl DirectLocalIncludePackageHorizonKey {
    fn new(workspace: NormalizedAbsolutePath, apparent_repo: ApparentRepoName) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self(workspace, apparent_repo))
    }
}
impl fmt::Display for DirectLocalIncludePackageHorizonKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-include-package-horizon:")?;
        self.0.fmt(f)?;
        write!(f, ":@{}", self.1.as_str())
    }
}

impl fmt::Display for DirectLocalIncludePackageHorizonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectionCompute { message } => {
                write!(
                    f,
                    "failed to compute direct-local MODULE inspection: {message}"
                )
            }
            Self::Inspection(error) => {
                write!(f, "failed to inspect direct-local MODULE: {error:?}")
            }
            Self::BadLabel {
                raw_label,
                location,
                message,
            } => write!(
                f,
                "bad include label {raw_label:?} at {}:{}:{}: {message}",
                location.file.0, location.start_line, location.start_column
            ),
            Self::Package {
                raw_label,
                location,
                package,
                failure,
            } => write!(
                f,
                "include {raw_label:?} at {}:{}:{} has package {package:?}: {failure:?}",
                location.file.0, location.start_line, location.start_column
            ),
        }
    }
}

impl std::error::Error for DirectLocalIncludePackageHorizonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::Package {
                failure: DirectLocalIncludePackageFailure::Lookup(error),
                ..
            } => Some(error),
            _ => None,
        }
    }
}

#[async_trait]
impl Key for DirectLocalIncludePackageHorizonKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalIncludePackageHorizon, DirectLocalIncludePackageHorizonError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let inspection = match ctx
            .compute(
                &DirectLocalModuleInspectionKey::new(self.0.dupe(), self.1.clone())
                    .expect("direct horizon key rejects root names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(inspection)) => inspection,
            Err(error) => {
                return direct_local_include_inspection_error(Err(Arc::from(error.to_string())));
            }
        };
        let inspection = match inspection.as_ref() {
            Ok(inspection) => inspection,
            Err(error) => {
                return direct_local_include_inspection_error(Ok(error.clone()));
            }
        };
        let route = inspection.0.0.clone();
        let requests = inspection
            .1
            .as_ref()
            .map_or(&[][..], |inspection| inspection.includes.as_ref());
        preflight_direct_local_include_package_horizon(ctx, route, requests).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

async fn preflight_direct_local_include_package_horizon(
    ctx: &mut DiceComputations<'_>,
    route: RootRepositoryRoute,
    requests: &[NonrootIncludeRequest],
) -> <DirectLocalIncludePackageHorizonKey as Key>::Value {
    let mut occurrences = Vec::with_capacity(requests.len());
    for request in requests {
        let parsed = match parse_root_include(request) {
            Ok(parsed) => parsed,
            Err(message) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    DirectLocalIncludePackageHorizonError::BadLabel {
                        raw_label: request.path.clone(),
                        location: request.location.clone(),
                        message,
                    },
                )));
            }
        };
        occurrences.push(DirectLocalIncludePackageOccurrence {
            package: PackageIdentifier::new(
                route.canonical_repo().clone(),
                parsed.package().package().clone(),
            ),
            target: parsed.target().clone(),
            raw_label: request.path.clone(),
            location: request.location.clone(),
        });
    }

    let mut unique = SmallSet::with_capacity(occurrences.len());
    for occurrence in &occurrences {
        unique.insert(occurrence.package.clone());
    }
    let computed = ctx
        .compute_join(unique, |ctx, package| {
            let route = route.clone();
            Box::pin(async move {
                let result = ctx
                    .compute(
                        &ExternalRepositoryPackageLookupKey::new(route, package.clone())
                            .expect("occurrence package uses the inspection route"),
                    )
                    .await
                    .map_err(|error| Arc::<str>::from(error.to_string()));
                (package, result)
            })
        })
        .await;
    let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
    finish_direct_local_include_package_horizon(route, occurrences, outcomes)
}

fn direct_local_include_inspection_error(
    error: Result<DirectLocalModuleInspectionError, Arc<str>>,
) -> <DirectLocalIncludePackageHorizonKey as Key>::Value {
    let error = match error {
        Ok(error) => DirectLocalIncludePackageHorizonError::Inspection(error),
        Err(message) => DirectLocalIncludePackageHorizonError::InspectionCompute { message },
    };
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

fn finish_direct_local_include_package_horizon(
    route: RootRepositoryRoute,
    occurrences: Vec<DirectLocalIncludePackageOccurrence>,
    outcomes: SmallMap<
        PackageIdentifier,
        Result<<ExternalRepositoryPackageLookupKey as Key>::Value, Arc<str>>,
    >,
) -> SourcePreparationOutcome<
    Arc<Result<DirectLocalIncludePackageHorizon, DirectLocalIncludePackageHorizonError>>,
> {
    let mut all_need: Option<SourcePreparationNeeds> = None;
    for outcome in outcomes.values() {
        if let Ok(SourcePreparationOutcome::Need(incoming)) = outcome {
            all_need = Some(match all_need {
                Some(current) => current
                    .try_union(incoming)
                    .expect("one-route package Needs cannot conflict"),
                None => incoming.dupe(),
            });
        }
    }

    for occurrence in &occurrences {
        let outcome = outcomes
            .get(&occurrence.package)
            .expect("every occurrence package was computed");
        let value = match outcome {
            Err(message) => {
                return package_horizon_error(
                    occurrence,
                    DirectLocalIncludePackageFailure::LookupCompute {
                        message: message.dupe(),
                    },
                );
            }
            Ok(SourcePreparationOutcome::Need(_)) => {
                return SourcePreparationOutcome::Need(
                    all_need.expect("the current occurrence contributed a Need"),
                );
            }
            Ok(SourcePreparationOutcome::Complete(value)) => value,
        };
        let failure = match value.as_ref() {
            Ok(ExternalRepositoryPackageLookup::Package(_)) => continue,
            Ok(ExternalRepositoryPackageLookup::InvalidPackageName { message }) => {
                DirectLocalIncludePackageFailure::InvalidPackageName {
                    message: message.dupe(),
                }
            }
            Ok(ExternalRepositoryPackageLookup::Deleted) => {
                DirectLocalIncludePackageFailure::Deleted
            }
            Ok(ExternalRepositoryPackageLookup::NoBuildFile) => {
                DirectLocalIncludePackageFailure::NoBuildFile
            }
            Err(error) => DirectLocalIncludePackageFailure::Lookup(error.clone()),
        };
        return package_horizon_error(occurrence, failure);
    }

    SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalIncludePackageHorizon {
        route,
        occurrences: occurrences.into(),
    })))
}

fn package_horizon_error(
    occurrence: &DirectLocalIncludePackageOccurrence,
    failure: DirectLocalIncludePackageFailure,
) -> SourcePreparationOutcome<
    Arc<Result<DirectLocalIncludePackageHorizon, DirectLocalIncludePackageHorizonError>>,
> {
    SourcePreparationOutcome::Complete(Arc::new(Err(
        DirectLocalIncludePackageHorizonError::Package {
            raw_label: occurrence.raw_label.clone(),
            location: occurrence.location.clone(),
            package: occurrence.package.clone(),
            failure,
        },
    )))
}

impl fmt::Display for RepositorySourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-source-file:{}:{}",
            self.module_name,
            self.repo_relative_path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModulePreparationKey(NormalizedAbsolutePath, ApparentRepoName);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModulePreparation {
    Supported(DirectLocalModuleClosure),
    Unsupported(DirectLocalIncludeCycleCapability),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalModuleClosure {
    root: DirectLocalModuleInspection,
    fragments: Arc<[DirectLocalIncludeFragment]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalIncludeFragment {
    package: PackageIdentifier,
    target: TargetName,
    raw_label: CompactString,
    location: crate::LogicalSpan,
    logical_path: NormalizedAbsolutePath,
    bytes: Arc<[u8]>,
    inspection: NonrootModuleFileInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalIncludeCycleCapability {
    package: PackageIdentifier,
    target: TargetName,
    repeated_raw_label: CompactString,
    repeated_location: crate::LogicalSpan,
    ancestor_raw_label: CompactString,
    ancestor_location: crate::LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModulePreparationError {
    InspectionCompute {
        message: Arc<str>,
    },
    Inspection(DirectLocalModuleInspectionError),
    RootValidation {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    Package(DirectLocalIncludePackageHorizonError),
    Fragment {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        repo_relative_path: Arc<PathBuf>,
        failure: DirectLocalIncludeFragmentFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalIncludeFragmentFailure {
    SourceCompute {
        message: Arc<str>,
    },
    Source(RepositorySourceFileError),
    Absent,
    Validation {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalIncludeAncestryEntry {
    package: PackageIdentifier,
    target: TargetName,
    raw_label: CompactString,
    location: crate::LogicalSpan,
}

#[derive(Debug, Clone)]
struct DirectLocalIncludeFrontierEntry {
    request: NonrootIncludeRequest,
    ancestry: Arc<[DirectLocalIncludeAncestryEntry]>,
}

impl DirectLocalModulePreparationKey {
    fn new(workspace: NormalizedAbsolutePath, apparent_repo: ApparentRepoName) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self(workspace, apparent_repo))
    }
}

impl fmt::Display for DirectLocalModulePreparationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-module-preparation:")?;
        self.0.fmt(f)?;
        write!(f, ":@{}", self.1.as_str())
    }
}

impl fmt::Display for DirectLocalModulePreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectionCompute { message } => {
                write!(
                    f,
                    "failed to compute direct-local MODULE inspection: {message}"
                )
            }
            Self::Inspection(error) => {
                write!(f, "failed to inspect direct-local MODULE: {error}")
            }
            Self::RootValidation {
                logical_path,
                message,
            } => write!(
                f,
                "failed to validate direct-local MODULE {}: {message}",
                logical_path.as_path().display()
            ),
            Self::Package(error) => fmt::Display::fmt(error, f),
            Self::Fragment {
                raw_label,
                location,
                repo_relative_path,
                failure,
            } => write!(
                f,
                "include {raw_label:?} at {}:{}:{} for {} failed: {failure:?}",
                location.file.0,
                location.start_line,
                location.start_column,
                repo_relative_path.display()
            ),
        }
    }
}

impl std::error::Error for DirectLocalModulePreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::Package(error) => Some(error),
            _ => None,
        }
    }
}

#[async_trait]
impl Key for DirectLocalModulePreparationKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalModulePreparation, DirectLocalModulePreparationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let inspection = match ctx
            .compute(
                &DirectLocalModuleInspectionKey::new(self.0.dupe(), self.1.clone())
                    .expect("direct preparation key rejects root names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(inspection)) => inspection,
            Err(error) => {
                return direct_local_preparation_error(
                    DirectLocalModulePreparationError::InspectionCompute {
                        message: Arc::from(error.to_string()),
                    },
                );
            }
        };
        let root = match inspection.as_ref() {
            Ok(inspection) => inspection.clone(),
            Err(error) => {
                return direct_local_preparation_error(
                    DirectLocalModulePreparationError::Inspection(error.clone()),
                );
            }
        };
        prepare_direct_local_module(ctx, root).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

async fn prepare_direct_local_module(
    ctx: &mut DiceComputations<'_>,
    root: DirectLocalModuleInspection,
) -> <DirectLocalModulePreparationKey as Key>::Value {
    let route = root.0.0.clone();
    let root_requests = match &root.0.1 {
        HostRepositorySourceFileValue::Absent => Arc::from([]),
        HostRepositorySourceFileValue::Present {
            bytes,
            logical_path,
        } => match validate_root_module_source(
            crate::LogicalModuleFileId::new(logical_path.as_path().display().to_string()),
            bytes,
        ) {
            Ok(inspection) => inspection.includes,
            Err(message) => {
                return direct_local_preparation_error(
                    DirectLocalModulePreparationError::RootValidation {
                        logical_path: logical_path.dupe(),
                        message,
                    },
                );
            }
        },
    };
    let mut frontier = root_requests
        .iter()
        .cloned()
        .map(|request| DirectLocalIncludeFrontierEntry {
            request,
            ancestry: Arc::from([]),
        })
        .collect::<Vec<_>>();
    let mut fragments = Vec::new();
    let mut pending_cycle = None;

    while !frontier.is_empty() {
        let requests = frontier
            .iter()
            .map(|entry| entry.request.clone())
            .collect::<Vec<_>>();
        let preflight =
            preflight_direct_local_include_package_horizon(ctx, route.clone(), &requests).await;
        let preflight = match preflight {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.clone(),
                Err(error) => {
                    return direct_local_preparation_error(
                        DirectLocalModulePreparationError::Package(error.clone()),
                    );
                }
            },
        };
        let paths = preflight
            .occurrences
            .iter()
            .map(direct_local_fragment_relative_path)
            .collect::<Vec<_>>();
        let mut unique_paths = SmallSet::with_capacity(paths.len());
        unique_paths.extend(paths.iter().cloned());
        let computed = ctx
            .compute_join(unique_paths, |ctx, repo_relative_path| {
                let route = route.clone();
                Box::pin(async move {
                    let outcome = ctx
                        .compute(&HostRepositorySourceFileKey::new(
                            route,
                            repo_relative_path.clone(),
                        ))
                        .await
                        .map_err(|error| Arc::<str>::from(error.to_string()));
                    (repo_relative_path, outcome)
                })
            })
            .await;
        let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
        let all_need = union_direct_local_fragment_needs(&outcomes);

        let mut next_frontier = Vec::new();
        for ((entry, occurrence), repo_relative_path) in frontier
            .iter()
            .zip(preflight.occurrences.iter())
            .zip(paths.iter())
        {
            let outcome = outcomes
                .get(repo_relative_path)
                .expect("every fragment path was computed");
            let source = match outcome {
                Err(message) => {
                    return direct_local_fragment_error(
                        occurrence,
                        repo_relative_path,
                        DirectLocalIncludeFragmentFailure::SourceCompute {
                            message: message.dupe(),
                        },
                    );
                }
                Ok(SourcePreparationOutcome::Need(_)) => {
                    return SourcePreparationOutcome::Need(
                        all_need.expect("the current fragment contributed a Need"),
                    );
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return direct_local_fragment_error(
                        occurrence,
                        repo_relative_path,
                        DirectLocalIncludeFragmentFailure::Source(error.clone()),
                    );
                }
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Absent,
                ))) => {
                    return direct_local_fragment_error(
                        occurrence,
                        repo_relative_path,
                        DirectLocalIncludeFragmentFailure::Absent,
                    );
                }
                Ok(SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Present {
                        bytes,
                        logical_path,
                    },
                ))) => (bytes, logical_path),
            };
            let logical_id =
                crate::LogicalModuleFileId::new(source.1.as_path().display().to_string());
            let inspection = match validate_root_module_source(logical_id, source.0) {
                Ok(inspection) => inspection,
                Err(message) => {
                    return direct_local_fragment_error(
                        occurrence,
                        repo_relative_path,
                        DirectLocalIncludeFragmentFailure::Validation {
                            logical_path: source.1.dupe(),
                            message,
                        },
                    );
                }
            };
            fragments.push(DirectLocalIncludeFragment {
                package: occurrence.package.clone(),
                target: occurrence.target.clone(),
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
                logical_path: source.1.dupe(),
                bytes: source.0.dupe(),
                inspection: inspection.clone(),
            });

            let repeated = entry.ancestry.iter().position(|ancestor| {
                ancestor.package == occurrence.package && ancestor.target == occurrence.target
            });
            if let Some(index) = repeated {
                if pending_cycle.is_none() {
                    let ancestor = &entry.ancestry[index];
                    pending_cycle = Some(DirectLocalIncludeCycleCapability {
                        package: occurrence.package.clone(),
                        target: occurrence.target.clone(),
                        repeated_raw_label: occurrence.raw_label.clone(),
                        repeated_location: occurrence.location.clone(),
                        ancestor_raw_label: ancestor.raw_label.clone(),
                        ancestor_location: ancestor.location.clone(),
                    });
                }
                continue;
            }

            let ancestry = entry
                .ancestry
                .iter()
                .cloned()
                .chain([DirectLocalIncludeAncestryEntry {
                    package: occurrence.package.clone(),
                    target: occurrence.target.clone(),
                    raw_label: occurrence.raw_label.clone(),
                    location: occurrence.location.clone(),
                }])
                .collect::<Arc<[_]>>();
            next_frontier.extend(inspection.includes.iter().cloned().map(|request| {
                DirectLocalIncludeFrontierEntry {
                    request,
                    ancestry: ancestry.dupe(),
                }
            }));
        }
        frontier = next_frontier;
    }

    let preparation = match pending_cycle {
        Some(cycle) => DirectLocalModulePreparation::Unsupported(cycle),
        None => DirectLocalModulePreparation::Supported(DirectLocalModuleClosure {
            root,
            fragments: fragments.into(),
        }),
    };
    SourcePreparationOutcome::Complete(Arc::new(Ok(preparation)))
}

fn direct_local_fragment_relative_path(
    occurrence: &DirectLocalIncludePackageOccurrence,
) -> PathBuf {
    PathBuf::from(occurrence.package.package().as_str()).join(occurrence.target.as_str())
}

fn union_direct_local_fragment_needs(
    outcomes: &SmallMap<PathBuf, Result<<HostRepositorySourceFileKey as Key>::Value, Arc<str>>>,
) -> Option<SourcePreparationNeeds> {
    outcomes.values().fold(None, |current, outcome| {
        let Ok(SourcePreparationOutcome::Need(incoming)) = outcome else {
            return current;
        };
        Some(match current {
            Some(current) => current
                .try_union(incoming)
                .expect("one-route fragment Needs cannot conflict"),
            None => incoming.dupe(),
        })
    })
}

fn direct_local_preparation_error(
    error: DirectLocalModulePreparationError,
) -> <DirectLocalModulePreparationKey as Key>::Value {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

fn direct_local_fragment_error(
    occurrence: &DirectLocalIncludePackageOccurrence,
    repo_relative_path: &Path,
    failure: DirectLocalIncludeFragmentFailure,
) -> <DirectLocalModulePreparationKey as Key>::Value {
    direct_local_preparation_error(DirectLocalModulePreparationError::Fragment {
        raw_label: occurrence.raw_label.clone(),
        location: occurrence.location.clone(),
        repo_relative_path: Arc::new(repo_relative_path.to_path_buf()),
        failure,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModuleEvaluationKey(NormalizedAbsolutePath, ApparentRepoName);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleEvaluation {
    Supported(DirectLocalEvaluatedModule),
    Unsupported(DirectLocalIncludeCycleCapability),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalEvaluatedModule {
    route: RootRepositoryRoute,
    module: EvaluatedNonrootModule,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleEvaluationError {
    PreparationCompute { message: Arc<str> },
    Preparation(DirectLocalModulePreparationError),
    RootAbsent { canonical_repo: CanonicalRepoName },
    Evaluation(DirectNonregistryEvaluationError),
}

impl DirectLocalModuleEvaluationKey {
    fn new(workspace: NormalizedAbsolutePath, apparent_repo: ApparentRepoName) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self(workspace, apparent_repo))
    }
}

impl fmt::Display for DirectLocalModuleEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-module-evaluation:")?;
        self.0.fmt(f)?;
        write!(f, ":@{}", self.1.as_str())
    }
}

impl fmt::Display for DirectLocalModuleEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreparationCompute { message } => {
                write!(
                    f,
                    "failed to compute direct-local MODULE preparation: {message}"
                )
            }
            Self::Preparation(error) => error.fmt(f),
            Self::RootAbsent { canonical_repo } => {
                write!(f, "MODULE.bazel expected but not found in {canonical_repo}")
            }
            Self::Evaluation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DirectLocalModuleEvaluationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preparation(error) => Some(error),
            Self::Evaluation(error) => Some(error),
            _ => None,
        }
    }
}

#[async_trait]
impl Key for DirectLocalModuleEvaluationKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let outcome = async {
            let preparation = match ctx
                .compute(
                    &DirectLocalModulePreparationKey::new(self.0.dupe(), self.1.clone())
                        .expect("direct evaluation key rejects root names"),
                )
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(preparation)) => preparation,
                Err(error) => {
                    return direct_local_evaluation_error(
                        DirectLocalModuleEvaluationError::PreparationCompute {
                            message: Arc::from(error.to_string()),
                        },
                    );
                }
            };
            let preparation = match preparation.as_ref() {
                Ok(preparation) => preparation,
                Err(error) => {
                    return direct_local_evaluation_error(
                        DirectLocalModuleEvaluationError::Preparation(error.clone()),
                    );
                }
            };
            let closure = match preparation {
                DirectLocalModulePreparation::Unsupported(capability) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Ok(
                        DirectLocalModuleEvaluation::Unsupported(capability.clone()),
                    )));
                }
                DirectLocalModulePreparation::Supported(closure) => closure,
            };
            let route = closure.root.0.0.clone();
            let root_bytes = match (&closure.root.0.1, &closure.root.1) {
                (HostRepositorySourceFileValue::Present { bytes, .. }, Some(_)) => bytes,
                (HostRepositorySourceFileValue::Absent, None) => {
                    return direct_local_evaluation_error(
                        DirectLocalModuleEvaluationError::RootAbsent {
                            canonical_repo: route.canonical_repo().clone(),
                        },
                    );
                }
                _ => panic!("direct preparation preserves root source and inspection together"),
            };
            let root_logical_id = crate::LogicalModuleFileId::new(format!(
                "{}//:MODULE.bazel",
                route.canonical_repo()
            ));
            let included = closure
                .fragments
                .iter()
                .map(|fragment| DirectNonregistryIncludeFile {
                    raw_label: fragment.raw_label.as_str(),
                    logical_id: crate::LogicalModuleFileId::new(format!(
                        "{}:{}",
                        fragment.package, fragment.target
                    )),
                    source: fragment.bytes.as_ref(),
                })
                .collect::<Vec<_>>();
            let expected_key = NonrootModuleKey::new(route.module_name(), "");
            let (evaluation, captured) = evaluate_direct_nonregistry_module_closure_with_events(
                expected_key,
                root_logical_id,
                root_bytes.as_ref(),
                &included,
                capture_events,
            );
            event_batch = captured;
            match evaluation {
                Ok(module) => SourcePreparationOutcome::Complete(Arc::new(Ok(
                    DirectLocalModuleEvaluation::Supported(DirectLocalEvaluatedModule {
                        route,
                        module,
                    }),
                ))),
                Err(error) => direct_local_evaluation_error(
                    DirectLocalModuleEvaluationError::Evaluation(error),
                ),
            }
        }
        .await;
        if capture_events && outcome.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("direct-local MODULE evaluation stores exactly one event batch");
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

fn direct_local_evaluation_error(
    error: DirectLocalModuleEvaluationError,
) -> <DirectLocalModuleEvaluationKey as Key>::Value {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum DirectLocalModuleSupport {
    Supported,
    Unsupported(DirectLocalUnsupportedCycle),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct DirectLocalUnsupportedCycle {
    apparent_repo: ApparentRepoName,
    module_name: CompactString,
    repeated_raw_label: CompactString,
    repeated_location: crate::LogicalSpan,
    ancestor_raw_label: CompactString,
    ancestor_location: crate::LogicalSpan,
}

impl fmt::Display for DirectLocalUnsupportedCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Slug does not support MODULE.bazel include cycles in direct local_path_override repository '@{}' for module '{}': include {:?} at {}:{}:{} repeats ancestor include {:?} at {}:{}:{}",
            self.apparent_repo.as_str(),
            self.module_name,
            self.repeated_raw_label,
            self.repeated_location.file.0,
            self.repeated_location.start_line,
            self.repeated_location.start_column,
            self.ancestor_raw_label,
            self.ancestor_location.file.0,
            self.ancestor_location.start_line,
            self.ancestor_location.start_column,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct DirectLocalModuleSupportError {
    inner: DirectLocalModuleSupportErrorInner,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleSupportErrorInner {
    Compute { message: Arc<str> },
    Evaluation(DirectLocalModuleEvaluationError),
}

impl fmt::Display for DirectLocalModuleSupportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            DirectLocalModuleSupportErrorInner::Compute { message } => {
                write!(
                    f,
                    "failed to compute direct-local MODULE evaluation: {message}"
                )
            }
            DirectLocalModuleSupportErrorInner::Evaluation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DirectLocalModuleSupportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            DirectLocalModuleSupportErrorInner::Evaluation(error) => Some(error),
            DirectLocalModuleSupportErrorInner::Compute { .. } => None,
        }
    }
}

pub(crate) async fn direct_local_module_support(
    ctx: &mut DiceComputations<'_>,
    route: &RootRepositoryRoute,
) -> SourcePreparationOutcome<Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>>>
{
    let key = DirectLocalModuleEvaluationKey::new(
        route.workspace().dupe(),
        route.apparent_repo().clone(),
    )
    .expect("repository source routes are nonroot");
    match ctx.compute(&key).await {
        Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
        Err(error) => {
            SourcePreparationOutcome::Complete(Arc::new(Err(DirectLocalModuleSupportError {
                inner: DirectLocalModuleSupportErrorInner::Compute {
                    message: Arc::from(error.to_string()),
                },
            })))
        }
        Ok(SourcePreparationOutcome::Complete(value)) => {
            SourcePreparationOutcome::Complete(Arc::new(match value.as_ref() {
                Err(error) => Err(DirectLocalModuleSupportError {
                    inner: DirectLocalModuleSupportErrorInner::Evaluation(error.clone()),
                }),
                Ok(DirectLocalModuleEvaluation::Supported(_)) => {
                    Ok(DirectLocalModuleSupport::Supported)
                }
                Ok(DirectLocalModuleEvaluation::Unsupported(capability)) => Ok(
                    DirectLocalModuleSupport::Unsupported(DirectLocalUnsupportedCycle {
                        apparent_repo: route.apparent_repo().clone(),
                        module_name: CompactString::new(route.module_name()),
                        repeated_raw_label: capability.repeated_raw_label.clone(),
                        repeated_location: capability.repeated_location.clone(),
                        ancestor_raw_label: capability.ancestor_raw_label.clone(),
                        ancestor_location: capability.ancestor_location.clone(),
                    }),
                ),
            }))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RepositoryMaterializationGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationGenerationKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RepositoryMaterializationGenerationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-generation:{}",
            self.workspace.display()
        )
    }
}

impl InjectedKey for RepositoryMaterializationGenerationKey {
    type Value = RepositoryMaterializationGeneration;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryIoOutcome {
    Local {
        source_root: PathBuf,
    },
    Immutable {
        source_identity: Arc<str>,
        generation_root: PathBuf,
        observation_instance: PathObservationInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryTransportError {
    pub message: CompactString,
}

#[async_trait]
pub trait RepositoryIo: Send + Sync + 'static {
    async fn materialize(
        &self,
        workspace: &Path,
        repo_spec: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError>;
}

pub fn install_repository_io(_: &mut DiceDataBuilder, _: Arc<dyn RepositoryIo>) {
    // Kept for the core runtime's compile-time integration boundary. The
    // request/result graph intentionally never reads this capability in DICE.
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterialization {
    Local {
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
        source_root: PathBuf,
    },
    Immutable {
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
        source_identity: Arc<str>,
        generation_root: PathBuf,
        observation_instance: PathObservationInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationError {
    RootModuleFiles(CompactString),
    MissingOverride(CompactString),
    UnsupportedOverride(CompactString),
    InvalidCanonicalRepository(CompactString),
    InvalidWorkspace(CompactString),
    ResultCompute(CompactString),
    MissingGeneration(CompactString),
    Spec(CompactString),
    Transport(CompactString),
    Materialization(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositorySourceFileValue {
    Present(Arc<[u8]>),
    Absent,
}

/// The Host-only source result retains the requested normalized logical path
/// submitted to resolution alongside the observed source bytes. The path can
/// name a symlink; resolver namespace, real path, and provenance stay below
/// this DICE boundary.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostRepositorySourceFileValue {
    Present {
        bytes: Arc<[u8]>,
        logical_path: NormalizedAbsolutePath,
    },
    Absent,
}

/// Semantic failure from reading a repository source file. Operational resolver
/// paths, namespaces, and symlink provenance deliberately remain below this
/// DICE boundary.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositorySourceFileError {
    InvalidRepoRelativePath {
        requested_path: Arc<PathBuf>,
    },
    MaterializationCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    Materialization {
        repo_relative_path: Arc<PathBuf>,
        error: Arc<RepositoryMaterializationError>,
    },
    InvalidMaterializedPath {
        repo_relative_path: Arc<PathBuf>,
    },
    Observation {
        repo_relative_path: Arc<PathBuf>,
        operation: PathObservationOperation,
        error: PathObservationError,
    },
    InconsistentState {
        repo_relative_path: Arc<PathBuf>,
        operation: PathObservationOperation,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    WrongKind {
        repo_relative_path: Arc<PathBuf>,
        actual: PathNodeKind,
    },
    Cycle {
        repo_relative_path: Arc<PathBuf>,
    },
    InfiniteExpansion {
        repo_relative_path: Arc<PathBuf>,
    },
    ResolutionCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    FileCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RepositoryMaterializationRequestKey {
    workspace: PathBuf,
    module_name: CompactString,
}

impl fmt::Display for RepositoryMaterializationRequestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-materialization-request:{}", self.module_name)
    }
}

#[async_trait]
impl Key for RepositoryMaterializationRequestKey {
    type Value = Arc<Result<RepositoryMaterializationRequest, RepositoryMaterializationError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root_files = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(root_files) => root_files,
            Err(error) => {
                return Arc::new(Err(RepositoryMaterializationError::RootModuleFiles(
                    error.to_string().into(),
                )));
            }
        };
        let root_files = match root_files.as_ref() {
            Ok(root_files) => root_files,
            Err(error) => {
                return Arc::new(Err(RepositoryMaterializationError::RootModuleFiles(
                    error.clone(),
                )));
            }
        };
        let repo_spec = match root_files.overrides.get(self.module_name.as_str()) {
            Some(RootModuleOverride::NonRegistry(repo_spec)) => repo_spec.clone(),
            Some(_) => {
                return Arc::new(Err(RepositoryMaterializationError::UnsupportedOverride(
                    format!(
                        "module {} does not have a non-registry override",
                        self.module_name
                    )
                    .into(),
                )));
            }
            None => {
                return Arc::new(Err(RepositoryMaterializationError::MissingOverride(
                    self.module_name.clone(),
                )));
            }
        };
        let canonical_repo = match CanonicalRepoName::new(format!("{}+", self.module_name)) {
            Ok(repo) => repo,
            Err(error) => {
                return Arc::new(Err(
                    RepositoryMaterializationError::InvalidCanonicalRepository(error.into()),
                ));
            }
        };
        let workspace = match NormalizedAbsolutePath::new(self.workspace.clone()) {
            Ok(workspace) => workspace,
            Err(error) => {
                return Arc::new(Err(RepositoryMaterializationError::InvalidWorkspace(
                    error.to_string().into(),
                )));
            }
        };
        let kind = match request_kind(&workspace, &repo_spec) {
            Ok(kind) => kind,
            Err(error) => return Arc::new(Err(error)),
        };
        Arc::new(Ok(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace,
                canonical_repo,
            },
            repo_spec,
            kind,
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

fn request_kind(
    workspace: &NormalizedAbsolutePath,
    repo_spec: &RepoSpec,
) -> Result<RepositoryMaterializationKind, RepositoryMaterializationError> {
    let local_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
        .expect("pinned local repository label is canonical");
    if repo_spec.rule_id.bzl_file == local_bzl && repo_spec.rule_id.rule_name == "local_repository"
    {
        if repo_spec.attributes.len() != 1 {
            return Err(RepositoryMaterializationError::Spec(
                "local_repository has unsupported attributes".into(),
            ));
        }
        let Some(OverrideAttributeValue::String(path)) = repo_spec.attributes.get("path") else {
            return Err(RepositoryMaterializationError::Spec(
                "local_repository requires a string path".into(),
            ));
        };
        let relative = Path::new(path.as_str());
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(RepositoryMaterializationError::Spec(
                "local_repository path must be normalized and workspace-relative".into(),
            ));
        }
        let root = NormalizedAbsolutePath::new(workspace.as_path().join(relative))
            .map_err(|error| RepositoryMaterializationError::Spec(error.to_string().into()))?;
        return Ok(RepositoryMaterializationKind::Local { logical_root: root });
    }
    let http_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
        .expect("pinned http repository label is canonical");
    let git_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:git.bzl")
        .expect("pinned git repository label is canonical");
    if (repo_spec.rule_id.bzl_file == http_bzl && repo_spec.rule_id.rule_name == "http_archive")
        || (repo_spec.rule_id.bzl_file == git_bzl
            && repo_spec.rule_id.rule_name == "git_repository")
    {
        return Ok(RepositoryMaterializationKind::Immutable);
    }
    Err(RepositoryMaterializationError::Spec(
        "unsupported repository override rule".into(),
    ))
}

fn host_repository_materialization_request(
    route: &RootRepositoryRoute,
) -> Result<Arc<RepositoryMaterializationRequest>, RepositoryMaterializationError> {
    if route.is_builtin_bazel_tools() {
        return Err(RepositoryMaterializationError::Spec(
            "built-in bazel_tools source requires its immutable source owner".into(),
        ));
    }
    Ok(Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: route.workspace().dupe(),
            canonical_repo: route.canonical_repo().clone(),
        },
        repo_spec: route.repo_spec().clone(),
        kind: request_kind(route.workspace(), route.repo_spec())?,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct RepositoryMaterializationResultKey {
    request: Arc<RepositoryMaterializationRequest>,
}

impl Hash for RepositoryMaterializationResultKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.request.id.workspace.hash(state);
        self.request.id.canonical_repo.hash(state);
    }
}

impl fmt::Display for RepositoryMaterializationResultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-result:{}",
            self.request.id.canonical_repo
        )
    }
}

#[async_trait]
impl Key for RepositoryMaterializationResultKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let epoch = match ctx
            .compute(&RepositoryMaterializationResultEpochKey {
                workspace: self.request.id.workspace.dupe(),
            })
            .await
        {
            Ok(epoch) => epoch,
            Err(_) => {
                return SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
                    (*self.request).clone(),
                ));
            }
        };
        let Some(entry) = epoch.get(&self.request) else {
            return SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
                (*self.request).clone(),
            ));
        };
        if entry.request != self.request {
            return SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
                (*self.request).clone(),
            ));
        }
        let result = match &entry.result {
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local) => {
                let RepositoryMaterializationKind::Local { logical_root } = &self.request.kind
                else {
                    unreachable!("validated epoch success kind must match request kind");
                };
                Ok(RepositoryMaterialization::Local {
                    canonical_repo: self.request.id.canonical_repo.clone(),
                    repo_spec: self.request.repo_spec.clone(),
                    source_root: logical_root.as_path().to_owned(),
                })
            }
            RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    source_identity,
                    generation_root,
                    observation_instance,
                },
            ) => Ok(RepositoryMaterialization::Immutable {
                canonical_repo: self.request.id.canonical_repo.clone(),
                repo_spec: self.request.repo_spec.clone(),
                source_identity: source_identity.clone(),
                generation_root: generation_root.clone(),
                observation_instance: *observation_instance,
            }),
            RepositoryMaterializationResult::SpecError(message) => {
                Err(RepositoryMaterializationError::Spec(message.clone()))
            }
            RepositoryMaterializationResult::TransportError {
                generation,
                message,
            } => {
                let current = match ctx
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: self.request.id.workspace.as_path().to_owned(),
                    })
                    .await
                {
                    Ok(current) => current,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Arc::new(Err(
                            RepositoryMaterializationError::MissingGeneration(
                                error.to_string().into(),
                            ),
                        )));
                    }
                };
                if current != *generation {
                    return SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
                        (*self.request).clone(),
                    ));
                }
                Err(RepositoryMaterializationError::Transport(message.clone()))
            }
            RepositoryMaterializationResult::MaterializationError {
                generation,
                message,
            } => {
                let current = match ctx
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: self.request.id.workspace.as_path().to_owned(),
                    })
                    .await
                {
                    Ok(current) => current,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Arc::new(Err(
                            RepositoryMaterializationError::MissingGeneration(
                                error.to_string().into(),
                            ),
                        )));
                    }
                };
                if current != *generation {
                    return SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
                        (*self.request).clone(),
                    ));
                }
                Err(RepositoryMaterializationError::Materialization(
                    message.clone(),
                ))
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(result))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| self.request.dupe());
    }
}

#[async_trait]
impl Key for RepositoryMaterializationKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let request = match ctx
            .compute(&RepositoryMaterializationRequestKey {
                workspace: self.workspace.clone(),
                module_name: self.module_name.clone(),
            })
            .await
        {
            Ok(request) => request,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    RepositoryMaterializationError::RootModuleFiles(error.to_string().into()),
                )));
            }
        };
        let request = match request.as_ref() {
            Ok(request) => request.clone(),
            Err(error) => return SourcePreparationOutcome::Complete(Arc::new(Err(error.clone()))),
        };
        match ctx
            .compute(&RepositoryMaterializationResultKey {
                request: Arc::new(request),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => SourcePreparationOutcome::Complete(Arc::new(Err(
                RepositoryMaterializationError::ResultCompute(error.to_string().into()),
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn project_resolution_error(
    repo_relative_path: Arc<PathBuf>,
    error: PathResolutionError,
) -> RepositorySourceFileError {
    match error {
        PathResolutionError::Observation { demand, error, .. } => {
            RepositorySourceFileError::Observation {
                repo_relative_path,
                operation: demand.operation(),
                error,
            }
        }
        PathResolutionError::InconsistentState {
            demand,
            before,
            after,
            ..
        } => RepositorySourceFileError::InconsistentState {
            repo_relative_path,
            operation: demand.operation(),
            before,
            after,
        },
        PathResolutionError::Cycle { .. } => {
            RepositorySourceFileError::Cycle { repo_relative_path }
        }
        PathResolutionError::InfiniteExpansion { .. } => {
            RepositorySourceFileError::InfiniteExpansion { repo_relative_path }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum ObservedRepositorySourceFile {
    Present {
        bytes: Arc<[u8]>,
        logical_path: NormalizedAbsolutePath,
    },
    Absent,
}

async fn observed_repository_source_file(
    ctx: &mut DiceComputations<'_>,
    namespace: PathObservationNamespace,
    materialized_root: &Path,
    relative: &Path,
    repo_relative_path: Arc<PathBuf>,
) -> PathResult<ObservedRepositorySourceFile, RepositorySourceFileError> {
    let logical_path = match NormalizedAbsolutePath::new(materialized_root.join(relative)) {
        Ok(path) => path,
        Err(_) => {
            return PathOutcome::Complete(Err(
                RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path },
            ));
        }
    };
    let resolved = match ctx
        .compute(&ResolvedPathKey::new(namespace, logical_path.dupe()))
        .await
    {
        Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
        Ok(PathOutcome::Complete(Err(error))) => {
            return PathOutcome::Complete(Err(project_resolution_error(repo_relative_path, error)));
        }
        Ok(PathOutcome::Complete(Ok(resolved))) => resolved,
        Err(error) => {
            return PathOutcome::Complete(Err(RepositorySourceFileError::ResolutionCompute {
                repo_relative_path,
                message: Arc::from(error.to_string()),
            }));
        }
    };
    observed_repository_source_file_from_resolved(ctx, resolved, repo_relative_path).await
}

async fn observed_repository_source_file_from_resolved(
    ctx: &mut DiceComputations<'_>,
    resolved: ResolvedPath,
    repo_relative_path: Arc<PathBuf>,
) -> PathResult<ObservedRepositorySourceFile, RepositorySourceFileError> {
    let lstat = match resolved.state() {
        ResolvedPathState::Missing => {
            return PathOutcome::Complete(Ok(ObservedRepositorySourceFile::Absent));
        }
        ResolvedPathState::Present(lstat)
            if matches!(
                lstat.kind(),
                PathNodeKind::RegularFile | PathNodeKind::SpecialFile
            ) =>
        {
            lstat
        }
        ResolvedPathState::Present(lstat) => {
            return PathOutcome::Complete(Err(RepositorySourceFileError::WrongKind {
                repo_relative_path,
                actual: lstat.kind(),
            }));
        }
    };
    let demand = PathObservationDemand::new(
        resolved.namespace(),
        resolved.real_path().dupe(),
        PathObservationOperation::FileBytes,
    );
    let observed = match ctx.compute(&PathObservationKey::new(demand)).await {
        Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
        Ok(PathOutcome::Complete(result)) => result,
        Err(error) => {
            return PathOutcome::Complete(Err(RepositorySourceFileError::FileCompute {
                repo_relative_path,
                message: Arc::from(error.to_string()),
            }));
        }
    };
    match observed.as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
            PathOutcome::Complete(Ok(ObservedRepositorySourceFile::Present {
                bytes: bytes.dupe(),
                logical_path: resolved.requested_path().dupe(),
            }))
        }
        PathObservationResult::FileBytes(PathOperationResult::Missing) => {
            PathOutcome::Complete(Err(RepositorySourceFileError::InconsistentState {
                repo_relative_path,
                operation: PathObservationOperation::FileBytes,
                before: Some(lstat),
                after: None,
            }))
        }
        PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
            PathOutcome::Complete(Err(RepositorySourceFileError::Observation {
                repo_relative_path,
                operation: PathObservationOperation::FileBytes,
                error: *error,
            }))
        }
        PathObservationResult::Lstat(_)
        | PathObservationResult::ReadLink(_)
        | PathObservationResult::DirectoryEntries(_)
        | PathObservationResult::WindowsLongPath(_)
        | PathObservationResult::WindowsOptionPathLongName(_) => {
            unreachable!("FileBytes demand must return a FileBytes observation")
        }
    }
}

async fn resolved_repository_path_from_materialization(
    ctx: &mut DiceComputations<'_>,
    materialization: SourcePreparationOutcome<
        Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>,
    >,
    relative: &Path,
    repo_relative_path: Arc<PathBuf>,
) -> SourcePreparationResult<HostRepositoryPathValue, RepositorySourceFileError> {
    let materialization = match materialization {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(value) => value,
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error.clone()),
                },
            ));
        }
    };
    let (namespace, root) = match materialization {
        RepositoryMaterialization::Local { source_root, .. } => {
            (PathObservationNamespace::Host, source_root)
        }
        RepositoryMaterialization::Immutable {
            generation_root,
            observation_instance,
            ..
        } => (
            PathObservationNamespace::Materialization(*observation_instance),
            generation_root,
        ),
    };
    let requested_path = match NormalizedAbsolutePath::new(root.join(relative)) {
        Ok(path) => path,
        Err(_) => {
            return SourcePreparationOutcome::Complete(Err(
                RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path },
            ));
        }
    };
    match ctx
        .compute(&ResolvedPathKey::new(namespace, requested_path))
        .await
    {
        Ok(PathOutcome::Need(need)) => SourcePreparationOutcome::path_need(need),
        Ok(PathOutcome::Complete(Ok(resolved))) => {
            SourcePreparationOutcome::Complete(Ok(HostRepositoryPathValue(resolved)))
        }
        Ok(PathOutcome::Complete(Err(error))) => SourcePreparationOutcome::Complete(Err(
            project_resolution_error(repo_relative_path, error),
        )),
        Err(error) => {
            SourcePreparationOutcome::Complete(Err(RepositorySourceFileError::ResolutionCompute {
                repo_relative_path,
                message: Arc::from(error.to_string()),
            }))
        }
    }
}

async fn observed_repository_source_file_from_materialization(
    ctx: &mut DiceComputations<'_>,
    materialization: SourcePreparationOutcome<
        Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>,
    >,
    relative: &Path,
    repo_relative_path: Arc<PathBuf>,
) -> SourcePreparationResult<ObservedRepositorySourceFile, RepositorySourceFileError> {
    let materialization = match materialization {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(value) => value,
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error.clone()),
                },
            ));
        }
    };
    match materialization {
        RepositoryMaterialization::Local { source_root, .. } => source_outcome_from_path(
            observed_repository_source_file(
                ctx,
                PathObservationNamespace::Host,
                source_root,
                relative,
                repo_relative_path,
            )
            .await,
        ),
        RepositoryMaterialization::Immutable {
            generation_root,
            observation_instance,
            ..
        } => source_outcome_from_path(
            observed_repository_source_file(
                ctx,
                PathObservationNamespace::Materialization(*observation_instance),
                generation_root,
                relative,
                repo_relative_path,
            )
            .await,
        ),
    }
}

fn legacy_repository_source_file_value(
    outcome: SourcePreparationResult<ObservedRepositorySourceFile, RepositorySourceFileError>,
) -> SourcePreparationResult<RepositorySourceFileValue, RepositorySourceFileError> {
    outcome.map(|result| {
        result.map(|observed| match observed {
            ObservedRepositorySourceFile::Present { bytes, .. } => {
                RepositorySourceFileValue::Present(bytes)
            }
            ObservedRepositorySourceFile::Absent => RepositorySourceFileValue::Absent,
        })
    })
}

fn host_repository_source_file_value(
    outcome: SourcePreparationResult<ObservedRepositorySourceFile, RepositorySourceFileError>,
) -> SourcePreparationResult<HostRepositorySourceFileValue, RepositorySourceFileError> {
    outcome.map(|result| {
        result.map(|observed| match observed {
            ObservedRepositorySourceFile::Present {
                bytes,
                logical_path,
            } => HostRepositorySourceFileValue::Present {
                bytes,
                logical_path,
            },
            ObservedRepositorySourceFile::Absent => HostRepositorySourceFileValue::Absent,
        })
    })
}

#[async_trait]
impl Key for HostRepositoryPathKey {
    type Value = SourcePreparationResult<HostRepositoryPathValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let relative = match checked_relative_path(&self.repo_relative_path) {
            Ok(relative) => relative,
            Err(_) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::InvalidRepoRelativePath {
                        requested_path: Arc::new(self.repo_relative_path.clone()),
                    },
                ));
            }
        };
        let repo_relative_path = Arc::new(relative.to_owned());
        let request = match host_repository_materialization_request(&self.route) {
            Ok(request) => request,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::Materialization {
                        repo_relative_path,
                        error: Arc::new(error),
                    },
                ));
            }
        };
        let materialization = match ctx
            .compute(&RepositoryMaterializationResultKey { request })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::MaterializationCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    },
                ));
            }
        };
        resolved_repository_path_from_materialization(
            ctx,
            materialization,
            relative,
            repo_relative_path,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| RepositorySourceScope {
            workspace: self.route.workspace().dupe(),
            module_name: CompactString::new(self.route.module_name()),
        });
    }
}

#[async_trait]
impl Key for HostRepositorySourceFileKey {
    type Value = SourcePreparationResult<HostRepositorySourceFileValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let repo_relative_path = Arc::new(self.repo_relative_path.clone());
        let path = match ctx
            .compute(&HostRepositoryPathKey::new(
                self.route.clone(),
                self.repo_relative_path.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(path))) => path,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::ResolutionCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    },
                ));
            }
        };
        // `HostRepositoryPathKey` remains the sole validation/materialization
        // owner. Once it succeeds, retain the same exact request as a direct
        // provenance dependency of this selected byte-source anchor.
        let request = host_repository_materialization_request(&self.route)
            .expect("a successful HostRepositoryPathKey has a valid materialization request");
        let materialization = ctx
            .compute(&RepositoryMaterializationResultKey { request })
            .await
            .expect("the successful HostRepositoryPathKey computed materialization");
        debug_assert!(materialization.is_complete());
        host_repository_source_file_value(source_outcome_from_path(
            observed_repository_source_file_from_resolved(
                ctx,
                path.resolved().dupe(),
                repo_relative_path,
            )
            .await,
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| RepositorySourceScope {
            workspace: self.route.workspace().dupe(),
            module_name: CompactString::new(self.route.module_name()),
        });
    }
}

#[async_trait]
impl Key for RepositorySourceFileKey {
    type Value = SourcePreparationResult<RepositorySourceFileValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let relative = match checked_relative_path(&self.repo_relative_path) {
            Ok(relative) => relative,
            Err(_) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::InvalidRepoRelativePath {
                        requested_path: Arc::new(self.repo_relative_path.clone()),
                    },
                ));
            }
        };
        let repo_relative_path = Arc::new(relative.to_owned());
        let materialization = match ctx
            .compute(&RepositoryMaterializationKey {
                workspace: self.workspace.clone(),
                module_name: self.module_name.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::MaterializationCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    },
                ));
            }
        };
        legacy_repository_source_file_value(
            observed_repository_source_file_from_materialization(
                ctx,
                materialization,
                relative,
                repo_relative_path,
            )
            .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        if self.workspace.is_absolute() {
            demand.provide_value_with(|| RepositorySourceScope {
                workspace: NormalizedAbsolutePath::new(self.workspace.clone())
                    .expect("an absolute repository-source workspace normalizes"),
                module_name: self.module_name.clone(),
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryPackagePreflight {
    BuildDotBazel,
    Build,
    Ignored,
    InvalidPackageName { message: Arc<str> },
    NoBuildFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryPackagePreflightError {
    RootModuleFiles(CompactString),
    NonregistryOverrideRequired(CompactString),
    PolicyInput(crate::RootPackagePolicyProjectionError),
    UnsupportedDeletedPackages,
    RepositoryIgnore(HostRepositoryIgnoreError),
    RepositorySource {
        marker: HostBuildFileName,
        error: RepositorySourceFileError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryPackagePreflightKey {
    workspace: NormalizedAbsolutePath,
    module: NonrootModuleKey,
    package: PackagePath,
}

impl HostNonregistryPackagePreflightKey {
    pub(crate) fn new(
        workspace: NormalizedAbsolutePath,
        module: NonrootModuleKey,
        package: PackagePath,
    ) -> Self {
        Self {
            workspace,
            module,
            package,
        }
    }
}

impl fmt::Display for HostNonregistryPackagePreflightKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-nonregistry-package-preflight:{}@{}//{}",
            self.module.name, self.module.version, self.package
        )
    }
}

#[track_caller]
fn preflight_dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("nonregistry package-preflight DICE invariant: {error:?}"))
}

type HostNonregistryPackagePreflightValue = SourcePreparationOutcome<
    Arc<Result<HostNonregistryPackagePreflight, HostNonregistryPackagePreflightError>>,
>;

fn preflight_complete(
    value: Result<HostNonregistryPackagePreflight, HostNonregistryPackagePreflightError>,
) -> HostNonregistryPackagePreflightValue {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostNonregistryPackagePreflightKey {
    type Value = HostNonregistryPackagePreflightValue;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root = preflight_dice_invariant(
            ctx.compute(&RootModuleFilesKey {
                workspace: self.workspace.as_path().to_owned(),
            })
            .await,
        );
        let root = match root.as_ref() {
            Ok(root) => root,
            Err(error) => {
                return preflight_complete(Err(
                    HostNonregistryPackagePreflightError::RootModuleFiles(error.clone()),
                ));
            }
        };
        if !matches!(
            root.overrides.get(self.module.name.as_str()),
            Some(RootModuleOverride::NonRegistry(_))
        ) {
            return preflight_complete(Err(
                HostNonregistryPackagePreflightError::NonregistryOverrideRequired(
                    self.module.name.clone(),
                ),
            ));
        }
        if let Some(message) = invalid_package_name(&self.package) {
            return preflight_complete(Ok(HostNonregistryPackagePreflight::InvalidPackageName {
                message,
            }));
        }
        let deleted = match preflight_dice_invariant(
            ctx.compute(&CanonicalDeletedPackagesProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(value) => value,
            Err(error) => {
                return preflight_complete(Err(HostNonregistryPackagePreflightError::PolicyInput(
                    error,
                )));
            }
        };
        if !deleted.is_empty() {
            return preflight_complete(Err(
                HostNonregistryPackagePreflightError::UnsupportedDeletedPackages,
            ));
        }
        let ignore = match preflight_dice_invariant(
            ctx.compute(&HostNonregistryRepositoryIgnoreKey::new(
                self.workspace.dupe(),
                self.module.clone(),
            ))
            .await,
        ) {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return preflight_complete(Err(
                        HostNonregistryPackagePreflightError::RepositoryIgnore(error.clone()),
                    ));
                }
            },
        };
        if ignore.matching_entry(&self.package).is_some() {
            return preflight_complete(Ok(HostNonregistryPackagePreflight::Ignored));
        }
        for marker in [HostBuildFileName::BuildDotBazel, HostBuildFileName::Build] {
            let name = match marker {
                HostBuildFileName::BuildDotBazel => "BUILD.bazel",
                HostBuildFileName::Build => "BUILD",
            };
            let path = if self.package.as_str().is_empty() {
                PathBuf::from(name)
            } else {
                PathBuf::from(self.package.as_str()).join(name)
            };
            match preflight_dice_invariant(
                ctx.compute(&RepositorySourceFileKey {
                    workspace: self.workspace.as_path().to_owned(),
                    module_name: self.module.name.clone(),
                    repo_relative_path: path,
                })
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Present(_))) => {
                    let value = match marker {
                        HostBuildFileName::BuildDotBazel => {
                            HostNonregistryPackagePreflight::BuildDotBazel
                        }
                        HostBuildFileName::Build => HostNonregistryPackagePreflight::Build,
                    };
                    return preflight_complete(Ok(value));
                }
                SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Absent))
                | SourcePreparationOutcome::Complete(Err(RepositorySourceFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                })) => {}
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return preflight_complete(Err(
                        HostNonregistryPackagePreflightError::RepositorySource { marker, error },
                    ));
                }
            }
        }
        preflight_complete(Ok(HostNonregistryPackagePreflight::NoBuildFile))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for ModuleSourcePreparationKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RootModuleFiles(error.to_string().into()),
                )));
            }
        };
        let root = match root.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RootModuleFiles(error.clone()),
                )));
            }
        };
        let override_ = root.overrides.get(self.module_name.as_str()).cloned();
        if matches!(override_, Some(RootModuleOverride::NonRegistry(_))) {
            let value = match ctx
                .compute(&RepositorySourceFileKey {
                    workspace: self.workspace.clone(),
                    module_name: self.module_name.clone(),
                    repo_relative_path: PathBuf::from("MODULE.bazel"),
                })
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Present(
                    bytes,
                )))) => Ok(ModuleSourcePreparation::NonRegistry { bytes }),
                Ok(SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Absent))) => {
                    Err(ModuleSourcePreparationError::ModuleNotFound {
                        module_file_attempts: Arc::from([]),
                    })
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    Err(ModuleSourcePreparationError::Source(error))
                }
                Err(error) => Err(ModuleSourcePreparationError::SourceCompute(Arc::from(
                    error.to_string(),
                ))),
            };
            return SourcePreparationOutcome::Complete(Arc::new(value));
        }
        if self.version.is_empty() {
            return SourcePreparationOutcome::Complete(Arc::new(Err(
                ModuleSourcePreparationError::MissingVersion,
            )));
        }
        let policy = match ctx
            .compute(&RegistryPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RegistryPolicyCompute(error.to_string().into()),
                )));
            }
        };
        let policy = match policy.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RegistryPolicy(error.clone()),
                )));
            }
        };
        let override_registry = match override_.as_ref() {
            Some(RootModuleOverride::RegistrySingle(value)) if !value.registry.is_empty() => {
                Some(value.registry.as_str())
            }
            Some(RootModuleOverride::RegistryMultiple(value)) if !value.registry.is_empty() => {
                Some(value.registry.as_str())
            }
            _ => None,
        };
        let module = ModuleKey::new(self.module_name.as_str(), self.version.as_str());
        if let Some(registry) = override_registry {
            let mut attempts = Vec::new();
            return match self
                .prepare_from_registry(ctx, override_.as_ref(), registry, &module, &mut attempts)
                .await
            {
                PathOutcome::Need(need) => SourcePreparationOutcome::path_need(need),
                PathOutcome::Complete(result) => {
                    SourcePreparationOutcome::Complete(Arc::new(match result {
                        Ok(Some(value)) => Ok(value),
                        Ok(None) => Err(ModuleSourcePreparationError::ModuleNotFound {
                            module_file_attempts: Arc::from(attempts),
                        }),
                        Err(error) => Err(error),
                    }))
                }
            };
        }
        let mut attempts = Vec::new();
        for registry in policy.urls().as_slice() {
            match self
                .prepare_from_registry(
                    ctx,
                    override_.as_ref(),
                    registry.as_str(),
                    &module,
                    &mut attempts,
                )
                .await
            {
                PathOutcome::Need(need) => return SourcePreparationOutcome::path_need(need),
                PathOutcome::Complete(Ok(Some(value))) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Ok(value)));
                }
                PathOutcome::Complete(Ok(None)) => {}
                PathOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Arc::new(Err(error)));
                }
            }
        }
        SourcePreparationOutcome::Complete(Arc::new(Err(
            ModuleSourcePreparationError::ModuleNotFound {
                module_file_attempts: Arc::from(attempts),
            },
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl ModuleSourcePreparationKey {
    async fn prepare_from_registry(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        registry: &str,
        module: &ModuleKey,
        attempts: &mut Vec<RegistryModuleFileAttempt>,
    ) -> PathOutcome<Result<Option<ModuleSourcePreparation>, ModuleSourcePreparationError>> {
        let url = RegistryFileUrl::new(registry_module_file_url(registry, module));
        let file = match ctx
            .compute(&RegistryFileKey {
                workspace: self.workspace.clone(),
                url: url.clone(),
            })
            .await
        {
            Ok(file) => file,
            Err(error) => {
                return PathOutcome::Complete(Err(
                    ModuleSourcePreparationError::RegistryFileCompute {
                        url: url.clone(),
                        prior_not_found_attempts: Arc::from(attempts.as_slice()),
                        message: error.to_string().into(),
                    },
                ));
            }
        };
        match file.as_ref() {
            Ok(RegistryFileValue::NotFound { .. }) => {
                attempts.push(RegistryModuleFileAttempt { url, sha256: None });
                PathOutcome::Complete(Ok(None))
            }
            Ok(RegistryFileValue::Found { bytes, sha256, .. }) => {
                let selected_registry = RegistryBaseUrl::new(registry);
                let bytes = match self.apply_root_patches(ctx, override_, bytes.clone()).await {
                    PathOutcome::Need(need) => return PathOutcome::Need(need),
                    PathOutcome::Complete(Ok(bytes)) => bytes,
                    PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
                };
                attempts.push(RegistryModuleFileAttempt {
                    url,
                    sha256: Some(*sha256),
                });
                PathOutcome::Complete(Ok(Some(ModuleSourcePreparation::Registry {
                    bytes,
                    selected_registry,
                    module_file_attempts: Arc::from(attempts.as_slice()),
                })))
            }
            Err(error) => PathOutcome::Complete(Err(ModuleSourcePreparationError::RegistryFile {
                url,
                prior_not_found_attempts: Arc::from(attempts.as_slice()),
                error: error.clone(),
            })),
        }
    }

    async fn apply_root_patches(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        mut bytes: Arc<[u8]>,
    ) -> PathOutcome<Result<Arc<[u8]>, ModuleSourcePreparationError>> {
        let Some(RootModuleOverride::RegistrySingle(override_)) = override_ else {
            return PathOutcome::Complete(Ok(bytes));
        };
        // PatchUtil filters this list to main-repository labels. `patch_cmds`
        // are deliberately inactive for module-file patching.
        let mut patches = Vec::new();
        for label in override_.patches.iter() {
            let Some(path) = main_repo_patch_path(label) else {
                continue;
            };
            let logical_path = match NormalizedAbsolutePath::new(self.workspace.join(path)) {
                Ok(path) => path,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::InvalidPatchPath {
                            path: error.path().to_owned(),
                        },
                    ));
                }
            };
            let resolved = match ctx
                .compute(&ResolvedPathKey::new(
                    PathObservationNamespace::Host,
                    logical_path.dupe(),
                ))
                .await
            {
                Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                Ok(PathOutcome::Complete(Err(error))) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchResolution(error),
                    ));
                }
                Ok(PathOutcome::Complete(Ok(resolved))) => resolved,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchResolutionCompute {
                            logical_path,
                            message: error.to_string().into(),
                        },
                    ));
                }
            };
            match resolved.state() {
                ResolvedPathState::Missing => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchMissing { logical_path },
                    ));
                }
                ResolvedPathState::Present(lstat)
                    if matches!(
                        lstat.kind(),
                        PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                    ) =>
                {
                    patches.push((logical_path, resolved));
                }
                ResolvedPathState::Present(lstat) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchWrongKind {
                            logical_path,
                            actual: lstat.kind(),
                        },
                    ));
                }
            }
        }

        for (_logical_path, resolved) in patches {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                resolved.real_path().dupe(),
                PathObservationOperation::FileBytes,
            );
            let observed = match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
                Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                Ok(PathOutcome::Complete(result)) => result,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileCompute {
                            demand,
                            message: error.to_string().into(),
                        },
                    ));
                }
            };
            let patch = match observed.as_ref() {
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
                    bytes.dupe()
                }
                PathObservationResult::FileBytes(PathOperationResult::Missing) => {
                    let before = match resolved.state() {
                        ResolvedPathState::Present(lstat) => Some(lstat),
                        ResolvedPathState::Missing => None,
                    };
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileInconsistentState {
                            demand,
                            before,
                            after: None,
                        },
                    ));
                }
                PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileObservation {
                            demand,
                            error: *error,
                        },
                    ));
                }
                PathObservationResult::Lstat(_)
                | PathObservationResult::ReadLink(_)
                | PathObservationResult::DirectoryEntries(_)
                | PathObservationResult::WindowsLongPath(_)
                | PathObservationResult::WindowsOptionPathLongName(_) => {
                    unreachable!("FileBytes demand must return a FileBytes observation")
                }
            };
            if !patch.is_empty() {
                bytes = match apply_unified_patch(bytes, &patch, override_.patch_strip) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        return PathOutcome::Complete(Err(ModuleSourcePreparationError::Patch(
                            error.0,
                        )));
                    }
                };
            }
        }
        PathOutcome::Complete(Ok(bytes))
    }
}

fn main_repo_patch_path(label: &CanonicalLabel) -> Option<PathBuf> {
    if !label.package().repo().as_str().is_empty() {
        return None;
    }
    let mut path = PathBuf::new();
    let package = label.package().package().as_str();
    if !package.is_empty() {
        path.push(package);
    }
    path.push(label.target().as_str());
    (!path
        .components()
        .any(|component| !matches!(component, Component::Normal(_))))
    .then_some(path)
}

fn checked_relative_path(path: &Path) -> Result<&Path, CompactString> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "repository source path is not a normalized relative path: {}",
            path.display()
        )
        .into());
    }
    Ok(path)
}

pub fn source_identity(bytes: &[u8]) -> Arc<str> {
    Arc::from(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_events_v2::EvaluationEvent;
    use slug_identity_v2::ApparentRepoName;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::*;
    use crate::RootPackagePolicyInputs;
    use crate::RootRepositoryRouteKey;
    use crate::inject_root_module_request_inputs;
    use crate::inject_root_package_policy_inputs;

    #[derive(Default)]
    struct HostSourceDependencyTracker {
        source_dependencies: Mutex<Vec<String>>,
        path_dependencies: Mutex<Vec<String>>,
    }

    impl ActivationTracker for HostSourceDependencyTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<HostRepositorySourceFileKey>().is_some() {
                self.source_dependencies
                    .lock()
                    .unwrap()
                    .extend(deps.map(ToString::to_string));
            } else if key.downcast_ref::<HostRepositoryPathKey>().is_some() {
                self.path_dependencies
                    .lock()
                    .unwrap()
                    .extend(deps.map(ToString::to_string));
            }
        }
    }

    #[derive(Default)]
    struct DirectTracker(Mutex<Vec<(ActivationKind, bool)>>);
    impl ActivationTracker for DirectTracker {
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
        fn key_activated_rich(&self, key: &DynKey, a: RichActivation<'_>) {
            if key.downcast_ref::<DirectLocalModuleFileKey>().is_some() {
                self.0
                    .lock()
                    .unwrap()
                    .push((a.kind(), a.evaluation_data().is_none()));
            }
        }
    }

    #[derive(Default)]
    struct InspectionTracker(Mutex<Vec<(ActivationKind, bool)>>);
    impl ActivationTracker for InspectionTracker {
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
        fn key_activated_rich(&self, key: &DynKey, a: RichActivation<'_>) {
            if key
                .downcast_ref::<DirectLocalModuleInspectionKey>()
                .is_some()
            {
                self.0
                    .lock()
                    .unwrap()
                    .push((a.kind(), a.evaluation_data().is_none()));
            }
        }
    }
    #[derive(Default)]
    struct NonregistryPreflightTracker {
        preflight: Mutex<Vec<(ActivationKind, bool)>>,
        repo: Mutex<Vec<(ActivationKind, bool)>>,
    }

    impl ActivationTracker for NonregistryPreflightTracker {
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
            let record = (activation.kind(), activation.evaluation_data().is_none());
            if key
                .downcast_ref::<HostNonregistryPackagePreflightKey>()
                .is_some()
            {
                self.preflight.lock().unwrap().push(record);
            } else if key
                .downcast_ref::<crate::repo_file::HostNonregistryRepoFileKey>()
                .is_some()
            {
                self.repo.lock().unwrap().push(record);
            }
        }
    }

    #[derive(Debug, Default)]
    struct HorizonTracker {
        horizon: Mutex<Vec<(ActivationKind, bool)>>,
        inspection: Mutex<Vec<ActivationKind>>,
        lookups: Mutex<Vec<String>>,
        route_repo: Mutex<Vec<(ActivationKind, bool)>>,
        downstream: AtomicUsize,
    }
    impl ActivationTracker for HorizonTracker {
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
            let event_free = activation.evaluation_data().is_none();
            if key
                .downcast_ref::<DirectLocalIncludePackageHorizonKey>()
                .is_some()
            {
                self.horizon
                    .lock()
                    .unwrap()
                    .push((activation.kind(), event_free));
            } else if key
                .downcast_ref::<DirectLocalModuleInspectionKey>()
                .is_some()
            {
                self.inspection.lock().unwrap().push(activation.kind());
            } else if let Some(key) = key.downcast_ref::<ExternalRepositoryPackageLookupKey>() {
                self.lookups.lock().unwrap().push(key.to_string());
            } else if key
                .downcast_ref::<crate::repo_file::HostRouteRepoFileKey>()
                .is_some()
            {
                self.route_repo
                    .lock()
                    .unwrap()
                    .push((activation.kind(), event_free));
            }
        }
    }
    #[derive(Debug, Clone, Allocative)]
    struct HorizonCounterKey(#[allocative(skip)] Arc<HorizonTracker>);
    impl PartialEq for HorizonCounterKey {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for HorizonCounterKey {}
    impl Hash for HorizonCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            Arc::as_ptr(&self.0).hash(state);
        }
    }
    impl fmt::Display for HorizonCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "direct-local-include-horizon-counter:{:p}", &self.0)
        }
    }
    #[async_trait]
    impl Key for HorizonCounterKey {
        type Value = <DirectLocalIncludePackageHorizonKey as Key>::Value;
        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            let value = ctx
                .compute(&horizon())
                .await
                .expect("horizon DICE invariant");
            if matches!(&value, SourcePreparationOutcome::Complete(result) if result.is_ok()) {
                self.0.downstream.fetch_add(1, Ordering::SeqCst);
            }
            value
        }
        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }
    #[derive(Debug, Default)]
    struct PreparationTracker {
        preparation: Mutex<Vec<(ActivationKind, bool)>>,
        lookups: Mutex<Vec<String>>,
        sources: Mutex<Vec<String>>,
        route_repo: Mutex<Vec<(ActivationKind, bool)>>,
        downstream: AtomicUsize,
    }
    impl ActivationTracker for PreparationTracker {
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
            let event_free = activation.evaluation_data().is_none();
            if key
                .downcast_ref::<DirectLocalModulePreparationKey>()
                .is_some()
            {
                self.preparation
                    .lock()
                    .unwrap()
                    .push((activation.kind(), event_free));
            } else if let Some(key) = key.downcast_ref::<ExternalRepositoryPackageLookupKey>() {
                self.lookups.lock().unwrap().push(key.to_string());
            } else if let Some(key) = key.downcast_ref::<HostRepositorySourceFileKey>() {
                self.sources.lock().unwrap().push(key.to_string());
            } else if key
                .downcast_ref::<crate::repo_file::HostRouteRepoFileKey>()
                .is_some()
            {
                self.route_repo
                    .lock()
                    .unwrap()
                    .push((activation.kind(), event_free));
            }
        }
    }
    #[derive(Debug, Clone, Allocative)]
    struct PreparationCounterKey(#[allocative(skip)] Arc<PreparationTracker>);
    impl PartialEq for PreparationCounterKey {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for PreparationCounterKey {}
    impl Hash for PreparationCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            Arc::as_ptr(&self.0).hash(state);
        }
    }
    impl fmt::Display for PreparationCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "direct-local-preparation-counter:{:p}", &self.0)
        }
    }
    #[async_trait]
    impl Key for PreparationCounterKey {
        type Value = <DirectLocalModulePreparationKey as Key>::Value;
        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            let value = ctx
                .compute(&preparation())
                .await
                .expect("preparation DICE invariant");
            if matches!(&value, SourcePreparationOutcome::Complete(result) if result.is_ok()) {
                self.0.downstream.fetch_add(1, Ordering::SeqCst);
            }
            value
        }
        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct EvaluationActivation {
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }
    #[derive(Debug, Default)]
    struct EvaluationTracker {
        evaluation: Mutex<Vec<EvaluationActivation>>,
        preparation: Mutex<Vec<EvaluationActivation>>,
        route_repo: Mutex<Vec<EvaluationActivation>>,
        downstream: AtomicUsize,
    }
    impl ActivationTracker for EvaluationTracker {
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
            let record = || EvaluationActivation {
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            };
            if key
                .downcast_ref::<DirectLocalModuleEvaluationKey>()
                .is_some()
            {
                self.evaluation.lock().unwrap().push(record());
            } else if key
                .downcast_ref::<DirectLocalModulePreparationKey>()
                .is_some()
            {
                self.preparation.lock().unwrap().push(record());
            } else if key
                .downcast_ref::<crate::repo_file::HostRouteRepoFileKey>()
                .is_some()
            {
                self.route_repo.lock().unwrap().push(record());
            }
        }
    }
    #[derive(Debug, Clone, Allocative)]
    struct EvaluationCounterKey(#[allocative(skip)] Arc<EvaluationTracker>);
    impl PartialEq for EvaluationCounterKey {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for EvaluationCounterKey {}
    impl Hash for EvaluationCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            Arc::as_ptr(&self.0).hash(state);
        }
    }
    impl fmt::Display for EvaluationCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "direct-local-evaluation-counter:{:p}", &self.0)
        }
    }
    #[async_trait]
    impl Key for EvaluationCounterKey {
        type Value = <DirectLocalModuleEvaluationKey as Key>::Value;
        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            let value = ctx
                .compute(&evaluation())
                .await
                .expect("evaluation DICE invariant");
            if matches!(&value, SourcePreparationOutcome::Complete(result) if result.is_ok()) {
                self.0.downstream.fetch_add(1, Ordering::SeqCst);
            }
            value
        }
        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }
    fn local_route_with_path(path: &str) -> RootRepositoryRoute {
        RootRepositoryRoute::for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::new("path"),
                    OverrideAttributeValue::String(path.into()),
                )])),
            },
        )
    }

    fn local_route() -> RootRepositoryRoute {
        local_route_with_path("dep")
    }

    #[test]
    fn builtin_route_fails_before_host_materialization() {
        let route = RootRepositoryRoute::builtin_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
        );
        let error = host_repository_materialization_request(&route).unwrap_err();
        assert!(matches!(error, RepositoryMaterializationError::Spec(_)));
    }

    fn immutable_route() -> RootRepositoryRoute {
        RootRepositoryRoute::for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::default(),
            },
        )
    }

    fn host_source_value(path: &str, bytes: &[u8]) -> HostRepositorySourceFileValue {
        HostRepositorySourceFileValue::Present {
            bytes: Arc::from(bytes),
            logical_path: NormalizedAbsolutePath::new(path).unwrap(),
        }
    }

    fn immutable(root: &str, instance: u64) -> RepositoryMaterialization {
        RepositoryMaterialization::Immutable {
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            repo_spec: RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::default(),
            },
            source_identity: Arc::from("fixed-content"),
            generation_root: PathBuf::from(root),
            observation_instance: PathObservationInstanceId::new(instance),
        }
    }

    #[test]
    fn immutable_materialization_equality_is_operationally_exact() {
        let left = Arc::new(Ok(immutable("/tmp/generation-a", 1)));
        let right = Arc::new(Ok(immutable("/tmp/generation-b", 2)));

        assert_ne!(left, right);
        assert!(!RepositoryMaterializationKey::equality(
            &SourcePreparationOutcome::Complete(left),
            &SourcePreparationOutcome::Complete(right),
        ));

        let same_bytes = SourcePreparationOutcome::Complete(Ok(
            RepositorySourceFileValue::Present(Arc::from(&b"same"[..])),
        ));
        let changed_bytes = SourcePreparationOutcome::Complete(Ok(
            RepositorySourceFileValue::Present(Arc::from(&b"changed"[..])),
        ));
        assert!(RepositorySourceFileKey::equality(&same_bytes, &same_bytes));
        assert!(!RepositorySourceFileKey::equality(
            &same_bytes,
            &changed_bytes,
        ));
    }

    #[test]
    fn materialization_result_key_provides_exact_request_through_dyn_key() {
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::default(),
            },
            kind: RepositoryMaterializationKind::Immutable,
        });
        let key = DynKey::from_key(RepositoryMaterializationResultKey {
            request: request.dupe(),
        });

        let provided = key
            .request_value::<Arc<RepositoryMaterializationRequest>>()
            .expect("result key must provide its complete materialization request");
        assert!(Arc::ptr_eq(&provided, &request));
    }

    #[test]
    fn repository_source_file_key_provides_workspace_module_scope_through_dyn_key() {
        let first = DynKey::from_key(RepositorySourceFileKey {
            workspace: PathBuf::from("/workspace/./source/.."),
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from("MODULE.bazel"),
        });
        let second = DynKey::from_key(RepositorySourceFileKey {
            workspace: PathBuf::from("/workspace"),
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from("nested/BUILD.bazel"),
        });
        let invalid = DynKey::from_key(RepositorySourceFileKey {
            workspace: PathBuf::from("relative/workspace"),
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from("MODULE.bazel"),
        });
        let different_module = DynKey::from_key(RepositorySourceFileKey {
            workspace: PathBuf::from("/workspace"),
            module_name: "other".into(),
            repo_relative_path: PathBuf::from("MODULE.bazel"),
        });
        let different_workspace = DynKey::from_key(RepositorySourceFileKey {
            workspace: PathBuf::from("/other-workspace"),
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from("MODULE.bazel"),
        });
        let expected = RepositorySourceScope {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            module_name: "dep".into(),
        };

        assert_eq!(
            first.request_value::<RepositorySourceScope>(),
            Some(expected.clone())
        );
        assert_eq!(
            second.request_value::<RepositorySourceScope>(),
            Some(expected)
        );
        assert_eq!(invalid.request_value::<RepositorySourceScope>(), None);
        assert_ne!(
            different_module.request_value::<RepositorySourceScope>(),
            second.request_value::<RepositorySourceScope>()
        );
        assert_ne!(
            different_workspace.request_value::<RepositorySourceScope>(),
            second.request_value::<RepositorySourceScope>()
        );
    }

    #[tokio::test]
    async fn host_repository_source_requests_native_materialization_without_legacy_snapshot_keys() {
        let key =
            HostRepositorySourceFileKey::new(local_route(), PathBuf::from("nested/BUILD.bazel"));
        let scope = DynKey::from_key(key.clone())
            .request_value::<RepositorySourceScope>()
            .expect("host source key must expose its repository scope");
        assert_eq!(
            scope,
            RepositorySourceScope {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                module_name: "dep".into(),
            }
        );

        let tracker = Arc::new(HostSourceDependencyTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                RepositoryMaterializationResultEpoch::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    [],
                )
                .unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&key).await.unwrap();
        assert!(!HostRepositorySourceFileKey::validity(&outcome));
        let SourcePreparationOutcome::Need(needs) = outcome else {
            panic!("an uninjected native repository result must request materialization");
        };
        let requests = needs.repository_materializations();
        assert_eq!(requests.len(), 1);
        let request = requests.values().next().unwrap();
        assert_eq!(
            request.as_ref(),
            &RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                    canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
                },
                repo_spec: local_route().repo_spec().clone(),
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
                },
            }
        );

        let dependencies = tracker.source_dependencies.lock().unwrap().clone();
        assert_eq!(
            dependencies,
            ["host-repository-path:@@dep+:nested/BUILD.bazel".to_owned()]
        );
        assert!(dependencies.iter().all(|dependency| {
            !dependency.starts_with("repository-materialization:")
                && !dependency.starts_with("repository-materialization-request:")
                && !dependency.starts_with("root-module-files:")
                && !dependency.starts_with("workspace-snapshot:")
        }));
        assert_eq!(
            *tracker.path_dependencies.lock().unwrap(),
            ["repository-materialization-result:@@dep+".to_owned()]
        );
    }

    #[tokio::test]
    async fn host_repository_source_rejects_invalid_relative_path_before_materialization() {
        let invalid_key =
            HostRepositorySourceFileKey::new(local_route(), PathBuf::from("../escape"));
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        let value = transaction.compute(&invalid_key).await.unwrap();
        assert!(matches!(
            value,
            SourcePreparationOutcome::Complete(Err(
                RepositorySourceFileError::InvalidRepoRelativePath { requested_path }
            )) if requested_path.as_path() == Path::new("../escape")
        ));
    }

    #[tokio::test]
    async fn host_repository_source_value_retains_requested_logical_path_and_bytes() {
        let route = local_route();
        let key = HostRepositorySourceFileKey::new(route.clone(), PathBuf::from("BUILD.bazel"));
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: route.repo_spec().clone(),
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
            },
        });
        let logical_path = NormalizedAbsolutePath::new("/workspace/dep/BUILD.bazel").unwrap();
        let physical_path = NormalizedAbsolutePath::new("/resolved/BUILD.bazel").unwrap();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                RepositoryMaterializationResultEpoch::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::empty(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let mut observations = Vec::new();

        for _ in 0..16 {
            let outcome = transaction.compute(&key).await.unwrap();
            match outcome {
                SourcePreparationOutcome::Complete(Ok(
                    HostRepositorySourceFileValue::Present {
                        bytes,
                        logical_path: observed_path,
                    },
                )) => {
                    assert_eq!(bytes.as_ref(), b"source");
                    assert_eq!(observed_path, logical_path);
                    return;
                }
                SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Absent)) => {
                    panic!("the injected source file must be present");
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    panic!("the injected source file must not error: {error:?}");
                }
                SourcePreparationOutcome::Need(needs) => {
                    let demands = needs
                        .path_observations()
                        .expect("materialization is injected, so only path observations remain");
                    for demand in demands.demands() {
                        assert_eq!(demand.namespace(), PathObservationNamespace::Host);
                        let result = match demand.operation() {
                            PathObservationOperation::Lstat if demand.path() == &logical_path => {
                                PathObservationResult::Lstat(PathOperationResult::Present(
                                    PathLstat::new(PathNodeKind::Symlink, 1, 2, 3, 4, 0o777),
                                ))
                            }
                            PathObservationOperation::Lstat if demand.path() == &physical_path => {
                                PathObservationResult::Lstat(PathOperationResult::Present(
                                    PathLstat::new(PathNodeKind::RegularFile, 6, 2, 3, 5, 0o644),
                                ))
                            }
                            PathObservationOperation::Lstat => {
                                PathObservationResult::Lstat(PathOperationResult::Present(
                                    PathLstat::new(PathNodeKind::Directory, 0, 2, 3, 6, 0o755),
                                ))
                            }
                            PathObservationOperation::ReadLink => {
                                assert_eq!(demand.path(), &logical_path);
                                PathObservationResult::ReadLink(PathOperationResult::Present(
                                    Arc::new(physical_path.as_path().to_owned()),
                                ))
                            }
                            PathObservationOperation::FileBytes => {
                                assert_eq!(demand.path(), &physical_path);
                                PathObservationResult::FileBytes(PathOperationResult::Present(
                                    Arc::from(&b"source"[..]),
                                ))
                            }
                            operation => panic!("unexpected source observation: {operation:?}"),
                        };
                        observations.push((demand.dupe(), result));
                    }
                    let mut updater = transaction.into_updater();
                    updater
                        .changed_to(vec![(
                            PathObservationEpochKey,
                            PathObservationEpoch::new(observations.clone()).unwrap(),
                        )])
                        .unwrap();
                    transaction = updater.commit().await;
                }
            }
        }
        panic!("the injected source file did not resolve");
    }

    #[test]
    fn host_repository_source_value_equality_requires_equal_bytes_and_logical_path() {
        let same = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/dep/BUILD.bazel",
            b"same",
        )));
        let equal = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/dep/BUILD.bazel",
            b"same",
        )));
        let different_path = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/other/BUILD.bazel",
            b"same",
        )));
        let different_bytes = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/dep/BUILD.bazel",
            b"changed",
        )));

        assert!(HostRepositorySourceFileKey::equality(&same, &equal));
        assert!(!HostRepositorySourceFileKey::equality(
            &same,
            &different_path
        ));
        assert!(!HostRepositorySourceFileKey::equality(
            &same,
            &different_bytes
        ));
    }

    #[test]
    fn host_repository_source_value_need_and_error_have_no_logical_path() {
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(
            RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                    canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
                },
                repo_spec: local_route().repo_spec().clone(),
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
                },
            },
        ));
        let error = RepositorySourceFileError::InvalidRepoRelativePath {
            requested_path: Arc::new(PathBuf::from("../BUILD.bazel")),
        };

        assert!(matches!(
            host_repository_source_file_value(need),
            SourcePreparationOutcome::Need(_),
        ));
        assert!(matches!(
            host_repository_source_file_value(SourcePreparationOutcome::Complete(Ok(
                ObservedRepositorySourceFile::Absent,
            ))),
            SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Absent)),
        ));
        let SourcePreparationOutcome::Complete(Err(actual)) = host_repository_source_file_value(
            SourcePreparationOutcome::Complete(Err(error.clone())),
        ) else {
            panic!("a source error must remain path-free and complete");
        };
        assert_eq!(actual, error);
    }

    #[test]
    fn legacy_immutable_repository_source_value_remains_bytes_only() {
        let first = SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Present(
            Arc::from(&b"same"[..]),
        )));
        let same_bytes_after_new_generation = SourcePreparationOutcome::Complete(Ok(
            RepositorySourceFileValue::Present(Arc::from(&b"same"[..])),
        ));
        let changed_bytes = SourcePreparationOutcome::Complete(Ok(
            RepositorySourceFileValue::Present(Arc::from(&b"changed"[..])),
        ));

        assert!(RepositorySourceFileKey::equality(
            &first,
            &same_bytes_after_new_generation,
        ));
        assert!(!RepositorySourceFileKey::equality(&first, &changed_bytes));
    }

    #[test]
    fn host_repository_source_local_override_root_change_is_distinct_key() {
        let first = HostRepositorySourceFileKey::new(
            local_route_with_path("dep"),
            PathBuf::from("BUILD.bazel"),
        );
        let second = HostRepositorySourceFileKey::new(
            local_route_with_path("other-dep"),
            PathBuf::from("BUILD.bazel"),
        );
        let first_value = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/dep/BUILD.bazel",
            b"same",
        )));
        let second_value = SourcePreparationOutcome::Complete(Ok(host_source_value(
            "/workspace/other-dep/BUILD.bazel",
            b"same",
        )));

        assert_ne!(first, second);
        assert!(!HostRepositorySourceFileKey::equality(
            &first_value,
            &second_value,
        ));
    }

    #[test]
    fn result_key_equality_is_exact_while_hash_intentionally_collides_across_repo_specs() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let request = RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace,
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::default(),
            },
            kind: RepositoryMaterializationKind::Immutable,
        };
        let equal = RepositoryMaterializationResultKey {
            request: Arc::new(request.clone()),
        };
        let mut distinct_request = request.clone();
        distinct_request.repo_spec.rule_id.rule_name = "git_repository".into();
        let distinct = RepositoryMaterializationResultKey {
            request: Arc::new(distinct_request),
        };
        let key = RepositoryMaterializationResultKey {
            request: Arc::new(request),
        };
        let hash = |key: &RepositoryMaterializationResultKey| {
            let mut state = DefaultHasher::new();
            key.hash(&mut state);
            state.finish()
        };

        assert_eq!(key, equal);
        assert_eq!(hash(&key), hash(&equal));
        assert_ne!(key, distinct);
        assert_eq!(hash(&key), hash(&distinct));
    }

    fn direct() -> DirectLocalModuleFileKey {
        DirectLocalModuleFileKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
        )
        .unwrap()
    }
    fn inspection() -> DirectLocalModuleInspectionKey {
        DirectLocalModuleInspectionKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
        )
        .unwrap()
    }
    fn horizon() -> DirectLocalIncludePackageHorizonKey {
        DirectLocalIncludePackageHorizonKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
        )
        .unwrap()
    }
    fn preparation() -> DirectLocalModulePreparationKey {
        DirectLocalModulePreparationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
        )
        .unwrap()
    }
    fn evaluation() -> DirectLocalModuleEvaluationKey {
        DirectLocalModuleEvaluationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
        )
        .unwrap()
    }
    fn root(path: &str, version: &str) -> String {
        format!(
            "bazel_dep(name = \"dep\", version = \"{version}\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"{path}\")\n"
        )
    }
    fn epoch(root: &str, path: &str, file: Option<&[u8]>) -> PathObservationEpoch {
        let d = |p, o| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(p).unwrap(),
                o,
            )
        };
        let n = |k| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                k, 1, 1, 1, 1, 0o755,
            )))
        };
        let file_path = format!("/workspace/{path}/MODULE.bazel");
        let dir = format!("/workspace/{path}");
        let mut e = SmallMap::from_iter([
            (
                d("/", PathObservationOperation::Lstat),
                n(PathNodeKind::Directory),
            ),
            (
                d("/workspace", PathObservationOperation::Lstat),
                n(PathNodeKind::Directory),
            ),
            (
                d("/workspace/MODULE.bazel", PathObservationOperation::Lstat),
                n(PathNodeKind::RegularFile),
            ),
            (
                d(
                    "/workspace/MODULE.bazel",
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    root.as_bytes(),
                ))),
            ),
            (
                d(&dir, PathObservationOperation::Lstat),
                n(PathNodeKind::Directory),
            ),
        ]);
        e.insert(
            d(&file_path, PathObservationOperation::Lstat),
            file.map(|_| n(PathNodeKind::RegularFile))
                .unwrap_or(PathObservationResult::Lstat(PathOperationResult::Missing)),
        );
        if let Some(bytes) = file {
            e.insert(
                d(&file_path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
            );
        }
        PathObservationEpoch::new(e).unwrap()
    }
    fn horizon_epoch(
        root_source: &str,
        namespace: PathObservationNamespace,
        route_path: &str,
        module: Option<&[u8]>,
        repo: Option<&[u8]>,
        ignore: Option<&[u8]>,
        packages: &[(&str, bool)],
        omitted: &[(&str, &str)],
        directory_markers: &[(&str, &str)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
    ) -> PathObservationEpoch {
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let lstat = |kind| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            )))
        };
        let mut observations = SmallMap::new();
        for directory in ["/", "/workspace", route_path] {
            observations.insert(
                demand(directory, PathObservationOperation::Lstat),
                lstat(PathNodeKind::Directory),
            );
        }
        for ancestor in Path::new(route_path).ancestors().skip(1) {
            observations.insert(
                demand(
                    ancestor.to_str().expect("test route paths are UTF-8"),
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::Directory),
            );
        }
        observations.insert(
            demand("/workspace/MODULE.bazel", PathObservationOperation::Lstat),
            lstat(PathNodeKind::RegularFile),
        );
        observations.insert(
            demand(
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                root_source.as_bytes(),
            ))),
        );
        let module_path = format!("{route_path}/MODULE.bazel");
        observations.insert(
            demand(&module_path, PathObservationOperation::Lstat),
            module
                .map(|_| lstat(PathNodeKind::RegularFile))
                .unwrap_or(PathObservationResult::Lstat(PathOperationResult::Missing)),
        );
        if let Some(module) = module {
            observations.insert(
                demand(&module_path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(module))),
            );
        }
        for (name, source) in [("REPO.bazel", repo), (".bazelignore", ignore)] {
            let path = format!("{route_path}/{name}");
            observations.insert(
                demand(&path, PathObservationOperation::Lstat),
                source
                    .map(|_| lstat(PathNodeKind::RegularFile))
                    .unwrap_or(PathObservationResult::Lstat(PathOperationResult::Missing)),
            );
            if let Some(source) = source {
                observations.insert(
                    demand(&path, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        source,
                    ))),
                );
            }
        }
        for (package, selected) in packages {
            let package_root = format!("{route_path}/{package}");
            let mut ancestor = route_path.to_owned();
            for segment in package.split('/') {
                ancestor.push('/');
                ancestor.push_str(segment);
                observations.insert(
                    demand(&ancestor, PathObservationOperation::Lstat),
                    lstat(PathNodeKind::Directory),
                );
            }
            for marker in ["BUILD.bazel", "BUILD"] {
                if omitted.contains(&(*package, marker)) {
                    continue;
                }
                let path = format!("{package_root}/{marker}");
                observations.insert(
                    demand(&path, PathObservationOperation::Lstat),
                    if directory_markers.contains(&(*package, marker)) {
                        lstat(PathNodeKind::Directory)
                    } else if *selected && marker == "BUILD.bazel" {
                        lstat(PathNodeKind::RegularFile)
                    } else {
                        PathObservationResult::Lstat(PathOperationResult::Missing)
                    },
                );
            }
        }
        for (relative, source) in fragments {
            let path = format!("{route_path}/{relative}");
            observations.insert(
                demand(&path, PathObservationOperation::Lstat),
                source
                    .map(|_| lstat(PathNodeKind::RegularFile))
                    .unwrap_or(PathObservationResult::Lstat(PathOperationResult::Missing)),
            );
            if let Some(source) = source {
                observations.insert(
                    demand(&path, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        *source,
                    ))),
                );
            }
        }
        for relative in fragment_needs {
            observations.insert(
                demand(
                    &format!("{route_path}/{relative}"),
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::RegularFile),
            );
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn horizon_compute(
        dice: &Arc<Dice>,
        route_path: &str,
        module: Option<&[u8]>,
        repo: Option<&[u8]>,
        ignore: Option<&[u8]>,
        packages: &[(&str, bool)],
        omitted: &[(&str, &str)],
        deleted: &[&str],
        variant: i64,
        capture: bool,
        tracker: Option<Arc<HorizonTracker>>,
    ) -> <DirectLocalIncludePackageHorizonKey as Key>::Value {
        let mut data = UserComputationData {
            activation_tracker: tracker
                .clone()
                .map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture {
            data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(data);
        let relative = route_path.strip_prefix("/workspace/").unwrap();
        let root_source = root(relative, &variant.to_string());
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    &root_source,
                    PathObservationNamespace::Host,
                    route_path,
                    module,
                    repo,
                    ignore,
                    packages,
                    omitted,
                    &[],
                    &[],
                    &[],
                    variant,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                material(relative),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                deleted,
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let direct = transaction.compute(&horizon()).await.unwrap();
        if let Some(tracker) = tracker {
            transaction
                .compute(&HorizonCounterKey(tracker))
                .await
                .unwrap()
        } else {
            direct
        }
    }
    async fn preparation_compute(
        dice: &Arc<Dice>,
        route_path: &str,
        module: Option<&[u8]>,
        repo: Option<&[u8]>,
        packages: &[(&str, bool)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        deleted: &[&str],
        variant: i64,
        capture: bool,
        tracker: Option<Arc<PreparationTracker>>,
    ) -> <DirectLocalModulePreparationKey as Key>::Value {
        let mut data = UserComputationData {
            activation_tracker: tracker
                .clone()
                .map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture {
            data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(data);
        let relative = route_path.strip_prefix("/workspace/").unwrap();
        let root_source = root(relative, &variant.to_string());
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    &root_source,
                    PathObservationNamespace::Host,
                    route_path,
                    module,
                    repo,
                    None,
                    packages,
                    &[],
                    &[],
                    fragments,
                    fragment_needs,
                    variant,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                material(relative),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                deleted,
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let direct = transaction.compute(&preparation()).await.unwrap();
        if let Some(tracker) = tracker {
            transaction
                .compute(&PreparationCounterKey(tracker))
                .await
                .unwrap()
        } else {
            direct
        }
    }
    async fn evaluation_transaction(
        dice: &Arc<Dice>,
        module: Option<&[u8]>,
        repo: Option<&[u8]>,
        packages: &[(&str, bool)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        capture: bool,
        tracker: Option<Arc<EvaluationTracker>>,
    ) -> dice::DiceTransaction {
        let mut data = UserComputationData {
            activation_tracker: tracker
                .clone()
                .map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture {
            data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(data);
        let root_source = root("dep", &variant.to_string());
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    &root_source,
                    PathObservationNamespace::Host,
                    "/workspace/dep",
                    module,
                    repo,
                    None,
                    packages,
                    &[],
                    &[],
                    fragments,
                    fragment_needs,
                    variant,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                material("dep"),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        updater.commit().await
    }
    async fn evaluation_compute(
        dice: &Arc<Dice>,
        module: Option<&[u8]>,
        repo: Option<&[u8]>,
        packages: &[(&str, bool)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        capture: bool,
        tracker: Option<Arc<EvaluationTracker>>,
    ) -> <DirectLocalModuleEvaluationKey as Key>::Value {
        let mut transaction = evaluation_transaction(
            dice,
            module,
            repo,
            packages,
            fragments,
            fragment_needs,
            variant,
            capture,
            tracker.clone(),
        )
        .await;
        let direct = transaction.compute(&evaluation()).await.unwrap();
        if let Some(tracker) = tracker {
            transaction
                .compute(&EvaluationCounterKey(tracker))
                .await
                .unwrap()
        } else {
            direct
        }
    }
    async fn support_compute(
        dice: &Arc<Dice>,
        module: Option<&[u8]>,
        packages: &[(&str, bool)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        tracker: Option<Arc<EvaluationTracker>>,
    ) -> SourcePreparationOutcome<
        Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>>,
    > {
        let mut transaction = evaluation_transaction(
            dice,
            module,
            None,
            packages,
            fragments,
            fragment_needs,
            variant,
            true,
            tracker,
        )
        .await;
        direct_local_module_support(&mut transaction, &local_route()).await
    }
    fn preparation_success(
        value: <DirectLocalModulePreparationKey as Key>::Value,
    ) -> DirectLocalModulePreparation {
        match value {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap().clone(),
            SourcePreparationOutcome::Need(_) => panic!("complete direct-local preparation"),
        }
    }
    fn preparation_failure(
        value: <DirectLocalModulePreparationKey as Key>::Value,
    ) -> DirectLocalModulePreparationError {
        match value {
            SourcePreparationOutcome::Complete(value) => {
                value.as_ref().as_ref().unwrap_err().clone()
            }
            SourcePreparationOutcome::Need(_) => panic!("terminal direct-local preparation"),
        }
    }
    fn evaluation_success(
        value: <DirectLocalModuleEvaluationKey as Key>::Value,
    ) -> DirectLocalModuleEvaluation {
        match value {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap().clone(),
            SourcePreparationOutcome::Need(_) => panic!("complete direct-local evaluation"),
        }
    }
    fn evaluation_failure(
        value: <DirectLocalModuleEvaluationKey as Key>::Value,
    ) -> DirectLocalModuleEvaluationError {
        match value {
            SourcePreparationOutcome::Complete(value) => {
                value.as_ref().as_ref().unwrap_err().clone()
            }
            SourcePreparationOutcome::Need(_) => panic!("terminal direct-local evaluation"),
        }
    }
    fn inputs(u: &mut dice::DiceTransactionUpdater) {
        inject_root_package_policy_inputs(
            u,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            u,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
    }
    fn root_only(root: &str) -> PathObservationEpoch {
        let d = |p, o| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(p).unwrap(),
                o,
            )
        };
        let n = |k| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                k, 1, 1, 1, 1, 0o755,
            )))
        };
        PathObservationEpoch::new([
            (
                d("/", PathObservationOperation::Lstat),
                n(PathNodeKind::Directory),
            ),
            (
                d("/workspace", PathObservationOperation::Lstat),
                n(PathNodeKind::Directory),
            ),
            (
                d("/workspace/MODULE.bazel", PathObservationOperation::Lstat),
                n(PathNodeKind::RegularFile),
            ),
            (
                d(
                    "/workspace/MODULE.bazel",
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    root.as_bytes(),
                ))),
            ),
        ])
        .unwrap()
    }
    fn missing_root() -> PathObservationEpoch {
        let d = |p| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(p).unwrap(),
                PathObservationOperation::Lstat,
            )
        };
        let n = |p| {
            (
                d(p),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::Directory,
                    1,
                    1,
                    1,
                    1,
                    0o755,
                ))),
            )
        };
        PathObservationEpoch::new([
            n("/"),
            n("/workspace"),
            (
                d("/workspace/MODULE.bazel"),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ),
        ])
        .unwrap()
    }
    fn material(path: &str) -> RepositoryMaterializationResultEpoch {
        let route = local_route_with_path(path);
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: route.repo_spec().clone(),
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(format!("/workspace/{path}")).unwrap(),
            },
        });
        RepositoryMaterializationResultEpoch::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            [RepositoryMaterializationEpochEntry {
                request,
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap()
    }

    fn immutable_material(
        generation_root: &str,
        observation_instance: PathObservationInstanceId,
    ) -> RepositoryMaterializationResultEpoch {
        let route = immutable_route();
        RepositoryMaterializationResultEpoch::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            [RepositoryMaterializationEpochEntry {
                request: Arc::new(RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: route.workspace().dupe(),
                        canonical_repo: route.canonical_repo().clone(),
                    },
                    repo_spec: route.repo_spec().clone(),
                    kind: RepositoryMaterializationKind::Immutable,
                }),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        source_identity: Arc::from("fixed-content"),
                        generation_root: PathBuf::from(generation_root),
                        observation_instance,
                    },
                ),
            }],
        )
        .unwrap()
    }

    fn host_path_epoch(
        namespace: PathObservationNamespace,
        requested: &str,
        kind: Option<PathNodeKind>,
        bytes: Option<&[u8]>,
    ) -> PathObservationEpoch {
        let requested = NormalizedAbsolutePath::new(requested).unwrap();
        let demand = |path: NormalizedAbsolutePath, operation| {
            PathObservationDemand::new(namespace, path, operation)
        };
        let lstat = |kind| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 2, 3, 4, 0o755,
            )))
        };
        let mut observations = SmallMap::new();
        for ancestor in requested.as_path().ancestors().skip(1) {
            observations.insert(
                demand(
                    NormalizedAbsolutePath::new(ancestor.to_owned()).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::Directory),
            );
        }
        observations.insert(
            demand(requested.dupe(), PathObservationOperation::Lstat),
            kind.map(lstat)
                .unwrap_or(PathObservationResult::Lstat(PathOperationResult::Missing)),
        );
        if let Some(bytes) = bytes {
            observations.insert(
                demand(requested, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
            );
        }
        PathObservationEpoch::new(observations).unwrap()
    }

    fn symlink_path_epoch(requested: &str, target: &str) -> PathObservationEpoch {
        let requested = NormalizedAbsolutePath::new(requested).unwrap();
        let target = NormalizedAbsolutePath::new(target).unwrap();
        let demand = |path: NormalizedAbsolutePath, operation| {
            PathObservationDemand::new(PathObservationNamespace::Host, path, operation)
        };
        let lstat = |kind| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 2, 3, 4, 0o755,
            )))
        };
        let mut observations = SmallMap::new();
        for path in [requested.as_path(), target.as_path()] {
            for ancestor in path.ancestors().skip(1) {
                observations.insert(
                    demand(
                        NormalizedAbsolutePath::new(ancestor.to_owned()).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    lstat(PathNodeKind::Directory),
                );
            }
        }
        observations.insert(
            demand(requested.dupe(), PathObservationOperation::Lstat),
            lstat(PathNodeKind::Symlink),
        );
        observations.insert(
            demand(requested, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(
                target.as_path().to_owned(),
            ))),
        );
        observations.insert(
            demand(target, PathObservationOperation::Lstat),
            lstat(PathNodeKind::RegularFile),
        );
        PathObservationEpoch::new(observations).unwrap()
    }

    async fn host_path_transaction(
        dice: &Arc<Dice>,
        materialization: RepositoryMaterializationResultEpoch,
        observations: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, observations)])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                materialization,
            )])
            .unwrap();
        updater.commit().await
    }

    fn resolved_kind(path: &HostRepositoryPathValue) -> Option<PathNodeKind> {
        match path.resolved().state() {
            ResolvedPathState::Missing => None,
            ResolvedPathState::Present(lstat) => Some(lstat.kind()),
        }
    }

    #[tokio::test]
    async fn host_repository_path_completes_all_kinds_before_source_file_bytes() {
        for kind in [
            None,
            Some(PathNodeKind::RegularFile),
            Some(PathNodeKind::SpecialFile),
            Some(PathNodeKind::Directory),
        ] {
            let dice = Dice::builder().build(DetectCycles::Enabled);
            let mut transaction = host_path_transaction(
                &dice,
                material("dep"),
                host_path_epoch(
                    PathObservationNamespace::Host,
                    "/workspace/dep/BUILD.bazel",
                    kind,
                    None,
                ),
            )
            .await;
            let route = local_route();
            let path_key = HostRepositoryPathKey::new(route.clone(), PathBuf::from("BUILD.bazel"));
            let SourcePreparationOutcome::Complete(Ok(path)) =
                transaction.compute(&path_key).await.unwrap()
            else {
                panic!("path-only lookup must complete without FileBytes");
            };
            assert_eq!(path.resolved().namespace(), PathObservationNamespace::Host);
            assert_eq!(
                path.resolved().requested_path().as_path(),
                Path::new("/workspace/dep/BUILD.bazel")
            );
            assert_eq!(resolved_kind(&path), kind);

            let source = transaction
                .compute(&HostRepositorySourceFileKey::new(
                    route,
                    PathBuf::from("BUILD.bazel"),
                ))
                .await
                .unwrap();
            match kind {
                None => assert!(matches!(
                    source,
                    SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Absent))
                )),
                Some(PathNodeKind::RegularFile | PathNodeKind::SpecialFile) => {
                    let needs = need(source);
                    let demands = needs.path_observations().unwrap().demands();
                    assert_eq!(demands.len(), 1);
                    assert_eq!(demands[0].operation(), PathObservationOperation::FileBytes);
                    assert_eq!(
                        demands[0].path().as_path(),
                        Path::new("/workspace/dep/BUILD.bazel")
                    );
                }
                Some(actual) => assert!(matches!(
                    source,
                    SourcePreparationOutcome::Complete(Err(
                        RepositorySourceFileError::WrongKind { actual: found, .. }
                    )) if found == actual
                )),
            }
        }
    }

    #[tokio::test]
    async fn host_repository_path_tracks_symlink_retarget_and_create_delete_recovery() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let key = HostRepositoryPathKey::new(local_route(), PathBuf::from("link"));
        for target in ["/physical/a", "/physical/b"] {
            let mut transaction = host_path_transaction(
                &dice,
                material("dep"),
                symlink_path_epoch("/workspace/dep/link", target),
            )
            .await;
            let SourcePreparationOutcome::Complete(Ok(path)) =
                transaction.compute(&key).await.unwrap()
            else {
                panic!("symlink path must resolve");
            };
            assert_eq!(path.resolved().real_path().as_path(), Path::new(target));
            assert_eq!(resolved_kind(&path), Some(PathNodeKind::RegularFile));
        }

        let file_key = HostRepositoryPathKey::new(local_route(), PathBuf::from("created"));
        for kind in [
            Some(PathNodeKind::RegularFile),
            None,
            Some(PathNodeKind::RegularFile),
        ] {
            let mut transaction = host_path_transaction(
                &dice,
                material("dep"),
                host_path_epoch(
                    PathObservationNamespace::Host,
                    "/workspace/dep/created",
                    kind,
                    None,
                ),
            )
            .await;
            let SourcePreparationOutcome::Complete(Ok(path)) =
                transaction.compute(&file_key).await.unwrap()
            else {
                panic!("create/delete lifecycle must remain complete");
            };
            assert_eq!(resolved_kind(&path), kind);
        }
    }

    #[tokio::test]
    async fn host_repository_path_route_aba_and_immutable_identity_are_exact() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut seen = Vec::new();
        for root in ["dep-a", "dep-b", "dep-a"] {
            let route = local_route_with_path(root);
            let key = HostRepositoryPathKey::new(route, PathBuf::from("BUILD.bazel"));
            let expected = format!("/workspace/{root}/BUILD.bazel");
            let mut transaction = host_path_transaction(
                &dice,
                material(root),
                host_path_epoch(
                    PathObservationNamespace::Host,
                    &expected,
                    Some(PathNodeKind::RegularFile),
                    None,
                ),
            )
            .await;
            let SourcePreparationOutcome::Complete(Ok(path)) =
                transaction.compute(&key).await.unwrap()
            else {
                panic!("local route path must complete");
            };
            assert_eq!(
                path.resolved().requested_path().as_path(),
                Path::new(&expected)
            );
            seen.push((key, path));
        }
        assert_ne!(seen[0].0, seen[1].0);
        assert_eq!(seen[0].0, seen[2].0);
        assert_ne!(seen[0].1, seen[1].1);
        assert_eq!(seen[0].1, seen[2].1);

        let route = immutable_route();
        let key = HostRepositoryPathKey::new(route.clone(), PathBuf::from("BUILD.bazel"));
        assert_eq!(
            DynKey::from_key(key.clone()).request_value::<RepositorySourceScope>(),
            Some(RepositorySourceScope {
                workspace: route.workspace().dupe(),
                module_name: "dep".into(),
            })
        );
        let mut immutable_paths = Vec::new();
        for (generation_root, instance_id) in [("/generation/41", 41), ("/generation/42", 42)] {
            let instance = PathObservationInstanceId::new(instance_id);
            let namespace = PathObservationNamespace::Materialization(instance);
            let expected = format!("{generation_root}/BUILD.bazel");
            let mut transaction = host_path_transaction(
                &dice,
                immutable_material(generation_root, instance),
                host_path_epoch(namespace, &expected, Some(PathNodeKind::SpecialFile), None),
            )
            .await;
            let SourcePreparationOutcome::Complete(Ok(path)) =
                transaction.compute(&key).await.unwrap()
            else {
                panic!("immutable path must complete");
            };
            assert_eq!(path.resolved().namespace(), namespace);
            assert_eq!(
                path.resolved().requested_path().as_path(),
                Path::new(&expected)
            );
            assert_eq!(
                path.resolved().requested_path(),
                path.resolved().real_path()
            );
            assert_eq!(resolved_kind(&path), Some(PathNodeKind::SpecialFile));
            immutable_paths.push(path);
        }
        assert_ne!(immutable_paths[0], immutable_paths[1]);
    }
    async fn complete(
        dice: &Arc<Dice>,
        root_source: &str,
        path: &str,
        file: Option<&[u8]>,
        tracker: Option<Arc<DirectTracker>>,
    ) -> <DirectLocalModuleFileKey as Key>::Value {
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|t| t as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut u = dice.updater_with_data(data);
        u.changed_to(vec![(
            PathObservationEpochKey,
            epoch(root_source, path, file),
        )])
        .unwrap();
        u.changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            material(path),
        )])
        .unwrap();
        inputs(&mut u);
        u.commit().await.compute(&direct()).await.unwrap()
    }
    fn success(v: <DirectLocalModuleFileKey as Key>::Value) -> DirectLocalModuleFile {
        match v {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap().clone(),
            _ => panic!("complete direct local source"),
        }
    }
    async fn inspect_complete(
        dice: &Arc<Dice>,
        root_source: &str,
        path: &str,
        file: Option<&[u8]>,
        tracker: Option<Arc<InspectionTracker>>,
    ) -> <DirectLocalModuleInspectionKey as Key>::Value {
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|t| t as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut u = dice.updater_with_data(data);
        u.changed_to(vec![(
            PathObservationEpochKey,
            epoch(root_source, path, file),
        )])
        .unwrap();
        u.changed_to(vec![(
            (RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            }),
            material(path),
        )])
        .unwrap();
        inputs(&mut u);
        u.commit().await.compute(&inspection()).await.unwrap()
    }
    fn inspection_success(
        value: <DirectLocalModuleInspectionKey as Key>::Value,
    ) -> DirectLocalModuleInspection {
        match value {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap().clone(),
            _ => panic!("complete direct local inspection"),
        }
    }
    fn need<T>(value: SourcePreparationOutcome<T>) -> SourcePreparationNeeds {
        match value {
            SourcePreparationOutcome::Need(need) => need,
            _ => panic!("source preparation Need"),
        }
    }
    fn horizon_success(
        value: <DirectLocalIncludePackageHorizonKey as Key>::Value,
    ) -> DirectLocalIncludePackageHorizon {
        match value {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap().clone(),
            _ => panic!("complete direct-local package horizon"),
        }
    }
    fn horizon_occurrence(package: &str, line: u32) -> DirectLocalIncludePackageOccurrence {
        DirectLocalIncludePackageOccurrence {
            package: PackageIdentifier::new(
                CanonicalRepoName::new("dep+").unwrap(),
                slug_identity_v2::PackagePath::parse(package).unwrap(),
            ),
            target: TargetName::parse("nested.MODULE.bazel").unwrap(),
            raw_label: format!("//{package}:nested.MODULE.bazel").into(),
            location: crate::LogicalSpan {
                file: crate::LogicalModuleFileId::new("dep/MODULE.bazel"),
                start_line: line,
                start_column: 3,
                end_line: line,
                end_column: 20,
            },
        }
    }
    fn lookup_complete(
        value: Result<ExternalRepositoryPackageLookup, ExternalRepositoryPackageLookupError>,
    ) -> Result<<ExternalRepositoryPackageLookupKey as Key>::Value, Arc<str>> {
        Ok(SourcePreparationOutcome::Complete(Arc::new(value)))
    }

    #[test]
    fn direct_identity_and_typed_errors() {
        assert_eq!(
            direct().to_string(),
            "direct-local-module-file:\"/workspace\":@dep_alias"
        );
        assert!(
            DirectLocalModuleFileKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                ApparentRepoName::root()
            )
            .is_err()
        );
        assert_ne!(
            DirectLocalModuleFileError::RouteCompute(Arc::from("r")),
            DirectLocalModuleFileError::SourceCompute(Arc::from("s"))
        );
    }

    #[tokio::test]
    async fn direct_local_success_lifecycle_a_b_a() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let a = success(complete(&dice, &root("dep-a", "1.0"), "dep-a", Some(b"one"), None).await);
        let b = success(complete(&dice, &root("dep-b", "1.0"), "dep-b", Some(b"two"), None).await);
        let edited =
            success(complete(&dice, &root("dep-a", "1.0"), "dep-a", Some(b"edited"), None).await);
        let absent = success(complete(&dice, &root("dep-a", "1.0"), "dep-a", None, None).await);
        let recreated = success(
            complete(
                &dice,
                &root("dep-a", "1.0"),
                "dep-a",
                Some(b"recreated"),
                None,
            )
            .await,
        );
        assert_eq!(a.0, edited.0);
        assert_ne!(a.0, b.0);
        assert!(
            matches!(&a.1, HostRepositorySourceFileValue::Present { bytes, logical_path } if bytes.as_ref()==b"one" && logical_path==&NormalizedAbsolutePath::new("/workspace/dep-a/MODULE.bazel").unwrap())
        );
        assert!(matches!(
            &edited.1,
            HostRepositorySourceFileValue::Present { bytes, .. } if bytes.as_ref()==b"edited"
        ));
        assert!(matches!(absent.1, HostRepositorySourceFileValue::Absent));
        assert_eq!(a.0, absent.0);
        assert!(matches!(
            &recreated.1,
            HostRepositorySourceFileValue::Present { bytes, .. } if bytes.as_ref()==b"recreated"
        ));
        assert_eq!(a.0, recreated.0);
    }

    #[tokio::test]
    async fn direct_local_success_version_edit_reuses_without_event_data() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let t = Arc::new(DirectTracker::default());
        let first = success(
            complete(
                &dice,
                &root("dep-a", "1.0"),
                "dep-a",
                Some(b"one"),
                Some(t.clone()),
            )
            .await,
        );
        let second = success(
            complete(
                &dice,
                &root("dep-a", "2.0"),
                "dep-a",
                Some(b"one"),
                Some(t.clone()),
            )
            .await,
        );
        assert_eq!(first, second);
        assert_eq!(
            *t.0.lock().unwrap(),
            [
                (ActivationKind::Evaluated, true),
                (ActivationKind::Reused, true)
            ]
        );
    }

    #[tokio::test]
    async fn direct_local_real_errors_and_exact_need() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut u = dice.updater();
        u.changed_to(vec![(
            PathObservationEpochKey,
            epoch(&root("dep", "1"), "dep", Some(b"x")),
        )])
        .unwrap();
        inputs(&mut u);
        let mut x = u.commit().await;
        let need = x.compute(&direct()).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let unknown=complete(&dice,"bazel_dep(name = \"dep\", repo_name = \"other\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n","dep",Some(b"x"),None).await;
        assert!(
            matches!(unknown,SourcePreparationOutcome::Complete(v) if matches!(v.as_ref(),Err(DirectLocalModuleFileError::Route(_))))
        );
        let source=complete(&dice,"bazel_dep(name = \"dep\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"../dep\")\n","dep",Some(b"x"),None).await;
        assert!(
            matches!(source,SourcePreparationOutcome::Complete(v) if matches!(v.as_ref(),Err(DirectLocalModuleFileError::Source(_))))
        );
    }

    #[test]
    fn direct_structural_scan() {
        let s = include_str!("source_preparation.rs");
        let d = s
            .split("struct DirectLocalModuleFileKey")
            .nth(1)
            .unwrap()
            .split("impl fmt::Display for RepositorySourceFileKey")
            .next()
            .unwrap();
        for x in [
            "ModuleSourcePreparationKey",
            "RootModuleFilesKey",
            "RegistryPolicyKey",
            "RegistryFileKey",
            "WorkspaceSnapshotKey",
            "RepositoryMaterializationRequestKey",
            "fault",
        ] {
            assert!(!d.contains(x));
        }
        assert!(
            d.contains("RootRepositoryRouteKey")
                && d.contains("HostRepositorySourceFileKey")
                && d.contains("MODULE.bazel")
        );
    }

    #[test]
    fn direct_value_complete_only_equality_and_need_identity() {
        let a = SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleFile(
            local_route(),
            host_source_value("/workspace/dep/MODULE.bazel", b"one"),
        ))));
        let b = SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleFile(
            local_route(),
            host_source_value("/workspace/dep/MODULE.bazel", b"one"),
        ))));
        let absent = SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleFile(
            local_route(),
            HostRepositorySourceFileValue::Absent,
        ))));
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::root_module_bootstrap(
            RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        ));
        assert!(DirectLocalModuleFileKey::equality(&a, &b));
        assert!(!DirectLocalModuleFileKey::equality(&a, &absent));
        assert!(DirectLocalModuleFileKey::validity(&absent));
        assert!(!DirectLocalModuleFileKey::validity(&need));
        assert!(!DirectLocalModuleFileKey::equality(&need, &need));
    }

    #[tokio::test]
    async fn direct_forwards_bootstrap_and_exact_route_source_needs() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut u = dice.updater();
        u.changed_to(vec![(PathObservationEpochKey, missing_root())])
            .unwrap();
        inputs(&mut u);
        let mut t = u.commit().await;
        let outer = t.compute(&direct()).await.unwrap();
        let route = t
            .compute(
                &RootRepositoryRouteKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    ApparentRepoName::new("dep_alias").unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let (SourcePreparationOutcome::Need(outer), SourcePreparationOutcome::Need(route)) =
            (outer, route)
        else {
            panic!("bootstrap Need")
        };
        assert_eq!(outer, route);
        let source = root("dep", "1");
        let mut u = t.into_updater();
        u.changed_to(vec![(PathObservationEpochKey, root_only(&source))])
            .unwrap();
        inputs(&mut u);
        let mut t = u.commit().await;
        let r = t
            .compute(
                &RootRepositoryRouteKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    ApparentRepoName::new("dep_alias").unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(r) = r else {
            panic!("route")
        };
        let r = r.as_ref().as_ref().unwrap().clone();
        let outer = t.compute(&direct()).await.unwrap();
        let child = t
            .compute(&HostRepositorySourceFileKey::new(
                r.clone(),
                PathBuf::from("MODULE.bazel"),
            ))
            .await
            .unwrap();
        let (SourcePreparationOutcome::Need(outer), SourcePreparationOutcome::Need(child)) =
            (outer, child)
        else {
            panic!("materialization Need")
        };
        assert_eq!(outer, child);
        let request = outer
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .dupe();
        assert_eq!(
            request.id.workspace,
            NormalizedAbsolutePath::new("/workspace").unwrap()
        );
        assert_eq!(
            request.id.canonical_repo,
            CanonicalRepoName::new("dep+").unwrap()
        );
        assert_eq!(
            request.kind,
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap()
            }
        );
        assert_eq!(request.repo_spec, r.repo_spec().clone());
        let mut u = t.into_updater();
        u.changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            material("dep"),
        )])
        .unwrap();
        let mut t = u.commit().await;
        let outer = t.compute(&direct()).await.unwrap();
        let child = t
            .compute(&HostRepositorySourceFileKey::new(
                r,
                PathBuf::from("MODULE.bazel"),
            ))
            .await
            .unwrap();
        let (SourcePreparationOutcome::Need(outer), SourcePreparationOutcome::Need(child)) =
            (outer, child)
        else {
            panic!("path Need")
        };
        assert_eq!(outer, child);
    }

    #[tokio::test]
    async fn direct_projects_unknown_nodep_nonlocal_and_source_errors() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let check = |v: <DirectLocalModuleFileKey as Key>::Value| matches!(v,SourcePreparationOutcome::Complete(x) if matches!(x.as_ref(),Err(DirectLocalModuleFileError::Route(_))));
        assert!(check(complete(&dice,"bazel_dep(name = \"dep\", repo_name = \"other\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n","dep",Some(b"x"),None).await));
        assert!(check(complete(&dice,"bazel_dep(name = \"dep\", repo_name = \"dep_alias\", nodep = True)\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n","dep",Some(b"x"),None).await));
        assert!(check(
            complete(
                &dice,
                "bazel_dep(name = \"dep\", repo_name = \"dep_alias\")\n",
                "dep",
                Some(b"x"),
                None
            )
            .await
        ));
        let source=complete(&dice,"bazel_dep(name = \"dep\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"../dep\")\n","dep",Some(b"x"),None).await;
        assert!(
            matches!(source,SourcePreparationOutcome::Complete(x) if matches!(x.as_ref(),Err(DirectLocalModuleFileError::Source(_))))
        );
    }

    #[test]
    fn direct_inspection_identity_errors_and_complete_only_equality() {
        assert_eq!(
            inspection().to_string(),
            "direct-local-module-inspection:\"/workspace\":@dep_alias"
        );
        assert!(
            DirectLocalModuleInspectionKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                ApparentRepoName::root(),
            )
            .is_none()
        );
        assert_ne!(
            DirectLocalModuleInspectionError::InputCompute(Arc::from("input")),
            DirectLocalModuleInspectionError::Input(DirectLocalModuleFileError::RouteCompute(
                Arc::from("route"),
            )),
        );
        let input = DirectLocalModuleFile(local_route(), HostRepositorySourceFileValue::Absent);
        let complete = SourcePreparationOutcome::Complete(Arc::new(Ok(
            DirectLocalModuleInspection(input.clone(), None),
        )));
        let equal = SourcePreparationOutcome::Complete(Arc::new(Ok(DirectLocalModuleInspection(
            input, None,
        ))));
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::root_module_bootstrap(
            RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        ));
        assert!(DirectLocalModuleInspectionKey::equality(&complete, &equal));
        assert!(DirectLocalModuleInspectionKey::validity(&complete));
        assert!(!DirectLocalModuleInspectionKey::validity(&need));
        assert!(!DirectLocalModuleInspectionKey::equality(&need, &need));
    }

    #[tokio::test]
    async fn direct_inspection_retains_input_and_inspects_present_only() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let present = inspection_success(
            inspect_complete(
                &dice,
                &root("dep", "1.0"),
                "dep",
                Some(b"include(\"//:nested.MODULE.bazel\")"),
                None,
            )
            .await,
        );
        assert!(matches!(
            &present.0.1,
            HostRepositorySourceFileValue::Present { bytes, logical_path }
                if bytes.as_ref() == b"include(\"//:nested.MODULE.bazel\")"
                    && logical_path == &NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap()
        ));
        let inspection = present.1.expect("present source is inspected");
        assert_eq!(
            inspection.logical_id.0.as_str(),
            "/workspace/dep/MODULE.bazel"
        );
        assert_eq!(inspection.includes.len(), 1);
        assert_eq!(
            inspection.includes[0].path.as_str(),
            "//:nested.MODULE.bazel"
        );
        assert_eq!(inspection.includes[0].location.file, inspection.logical_id);
        let absent = inspection_success(
            inspect_complete(&dice, &root("dep", "1.0"), "dep", None, None).await,
        );
        assert!(matches!(absent.0.1, HostRepositorySourceFileValue::Absent));
        assert!(absent.1.is_none());
    }

    #[tokio::test]
    async fn direct_inspection_projects_input_and_parser_errors() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let unknown = inspect_complete(
            &dice,
            "bazel_dep(name = \"dep\", repo_name = \"other\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
            "dep",
            Some(b"x"),
            None,
        )
        .await;
        assert!(matches!(
            unknown,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(DirectLocalModuleInspectionError::Input(DirectLocalModuleFileError::Route(_))))
        ));
        let source = inspect_complete(
            &dice,
            "bazel_dep(name = \"dep\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"../dep\")\n",
            "dep",
            Some(b"x"),
            None,
        )
        .await;
        assert!(matches!(
            source,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(DirectLocalModuleInspectionError::Input(DirectLocalModuleFileError::Source(_))))
        ));
        let parser = inspect_complete(&dice, &root("dep", "1.0"), "dep", Some(&[0xff]), None).await;
        assert!(matches!(
            parser,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(DirectLocalModuleInspectionError::Inspection(logical_path, message)) if logical_path == &NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap() && message.as_ref() == "MODULE file is not valid UTF-8")
        ));
        let malformed =
            inspect_complete(&dice, &root("dep", "1.0"), "dep", Some(b"module("), None).await;
        assert!(matches!(
            malformed,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(DirectLocalModuleInspectionError::Inspection(logical_path, _)) if logical_path == &NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap())
        ));
    }

    #[tokio::test]
    async fn direct_inspection_forwards_direct_needs_exactly() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut u = dice.updater();
        u.changed_to(vec![(PathObservationEpochKey, missing_root())])
            .unwrap();
        inputs(&mut u);
        let mut t = u.commit().await;
        assert_eq!(
            need(t.compute(&inspection()).await.unwrap()),
            need(t.compute(&direct()).await.unwrap())
        );
        let source = root("dep", "1");
        let mut u = t.into_updater();
        u.changed_to(vec![(PathObservationEpochKey, root_only(&source))])
            .unwrap();
        inputs(&mut u);
        let mut t = u.commit().await;
        assert_eq!(
            need(t.compute(&inspection()).await.unwrap()),
            need(t.compute(&direct()).await.unwrap())
        );
        let mut u = t.into_updater();
        u.changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
            material("dep"),
        )])
        .unwrap();
        let mut t = u.commit().await;
        assert_eq!(
            need(t.compute(&inspection()).await.unwrap()),
            need(t.compute(&direct()).await.unwrap())
        );
    }

    #[tokio::test]
    async fn direct_inspection_lifecycle_and_capture_stay_callerless() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(InspectionTracker::default());
        let a = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-a", "1.0"),
                "dep-a",
                Some(b"bazel_dep(name = 'one', version = '1.0')"),
                Some(tracker.clone()),
            )
            .await,
        );
        let warm = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-a", "2.0"),
                "dep-a",
                Some(b"bazel_dep(name = 'one', version = '1.0')"),
                Some(tracker.clone()),
            )
            .await,
        );
        let b = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-b", "1.0"),
                "dep-b",
                Some(b"bazel_dep(name = 'two', version = '1.0')"),
                None,
            )
            .await,
        );
        let replayed = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-a", "1.0"),
                "dep-a",
                Some(b"bazel_dep(name = 'one', version = '1.0')"),
                None,
            )
            .await,
        );
        let edited = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-a", "1.0"),
                "dep-a",
                Some(b"bazel_dep(name = 'edited', version = '1.0')"),
                None,
            )
            .await,
        );
        let absent = inspection_success(
            inspect_complete(&dice, &root("dep-a", "2.0"), "dep-a", None, None).await,
        );
        let recreated = inspection_success(
            inspect_complete(
                &dice,
                &root("dep-a", "2.0"),
                "dep-a",
                Some(b"bazel_dep(name = 'recreated', version = '1.0')"),
                None,
            )
            .await,
        );
        assert_eq!(a, warm);
        assert_ne!(a.0, b.0);
        assert_eq!(a, replayed);
        assert_ne!(a.0, edited.0);
        assert!(a.1.is_some() && edited.1.is_some() && recreated.1.is_some());
        assert!(matches!(absent.0.1, HostRepositorySourceFileValue::Absent));
        assert!(absent.1.is_none());
        assert_ne!(a.0, absent.0);
        assert_ne!(absent.0, recreated.0);
        assert_eq!(
            *tracker.0.lock().unwrap(),
            [
                (ActivationKind::Evaluated, true),
                (ActivationKind::Reused, true)
            ]
        );
    }
    #[test]
    fn direct_include_horizon_finish_maps_every_result_and_unions_need_kinds() {
        assert_eq!(
            horizon().to_string(),
            "direct-local-include-package-horizon:\"/workspace\":@dep_alias"
        );
        assert!(
            DirectLocalIncludePackageHorizonKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                ApparentRepoName::root()
            )
            .is_none()
        );
        let p = horizon_occurrence("p", 1);
        let q = horizon_occurrence("q", 2);
        let finish = |occurrences, outcomes| {
            finish_direct_local_include_package_horizon(local_route(), occurrences, outcomes)
        };
        let complete = finish(Vec::new(), SmallMap::new());
        assert!(DirectLocalIncludePackageHorizonKey::validity(&complete));
        assert!(DirectLocalIncludePackageHorizonKey::equality(
            &complete, &complete
        ));
        let failure = |value: <DirectLocalIncludePackageHorizonKey as Key>::Value| match value {
            SourcePreparationOutcome::Complete(value) => {
                value.as_ref().as_ref().unwrap_err().clone()
            }
            SourcePreparationOutcome::Need(_) => panic!("expected terminal"),
        };
        for (error, sourced) in [
            (Err(Arc::from("compute")), false),
            (
                Ok(DirectLocalModuleInspectionError::InputCompute(Arc::from(
                    "input",
                ))),
                true,
            ),
        ] {
            let SourcePreparationOutcome::Complete(mapped) =
                direct_local_include_inspection_error(error)
            else {
                panic!("inspection mapping")
            };
            let mapped = mapped.as_ref().as_ref().unwrap_err();
            assert_eq!(std::error::Error::source(mapped).is_some(), sourced);
        }
        let typed = ExternalRepositoryPackageLookupError::Path(RepositorySourceFileError::Cycle {
            repo_relative_path: Arc::new(PathBuf::from("p/BUILD.bazel")),
        });
        for (outcome, kind, source) in [
            (
                Err(Arc::from("lookup compute failed")),
                "LookupCompute",
                false,
            ),
            (
                lookup_complete(Ok(ExternalRepositoryPackageLookup::InvalidPackageName {
                    message: Arc::from("invalid"),
                })),
                "InvalidPackageName",
                false,
            ),
            (lookup_complete(Err(typed)), "Lookup", true),
        ] {
            let mut outcomes = SmallMap::new();
            outcomes.insert(p.package.clone(), outcome);
            let error = failure(finish(vec![p.clone()], outcomes));
            assert!(format!("{error:?}").contains(kind));
            assert!(error.to_string().contains("dep/MODULE.bazel:1:3"));
            assert_eq!(std::error::Error::source(&error).is_some(), source);
        }
        let mut outcomes = SmallMap::new();
        outcomes.insert(
            q.package.clone(),
            lookup_complete(Ok(ExternalRepositoryPackageLookup::Deleted)),
        );
        outcomes.insert(
            p.package.clone(),
            lookup_complete(Ok(ExternalRepositoryPackageLookup::NoBuildFile)),
        );
        assert!(matches!(
            failure(finish(vec![p.clone(), q.clone()], outcomes)),
            DirectLocalIncludePackageHorizonError::Package { raw_label, failure: DirectLocalIncludePackageFailure::NoBuildFile, .. }
                if raw_label == p.raw_label
        ));
        let path_need = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/dep/p/BUILD.bazel").unwrap(),
                PathObservationOperation::Lstat,
            ),
        ));
        let bootstrap_need =
            SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            });
        let mut outcomes = SmallMap::new();
        outcomes.insert(
            p.package.clone(),
            Ok(SourcePreparationOutcome::Need(path_need)),
        );
        outcomes.insert(
            q.package.clone(),
            Ok(SourcePreparationOutcome::Need(bootstrap_need)),
        );
        let SourcePreparationOutcome::Need(union) = finish(vec![p, q], outcomes) else {
            panic!("expected union")
        };
        assert_eq!(union.path_observations().unwrap().demands().len(), 1);
        assert!(union.root_module_bootstrap_request().is_some());
    }
    #[tokio::test]
    async fn direct_include_horizon_parses_first_deduplicates_and_preserves_occurrences() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(HorizonTracker::default());
        let module = b"include(\"//p:first.MODULE.bazel\")\ninclude(\"//p:second.MODULE.bazel\")\ninclude(\"//q:third.MODULE.bazel\")\n";
        let value = horizon_success(
            horizon_compute(
                &dice,
                "/workspace/dep",
                Some(module),
                None,
                None,
                &[("p", true), ("q", true)],
                &[],
                &[],
                100,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        assert_eq!(
            value
                .occurrences
                .iter()
                .map(|occurrence| occurrence.raw_label.as_str())
                .collect::<Vec<_>>(),
            [
                "//p:first.MODULE.bazel",
                "//p:second.MODULE.bazel",
                "//q:third.MODULE.bazel"
            ]
        );
        assert!(value.occurrences.iter().all(|occurrence| {
            occurrence.package.repo() == &CanonicalRepoName::new("dep+").unwrap()
                && occurrence.location.file.0.as_str() == "/workspace/dep/MODULE.bazel"
        }));
        assert_eq!(value.occurrences[1].target.as_str(), "second.MODULE.bazel");
        assert_eq!(tracker.lookups.lock().unwrap().len(), 2);
        tracker.lookups.lock().unwrap().clear();
        let malformed = horizon_compute(
            &dice,
            "/workspace/dep",
            Some(b"include(\"//p:first.MODULE.bazel\")\ninclude(\"@bad//:x.MODULE.bazel\")\n"),
            None,
            None,
            &[("p", true)],
            &[("p", "BUILD.bazel")],
            &[],
            101,
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(
            malformed,
            SourcePreparationOutcome::Complete(error)
                if matches!(error.as_ref(), Err(DirectLocalIncludePackageHorizonError::BadLabel { raw_label, location, .. }) if raw_label == "@bad//:x.MODULE.bazel" && location.start_line == 2)
        ));
        assert!(tracker.lookups.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn direct_include_horizon_module_and_route_lifecycle_prunes_empty_presence() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(HorizonTracker::default());
        let p = b"include(\"//p:a.MODULE.bazel\")\n".as_slice();
        let pq = b"include(\"//p:a.MODULE.bazel\")\ninclude(\"//q:q.MODULE.bazel\")\n".as_slice();
        let qp = b"include(\"//q:q.MODULE.bazel\")\ninclude(\"//p:a.MODULE.bazel\")\n".as_slice();
        let mut values = Vec::new();
        let mut downstream = Vec::new();
        for (variant, route, module, packages) in [
            (130, "dep-a", None, &[][..]),
            (131, "dep-a", Some(b"".as_slice()), &[][..]),
            (132, "dep-a", Some(p), &[("p", true)][..]),
            (133, "dep-a", Some(pq), &[("p", true), ("q", true)][..]),
            (134, "dep-a", Some(qp), &[("p", true), ("q", true)][..]),
            (135, "dep-a", None, &[][..]),
            (136, "dep-a", Some(p), &[("p", true)][..]),
            (137, "dep-b", Some(p), &[("p", true)][..]),
            (138, "dep-a", Some(p), &[("p", true)][..]),
        ] {
            values.push(horizon_success(
                horizon_compute(
                    &dice,
                    &format!("/workspace/{route}"),
                    module,
                    None,
                    None,
                    packages,
                    &[],
                    &[],
                    variant,
                    true,
                    Some(tracker.clone()),
                )
                .await,
            ));
            downstream.push(tracker.downstream.load(Ordering::SeqCst));
        }
        assert_eq!(values[0], values[1]);
        assert_ne!(values[2], values[3]);
        assert_ne!(values[3], values[4]);
        assert_ne!(values[2], values[5]);
        assert_eq!(values[2], values[6]);
        assert_ne!(values[6].route, values[7].route);
        assert_eq!(values[6], values[8]);
        assert!(tracker.inspection.lock().unwrap().len() >= 9);
        assert!(tracker.horizon.lock().unwrap().len() >= 9);
        assert_eq!(downstream, [1, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
    #[tokio::test]
    async fn direct_include_horizon_policy_marker_and_event_lifecycle() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(HorizonTracker::default());
        let module = Some(b"include(\"//pkg:nested.MODULE.bazel\")\n".as_slice());
        let _success = horizon_compute(
            &dice,
            "/workspace/dep",
            module,
            Some(b"print('captured')\n"),
            None,
            &[("pkg", true)],
            &[],
            &[],
            140,
            true,
            Some(tracker.clone()),
        )
        .await;
        assert_eq!(
            *tracker.route_repo.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert_eq!(
            tracker.horizon.lock().unwrap()[0],
            (ActivationKind::Evaluated, true)
        );
        let _warm = horizon_compute(
            &dice,
            "/workspace/dep",
            module,
            Some(b"print('captured')\n"),
            None,
            &[("pkg", true)],
            &[],
            &[],
            140,
            true,
            Some(tracker.clone()),
        )
        .await;
        assert_eq!(tracker.downstream.load(Ordering::SeqCst), 1);
        assert_eq!(
            tracker.horizon.lock().unwrap().last().copied(),
            Some((ActivationKind::Reused, true))
        );
        for (variant, repo, ignore, selected, deleted, expected) in [
            (141, None, None, true, &["@dep+//pkg"][..], "Deleted"),
            (
                142,
                Some(b"ignore_directories(['pkg'])\n".as_slice()),
                None,
                true,
                &[][..],
                "Deleted",
            ),
            (
                143,
                None,
                Some(b"pkg\n".as_slice()),
                true,
                &[][..],
                "Deleted",
            ),
            (144, None, None, false, &[][..], "NoBuildFile"),
        ] {
            let outcome = horizon_compute(
                &dice,
                "/workspace/dep",
                module,
                repo,
                ignore,
                &[("pkg", selected)],
                &[],
                deleted,
                variant,
                true,
                None,
            )
            .await;
            assert!(matches!(
                outcome,
                SourcePreparationOutcome::Complete(error)
                    if matches!(error.as_ref(), Err(DirectLocalIncludePackageHorizonError::Package { failure, .. }) if format!("{failure:?}").starts_with(expected))
            ));
        }
        let recovered = horizon_compute(
            &dice,
            "/workspace/dep",
            module,
            Some(b"print('direct')\n"),
            None,
            &[("pkg", true)],
            &[],
            &[],
            145,
            false,
            Some(tracker.clone()),
        )
        .await;
        horizon_success(recovered);
        assert_eq!(
            tracker.route_repo.lock().unwrap().last().copied(),
            Some((ActivationKind::Evaluated, true))
        );
        assert!(tracker.horizon.lock().unwrap().last().unwrap().1);
    }
    #[tokio::test]
    async fn direct_module_evaluation_typed_boundaries_and_equality_are_exact() {
        assert_eq!(
            evaluation().to_string(),
            "direct-local-module-evaluation:\"/workspace\":@dep_alias"
        );
        assert!(
            DirectLocalModuleEvaluationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                ApparentRepoName::root(),
            )
            .is_none()
        );
        let tracker = Arc::new(EvaluationTracker::default());
        let absent = evaluation_failure(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                None,
                None,
                &[],
                &[],
                &[],
                300,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        assert!(matches!(
            absent,
            DirectLocalModuleEvaluationError::RootAbsent { canonical_repo }
                if canonical_repo.as_str() == "dep+"
        ));
        assert!(tracker
            .evaluation
            .lock()
            .unwrap()
            .iter()
            .any(|activation| matches!(activation.batch.as_ref(), Some(batch) if batch.events().is_empty())));

        let invalid_tracker = Arc::new(EvaluationTracker::default());
        let invalid = evaluation_failure(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                Some(b"unknown_identifier\n"),
                None,
                &[],
                &[],
                &[],
                301,
                true,
                Some(invalid_tracker.clone()),
            )
            .await,
        );
        assert!(matches!(
            invalid,
            DirectLocalModuleEvaluationError::Preparation(
                DirectLocalModulePreparationError::RootValidation { .. }
            )
        ));
        assert!(invalid_tracker
            .evaluation
            .lock()
            .unwrap()
            .iter()
            .any(|activation| matches!(activation.batch.as_ref(), Some(batch) if batch.events().is_empty())));

        let need_tracker = Arc::new(EvaluationTracker::default());
        let need = evaluation_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            Some(b"include('//missing:a.MODULE.bazel')\n"),
            None,
            &[],
            &[],
            &[],
            302,
            true,
            Some(need_tracker.clone()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!DirectLocalModuleEvaluationKey::validity(&need));
        assert!(!DirectLocalModuleEvaluationKey::equality(&need, &need));
        assert!(
            need_tracker
                .evaluation
                .lock()
                .unwrap()
                .iter()
                .all(|activation| activation.batch.is_none())
        );

        let compute = evaluation_failure(direct_local_evaluation_error(
            DirectLocalModuleEvaluationError::PreparationCompute {
                message: Arc::from("structural compute failure"),
            },
        ));
        assert!(compute.to_string().contains("structural compute failure"));
        assert!(std::error::Error::source(&compute).is_none());
    }

    #[tokio::test]
    async fn direct_module_support_projects_need_supported_and_ordinary_error_exactly() {
        let need_tracker = Arc::new(EvaluationTracker::default());
        let need = support_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            Some(b"include('//missing:a.MODULE.bazel')\n"),
            &[],
            &[],
            &[],
            320,
            Some(need_tracker.clone()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(
            need_tracker
                .evaluation
                .lock()
                .unwrap()
                .iter()
                .all(|activation| activation.batch.is_none())
        );

        let supported_tracker = Arc::new(EvaluationTracker::default());
        let supported = support_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            Some(b"module(name='dep')\nprint('supported-once')\n"),
            &[],
            &[],
            &[],
            321,
            Some(supported_tracker.clone()),
        )
        .await;
        assert!(matches!(
            supported,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(DirectLocalModuleSupport::Supported))
        ));
        let event_count = supported_tracker
            .evaluation
            .lock()
            .unwrap()
            .iter()
            .filter_map(|activation| activation.batch.as_ref())
            .flat_map(|batch| batch.events())
            .filter(|event| {
                matches!(
                    event,
                    EvaluationEvent::StarlarkPrint { text, .. } if text == "supported-once"
                )
            })
            .count();
        assert_eq!(event_count, 1);

        let ordinary = support_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            Some(b"module(name='dep')\nfail('ordinary-error')\n"),
            &[],
            &[],
            &[],
            322,
            None,
        )
        .await;
        let SourcePreparationOutcome::Complete(ordinary) = ordinary else {
            panic!("ordinary evaluation failure is complete")
        };
        let error = ordinary.as_ref().as_ref().unwrap_err();
        assert!(error.to_string().contains("ordinary-error"));
        let evaluation = std::error::Error::source(error).expect("support error retains owner");
        assert!(evaluation.to_string().contains("ordinary-error"));
        assert!(
            evaluation.source().is_some(),
            "evaluation owner retains the interpreter error"
        );
    }

    #[tokio::test]
    async fn direct_module_evaluation_owns_only_canonical_local_prints() {
        let tracker = Arc::new(EvaluationTracker::default());
        let module =
            b"module(name='dep', version='7.0')\nprint('root')\ninclude('//p:a.MODULE.bazel')\n";
        let fragment = b"print('fragment')\n";
        let value = evaluation_success(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                Some(module),
                Some(b"print('policy')\n"),
                &[("p", true)],
                &[("p/a.MODULE.bazel", Some(fragment.as_slice()))],
                &[],
                303,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let DirectLocalModuleEvaluation::Supported(value) = value else {
            panic!("acyclic module evaluates")
        };
        assert_eq!(value.route.canonical_repo().as_str(), "dep+");
        assert_eq!(
            value.module.base.expected_key,
            NonrootModuleKey::new("dep", "")
        );
        assert_eq!(value.module.base.declared_version, "7.0");
        let batches = tracker.evaluation.lock().unwrap();
        let batch = batches
            .iter()
            .find_map(|activation| activation.batch.as_ref())
            .unwrap();
        assert!(matches!(
            batch.events(),
            [
                EvaluationEvent::StarlarkPrint { location: root, text: root_text },
                EvaluationEvent::StarlarkPrint { location: fragment, text: fragment_text },
            ] if root_text == "root"
                && fragment_text == "fragment"
                && root.to_string().starts_with("@@dep+//:MODULE.bazel:")
                && fragment.to_string().starts_with("@@dep+//p:a.MODULE.bazel:")
        ));
        drop(batches);
        assert!(
            tracker
                .preparation
                .lock()
                .unwrap()
                .iter()
                .all(|a| a.batch.is_none())
        );
        assert!(
            tracker
                .route_repo
                .lock()
                .unwrap()
                .iter()
                .any(|activation| matches!(
                    activation.batch.as_ref().map(EventBatch::events),
                    Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "policy"
                ))
        );

        let uncaptured = Arc::new(EvaluationTracker::default());
        let value = evaluation_success(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                Some(b"module(name='dep')\nprint('direct')\n"),
                None,
                &[],
                &[],
                &[],
                304,
                false,
                Some(uncaptured.clone()),
            )
            .await,
        );
        assert!(matches!(value, DirectLocalModuleEvaluation::Supported(_)));
        assert!(
            uncaptured
                .evaluation
                .lock()
                .unwrap()
                .iter()
                .all(|a| a.batch.is_none())
        );

        let failed = Arc::new(EvaluationTracker::default());
        let error = evaluation_failure(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                Some(b"module(name='dep')\nprint('prefix')\nfail('boom')\n"),
                None,
                &[],
                &[],
                &[],
                305,
                true,
                Some(failed.clone()),
            )
            .await,
        );
        assert!(matches!(
            error,
            DirectLocalModuleEvaluationError::Evaluation(
                DirectNonregistryEvaluationError::Execution(ref message)
            ) if message.contains("boom")
        ));
        assert!(
            failed
                .evaluation
                .lock()
                .unwrap()
                .iter()
                .any(|activation| matches!(
                    activation.batch.as_ref().map(EventBatch::events),
                    Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "prefix"
                ))
        );
    }

    #[tokio::test]
    async fn direct_module_evaluation_support_gate_reuse_and_event_pruning_are_exact() {
        let unsupported_tracker = Arc::new(EvaluationTracker::default());
        let unsupported = evaluation_success(
            evaluation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                Some(b"module(name='dep')\nprint('must-not-run')\nfail('must-not-run')\ninclude('//a:a.MODULE.bazel')\n"),
                None,
                &[("a", true)],
                &[(
                    "a/a.MODULE.bazel",
                    Some(b"include('//a:a.MODULE.bazel')\n".as_slice()),
                )],
                &[],
                306,
                true,
                Some(unsupported_tracker.clone()),
            )
            .await,
        );
        assert!(matches!(
            unsupported,
            DirectLocalModuleEvaluation::Unsupported(_)
        ));
        assert!(
            unsupported_tracker
                .evaluation
                .lock()
                .unwrap()
                .iter()
                .any(|activation| matches!(
                    activation.batch.as_ref(),
                    Some(batch) if batch.events().is_empty()
                ))
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(EvaluationTracker::default());
        let first = evaluation_success(
            evaluation_compute(
                &dice,
                Some(b"module(name='dep', version='1.0')\nprint('A')\n"),
                None,
                &[],
                &[],
                &[],
                307,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let warm = evaluation_success(
            evaluation_compute(
                &dice,
                Some(b"module(name='dep', version='1.0')\nprint('A')\n"),
                None,
                &[],
                &[],
                &[],
                307,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let edited = evaluation_success(
            evaluation_compute(
                &dice,
                Some(b"module(name='dep', version='1.0')\nprint('B')\n"),
                None,
                &[],
                &[],
                &[],
                308,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let semantic_edit = evaluation_success(
            evaluation_compute(
                &dice,
                Some(b"module(name='dep', version='2.0')\nprint('C')\n"),
                None,
                &[],
                &[],
                &[],
                309,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        assert_eq!(first, warm);
        assert_eq!(first, edited, "events are excluded from semantic equality");
        assert_ne!(first, semantic_edit);
        assert_eq!(tracker.downstream.load(Ordering::SeqCst), 2);
        let activations = tracker.evaluation.lock().unwrap();
        assert!(
            activations
                .iter()
                .any(|a| a.kind == ActivationKind::Reused && a.batch.is_none())
        );
        assert!(activations.iter().any(|a| matches!(
            (a.kind, a.batch.as_ref().map(EventBatch::events)),
            (
                ActivationKind::Evaluated,
                Some([EvaluationEvent::StarlarkPrint { text, .. }])
            ) if text == "A"
        )));
        assert!(activations.iter().any(|a| matches!(
            a.batch.as_ref().map(EventBatch::events),
            Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "B"
        )));
    }

    #[tokio::test]
    async fn direct_module_preparation_root_gate_absence_and_equality_are_exact() {
        assert_eq!(
            preparation().to_string(),
            "direct-local-module-preparation:\"/workspace\":@dep_alias"
        );
        assert!(
            DirectLocalModulePreparationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                ApparentRepoName::root(),
            )
            .is_none()
        );
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let absent = preparation_success(
            preparation_compute(
                &dice,
                "/workspace/dep",
                None,
                None,
                &[],
                &[],
                &[],
                &[],
                200,
                true,
                None,
            )
            .await,
        );
        let DirectLocalModulePreparation::Supported(absent) = absent else {
            panic!("absent root is supported")
        };
        assert!(matches!(
            absent.root.0.1,
            HostRepositorySourceFileValue::Absent
        ));
        assert!(absent.fragments.is_empty());

        let tracker = Arc::new(PreparationTracker::default());
        let invalid = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(b"include(\"//p:a.MODULE.bazel\")\nunknown_identifier\n".as_slice()),
                None,
                &[("p", true)],
                &[("p/a.MODULE.bazel", Some(b"".as_slice()))],
                &[],
                &[],
                201,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        assert!(matches!(
            invalid,
            DirectLocalModulePreparationError::RootValidation { logical_path, ref message }
                if logical_path.as_path() == Path::new("/workspace/dep/MODULE.bazel")
                    && message.contains("unknown_identifier")
        ));
        assert!(tracker.lookups.lock().unwrap().is_empty());
        assert_eq!(tracker.sources.lock().unwrap().len(), 1);
        assert!(tracker.sources.lock().unwrap()[0].ends_with(":MODULE.bazel"));

        let complete = SourcePreparationOutcome::Complete(Arc::new(Ok(
            DirectLocalModulePreparation::Supported(absent.clone()),
        )));
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/dep/MODULE.bazel").unwrap(),
                PathObservationOperation::FileBytes,
            )),
        ));
        assert!(DirectLocalModulePreparationKey::equality(
            &complete, &complete
        ));
        assert!(DirectLocalModulePreparationKey::validity(&complete));
        assert!(!DirectLocalModulePreparationKey::equality(&need, &need));
        assert!(!DirectLocalModulePreparationKey::validity(&need));
    }

    #[tokio::test]
    async fn direct_module_preparation_is_breadth_first_and_compiles_every_occurrence() {
        crate::module_eval::clear_validated_root_module_logical_ids();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(PreparationTracker::default());
        let module = b"include(\"//p:a.MODULE.bazel\")\ninclude(\"//q:b.MODULE.bazel\")\ninclude(\"//p:a.MODULE.bazel\")\n";
        let a = b"include(\"//r:c.MODULE.bazel\")\n";
        let b = b"include(\"//r:c.MODULE.bazel\")\n";
        let value = preparation_success(
            preparation_compute(
                &dice,
                "/workspace/dep",
                Some(module),
                None,
                &[("p", true), ("q", true), ("r", true)],
                &[
                    ("p/a.MODULE.bazel", Some(a.as_slice())),
                    ("q/b.MODULE.bazel", Some(b.as_slice())),
                    ("r/c.MODULE.bazel", Some(b"".as_slice())),
                ],
                &[],
                &[],
                202,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let DirectLocalModulePreparation::Supported(value) = value else {
            panic!("acyclic diamond is supported")
        };
        assert_eq!(
            value
                .fragments
                .iter()
                .map(|fragment| fragment.raw_label.as_str())
                .collect::<Vec<_>>(),
            [
                "//p:a.MODULE.bazel",
                "//q:b.MODULE.bazel",
                "//p:a.MODULE.bazel",
                "//r:c.MODULE.bazel",
                "//r:c.MODULE.bazel",
                "//r:c.MODULE.bazel",
            ]
        );
        assert_eq!(
            crate::module_eval::take_validated_root_module_logical_ids()
                .iter()
                .map(|id| id.0.as_str())
                .collect::<Vec<_>>(),
            [
                "/workspace/MODULE.bazel",
                "/workspace/dep/MODULE.bazel",
                "/workspace/dep/p/a.MODULE.bazel",
                "/workspace/dep/q/b.MODULE.bazel",
                "/workspace/dep/p/a.MODULE.bazel",
                "/workspace/dep/r/c.MODULE.bazel",
                "/workspace/dep/r/c.MODULE.bazel",
                "/workspace/dep/r/c.MODULE.bazel",
            ]
        );
        let sources = tracker.sources.lock().unwrap();
        assert_eq!(
            sources
                .iter()
                .filter(|source| source.ends_with(":p/a.MODULE.bazel"))
                .count(),
            1
        );
        assert_eq!(
            sources
                .iter()
                .filter(|source| source.ends_with(":r/c.MODULE.bazel"))
                .count(),
            1
        );
        assert_eq!(tracker.lookups.lock().unwrap().len(), 3);
        assert_eq!(value.fragments[0].bytes.as_ref(), a);
        assert_eq!(
            value.fragments[0].logical_path.as_path(),
            Path::new("/workspace/dep/p/a.MODULE.bazel")
        );
        assert_eq!(value.fragments[0].location.start_line, 1);
    }

    #[tokio::test]
    async fn direct_module_preparation_distinct_labels_keep_one_canonical_dependency() {
        crate::module_eval::clear_validated_root_module_logical_ids();
        let tracker = Arc::new(PreparationTracker::default());
        let value = preparation_success(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(
                    b"include(\"//p/a.MODULE.bazel\")\ninclude(\"//p/a.MODULE.bazel:a.MODULE.bazel\")\n",
                ),
                None,
                &[("p/a.MODULE.bazel", true)],
                &[(
                    "p/a.MODULE.bazel/a.MODULE.bazel",
                    Some(b"".as_slice()),
                )],
                &[],
                &[],
                232,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        let DirectLocalModulePreparation::Supported(value) = value else {
            panic!("canonical-equivalent siblings are not a cycle")
        };
        assert_eq!(value.fragments.len(), 2);
        assert_ne!(value.fragments[0].raw_label, value.fragments[1].raw_label);
        assert_eq!(value.fragments[0].package, value.fragments[1].package);
        assert_eq!(value.fragments[0].target, value.fragments[1].target);
        assert_eq!(value.fragments[0].bytes, value.fragments[1].bytes);
        assert_eq!(tracker.lookups.lock().unwrap().len(), 1);
        assert_eq!(
            tracker
                .sources
                .lock()
                .unwrap()
                .iter()
                .filter(|source| { source.ends_with(":p/a.MODULE.bazel/a.MODULE.bazel") })
                .count(),
            1
        );
        assert_eq!(
            crate::module_eval::take_validated_root_module_logical_ids()
                .iter()
                .filter(|id| { id.0.ends_with("/p/a.MODULE.bazel/a.MODULE.bazel") })
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn direct_module_preparation_fragment_precedence_and_need_union_are_exact() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let missing_first = preparation_failure(
            preparation_compute(
                &dice,
                "/workspace/dep",
                Some(
                    b"include(\"//p:missing.MODULE.bazel\")\ninclude(\"//q:need.MODULE.bazel\")\n",
                ),
                None,
                &[("p", true), ("q", true)],
                &[("p/missing.MODULE.bazel", None)],
                &["q/need.MODULE.bazel"],
                &[],
                203,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            missing_first,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                location,
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            } if raw_label == "//p:missing.MODULE.bazel" && location.start_line == 1
        ));
        let need_first = preparation_compute(
            &dice,
            "/workspace/dep",
            Some(b"include(\"//q:need.MODULE.bazel\")\ninclude(\"//p:missing.MODULE.bazel\")\n"),
            None,
            &[("p", true), ("q", true)],
            &[("p/missing.MODULE.bazel", None)],
            &["q/need.MODULE.bazel"],
            &[],
            204,
            true,
            None,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = need_first else {
            panic!("earlier fragment Need wins")
        };
        let demands = need.path_observations().unwrap().demands();
        assert_eq!(demands.len(), 1);
        assert_eq!(demands[0].operation(), PathObservationOperation::FileBytes);
        assert!(demands[0].path().as_path().ends_with("q/need.MODULE.bazel"));

        let path_need = SourcePreparationNeeds::path(NeedPathObservations::singleton(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/dep/p/a.MODULE.bazel").unwrap(),
                PathObservationOperation::FileBytes,
            ),
        ));
        let repository_need =
            SourcePreparationNeeds::repository(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                    canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
                },
                repo_spec: local_route().repo_spec().clone(),
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
                },
            });
        let mut outcomes = SmallMap::new();
        outcomes.insert(
            PathBuf::from("p/a.MODULE.bazel"),
            Ok(SourcePreparationOutcome::Need(path_need)),
        );
        outcomes.insert(
            PathBuf::from("q/b.MODULE.bazel"),
            Ok(SourcePreparationOutcome::Need(repository_need)),
        );
        let union = union_direct_local_fragment_needs(&outcomes).unwrap();
        assert!(union.path_observations().is_some());
        assert_eq!(union.repository_materializations().len(), 1);
    }

    #[tokio::test]
    async fn direct_module_preparation_cycle_waits_for_later_side_branch_failure_and_need() {
        let module = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//x:x.MODULE.bazel\")\n";
        let a = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//side:b.MODULE.bazel\")\n";
        let x = b"include(\"//y:y.MODULE.bazel\")\n";
        let side = b"include(\"//late:c.MODULE.bazel\")\n";
        let packages = [
            ("a", true),
            ("x", true),
            ("side", true),
            ("y", true),
            ("late", true),
        ];
        let base = [
            ("a/a.MODULE.bazel", Some(a.as_slice())),
            ("x/x.MODULE.bazel", Some(x.as_slice())),
            ("side/b.MODULE.bazel", Some(side.as_slice())),
            ("y/y.MODULE.bazel", Some(b"".as_slice())),
        ];
        let terminal = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(module),
                None,
                &packages,
                &[
                    base[0],
                    base[1],
                    base[2],
                    base[3],
                    ("late/c.MODULE.bazel", None),
                ],
                &[],
                &[],
                205,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            terminal,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            } if raw_label == "//late:c.MODULE.bazel"
        ));
        let need = preparation_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            "/workspace/dep",
            Some(module),
            None,
            &packages,
            &base,
            &["late/c.MODULE.bazel"],
            &[],
            206,
            true,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));

        let reversed = b"include(\"//x:x.MODULE.bazel\")\ninclude(\"//a:a.MODULE.bazel\")\n";
        let a_reversed = b"include(\"//side:b.MODULE.bazel\")\ninclude(\"//a:a.MODULE.bazel\")\n";
        let reversed_base = [
            ("a/a.MODULE.bazel", Some(a_reversed.as_slice())),
            base[1],
            base[2],
            base[3],
        ];
        let terminal = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(reversed),
                None,
                &packages,
                &[
                    reversed_base[0],
                    reversed_base[1],
                    reversed_base[2],
                    reversed_base[3],
                    ("late/c.MODULE.bazel", None),
                ],
                &[],
                &[],
                224,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            terminal,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            } if raw_label == "//late:c.MODULE.bazel"
        ));
        let need = preparation_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            "/workspace/dep",
            Some(reversed),
            None,
            &packages,
            &reversed_base,
            &["late/c.MODULE.bazel"],
            &[],
            225,
            true,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    }

    #[tokio::test]
    async fn direct_module_preparation_cycle_candidate_loses_to_same_horizon_terminal_and_need() {
        let module = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//x:x.MODULE.bazel\")\n";
        let a = b"include(\"//a:a.MODULE.bazel\")\n";
        let x = b"include(\"//bad:b.MODULE.bazel\")\n";
        let packages = [("a", true), ("x", true), ("bad", true)];
        let terminal = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(module),
                None,
                &packages,
                &[
                    ("a/a.MODULE.bazel", Some(a.as_slice())),
                    ("x/x.MODULE.bazel", Some(x.as_slice())),
                    ("bad/b.MODULE.bazel", None),
                ],
                &[],
                &[],
                209,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            terminal,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            } if raw_label == "//bad:b.MODULE.bazel"
        ));
        let need = preparation_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            "/workspace/dep",
            Some(module),
            None,
            &packages,
            &[
                ("a/a.MODULE.bazel", Some(a.as_slice())),
                ("x/x.MODULE.bazel", Some(x.as_slice())),
            ],
            &["bad/b.MODULE.bazel"],
            &[],
            210,
            true,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    }

    #[tokio::test]
    async fn direct_module_preparation_cycle_capability_uses_first_breadth_first_provenance() {
        let module = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//x:x.MODULE.bazel\")\n";
        let a = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//side:b.MODULE.bazel\")\n";
        let x = b"include(\"//x:x.MODULE.bazel\")\n";
        let value = preparation_success(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(module),
                None,
                &[("a", true), ("x", true), ("side", true)],
                &[
                    ("a/a.MODULE.bazel", Some(a.as_slice())),
                    ("x/x.MODULE.bazel", Some(x.as_slice())),
                    ("side/b.MODULE.bazel", Some(b"".as_slice())),
                ],
                &[],
                &[],
                207,
                true,
                None,
            )
            .await,
        );
        let DirectLocalModulePreparation::Unsupported(cycle) = value else {
            panic!("active-ancestry repeat is unsupported")
        };
        assert_eq!(cycle.package.package().as_str(), "a");
        assert_eq!(cycle.target.as_str(), "a.MODULE.bazel");
        assert_eq!(cycle.ancestor_raw_label, "//a:a.MODULE.bazel");
        assert_eq!(
            cycle.ancestor_location.file.0,
            "/workspace/dep/MODULE.bazel"
        );
        assert_eq!(cycle.ancestor_location.start_line, 1);
        assert_eq!(cycle.repeated_raw_label, "//a:a.MODULE.bazel");
        assert_eq!(
            cycle.repeated_location.file.0,
            "/workspace/dep/a/a.MODULE.bazel"
        );
        assert_eq!(cycle.repeated_location.start_line, 1);

        let multi = preparation_success(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(b"include(\"//a:a.MODULE.bazel\")\n"),
                None,
                &[("a", true), ("b", true)],
                &[
                    (
                        "a/a.MODULE.bazel",
                        Some(b"include(\"//b:b.MODULE.bazel\")\n".as_slice()),
                    ),
                    (
                        "b/b.MODULE.bazel",
                        Some(b"include(\"//a:a.MODULE.bazel\")\n".as_slice()),
                    ),
                ],
                &[],
                &[],
                208,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            multi,
            DirectLocalModulePreparation::Unsupported(
                DirectLocalIncludeCycleCapability { ref package, .. }
            ) if package.package().as_str() == "a"
        ));
    }

    #[tokio::test]
    async fn direct_module_preparation_package_barrier_and_fragment_validation_are_typed() {
        let tracker = Arc::new(PreparationTracker::default());
        let package_need = preparation_compute(
            &Dice::builder().build(DetectCycles::Enabled),
            "/workspace/dep",
            Some(b"include(\"//p:a.MODULE.bazel\")\n"),
            None,
            &[],
            &[("p/a.MODULE.bazel", Some(b"".as_slice()))],
            &[],
            &[],
            211,
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(package_need, SourcePreparationOutcome::Need(_)));
        assert!(
            !tracker
                .sources
                .lock()
                .unwrap()
                .iter()
                .any(|source| source.ends_with(":p/a.MODULE.bazel"))
        );

        let tracker = Arc::new(PreparationTracker::default());
        let package_error = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(b"include(\"//p:a.MODULE.bazel\")\ninclude(\"//missing:m.MODULE.bazel\")\n"),
                None,
                &[("p", true), ("missing", false)],
                &[("p/a.MODULE.bazel", Some(b"".as_slice()))],
                &[],
                &[],
                212,
                true,
                Some(tracker.clone()),
            )
            .await,
        );
        assert!(matches!(
            package_error,
            DirectLocalModulePreparationError::Package(
                DirectLocalIncludePackageHorizonError::Package {
                    raw_label,
                    failure: DirectLocalIncludePackageFailure::NoBuildFile,
                    ..
                }
            ) if raw_label == "//missing:m.MODULE.bazel"
        ));
        assert!(
            !tracker
                .sources
                .lock()
                .unwrap()
                .iter()
                .any(|source| source.ends_with(":p/a.MODULE.bazel"))
        );

        let validation = preparation_failure(
            preparation_compute(
                &Dice::builder().build(DetectCycles::Enabled),
                "/workspace/dep",
                Some(b"include(\"//p:a.MODULE.bazel\")\n"),
                None,
                &[("p", true)],
                &[("p/a.MODULE.bazel", Some(b"unknown_identifier\n".as_slice()))],
                &[],
                &[],
                213,
                true,
                None,
            )
            .await,
        );
        assert!(matches!(
            validation,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                failure: DirectLocalIncludeFragmentFailure::Validation {
                    logical_path,
                    ref message,
                },
                ..
            } if raw_label == "//p:a.MODULE.bazel"
                && logical_path.as_path() == Path::new("/workspace/dep/p/a.MODULE.bazel")
                && message.contains("unknown_identifier")
        ));

        for (variant, bytes, expected) in [
            (233, &[0xff][..], "UTF-8"),
            (
                234,
                b"for x in []:\n  pass\n".as_slice(),
                "`for` cannot be used",
            ),
            (
                235,
                b"unknown_identifier\n".as_slice(),
                "unknown_identifier",
            ),
        ] {
            let validation = preparation_failure(
                preparation_compute(
                    &Dice::builder().build(DetectCycles::Enabled),
                    "/workspace/dep",
                    Some(b"include(\"//p:a.MODULE.bazel\")\n"),
                    None,
                    &[("p", true)],
                    &[("p/a.MODULE.bazel", Some(bytes))],
                    &[],
                    &[],
                    variant,
                    true,
                    None,
                )
                .await,
            );
            let DirectLocalModulePreparationError::Fragment {
                raw_label,
                location,
                repo_relative_path,
                failure:
                    DirectLocalIncludeFragmentFailure::Validation {
                        logical_path,
                        message,
                    },
            } = &validation
            else {
                panic!("expected fragment validation, got {validation:?}")
            };
            assert_eq!(raw_label, "//p:a.MODULE.bazel");
            assert_eq!(location.start_line, 1);
            assert_eq!(repo_relative_path.as_path(), Path::new("p/a.MODULE.bazel"));
            assert_eq!(
                logical_path.as_path(),
                Path::new("/workspace/dep/p/a.MODULE.bazel")
            );
            assert!(
                message.contains(expected),
                "expected {expected:?} in {message:?}"
            );
        }
    }

    #[tokio::test]
    async fn direct_module_unsupported_equality_prunes_irrelevant_side_branch_edits() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(PreparationTracker::default());
        let module = b"include(\"//a:a.MODULE.bazel\")\n";
        let a = b"include(\"//a:a.MODULE.bazel\")\ninclude(\"//side:b.MODULE.bazel\")\n";
        macro_rules! compute {
            ($side:expr, $variant:expr) => {{
                let side: &'static [u8] = $side;
                preparation_compute(
                    &dice,
                    "/workspace/dep",
                    Some(module),
                    None,
                    &[("a", true), ("side", true)],
                    &[
                        ("a/a.MODULE.bazel", Some(a.as_slice())),
                        ("side/b.MODULE.bazel", Some(side)),
                    ],
                    &[],
                    &[],
                    $variant,
                    true,
                    Some(tracker.clone()),
                )
                .await
            }};
        }
        let first = preparation_success(compute!(b"", 236));
        let edited =
            preparation_success(compute!(b"bazel_dep(name = 'side', version = '1')\n", 237));
        assert_eq!(first, edited);
        assert!(matches!(
            first,
            DirectLocalModulePreparation::Unsupported(_)
        ));
        assert_eq!(
            tracker.downstream.load(Ordering::SeqCst),
            1,
            "equal unsupported capability prunes downstream recomputation"
        );
    }

    #[tokio::test]
    async fn direct_module_preparation_lifecycle_route_reuse_and_events_are_exact() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(PreparationTracker::default());
        let module = Some(b"include(\"//p:a.MODULE.bazel\")\n".as_slice());
        macro_rules! compute {
            ($route_path:expr, $bytes:expr, $variant:expr, $capture:expr $(,)?) => {{
                let bytes: &'static [u8] = $bytes;
                let fragments = [("p/a.MODULE.bazel", Some(bytes))];
                preparation_compute(
                    &dice,
                    $route_path,
                    module,
                    Some(b"print('policy')\n"),
                    &[("p", true)],
                    &fragments,
                    &[],
                    &[],
                    $variant,
                    $capture,
                    Some(tracker.clone()),
                )
                .await
            }};
        }
        let cold = preparation_success(compute!("/workspace/dep-a", b"", 214, true));
        let cold_child_activations = tracker.route_repo.lock().unwrap().len();
        let warm = preparation_success(compute!("/workspace/dep-a", b"", 214, true));
        assert_eq!(cold, warm);
        assert_eq!(tracker.downstream.load(Ordering::SeqCst), 1);
        let preparation_activations = tracker.preparation.lock().unwrap();
        assert_eq!(
            preparation_activations[0],
            (ActivationKind::Evaluated, true)
        );
        assert_eq!(
            preparation_activations.last().copied(),
            Some((ActivationKind::Reused, true))
        );
        assert_eq!(
            preparation_activations
                .iter()
                .filter(|(kind, _)| *kind == ActivationKind::Evaluated)
                .count(),
            1
        );
        drop(preparation_activations);
        assert_eq!(
            tracker.route_repo.lock().unwrap().len(),
            cold_child_activations,
            "warm preparation must not replay its child event batch"
        );
        assert_eq!(
            tracker.route_repo.lock().unwrap()[0],
            (ActivationKind::Evaluated, false)
        );

        let edited = preparation_success(compute!(
            "/workspace/dep-a",
            b"bazel_dep(name = 'edited', version = '1')\n",
            215,
            true,
        ));
        assert_ne!(cold, edited);
        assert_eq!(tracker.downstream.load(Ordering::SeqCst), 2);
        let b = preparation_success(compute!("/workspace/dep-b", b"", 216, true));
        let restored = preparation_success(compute!("/workspace/dep-a", b"", 217, true));
        assert_ne!(cold, b);
        assert_eq!(cold, restored);
        assert_eq!(tracker.downstream.load(Ordering::SeqCst), 4);

        let uncaptured = preparation_success(compute!(
            "/workspace/dep-a",
            b"print('fragment')\n",
            218,
            false
        ));
        assert_ne!(restored, uncaptured);
        assert!(
            tracker
                .preparation
                .lock()
                .unwrap()
                .iter()
                .all(|(_, event_free)| *event_free)
        );
        assert_eq!(
            tracker.route_repo.lock().unwrap().last().copied(),
            Some((ActivationKind::Reused, true))
        );
    }

    #[tokio::test]
    async fn direct_module_preparation_fragment_and_include_lifecycle_recovers_and_reorders() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let p_then_q = b"include(\"//p:a.MODULE.bazel\")\ninclude(\"//q:b.MODULE.bazel\")\n";
        let q_then_p = b"include(\"//q:b.MODULE.bazel\")\ninclude(\"//p:a.MODULE.bazel\")\n";
        macro_rules! compute {
            ($module:expr, $p:expr, $variant:expr $(,)?) => {{
                let p: Option<&'static [u8]> = $p;
                let fragments = [
                    ("p/a.MODULE.bazel", p),
                    ("q/b.MODULE.bazel", Some(b"".as_slice())),
                ];
                preparation_compute(
                    &dice,
                    "/workspace/dep",
                    Some($module),
                    None,
                    &[("p", true), ("q", true)],
                    &fragments,
                    &[],
                    &[],
                    $variant,
                    true,
                    None,
                )
                .await
            }};
        }
        let initial = preparation_success(compute!(p_then_q, Some(b""), 219));
        let edited = preparation_success(compute!(
            p_then_q,
            Some(b"bazel_dep(name = 'nested', version = '1')\n"),
            220,
        ));
        assert_ne!(initial, edited);
        let deleted = preparation_failure(compute!(p_then_q, None, 221));
        assert!(matches!(
            deleted,
            DirectLocalModulePreparationError::Fragment {
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            }
        ));
        let recreated = preparation_success(compute!(p_then_q, Some(b""), 222));
        assert_eq!(initial, recreated);
        let reordered = preparation_success(compute!(q_then_p, Some(b""), 223));
        assert_ne!(initial, reordered);
        let DirectLocalModulePreparation::Supported(reordered) = reordered else {
            panic!("reordered acyclic closure")
        };
        assert_eq!(
            reordered
                .fragments
                .iter()
                .map(|fragment| fragment.package.package().as_str())
                .collect::<Vec<_>>(),
            ["q", "p"]
        );
    }

    #[tokio::test]
    async fn direct_module_preparation_nested_include_lifecycle_is_occurrence_exact() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let root = b"include(\"//p:a.MODULE.bazel\")\n";
        let r = b"include(\"//r:r.MODULE.bazel\")\n";
        let s = b"include(\"//s:s.MODULE.bazel\")\n";
        let rs = b"include(\"//r:r.MODULE.bazel\")\ninclude(\"//s:s.MODULE.bazel\")\n";
        let sr = b"include(\"//s:s.MODULE.bazel\")\ninclude(\"//r:r.MODULE.bazel\")\n";
        macro_rules! compute {
            ($nested:expr, $r_source:expr, $s_source:expr, $variant:expr) => {{
                let fragments = [
                    ("p/a.MODULE.bazel", Some($nested.as_slice())),
                    ("r/r.MODULE.bazel", $r_source),
                    ("s/s.MODULE.bazel", $s_source),
                ];
                preparation_compute(
                    &dice,
                    "/workspace/dep",
                    Some(root),
                    None,
                    &[("p", true), ("r", true), ("s", true)],
                    &fragments,
                    &[],
                    &[],
                    $variant,
                    true,
                    None,
                )
                .await
            }};
        }
        let initial = preparation_success(compute!(r, Some(b"".as_slice()), None, 226));
        let added = preparation_success(compute!(
            rs,
            Some(b"".as_slice()),
            Some(b"".as_slice()),
            227
        ));
        let edited = preparation_success(compute!(s, None, Some(b"".as_slice()), 228));
        assert_ne!(initial, added);
        assert_ne!(added, edited);
        let deleted = preparation_failure(compute!(s, None, None, 229));
        assert!(matches!(
            deleted,
            DirectLocalModulePreparationError::Fragment {
                raw_label,
                failure: DirectLocalIncludeFragmentFailure::Absent,
                ..
            } if raw_label == "//s:s.MODULE.bazel"
        ));
        let recreated = preparation_success(compute!(s, None, Some(b"".as_slice()), 230));
        assert_eq!(edited, recreated);
        let reordered = preparation_success(compute!(
            sr,
            Some(b"".as_slice()),
            Some(b"".as_slice()),
            231
        ));
        assert_ne!(added, reordered);
        let DirectLocalModulePreparation::Supported(added) = added else {
            panic!("nested add remains acyclic")
        };
        assert_eq!(
            added
                .fragments
                .iter()
                .map(|fragment| fragment.package.package().as_str())
                .collect::<Vec<_>>(),
            ["p", "r", "s"]
        );
        let DirectLocalModulePreparation::Supported(reordered) = reordered else {
            panic!("nested reorder remains acyclic")
        };
        assert_eq!(
            reordered
                .fragments
                .iter()
                .map(|fragment| fragment.package.package().as_str())
                .collect::<Vec<_>>(),
            ["p", "s", "r"]
        );
    }

    #[test]
    fn direct_module_preparation_typed_fragment_failures_restore_context() {
        let occurrence = horizon_occurrence("pkg", 7);
        for (failure, expected, sourced) in [
            (
                DirectLocalIncludeFragmentFailure::SourceCompute {
                    message: Arc::from("compute"),
                },
                "SourceCompute",
                false,
            ),
            (
                DirectLocalIncludeFragmentFailure::Source(RepositorySourceFileError::WrongKind {
                    repo_relative_path: Arc::new(PathBuf::from("pkg/pkg.MODULE.bazel")),
                    actual: PathNodeKind::Directory,
                }),
                "WrongKind",
                false,
            ),
            (
                DirectLocalIncludeFragmentFailure::Validation {
                    logical_path: NormalizedAbsolutePath::new(
                        "/workspace/dep/pkg/pkg.MODULE.bazel",
                    )
                    .unwrap(),
                    message: "invalid".into(),
                },
                "Validation",
                false,
            ),
        ] {
            let error = match direct_local_fragment_error(
                &occurrence,
                Path::new("pkg/pkg.MODULE.bazel"),
                failure,
            ) {
                SourcePreparationOutcome::Complete(value) => {
                    value.as_ref().as_ref().unwrap_err().clone()
                }
                SourcePreparationOutcome::Need(_) => panic!("typed terminal"),
            };
            assert!(format!("{error:?}").contains(expected));
            assert!(error.to_string().contains("dep/MODULE.bazel:7:3"));
            assert_eq!(std::error::Error::source(&error).is_some(), sourced);
        }
        for error in [
            DirectLocalModulePreparationError::InspectionCompute {
                message: Arc::from("inspection"),
            },
            DirectLocalModulePreparationError::Inspection(
                DirectLocalModuleInspectionError::InputCompute(Arc::from("input")),
            ),
        ] {
            assert_eq!(
                std::error::Error::source(&error).is_some(),
                matches!(error, DirectLocalModulePreparationError::Inspection(_))
            );
        }
    }

    #[test]
    fn direct_module_preparation_structural_boundary_is_private_and_event_free() {
        let source = include_str!("source_preparation.rs");
        let owner = source
            .split("struct DirectLocalModulePreparationKey")
            .nth(1)
            .unwrap()
            .split("struct DirectLocalModuleEvaluationKey")
            .next()
            .unwrap();
        for required in [
            "DirectLocalModuleInspectionKey",
            "validate_root_module_source",
            "preflight_direct_local_include_package_horizon",
            "HostRepositorySourceFileKey",
            "compute_join",
            "try_union",
            "DirectLocalModulePreparation::Unsupported",
        ] {
            assert!(owner.contains(required), "{required}");
        }
        for forbidden in [
            "pub ",
            "store_evaluation_data",
            "CaptureEvaluationEvents",
            "EventBatch",
            "evaluate_nonroot_module_file",
            "evaluate_root_module_closure",
            "Evaluator",
            "std::fs",
            "compute(&DirectLocalModulePreparationKey",
            "DirectLocalIncludePackageHorizonKey",
            "RootRepositoryRouteKey",
            "MAX_DEPTH",
            "visited",
            "seen",
            "Mutex",
        ] {
            assert!(!owner.contains(forbidden), "{forbidden}");
        }
        let closure = owner
            .split("struct DirectLocalModuleClosure")
            .nth(1)
            .unwrap()
            .split("struct DirectLocalIncludeFragment")
            .next()
            .unwrap();
        assert!(!closure.contains("Cycle"));
    }
    #[test]
    fn direct_module_evaluation_structural_boundary_is_private_and_support_gated() {
        let source = include_str!("source_preparation.rs");
        let owner = source
            .split("struct DirectLocalModuleEvaluationKey")
            .nth(1)
            .unwrap()
            .split("pub struct RepositoryMaterializationGeneration")
            .next()
            .unwrap();
        for required in [
            "DirectLocalModulePreparationKey",
            "CaptureEvaluationEvents",
            "evaluate_direct_nonregistry_module_closure_with_events",
            "NonrootModuleKey::new(route.module_name(), \"\")",
            "store_evaluation_data",
            "complete_eq",
            "is_complete",
        ] {
            assert!(owner.contains(required), "{required}");
        }
        for forbidden in [
            "pub ",
            "RootModuleFilesKey",
            "ModuleSourcePreparationKey",
            "RegistryFileKey",
            "std::fs",
            "ctx.compute(&DirectLocalModuleInspectionKey",
        ] {
            assert!(!owner.contains(forbidden), "{forbidden}");
        }
        let unsupported = owner
            .find("DirectLocalModulePreparation::Unsupported")
            .unwrap();
        let evaluator = owner
            .find("evaluate_direct_nonregistry_module_closure_with_events")
            .unwrap();
        assert!(unsupported < evaluator);
        assert!(!include_str!("lib.rs").contains("DirectLocalModuleEvaluationKey"));
    }
    #[test]
    fn direct_include_horizon_structural_boundary_is_private_and_fragment_free() {
        let source = include_str!("source_preparation.rs");
        let owner = source
            .split("struct DirectLocalIncludePackageHorizonKey")
            .nth(1)
            .unwrap()
            .split("struct DirectLocalModulePreparationKey")
            .next()
            .unwrap();
        let (key_owner, _) = owner
            .split_once("async fn preflight_direct_local_include_package_horizon")
            .unwrap();
        assert!(key_owner.contains("preflight_direct_local_include_package_horizon(ctx"));
        assert!(owner.contains("DirectLocalModuleInspectionKey"));
        assert!(owner.contains("ExternalRepositoryPackageLookupKey"));
        assert!(owner.contains("parse_root_include"));
        for forbidden in "HostRepositorySourceFileKey,FileBytes,std::fs,evaluate_nonroot_module_file,store_evaluation_data,pub ,fragment".split(',') {
            assert!(!owner.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn direct_inspection_structural_scan() {
        let s = include_str!("source_preparation.rs");
        let inspection = s
            .split("struct DirectLocalModuleInspectionKey")
            .nth(1)
            .unwrap()
            .split("impl fmt::Display for RepositorySourceFileKey")
            .next()
            .unwrap();
        assert!(inspection.contains("DirectLocalModuleFileKey"));
        assert!(inspection.contains("inspect_nonroot_module_file"));
        for forbidden in [
            "NonrootModuleKey",
            "EvaluatedNonrootModule",
            "evaluate_nonroot_module_file",
            "RootRepositoryRouteKey",
            "HostRepositorySourceFileKey",
            "HostInclude",
            "ModuleSourcePreparationKey",
            "RootModuleFilesKey",
            "RegistryPolicyKey",
            "RegistryFileKey",
            "WorkspaceSnapshotKey",
            "RepositoryMaterializationRequestKey",
            "store_evaluation_data",
            "std::fs",
            "fault",
        ] {
            assert!(!inspection.contains(forbidden), "{forbidden}");
        }
    }
    #[test]
    fn host_discovered_module_registry_value_preserves_semantics_and_provenance() {
        let key = NonrootModuleKey::new("dep", "1.0");
        let (module, events) = evaluate_direct_nonregistry_module_closure_with_events(
            key.clone(),
            crate::LogicalModuleFileId::new("https://registry.example/modules/dep/1.0/MODULE.bazel"),
            b"module(name = \"dep\", version = \"1.0\")\nbazel_dep(name = \"child\", version = \"2.0\")\n",
            &[],
            true,
        );
        assert!(events.unwrap().events().is_empty());
        let attempts: Arc<[RegistryModuleFileAttempt]> = Arc::from([
            RegistryModuleFileAttempt {
                url: RegistryFileUrl::new("https://a.example/modules/dep/1.0/MODULE.bazel"),
                sha256: None,
            },
            RegistryModuleFileAttempt {
                url: RegistryFileUrl::new("https://b.example/modules/dep/1.0/MODULE.bazel"),
                sha256: Some([7; 32]),
            },
        ]);
        let value = HostDiscoveredModule {
            module: module.unwrap(),
            provenance: HostDiscoveredModuleProvenance::Registry {
                selected_registry: RegistryBaseUrl::new("https://b.example/"),
                module_file_attempts: attempts.clone(),
            },
        };
        assert_eq!(value.module.base.expected_key, key);
        assert!(value.module.base.dependencies.contains_key("child"));
        assert!(matches!(
            &value.provenance,
            HostDiscoveredModuleProvenance::Registry {
                selected_registry,
                module_file_attempts,
            } if selected_registry.as_str().contains("b.example")
                && module_file_attempts.as_ref() == attempts.as_ref()
        ));
        assert_eq!(value, value.clone());
    }

    #[test]
    fn host_discovered_module_identity_and_typed_terminals_are_distinct() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let builtin = HostDiscoveredModuleKey::new(
            workspace.dupe(),
            NonrootModuleKey::new("bazel_tools", ""),
        );
        let registry = HostDiscoveredModuleKey::new(workspace, NonrootModuleKey::new("dep", "1.0"));
        assert_ne!(builtin, registry);
        assert!(builtin.to_string().ends_with(":bazel_tools@"));
        assert!(matches!(
            HostDiscoveredModuleError::ExplicitBuiltinOverride,
            HostDiscoveredModuleError::ExplicitBuiltinOverride
        ));
        assert!(matches!(
            HostDiscoveredModuleError::MissingVersion {
                module_name: "dep".into(),
            },
            HostDiscoveredModuleError::MissingVersion { module_name }
                if module_name == "dep"
        ));
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::root_module_bootstrap(
            RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        ));
        assert!(!HostDiscoveredModuleKey::validity(&need));
        assert!(!HostDiscoveredModuleKey::equality(&need, &need));
    }

    #[test]
    fn host_discovered_module_owner_has_no_graph_or_consumer_edge() {
        let source = include_str!("source_preparation.rs");
        let owner = source
            .split("impl Key for HostDiscoveredModuleKey")
            .nth(1)
            .unwrap()
            .split("impl fmt::Display for RepositoryMaterializationKey")
            .next()
            .unwrap();
        for required in [
            "RootModuleFilesKey",
            "BuiltinBazelToolsModuleKey",
            "ModuleSourcePreparationKey",
            "evaluate_direct_nonregistry_module_closure_with_events",
            "module_file_attempts",
            "store_evaluation_data",
        ] {
            assert!(owner.contains(required), "{required}");
        }
        for forbidden in [
            "ResolvedGraph",
            "RootModuleGraphKey",
            "RepositoryMapping",
            "PackageLoad",
            "Toolchain",
            "std::fs",
        ] {
            assert!(!owner.contains(forbidden), "{forbidden}");
        }
    }
    #[derive(Default)]
    struct HostDiscoveryTracker {
        host: Mutex<Vec<(ActivationKind, bool)>>,
        builtin: Mutex<Vec<ActivationKind>>,
    }

    impl ActivationTracker for HostDiscoveryTracker {
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
            if key.downcast_ref::<HostDiscoveredModuleKey>().is_some() {
                self.host
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_none()));
            } else if key.downcast_ref::<BuiltinBazelToolsModuleKey>().is_some() {
                self.builtin.lock().unwrap().push(activation.kind());
            }
        }
    }

    struct HostDiscoveryRegistryIo(std::collections::BTreeMap<String, Arc<[u8]>>);

    #[async_trait]
    impl crate::RegistryIo for HostDiscoveryRegistryIo {
        async fn read_exact(
            &self,
            url: &RegistryFileUrl,
        ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
            Ok(self
                .0
                .get(url.as_str())
                .map_or(crate::RegistryIoOutcome::NotFound, |bytes| {
                    crate::RegistryIoOutcome::Found(bytes.clone())
                }))
        }
    }

    async fn compute_host_discovered(
        dice: &Arc<Dice>,
        tracker: Arc<HostDiscoveryTracker>,
        root_source: &str,
        registries: &[&str],
        generation: u64,
        module: NonrootModuleKey,
    ) -> <HostDiscoveredModuleKey as Key>::Value {
        let workspace = NormalizedAbsolutePath::new("/host-discovered-test").unwrap();
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_path_buf(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                            root_source.to_owned(),
                        )),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_path_buf(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        slug_workspace_v2::WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        crate::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::RegistryUrls::new(registries.iter().copied()),
            crate::RegistryRequestGeneration(generation),
        )
        .unwrap();
        updater
            .commit()
            .await
            .compute(&HostDiscoveredModuleKey::new(workspace, module))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn host_discovered_module_dice_lifecycle_and_builtin_override_bypass() {
        let a = "https://a.invalid/modules/dep/1.0/MODULE.bazel";
        let b = "https://b.invalid/modules/dep/1.0/MODULE.bazel";
        let source: Arc<[u8]> = Arc::from(&b"module(name = \"dep\", version = \"1.0\")\n"[..]);
        let mut builder = Dice::builder();
        crate::install_registry_io(
            &mut builder,
            Arc::new(HostDiscoveryRegistryIo(
                [(a.to_owned(), source.clone()), (b.to_owned(), source)]
                    .into_iter()
                    .collect(),
            )),
        );
        let dice = Arc::new(builder.build(DetectCycles::Enabled));
        let tracker = Arc::new(HostDiscoveryTracker::default());

        let overridden = compute_host_discovered(
            &dice,
            tracker.clone(),
            "module(name = \"root\")\nlocal_path_override(module_name = \"bazel_tools\", path = \"tools\")\n",
            &[],
            0,
            NonrootModuleKey::new("bazel_tools", ""),
        )
        .await;
        assert!(matches!(
            overridden,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostDiscoveredModuleError::ExplicitBuiltinOverride))
        ));
        assert!(tracker.builtin.lock().unwrap().is_empty());

        let root = "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0\")\n";
        let first_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            1,
            NonrootModuleKey::new("dep", "1.0"),
        )
        .await;
        let selected_b = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://b.invalid"],
            2,
            NonrootModuleKey::new("dep", "1.0"),
        )
        .await;
        let second_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            3,
            NonrootModuleKey::new("dep", "1.0"),
        )
        .await;
        let warm_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            3,
            NonrootModuleKey::new("dep", "1.0"),
        )
        .await;
        assert!(HostDiscoveredModuleKey::equality(&first_a, &second_a));
        assert!(HostDiscoveredModuleKey::equality(&second_a, &warm_a));
        assert!(!HostDiscoveredModuleKey::equality(&first_a, &selected_b));
        assert!(matches!(
            first_a,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    &value.as_ref().as_ref().unwrap().provenance,
                    HostDiscoveredModuleProvenance::Registry {
                        selected_registry,
                        module_file_attempts,
                    } if selected_registry.as_str() == "https://a.invalid"
                        && module_file_attempts.len() == 1
                        && module_file_attempts[0].sha256.is_some()
                )
        ));
        let activations = tracker.host.lock().unwrap();
        assert!(
            activations
                .iter()
                .any(|(kind, event_free)| *kind == ActivationKind::Evaluated && !event_free)
        );
        assert!(
            activations
                .iter()
                .any(|(kind, _)| *kind == ActivationKind::Reused)
        );
    }
    fn nonregistry_preflight(package: &str) -> HostNonregistryPackagePreflightKey {
        HostNonregistryPackagePreflightKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", "1"),
            PackagePath::parse(package).unwrap(),
        )
    }

    const PREFLIGHT_DEFAULT: (&str, Option<&str>, bool, bool) =
        ("pkg", Some("BUILD.bazel"), false, false);

    async fn nonregistry_preflight_compute(
        dice: &Arc<Dice>,
        repo: Option<&[u8]>,
        ignore: Option<&[u8]>,
        build: Option<&[u8]>,
        deleted: &[&str],
        variant: i64,
        capture: bool,
        tracker: Option<Arc<NonregistryPreflightTracker>>,
        scenario: (&str, Option<&str>, bool, bool),
    ) -> HostNonregistryPackagePreflightValue {
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|value| value as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture {
            data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(data);
        let root_source = root("dep", "1");
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        PathBuf::from("/workspace/MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                            root_source.clone(),
                        )),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        PathBuf::from("/workspace/MODULE.bazel.lock"),
                        slug_workspace_v2::WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        let fragments = build
            .zip(scenario.1)
            .map(|(bytes, marker)| {
                let relative = if marker == "BUILD" {
                    "pkg/BUILD"
                } else {
                    "pkg/BUILD.bazel"
                };
                vec![(relative, Some(bytes))]
            })
            .unwrap_or_default();
        let omitted = scenario
            .2
            .then_some(("pkg", "BUILD.bazel"))
            .into_iter()
            .collect::<Vec<_>>();
        let directories = scenario
            .3
            .then_some(("pkg", "BUILD.bazel"))
            .into_iter()
            .collect::<Vec<_>>();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    &root_source,
                    PathObservationNamespace::Host,
                    "/workspace/dep",
                    Some(b"module(name = \"dep\", version = \"1\")\n"),
                    repo,
                    ignore,
                    &[("pkg", false)],
                    &omitted,
                    &directories,
                    &fragments,
                    &[],
                    variant,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                material("dep"),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                deleted,
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        updater
            .commit()
            .await
            .compute(&nonregistry_preflight(scenario.0))
            .await
            .unwrap()
    }

    async fn local_preflight(
        dice: &Arc<Dice>,
        build: Option<&[u8]>,
        variant: i64,
    ) -> HostNonregistryPackagePreflightValue {
        nonregistry_preflight_compute(
            dice,
            None,
            None,
            build,
            &[],
            variant,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await
    }

    #[tokio::test]
    async fn host_nonregistry_package_build_marker_and_ignore_order() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let built = nonregistry_preflight_compute(
            &dice,
            Some(b"ignore_directories([\"repo_ignored\"])\n"),
            None,
            Some(b"exports_files([])\n"),
            &[],
            1,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        assert!(matches!(built, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::BuildDotBazel))));
        let ignored = nonregistry_preflight_compute(
            &dice,
            None,
            Some(b"pkg\n"),
            Some(b"exports_files([])\n"),
            &[],
            2,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        assert!(matches!(ignored, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::Ignored))));
    }

    #[tokio::test]
    async fn host_nonregistry_package_deleted_policy_fails_closed_and_recovers() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let a = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            None,
            &[],
            1,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        let blocked = nonregistry_preflight_compute(
            &dice,
            Some(b"this is not evaluated\n"),
            Some(b"/invalid/absolute\n"),
            None,
            &["@dep+//pkg"],
            2,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        assert!(matches!(blocked, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Err(HostNonregistryPackagePreflightError::UnsupportedDeletedPackages))));
        let restored = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            None,
            &[],
            3,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        assert!(HostNonregistryPackagePreflightKey::equality(&a, &restored));
    }
    #[tokio::test]
    async fn host_nonregistry_package_repo_events_are_captured_and_reused() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        for _ in 0..2 {
            let value = nonregistry_preflight_compute(
                &dice,
                Some(b"print('captured')\n"),
                None,
                None,
                &[],
                7,
                true,
                Some(tracker.clone()),
                PREFLIGHT_DEFAULT,
            )
            .await;
            assert!(matches!(
                value,
                SourcePreparationOutcome::Complete(value)
                    if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::NoBuildFile))
            ));
        }
        assert_eq!(
            *tracker.preflight.lock().unwrap(),
            [
                (ActivationKind::Evaluated, true),
                (ActivationKind::Reused, true),
            ]
        );
        assert_eq!(
            *tracker.repo.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
    }

    async fn immutable_preflight_compute(
        dice: &Arc<Dice>,
        generation_root: &str,
        instance_id: u64,
        tracker: Arc<NonregistryPreflightTracker>,
    ) -> HostNonregistryPackagePreflightValue {
        let root_source =
            "bazel_dep(name = 'dep', version = '1')\narchive_override(module_name = 'dep')\n";
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        PathBuf::from("/workspace/MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                            root_source.to_owned(),
                        )),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        PathBuf::from("/workspace/MODULE.bazel.lock"),
                        slug_workspace_v2::WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        let instance = PathObservationInstanceId::new(instance_id);
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    root_source,
                    PathObservationNamespace::Materialization(instance),
                    generation_root,
                    None,
                    None,
                    None,
                    &[("pkg", false)],
                    &[],
                    &[],
                    &[("pkg/BUILD.bazel", Some(b"exports_files([])\n"))],
                    &[],
                    instance_id as i64,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                immutable_material(generation_root, instance),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                [NormalizedAbsolutePath::new("/workspace").unwrap()],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            Path::new("/workspace"),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
        )
        .unwrap();
        updater
            .commit()
            .await
            .compute(&nonregistry_preflight("pkg"))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn host_nonregistry_package_immutable_generation_aba() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let a = immutable_preflight_compute(&dice, "/generation/41", 41, tracker.clone()).await;
        let b = immutable_preflight_compute(&dice, "/generation/42", 42, tracker.clone()).await;
        let restored =
            immutable_preflight_compute(&dice, "/generation/41", 41, tracker.clone()).await;
        for value in [&a, &b, &restored] {
            assert!(matches!(value, SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::BuildDotBazel))));
        }
        assert!(HostNonregistryPackagePreflightKey::equality(&a, &restored));
        let local = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            Some(b"local\n"),
            &[],
            43,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        let category_b =
            immutable_preflight_compute(&dice, "/generation/44", 44, tracker.clone()).await;
        let local_restored = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            Some(b"local\n"),
            &[],
            45,
            false,
            None,
            PREFLIGHT_DEFAULT,
        )
        .await;
        assert!(HostNonregistryPackagePreflightKey::equality(
            &local,
            &local_restored,
        ));
        assert!(
            matches!(category_b, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::BuildDotBazel)))
        );
        assert!(
            tracker
                .preflight
                .lock()
                .unwrap()
                .iter()
                .filter(|(kind, _)| *kind == ActivationKind::Evaluated)
                .count()
                >= 2
        );
    }
    #[tokio::test]
    async fn host_nonregistry_package_local_table_and_failure_recovery() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let a = local_preflight(&dice, Some(b"a\n"), 20).await;
        let b = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            None,
            &[],
            21,
            false,
            None,
            ("pkg", None, false, false),
        )
        .await;
        let restored = local_preflight(&dice, Some(b"a\n"), 22).await;
        assert!(matches!(&b, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::NoBuildFile))));
        assert!(HostNonregistryPackagePreflightKey::equality(&a, &restored));

        for scenario in [
            ("pkg", Some("BUILD"), false, false),
            ("pkg", Some("BUILD"), false, true),
        ] {
            let value = nonregistry_preflight_compute(
                &dice,
                None,
                None,
                Some(b"fallback\n"),
                &[],
                23,
                false,
                None,
                scenario,
            )
            .await;
            assert!(matches!(value, SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::Build))));
        }

        let invalid = nonregistry_preflight_compute(
            &dice,
            Some(b"not evaluated\n"),
            None,
            None,
            &[],
            24,
            false,
            None,
            ("bad:name", None, false, false),
        )
        .await;
        assert!(matches!(invalid, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::InvalidPackageName { .. }))));

        let need = nonregistry_preflight_compute(
            &dice,
            None,
            None,
            None,
            &[],
            25,
            false,
            None,
            ("pkg", None, true, false),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let recovered = local_preflight(&dice, Some(b"recovered\n"), 26).await;
        assert!(
            matches!(recovered, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Ok(HostNonregistryPackagePreflight::BuildDotBazel)))
        );
    }
}
