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

async fn observed_repository_source_file(
    ctx: &mut DiceComputations<'_>,
    namespace: PathObservationNamespace,
    materialized_root: &Path,
    relative: &Path,
    repo_relative_path: Arc<PathBuf>,
) -> PathResult<RepositorySourceFileValue, RepositorySourceFileError> {
    let logical_path = match NormalizedAbsolutePath::new(materialized_root.join(relative)) {
        Ok(path) => path,
        Err(_) => {
            return PathOutcome::Complete(Err(
                RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path },
            ));
        }
    };
    let resolved = match ctx
        .compute(&ResolvedPathKey::new(namespace, logical_path))
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
            return PathOutcome::Complete(Ok(RepositorySourceFileValue::Absent));
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
            PathOutcome::Complete(Ok(RepositorySourceFileValue::Present(bytes.dupe())))
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
        | PathObservationResult::DirectoryEntries(_) => {
            unreachable!("FileBytes demand must return a FileBytes observation")
        }
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
                    &source_root,
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
                    &generation_root,
                    relative,
                    repo_relative_path,
                )
                .await,
            ),
        }
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
                | PathObservationResult::DirectoryEntries(_) => {
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

    use dice::DynKey;

    use super::*;

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
}
