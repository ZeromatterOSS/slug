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
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
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
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;
use starlark_map::small_map::SmallMap;

use crate::ModuleKey;
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
use crate::registry_module_file_url;

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
        namespace,
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
                logical_path,
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
        | PathObservationResult::WindowsLongPath(_) => {
            unreachable!("FileBytes demand must return a FileBytes observation")
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
impl Key for HostRepositorySourceFileKey {
    type Value = SourcePreparationResult<HostRepositorySourceFileValue, RepositorySourceFileError>;

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
        let kind = match request_kind(self.route.workspace(), self.route.repo_spec()) {
            Ok(kind) => kind,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    RepositorySourceFileError::Materialization {
                        repo_relative_path,
                        error: Arc::new(error),
                    },
                ));
            }
        };
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: self.route.workspace().dupe(),
                canonical_repo: self.route.canonical_repo().clone(),
            },
            repo_spec: self.route.repo_spec().clone(),
            kind,
        });
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
        host_repository_source_file_value(
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
                | PathObservationResult::WindowsLongPath(_) => {
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

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_events_v2::CaptureEvaluationEvents;
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
        dependencies: Mutex<Vec<String>>,
    }

    impl ActivationTracker for HostSourceDependencyTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<HostRepositorySourceFileKey>().is_some() {
                self.dependencies
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

        let dependencies = tracker.dependencies.lock().unwrap().clone();
        assert_eq!(
            dependencies,
            ["repository-materialization-result:@@dep+".to_owned()]
        );
        assert!(dependencies.iter().all(|dependency| {
            !dependency.starts_with("repository-materialization:")
                && !dependency.starts_with("repository-materialization-request:")
                && !dependency.starts_with("root-module-files:")
                && !dependency.starts_with("workspace-snapshot:")
        }));
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
}
