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
use std::ops::ControlFlow;
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
use slug_workspace_v2::ObservedPathDirectoryListing;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::ObservedResolvedPath;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryListingError;
use slug_workspace_v2::PathDirectoryListingKey;
use slug_workspace_v2::PathDirectoryListingObservationKey;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::ResolvedPathState;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::BuiltinBazelToolsSnapshot;
use crate::BuiltinBazelToolsSourceFileError;
use crate::BuiltinBazelToolsSourceFileKey;
use crate::BuiltinBazelToolsSourceFileValue;
use crate::EvaluatedNonrootModule;
use crate::GeneratedRepositoryFileEffectPlan;
use crate::HostRepositorySourceCapability;
use crate::HostRepositorySourceCapabilitySource;
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
use crate::RootModuleOverride;
use crate::RootRepositoryRoute;
use crate::apply_unified_patch;
use crate::builtin_repository::BuiltinBazelToolsDirectoryListingKey;
use crate::builtin_repository::BuiltinBazelToolsModuleError;
use crate::builtin_repository::BuiltinBazelToolsModuleKey;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupError;
use crate::host_package::ExternalRepositoryPackageLookupKey;
use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::HostBuildFileName;
use crate::host_package::ObservedExternalRepositoryPackageLookup;
use crate::host_package::invalid_package_name;
use crate::module_eval::DirectNonregistryEvaluationError;
use crate::module_eval::DirectNonregistryIncludeFile;
use crate::module_eval::HostEffectiveModuleOverride;
use crate::module_eval::HostEffectiveModuleOverrideError;
use crate::module_eval::HostEffectiveModuleOverrideKey;
use crate::module_eval::HostEffectiveModuleOverrideObservationKey;
use crate::module_eval::NonrootIncludeRequest;
use crate::module_eval::NonrootModuleFileInspection;
use crate::module_eval::ObservedHostEffectiveModuleOverride;
use crate::module_eval::evaluate_direct_nonregistry_module_closure_with_events;
use crate::module_eval::parse_nonroot_include;
use crate::module_eval::parse_root_include;
use crate::module_eval::validate_root_module_source;
use crate::module_version::BazelModuleVersion;
use crate::module_version::BazelModuleVersionParseError;
use crate::package_policy::CanonicalDeletedPackagesProjectionKey;
use crate::registry_dice::RegistryFileObservationKey;
use crate::registry_dice::RegistryPolicyObservationKey;
use crate::registry_module_file_url;
use crate::repository_ignore::HostNonregistryRepositoryIgnoreKey;
use crate::repository_ignore::HostNonregistryRepositoryIgnoreObservationKey;
use crate::repository_ignore::HostRepositoryIgnoreError;
use crate::repository_ignore::RepositoryIgnoreMatcher;

mod canonical_repository_source;
pub use canonical_repository_source::*;
mod repository_source_observation;
#[cfg(test)]
use repository_source_observation::HostRepositorySourceObservationErrorKind;
pub use repository_source_observation::*;

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
    GeneratedFileEffects(GeneratedRepositoryFileEffectPlan),
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
            | (
                RepositoryMaterializationKind::GeneratedFileEffects(_),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct ModuleSourcePreparationObservationKey(ModuleSourcePreparationKey);

#[allow(dead_code)]
impl ModuleSourcePreparationObservationKey {
    fn new(workspace: PathBuf, module_name: CompactString, version: CompactString) -> Self {
        Self(ModuleSourcePreparationKey {
            workspace,
            module_name,
            version,
        })
    }
}

impl fmt::Display for ModuleSourcePreparationObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type ModuleSourcePreparationResult =
    Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
pub(crate) struct ObservedModuleSourcePreparation {
    result: ModuleSourcePreparationResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedModuleSourcePreparation {
    pub(crate) fn result(&self) -> &ModuleSourcePreparationResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type ModuleSourcePreparationDriverOutcome = SourcePreparationOutcome<
    Result<(ModuleSourcePreparationResult, PathObservationEpoch), ObservedPathFrontierError>,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleSourcePreparationMode {
    Legacy,
    Observed,
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
    NonRegistry {
        closure: HostNonregistryPreparedClosure,
    },
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum HostRepositoryLocalPathPolicy {
    WorkspaceRelative,
    CommandAbsolute,
    LocalUnsupported,
}

fn local_path_policy(effective: &HostEffectiveModuleOverride) -> HostRepositoryLocalPathPolicy {
    if effective.is_command() {
        HostRepositoryLocalPathPolicy::CommandAbsolute
    } else {
        HostRepositoryLocalPathPolicy::WorkspaceRelative
    }
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
    InvalidBuiltinVersion {
        version: CompactString,
    },
    MissingVersion {
        module_name: CompactString,
    },
    SourcePreparationCompute(Arc<str>),
    SourcePreparation(ModuleSourcePreparationError),
    InvalidNonRegistryVersion {
        module_name: CompactString,
        version: CompactString,
    },
    NonRegistryClosureCompute(Arc<str>),
    NonRegistryClosure(HostNonregistryModuleClosureError),
    NonRegistryCycle {
        closure: HostNonregistryPreparedClosure,
        capability: NonregistryIncludeCycleCapability,
    },
    NonRegistryUnsupported {
        module_name: CompactString,
    },
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
            Self::InvalidNonRegistryVersion {
                module_name,
                version,
            } => write!(
                f,
                "nonregistry module {module_name} requires the empty effective version, got {version}"
            ),
            Self::NonRegistryClosureCompute(message) => {
                write!(f, "nonregistry MODULE closure failed to compute: {message}")
            }
            Self::NonRegistryClosure(error) => write!(f, "{error:?}"),
            Self::NonRegistryCycle { capability, .. } => write!(
                f,
                "nonregistry MODULE include cycle at {:?}:{}:{}",
                capability.repeated_raw_label,
                capability.repeated_location.start_line,
                capability.repeated_location.start_column
            ),
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
    pub(crate) fn try_new(
        workspace: NormalizedAbsolutePath,
        module: NonrootModuleKey,
    ) -> Result<Self, BazelModuleVersionParseError> {
        let version = BazelModuleVersion::parse(&module.version)?;
        Ok(Self {
            workspace,
            module: NonrootModuleKey::new(module.name, version.normalized()),
        })
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling stays callerless until selected-graph activation.
pub(crate) struct HostDiscoveredModuleObservationKey(HostDiscoveredModuleKey);

#[allow(dead_code)]
impl HostDiscoveredModuleObservationKey {
    pub(crate) fn try_new(
        workspace: NormalizedAbsolutePath,
        module: NonrootModuleKey,
    ) -> Result<Self, BazelModuleVersionParseError> {
        HostDiscoveredModuleKey::try_new(workspace, module).map(Self)
    }
}

impl fmt::Display for HostDiscoveredModuleObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type HostDiscoveredModuleResult = Arc<Result<HostDiscoveredModule, HostDiscoveredModuleError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)]
pub(crate) struct ObservedHostDiscoveredModule {
    result: HostDiscoveredModuleResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostDiscoveredModule {
    pub(crate) fn result(&self) -> &HostDiscoveredModuleResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostDiscoveredModuleClosureFrontier(HostNonregistryModuleClosureObservationError);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostDiscoveredModuleObservationError {
    EffectiveFrontier(ObservedPathFrontierError),
    ClosureFrontier(HostDiscoveredModuleClosureFrontier),
    PreparationFrontier(ObservedPathFrontierError),
    MergeFrontier(ObservedPathFrontierError),
}

/// Classifies an observed selected-graph failure without collapsing a DICE
/// computation failure into the retryable path frontier.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostSelectedObservationFrontier {
    Path(ObservedPathFrontierError),
    Infrastructure(Arc<str>),
}

fn preflight_observation_frontier(
    error: &HostNonregistryPackagePreflightObservationError,
) -> HostSelectedObservationFrontier {
    match error {
        HostNonregistryPackagePreflightObservationError::EffectiveFrontier(error)
        | HostNonregistryPackagePreflightObservationError::IgnoreFrontier(error)
        | HostNonregistryPackagePreflightObservationError::MarkerFrontier { error, .. } => {
            HostSelectedObservationFrontier::Path(error.clone())
        }
        HostNonregistryPackagePreflightObservationError::EffectiveCompute(message)
        | HostNonregistryPackagePreflightObservationError::PolicyCompute(message)
        | HostNonregistryPackagePreflightObservationError::IgnoreCompute(message)
        | HostNonregistryPackagePreflightObservationError::MarkerCompute { message, .. } => {
            HostSelectedObservationFrontier::Infrastructure(message.dupe())
        }
    }
}

impl HostDiscoveredModuleObservationError {
    #[doc(hidden)]
    pub(crate) fn selected_frontier(&self) -> HostSelectedObservationFrontier {
        match self {
            Self::EffectiveFrontier(error) | Self::PreparationFrontier(error) => {
                HostSelectedObservationFrontier::Path(error.clone())
            }
            Self::MergeFrontier(error) => HostSelectedObservationFrontier::Path(error.clone()),
            Self::ClosureFrontier(HostDiscoveredModuleClosureFrontier(error)) => match error {
                HostNonregistryModuleClosureObservationError::EffectiveFrontier(error)
                | HostNonregistryModuleClosureObservationError::MaterializationFrontier(error)
                | HostNonregistryModuleClosureObservationError::RootSourceFrontier(error) => {
                    HostSelectedObservationFrontier::Path(error.clone())
                }
                HostNonregistryModuleClosureObservationError::EffectiveCompute(message) => {
                    HostSelectedObservationFrontier::Infrastructure(message.dupe())
                }
                HostNonregistryModuleClosureObservationError::PreparationFrontier(error) => {
                    match error {
                        NonregistryPreparationFrontierError::Path(error) => {
                            HostSelectedObservationFrontier::Path(error.clone())
                        }
                        NonregistryPreparationFrontierError::Package(error) => {
                            preflight_observation_frontier(error)
                        }
                    }
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum HostDiscoveredModuleMode {
    Legacy,
    Observed,
}

type HostDiscoveredModuleDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            HostDiscoveredModuleResult,
            PathObservationEpoch,
            Option<EventBatch>,
        ),
        HostDiscoveredModuleObservationError,
    >,
>;

fn discovered_complete(
    result: Result<HostDiscoveredModule, HostDiscoveredModuleError>,
    observations: PathObservationEpoch,
    events: Option<EventBatch>,
) -> HostDiscoveredModuleDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations, events)))
}

fn discovered_error(
    error: HostDiscoveredModuleError,
    observations: PathObservationEpoch,
) -> HostDiscoveredModuleDriverOutcome {
    discovered_complete(Err(error), observations, None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostDiscoveredComputeStage {
    Effective,
    Closure,
    Preparation,
}

fn discovered_compute_error(
    stage: HostDiscoveredComputeStage,
    message: Arc<str>,
    observations: PathObservationEpoch,
) -> HostDiscoveredModuleDriverOutcome {
    let error = match stage {
        HostDiscoveredComputeStage::Effective => {
            HostDiscoveredModuleError::RootModuleFiles(CompactString::new(message.as_ref()))
        }
        HostDiscoveredComputeStage::Closure => {
            HostDiscoveredModuleError::NonRegistryClosureCompute(message)
        }
        HostDiscoveredComputeStage::Preparation => {
            HostDiscoveredModuleError::SourcePreparationCompute(message)
        }
    };
    discovered_error(error, observations)
}

fn finish_discovered_observed_child<T, E>(
    outcome: SourcePreparationOutcome<Result<T, E>>,
    outer: impl FnOnce(E) -> HostDiscoveredModuleObservationError,
) -> ControlFlow<HostDiscoveredModuleDriverOutcome, T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(outer(error))))
        }
        SourcePreparationOutcome::Complete(Ok(value)) => ControlFlow::Continue(value),
    }
}

fn merge_discovered_prefix(
    prefix: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, HostDiscoveredModuleDriverOutcome> {
    merge_path_observations(prefix, incoming).map_err(|error| {
        SourcePreparationOutcome::Complete(Err(
            HostDiscoveredModuleObservationError::MergeFrontier(error),
        ))
    })
}

impl HostDiscoveredModuleKey {
    async fn discover_nonregistry(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostDiscoveredModuleMode,
        observations: PathObservationEpoch,
    ) -> HostDiscoveredModuleDriverOutcome {
        let key = HostNonregistryModuleClosureKey::new(self.workspace.dupe(), self.module.clone());
        let (result, incoming) = if mode == HostDiscoveredModuleMode::Legacy {
            let outcome = match ctx.compute(&key).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    return discovered_compute_error(
                        HostDiscoveredComputeStage::Closure,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            };
            match outcome {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(result) => {
                    (result, PathObservationEpoch::empty())
                }
            }
        } else {
            let outcome = match ctx
                .compute(&HostNonregistryModuleClosureObservationKey(key))
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    return discovered_compute_error(
                        HostDiscoveredComputeStage::Closure,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            };
            match finish_discovered_observed_child(outcome, |error| {
                HostDiscoveredModuleObservationError::ClosureFrontier(
                    HostDiscoveredModuleClosureFrontier(error),
                )
            }) {
                ControlFlow::Break(outcome) => return outcome,
                ControlFlow::Continue(observed) => {
                    (observed.result().dupe(), observed.observations().dupe())
                }
            }
        };
        let observations = match merge_discovered_prefix(&observations, &incoming) {
            Ok(observations) => observations,
            Err(outcome) => return outcome,
        };
        let closure = match result.as_ref() {
            Ok(HostNonregistryModuleClosure::Supported(closure)) => closure.clone(),
            Ok(HostNonregistryModuleClosure::UnsupportedCycle {
                closure,
                capability,
            }) => {
                return discovered_error(
                    HostDiscoveredModuleError::NonRegistryCycle {
                        closure: closure.clone(),
                        capability: capability.clone(),
                    },
                    observations,
                );
            }
            Err(error) => {
                return discovered_error(
                    HostDiscoveredModuleError::NonRegistryClosure(error.clone()),
                    observations,
                );
            }
        };
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let included = closure
            .fragments
            .iter()
            .map(|fragment| DirectNonregistryIncludeFile {
                raw_label: fragment.occurrence.raw_label.as_str(),
                logical_id: crate::LogicalModuleFileId::new(
                    fragment.logical_path.as_path().display().to_string(),
                ),
                source: fragment.bytes.as_ref(),
            })
            .collect::<Vec<_>>();
        let (module, events) = evaluate_direct_nonregistry_module_closure_with_events(
            self.module.clone(),
            crate::LogicalModuleFileId::new(
                closure.root.logical_path.as_path().display().to_string(),
            ),
            closure.root.bytes.as_ref(),
            &included,
            capture_events,
        );
        let value = module
            .map(|module| HostDiscoveredModule {
                module,
                provenance: HostDiscoveredModuleProvenance::NonRegistry { closure },
            })
            .map_err(HostDiscoveredModuleError::Evaluation);
        let events = capture_events.then(|| events.unwrap_or_else(EventBatch::empty));
        discovered_complete(value, observations, events)
    }
}
impl HostDiscoveredModuleKey {
    async fn discover_effective(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostDiscoveredModuleMode,
    ) -> Result<
        (
            <HostEffectiveModuleOverrideKey as Key>::Value,
            PathObservationEpoch,
        ),
        HostDiscoveredModuleDriverOutcome,
    > {
        if mode == HostDiscoveredModuleMode::Legacy {
            return match ctx
                .compute(&HostEffectiveModuleOverrideKey::new(
                    self.workspace.dupe(),
                    self.module.name.clone(),
                ))
                .await
            {
                Ok(result) => Ok((result, PathObservationEpoch::empty())),
                Err(error) => Err(discovered_compute_error(
                    HostDiscoveredComputeStage::Effective,
                    Arc::from(error.to_string()),
                    PathObservationEpoch::empty(),
                )),
            };
        }
        match ctx
            .compute(&HostEffectiveModuleOverrideObservationKey::new(
                self.workspace.dupe(),
                self.module.name.clone(),
            ))
            .await
        {
            Err(error) => Err(discovered_compute_error(
                HostDiscoveredComputeStage::Effective,
                Arc::from(error.to_string()),
                PathObservationEpoch::empty(),
            )),
            Ok(SourcePreparationOutcome::Need(need)) => Err(SourcePreparationOutcome::Need(need)),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                Err(SourcePreparationOutcome::Complete(Err(
                    HostDiscoveredModuleObservationError::EffectiveFrontier(error),
                )))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                Ok((observed.result().dupe(), observed.observations().dupe()))
            }
        }
    }

    async fn drive(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: HostDiscoveredModuleMode,
    ) -> HostDiscoveredModuleDriverOutcome {
        let (effective_result, observations) = match self.discover_effective(ctx, mode).await {
            Ok(complete) => complete,
            Err(outcome) => return outcome,
        };
        let effective = match effective_result.as_ref() {
            Ok(effective) => effective,
            Err(error) => {
                return discovered_error(
                    HostDiscoveredModuleError::RootModuleFiles(error.to_string().into()),
                    observations,
                );
            }
        };
        let override_ = effective.override_();
        if self.module.name == "bazel_tools" && !effective.is_command() {
            if override_.is_some() {
                return discovered_error(
                    HostDiscoveredModuleError::ExplicitBuiltinOverride,
                    observations,
                );
            }
            if !self.module.version.is_empty() {
                return discovered_error(
                    HostDiscoveredModuleError::InvalidBuiltinVersion {
                        version: self.module.version.clone(),
                    },
                    observations,
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
                    return discovered_error(
                        HostDiscoveredModuleError::BuiltinCompute(Arc::from(error.to_string())),
                        observations,
                    );
                }
            };
            return discovered_complete(
                match value.as_ref() {
                    Ok(value) => Ok(HostDiscoveredModule {
                        module: value.module.clone(),
                        provenance: HostDiscoveredModuleProvenance::BuiltinBazelTools {
                            route_identity: value.route_identity.clone(),
                            module_sha256: value.module_sha256,
                        },
                    }),
                    Err(error) => Err(HostDiscoveredModuleError::Builtin(error.clone())),
                },
                observations,
                None,
            );
        }
        if matches!(override_, Some(RootModuleOverride::NonRegistry(_))) {
            if !self.module.version.is_empty() {
                return discovered_error(
                    HostDiscoveredModuleError::InvalidNonRegistryVersion {
                        module_name: self.module.name.clone(),
                        version: self.module.version.clone(),
                    },
                    observations,
                );
            }
            return self.discover_nonregistry(ctx, mode, observations).await;
        }
        if self.module.version.is_empty() {
            return discovered_error(
                HostDiscoveredModuleError::MissingVersion {
                    module_name: self.module.name.clone(),
                },
                observations,
            );
        }
        let preparation_key = ModuleSourcePreparationKey {
            workspace: self.workspace.as_path().to_path_buf(),
            module_name: self.module.name.clone(),
            version: self.module.version.clone(),
        };
        let (preparation_result, incoming) = if mode == HostDiscoveredModuleMode::Legacy {
            let outcome = match ctx.compute(&preparation_key).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    return discovered_compute_error(
                        HostDiscoveredComputeStage::Preparation,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            };
            match outcome {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(result) => {
                    (result, PathObservationEpoch::empty())
                }
            }
        } else {
            let outcome = match ctx
                .compute(&ModuleSourcePreparationObservationKey(preparation_key))
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    return discovered_compute_error(
                        HostDiscoveredComputeStage::Preparation,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            };
            match finish_discovered_observed_child(outcome, |error| {
                HostDiscoveredModuleObservationError::PreparationFrontier(error)
            }) {
                ControlFlow::Break(outcome) => return outcome,
                ControlFlow::Continue(observed) => {
                    (observed.result().dupe(), observed.observations().dupe())
                }
            }
        };
        let observations = match merge_discovered_prefix(&observations, &incoming) {
            Ok(observations) => observations,
            Err(outcome) => return outcome,
        };
        let preparation = match preparation_result.as_ref() {
            Ok(preparation) => preparation,
            Err(error) => {
                return discovered_error(
                    HostDiscoveredModuleError::SourcePreparation(error.clone()),
                    observations,
                );
            }
        };
        let ModuleSourcePreparation::Registry {
            bytes,
            selected_registry,
            module_file_attempts,
        } = preparation
        else {
            return discovered_error(
                HostDiscoveredModuleError::NonRegistryUnsupported {
                    module_name: self.module.name.clone(),
                },
                observations,
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
        let value = module
            .map(|module| HostDiscoveredModule {
                module,
                provenance: HostDiscoveredModuleProvenance::Registry {
                    selected_registry: selected_registry.clone(),
                    module_file_attempts: module_file_attempts.clone(),
                },
            })
            .map_err(HostDiscoveredModuleError::Evaluation);
        let events = capture_events.then(|| events.unwrap_or_else(EventBatch::empty));
        discovered_complete(value, observations, events)
    }
}

fn store_discovered_events(ctx: &mut DiceComputations<'_>, events: Option<EventBatch>) {
    if let Some(events) = events {
        ctx.store_evaluation_data(events)
            .expect("Host discovered MODULE stores exactly one event batch");
    }
}
fn project_legacy_discovered(
    result: HostDiscoveredModuleResult,
) -> SourcePreparationOutcome<HostDiscoveredModuleResult> {
    SourcePreparationOutcome::Complete(result)
}

#[async_trait]
impl Key for HostDiscoveredModuleKey {
    type Value = SourcePreparationOutcome<HostDiscoveredModuleResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match self.drive(ctx, HostDiscoveredModuleMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _, events))) => {
                store_discovered_events(ctx, events);
                project_legacy_discovered(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy discovery has no observed frontier")
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
impl Key for HostDiscoveredModuleObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostDiscoveredModule, HostDiscoveredModuleObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match self.0.drive(ctx, HostDiscoveredModuleMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations, events))) => {
                store_discovered_events(ctx, events);
                SourcePreparationOutcome::Complete(Ok(ObservedHostDiscoveredModule {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct RepositorySourceFileObservationKey(pub(crate) RepositorySourceFileKey);

type RepositorySourceResult = Arc<Result<RepositorySourceFileValue, RepositorySourceFileError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedRepositorySourceFileValue {
    result: RepositorySourceResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedRepositorySourceFileValue {
    pub(crate) fn result(&self) -> &RepositorySourceResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRepositoryPathKey {
    route: RootRepositoryRoute,
    repo_relative_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRepositoryPathObservationKey(pub(crate) HostRepositoryPathKey);

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

impl fmt::Display for HostRepositoryPathObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostRepositoryPathValue(ResolvedPath);

impl HostRepositoryPathValue {
    pub(crate) fn resolved(&self) -> &ResolvedPath {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRepositoryPath {
    pub(crate) result: Arc<Result<HostRepositoryPathValue, RepositorySourceFileError>>,
    pub(crate) observations: PathObservationEpoch,
}

#[doc(hidden)]
pub type HostRepositoryDirectoryListing = PathDirectoryListing;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostRepositoryDirectoryListingError {
    directory: Arc<PackagePath>,
    kind: HostRepositoryDirectoryListingErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostRepositoryDirectoryListingErrorKind {
    Builtin,
    BuiltinCompute,
    Materialization,
    MaterializationCompute,
    InvalidMaterializedPath,
    Observation {
        operation: PathObservationOperation,
    },
    InconsistentState {
        operation: PathObservationOperation,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    WrongKind {
        expected: PathNodeKind,
        actual: PathNodeKind,
    },
    Cycle,
    InfiniteExpansion,
    ListingCompute,
}

impl HostRepositoryDirectoryListingError {
    fn new(directory: Arc<PackagePath>, kind: HostRepositoryDirectoryListingErrorKind) -> Self {
        Self { directory, kind }
    }
}

impl fmt::Display for HostRepositoryDirectoryListingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostRepositoryDirectoryListingError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositoryDirectoryListingKey {
    route: RootRepositoryRoute,
    directory: PackagePath,
}

impl HostRepositoryDirectoryListingKey {
    pub fn new(route: RootRepositoryRoute, directory: PackagePath) -> Self {
        Self { route, directory }
    }
}

impl Hash for HostRepositoryDirectoryListingKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.directory.hash(state);
    }
}

impl fmt::Display for HostRepositoryDirectoryListingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-repository-directory-listing:{}://{}",
            self.route.canonical_repo(),
            self.directory
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRepositoryDirectoryListingObservationKey(HostRepositoryDirectoryListingKey);

impl HostRepositoryDirectoryListingObservationKey {
    pub fn new(route: RootRepositoryRoute, directory: PackagePath) -> Self {
        Self(HostRepositoryDirectoryListingKey::new(route, directory))
    }
}

impl fmt::Display for HostRepositoryDirectoryListingObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRepositoryDirectoryListing {
    result: Arc<Result<HostRepositoryDirectoryListing, HostRepositoryDirectoryListingError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRepositoryDirectoryListing {
    pub fn result(
        &self,
    ) -> &Arc<Result<HostRepositoryDirectoryListing, HostRepositoryDirectoryListingError>> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceFileKey {
    route: RootRepositoryRoute,
    repo_relative_path: PathBuf,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRepositorySourceFileObservationKey(HostRepositorySourceFileKey);

impl HostRepositorySourceFileObservationKey {
    pub fn new(route: RootRepositoryRoute, repo_relative_path: PathBuf) -> Self {
        Self(HostRepositorySourceFileKey::new(route, repo_relative_path))
    }
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

impl fmt::Display for HostRepositorySourceFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRepositorySourceFile {
    result: Arc<Result<HostRepositorySourceFileValue, RepositorySourceFileError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRepositorySourceFile {
    pub fn result(&self) -> &Arc<Result<HostRepositorySourceFileValue, RepositorySourceFileError>> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModuleFileKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: slug_identity_v2::ApparentRepoName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
pub(crate) struct DirectLocalModuleFileObservationKey(DirectLocalModuleFileKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalModuleFile(RootRepositoryRoute, HostRepositorySourceFileValue);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedDirectLocalModuleFile {
    result: Arc<Result<DirectLocalModuleFile, DirectLocalModuleFileError>>,
    observations: PathObservationEpoch,
}

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
        if apparent_repo.is_root() {
            return Err("direct local module file requires a nonroot apparent name".to_owned());
        }
        Ok(Self {
            workspace,
            apparent_repo,
        })
    }
}

impl fmt::Display for DirectLocalModuleFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("direct-local-module-file:")?;
        self.workspace.fmt(f)?;
        write!(f, ":@{}", self.apparent_repo.as_str())
    }
}

impl fmt::Display for DirectLocalModuleFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type DirectLocalModuleFileDriverOutcome =
    SourcePreparationOutcome<Result<ObservedDirectLocalModuleFile, ObservedPathFrontierError>>;

fn merge_path_observations(
    first: &PathObservationEpoch,
    second: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        first
            .observations()
            .iter()
            .chain(second.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn direct_local_file_complete(
    result: Result<DirectLocalModuleFile, DirectLocalModuleFileError>,
    observations: PathObservationEpoch,
) -> DirectLocalModuleFileDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleFile {
        result: Arc::new(result),
        observations,
    }))
}

fn direct_local_observed_child<T>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
) -> ControlFlow<DirectLocalModuleFileDriverOutcome, T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(result) => result.map_or_else(
            |error| ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))),
            ControlFlow::Continue,
        ),
    }
}

async fn drive_direct_local_module_file(
    ctx: &mut DiceComputations<'_>,
    key: &DirectLocalModuleFileKey,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModuleFileDriverOutcome {
    let route_key =
        crate::RootRepositoryRouteKey::new(key.workspace.dupe(), key.apparent_repo.clone())
            .expect("direct key rejects root names");
    let (route, route_observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&route_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(route)) => {
                (route.as_ref().clone(), PathObservationEpoch::empty())
            }
            Err(error) => {
                return direct_local_file_complete(
                    Err(DirectLocalModuleFileError::RouteCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                );
            }
        },
        HostRepositoryObservationMode::Observed => {
            let outcome = ctx
                .compute(
                    &crate::RootRepositoryRouteObservationKey::new(
                        key.workspace.dupe(),
                        key.apparent_repo.clone(),
                    )
                    .expect("direct key rejects root names"),
                )
                .await;
            let observed = match outcome {
                Err(error) => {
                    return direct_local_file_complete(
                        Err(DirectLocalModuleFileError::RouteCompute(
                            error.to_string().into(),
                        )),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(outcome) => match direct_local_observed_child(
                    outcome.map(|value| value.map_err(|error| error.ordinary_path())),
                ) {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(observed) => observed,
                },
            };
            let observations = match merge_path_observations(
                &PathObservationEpoch::empty(),
                observed.observations(),
            ) {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
            (observed.result().as_ref().clone(), observations)
        }
    };
    let route = match route {
        Ok(route) => route,
        Err(error) => {
            return direct_local_file_complete(
                Err(DirectLocalModuleFileError::Route(error)),
                route_observations,
            );
        }
    };
    let source_key = HostRepositorySourceFileKey::new(route.clone(), PathBuf::from("MODULE.bazel"));
    let (source, observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&source_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(source)) => (source, route_observations),
            Err(error) => {
                return direct_local_file_complete(
                    Err(DirectLocalModuleFileError::SourceCompute(
                        error.to_string().into(),
                    )),
                    route_observations,
                );
            }
        },
        HostRepositoryObservationMode::Observed => {
            let outcome = ctx
                .compute(&HostRepositorySourceFileObservationKey(source_key))
                .await;
            let observed = match outcome {
                Err(error) => {
                    return direct_local_file_complete(
                        Err(DirectLocalModuleFileError::SourceCompute(
                            error.to_string().into(),
                        )),
                        route_observations,
                    );
                }
                Ok(outcome) => match direct_local_observed_child(outcome) {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(observed) => observed,
                },
            };
            let observations =
                match merge_path_observations(&route_observations, observed.observations()) {
                    Ok(observations) => observations,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                };
            (observed.result().as_ref().clone(), observations)
        }
    };
    direct_local_file_complete(
        source
            .map(|source| DirectLocalModuleFile(route, source))
            .map_err(DirectLocalModuleFileError::Source),
        observations,
    )
}

#[async_trait]
impl Key for DirectLocalModuleFileKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<DirectLocalModuleFile, DirectLocalModuleFileError>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_module_file(ctx, self, HostRepositoryObservationMode::Legacy)
            .await
            .map(|observed| {
                observed
                    .expect("legacy direct-local file cannot produce an observed outer error")
                    .result
            })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for DirectLocalModuleFileObservationKey {
    type Value = DirectLocalModuleFileDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_module_file(ctx, &self.0, HostRepositoryObservationMode::Observed).await
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct DirectLocalModuleInspectionObservationKey(DirectLocalModuleInspectionKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct DirectLocalModuleInspection(DirectLocalModuleFile, Option<NonrootModuleFileInspection>);

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum DirectLocalModuleInspectionError {
    InputCompute(Arc<str>),
    Input(DirectLocalModuleFileError),
    Inspection(NormalizedAbsolutePath, Arc<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedDirectLocalModuleInspection {
    result: Arc<Result<DirectLocalModuleInspection, DirectLocalModuleInspectionError>>,
    observations: PathObservationEpoch,
}

type DirectLocalModuleInspectionDriverOutcome = SourcePreparationOutcome<
    Result<ObservedDirectLocalModuleInspection, ObservedPathFrontierError>,
>;

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

impl fmt::Display for DirectLocalModuleInspectionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

fn direct_local_inspection_complete(
    result: Result<DirectLocalModuleInspection, DirectLocalModuleInspectionError>,
    observations: PathObservationEpoch,
) -> DirectLocalModuleInspectionDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleInspection {
        result: Arc::new(result),
        observations,
    }))
}

fn direct_local_inspection_observed_child(
    outcome: DirectLocalModuleFileDriverOutcome,
) -> ControlFlow<DirectLocalModuleInspectionDriverOutcome, ObservedDirectLocalModuleFile> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(result) => result.map_or_else(
            |error| ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))),
            ControlFlow::Continue,
        ),
    }
}

async fn drive_direct_local_module_inspection(
    ctx: &mut DiceComputations<'_>,
    key: &DirectLocalModuleInspectionKey,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModuleInspectionDriverOutcome {
    let input_key = DirectLocalModuleFileKey::new(key.0.dupe(), key.1.clone())
        .expect("direct inspection key rejects root names");
    let (input, observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&input_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(input)) => {
                (input.as_ref().clone(), PathObservationEpoch::empty())
            }
            Err(error) => {
                return direct_local_inspection_complete(
                    Err(DirectLocalModuleInspectionError::InputCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                );
            }
        },
        HostRepositoryObservationMode::Observed => {
            let outcome = ctx
                .compute(&DirectLocalModuleFileObservationKey(input_key))
                .await;
            let observed = match outcome {
                Err(error) => {
                    return direct_local_inspection_complete(
                        Err(DirectLocalModuleInspectionError::InputCompute(
                            error.to_string().into(),
                        )),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(outcome) => match direct_local_inspection_observed_child(outcome) {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(observed) => observed,
                },
            };
            (observed.result.as_ref().clone(), observed.observations)
        }
    };
    let input = match input {
        Ok(input) => input,
        Err(error) => {
            return direct_local_inspection_complete(
                Err(DirectLocalModuleInspectionError::Input(error)),
                observations,
            );
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
                return direct_local_inspection_complete(
                    Err(DirectLocalModuleInspectionError::Inspection(
                        logical_path.dupe(),
                        Arc::from(error.to_string()),
                    )),
                    observations,
                );
            }
        },
    };
    direct_local_inspection_complete(
        Ok(DirectLocalModuleInspection(input, inspection)),
        observations,
    )
}

fn project_legacy_direct_local_inspection(
    outcome: DirectLocalModuleInspectionDriverOutcome,
) -> <DirectLocalModuleInspectionKey as Key>::Value {
    outcome.map(|observed| {
        observed
            .expect("legacy direct-local inspection cannot produce an observed outer error")
            .result
    })
}

#[async_trait]
impl Key for DirectLocalModuleInspectionKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalModuleInspection, DirectLocalModuleInspectionError>>,
    >;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_direct_local_inspection(
            drive_direct_local_module_inspection(ctx, self, HostRepositoryObservationMode::Legacy)
                .await,
        )
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for DirectLocalModuleInspectionObservationKey {
    type Value = DirectLocalModuleInspectionDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_module_inspection(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct DirectLocalIncludePackageHorizonObservationKey(DirectLocalIncludePackageHorizonKey);
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
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedDirectLocalIncludePackageHorizon {
    result: Arc<Result<DirectLocalIncludePackageHorizon, DirectLocalIncludePackageHorizonError>>,
    observations: PathObservationEpoch,
}

type DirectLocalIncludePackageHorizonDriverOutcome = SourcePreparationOutcome<
    Result<ObservedDirectLocalIncludePackageHorizon, ObservedPathFrontierError>,
>;
type DirectLocalIncludePackageLookupOutcome = Result<
    SourcePreparationOutcome<
        Result<
            (
                Arc<Result<ExternalRepositoryPackageLookup, ExternalRepositoryPackageLookupError>>,
                PathObservationEpoch,
            ),
            ObservedPathFrontierError,
        >,
    >,
    Arc<str>,
>;
impl DirectLocalIncludePackageHorizonKey {
    #[allow(dead_code)]
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

impl fmt::Display for DirectLocalIncludePackageHorizonObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
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
        project_legacy_direct_local_include_horizon(
            drive_direct_local_include_horizon_key(
                ctx,
                self,
                HostRepositoryObservationMode::Legacy,
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
}

#[async_trait]
impl Key for DirectLocalIncludePackageHorizonObservationKey {
    type Value = DirectLocalIncludePackageHorizonDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_include_horizon_key(
            ctx,
            &self.0,
            HostRepositoryObservationMode::Observed,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn direct_local_horizon_complete(
    result: Result<DirectLocalIncludePackageHorizon, DirectLocalIncludePackageHorizonError>,
    observations: PathObservationEpoch,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalIncludePackageHorizon {
        result: Arc::new(result),
        observations,
    }))
}

fn direct_local_horizon_inspection_error(
    error: Result<DirectLocalModuleInspectionError, Arc<str>>,
    observations: PathObservationEpoch,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
    direct_local_horizon_complete(
        Err(match error {
            Ok(error) => DirectLocalIncludePackageHorizonError::Inspection(error),
            Err(message) => DirectLocalIncludePackageHorizonError::InspectionCompute { message },
        }),
        observations,
    )
}

fn direct_local_horizon_observed_inspection(
    outcome: DirectLocalModuleInspectionDriverOutcome,
) -> ControlFlow<DirectLocalIncludePackageHorizonDriverOutcome, ObservedDirectLocalModuleInspection>
{
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(result) => result.map_or_else(
            |error| ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))),
            ControlFlow::Continue,
        ),
    }
}

async fn drive_direct_local_include_horizon_key(
    ctx: &mut DiceComputations<'_>,
    key: &DirectLocalIncludePackageHorizonKey,
    mode: HostRepositoryObservationMode,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
    let inspection_key = DirectLocalModuleInspectionKey::new(key.0.dupe(), key.1.clone())
        .expect("direct horizon key rejects root names");
    let (inspection, observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&inspection_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(inspection)) => {
                (inspection.as_ref().clone(), PathObservationEpoch::empty())
            }
            Err(error) => {
                return direct_local_horizon_inspection_error(
                    Err(Arc::from(error.to_string())),
                    PathObservationEpoch::empty(),
                );
            }
        },
        HostRepositoryObservationMode::Observed => {
            match ctx
                .compute(&DirectLocalModuleInspectionObservationKey(inspection_key))
                .await
            {
                Err(error) => {
                    return direct_local_horizon_inspection_error(
                        Err(Arc::from(error.to_string())),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(outcome) => match direct_local_horizon_observed_inspection(outcome) {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(observed) => {
                        (observed.result.as_ref().clone(), observed.observations)
                    }
                },
            }
        }
    };
    let inspection = match inspection {
        Ok(inspection) => inspection,
        Err(error) => {
            return direct_local_horizon_inspection_error(Ok(error), observations);
        }
    };
    let route = inspection.0.0.clone();
    let requests = inspection
        .1
        .as_ref()
        .map_or(&[][..], |inspection| inspection.includes.as_ref());
    drive_direct_local_include_package_horizon(ctx, route, requests, observations, mode).await
}

async fn preflight_direct_local_include_package_horizon(
    ctx: &mut DiceComputations<'_>,
    route: RootRepositoryRoute,
    requests: &[NonrootIncludeRequest],
) -> <DirectLocalIncludePackageHorizonKey as Key>::Value {
    project_legacy_direct_local_include_horizon(
        drive_direct_local_include_package_horizon(
            ctx,
            route,
            requests,
            PathObservationEpoch::empty(),
            HostRepositoryObservationMode::Legacy,
        )
        .await,
    )
}

async fn drive_direct_local_include_package_horizon(
    ctx: &mut DiceComputations<'_>,
    route: RootRepositoryRoute,
    requests: &[NonrootIncludeRequest],
    initial_observations: PathObservationEpoch,
    mode: HostRepositoryObservationMode,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
    let occurrences = match parse_direct_local_include_horizon(&route, requests) {
        Ok(occurrences) => occurrences,
        Err(error) => return direct_local_horizon_complete(Err(error), initial_observations),
    };
    let mut unique = SmallSet::with_capacity(occurrences.len());
    let mut packages = Vec::with_capacity(occurrences.len());
    for occurrence in &occurrences {
        if unique.insert(occurrence.package.clone()) {
            packages.push(occurrence.package.clone());
        }
    }
    let computed = ctx
        .compute_join(packages.clone(), |ctx, package| {
            let route = route.clone();
            Box::pin(async move {
                let result = match mode {
                    HostRepositoryObservationMode::Legacy => ctx
                        .compute(
                            &ExternalRepositoryPackageLookupKey::new(route, package.clone())
                                .expect("occurrence package uses the inspection route"),
                        )
                        .await
                        .map(|outcome| {
                            outcome.map(|result| Ok((result, PathObservationEpoch::empty())))
                        }),
                    HostRepositoryObservationMode::Observed => ctx
                        .compute(
                            &ExternalRepositoryPackageLookupObservationKey::new(
                                route,
                                package.clone(),
                            )
                            .expect("occurrence package uses the inspection route"),
                        )
                        .await
                        .map(|outcome| {
                            outcome.map(|result| {
                                result.map(|observed: ObservedExternalRepositoryPackageLookup| {
                                    (observed.result().dupe(), observed.observations().dupe())
                                })
                            })
                        }),
                }
                .map_err(|error| Arc::<str>::from(error.to_string()));
                (package, result)
            })
        })
        .await;
    let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
    finish_direct_local_include_package_horizon_observed(
        route,
        occurrences,
        &packages,
        outcomes,
        initial_observations,
    )
}

fn parse_direct_local_include_horizon(
    route: &RootRepositoryRoute,
    requests: &[NonrootIncludeRequest],
) -> Result<Vec<DirectLocalIncludePackageOccurrence>, DirectLocalIncludePackageHorizonError> {
    let mut occurrences = Vec::with_capacity(requests.len());
    for request in requests {
        let parsed = match parse_root_include(request) {
            Ok(parsed) => parsed,
            Err(message) => {
                return Err(DirectLocalIncludePackageHorizonError::BadLabel {
                    raw_label: request.path.clone(),
                    location: request.location.clone(),
                    message,
                });
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
    Ok(occurrences)
}

fn finish_direct_local_include_package_horizon_observed(
    route: RootRepositoryRoute,
    occurrences: Vec<DirectLocalIncludePackageOccurrence>,
    packages: &[PackageIdentifier],
    outcomes: SmallMap<PackageIdentifier, DirectLocalIncludePackageLookupOutcome>,
    mut observations: PathObservationEpoch,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
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

    let mut saw_need = false;
    let mut remaining_occurrences = occurrences.iter();
    for package in packages {
        let occurrence = remaining_occurrences
            .find(|occurrence| &occurrence.package == package)
            .expect("each unique package has a first occurrence");
        let outcome = outcomes
            .get(package)
            .expect("every unique package was computed");
        let value = match outcome {
            Err(message) => {
                if saw_need {
                    return SourcePreparationOutcome::Need(
                        all_need.expect("an earlier package contributed a Need"),
                    );
                }
                return direct_local_horizon_package_error(
                    occurrence,
                    DirectLocalIncludePackageFailure::LookupCompute {
                        message: message.dupe(),
                    },
                    observations,
                );
            }
            Ok(SourcePreparationOutcome::Need(_)) => {
                saw_need = true;
                continue;
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(error.dupe()));
            }
            Ok(SourcePreparationOutcome::Complete(Ok((value, incoming)))) => {
                observations = match merge_path_observations(&observations, incoming) {
                    Ok(observations) => observations,
                    Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                };
                value
            }
        };
        let failure = match value.as_ref() {
            Ok(ExternalRepositoryPackageLookup::Package(_)) => continue,
            Ok(ExternalRepositoryPackageLookup::InvalidPackageName { message }) => {
                DirectLocalIncludePackageFailure::InvalidPackageName {
                    message: message.dupe(),
                }
            }
            Ok(
                ExternalRepositoryPackageLookup::Deleted
                | ExternalRepositoryPackageLookup::IgnoredDirectory,
            ) => DirectLocalIncludePackageFailure::Deleted,
            Ok(ExternalRepositoryPackageLookup::NoBuildFile) => {
                DirectLocalIncludePackageFailure::NoBuildFile
            }
            Err(error) => DirectLocalIncludePackageFailure::Lookup(error.clone()),
        };
        if saw_need {
            return SourcePreparationOutcome::Need(
                all_need.expect("an earlier package contributed a Need"),
            );
        }
        return direct_local_horizon_package_error(occurrence, failure, observations);
    }

    if saw_need {
        SourcePreparationOutcome::Need(all_need.expect("a package contributed a Need"))
    } else {
        direct_local_horizon_complete(
            Ok(DirectLocalIncludePackageHorizon {
                route,
                occurrences: occurrences.into(),
            }),
            observations,
        )
    }
}

fn direct_local_horizon_package_error(
    occurrence: &DirectLocalIncludePackageOccurrence,
    failure: DirectLocalIncludePackageFailure,
    observations: PathObservationEpoch,
) -> DirectLocalIncludePackageHorizonDriverOutcome {
    direct_local_horizon_complete(
        Err(DirectLocalIncludePackageHorizonError::Package {
            raw_label: occurrence.raw_label.clone(),
            location: occurrence.location.clone(),
            package: occurrence.package.clone(),
            failure,
        }),
        observations,
    )
}

fn project_legacy_direct_local_include_horizon(
    outcome: DirectLocalIncludePackageHorizonDriverOutcome,
) -> <DirectLocalIncludePackageHorizonKey as Key>::Value {
    outcome.map(|observed| {
        observed
            .expect("legacy direct-local horizon cannot produce an observed outer error")
            .result
    })
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

impl fmt::Display for RepositorySourceFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
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
pub(crate) enum DirectLocalIncludeFragmentFailure {
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

#[derive(Debug, Clone)]
enum NonregistryPreparationOwner {
    Direct(RootRepositoryRoute),
    Host {
        workspace: NormalizedAbsolutePath,
        module: NonrootModuleKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct NonregistryIncludeOccurrence {
    package: PackagePath,
    target: TargetName,
    raw_label: CompactString,
    location: crate::LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct NonregistryPreparedFragment {
    occurrence: NonregistryIncludeOccurrence,
    logical_path: NormalizedAbsolutePath,
    bytes: Arc<[u8]>,
    inspection: NonrootModuleFileInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct NonregistryIncludeCycleCapability {
    package: PackagePath,
    target: TargetName,
    repeated_raw_label: CompactString,
    repeated_location: crate::LogicalSpan,
    ancestor_raw_label: CompactString,
    ancestor_location: crate::LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum NonregistryPreparedModule {
    Supported(Arc<[NonregistryPreparedFragment]>),
    UnsupportedCycle {
        fragments: Arc<[NonregistryPreparedFragment]>,
        capability: NonregistryIncludeCycleCapability,
    },
}

#[derive(Debug, Clone)]
struct NonregistryIncludeFrontierEntry {
    request: NonrootIncludeRequest,
    ancestry: Arc<[NonregistryIncludeAncestryEntry]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct NonregistryIncludeAncestryEntry {
    package: PackagePath,
    target: TargetName,
    raw_label: CompactString,
    location: crate::LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryModuleSourceIdentity {
    Local {
        repo_spec: RepoSpec,
    },
    Immutable {
        repo_spec: RepoSpec,
        source_identity: Arc<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostNonregistryModuleRoot {
    logical_path: NormalizedAbsolutePath,
    bytes: Arc<[u8]>,
    inspection: NonrootModuleFileInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostNonregistryPreparedClosure {
    module: NonrootModuleKey,
    source_identity: HostNonregistryModuleSourceIdentity,
    local_path_policy: HostRepositoryLocalPathPolicy,
    root: HostNonregistryModuleRoot,
    fragments: Arc<[NonregistryPreparedFragment]>,
}

impl HostNonregistryPreparedClosure {
    pub(crate) fn repo_spec(&self) -> &RepoSpec {
        match &self.source_identity {
            HostNonregistryModuleSourceIdentity::Local { repo_spec }
            | HostNonregistryModuleSourceIdentity::Immutable { repo_spec, .. } => repo_spec,
        }
    }

    pub(crate) fn local_path_policy(&self) -> HostRepositoryLocalPathPolicy {
        self.local_path_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryModuleClosure {
    Supported(HostNonregistryPreparedClosure),
    UnsupportedCycle {
        closure: HostNonregistryPreparedClosure,
        capability: NonregistryIncludeCycleCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryIncludePackageFailure {
    Compute(Arc<str>),
    Preflight(HostNonregistryPackagePreflightError),
    InvalidPackageName { message: Arc<str> },
    Ignored,
    NoBuildFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostNonregistryModuleClosureError {
    RootModuleFiles(CompactString),
    NonregistryOverrideRequired(CompactString),
    MaterializationCompute(Arc<str>),
    Materialization(RepositoryMaterializationError),
    RootSourceCompute(Arc<str>),
    RootSource(RepositorySourceFileError),
    RootAbsent,
    RootValidation {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    BadLabel {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        message: CompactString,
    },
    Package {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        package: PackagePath,
        failure: HostNonregistryIncludePackageFailure,
    },
    Fragment {
        raw_label: CompactString,
        location: crate::LogicalSpan,
        repo_relative_path: Arc<PathBuf>,
        failure: DirectLocalIncludeFragmentFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum NonregistryPreparationError {
    Direct(DirectLocalModulePreparationError),
    Host(HostNonregistryModuleClosureError),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct DirectLocalModulePreparationObservationKey(DirectLocalModulePreparationKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedDirectLocalModulePreparation {
    result: Arc<Result<DirectLocalModulePreparation, DirectLocalModulePreparationError>>,
    observations: PathObservationEpoch,
}

type DirectLocalModulePreparationDriverOutcome = SourcePreparationOutcome<
    Result<ObservedDirectLocalModulePreparation, ObservedPathFrontierError>,
>;

#[derive(Debug)]
struct ObservedNonregistryPreparation {
    result: Result<NonregistryPreparedModule, NonregistryPreparationError>,
    observations: PathObservationEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum NonregistryPreparationFrontierError {
    Path(ObservedPathFrontierError),
    Package(HostNonregistryPackagePreflightObservationError),
}

type NonregistryPreparationDriverOutcome = SourcePreparationOutcome<
    Result<ObservedNonregistryPreparation, NonregistryPreparationFrontierError>,
>;

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

impl fmt::Display for DirectLocalModulePreparationObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
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
        project_legacy_direct_local_preparation(
            drive_direct_local_module_preparation(ctx, self, HostRepositoryObservationMode::Legacy)
                .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for DirectLocalModulePreparationObservationKey {
    type Value = DirectLocalModulePreparationDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_module_preparation(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

async fn drive_direct_local_module_preparation(
    ctx: &mut DiceComputations<'_>,
    key: &DirectLocalModulePreparationKey,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModulePreparationDriverOutcome {
    let inspection_key = DirectLocalModuleInspectionKey::new(key.0.dupe(), key.1.clone())
        .expect("direct preparation key rejects root names");
    let (inspection, observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&inspection_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(inspection)) => {
                (inspection, PathObservationEpoch::empty())
            }
            Err(error) => {
                return direct_local_preparation_complete(
                    Err(DirectLocalModulePreparationError::InspectionCompute {
                        message: Arc::from(error.to_string()),
                    }),
                    PathObservationEpoch::empty(),
                );
            }
        },
        HostRepositoryObservationMode::Observed => {
            match ctx
                .compute(&DirectLocalModuleInspectionObservationKey(inspection_key))
                .await
            {
                Err(error) => {
                    return direct_local_preparation_complete(
                        Err(DirectLocalModulePreparationError::InspectionCompute {
                            message: Arc::from(error.to_string()),
                        }),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                    (observed.result, observed.observations)
                }
            }
        }
    };
    let root = match inspection.as_ref() {
        Ok(inspection) => inspection.clone(),
        Err(error) => {
            return direct_local_preparation_complete(
                Err(DirectLocalModulePreparationError::Inspection(error.clone())),
                observations,
            );
        }
    };
    prepare_direct_local_module(ctx, root, observations, mode).await
}

async fn prepare_direct_local_module(
    ctx: &mut DiceComputations<'_>,
    root: DirectLocalModuleInspection,
    observations: PathObservationEpoch,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModulePreparationDriverOutcome {
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
                return direct_local_preparation_complete(
                    Err(DirectLocalModulePreparationError::RootValidation {
                        logical_path: logical_path.dupe(),
                        message,
                    }),
                    observations,
                );
            }
        },
    };
    let prepared = match drive_nonregistry_module(
        ctx,
        NonregistryPreparationOwner::Direct(route.clone()),
        root_requests,
        observations,
        mode,
    )
    .await
    {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(NonregistryPreparationFrontierError::Path(
            error,
        ))) => return SourcePreparationOutcome::Complete(Err(error)),
        SourcePreparationOutcome::Complete(Err(NonregistryPreparationFrontierError::Package(
            _,
        ))) => unreachable!("direct preparation cannot produce a Host package frontier"),
        SourcePreparationOutcome::Complete(Ok(observed)) => match observed.result {
            Ok(value) => (value, observed.observations),
            Err(NonregistryPreparationError::Direct(error)) => {
                return direct_local_preparation_complete(Err(error), observed.observations);
            }
            Err(NonregistryPreparationError::Host(_)) => {
                unreachable!("direct-local preparation cannot produce a Host error");
            }
        },
    };
    let (prepared, observations) = prepared;
    let convert_fragment = |fragment: &NonregistryPreparedFragment| DirectLocalIncludeFragment {
        package: PackageIdentifier::new(
            route.canonical_repo().clone(),
            fragment.occurrence.package.clone(),
        ),
        target: fragment.occurrence.target.clone(),
        raw_label: fragment.occurrence.raw_label.clone(),
        location: fragment.occurrence.location.clone(),
        logical_path: fragment.logical_path.dupe(),
        bytes: fragment.bytes.dupe(),
        inspection: fragment.inspection.clone(),
    };
    let preparation = match prepared {
        NonregistryPreparedModule::Supported(fragments) => {
            DirectLocalModulePreparation::Supported(DirectLocalModuleClosure {
                root,
                fragments: fragments.iter().map(convert_fragment).collect(),
            })
        }
        NonregistryPreparedModule::UnsupportedCycle {
            fragments: _,
            capability,
        } => DirectLocalModulePreparation::Unsupported(DirectLocalIncludeCycleCapability {
            package: PackageIdentifier::new(route.canonical_repo().clone(), capability.package),
            target: capability.target,
            repeated_raw_label: capability.repeated_raw_label,
            repeated_location: capability.repeated_location,
            ancestor_raw_label: capability.ancestor_raw_label,
            ancestor_location: capability.ancestor_location,
        }),
    };
    direct_local_preparation_complete(Ok(preparation), observations)
}
type NonregistryIncludeHorizonValue = SourcePreparationOutcome<
    Result<Vec<NonregistryIncludeOccurrence>, NonregistryPreparationError>,
>;

#[derive(Debug)]
struct ObservedNonregistryIncludeHorizon {
    result: Result<Vec<NonregistryIncludeOccurrence>, NonregistryPreparationError>,
    observations: PathObservationEpoch,
}

type NonregistryIncludeHorizonDriverOutcome = SourcePreparationOutcome<
    Result<ObservedNonregistryIncludeHorizon, NonregistryPreparationFrontierError>,
>;

async fn drive_nonregistry_include_horizon(
    ctx: &mut DiceComputations<'_>,
    owner: &NonregistryPreparationOwner,
    requests: &[NonrootIncludeRequest],
    observations: PathObservationEpoch,
    mode: HostRepositoryObservationMode,
) -> NonregistryIncludeHorizonDriverOutcome {
    match owner {
        NonregistryPreparationOwner::Direct(route) => {
            match drive_direct_local_include_package_horizon(
                ctx,
                route.clone(),
                requests,
                observations,
                mode,
            )
            .await
            {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    SourcePreparationOutcome::Complete(Err(
                        NonregistryPreparationFrontierError::Path(error),
                    ))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    SourcePreparationOutcome::Complete(Ok(ObservedNonregistryIncludeHorizon {
                        result: observed.result.as_ref().as_ref().map_or_else(
                            |error| {
                                Err(NonregistryPreparationError::Direct(
                                    DirectLocalModulePreparationError::Package(error.clone()),
                                ))
                            },
                            |horizon| {
                                Ok(horizon
                                    .occurrences
                                    .iter()
                                    .map(|occurrence| NonregistryIncludeOccurrence {
                                        package: occurrence.package.package().clone(),
                                        target: occurrence.target.clone(),
                                        raw_label: occurrence.raw_label.clone(),
                                        location: occurrence.location.clone(),
                                    })
                                    .collect())
                            },
                        ),
                        observations: observed.observations,
                    }))
                }
            }
        }
        NonregistryPreparationOwner::Host { .. }
            if mode == HostRepositoryObservationMode::Observed =>
        {
            drive_observed_host_nonregistry_include_horizon(ctx, owner, requests, observations)
                .await
        }
        NonregistryPreparationOwner::Host { .. } => {
            preflight_nonregistry_include_horizon(ctx, owner, requests)
                .await
                .map(|result| {
                    Ok(ObservedNonregistryIncludeHorizon {
                        result,
                        observations,
                    })
                })
        }
    }
}

async fn preflight_nonregistry_include_horizon(
    ctx: &mut DiceComputations<'_>,
    owner: &NonregistryPreparationOwner,
    requests: &[NonrootIncludeRequest],
) -> NonregistryIncludeHorizonValue {
    if let NonregistryPreparationOwner::Direct(route) = owner {
        return match preflight_direct_local_include_package_horizon(ctx, route.clone(), requests)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Err(error) => {
                    SourcePreparationOutcome::Complete(Err(NonregistryPreparationError::Direct(
                        DirectLocalModulePreparationError::Package(error.clone()),
                    )))
                }
                Ok(value) => SourcePreparationOutcome::Complete(Ok(value
                    .occurrences
                    .iter()
                    .map(|occurrence| NonregistryIncludeOccurrence {
                        package: occurrence.package.package().clone(),
                        target: occurrence.target.clone(),
                        raw_label: occurrence.raw_label.clone(),
                        location: occurrence.location.clone(),
                    })
                    .collect())),
            },
        };
    }

    let NonregistryPreparationOwner::Host { workspace, module } = owner else {
        unreachable!()
    };
    let mut occurrences = Vec::with_capacity(requests.len());
    for request in requests {
        let (package, target) = match parse_nonroot_include(request) {
            Ok(parsed) => parsed,
            Err(message) => {
                return SourcePreparationOutcome::Complete(Err(NonregistryPreparationError::Host(
                    HostNonregistryModuleClosureError::BadLabel {
                        raw_label: request.path.clone(),
                        location: request.location.clone(),
                        message,
                    },
                )));
            }
        };
        occurrences.push(NonregistryIncludeOccurrence {
            package,
            target,
            raw_label: request.path.clone(),
            location: request.location.clone(),
        });
    }

    let mut unique = SmallSet::with_capacity(occurrences.len());
    unique.extend(
        occurrences
            .iter()
            .map(|occurrence| occurrence.package.clone()),
    );
    let outcomes = ctx
        .compute_join(unique, |ctx, package| {
            let workspace = workspace.dupe();
            let module = module.clone();
            Box::pin(async move {
                let value = ctx
                    .compute(&HostNonregistryPackagePreflightKey::new(
                        workspace,
                        module,
                        package.clone(),
                    ))
                    .await
                    .map_err(|error| Arc::<str>::from(error.to_string()));
                (package, value)
            })
        })
        .await
        .into_iter()
        .collect::<SmallMap<_, _>>();
    let all_need =
        outcomes
            .values()
            .fold(None, |current: Option<SourcePreparationNeeds>, outcome| {
                let Ok(SourcePreparationOutcome::Need(incoming)) = outcome else {
                    return current;
                };
                Some(match current {
                    Some(current) => current
                        .try_union(incoming)
                        .expect("one nonregistry package horizon's Needs cannot conflict"),
                    None => incoming.dupe(),
                })
            });

    for occurrence in &occurrences {
        let outcome = outcomes
            .get(&occurrence.package)
            .expect("every package was computed");
        let failure = match outcome {
            Err(message) => HostNonregistryIncludePackageFailure::Compute(message.dupe()),
            Ok(SourcePreparationOutcome::Need(_)) => {
                return SourcePreparationOutcome::Need(
                    all_need.expect("the current package contributed a Need"),
                );
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(HostNonregistryPackagePreflight::BuildDotBazel)
                | Ok(HostNonregistryPackagePreflight::Build) => continue,
                Ok(HostNonregistryPackagePreflight::Ignored) => {
                    HostNonregistryIncludePackageFailure::Ignored
                }
                Ok(HostNonregistryPackagePreflight::InvalidPackageName { message }) => {
                    HostNonregistryIncludePackageFailure::InvalidPackageName {
                        message: message.dupe(),
                    }
                }
                Ok(HostNonregistryPackagePreflight::NoBuildFile) => {
                    HostNonregistryIncludePackageFailure::NoBuildFile
                }
                Err(error) => HostNonregistryIncludePackageFailure::Preflight(error.clone()),
            },
        };
        return SourcePreparationOutcome::Complete(Err(NonregistryPreparationError::Host(
            HostNonregistryModuleClosureError::Package {
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
                package: occurrence.package.clone(),
                failure,
            },
        )));
    }
    SourcePreparationOutcome::Complete(Ok(occurrences))
}

fn observed_host_horizon_failure(
    occurrence: &NonregistryIncludeOccurrence,
    failure: HostNonregistryIncludePackageFailure,
    observations: PathObservationEpoch,
) -> NonregistryIncludeHorizonDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedNonregistryIncludeHorizon {
        result: Err(NonregistryPreparationError::Host(
            HostNonregistryModuleClosureError::Package {
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
                package: occurrence.package.clone(),
                failure,
            },
        )),
        observations,
    }))
}

async fn drive_observed_host_nonregistry_include_horizon(
    ctx: &mut DiceComputations<'_>,
    owner: &NonregistryPreparationOwner,
    requests: &[NonrootIncludeRequest],
    observations: PathObservationEpoch,
) -> NonregistryIncludeHorizonDriverOutcome {
    let NonregistryPreparationOwner::Host { workspace, module } = owner else {
        unreachable!("Host horizon requires a Host owner")
    };
    let mut occurrences = Vec::with_capacity(requests.len());
    for request in requests {
        let (package, target) = match parse_nonroot_include(request) {
            Ok(parsed) => parsed,
            Err(message) => {
                return SourcePreparationOutcome::Complete(Ok(ObservedNonregistryIncludeHorizon {
                    result: Err(NonregistryPreparationError::Host(
                        HostNonregistryModuleClosureError::BadLabel {
                            raw_label: request.path.clone(),
                            location: request.location.clone(),
                            message,
                        },
                    )),
                    observations,
                }));
            }
        };
        occurrences.push(NonregistryIncludeOccurrence {
            package,
            target,
            raw_label: request.path.clone(),
            location: request.location.clone(),
        });
    }

    let mut unique = SmallSet::with_capacity(occurrences.len());
    unique.extend(
        occurrences
            .iter()
            .map(|occurrence| occurrence.package.clone()),
    );
    let outcomes = ctx
        .compute_join(unique, |ctx, package| {
            let key = HostNonregistryPackagePreflightObservationKey(
                HostNonregistryPackagePreflightKey::new(
                    workspace.dupe(),
                    module.clone(),
                    package.clone(),
                ),
            );
            Box::pin(async move {
                let value = ctx
                    .compute(&key)
                    .await
                    .map_err(|error| Arc::<str>::from(error.to_string()));
                (package, value)
            })
        })
        .await
        .into_iter()
        .collect::<SmallMap<_, _>>();
    let all_need = outcomes
        .values()
        .fold(None::<SourcePreparationNeeds>, |current, outcome| {
            let Ok(SourcePreparationOutcome::Need(incoming)) = outcome else {
                return current;
            };
            Some(match current {
                Some(current) => current
                    .try_union(incoming)
                    .expect("one nonregistry package horizon's Needs cannot conflict"),
                None => incoming.dupe(),
            })
        });
    finish_observed_host_nonregistry_include_horizon(occurrences, &outcomes, all_need, observations)
}

type ObservedHostPreflightOutcome =
    Result<<HostNonregistryPackagePreflightObservationKey as Key>::Value, Arc<str>>;

fn finish_observed_host_nonregistry_include_horizon(
    occurrences: Vec<NonregistryIncludeOccurrence>,
    outcomes: &SmallMap<PackagePath, ObservedHostPreflightOutcome>,
    all_need: Option<SourcePreparationNeeds>,
    mut observations: PathObservationEpoch,
) -> NonregistryIncludeHorizonDriverOutcome {
    for occurrence in &occurrences {
        let outcome = outcomes
            .get(&occurrence.package)
            .expect("every package was computed");
        let result = match outcome {
            Err(message) => {
                return observed_host_horizon_failure(
                    occurrence,
                    HostNonregistryIncludePackageFailure::Compute(message.dupe()),
                    observations,
                );
            }
            Ok(SourcePreparationOutcome::Need(_)) => {
                return SourcePreparationOutcome::Need(
                    all_need.expect("the current package contributed a Need"),
                );
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    NonregistryPreparationFrontierError::Package(error.dupe()),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                observations = match merge_path_observations(&observations, observed.observations())
                {
                    Ok(observations) => observations,
                    Err(error) => {
                        return SourcePreparationOutcome::Complete(Err(
                            NonregistryPreparationFrontierError::Path(error),
                        ));
                    }
                };
                observed.result().as_ref()
            }
        };
        let failure = match result {
            Ok(HostNonregistryPackagePreflight::BuildDotBazel)
            | Ok(HostNonregistryPackagePreflight::Build) => continue,
            Ok(HostNonregistryPackagePreflight::Ignored) => {
                HostNonregistryIncludePackageFailure::Ignored
            }
            Ok(HostNonregistryPackagePreflight::InvalidPackageName { message }) => {
                HostNonregistryIncludePackageFailure::InvalidPackageName {
                    message: message.dupe(),
                }
            }
            Ok(HostNonregistryPackagePreflight::NoBuildFile) => {
                HostNonregistryIncludePackageFailure::NoBuildFile
            }
            Err(error) => HostNonregistryIncludePackageFailure::Preflight(error.clone()),
        };
        return observed_host_horizon_failure(occurrence, failure, observations);
    }
    SourcePreparationOutcome::Complete(Ok(ObservedNonregistryIncludeHorizon {
        result: Ok(occurrences),
        observations,
    }))
}

async fn drive_nonregistry_module(
    ctx: &mut DiceComputations<'_>,
    owner: NonregistryPreparationOwner,
    root_requests: Arc<[NonrootIncludeRequest]>,
    mut observations: PathObservationEpoch,
    mode: HostRepositoryObservationMode,
) -> NonregistryPreparationDriverOutcome {
    let mut frontier = root_requests
        .iter()
        .cloned()
        .map(|request| NonregistryIncludeFrontierEntry {
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
        let occurrences =
            match drive_nonregistry_include_horizon(ctx, &owner, &requests, observations, mode)
                .await
            {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    observations = observed.observations;
                    match observed.result {
                        Ok(value) => value,
                        Err(error) => {
                            return nonregistry_preparation_complete(Err(error), observations);
                        }
                    }
                }
            };
        let (paths, outcomes, all_need) =
            read_nonregistry_frontier_sources(ctx, &owner, &occurrences, mode).await;

        match finish_nonregistry_fragment_batch(
            &owner,
            &frontier,
            &occurrences,
            &paths,
            &outcomes,
            all_need,
            observations,
            &mut fragments,
            &mut pending_cycle,
        ) {
            ControlFlow::Break(outcome) => return outcome,
            ControlFlow::Continue((next_frontier, next_observations)) => {
                frontier = next_frontier;
                observations = next_observations;
            }
        }
    }

    let fragments: Arc<[NonregistryPreparedFragment]> = fragments.into();
    nonregistry_preparation_complete(
        Ok(match pending_cycle {
            Some(capability) => NonregistryPreparedModule::UnsupportedCycle {
                fragments,
                capability,
            },
            None => NonregistryPreparedModule::Supported(fragments),
        }),
        observations,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_nonregistry_fragment_batch(
    owner: &NonregistryPreparationOwner,
    frontier: &[NonregistryIncludeFrontierEntry],
    occurrences: &[NonregistryIncludeOccurrence],
    paths: &[PathBuf],
    outcomes: &SmallMap<PathBuf, NonregistryFragmentSourceOutcome>,
    all_need: Option<SourcePreparationNeeds>,
    mut observations: PathObservationEpoch,
    fragments: &mut Vec<NonregistryPreparedFragment>,
    pending_cycle: &mut Option<NonregistryIncludeCycleCapability>,
) -> ControlFlow<
    NonregistryPreparationDriverOutcome,
    (Vec<NonregistryIncludeFrontierEntry>, PathObservationEpoch),
> {
    let mut saw_need = false;
    let mut next_frontier = Vec::new();
    for ((entry, occurrence), repo_relative_path) in
        frontier.iter().zip(occurrences.iter()).zip(paths.iter())
    {
        let outcome = outcomes
            .get(repo_relative_path)
            .expect("every fragment path was computed");
        let source = match outcome {
            Err(message) => {
                if saw_need {
                    return ControlFlow::Break(SourcePreparationOutcome::Need(
                        all_need.expect("an earlier source contributed a Need"),
                    ));
                }
                return ControlFlow::Break(nonregistry_fragment_driver_error(
                    owner,
                    occurrence,
                    repo_relative_path,
                    DirectLocalIncludeFragmentFailure::SourceCompute {
                        message: message.dupe(),
                    },
                    observations,
                ));
            }
            Ok(SourcePreparationOutcome::Need(_)) => {
                saw_need = true;
                continue;
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
                    NonregistryPreparationFrontierError::Path(error.dupe()),
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok((result, incoming)))) => {
                observations = match merge_path_observations(&observations, incoming) {
                    Ok(observations) => observations,
                    Err(error) => {
                        return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
                            NonregistryPreparationFrontierError::Path(error),
                        )));
                    }
                };
                match result {
                    Err(error) => {
                        if saw_need {
                            return ControlFlow::Break(SourcePreparationOutcome::Need(
                                all_need.expect("an earlier source contributed a Need"),
                            ));
                        }
                        return ControlFlow::Break(nonregistry_fragment_driver_error(
                            owner,
                            occurrence,
                            repo_relative_path,
                            DirectLocalIncludeFragmentFailure::Source(error.clone()),
                            observations,
                        ));
                    }
                    Ok(None) => {
                        if saw_need {
                            return ControlFlow::Break(SourcePreparationOutcome::Need(
                                all_need.expect("an earlier source contributed a Need"),
                            ));
                        }
                        return ControlFlow::Break(nonregistry_fragment_driver_error(
                            owner,
                            occurrence,
                            repo_relative_path,
                            DirectLocalIncludeFragmentFailure::Absent,
                            observations,
                        ));
                    }
                    Ok(Some(source)) => source,
                }
            }
        };
        let logical_id = crate::LogicalModuleFileId::new(source.1.as_path().display().to_string());
        let inspection = match validate_root_module_source(logical_id, &source.0) {
            Ok(inspection) => inspection,
            Err(message) => {
                if saw_need {
                    return ControlFlow::Break(SourcePreparationOutcome::Need(
                        all_need.expect("an earlier source contributed a Need"),
                    ));
                }
                return ControlFlow::Break(nonregistry_fragment_driver_error(
                    owner,
                    occurrence,
                    repo_relative_path,
                    DirectLocalIncludeFragmentFailure::Validation {
                        logical_path: source.1.dupe(),
                        message,
                    },
                    observations,
                ));
            }
        };
        fragments.push(NonregistryPreparedFragment {
            occurrence: occurrence.clone(),
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
                *pending_cycle = Some(NonregistryIncludeCycleCapability {
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
            .chain([NonregistryIncludeAncestryEntry {
                package: occurrence.package.clone(),
                target: occurrence.target.clone(),
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
            }])
            .collect::<Arc<[_]>>();
        next_frontier.extend(inspection.includes.iter().cloned().map(|request| {
            NonregistryIncludeFrontierEntry {
                request,
                ancestry: ancestry.dupe(),
            }
        }));
    }
    if saw_need {
        ControlFlow::Break(SourcePreparationOutcome::Need(
            all_need.expect("a source contributed a Need"),
        ))
    } else {
        ControlFlow::Continue((next_frontier, observations))
    }
}

async fn read_nonregistry_frontier_sources(
    ctx: &mut DiceComputations<'_>,
    owner: &NonregistryPreparationOwner,
    occurrences: &[NonregistryIncludeOccurrence],
    mode: HostRepositoryObservationMode,
) -> (
    Vec<PathBuf>,
    SmallMap<PathBuf, NonregistryFragmentSourceOutcome>,
    Option<SourcePreparationNeeds>,
) {
    let paths = occurrences
        .iter()
        .map(nonregistry_fragment_relative_path)
        .collect::<Vec<_>>();
    let mut unique_paths = SmallSet::with_capacity(paths.len());
    unique_paths.extend(paths.iter().cloned());
    let outcomes = read_nonregistry_fragment_sources(ctx, owner, unique_paths, mode).await;
    let all_need = outcomes
        .values()
        .fold(None::<SourcePreparationNeeds>, |current, outcome| {
            let Ok(SourcePreparationOutcome::Need(incoming)) = outcome else {
                return current;
            };
            Some(match current {
                Some(current) => current
                    .try_union(incoming)
                    .expect("one nonregistry module's fragment Needs cannot conflict"),
                None => incoming.dupe(),
            })
        });
    (paths, outcomes, all_need)
}

fn nonregistry_preparation_complete(
    result: Result<NonregistryPreparedModule, NonregistryPreparationError>,
    observations: PathObservationEpoch,
) -> NonregistryPreparationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedNonregistryPreparation {
        result,
        observations,
    }))
}

type NonregistryFragmentSource =
    Result<Option<(Arc<[u8]>, NormalizedAbsolutePath)>, RepositorySourceFileError>;
type NonregistryFragmentSourceOutcome = Result<
    SourcePreparationOutcome<
        Result<(NonregistryFragmentSource, PathObservationEpoch), ObservedPathFrontierError>,
    >,
    Arc<str>,
>;

async fn read_nonregistry_fragment_sources(
    ctx: &mut DiceComputations<'_>,
    owner: &NonregistryPreparationOwner,
    paths: SmallSet<PathBuf>,
    mode: HostRepositoryObservationMode,
) -> SmallMap<PathBuf, NonregistryFragmentSourceOutcome> {
    match owner {
        NonregistryPreparationOwner::Direct(route) => ctx
            .compute_join(paths, |ctx, path| {
                let route = route.clone();
                Box::pin(async move {
                    let value = match mode {
                        HostRepositoryObservationMode::Legacy => ctx
                            .compute(&HostRepositorySourceFileKey::new(route, path.clone()))
                            .await
                            .map(|value| {
                                value.map(|result| {
                                    Ok((
                                        result
                                            .as_ref()
                                            .map(host_fragment_source)
                                            .map_err(Clone::clone),
                                        PathObservationEpoch::empty(),
                                    ))
                                })
                            }),
                        HostRepositoryObservationMode::Observed => ctx
                            .compute(&HostRepositorySourceFileObservationKey::new(
                                route,
                                path.clone(),
                            ))
                            .await
                            .map(|value| {
                                value.map(|result| {
                                    result.map(|observed| {
                                        (
                                            match observed.result().as_ref() {
                                                Ok(source) => Ok(host_fragment_source(source)),
                                                Err(error) => Err(error.clone()),
                                            },
                                            observed.observations().dupe(),
                                        )
                                    })
                                })
                            }),
                    }
                    .map_err(|error| Arc::<str>::from(error.to_string()));
                    (path, value)
                })
            })
            .await
            .into_iter()
            .collect(),
        NonregistryPreparationOwner::Host { workspace, module } => ctx
            .compute_join(paths, |ctx, path| {
                let workspace = workspace.dupe();
                let module = module.clone();
                Box::pin(async move {
                    let key = RepositorySourceFileKey {
                        workspace: workspace.as_path().to_path_buf(),
                        module_name: module.name.clone(),
                        repo_relative_path: path.clone(),
                    };
                    let project =
                        |result: Result<RepositorySourceFileValue, RepositorySourceFileError>| {
                            result.map(|source| match source {
                                RepositorySourceFileValue::Absent => None,
                                RepositorySourceFileValue::Present(bytes) => {
                                    Some((bytes, host_nonregistry_logical_path(&module, &path)))
                                }
                            })
                        };
                    let value = match mode {
                        HostRepositoryObservationMode::Legacy => {
                            ctx.compute(&key).await.map(|value| {
                                value.map(|result| {
                                    Ok((project(result), PathObservationEpoch::empty()))
                                })
                            })
                        }
                        HostRepositoryObservationMode::Observed => ctx
                            .compute(&RepositorySourceFileObservationKey(key))
                            .await
                            .map(|value| {
                                value.map(|result| {
                                    result.map(|observed| {
                                        (
                                            project(observed.result().as_ref().clone()),
                                            observed.observations().dupe(),
                                        )
                                    })
                                })
                            }),
                    }
                    .map_err(|error| Arc::<str>::from(error.to_string()));
                    (path, value)
                })
            })
            .await
            .into_iter()
            .collect(),
    }
}

fn host_fragment_source(
    source: &HostRepositorySourceFileValue,
) -> Option<(Arc<[u8]>, NormalizedAbsolutePath)> {
    match source {
        HostRepositorySourceFileValue::Absent => None,
        HostRepositorySourceFileValue::Present {
            bytes,
            logical_path,
        } => Some((bytes.dupe(), logical_path.dupe())),
    }
}

fn nonregistry_fragment_driver_error(
    owner: &NonregistryPreparationOwner,
    occurrence: &NonregistryIncludeOccurrence,
    repo_relative_path: &Path,
    failure: DirectLocalIncludeFragmentFailure,
    observations: PathObservationEpoch,
) -> NonregistryPreparationDriverOutcome {
    let error = match owner {
        NonregistryPreparationOwner::Direct(_) => {
            NonregistryPreparationError::Direct(DirectLocalModulePreparationError::Fragment {
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
                repo_relative_path: Arc::new(repo_relative_path.to_path_buf()),
                failure,
            })
        }
        NonregistryPreparationOwner::Host { .. } => {
            NonregistryPreparationError::Host(HostNonregistryModuleClosureError::Fragment {
                raw_label: occurrence.raw_label.clone(),
                location: occurrence.location.clone(),
                repo_relative_path: Arc::new(repo_relative_path.to_path_buf()),
                failure,
            })
        }
    };
    nonregistry_preparation_complete(Err(error), observations)
}

fn nonregistry_fragment_relative_path(occurrence: &NonregistryIncludeOccurrence) -> PathBuf {
    PathBuf::from(occurrence.package.as_str()).join(occurrence.target.as_str())
}

#[cfg(test)]
fn direct_local_preparation_error(
    error: DirectLocalModulePreparationError,
) -> <DirectLocalModulePreparationKey as Key>::Value {
    project_legacy_direct_local_preparation(direct_local_preparation_complete(
        Err(error),
        PathObservationEpoch::empty(),
    ))
}

fn direct_local_preparation_complete(
    result: Result<DirectLocalModulePreparation, DirectLocalModulePreparationError>,
    observations: PathObservationEpoch,
) -> DirectLocalModulePreparationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModulePreparation {
        result: Arc::new(result),
        observations,
    }))
}

fn project_legacy_direct_local_preparation(
    outcome: DirectLocalModulePreparationDriverOutcome,
) -> <DirectLocalModulePreparationKey as Key>::Value {
    outcome.map(|observed| {
        observed
            .expect("legacy direct-local preparation cannot produce an observed outer error")
            .result
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct DirectLocalModuleEvaluationKey(NormalizedAbsolutePath, ApparentRepoName);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)]
struct DirectLocalModuleEvaluationObservationKey(DirectLocalModuleEvaluationKey);

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

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedDirectLocalModuleEvaluation {
    result: Arc<Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>>,
    observations: PathObservationEpoch,
}

type DirectLocalModuleEvaluationDriverOutcome = SourcePreparationOutcome<
    Result<ObservedDirectLocalModuleEvaluation, ObservedPathFrontierError>,
>;

fn direct_local_evaluation_observed_child(
    outcome: DirectLocalModulePreparationDriverOutcome,
) -> ControlFlow<DirectLocalModuleEvaluationDriverOutcome, ObservedDirectLocalModulePreparation> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(result) => result.map_or_else(
            |error| ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error))),
            ControlFlow::Continue,
        ),
    }
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

impl fmt::Display for DirectLocalModuleEvaluationObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
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

async fn drive_direct_local_module_evaluation(
    ctx: &mut DiceComputations<'_>,
    key: &DirectLocalModuleEvaluationKey,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModuleEvaluationDriverOutcome {
    let capture_events = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok();
    let mut event_batch = None;
    let outcome = async {
        let preparation_key = DirectLocalModulePreparationKey::new(key.0.dupe(), key.1.clone())
            .expect("direct evaluation key rejects root names");
        let (preparation, observations) = match mode {
            HostRepositoryObservationMode::Legacy => match ctx.compute(&preparation_key).await {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(preparation)) => {
                    (preparation, PathObservationEpoch::empty())
                }
                Err(error) => {
                    return direct_local_evaluation_complete(
                        Err(DirectLocalModuleEvaluationError::PreparationCompute {
                            message: Arc::from(error.to_string()),
                        }),
                        PathObservationEpoch::empty(),
                    );
                }
            },
            HostRepositoryObservationMode::Observed => match ctx
                .compute(&DirectLocalModulePreparationObservationKey(preparation_key))
                .await
            {
                Ok(outcome) => match direct_local_evaluation_observed_child(outcome) {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(observed) => (observed.result, observed.observations),
                },
                Err(error) => {
                    return direct_local_evaluation_complete(
                        Err(DirectLocalModuleEvaluationError::PreparationCompute {
                            message: Arc::from(error.to_string()),
                        }),
                        PathObservationEpoch::empty(),
                    );
                }
            },
        };
        let preparation = match preparation.as_ref() {
            Ok(preparation) => preparation,
            Err(error) => {
                return direct_local_evaluation_complete(
                    Err(DirectLocalModuleEvaluationError::Preparation(error.clone())),
                    observations,
                );
            }
        };
        let closure = match preparation {
            DirectLocalModulePreparation::Unsupported(capability) => {
                return direct_local_evaluation_complete(
                    Ok(DirectLocalModuleEvaluation::Unsupported(capability.clone())),
                    observations,
                );
            }
            DirectLocalModulePreparation::Supported(closure) => closure,
        };
        let route = closure.root.0.0.clone();
        let root_bytes = match (&closure.root.0.1, &closure.root.1) {
            (HostRepositorySourceFileValue::Present { bytes, .. }, Some(_)) => bytes,
            (HostRepositorySourceFileValue::Absent, None) => {
                return direct_local_evaluation_complete(
                    Err(DirectLocalModuleEvaluationError::RootAbsent {
                        canonical_repo: route.canonical_repo().clone(),
                    }),
                    observations,
                );
            }
            _ => panic!("direct preparation preserves root source and inspection together"),
        };
        let root_logical_id =
            crate::LogicalModuleFileId::new(format!("{}//:MODULE.bazel", route.canonical_repo()));
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
        direct_local_evaluation_complete(
            evaluation
                .map(|module| {
                    DirectLocalModuleEvaluation::Supported(DirectLocalEvaluatedModule {
                        route,
                        module,
                    })
                })
                .map_err(DirectLocalModuleEvaluationError::Evaluation),
            observations,
        )
    }
    .await;
    if capture_events && direct_local_evaluation_publishes_batch(&outcome) {
        ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
            .expect("direct-local MODULE evaluation stores exactly one event batch");
    }
    outcome
}

fn direct_local_evaluation_publishes_batch(
    outcome: &DirectLocalModuleEvaluationDriverOutcome,
) -> bool {
    matches!(outcome, SourcePreparationOutcome::Complete(Ok(_observed)))
}

fn direct_local_evaluation_complete(
    result: Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>,
    observations: PathObservationEpoch,
) -> DirectLocalModuleEvaluationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleEvaluation {
        result: Arc::new(result),
        observations,
    }))
}

fn project_legacy_direct_local_evaluation(
    outcome: DirectLocalModuleEvaluationDriverOutcome,
) -> <DirectLocalModuleEvaluationKey as Key>::Value {
    outcome.map(|observed| {
        observed
            .expect("legacy direct-local evaluation cannot produce an observed outer error")
            .result
    })
}

#[async_trait]
impl Key for DirectLocalModuleEvaluationKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_direct_local_evaluation(
            drive_direct_local_module_evaluation(ctx, self, HostRepositoryObservationMode::Legacy)
                .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for DirectLocalModuleEvaluationObservationKey {
    type Value = DirectLocalModuleEvaluationDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_direct_local_module_evaluation(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
fn direct_local_evaluation_error(
    error: DirectLocalModuleEvaluationError,
) -> <DirectLocalModuleEvaluationKey as Key>::Value {
    project_legacy_direct_local_evaluation(direct_local_evaluation_complete(
        Err(error),
        PathObservationEpoch::empty(),
    ))
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedDirectLocalModuleSupport {
    result: Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>>,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedDirectLocalModuleSupport {
    pub(crate) fn result(
        &self,
    ) -> &Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

pub(crate) type DirectLocalModuleSupportDriverOutcome =
    SourcePreparationOutcome<Result<ObservedDirectLocalModuleSupport, ObservedPathFrontierError>>;

fn direct_local_module_support_result(
    route: &RootRepositoryRoute,
    value: &Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>,
) -> Result<DirectLocalModuleSupport, DirectLocalModuleSupportError> {
    match value {
        Err(error) => Err(DirectLocalModuleSupportError {
            inner: DirectLocalModuleSupportErrorInner::Evaluation(error.clone()),
        }),
        Ok(DirectLocalModuleEvaluation::Supported(_)) => Ok(DirectLocalModuleSupport::Supported),
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
    }
}

fn direct_local_module_support_complete(
    result: Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>,
    observations: PathObservationEpoch,
) -> DirectLocalModuleSupportDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModuleSupport {
        result: Arc::new(result),
        observations,
    }))
}

async fn drive_direct_local_module_support(
    ctx: &mut DiceComputations<'_>,
    route: &RootRepositoryRoute,
    mode: HostRepositoryObservationMode,
) -> DirectLocalModuleSupportDriverOutcome {
    let key = DirectLocalModuleEvaluationKey::new(
        route.workspace().dupe(),
        route.apparent_repo().clone(),
    )
    .expect("repository source routes are nonroot");
    let (value, observations) = match mode {
        HostRepositoryObservationMode::Legacy => match ctx.compute(&key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => {
                return direct_local_module_support_complete(
                    Err(DirectLocalModuleSupportError {
                        inner: DirectLocalModuleSupportErrorInner::Compute {
                            message: Arc::from(error.to_string()),
                        },
                    }),
                    PathObservationEpoch::empty(),
                );
            }
        },
        HostRepositoryObservationMode::Observed => match ctx
            .compute(&DirectLocalModuleEvaluationObservationKey(key))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result, observed.observations)
            }
            Err(error) => {
                return direct_local_module_support_complete(
                    Err(DirectLocalModuleSupportError {
                        inner: DirectLocalModuleSupportErrorInner::Compute {
                            message: Arc::from(error.to_string()),
                        },
                    }),
                    PathObservationEpoch::empty(),
                );
            }
        },
    };
    direct_local_module_support_complete(
        direct_local_module_support_result(route, value.as_ref()),
        observations,
    )
}

#[allow(dead_code)]
pub(crate) async fn direct_local_module_support_observed(
    ctx: &mut DiceComputations<'_>,
    route: &RootRepositoryRoute,
) -> DirectLocalModuleSupportDriverOutcome {
    drive_direct_local_module_support(ctx, route, HostRepositoryObservationMode::Observed).await
}

fn project_legacy_direct_local_module_support(
    outcome: DirectLocalModuleSupportDriverOutcome,
) -> SourcePreparationOutcome<Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>>>
{
    outcome.map(|observed| {
        observed
            .expect("legacy direct-local support cannot produce an observed outer error")
            .result
    })
}

pub(crate) async fn direct_local_module_support(
    ctx: &mut DiceComputations<'_>,
    route: &RootRepositoryRoute,
) -> SourcePreparationOutcome<Arc<Result<DirectLocalModuleSupport, DirectLocalModuleSupportError>>>
{
    project_legacy_direct_local_module_support(
        drive_direct_local_module_support(ctx, route, HostRepositoryObservationMode::Legacy).await,
    )
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RepositoryMaterializationRequestObservationKey(RepositoryMaterializationRequestKey);

impl RepositoryMaterializationRequestObservationKey {
    fn new(workspace: PathBuf, module_name: CompactString) -> Self {
        Self(RepositoryMaterializationRequestKey {
            workspace,
            module_name,
        })
    }
}

impl fmt::Display for RepositoryMaterializationRequestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-materialization-request:{}", self.module_name)
    }
}

impl fmt::Display for RepositoryMaterializationRequestObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type MaterializationRequestResult =
    Arc<Result<RepositoryMaterializationRequest, RepositoryMaterializationError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedRepositoryMaterializationRequest {
    result: MaterializationRequestResult,
    observations: PathObservationEpoch,
}

impl ObservedRepositoryMaterializationRequest {
    fn result(&self) -> &MaterializationRequestResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type MaterializationRequestDriverOutcome = SourcePreparationOutcome<
    Result<ObservedRepositoryMaterializationRequest, ObservedPathFrontierError>,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializationRequestMode {
    Legacy,
    Observed,
}

fn materialization_request_complete(
    result: Result<RepositoryMaterializationRequest, RepositoryMaterializationError>,
    observations: PathObservationEpoch,
) -> MaterializationRequestDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedRepositoryMaterializationRequest {
        result: Arc::new(result),
        observations,
    }))
}

fn materialization_effective_compute_error(
    error: impl fmt::Display,
) -> MaterializationRequestDriverOutcome {
    materialization_request_complete(
        Err(RepositoryMaterializationError::RootModuleFiles(
            error.to_string().into(),
        )),
        PathObservationEpoch::empty(),
    )
}
fn finish_observed_materialization_effective(
    outcome: SourcePreparationOutcome<
        Result<ObservedHostEffectiveModuleOverride, ObservedPathFrontierError>,
    >,
) -> ControlFlow<
    MaterializationRequestDriverOutcome,
    (HostEffectiveModuleOverride, PathObservationEpoch),
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            let observations = observed.observations().dupe();
            match observed.result().as_ref() {
                Ok(effective) => ControlFlow::Continue((effective.clone(), observations)),
                Err(error) => ControlFlow::Break(materialization_request_complete(
                    Err(RepositoryMaterializationError::RootModuleFiles(
                        error.to_string().into(),
                    )),
                    observations,
                )),
            }
        }
    }
}

fn project_materialization_request(
    workspace: NormalizedAbsolutePath,
    module_name: &CompactString,
    effective: &HostEffectiveModuleOverride,
) -> Result<RepositoryMaterializationRequest, RepositoryMaterializationError> {
    let repo_spec = match effective.override_() {
        Some(RootModuleOverride::NonRegistry(repo_spec)) => repo_spec.clone(),
        Some(_) => {
            return Err(RepositoryMaterializationError::UnsupportedOverride(
                format!("module {module_name} does not have a non-registry override").into(),
            ));
        }
        None => {
            return Err(RepositoryMaterializationError::MissingOverride(
                module_name.clone(),
            ));
        }
    };
    let canonical_repo = CanonicalRepoName::new(format!("{module_name}+")).map_err(|error| {
        RepositoryMaterializationError::InvalidCanonicalRepository(error.into())
    })?;
    let kind = request_kind(&workspace, &repo_spec, local_path_policy(effective))?;
    Ok(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace,
            canonical_repo,
        },
        repo_spec,
        kind,
    })
}

async fn drive_repository_materialization_request(
    ctx: &mut DiceComputations<'_>,
    key: &RepositoryMaterializationRequestKey,
    mode: MaterializationRequestMode,
) -> MaterializationRequestDriverOutcome {
    let workspace = match NormalizedAbsolutePath::new(key.workspace.clone()) {
        Ok(workspace) => workspace,
        Err(error) => {
            return materialization_request_complete(
                Err(RepositoryMaterializationError::InvalidWorkspace(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let (effective, observations) = match mode {
        MaterializationRequestMode::Legacy => {
            let effective = match ctx
                .compute(&HostEffectiveModuleOverrideKey::new(
                    workspace.dupe(),
                    key.module_name.clone(),
                ))
                .await
            {
                Ok(effective) => effective,
                Err(error) => return materialization_effective_compute_error(error),
            };
            match effective.as_ref() {
                Ok(effective) => (effective.clone(), PathObservationEpoch::empty()),
                Err(error) => {
                    return materialization_request_complete(
                        Err(RepositoryMaterializationError::RootModuleFiles(
                            error.to_string().into(),
                        )),
                        PathObservationEpoch::empty(),
                    );
                }
            }
        }
        MaterializationRequestMode::Observed => {
            let effective = match ctx
                .compute(&HostEffectiveModuleOverrideObservationKey::new(
                    workspace.dupe(),
                    key.module_name.clone(),
                ))
                .await
            {
                Ok(effective) => effective,
                Err(error) => return materialization_effective_compute_error(error),
            };
            match finish_observed_materialization_effective(effective) {
                ControlFlow::Continue(effective) => effective,
                ControlFlow::Break(outcome) => return outcome,
            }
        }
    };
    materialization_request_complete(
        project_materialization_request(workspace, &key.module_name, &effective),
        observations,
    )
}

fn project_legacy_materialization_request(
    outcome: MaterializationRequestDriverOutcome,
) -> MaterializationRequestResult {
    match outcome {
        SourcePreparationOutcome::Complete(Ok(observed)) => observed.result,
        _ => panic!("legacy materialization request invariant failed"),
    }
}

#[async_trait]
impl Key for RepositoryMaterializationRequestKey {
    type Value = MaterializationRequestResult;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_materialization_request(
            drive_repository_materialization_request(ctx, self, MaterializationRequestMode::Legacy)
                .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RepositoryMaterializationRequestObservationKey {
    type Value = MaterializationRequestDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_repository_materialization_request(ctx, &self.0, MaterializationRequestMode::Observed)
            .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RepositoryMaterializationObservationKey(RepositoryMaterializationKey);

impl RepositoryMaterializationObservationKey {
    fn new(workspace: PathBuf, module_name: CompactString) -> Self {
        Self(RepositoryMaterializationKey {
            workspace,
            module_name,
        })
    }
}

impl fmt::Display for RepositoryMaterializationObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type MaterializationSemanticResult =
    Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedRepositoryMaterialization {
    result: MaterializationSemanticResult,
    observations: PathObservationEpoch,
}

impl ObservedRepositoryMaterialization {
    fn result(&self) -> &MaterializationSemanticResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type RepositoryMaterializationDriverOutcome =
    SourcePreparationOutcome<Result<ObservedRepositoryMaterialization, ObservedPathFrontierError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepositoryMaterializationMode {
    Legacy,
    Observed,
}
fn request_kind(
    workspace: &NormalizedAbsolutePath,
    repo_spec: &RepoSpec,
    local_path_policy: HostRepositoryLocalPathPolicy,
) -> Result<RepositoryMaterializationKind, RepositoryMaterializationError> {
    let local_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
        .expect("pinned local repository label is canonical");
    if repo_spec.rule_id.bzl_file == local_bzl && repo_spec.rule_id.rule_name == "local_repository"
    {
        if local_path_policy == HostRepositoryLocalPathPolicy::LocalUnsupported {
            return Err(RepositoryMaterializationError::Spec(
                "local_repository is unsupported for this repository source".into(),
            ));
        }
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
        let source = Path::new(path.as_str());
        if source.as_os_str().is_empty()
            || source.components().any(|component| {
                !matches!(
                    component,
                    Component::Prefix(_) | Component::RootDir | Component::Normal(_)
                )
            })
            || source.is_absolute()
                != (local_path_policy == HostRepositoryLocalPathPolicy::CommandAbsolute)
        {
            return Err(RepositoryMaterializationError::Spec(
                if local_path_policy == HostRepositoryLocalPathPolicy::CommandAbsolute {
                    "command local_repository path must be normalized and absolute"
                } else {
                    "local_repository path must be normalized and workspace-relative"
                }
                .into(),
            ));
        }
        let root = NormalizedAbsolutePath::new(
            if local_path_policy == HostRepositoryLocalPathPolicy::CommandAbsolute {
                source.to_owned()
            } else {
                workspace.as_path().join(source)
            },
        )
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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositoryMaterializationDisposition {
    Builtin(BuiltinBazelToolsRouteIdentity),
    Request(Arc<RepositoryMaterializationRequest>),
}

#[doc(hidden)]
pub fn host_repository_materialization_request(
    capability: &HostRepositorySourceCapability,
) -> Result<HostRepositoryMaterializationDisposition, RepositoryMaterializationError> {
    match capability.source() {
        HostRepositorySourceCapabilitySource::Builtin(identity) => Ok(
            HostRepositoryMaterializationDisposition::Builtin(identity.clone()),
        ),
        HostRepositorySourceCapabilitySource::RepoSpec {
            repo_spec,
            local_path_policy,
        } => Ok(HostRepositoryMaterializationDisposition::Request(Arc::new(
            RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: capability.workspace().dupe(),
                    canonical_repo: capability.canonical_repo().clone(),
                },
                repo_spec: repo_spec.as_ref().clone(),
                kind: match capability.generated_file_effect_plan() {
                    Some(plan) => RepositoryMaterializationKind::GeneratedFileEffects(plan.clone()),
                    None => request_kind(capability.workspace(), repo_spec, *local_path_policy)?,
                },
            },
        ))),
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceInput {
    capability: HostRepositorySourceCapability,
    disposition: HostRepositoryMaterializationDisposition,
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositorySourceInputError {
    Projection(RepositoryMaterializationError),
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum HostRepositorySourceInputDispositionView<'a> {
    Builtin(&'a BuiltinBazelToolsRouteIdentity),
    Request(&'a Arc<RepositoryMaterializationRequest>),
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostRepositorySourceInputView<'a> {
    capability: &'a HostRepositorySourceCapability,
    disposition: HostRepositorySourceInputDispositionView<'a>,
}
impl HostRepositorySourceInput {
    #[doc(hidden)]
    pub fn view(&self) -> HostRepositorySourceInputView<'_> {
        let disposition = match &self.disposition {
            HostRepositoryMaterializationDisposition::Builtin(identity) => {
                HostRepositorySourceInputDispositionView::Builtin(identity)
            }
            HostRepositoryMaterializationDisposition::Request(request) => {
                HostRepositorySourceInputDispositionView::Request(request)
            }
        };
        HostRepositorySourceInputView {
            capability: &self.capability,
            disposition,
        }
    }
}
impl<'a> HostRepositorySourceInputView<'a> {
    pub fn capability(self) -> &'a HostRepositorySourceCapability {
        self.capability
    }
    pub fn disposition(self) -> HostRepositorySourceInputDispositionView<'a> {
        self.disposition
    }
}
#[doc(hidden)]
pub fn host_repository_source_input(
    capability: HostRepositorySourceCapability,
) -> Result<HostRepositorySourceInput, HostRepositorySourceInputError> {
    let disposition = host_repository_materialization_request(&capability)
        .map_err(HostRepositorySourceInputError::Projection)?;
    Ok(HostRepositorySourceInput {
        capability,
        disposition,
    })
}

fn root_repository_materialization_request(
    route: &RootRepositoryRoute,
) -> Result<Arc<RepositoryMaterializationRequest>, RepositoryMaterializationError> {
    match host_repository_materialization_request(&route.source_capability())? {
        HostRepositoryMaterializationDisposition::Builtin(_) => {
            Err(RepositoryMaterializationError::Spec(
                "built-in bazel_tools source requires its immutable source owner".into(),
            ))
        }
        HostRepositoryMaterializationDisposition::Request(request) => Ok(request),
    }
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

fn repository_materialization_complete(
    result: MaterializationSemanticResult,
    observations: PathObservationEpoch,
) -> RepositoryMaterializationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedRepositoryMaterialization {
        result,
        observations,
    }))
}

fn repository_materialization_request_compute_error(
    error: impl fmt::Display,
) -> RepositoryMaterializationDriverOutcome {
    repository_materialization_complete(
        Arc::new(Err(RepositoryMaterializationError::RootModuleFiles(
            error.to_string().into(),
        ))),
        PathObservationEpoch::empty(),
    )
}

fn repository_materialization_result_compute_error(
    error: impl fmt::Display,
    observations: PathObservationEpoch,
) -> RepositoryMaterializationDriverOutcome {
    repository_materialization_complete(
        Arc::new(Err(RepositoryMaterializationError::ResultCompute(
            error.to_string().into(),
        ))),
        observations,
    )
}

fn finish_observed_materialization_request(
    outcome: MaterializationRequestDriverOutcome,
) -> ControlFlow<
    RepositoryMaterializationDriverOutcome,
    (RepositoryMaterializationRequest, PathObservationEpoch),
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            let observations = observed.observations().dupe();
            match observed.result().as_ref() {
                Ok(request) => ControlFlow::Continue((request.clone(), observations)),
                Err(error) => ControlFlow::Break(repository_materialization_complete(
                    Arc::new(Err(error.clone())),
                    observations,
                )),
            }
        }
    }
}

async fn drive_repository_materialization(
    ctx: &mut DiceComputations<'_>,
    key: &RepositoryMaterializationKey,
    mode: RepositoryMaterializationMode,
) -> RepositoryMaterializationDriverOutcome {
    let (request, observations) = match mode {
        RepositoryMaterializationMode::Legacy => {
            let request = match ctx
                .compute(&RepositoryMaterializationRequestKey {
                    workspace: key.workspace.clone(),
                    module_name: key.module_name.clone(),
                })
                .await
            {
                Ok(request) => request,
                Err(error) => return repository_materialization_request_compute_error(error),
            };
            match request.as_ref() {
                Ok(request) => (request.clone(), PathObservationEpoch::empty()),
                Err(error) => {
                    return repository_materialization_complete(
                        Arc::new(Err(error.clone())),
                        PathObservationEpoch::empty(),
                    );
                }
            }
        }
        RepositoryMaterializationMode::Observed => {
            let request = match ctx
                .compute(&RepositoryMaterializationRequestObservationKey::new(
                    key.workspace.clone(),
                    key.module_name.clone(),
                ))
                .await
            {
                Ok(request) => request,
                Err(error) => return repository_materialization_request_compute_error(error),
            };
            match finish_observed_materialization_request(request) {
                ControlFlow::Continue(request) => request,
                ControlFlow::Break(outcome) => return outcome,
            }
        }
    };
    match ctx
        .compute(&RepositoryMaterializationResultKey {
            request: Arc::new(request),
        })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(result)) => {
            repository_materialization_complete(result, observations)
        }
        Err(error) => repository_materialization_result_compute_error(error, observations),
    }
}

fn project_legacy_repository_materialization(
    outcome: RepositoryMaterializationDriverOutcome,
) -> <RepositoryMaterializationKey as Key>::Value {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            SourcePreparationOutcome::Complete(observed.result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy materialization cannot produce an observed outer error")
        }
    }
}

#[async_trait]
impl Key for RepositoryMaterializationKey {
    type Value = SourcePreparationOutcome<MaterializationSemanticResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_repository_materialization(
            drive_repository_materialization(ctx, self, RepositoryMaterializationMode::Legacy)
                .await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for RepositoryMaterializationObservationKey {
    type Value = RepositoryMaterializationDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_repository_materialization(ctx, &self.0, RepositoryMaterializationMode::Observed)
            .await
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostRepositoryObservationMode {
    Legacy,
    Observed,
}

type HostRepositoryPathProjection = (
    Result<HostRepositoryPathValue, RepositorySourceFileError>,
    PathObservationEpoch,
);
type HostRepositoryPathDriverOutcome =
    SourcePreparationOutcome<Result<HostRepositoryPathProjection, ObservedPathFrontierError>>;
type HostRepositorySourceProjection = (
    Result<HostRepositorySourceFileValue, RepositorySourceFileError>,
    PathObservationEpoch,
);
type HostRepositorySourceDriverOutcome =
    SourcePreparationOutcome<Result<HostRepositorySourceProjection, ObservedPathFrontierError>>;

fn host_repository_complete<T>(
    result: Result<T, RepositorySourceFileError>,
    observations: PathObservationEpoch,
) -> SourcePreparationOutcome<
    Result<(Result<T, RepositorySourceFileError>, PathObservationEpoch), ObservedPathFrontierError>,
> {
    SourcePreparationOutcome::Complete(Ok((result, observations)))
}

fn project_host_repository_path(
    result: Result<ResolvedPath, PathResolutionError>,
    repo_relative_path: Arc<PathBuf>,
) -> Result<HostRepositoryPathValue, RepositorySourceFileError> {
    result
        .map(HostRepositoryPathValue)
        .map_err(|error| project_resolution_error(repo_relative_path, error))
}

async fn drive_host_repository_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositoryPathKey,
    mode: HostRepositoryObservationMode,
) -> HostRepositoryPathDriverOutcome {
    let relative = match checked_relative_path(&key.repo_relative_path) {
        Ok(relative) => relative,
        Err(_) => {
            return host_repository_complete(
                Err(RepositorySourceFileError::InvalidRepoRelativePath {
                    requested_path: Arc::new(key.repo_relative_path.clone()),
                }),
                PathObservationEpoch::empty(),
            );
        }
    };
    let repo_relative_path = Arc::new(relative.to_owned());
    let request = match root_repository_materialization_request(&key.route) {
        Ok(request) => request,
        Err(error) => {
            return host_repository_complete(
                Err(RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error),
                }),
                PathObservationEpoch::empty(),
            );
        }
    };
    let materialization = match ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
        Err(error) => {
            return host_repository_complete(
                Err(RepositorySourceFileError::MaterializationCompute {
                    repo_relative_path,
                    message: Arc::from(error.to_string()),
                }),
                PathObservationEpoch::empty(),
            );
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return host_repository_complete(
                Err(RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error.clone()),
                }),
                PathObservationEpoch::empty(),
            );
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
            return host_repository_complete(
                Err(RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path }),
                PathObservationEpoch::empty(),
            );
        }
    };
    match mode {
        HostRepositoryObservationMode::Legacy => {
            match ctx
                .compute(&ResolvedPathKey::new(namespace, requested_path))
                .await
            {
                Ok(PathOutcome::Need(need)) => SourcePreparationOutcome::path_need(need),
                Ok(PathOutcome::Complete(result)) => host_repository_complete(
                    project_host_repository_path(result, repo_relative_path),
                    PathObservationEpoch::empty(),
                ),
                Err(error) => host_repository_complete(
                    Err(RepositorySourceFileError::ResolutionCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    }),
                    PathObservationEpoch::empty(),
                ),
            }
        }
        HostRepositoryObservationMode::Observed => {
            match ctx
                .compute(&ResolvedPathObservationKey::new(namespace, requested_path))
                .await
            {
                Ok(PathOutcome::Need(need)) => SourcePreparationOutcome::path_need(need),
                Ok(PathOutcome::Complete(Err(error))) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                Ok(PathOutcome::Complete(Ok(observed))) => host_repository_complete(
                    project_host_repository_path(observed.result().clone(), repo_relative_path),
                    observed.observations().dupe(),
                ),
                Err(error) => host_repository_complete(
                    Err(RepositorySourceFileError::ResolutionCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    }),
                    PathObservationEpoch::empty(),
                ),
            }
        }
    }
}

type HostRepositoryDirectoryListingProjection = (
    Result<HostRepositoryDirectoryListing, HostRepositoryDirectoryListingError>,
    PathObservationEpoch,
);
type HostRepositoryDirectoryListingDriverOutcome = SourcePreparationOutcome<
    Result<HostRepositoryDirectoryListingProjection, ObservedPathFrontierError>,
>;

fn host_repository_directory_listing_complete(
    result: Result<HostRepositoryDirectoryListing, HostRepositoryDirectoryListingError>,
    observations: PathObservationEpoch,
) -> HostRepositoryDirectoryListingDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((result, observations)))
}

fn project_directory_listing_error(
    directory: Arc<PackagePath>,
    error: PathDirectoryListingError,
) -> HostRepositoryDirectoryListingError {
    match error {
        PathDirectoryListingError::Observation { operation, .. } => {
            HostRepositoryDirectoryListingError::new(
                directory,
                HostRepositoryDirectoryListingErrorKind::Observation { operation },
            )
        }
        PathDirectoryListingError::InconsistentState {
            operation,
            before,
            after,
            ..
        } => HostRepositoryDirectoryListingError::new(
            directory,
            HostRepositoryDirectoryListingErrorKind::InconsistentState {
                operation,
                before,
                after,
            },
        ),
        PathDirectoryListingError::WrongKind {
            expected, actual, ..
        } => HostRepositoryDirectoryListingError::new(
            directory,
            HostRepositoryDirectoryListingErrorKind::WrongKind { expected, actual },
        ),
        PathDirectoryListingError::Cycle { .. } => HostRepositoryDirectoryListingError::new(
            directory,
            HostRepositoryDirectoryListingErrorKind::Cycle,
        ),
        PathDirectoryListingError::InfiniteExpansion { .. } => {
            HostRepositoryDirectoryListingError::new(
                directory,
                HostRepositoryDirectoryListingErrorKind::InfiniteExpansion,
            )
        }
    }
}

fn finish_observed_host_repository_directory_listing(
    child: PathOutcome<Result<ObservedPathDirectoryListing, ObservedPathFrontierError>>,
    directory: Arc<PackagePath>,
) -> HostRepositoryDirectoryListingDriverOutcome {
    match child {
        PathOutcome::Need(need) => SourcePreparationOutcome::path_need(need),
        PathOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(error)),
        PathOutcome::Complete(Ok(observed)) => host_repository_directory_listing_complete(
            observed
                .result()
                .clone()
                .map_err(|error| project_directory_listing_error(directory, error)),
            observed.observations().dupe(),
        ),
    }
}

async fn drive_host_repository_directory_listing(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositoryDirectoryListingKey,
    mode: HostRepositoryObservationMode,
) -> HostRepositoryDirectoryListingDriverOutcome {
    let disposition = match host_repository_materialization_request(&key.route.source_capability())
    {
        Ok(disposition) => disposition,
        Err(_) => {
            return host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    Arc::new(key.directory.clone()),
                    HostRepositoryDirectoryListingErrorKind::Materialization,
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    drive_repository_directory_listing_from_disposition(ctx, disposition, &key.directory, mode)
        .await
}

async fn drive_repository_directory_listing_from_disposition(
    ctx: &mut DiceComputations<'_>,
    disposition: HostRepositoryMaterializationDisposition,
    requested_directory: &PackagePath,
    mode: HostRepositoryObservationMode,
) -> HostRepositoryDirectoryListingDriverOutcome {
    let directory = Arc::new(requested_directory.clone());
    let HostRepositoryMaterializationDisposition::Request(request) = disposition else {
        let HostRepositoryMaterializationDisposition::Builtin(identity) = disposition else {
            unreachable!()
        };
        return match ctx
            .compute(&BuiltinBazelToolsDirectoryListingKey::new(
                identity.snapshot(),
                requested_directory.clone(),
            ))
            .await
        {
            Ok(value) => host_repository_directory_listing_complete(
                value.as_ref().clone().map_err(|_| {
                    HostRepositoryDirectoryListingError::new(
                        directory.dupe(),
                        HostRepositoryDirectoryListingErrorKind::Builtin,
                    )
                }),
                PathObservationEpoch::empty(),
            ),
            Err(_) => host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::BuiltinCompute,
                )),
                PathObservationEpoch::empty(),
            ),
        };
    };
    let materialization = match ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
        Err(_) => {
            return host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::MaterializationCompute,
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(materialization) => materialization,
        Err(_) => {
            return host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::Materialization,
                )),
                PathObservationEpoch::empty(),
            );
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
    let requested_path = match NormalizedAbsolutePath::new(root.join(requested_directory.as_str()))
    {
        Ok(path) => path,
        Err(_) => {
            return host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::InvalidMaterializedPath,
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    match mode {
        HostRepositoryObservationMode::Legacy => match ctx
            .compute(&PathDirectoryListingKey::new(namespace, requested_path))
            .await
        {
            Ok(PathOutcome::Need(need)) => SourcePreparationOutcome::path_need(need),
            Ok(PathOutcome::Complete(result)) => host_repository_directory_listing_complete(
                result.map_err(|error| project_directory_listing_error(directory.dupe(), error)),
                PathObservationEpoch::empty(),
            ),
            Err(_) => host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::ListingCompute,
                )),
                PathObservationEpoch::empty(),
            ),
        },
        HostRepositoryObservationMode::Observed => match ctx
            .compute(&PathDirectoryListingObservationKey::new(
                namespace,
                requested_path,
            ))
            .await
        {
            Ok(child) => finish_observed_host_repository_directory_listing(child, directory.dupe()),
            Err(_) => host_repository_directory_listing_complete(
                Err(HostRepositoryDirectoryListingError::new(
                    directory.dupe(),
                    HostRepositoryDirectoryListingErrorKind::ListingCompute,
                )),
                PathObservationEpoch::empty(),
            ),
        },
    }
}

fn append_host_repository_source_observation(
    path: &PathObservationEpoch,
    demand: PathObservationDemand,
    result: Arc<PathObservationResult>,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        path.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(std::iter::once((demand, result))),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn host_repository_source_file_compute_error(
    repo_relative_path: Arc<PathBuf>,
    message: Arc<str>,
    observations: &PathObservationEpoch,
) -> HostRepositorySourceDriverOutcome {
    host_repository_complete(
        Err(RepositorySourceFileError::FileCompute {
            repo_relative_path,
            message,
        }),
        observations.dupe(),
    )
}

async fn drive_host_repository_source_from_resolved(
    ctx: &mut DiceComputations<'_>,
    resolved: ResolvedPath,
    repo_relative_path: Arc<PathBuf>,
    mode: HostRepositoryObservationMode,
    mut observations: PathObservationEpoch,
) -> HostRepositorySourceDriverOutcome {
    let lstat = match resolved.state() {
        ResolvedPathState::Missing => {
            return host_repository_complete(
                Ok(HostRepositorySourceFileValue::Absent),
                observations,
            );
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
            return host_repository_complete(
                Err(RepositorySourceFileError::WrongKind {
                    repo_relative_path,
                    actual: lstat.kind(),
                }),
                observations,
            );
        }
    };
    let demand = PathObservationDemand::new(
        resolved.namespace(),
        resolved.real_path().dupe(),
        PathObservationOperation::FileBytes,
    );
    let observed = match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
        Ok(PathOutcome::Need(need)) => return SourcePreparationOutcome::path_need(need),
        Ok(PathOutcome::Complete(result)) => result,
        Err(error) => {
            return host_repository_source_file_compute_error(
                repo_relative_path,
                Arc::from(error.to_string()),
                &observations,
            );
        }
    };
    if matches!(mode, HostRepositoryObservationMode::Observed) {
        observations =
            match append_host_repository_source_observation(&observations, demand, observed.dupe())
            {
                Ok(observations) => observations,
                Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
            };
    }
    let result = match observed.as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
            Ok(HostRepositorySourceFileValue::Present {
                bytes: bytes.dupe(),
                logical_path: resolved.requested_path().dupe(),
            })
        }
        PathObservationResult::FileBytes(PathOperationResult::Missing) => {
            Err(RepositorySourceFileError::InconsistentState {
                repo_relative_path,
                operation: PathObservationOperation::FileBytes,
                before: Some(lstat),
                after: None,
            })
        }
        PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
            Err(RepositorySourceFileError::Observation {
                repo_relative_path,
                operation: PathObservationOperation::FileBytes,
                error: *error,
            })
        }
        PathObservationResult::Lstat(_)
        | PathObservationResult::ReadLink(_)
        | PathObservationResult::DirectoryEntries(_)
        | PathObservationResult::WindowsLongPath(_)
        | PathObservationResult::WindowsOptionPathLongName(_) => {
            unreachable!("FileBytes demand must return a FileBytes observation")
        }
    };
    host_repository_complete(result, observations)
}

async fn drive_host_repository_source(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositorySourceFileKey,
    mode: HostRepositoryObservationMode,
) -> HostRepositorySourceDriverOutcome {
    let repo_relative_path = Arc::new(key.repo_relative_path.clone());
    let (path, observations) = match mode {
        HostRepositoryObservationMode::Legacy => {
            match ctx
                .compute(&HostRepositoryPathKey::new(
                    key.route.clone(),
                    key.repo_relative_path.clone(),
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Ok(path))) => {
                    (path, PathObservationEpoch::empty())
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return host_repository_complete(Err(error), PathObservationEpoch::empty());
                }
                Err(error) => {
                    return host_repository_complete(
                        Err(RepositorySourceFileError::ResolutionCompute {
                            repo_relative_path,
                            message: Arc::from(error.to_string()),
                        }),
                        PathObservationEpoch::empty(),
                    );
                }
            }
        }
        HostRepositoryObservationMode::Observed => {
            match ctx
                .compute(&HostRepositoryPathObservationKey(
                    HostRepositoryPathKey::new(key.route.clone(), key.repo_relative_path.clone()),
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                    let observations = observed.observations.dupe();
                    match observed.result.as_ref() {
                        Ok(path) => (path.dupe(), observations),
                        Err(error) => {
                            return host_repository_complete(Err(error.dupe()), observations);
                        }
                    }
                }
                Err(error) => {
                    return host_repository_complete(
                        Err(RepositorySourceFileError::ResolutionCompute {
                            repo_relative_path,
                            message: Arc::from(error.to_string()),
                        }),
                        PathObservationEpoch::empty(),
                    );
                }
            }
        }
    };
    let request = root_repository_materialization_request(&key.route)
        .expect("a successful Host repository path has a valid materialization request");
    let materialization = ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
        .expect("the successful Host repository path computed materialization");
    debug_assert!(materialization.is_complete());
    drive_host_repository_source_from_resolved(
        ctx,
        path.resolved().dupe(),
        repo_relative_path,
        mode,
        observations,
    )
    .await
}

type RepositorySourceFileDriverOutcome =
    SourcePreparationOutcome<Result<ObservedRepositorySourceFileValue, ObservedPathFrontierError>>;

fn repository_source_file_complete(
    result: Result<RepositorySourceFileValue, RepositorySourceFileError>,
    observations: PathObservationEpoch,
) -> RepositorySourceFileDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedRepositorySourceFileValue {
        result: Arc::new(result),
        observations,
    }))
}

fn project_legacy_repository_source_file(
    outcome: RepositorySourceFileDriverOutcome,
) -> <RepositorySourceFileKey as Key>::Value {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            SourcePreparationOutcome::Complete(observed.result.as_ref().clone())
        }
        SourcePreparationOutcome::Complete(Err(_)) => unreachable!(),
    }
}

fn repository_source_materialization_compute_error(
    repo_relative_path: Arc<PathBuf>,
    message: Arc<str>,
) -> RepositorySourceFileDriverOutcome {
    repository_source_file_complete(
        Err(RepositorySourceFileError::MaterializationCompute {
            repo_relative_path,
            message,
        }),
        PathObservationEpoch::empty(),
    )
}

fn repository_source_resolution_compute_error(
    repo_relative_path: Arc<PathBuf>,
    message: Arc<str>,
    observations: PathObservationEpoch,
) -> RepositorySourceFileDriverOutcome {
    repository_source_file_complete(
        Err(RepositorySourceFileError::ResolutionCompute {
            repo_relative_path,
            message,
        }),
        observations,
    )
}

fn finish_observed_repository_source_materialization(
    outcome: RepositoryMaterializationDriverOutcome,
    repo_relative_path: Arc<PathBuf>,
) -> ControlFlow<
    RepositorySourceFileDriverOutcome,
    (MaterializationSemanticResult, PathObservationEpoch),
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            let observations = observed.observations().dupe();
            match observed.result().as_ref() {
                Ok(_) => ControlFlow::Continue((observed.result().dupe(), observations)),
                Err(error) => ControlFlow::Break(repository_source_file_complete(
                    Err(RepositorySourceFileError::Materialization {
                        repo_relative_path,
                        error: Arc::new(error.clone()),
                    }),
                    observations,
                )),
            }
        }
    }
}

fn finish_observed_repository_source_resolution(
    outcome: PathOutcome<Result<ObservedResolvedPath, ObservedPathFrontierError>>,
    observations: &PathObservationEpoch,
    repo_relative_path: Arc<PathBuf>,
) -> ControlFlow<RepositorySourceFileDriverOutcome, (ResolvedPath, PathObservationEpoch)> {
    match outcome {
        PathOutcome::Need(need) => ControlFlow::Break(SourcePreparationOutcome::path_need(need)),
        PathOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        PathOutcome::Complete(Ok(observed)) => {
            let merged = match merge_path_observations(observations, observed.observations()) {
                Ok(merged) => merged,
                Err(error) => {
                    return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)));
                }
            };
            match observed.result() {
                Ok(resolved) => ControlFlow::Continue((resolved.clone(), merged)),
                Err(error) => ControlFlow::Break(repository_source_file_complete(
                    Err(project_resolution_error(repo_relative_path, error.clone())),
                    merged,
                )),
            }
        }
    }
}

async fn drive_repository_source_file(
    ctx: &mut DiceComputations<'_>,
    key: &RepositorySourceFileKey,
    mode: HostRepositoryObservationMode,
) -> RepositorySourceFileDriverOutcome {
    let relative = match checked_relative_path(&key.repo_relative_path) {
        Ok(relative) => relative,
        Err(_) => {
            return repository_source_file_complete(
                Err(RepositorySourceFileError::InvalidRepoRelativePath {
                    requested_path: Arc::new(key.repo_relative_path.clone()),
                }),
                PathObservationEpoch::empty(),
            );
        }
    };
    let repo_relative_path = Arc::new(relative.to_owned());
    let (materialization, observations) = match mode {
        HostRepositoryObservationMode::Legacy => {
            match ctx
                .compute(&RepositoryMaterializationKey {
                    workspace: key.workspace.clone(),
                    module_name: key.module_name.clone(),
                })
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(result)) => {
                    (result, PathObservationEpoch::empty())
                }
                Err(error) => {
                    return repository_source_materialization_compute_error(
                        repo_relative_path,
                        Arc::from(error.to_string()),
                    );
                }
            }
        }
        HostRepositoryObservationMode::Observed => {
            match ctx
                .compute(&RepositoryMaterializationObservationKey::new(
                    key.workspace.clone(),
                    key.module_name.clone(),
                ))
                .await
            {
                Ok(outcome) => match finish_observed_repository_source_materialization(
                    outcome,
                    repo_relative_path.dupe(),
                ) {
                    ControlFlow::Continue(materialization) => materialization,
                    ControlFlow::Break(outcome) => return outcome,
                },
                Err(error) => {
                    return repository_source_materialization_compute_error(
                        repo_relative_path,
                        Arc::from(error.to_string()),
                    );
                }
            }
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(materialization) => materialization,
        Err(error) => {
            return repository_source_file_complete(
                Err(RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error.clone()),
                }),
                observations,
            );
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
    let Ok(requested_path) = NormalizedAbsolutePath::new(root.join(relative)) else {
        return repository_source_file_complete(
            Err(RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path }),
            observations,
        );
    };
    let (resolved, observations) = match mode {
        HostRepositoryObservationMode::Legacy => {
            match ctx
                .compute(&ResolvedPathKey::new(namespace, requested_path))
                .await
            {
                Ok(PathOutcome::Need(need)) => return SourcePreparationOutcome::path_need(need),
                Ok(PathOutcome::Complete(result)) => (result, observations),
                Err(error) => {
                    return repository_source_resolution_compute_error(
                        repo_relative_path,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            }
        }
        HostRepositoryObservationMode::Observed => {
            let outcome = match ctx
                .compute(&ResolvedPathObservationKey::new(namespace, requested_path))
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    return repository_source_resolution_compute_error(
                        repo_relative_path,
                        Arc::from(error.to_string()),
                        observations,
                    );
                }
            };
            match finish_observed_repository_source_resolution(
                outcome,
                &observations,
                repo_relative_path.dupe(),
            ) {
                ControlFlow::Continue(resolved) => (Ok(resolved.0), resolved.1),
                ControlFlow::Break(outcome) => return outcome,
            }
        }
    };
    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) => {
            return repository_source_file_complete(
                Err(project_resolution_error(repo_relative_path, error)),
                observations,
            );
        }
    };
    match drive_host_repository_source_from_resolved(
        ctx,
        resolved,
        repo_relative_path,
        mode,
        observations,
    )
    .await
    {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(error))
        }
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            repository_source_file_complete(
                result.map(|value| match value {
                    HostRepositorySourceFileValue::Present { bytes, .. } => {
                        RepositorySourceFileValue::Present(bytes)
                    }
                    HostRepositorySourceFileValue::Absent => RepositorySourceFileValue::Absent,
                }),
                observations,
            )
        }
    }
}

#[async_trait]
impl Key for HostRepositoryDirectoryListingKey {
    type Value = SourcePreparationResult<
        HostRepositoryDirectoryListing,
        HostRepositoryDirectoryListingError,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_host_repository_directory_listing(
            ctx,
            self,
            HostRepositoryObservationMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => unreachable!(
                "legacy Host repository directory listing cannot produce an observed outer error"
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
        demand.provide_value_with(|| RepositorySourceScope {
            workspace: self.route.workspace().dupe(),
            module_name: CompactString::new(self.route.module_name()),
        });
    }
}

#[async_trait]
impl Key for HostRepositoryDirectoryListingObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRepositoryDirectoryListing, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_repository_directory_listing(
            ctx,
            &self.0,
            HostRepositoryObservationMode::Observed,
        )
        .await
        .map(|outcome| {
            outcome.map(
                |(result, observations)| ObservedHostRepositoryDirectoryListing {
                    result: Arc::new(result),
                    observations,
                },
            )
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        self.0.provide(demand);
    }
}

#[async_trait]
impl Key for HostRepositoryPathKey {
    type Value = SourcePreparationResult<HostRepositoryPathValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_host_repository_path(ctx, self, HostRepositoryObservationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy Host repository path cannot produce an observed outer error")
            }
        }
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
impl Key for HostRepositoryPathObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedHostRepositoryPath, ObservedPathFrontierError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_repository_path(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedHostRepositoryPath {
                    result: Arc::new(result),
                    observations,
                })
            })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        self.0.provide(demand);
    }
}

#[async_trait]
impl Key for HostRepositorySourceFileKey {
    type Value = SourcePreparationResult<HostRepositorySourceFileValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_host_repository_source(ctx, self, HostRepositoryObservationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy Host repository source cannot produce an observed outer error")
            }
        }
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
impl Key for HostRepositorySourceFileObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRepositorySourceFile, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_repository_source(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedHostRepositorySourceFile {
                    result: Arc::new(result),
                    observations,
                })
            })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        self.0.provide(demand);
    }
}

#[async_trait]
impl Key for RepositorySourceFileKey {
    type Value = SourcePreparationResult<RepositorySourceFileValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_repository_source_file(
            drive_repository_source_file(ctx, self, HostRepositoryObservationMode::Legacy).await,
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
impl Key for RepositorySourceFileObservationKey {
    type Value = RepositorySourceFileDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_repository_source_file(ctx, &self.0, HostRepositoryObservationMode::Observed).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        self.0.provide(demand);
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryPackagePreflightObservationKey(
    pub(crate) HostNonregistryPackagePreflightKey,
);

impl fmt::Display for HostNonregistryPackagePreflightObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type HostNonregistryPackagePreflightResult =
    Arc<Result<HostNonregistryPackagePreflight, HostNonregistryPackagePreflightError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostNonregistryPackagePreflight {
    result: HostNonregistryPackagePreflightResult,
    observations: PathObservationEpoch,
}

impl ObservedHostNonregistryPackagePreflight {
    pub(crate) fn result(&self) -> &HostNonregistryPackagePreflightResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostNonregistryPackagePreflightObservationError {
    EffectiveFrontier(ObservedPathFrontierError),
    EffectiveCompute(Arc<str>),
    PolicyCompute(Arc<str>),
    IgnoreFrontier(ObservedPathFrontierError),
    IgnoreCompute(Arc<str>),
    MarkerFrontier {
        marker: HostBuildFileName,
        error: ObservedPathFrontierError,
    },
    MarkerCompute {
        marker: HostBuildFileName,
        message: Arc<str>,
    },
}

impl HostNonregistryPackagePreflightObservationError {
    fn effective_compute(message: Arc<str>) -> Self {
        Self::EffectiveCompute(message)
    }
    fn policy_compute(message: Arc<str>) -> Self {
        Self::PolicyCompute(message)
    }
    fn ignore_compute(message: Arc<str>) -> Self {
        Self::IgnoreCompute(message)
    }
    fn marker_compute(marker: HostBuildFileName, message: Arc<str>) -> Self {
        Self::MarkerCompute { marker, message }
    }
}
#[derive(Clone, Copy)]
enum HostNonregistryPackagePreflightMode {
    Legacy,
    Observed,
}

type HostNonregistryPackagePreflightDriverOutcome = SourcePreparationOutcome<
    Result<
        (HostNonregistryPackagePreflightResult, PathObservationEpoch),
        HostNonregistryPackagePreflightObservationError,
    >,
>;

type PreflightComputed<T> =
    ControlFlow<HostNonregistryPackagePreflightDriverOutcome, (T, PathObservationEpoch)>;
#[track_caller]
fn preflight_dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("nonregistry package-preflight DICE invariant: {error:?}"))
}

type HostNonregistryPackagePreflightValue =
    SourcePreparationOutcome<HostNonregistryPackagePreflightResult>;
type HostEffectiveModuleOverrideResult =
    Arc<Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>>;
type HostNonregistryRepositoryIgnoreResult =
    Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>;

fn preflight_complete(
    value: Result<HostNonregistryPackagePreflight, HostNonregistryPackagePreflightError>,
    observations: PathObservationEpoch,
) -> HostNonregistryPackagePreflightDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}
fn preflight_outer(
    error: HostNonregistryPackagePreflightObservationError,
) -> HostNonregistryPackagePreflightDriverOutcome {
    SourcePreparationOutcome::Complete(Err(error))
}

fn finish_preflight_child<T, R>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
    complete: impl FnOnce(T) -> R,
    frontier: impl FnOnce(ObservedPathFrontierError) -> HostNonregistryPackagePreflightObservationError,
) -> ControlFlow<HostNonregistryPackagePreflightDriverOutcome, R> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(preflight_outer(frontier(error)))
        }
        SourcePreparationOutcome::Complete(Ok(value)) => ControlFlow::Continue(complete(value)),
    }
}

fn finish_legacy_preflight_child<T>(outcome: SourcePreparationOutcome<T>) -> PreflightComputed<T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(value) => {
            ControlFlow::Continue((value, PathObservationEpoch::empty()))
        }
    }
}

async fn compute_preflight_effective(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryPackagePreflightKey,
    mode: HostNonregistryPackagePreflightMode,
) -> PreflightComputed<HostEffectiveModuleOverrideResult> {
    match mode {
        HostNonregistryPackagePreflightMode::Legacy => ControlFlow::Continue((
            preflight_dice_invariant(
                ctx.compute(&HostEffectiveModuleOverrideKey::new(
                    key.workspace.dupe(),
                    key.module.name.clone(),
                ))
                .await,
            ),
            PathObservationEpoch::empty(),
        )),
        HostNonregistryPackagePreflightMode::Observed => match ctx
            .compute(&HostEffectiveModuleOverrideObservationKey::new(
                key.workspace.dupe(),
                key.module.name.clone(),
            ))
            .await
        {
            Err(error) => ControlFlow::Break(preflight_outer(
                HostNonregistryPackagePreflightObservationError::effective_compute(
                    error.to_string().into(),
                ),
            )),
            Ok(outcome) => finish_preflight_child(
                outcome,
                |value| (value.result().dupe(), value.observations().dupe()),
                HostNonregistryPackagePreflightObservationError::EffectiveFrontier,
            ),
        },
    }
}

async fn compute_preflight_ignore(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryPackagePreflightKey,
    mode: HostNonregistryPackagePreflightMode,
) -> PreflightComputed<HostNonregistryRepositoryIgnoreResult> {
    let ignore_key =
        HostNonregistryRepositoryIgnoreKey::new(key.workspace.dupe(), key.module.clone());
    match mode {
        HostNonregistryPackagePreflightMode::Legacy => {
            finish_legacy_preflight_child(preflight_dice_invariant(ctx.compute(&ignore_key).await))
        }
        HostNonregistryPackagePreflightMode::Observed => match ctx
            .compute(&HostNonregistryRepositoryIgnoreObservationKey(ignore_key))
            .await
        {
            Err(error) => ControlFlow::Break(preflight_outer(
                HostNonregistryPackagePreflightObservationError::ignore_compute(
                    error.to_string().into(),
                ),
            )),
            Ok(outcome) => finish_preflight_child(
                outcome,
                |value| (value.result().dupe(), value.observations().dupe()),
                HostNonregistryPackagePreflightObservationError::IgnoreFrontier,
            ),
        },
    }
}

async fn compute_preflight_marker(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryPackagePreflightKey,
    marker: HostBuildFileName,
    path: PathBuf,
    mode: HostNonregistryPackagePreflightMode,
) -> PreflightComputed<Result<RepositorySourceFileValue, RepositorySourceFileError>> {
    let source_key = RepositorySourceFileKey {
        workspace: key.workspace.as_path().to_owned(),
        module_name: key.module.name.clone(),
        repo_relative_path: path,
    };
    match mode {
        HostNonregistryPackagePreflightMode::Legacy => {
            finish_legacy_preflight_child(preflight_dice_invariant(ctx.compute(&source_key).await))
        }
        HostNonregistryPackagePreflightMode::Observed => match ctx
            .compute(&RepositorySourceFileObservationKey(source_key))
            .await
        {
            Err(error) => ControlFlow::Break(preflight_outer(
                HostNonregistryPackagePreflightObservationError::marker_compute(
                    marker,
                    error.to_string().into(),
                ),
            )),
            Ok(outcome) => finish_preflight_child(
                outcome,
                |value| (value.result().as_ref().clone(), value.observations().dupe()),
                |error| HostNonregistryPackagePreflightObservationError::MarkerFrontier {
                    marker,
                    error,
                },
            ),
        },
    }
}

async fn drive_host_nonregistry_package_preflight(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryPackagePreflightKey,
    mode: HostNonregistryPackagePreflightMode,
) -> HostNonregistryPackagePreflightDriverOutcome {
    let (effective, mut observations) = match compute_preflight_effective(ctx, key, mode).await {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return outcome,
    };
    let effective = match effective.as_ref() {
        Ok(effective) => effective,
        Err(error) => {
            return preflight_complete(
                Err(HostNonregistryPackagePreflightError::RootModuleFiles(
                    error.to_string().into(),
                )),
                observations,
            );
        }
    };
    if !matches!(
        effective.override_(),
        Some(RootModuleOverride::NonRegistry(_))
    ) {
        return preflight_complete(
            Err(
                HostNonregistryPackagePreflightError::NonregistryOverrideRequired(
                    key.module.name.clone(),
                ),
            ),
            observations,
        );
    }
    if let Some(message) = invalid_package_name(&key.package) {
        return preflight_complete(
            Ok(HostNonregistryPackagePreflight::InvalidPackageName { message }),
            observations,
        );
    }
    let deleted = match ctx
        .compute(&CanonicalDeletedPackagesProjectionKey::new(
            key.workspace.dupe(),
        ))
        .await
    {
        Ok(value) => value,
        Err(error) => match mode {
            HostNonregistryPackagePreflightMode::Legacy => {
                preflight_dice_invariant::<(), _>(Err(error));
                unreachable!()
            }
            HostNonregistryPackagePreflightMode::Observed => {
                return preflight_outer(
                    HostNonregistryPackagePreflightObservationError::policy_compute(Arc::from(
                        error.to_string(),
                    )),
                );
            }
        },
    };
    let deleted = match deleted {
        Ok(value) => value,
        Err(error) => {
            return preflight_complete(
                Err(HostNonregistryPackagePreflightError::PolicyInput(error)),
                observations,
            );
        }
    };
    if !deleted.is_empty() {
        return preflight_complete(
            Err(HostNonregistryPackagePreflightError::UnsupportedDeletedPackages),
            observations,
        );
    }
    let (ignore, ignore_observations) = match compute_preflight_ignore(ctx, key, mode).await {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return outcome,
    };
    observations = match merge_path_observations(&observations, &ignore_observations) {
        Ok(observations) => observations,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                HostNonregistryPackagePreflightObservationError::IgnoreFrontier(error),
            ));
        }
    };
    let ignore = match ignore.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return preflight_complete(
                Err(HostNonregistryPackagePreflightError::RepositoryIgnore(
                    error.clone(),
                )),
                observations,
            );
        }
    };
    if ignore.matching_entry(&key.package).is_some() {
        return preflight_complete(Ok(HostNonregistryPackagePreflight::Ignored), observations);
    }
    for marker in [HostBuildFileName::BuildDotBazel, HostBuildFileName::Build] {
        let name = match marker {
            HostBuildFileName::BuildDotBazel => "BUILD.bazel",
            HostBuildFileName::Build => "BUILD",
        };
        let path = if key.package.as_str().is_empty() {
            PathBuf::from(name)
        } else {
            PathBuf::from(key.package.as_str()).join(name)
        };
        let (source, source_observations) =
            match compute_preflight_marker(ctx, key, marker, path, mode).await {
                ControlFlow::Continue(value) => value,
                ControlFlow::Break(outcome) => return outcome,
            };
        observations = match merge_path_observations(&observations, &source_observations) {
            Ok(observations) => observations,
            Err(error) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostNonregistryPackagePreflightObservationError::MarkerFrontier {
                        marker,
                        error,
                    },
                ));
            }
        };
        match source {
            Ok(RepositorySourceFileValue::Present(_)) => {
                let value = match marker {
                    HostBuildFileName::BuildDotBazel => {
                        HostNonregistryPackagePreflight::BuildDotBazel
                    }
                    HostBuildFileName::Build => HostNonregistryPackagePreflight::Build,
                };
                return preflight_complete(Ok(value), observations);
            }
            Ok(RepositorySourceFileValue::Absent)
            | Err(RepositorySourceFileError::WrongKind {
                actual: PathNodeKind::Directory,
                ..
            }) => {}
            Err(error) => {
                return preflight_complete(
                    Err(HostNonregistryPackagePreflightError::RepositorySource { marker, error }),
                    observations,
                );
            }
        }
    }
    preflight_complete(
        Ok(HostNonregistryPackagePreflight::NoBuildFile),
        observations,
    )
}

fn project_preflight_legacy(
    outcome: HostNonregistryPackagePreflightDriverOutcome,
) -> HostNonregistryPackagePreflightValue {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy preflight cannot produce an observed outer error")
        }
    }
}

#[async_trait]
impl Key for HostNonregistryPackagePreflightKey {
    type Value = HostNonregistryPackagePreflightValue;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_preflight_legacy(
            drive_host_nonregistry_package_preflight(
                ctx,
                self,
                HostNonregistryPackagePreflightMode::Legacy,
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
}

#[async_trait]
impl Key for HostNonregistryPackagePreflightObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostNonregistryPackagePreflight,
            HostNonregistryPackagePreflightObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_nonregistry_package_preflight(
            ctx,
            &self.0,
            HostNonregistryPackagePreflightMode::Observed,
        )
        .await
        .map(|result| {
            result.map(
                |(result, observations)| ObservedHostNonregistryPackagePreflight {
                    result,
                    observations,
                },
            )
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn observed_module_source_complete(
    result: Result<ModuleSourcePreparation, ModuleSourcePreparationError>,
    observations: PathObservationEpoch,
) -> ModuleSourcePreparationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn module_source_error(
    error: ModuleSourcePreparationError,
    observations: PathObservationEpoch,
) -> ModuleSourcePreparationDriverOutcome {
    observed_module_source_complete(Err(error), observations)
}

fn observed_module_source_outer(
    error: ObservedPathFrontierError,
) -> ModuleSourcePreparationDriverOutcome {
    SourcePreparationOutcome::Complete(Err(error))
}

fn merge_module_source_observations(
    prefix: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    merge_path_observations(prefix, incoming)
}

fn finish_module_source_observed_child<T>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
    prefix: PathObservationEpoch,
    observations: impl FnOnce(&T) -> &PathObservationEpoch,
) -> Result<(T, PathObservationEpoch), ModuleSourcePreparationDriverOutcome> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => Err(observed_module_source_outer(error)),
        SourcePreparationOutcome::Complete(Ok(value)) => {
            let merged = merge_module_source_observations(&prefix, observations(&value))
                .map_err(observed_module_source_outer)?;
            Ok((value, merged))
        }
    }
}

fn finish_module_source_observed_path<T>(
    outcome: PathOutcome<Result<T, ObservedPathFrontierError>>,
    prefix: PathObservationEpoch,
    observations: impl FnOnce(&T) -> &PathObservationEpoch,
) -> Result<(T, PathObservationEpoch), ModuleSourcePreparationDriverOutcome> {
    match outcome {
        PathOutcome::Need(need) => Err(SourcePreparationOutcome::path_need(need)),
        PathOutcome::Complete(Err(error)) => Err(observed_module_source_outer(error)),
        PathOutcome::Complete(Ok(value)) => {
            let merged = merge_module_source_observations(&prefix, observations(&value))
                .map_err(observed_module_source_outer)?;
            Ok((value, merged))
        }
    }
}

fn finish_module_source_patch_file(
    demand: PathObservationDemand,
    result: Arc<PathObservationResult>,
    before: Option<PathLstat>,
    observations: PathObservationEpoch,
    observed: bool,
) -> Result<(Arc<[u8]>, PathObservationEpoch), ModuleSourcePreparationDriverOutcome> {
    let observations = if observed {
        append_host_repository_source_observation(&observations, demand.dupe(), result.dupe())
            .map_err(observed_module_source_outer)?
    } else {
        observations
    };
    match result.as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
            Ok((bytes.dupe(), observations))
        }
        PathObservationResult::FileBytes(PathOperationResult::Missing) => {
            Err(observed_module_source_complete(
                Err(ModuleSourcePreparationError::PatchFileInconsistentState {
                    demand,
                    before,
                    after: None,
                }),
                observations,
            ))
        }
        PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
            Err(observed_module_source_complete(
                Err(ModuleSourcePreparationError::PatchFileObservation {
                    demand,
                    error: *error,
                }),
                observations,
            ))
        }
        PathObservationResult::Lstat(_)
        | PathObservationResult::ReadLink(_)
        | PathObservationResult::DirectoryEntries(_)
        | PathObservationResult::WindowsLongPath(_)
        | PathObservationResult::WindowsOptionPathLongName(_) => {
            unreachable!("FileBytes demand must return FileBytes")
        }
    }
}

async fn module_source_effective(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
) -> Result<
    (
        Arc<Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>>,
        PathObservationEpoch,
    ),
    ModuleSourcePreparationDriverOutcome,
> {
    let workspace = NormalizedAbsolutePath::new(key.workspace.clone()).map_err(|error| {
        module_source_error(
            ModuleSourcePreparationError::RootModuleFiles(error.to_string().into()),
            PathObservationEpoch::empty(),
        )
    })?;
    if mode == ModuleSourcePreparationMode::Legacy {
        let result = ctx
            .compute(&HostEffectiveModuleOverrideKey::new(
                workspace,
                key.module_name.clone(),
            ))
            .await
            .map_err(|error| {
                module_source_error(
                    ModuleSourcePreparationError::RootModuleFiles(error.to_string().into()),
                    PathObservationEpoch::empty(),
                )
            })?;
        return Ok((result, PathObservationEpoch::empty()));
    }
    let outcome = ctx
        .compute(&HostEffectiveModuleOverrideObservationKey::new(
            workspace,
            key.module_name.clone(),
        ))
        .await
        .map_err(|error| {
            module_source_error(
                ModuleSourcePreparationError::RootModuleFiles(error.to_string().into()),
                PathObservationEpoch::empty(),
            )
        })?;
    let (observed, observations) =
        finish_module_source_observed_child(outcome, PathObservationEpoch::empty(), |observed| {
            observed.observations()
        })?;
    Ok((observed.result().dupe(), observations))
}

async fn module_source_nonregistry(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
    observations: PathObservationEpoch,
) -> ModuleSourcePreparationDriverOutcome {
    let source_key = RepositorySourceFileKey {
        workspace: key.workspace.clone(),
        module_name: key.module_name.clone(),
        repo_relative_path: PathBuf::from("MODULE.bazel"),
    };
    let (result, incoming) = if mode == ModuleSourcePreparationMode::Legacy {
        let outcome = match ctx.compute(&source_key).await {
            Ok(outcome) => outcome,
            Err(error) => {
                return module_source_error(
                    ModuleSourcePreparationError::SourceCompute(Arc::from(error.to_string())),
                    observations,
                );
            }
        };
        match outcome {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => {
                (Arc::new(result), PathObservationEpoch::empty())
            }
        }
    } else {
        let outcome = match ctx
            .compute(&RepositorySourceFileObservationKey(source_key))
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                return module_source_error(
                    ModuleSourcePreparationError::SourceCompute(Arc::from(error.to_string())),
                    observations,
                );
            }
        };
        let (observed, incoming) = match finish_module_source_observed_child(
            outcome,
            PathObservationEpoch::empty(),
            |observed| observed.observations(),
        ) {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        (observed.result().dupe(), incoming)
    };
    let observations = match merge_module_source_observations(&observations, &incoming) {
        Ok(observations) => observations,
        Err(error) => return observed_module_source_outer(error),
    };
    observed_module_source_complete(
        match result.as_ref() {
            Ok(RepositorySourceFileValue::Present(bytes)) => {
                Ok(ModuleSourcePreparation::NonRegistry {
                    bytes: bytes.dupe(),
                })
            }
            Ok(RepositorySourceFileValue::Absent) => {
                Err(ModuleSourcePreparationError::ModuleNotFound {
                    module_file_attempts: Arc::from([]),
                })
            }
            Err(error) => Err(ModuleSourcePreparationError::Source(error.clone())),
        },
        observations,
    )
}

async fn module_source_policy(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
    observations: PathObservationEpoch,
) -> Result<(crate::RegistryPolicy, PathObservationEpoch), ModuleSourcePreparationDriverOutcome> {
    if mode == ModuleSourcePreparationMode::Legacy {
        let result = ctx
            .compute(&RegistryPolicyKey {
                workspace: key.workspace.clone(),
            })
            .await
            .map_err(|error| {
                module_source_error(
                    ModuleSourcePreparationError::RegistryPolicyCompute(error.to_string().into()),
                    observations.dupe(),
                )
            })?;
        return match result.as_ref() {
            Ok(policy) => Ok((policy.clone(), observations)),
            Err(error) => Err(observed_module_source_complete(
                Err(ModuleSourcePreparationError::RegistryPolicy(error.clone())),
                observations,
            )),
        };
    }
    let outcome = ctx
        .compute(&RegistryPolicyObservationKey::new(key.workspace.clone()))
        .await
        .map_err(|error| {
            module_source_error(
                ModuleSourcePreparationError::RegistryPolicyCompute(error.to_string().into()),
                observations.dupe(),
            )
        })?;
    let (observed, merged) =
        finish_module_source_observed_child(outcome, observations, |observed| {
            observed.observations()
        })?;
    match observed.result().as_ref() {
        Ok(policy) => Ok((policy.clone(), merged)),
        Err(error) => Err(observed_module_source_complete(
            Err(ModuleSourcePreparationError::RegistryPolicy(error.clone())),
            merged,
        )),
    }
}

async fn module_source_registry_file(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
    url: &RegistryFileUrl,
    attempts: &[RegistryModuleFileAttempt],
    observations: PathObservationEpoch,
) -> Result<
    (
        Arc<Result<RegistryFileValue, RegistryFileError>>,
        PathObservationEpoch,
    ),
    ModuleSourcePreparationDriverOutcome,
> {
    if mode == ModuleSourcePreparationMode::Legacy {
        let result = ctx
            .compute(&RegistryFileKey {
                workspace: key.workspace.clone(),
                url: url.dupe(),
            })
            .await
            .map_err(|error| {
                module_source_error(
                    ModuleSourcePreparationError::RegistryFileCompute {
                        url: url.dupe(),
                        prior_not_found_attempts: Arc::from(attempts),
                        message: error.to_string().into(),
                    },
                    observations.dupe(),
                )
            })?;
        return Ok((result, observations));
    }
    let outcome = ctx
        .compute(&RegistryFileObservationKey::new(
            key.workspace.clone(),
            url.dupe(),
        ))
        .await
        .map_err(|error| {
            module_source_error(
                ModuleSourcePreparationError::RegistryFileCompute {
                    url: url.dupe(),
                    prior_not_found_attempts: Arc::from(attempts),
                    message: error.to_string().into(),
                },
                observations.dupe(),
            )
        })?;
    let (observed, merged) =
        finish_module_source_observed_child(outcome, observations, |observed| {
            observed.observations()
        })?;
    Ok((observed.result().dupe(), merged))
}

async fn module_source_patches(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
    override_: Option<&RootModuleOverride>,
    mut bytes: Arc<[u8]>,
    mut observations: PathObservationEpoch,
) -> Result<(Arc<[u8]>, PathObservationEpoch), ModuleSourcePreparationDriverOutcome> {
    let Some(RootModuleOverride::RegistrySingle(override_)) = override_ else {
        return Ok((bytes, observations));
    };
    let mut patches = Vec::new();
    for label in override_.patches.iter() {
        let Some(path) = main_repo_patch_path(label) else {
            continue;
        };
        let logical_path =
            NormalizedAbsolutePath::new(key.workspace.join(path)).map_err(|error| {
                observed_module_source_complete(
                    Err(ModuleSourcePreparationError::InvalidPatchPath {
                        path: error.path().to_owned(),
                    }),
                    observations.dupe(),
                )
            })?;
        let (resolved, incoming) = if mode == ModuleSourcePreparationMode::Legacy {
            let outcome = ctx
                .compute(&ResolvedPathKey::new(
                    PathObservationNamespace::Host,
                    logical_path.dupe(),
                ))
                .await
                .map_err(|error| {
                    module_source_error(
                        ModuleSourcePreparationError::PatchResolutionCompute {
                            logical_path: logical_path.dupe(),
                            message: error.to_string().into(),
                        },
                        observations.dupe(),
                    )
                })?;
            match outcome {
                PathOutcome::Need(need) => {
                    return Err(SourcePreparationOutcome::path_need(need));
                }
                PathOutcome::Complete(result) => (result, PathObservationEpoch::empty()),
            }
        } else {
            let outcome = ctx
                .compute(&ResolvedPathObservationKey::new(
                    PathObservationNamespace::Host,
                    logical_path.dupe(),
                ))
                .await
                .map_err(|error| {
                    module_source_error(
                        ModuleSourcePreparationError::PatchResolutionCompute {
                            logical_path: logical_path.dupe(),
                            message: error.to_string().into(),
                        },
                        observations.dupe(),
                    )
                })?;
            let (observed, merged) =
                finish_module_source_observed_path(outcome, observations, |observed| {
                    observed.observations()
                })?;
            observations = merged;
            (observed.result().clone(), PathObservationEpoch::empty())
        };
        observations = merge_module_source_observations(&observations, &incoming)
            .map_err(observed_module_source_outer)?;
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                return Err(observed_module_source_complete(
                    Err(ModuleSourcePreparationError::PatchResolution(error)),
                    observations,
                ));
            }
        };
        match resolved.state() {
            ResolvedPathState::Missing => {
                return Err(observed_module_source_complete(
                    Err(ModuleSourcePreparationError::PatchMissing { logical_path }),
                    observations,
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
                return Err(observed_module_source_complete(
                    Err(ModuleSourcePreparationError::PatchWrongKind {
                        logical_path,
                        actual: lstat.kind(),
                    }),
                    observations,
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
        let outcome = ctx
            .compute(&PathObservationKey::new(demand.dupe()))
            .await
            .map_err(|error| {
                module_source_error(
                    ModuleSourcePreparationError::PatchFileCompute {
                        demand: demand.dupe(),
                        message: error.to_string().into(),
                    },
                    observations.dupe(),
                )
            })?;
        let result = match outcome {
            PathOutcome::Need(need) => {
                return Err(SourcePreparationOutcome::path_need(need));
            }
            PathOutcome::Complete(result) => result,
        };
        let before = match resolved.state() {
            ResolvedPathState::Present(lstat) => Some(lstat),
            ResolvedPathState::Missing => None,
        };
        let (patch, merged) = finish_module_source_patch_file(
            demand,
            result,
            before,
            observations,
            mode == ModuleSourcePreparationMode::Observed,
        )?;
        observations = merged;
        if !patch.is_empty() {
            bytes = apply_unified_patch(bytes, &patch, override_.patch_strip).map_err(|error| {
                observed_module_source_complete(
                    Err(ModuleSourcePreparationError::Patch(error.0)),
                    observations.dupe(),
                )
            })?;
        }
    }
    Ok((bytes, observations))
}

async fn drive_module_source_preparation(
    ctx: &mut DiceComputations<'_>,
    key: &ModuleSourcePreparationKey,
    mode: ModuleSourcePreparationMode,
) -> ModuleSourcePreparationDriverOutcome {
    let (effective, observations) = match module_source_effective(ctx, key, mode).await {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    let effective = match effective.as_ref() {
        Ok(effective) => effective,
        Err(error) => {
            return observed_module_source_complete(
                Err(ModuleSourcePreparationError::RootModuleFiles(
                    error.to_string().into(),
                )),
                observations,
            );
        }
    };
    let override_ = effective.override_().cloned();
    if matches!(override_, Some(RootModuleOverride::NonRegistry(_))) {
        return module_source_nonregistry(ctx, key, mode, observations).await;
    }
    if key.version.is_empty() {
        return observed_module_source_complete(
            Err(ModuleSourcePreparationError::MissingVersion),
            observations,
        );
    }
    let (policy, mut observations) = match module_source_policy(ctx, key, mode, observations).await
    {
        Ok(value) => value,
        Err(outcome) => return outcome,
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
    let module = ModuleKey::new(key.module_name.as_str(), key.version.as_str());
    let registries = match override_registry {
        Some(registry) => vec![registry],
        None => policy
            .urls()
            .as_slice()
            .iter()
            .map(|url| url.as_str())
            .collect(),
    };
    let mut attempts = Vec::new();
    for registry in registries {
        let url = RegistryFileUrl::new(registry_module_file_url(registry, &module));
        let (file, merged) = match module_source_registry_file(
            ctx,
            key,
            mode,
            &url,
            &attempts,
            observations,
        )
        .await
        {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        observations = merged;
        match file.as_ref() {
            Ok(RegistryFileValue::NotFound { .. }) => {
                attempts.push(RegistryModuleFileAttempt { url, sha256: None });
            }
            Ok(RegistryFileValue::Found { bytes, sha256, .. }) => {
                let selected_registry = RegistryBaseUrl::new(registry);
                let (bytes, observations) = match module_source_patches(
                    ctx,
                    key,
                    mode,
                    override_.as_ref(),
                    bytes.dupe(),
                    observations,
                )
                .await
                {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                };
                attempts.push(RegistryModuleFileAttempt {
                    url,
                    sha256: Some(*sha256),
                });
                return observed_module_source_complete(
                    Ok(ModuleSourcePreparation::Registry {
                        bytes,
                        selected_registry,
                        module_file_attempts: Arc::from(attempts),
                    }),
                    observations,
                );
            }
            Err(error) => {
                return observed_module_source_complete(
                    Err(ModuleSourcePreparationError::RegistryFile {
                        url,
                        prior_not_found_attempts: Arc::from(attempts),
                        error: error.clone(),
                    }),
                    observations,
                );
            }
        }
    }
    observed_module_source_complete(
        Err(ModuleSourcePreparationError::ModuleNotFound {
            module_file_attempts: Arc::from(attempts),
        }),
        observations,
    )
}

fn project_legacy_module_source(
    outcome: ModuleSourcePreparationDriverOutcome,
) -> SourcePreparationOutcome<ModuleSourcePreparationResult> {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            debug_assert!(observations.observations().is_empty());
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy module-source preparation cannot produce an observed outer")
        }
    }
}

#[async_trait]
impl Key for ModuleSourcePreparationKey {
    type Value = SourcePreparationOutcome<ModuleSourcePreparationResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_module_source(
            drive_module_source_preparation(ctx, self, ModuleSourcePreparationMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for ModuleSourcePreparationObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedModuleSourcePreparation, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_module_source_preparation(ctx, &self.0, ModuleSourcePreparationMode::Observed)
            .await
            .map(|result| {
                result.map(|(result, observations)| ObservedModuleSourcePreparation {
                    result,
                    observations,
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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRepositoryRelativePath(Arc<PathBuf>);
impl HostRepositoryRelativePath {
    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }
    pub fn path_arc(&self) -> &Arc<PathBuf> {
        &self.0
    }
}
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositoryRelativePathError {
    requested_path: Arc<PathBuf>,
}
impl HostRepositoryRelativePathError {
    pub fn requested_path(&self) -> &Path {
        self.requested_path.as_path()
    }
}
impl fmt::Display for HostRepositoryRelativePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid repository-relative path: {}",
            self.requested_path.display()
        )
    }
}
impl std::error::Error for HostRepositoryRelativePathError {}
#[doc(hidden)]
pub fn host_repository_relative_path(
    requested_path: PathBuf,
) -> Result<HostRepositoryRelativePath, HostRepositoryRelativePathError> {
    match checked_relative_path(&requested_path) {
        Ok(_) => Ok(HostRepositoryRelativePath(Arc::new(requested_path))),
        Err(_) => Err(HostRepositoryRelativePathError {
            requested_path: Arc::new(requested_path),
        }),
    }
}

pub fn source_identity(bytes: &[u8]) -> Arc<str> {
    Arc::from(hex::encode(Sha256::digest(bytes)))
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryModuleClosureKey {
    workspace: NormalizedAbsolutePath,
    module: NonrootModuleKey,
}

impl HostNonregistryModuleClosureKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, module: NonrootModuleKey) -> Self {
        Self { workspace, module }
    }
}

impl fmt::Display for HostNonregistryModuleClosureKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-nonregistry-module-closure:{}@{}",
            self.module.name, self.module.version
        )
    }
}

type HostNonregistryModuleClosureValue = SourcePreparationOutcome<
    Arc<Result<HostNonregistryModuleClosure, HostNonregistryModuleClosureError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling is callerless until the selected-graph frontier.
struct HostNonregistryModuleClosureObservationKey(HostNonregistryModuleClosureKey);

impl fmt::Display for HostNonregistryModuleClosureObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type HostNonregistryModuleClosureResult =
    Arc<Result<HostNonregistryModuleClosure, HostNonregistryModuleClosureError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedHostNonregistryModuleClosure {
    result: HostNonregistryModuleClosureResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)] // Accessed only by the callerless observed sibling and its proof.
impl ObservedHostNonregistryModuleClosure {
    fn result(&self) -> &HostNonregistryModuleClosureResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostNonregistryModuleClosureObservationError {
    EffectiveFrontier(ObservedPathFrontierError),
    EffectiveCompute(Arc<str>),
    MaterializationFrontier(ObservedPathFrontierError),
    RootSourceFrontier(ObservedPathFrontierError),
    PreparationFrontier(NonregistryPreparationFrontierError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Observed mode remains dormant until an upper owner is accepted.
enum HostNonregistryModuleClosureMode {
    Legacy,
    Observed,
}

type HostNonregistryModuleClosureDriverOutcome = SourcePreparationOutcome<
    Result<ObservedHostNonregistryModuleClosure, HostNonregistryModuleClosureObservationError>,
>;

struct HostNonregistryModuleClosureInput {
    source_identity: HostNonregistryModuleSourceIdentity,
    local_path_policy: HostRepositoryLocalPathPolicy,
    observations: PathObservationEpoch,
}

fn observed_host_nonregistry_closure_complete(
    result: Result<HostNonregistryModuleClosure, HostNonregistryModuleClosureError>,
    observations: PathObservationEpoch,
) -> HostNonregistryModuleClosureDriverOutcome {
    SourcePreparationOutcome::Complete(Ok(ObservedHostNonregistryModuleClosure {
        result: Arc::new(result),
        observations,
    }))
}

fn observed_host_nonregistry_closure_outer(
    error: HostNonregistryModuleClosureObservationError,
) -> HostNonregistryModuleClosureDriverOutcome {
    SourcePreparationOutcome::Complete(Err(error))
}

fn forward_host_nonregistry_closure_observation<T>(
    outcome: SourcePreparationOutcome<Result<T, ObservedPathFrontierError>>,
    frontier: impl FnOnce(ObservedPathFrontierError) -> HostNonregistryModuleClosureObservationError,
) -> ControlFlow<HostNonregistryModuleClosureDriverOutcome, T> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(observed_host_nonregistry_closure_outer(frontier(error)))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => ControlFlow::Continue(observed),
    }
}

fn host_nonregistry_closure_compute_error(
    error: HostNonregistryModuleClosureError,
    observations: PathObservationEpoch,
) -> HostNonregistryModuleClosureDriverOutcome {
    observed_host_nonregistry_closure_complete(Err(error), observations)
}
fn finish_host_nonregistry_effective(
    key: &HostNonregistryModuleClosureKey,
    effective: &Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>,
    observations: PathObservationEpoch,
) -> ControlFlow<HostNonregistryModuleClosureDriverOutcome, HostRepositoryLocalPathPolicy> {
    let effective = match effective {
        Ok(effective) => effective,
        Err(error) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_complete(
                Err(HostNonregistryModuleClosureError::RootModuleFiles(
                    error.to_string().into(),
                )),
                observations,
            ));
        }
    };
    if !matches!(
        effective.override_(),
        Some(RootModuleOverride::NonRegistry(_))
    ) {
        return ControlFlow::Break(observed_host_nonregistry_closure_complete(
            Err(
                HostNonregistryModuleClosureError::NonregistryOverrideRequired(
                    key.module.name.clone(),
                ),
            ),
            observations,
        ));
    }
    ControlFlow::Continue(local_path_policy(effective))
}
fn finish_host_nonregistry_materialization(
    materialization: &Result<RepositoryMaterialization, RepositoryMaterializationError>,
    incoming: &PathObservationEpoch,
    mut observations: PathObservationEpoch,
    local_path_policy: HostRepositoryLocalPathPolicy,
) -> ControlFlow<HostNonregistryModuleClosureDriverOutcome, HostNonregistryModuleClosureInput> {
    observations = match merge_path_observations(&observations, incoming) {
        Ok(observations) => observations,
        Err(error) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_outer(
                HostNonregistryModuleClosureObservationError::MaterializationFrontier(error),
            ));
        }
    };
    let source_identity = match materialization {
        Err(error) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_complete(
                Err(HostNonregistryModuleClosureError::Materialization(
                    error.clone(),
                )),
                observations,
            ));
        }
        Ok(RepositoryMaterialization::Local { repo_spec, .. }) => {
            HostNonregistryModuleSourceIdentity::Local {
                repo_spec: repo_spec.clone(),
            }
        }
        Ok(RepositoryMaterialization::Immutable {
            repo_spec,
            source_identity,
            ..
        }) => HostNonregistryModuleSourceIdentity::Immutable {
            repo_spec: repo_spec.clone(),
            source_identity: source_identity.dupe(),
        },
    };
    ControlFlow::Continue(HostNonregistryModuleClosureInput {
        source_identity,
        local_path_policy,
        observations,
    })
}

fn finish_host_nonregistry_root_source(
    source: Result<RepositorySourceFileValue, RepositorySourceFileError>,
    incoming: &PathObservationEpoch,
    mut input: HostNonregistryModuleClosureInput,
) -> ControlFlow<
    HostNonregistryModuleClosureDriverOutcome,
    (Arc<[u8]>, HostNonregistryModuleClosureInput),
> {
    input.observations = match merge_path_observations(&input.observations, incoming) {
        Ok(observations) => observations,
        Err(error) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_outer(
                HostNonregistryModuleClosureObservationError::RootSourceFrontier(error),
            ));
        }
    };
    let bytes = match source {
        Ok(RepositorySourceFileValue::Present(bytes)) => bytes,
        Ok(RepositorySourceFileValue::Absent) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_complete(
                Err(HostNonregistryModuleClosureError::RootAbsent),
                input.observations,
            ));
        }
        Err(error) => {
            return ControlFlow::Break(observed_host_nonregistry_closure_complete(
                Err(HostNonregistryModuleClosureError::RootSource(error)),
                input.observations,
            ));
        }
    };
    ControlFlow::Continue((bytes, input))
}

async fn compute_host_nonregistry_closure_input(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryModuleClosureKey,
    mode: HostNonregistryModuleClosureMode,
) -> ControlFlow<HostNonregistryModuleClosureDriverOutcome, HostNonregistryModuleClosureInput> {
    let (effective, observations) = match mode {
        HostNonregistryModuleClosureMode::Legacy => (
            preflight_dice_invariant(
                ctx.compute(&HostEffectiveModuleOverrideKey::new(
                    key.workspace.dupe(),
                    key.module.name.clone(),
                ))
                .await,
            ),
            PathObservationEpoch::empty(),
        ),
        HostNonregistryModuleClosureMode::Observed => match ctx
            .compute(&HostEffectiveModuleOverrideObservationKey::new(
                key.workspace.dupe(),
                key.module.name.clone(),
            ))
            .await
        {
            Err(error) => {
                return ControlFlow::Break(observed_host_nonregistry_closure_outer(
                    HostNonregistryModuleClosureObservationError::EffectiveCompute(
                        error.to_string().into(),
                    ),
                ));
            }
            Ok(outcome) => match forward_host_nonregistry_closure_observation(
                outcome,
                HostNonregistryModuleClosureObservationError::EffectiveFrontier,
            ) {
                ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                ControlFlow::Continue(observed) => {
                    (observed.result().dupe(), observed.observations().dupe())
                }
            },
        },
    };
    let local_path_policy =
        match finish_host_nonregistry_effective(key, effective.as_ref(), observations.dupe()) {
            ControlFlow::Continue(policy) => policy,
            ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
        };

    let materialization_key = RepositoryMaterializationKey {
        workspace: key.workspace.as_path().to_path_buf(),
        module_name: key.module.name.clone(),
    };
    let (materialization, incoming) = match mode {
        HostNonregistryModuleClosureMode::Legacy => match ctx.compute(&materialization_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return ControlFlow::Break(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return ControlFlow::Break(host_nonregistry_closure_compute_error(
                    HostNonregistryModuleClosureError::MaterializationCompute(
                        error.to_string().into(),
                    ),
                    observations,
                ));
            }
        },
        HostNonregistryModuleClosureMode::Observed => match ctx
            .compute(&RepositoryMaterializationObservationKey(
                materialization_key,
            ))
            .await
        {
            Ok(outcome) => match forward_host_nonregistry_closure_observation(
                outcome,
                HostNonregistryModuleClosureObservationError::MaterializationFrontier,
            ) {
                ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                ControlFlow::Continue(observed) => {
                    (observed.result().dupe(), observed.observations().dupe())
                }
            },
            Err(error) => {
                return ControlFlow::Break(host_nonregistry_closure_compute_error(
                    HostNonregistryModuleClosureError::MaterializationCompute(
                        error.to_string().into(),
                    ),
                    observations,
                ));
            }
        },
    };
    finish_host_nonregistry_materialization(
        materialization.as_ref(),
        &incoming,
        observations,
        local_path_policy,
    )
}

async fn compute_host_nonregistry_root_source(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryModuleClosureKey,
    mode: HostNonregistryModuleClosureMode,
    input: HostNonregistryModuleClosureInput,
) -> ControlFlow<
    HostNonregistryModuleClosureDriverOutcome,
    (Arc<[u8]>, HostNonregistryModuleClosureInput),
> {
    let source_key = RepositorySourceFileKey {
        workspace: key.workspace.as_path().to_path_buf(),
        module_name: key.module.name.clone(),
        repo_relative_path: PathBuf::from("MODULE.bazel"),
    };
    let (source, incoming) = match mode {
        HostNonregistryModuleClosureMode::Legacy => match ctx.compute(&source_key).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return ControlFlow::Break(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return ControlFlow::Break(host_nonregistry_closure_compute_error(
                    HostNonregistryModuleClosureError::RootSourceCompute(error.to_string().into()),
                    input.observations,
                ));
            }
        },
        HostNonregistryModuleClosureMode::Observed => match ctx
            .compute(&RepositorySourceFileObservationKey(source_key))
            .await
        {
            Ok(outcome) => match forward_host_nonregistry_closure_observation(
                outcome,
                HostNonregistryModuleClosureObservationError::RootSourceFrontier,
            ) {
                ControlFlow::Break(outcome) => return ControlFlow::Break(outcome),
                ControlFlow::Continue(observed) => (
                    observed.result().as_ref().clone(),
                    observed.observations().dupe(),
                ),
            },
            Err(error) => {
                return ControlFlow::Break(host_nonregistry_closure_compute_error(
                    HostNonregistryModuleClosureError::RootSourceCompute(error.to_string().into()),
                    input.observations,
                ));
            }
        },
    };
    finish_host_nonregistry_root_source(source, &incoming, input)
}
fn host_nonregistry_logical_path(
    module: &NonrootModuleKey,
    repo_relative_path: &Path,
) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(
        PathBuf::from("/.slug-nonregistry")
            .join(format!("{}@{}", module.name, module.version))
            .join(repo_relative_path),
    )
    .expect("module identity and normalized repository-relative path form a logical path")
}

async fn drive_host_nonregistry_module_closure(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryModuleClosureKey,
    mode: HostNonregistryModuleClosureMode,
) -> HostNonregistryModuleClosureDriverOutcome {
    let input = match compute_host_nonregistry_closure_input(ctx, key, mode).await {
        ControlFlow::Continue(input) => input,
        ControlFlow::Break(outcome) => return outcome,
    };
    let (root_source, input) =
        match compute_host_nonregistry_root_source(ctx, key, mode, input).await {
            ControlFlow::Continue(value) => value,
            ControlFlow::Break(outcome) => return outcome,
        };
    let root_path = host_nonregistry_logical_path(&key.module, Path::new("MODULE.bazel"));
    let inspection = match validate_root_module_source(
        crate::LogicalModuleFileId::new(root_path.as_path().display().to_string()),
        &root_source,
    ) {
        Ok(inspection) => inspection,
        Err(message) => {
            return observed_host_nonregistry_closure_complete(
                Err(HostNonregistryModuleClosureError::RootValidation {
                    logical_path: root_path,
                    message,
                }),
                input.observations,
            );
        }
    };
    let root = HostNonregistryModuleRoot {
        logical_path: root_path,
        bytes: root_source,
        inspection: inspection.clone(),
    };
    let preparation_mode = match mode {
        HostNonregistryModuleClosureMode::Legacy => HostRepositoryObservationMode::Legacy,
        HostNonregistryModuleClosureMode::Observed => HostRepositoryObservationMode::Observed,
    };
    let prepared = match drive_nonregistry_module(
        ctx,
        NonregistryPreparationOwner::Host {
            workspace: key.workspace.dupe(),
            module: key.module.clone(),
        },
        inspection.includes,
        input.observations,
        preparation_mode,
    )
    .await
    {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return observed_host_nonregistry_closure_outer(
                HostNonregistryModuleClosureObservationError::PreparationFrontier(error),
            );
        }
        SourcePreparationOutcome::Complete(Ok(prepared)) => prepared,
    };
    let observations = prepared.observations;
    let prepared = match prepared.result {
        Ok(prepared) => prepared,
        Err(NonregistryPreparationError::Host(error)) => {
            return observed_host_nonregistry_closure_complete(Err(error), observations);
        }
        Err(NonregistryPreparationError::Direct(_)) => {
            unreachable!("Host preparation cannot produce a direct-local error")
        }
    };
    let (fragments, capability) = match prepared {
        NonregistryPreparedModule::Supported(fragments) => (fragments, None),
        NonregistryPreparedModule::UnsupportedCycle {
            fragments,
            capability,
        } => (fragments, Some(capability)),
    };
    let closure = HostNonregistryPreparedClosure {
        module: key.module.clone(),
        source_identity: input.source_identity,
        local_path_policy: input.local_path_policy,
        root,
        fragments,
    };
    observed_host_nonregistry_closure_complete(
        Ok(match capability {
            Some(capability) => HostNonregistryModuleClosure::UnsupportedCycle {
                closure,
                capability,
            },
            None => HostNonregistryModuleClosure::Supported(closure),
        }),
        observations,
    )
}

fn project_legacy_host_nonregistry_closure(
    outcome: HostNonregistryModuleClosureDriverOutcome,
) -> HostNonregistryModuleClosureValue {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            SourcePreparationOutcome::Complete(observed.result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy closure cannot produce an observed outer error")
        }
    }
}
#[async_trait]
impl Key for HostNonregistryModuleClosureKey {
    type Value = HostNonregistryModuleClosureValue;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_host_nonregistry_closure(
            drive_host_nonregistry_module_closure(
                ctx,
                self,
                HostNonregistryModuleClosureMode::Legacy,
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
}

#[async_trait]
impl Key for HostNonregistryModuleClosureObservationKey {
    type Value = HostNonregistryModuleClosureDriverOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_nonregistry_module_closure(
            ctx,
            &self.0,
            HostNonregistryModuleClosureMode::Observed,
        )
        .await
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
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
    struct SourceObservationTracker {
        observation: Mutex<Vec<(ActivationKind, bool)>>,
        builtin: Mutex<Vec<ActivationKind>>,
        result: Mutex<Vec<ActivationKind>>,
        forbidden: Mutex<Vec<String>>,
    }

    impl ActivationTracker for SourceObservationTracker {
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
            let kind = activation.kind();
            if key
                .downcast_ref::<HostRepositorySourceObservationKey>()
                .is_some()
            {
                self.observation
                    .lock()
                    .unwrap()
                    .push((kind, activation.evaluation_data().is_none()));
            } else if key
                .downcast_ref::<BuiltinBazelToolsSourceFileKey>()
                .is_some()
            {
                self.builtin.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<RepositoryMaterializationResultKey>()
                .is_some()
            {
                self.result.lock().unwrap().push(kind);
            } else if key.downcast_ref::<HostRepositoryPathKey>().is_some()
                || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
                || key.downcast_ref::<RepositoryMaterializationKey>().is_some()
                || key
                    .downcast_ref::<RepositoryMaterializationGenerationKey>()
                    .is_some()
                || key.downcast_ref::<RootRepositoryRouteKey>().is_some()
                || key
                    .downcast_ref::<crate::RepositoryPackageSourceKey>()
                    .is_some()
                || key.downcast_ref::<crate::RootPackageSourceKey>().is_some()
            {
                self.forbidden.lock().unwrap().push(key.to_string());
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
        closure: Mutex<Vec<ActivationKind>>,
        discovered: Mutex<Vec<(ActivationKind, bool)>>,
        observed: Mutex<Vec<String>>,
        rows: Mutex<Vec<(String, Vec<String>)>>,
        batches: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
    }

    impl ActivationTracker for NonregistryPreflightTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.rows
                .lock()
                .unwrap()
                .push((key.to_string(), deps.map(ToString::to_string).collect()));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let record = (activation.kind(), activation.evaluation_data().is_none());
            self.batches.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
            if key
                .downcast_ref::<HostNonregistryPackagePreflightObservationKey>()
                .is_some()
            {
                self.preflight.lock().unwrap().push(record);
            } else if key
                .downcast_ref::<crate::repo_file::HostNonregistryRepoFileObservationKey>()
                .is_some()
            {
                self.repo.lock().unwrap().push(record);
            } else if key
                .downcast_ref::<HostNonregistryModuleClosureObservationKey>()
                .is_some()
            {
                self.closure.lock().unwrap().push(activation.kind());
            } else if key.to_string().starts_with("observed-") {
                self.observed.lock().unwrap().push(key.to_string());
            } else if key.downcast_ref::<HostDiscoveredModuleKey>().is_some() {
                self.discovered.lock().unwrap().push(record);
            } else if key
                .downcast_ref::<HostNonregistryModuleClosureKey>()
                .is_some()
            {
                self.closure.lock().unwrap().push(activation.kind());
            } else if key
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
        let error = root_repository_materialization_request(&route).unwrap_err();
        assert_eq!(
            error,
            RepositoryMaterializationError::Spec(
                "built-in bazel_tools source requires its immutable source owner".into()
            )
        );
        let capability = route.source_capability();
        let crate::HostRepositoryMaterializationDisposition::Builtin(actual) =
            crate::host_repository_materialization_request(&capability).unwrap()
        else {
            unreachable!()
        };
        let expected = BuiltinBazelToolsSnapshot::CURRENT.route_identity();
        assert_eq!(actual, expected);
    }

    #[test]
    fn source_capability_projection_is_policy_exact_and_computation_free() {
        use HostRepositoryLocalPathPolicy::CommandAbsolute;
        use HostRepositoryLocalPathPolicy::LocalUnsupported;
        use HostRepositoryLocalPathPolicy::WorkspaceRelative;
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let project = |spec: &RepoSpec, policy, apparent: &str| {
            let capability = crate::HostRepositorySourceCapability::from_repo_spec(
                workspace.dupe(),
                ApparentRepoName::new(apparent).unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
                spec,
                policy,
            )
            .unwrap();
            crate::host_repository_materialization_request(&capability)
        };
        let request = |spec, policy, apparent| match project(spec, policy, apparent).unwrap() {
            crate::HostRepositoryMaterializationDisposition::Request(request) => request,
            crate::HostRepositoryMaterializationDisposition::Builtin(_) => unreachable!(),
        };
        let relative = local_route().repo_spec().clone();
        let relative_request = request(&relative, WorkspaceRelative, "dep_alias");
        assert_eq!(relative_request.id.workspace, workspace);
        assert_eq!(relative_request.id.canonical_repo.as_str(), "dep+");
        let projected = &relative_request.repo_spec;
        assert_eq!(projected, &relative);
        assert!(Arc::ptr_eq(&projected.attributes, &relative.attributes));
        assert_eq!(
            relative_request.kind,
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap()
            }
        );
        let spec_error = |message: &str| RepositoryMaterializationError::Spec(message.into());
        assert_eq!(
            project(&relative, CommandAbsolute, "dep_alias").unwrap_err(),
            spec_error("command local_repository path must be normalized and absolute")
        );
        assert_eq!(
            project(&relative, LocalUnsupported, "dep_alias").unwrap_err(),
            spec_error("local_repository is unsupported for this repository source")
        );
        let mut git = immutable_route().repo_spec().clone();
        git.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:git.bzl").unwrap();
        git.rule_id.rule_name = "git_repository".into();
        let immutable = immutable_route();
        for spec in [immutable.repo_spec(), &git] {
            for policy in [WorkspaceRelative, CommandAbsolute, LocalUnsupported] {
                assert_eq!(
                    request(spec, policy, "bazel_tools").kind,
                    RepositoryMaterializationKind::Immutable
                );
            }
        }
        let repeated = request(&relative, WorkspaceRelative, "dep_alias");
        assert_eq!(relative_request, repeated);
        assert_eq!(
            relative_request,
            request(&relative, WorkspaceRelative, "other_alias")
        );
        assert!(Arc::ptr_eq(&relative_request, &relative_request.dupe()));
        assert_eq!(
            root_repository_materialization_request(&local_route()).unwrap(),
            relative_request
        );

        let mut custom = relative.clone();
        custom.rule_id.rule_name = "user_repository_rule".into();
        let plan = crate::GeneratedRepositoryFileEffectPlan::build([(
            CompactString::new("BUILD.bazel"),
            Arc::<[u8]>::from(&b"exports_files([])\n"[..]),
            true,
        )])
        .unwrap();
        let generated = RootRepositoryRoute::for_generated_repo_spec(
            workspace.dupe(),
            ApparentRepoName::new("generated").unwrap(),
            CanonicalRepoName::new("ext+generated").unwrap(),
            custom,
            LocalUnsupported,
            plan.clone(),
        )
        .unwrap();
        let crate::HostRepositoryMaterializationDisposition::Request(generated_request) =
            crate::host_repository_materialization_request(&generated.source_capability()).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            generated_request.kind,
            RepositoryMaterializationKind::GeneratedFileEffects(plan)
        );
    }

    #[test]
    fn generated_plan_is_structural_for_routes_and_requests_but_not_result_key_identity() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let spec = RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@extension+repo//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        };
        let plan = |effects: [(&str, &[u8], bool); 2]| {
            crate::GeneratedRepositoryFileEffectPlan::build(effects.into_iter().map(
                |(path, content, executable)| {
                    (
                        CompactString::new(path),
                        Arc::<[u8]>::from(content),
                        executable,
                    )
                },
            ))
            .unwrap()
        };
        let route = |plan| {
            RootRepositoryRoute::for_generated_repo_spec(
                workspace.dupe(),
                ApparentRepoName::new("generated").unwrap(),
                CanonicalRepoName::new("extension+generated").unwrap(),
                spec.clone(),
                HostRepositoryLocalPathPolicy::LocalUnsupported,
                plan,
            )
            .unwrap()
        };
        let a = route(plan([
            ("BUILD.bazel", b"a", true),
            ("generated.txt", b"x", false),
        ]));
        let content = route(plan([
            ("BUILD.bazel", b"b", true),
            ("generated.txt", b"x", false),
        ]));
        let order = route(plan([
            ("generated.txt", b"x", false),
            ("BUILD.bazel", b"a", true),
        ]));
        let executable = route(plan([
            ("BUILD.bazel", b"a", false),
            ("generated.txt", b"x", false),
        ]));
        let restored = route(plan([
            ("BUILD.bazel", b"a", true),
            ("generated.txt", b"x", false),
        ]));
        fn hash<T: Hash>(value: &T) -> u64 {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        }
        let a_capability = a.source_capability();
        let restored_capability = restored.source_capability();
        for variant in [&content, &order, &executable] {
            assert_ne!(a, *variant);
            assert_ne!(hash(&a), hash(variant));
            let capability = variant.source_capability();
            assert_ne!(a_capability, capability);
            assert_ne!(hash(&a_capability), hash(&capability));
        }
        assert_eq!(a, restored);
        assert_eq!(hash(&a), hash(&restored));
        assert_eq!(a_capability, restored_capability);
        assert_eq!(hash(&a_capability), hash(&restored_capability));
        let request_a = root_repository_materialization_request(&a).unwrap();
        let request_content = root_repository_materialization_request(&content).unwrap();
        let request_order = root_repository_materialization_request(&order).unwrap();
        let request_executable = root_repository_materialization_request(&executable).unwrap();
        let request_restored = root_repository_materialization_request(&restored).unwrap();
        for variant in [&request_content, &request_order, &request_executable] {
            assert_ne!(request_a, *variant);
        }
        assert_eq!(request_a, request_restored);
        let key_a = RepositoryMaterializationResultKey { request: request_a };
        let key_content = RepositoryMaterializationResultKey {
            request: request_content,
        };
        let key_order = RepositoryMaterializationResultKey {
            request: request_order,
        };
        let key_executable = RepositoryMaterializationResultKey {
            request: request_executable,
        };
        let key_restored = RepositoryMaterializationResultKey {
            request: request_restored,
        };
        let hash_key = |value: &RepositoryMaterializationResultKey| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        for variant in [&key_content, &key_order, &key_executable, &key_restored] {
            assert_eq!(hash_key(&key_a), hash_key(variant));
        }
    }

    #[test]
    fn source_capability_projection_local_matrix_and_identity_are_exact() {
        use HostRepositoryLocalPathPolicy::CommandAbsolute;
        use HostRepositoryLocalPathPolicy::LocalUnsupported;
        use HostRepositoryLocalPathPolicy::WorkspaceRelative;
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let project_with = |workspace: &NormalizedAbsolutePath,
                            canonical: &str,
                            spec: &RepoSpec,
                            policy: HostRepositoryLocalPathPolicy|
         -> Result<
            crate::HostRepositoryMaterializationDisposition,
            RepositoryMaterializationError,
        > {
            let capability = crate::HostRepositorySourceCapability::from_repo_spec(
                workspace.dupe(),
                ApparentRepoName::new("dep_alias").unwrap(),
                CanonicalRepoName::new(canonical).unwrap(),
                spec,
                policy,
            )
            .unwrap();
            crate::host_repository_materialization_request(&capability)
        };
        let project = |spec: &RepoSpec, policy| project_with(&workspace, "dep+", spec, policy);
        let spec = |path: &str| local_route_with_path(path).repo_spec().clone();
        let error = |spec: &RepoSpec, policy| match project(spec, policy).unwrap_err() {
            RepositoryMaterializationError::Spec(message) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        let relative = "local_repository path must be normalized and workspace-relative";
        let absolute = "command local_repository path must be normalized and absolute";
        for (path, policy, expected) in [
            ("", WorkspaceRelative, relative),
            (".", WorkspaceRelative, relative),
            ("..", WorkspaceRelative, relative),
            ("/absolute", WorkspaceRelative, relative),
            ("", CommandAbsolute, absolute),
            (".", CommandAbsolute, absolute),
            ("..", CommandAbsolute, absolute),
            ("relative", CommandAbsolute, absolute),
        ] {
            assert_eq!(error(&spec(path), policy).as_str(), expected);
            assert_eq!(
                error(&spec(path), LocalUnsupported).as_str(),
                "local_repository is unsupported for this repository source"
            );
        }
        for (path, root) in [("/", "/"), ("/command", "/command")] {
            let crate::HostRepositoryMaterializationDisposition::Request(request) =
                project(&spec(path), CommandAbsolute).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(
                request.kind,
                RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new(root).unwrap()
                }
            );
        }

        let mut missing = spec("dep");
        missing.attributes = Arc::default();
        let mut malformed = spec("dep");
        malformed.attributes = Arc::new(SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::Bool(true),
        )]));
        let mut extra = spec("dep");
        Arc::make_mut(&mut extra.attributes).insert(
            CompactString::new("extra"),
            OverrideAttributeValue::String("value".into()),
        );
        for (candidate, expected) in [
            (&missing, "local_repository has unsupported attributes"),
            (&malformed, "local_repository requires a string path"),
            (&extra, "local_repository has unsupported attributes"),
        ] {
            assert_eq!(error(candidate, WorkspaceRelative).as_str(), expected);
            assert_eq!(
                error(candidate, LocalUnsupported).as_str(),
                "local_repository is unsupported for this repository source"
            );
        }

        let mut unsupported = immutable_route().repo_spec().clone();
        unsupported.rule_id.rule_name = "custom_repository".into();
        assert_eq!(
            error(&unsupported, WorkspaceRelative).as_str(),
            "unsupported repository override rule"
        );
        let baseline = spec("dep");
        let a = project(&baseline, WorkspaceRelative);
        let other_workspace = NormalizedAbsolutePath::new("/other").unwrap();
        for b in [
            project_with(&other_workspace, "dep+", &baseline, WorkspaceRelative),
            project_with(&workspace, "other+", &baseline, WorkspaceRelative),
            project(&baseline, CommandAbsolute),
            project(&spec("other"), WorkspaceRelative),
            project(&extra, WorkspaceRelative),
        ] {
            assert_ne!(a, b);
            assert_eq!(a, project(&baseline, WorkspaceRelative));
        }
    }

    #[test]
    fn repository_source_input_owns_one_exact_projection() {
        use HostRepositoryLocalPathPolicy::CommandAbsolute;
        use HostRepositoryLocalPathPolicy::LocalUnsupported;
        use HostRepositoryLocalPathPolicy::WorkspaceRelative;

        use crate::HostRepositorySourceInputDispositionView as InputView;
        let builtin_capability = RootRepositoryRoute::builtin_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
        )
        .source_capability();
        let builtin = crate::host_repository_source_input(builtin_capability.clone()).unwrap();
        assert_eq!(builtin.view().capability(), &builtin_capability);
        let InputView::Builtin(identity) = builtin.view().disposition() else {
            panic!("builtin capability must retain the builtin disposition");
        };
        let expected_builtin = BuiltinBazelToolsSnapshot::CURRENT.route_identity();
        assert_eq!(identity, &expected_builtin);
        let route = local_route();
        let capability = route.source_capability();
        let input = crate::host_repository_source_input(capability.clone()).unwrap();
        let InputView::Request(request) = input.view().disposition() else {
            panic!("RepoSpec capability must retain the request disposition");
        };
        assert_eq!(input.view().capability(), &capability);
        assert_eq!(&request.id.workspace, capability.workspace());
        assert_eq!(&request.id.canonical_repo, capability.canonical_repo());
        assert_eq!(&request.repo_spec, capability.repo_spec().unwrap());
        assert!(Arc::ptr_eq(
            &request.repo_spec.attributes,
            &capability.repo_spec().unwrap().attributes,
        ));
        assert_eq!(
            request.kind,
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
            }
        );
        let cloned = input.clone();
        let InputView::Request(cloned_request) = cloned.view().disposition() else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(request, cloned_request));
        let repeated = crate::host_repository_source_input(capability).unwrap();
        assert_eq!(input, repeated);
        let rejected = crate::HostRepositorySourceCapability::from_repo_spec(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
            CanonicalRepoName::new("dep+").unwrap(),
            local_route().repo_spec(),
            LocalUnsupported,
        )
        .unwrap();
        assert_eq!(
            crate::host_repository_source_input(rejected).unwrap_err(),
            crate::HostRepositorySourceInputError::Projection(
                RepositoryMaterializationError::Spec(
                    "local_repository is unsupported for this repository source".into(),
                ),
            )
        );
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let baseline_spec = immutable_route().repo_spec().clone();
        let make = |workspace: &NormalizedAbsolutePath,
                    apparent: &str,
                    canonical: &str,
                    spec: &RepoSpec,
                    policy| {
            crate::host_repository_source_input(
                crate::HostRepositorySourceCapability::from_repo_spec(
                    workspace.dupe(),
                    ApparentRepoName::new(apparent).unwrap(),
                    CanonicalRepoName::new(canonical).unwrap(),
                    spec,
                    policy,
                )
                .unwrap(),
            )
            .unwrap()
        };
        let a = make(
            &workspace,
            "dep_alias",
            "dep+",
            &baseline_spec,
            WorkspaceRelative,
        );
        let mut changed_spec = baseline_spec.clone();
        Arc::make_mut(&mut changed_spec.attributes).insert(
            CompactString::new("integrity"),
            OverrideAttributeValue::String("changed".into()),
        );
        let other_workspace = NormalizedAbsolutePath::new("/other").unwrap();
        for b in [
            make(
                &other_workspace,
                "dep_alias",
                "dep+",
                &baseline_spec,
                WorkspaceRelative,
            ),
            make(
                &workspace,
                "other_alias",
                "dep+",
                &baseline_spec,
                WorkspaceRelative,
            ),
            make(
                &workspace,
                "dep_alias",
                "other+",
                &baseline_spec,
                WorkspaceRelative,
            ),
            make(
                &workspace,
                "dep_alias",
                "dep+",
                &baseline_spec,
                CommandAbsolute,
            ),
            make(
                &workspace,
                "dep_alias",
                "dep+",
                &changed_spec,
                WorkspaceRelative,
            ),
            builtin,
        ] {
            assert_ne!(a, b);
            assert_eq!(
                a,
                make(
                    &workspace,
                    "dep_alias",
                    "dep+",
                    &baseline_spec,
                    WorkspaceRelative,
                )
            );
        }
    }

    fn source_observation_key(
        input: HostRepositorySourceInput,
        path: &str,
    ) -> HostRepositorySourceObservationKey {
        HostRepositorySourceObservationKey::new(
            input,
            host_repository_relative_path(path.into()).unwrap(),
        )
    }

    #[tokio::test]
    async fn source_observation_builtin_is_lossless_and_scope_free() {
        let input = host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .source_capability(),
        )
        .unwrap();
        let key = source_observation_key(input.clone(), "MODULE.bazel");
        assert_eq!(
            DynKey::from_key(key.clone()).request_value::<RepositorySourceScope>(),
            None
        );
        let tracker = Arc::new(SourceObservationTracker::default());
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        };
        let mut tx = dice.updater_with_data(user_data).commit().await;
        let outcome = tx.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &outcome else {
            unreachable!()
        };
        let HostRepositorySourceObservationView::Builtin(value) =
            value.as_ref().as_ref().unwrap().view()
        else {
            unreachable!()
        };
        assert_eq!(value.path(), "MODULE.bazel");
        assert_eq!(
            value.sha256().as_slice(),
            Sha256::digest(value.bytes()).as_slice()
        );
        assert!(!value.bytes().is_empty());
        assert!(value.executable());
        assert!(HostRepositorySourceObservationKey::equality(
            &tx.compute(&key).await.unwrap(),
            &outcome,
        ));
        assert_eq!(
            tracker.observation.lock().unwrap().as_slice(),
            &[
                (ActivationKind::Evaluated, true),
                (ActivationKind::Reused, true)
            ]
        );
        assert_eq!(
            tracker.builtin.lock().unwrap().as_slice(),
            &[ActivationKind::Evaluated]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let unsupported = source_observation_key(input.clone(), "not/in/catalog");
        let SourcePreparationOutcome::Complete(error) = dice
            .updater()
            .commit()
            .await
            .compute(&unsupported)
            .await
            .unwrap()
        else {
            unreachable!()
        };
        let error = error.as_ref().as_ref().unwrap_err();
        assert_eq!(error.input(), &input);
        assert!(matches!(
            error.source_observation_input(),
            HostRepositorySourceObservationInput::Root(actual) if actual == &input
        ));
        assert_eq!(error.relative_path().as_path(), Path::new("not/in/catalog"));
        assert!(matches!(
            error.kind,
            HostRepositorySourceObservationErrorKind::Builtin(
                BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. }
            )
        ));
        let directory = source_observation_key(input.clone(), "tools");
        let SourcePreparationOutcome::Complete(error) = dice
            .updater()
            .commit()
            .await
            .compute(&directory)
            .await
            .unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            error.as_ref().as_ref().unwrap_err().kind,
            HostRepositorySourceObservationErrorKind::Builtin(
                BuiltinBazelToolsSourceFileError::WrongKind { .. }
            )
        ));
        assert!(HostRepositorySourceObservationKey::equality(
            &outcome, &outcome
        ));
        let mut a = DefaultHasher::new();
        key.hash(&mut a);
        let mut b = DefaultHasher::new();
        source_observation_key(input, "other").hash(&mut b);
        assert_ne!(a.finish(), b.finish());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_observation_builtin_rejects_non_utf8_without_catalog_compute() {
        use std::os::unix::ffi::OsStringExt;

        let input = host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .source_capability(),
        )
        .unwrap();
        let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'x', 0xff]));
        let key = HostRepositorySourceObservationKey::new(
            input,
            host_repository_relative_path(path).unwrap(),
        );
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let SourcePreparationOutcome::Complete(value) =
            dice.updater().commit().await.compute(&key).await.unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            value.as_ref().as_ref().unwrap_err().kind,
            HostRepositorySourceObservationErrorKind::BuiltinPath
        ));
    }

    #[tokio::test]
    async fn source_observation_request_forwards_first_need_and_shares_input() {
        let route = local_route();
        let input = host_repository_source_input(route.source_capability()).unwrap();
        let key = source_observation_key(input.clone(), "BUILD.bazel");
        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            unreachable!()
        };
        let retained_request = request.clone();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
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
        let mut tx = updater.commit().await;
        let need = tx.compute(&key).await.unwrap();
        assert!(!HostRepositorySourceObservationKey::validity(&need));
        assert!(!HostRepositorySourceObservationKey::equality(&need, &need));
        let SourcePreparationOutcome::Need(need) = need else {
            unreachable!()
        };
        assert_eq!(
            need.repository_materializations().values().next().unwrap(),
            request
        );
        assert!(Arc::ptr_eq(request, &retained_request));
        let cloned = key.clone();
        let HostRepositorySourceInputDispositionView::Request(cloned_request) =
            cloned.input.view().disposition()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(request, cloned_request));
        assert!(Arc::ptr_eq(
            key.relative_path.path_arc(),
            cloned.relative_path.path_arc()
        ));
    }

    fn observation_hash(key: &HostRepositorySourceObservationKey) -> u64 {
        let mut state = DefaultHasher::new();
        key.hash(&mut state);
        state.finish()
    }

    #[test]
    fn source_observation_identity_hash_and_errors_are_structural() {
        let route = immutable_route();
        let spec = route.repo_spec().clone();
        let make = |workspace: &str,
                    apparent: &str,
                    canonical: &str,
                    spec: &RepoSpec,
                    policy: HostRepositoryLocalPathPolicy,
                    path: &str| {
            source_observation_key(
                host_repository_source_input(
                    HostRepositorySourceCapability::from_repo_spec(
                        NormalizedAbsolutePath::new(workspace).unwrap(),
                        ApparentRepoName::new(apparent).unwrap(),
                        CanonicalRepoName::new(canonical).unwrap(),
                        spec,
                        policy,
                    )
                    .unwrap(),
                )
                .unwrap(),
                path,
            )
        };
        macro_rules! make_key {
            ($workspace:expr, $apparent:expr, $canonical:expr, $spec:expr, $policy:expr, $path:expr) => {
                make($workspace, $apparent, $canonical, $spec, $policy, $path)
            };
        }
        #[rustfmt::skip]
        let baseline = make_key!("/workspace", "dep_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel");
        let mut changed_spec = spec.clone();
        changed_spec.attributes = Arc::new(SmallMap::from_iter([(
            CompactString::new("urls"),
            OverrideAttributeValue::String("https://example.test/archive.tgz".into()),
        )]));
        #[rustfmt::skip]
        let variants = [
            (make_key!("/other", "dep_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel"), false),
            (make_key!("/workspace", "other_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel"), true),
            (make_key!("/workspace", "dep_alias", "other+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel"), false),
            (make_key!("/workspace", "dep_alias", "dep+", &changed_spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel"), true),
            (make_key!("/workspace", "dep_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::CommandAbsolute, "BUILD.bazel"), true),
            (source_observation_key(host_repository_source_input(local_route().source_capability()).unwrap(), "BUILD.bazel"), true),
            (make_key!("/workspace", "dep_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "other"), true),
        ];
        let HostRepositorySourceInputDispositionView::Request(baseline_request) =
            baseline.input.view().disposition()
        else {
            unreachable!()
        };
        for (variant, same_id) in &variants {
            assert_ne!(&baseline, variant);
            assert_ne!(observation_hash(&baseline), observation_hash(variant));
            if *same_id {
                let HostRepositorySourceInputDispositionView::Request(request) =
                    variant.input.view().disposition()
                else {
                    unreachable!()
                };
                assert_eq!(request.id, baseline_request.id);
            }
            #[rustfmt::skip]
            let restored = make_key!("/workspace", "dep_alias", "dep+", &spec, HostRepositoryLocalPathPolicy::WorkspaceRelative, "BUILD.bazel");
            assert_eq!(restored, baseline);
            assert_eq!(observation_hash(&restored), observation_hash(&baseline));
        }
        #[rustfmt::skip]
        let builtin = source_observation_key(host_repository_source_input(RootRepositoryRoute::builtin_for_test(NormalizedAbsolutePath::new("/workspace").unwrap()).source_capability()).unwrap(), "BUILD.bazel");
        assert_ne!(baseline, builtin);
        assert_ne!(observation_hash(&baseline), observation_hash(&builtin));

        #[rustfmt::skip]
        let integrity = HostRepositorySourceObservationError { input: builtin.input.clone(), relative_path: builtin.relative_path.clone(), kind: HostRepositorySourceObservationErrorKind::Builtin(BuiltinBazelToolsSourceFileError::Integrity { path: "BUILD.bazel".into(), expected_sha256: "expected".into(), actual_sha256: "actual".into() }) };
        #[rustfmt::skip]
        let builtin_compute = HostRepositorySourceObservationError { kind: HostRepositorySourceObservationErrorKind::BuiltinCompute("compute".into()), ..integrity.clone() };
        #[rustfmt::skip]
        let request_observation = HostRepositorySourceObservationError { input: baseline.input.clone(), relative_path: baseline.relative_path.clone(), kind: HostRepositorySourceObservationErrorKind::Request(RepositorySourceFileError::Observation { repo_relative_path: baseline.relative_path.path_arc().clone(), operation: PathObservationOperation::FileBytes, error: PathObservationError::NotALink }) };
        #[rustfmt::skip]
        let request_compute = HostRepositorySourceObservationError { kind: HostRepositorySourceObservationErrorKind::RequestCompute("compute".into()), ..request_observation.clone() };
        assert_ne!(integrity, builtin_compute);
        assert_ne!(request_observation, request_compute);
        #[rustfmt::skip]
        let same_path = Arc::ptr_eq(request_observation.relative_path().path_arc(), baseline.relative_path.path_arc());
        assert!(same_path);
        assert_eq!(
            request_observation.source_observation_input(),
            &baseline.input
        );
    }

    async fn observation_transaction(
        dice: &Arc<Dice>,
        tracker: Arc<SourceObservationTracker>,
        materialization: RepositoryMaterializationResultEpoch,
        observations: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let user_data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        let mut updater = dice.updater_with_data(user_data);
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

    #[tokio::test]
    async fn source_observation_request_local_immutable_errors_recover_and_reuse() {
        let input = host_repository_source_input(local_route().source_capability()).unwrap();
        let key = source_observation_key(input.clone(), "BUILD.bazel");
        let tracker = Arc::new(SourceObservationTracker::default());
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut tx = observation_transaction(
            &dice,
            tracker.clone(),
            material("dep"),
            host_path_epoch(
                PathObservationNamespace::Host,
                "/workspace/dep/BUILD.bazel",
                Some(PathNodeKind::RegularFile),
                Some(b"local"),
            ),
        )
        .await;
        let a = tx.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &a else {
            unreachable!()
        };
        assert!(matches!(
            value.as_ref().as_ref().unwrap().view(),
            HostRepositorySourceObservationView::Request(
                HostRepositorySourceFileValue::Present { bytes, logical_path }
            ) if bytes.as_ref() == b"local"
                && logical_path.as_path() == Path::new("/workspace/dep/BUILD.bazel")
        ));
        assert!(HostRepositorySourceObservationKey::equality(
            &tx.compute(&key).await.unwrap(),
            &a,
        ));
        #[rustfmt::skip]
        let expected_lifecycle = [(ActivationKind::Evaluated, true), (ActivationKind::Reused, true)];
        assert_eq!(
            tracker.observation.lock().unwrap().as_slice(),
            &expected_lifecycle
        );
        assert_eq!(
            *tracker.result.lock().unwrap(),
            vec![ActivationKind::Evaluated]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());

        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                host_path_epoch(
                    PathObservationNamespace::Host,
                    "/workspace/dep/BUILD.bazel",
                    Some(PathNodeKind::Directory),
                    None,
                ),
            )])
            .unwrap();
        tx = updater.commit().await;
        let SourcePreparationOutcome::Complete(error) = tx.compute(&key).await.unwrap() else {
            unreachable!()
        };
        let error = error.as_ref().as_ref().unwrap_err();
        assert_eq!(error.input(), &input);
        assert!(matches!(
            error.source_observation_input(),
            HostRepositorySourceObservationInput::Root(actual) if actual == &input
        ));
        assert!(Arc::ptr_eq(
            error.relative_path().path_arc(),
            key.relative_path.path_arc()
        ));
        assert!(matches!(
            error.kind,
            HostRepositorySourceObservationErrorKind::Request(
                RepositorySourceFileError::WrongKind {
                    actual: PathNodeKind::Directory,
                    ..
                }
            )
        ));

        let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition()
        else {
            unreachable!()
        };
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                RepositoryMaterializationResultEpoch::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    [RepositoryMaterializationEpochEntry {
                        request: request.clone(),
                        result: RepositoryMaterializationResult::SpecError("bad spec".into()),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        tx = updater.commit().await;
        let SourcePreparationOutcome::Complete(error) = tx.compute(&key).await.unwrap() else {
            unreachable!()
        };
        assert!(matches!(
            error.as_ref().as_ref().unwrap_err().kind,
            HostRepositorySourceObservationErrorKind::Request(
                RepositorySourceFileError::Materialization { .. }
            )
        ));

        #[rustfmt::skip]
        let mut restored = observation_transaction(&dice, Arc::new(SourceObservationTracker::default()), material("dep"), host_path_epoch(PathObservationNamespace::Host, "/workspace/dep/BUILD.bazel", Some(PathNodeKind::RegularFile), Some(b"local"))).await;
        #[rustfmt::skip]
        let restored_matches = HostRepositorySourceObservationKey::equality(&restored.compute(&key).await.unwrap(), &a);
        assert!(restored_matches);

        let immutable_input =
            host_repository_source_input(immutable_route().source_capability()).unwrap();
        let immutable_key = source_observation_key(immutable_input, "BUILD.bazel");
        let immutable_instance = PathObservationInstanceId::new(17);
        #[rustfmt::skip]
        let mut immutable_tx = observation_transaction(&dice, Arc::new(SourceObservationTracker::default()), immutable_material("/generation", immutable_instance), host_path_epoch(PathObservationNamespace::Materialization(immutable_instance), "/generation/BUILD.bazel", Some(PathNodeKind::RegularFile), Some(b"immutable"))).await;
        let SourcePreparationOutcome::Complete(value) =
            immutable_tx.compute(&immutable_key).await.unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            value.as_ref().as_ref().unwrap().view(),
            HostRepositorySourceObservationView::Request(
                HostRepositorySourceFileValue::Present { bytes, logical_path }
            ) if bytes.as_ref() == b"immutable"
                && logical_path.as_path() == Path::new("/generation/BUILD.bazel")
        ));
        #[rustfmt::skip]
        let mut absent_tx = observation_transaction(&dice, Arc::new(SourceObservationTracker::default()), immutable_material("/generation", immutable_instance), host_path_epoch(PathObservationNamespace::Materialization(immutable_instance), "/generation/BUILD.bazel", None, None)).await;
        #[rustfmt::skip]
        let is_absent = matches!(absent_tx.compute(&immutable_key).await.unwrap(), SourcePreparationOutcome::Complete(value) if matches!(value.as_ref().as_ref().unwrap().view(), HostRepositorySourceObservationView::Request(HostRepositorySourceFileValue::Absent)));
        assert!(is_absent);
    }

    #[test]
    fn source_observation_production_has_no_legacy_or_second_result_owner() {
        let production = include_str!("source_preparation/repository_source_observation.rs");
        assert_eq!(
            production
                .matches("RepositoryMaterializationResultKey")
                .count(),
            1
        );
        for forbidden in [
            "HostRepositoryPathKey",
            "HostRepositorySourceFileKey",
            "RepositorySourceScope",
            "RootRepositoryRoute",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden edge: {forbidden}"
            );
        }
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
        module_wrong_kind: Option<PathNodeKind>,
        ignore: Option<&[u8]>,
        packages: &[(&str, bool)],
        omitted: &[(&str, &str)],
        directory_markers: &[(&str, &str)],
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        fragment_wrong_kinds: &[(&str, PathNodeKind)],
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
                .or_else(|| module_wrong_kind.map(lstat))
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
        for (relative, kind) in fragment_wrong_kinds {
            observations.insert(
                demand(
                    &format!("{route_path}/{relative}"),
                    PathObservationOperation::Lstat,
                ),
                lstat(*kind),
            );
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
                    None,
                    ignore,
                    packages,
                    omitted,
                    &[],
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
                    None,
                    packages,
                    &[],
                    &[],
                    fragments,
                    fragment_needs,
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
                    None,
                    packages,
                    &[],
                    &[],
                    fragments,
                    fragment_needs,
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
        immutable_material_with_identity(generation_root, observation_instance, "fixed-content")
    }

    fn immutable_material_with_identity(
        generation_root: &str,
        observation_instance: PathObservationInstanceId,
        source_identity: &str,
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
                        source_identity: Arc::from(source_identity),
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
        let r = horizon_occurrence("r", 3);
        let packages = vec![p.package.clone(), q.package.clone(), r.package.clone()];
        let prefix = epoch(&root("dep", "1"), "dep", Some(b""));
        let inspection = direct_local_horizon_inspection_error(
            Ok(DirectLocalModuleInspectionError::InputCompute(
                "input".into(),
            )),
            prefix.dupe(),
        );
        let SourcePreparationOutcome::Complete(Ok(inspection)) = inspection else {
            panic!("inspection semantic must complete")
        };
        assert!(matches!(
            inspection.result.as_ref(),
            Err(DirectLocalIncludePackageHorizonError::Inspection(_))
        ));
        assert_eq!(inspection.observations, prefix);
        let bad_request = NonrootIncludeRequest {
            path: "@bad//:x.MODULE.bazel".into(),
            location: p.location.clone(),
        };
        assert!(matches!(
            parse_direct_local_include_horizon(&local_route(), &[bad_request]),
            Err(DirectLocalIncludePackageHorizonError::BadLabel { .. })
        ));

        let need = SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        });
        let ControlFlow::Break(inspection_need) =
            direct_local_horizon_observed_inspection(SourcePreparationOutcome::Need(need.dupe()))
        else {
            panic!("inspection Need must stop")
        };
        assert!(matches!(inspection_need, SourcePreparationOutcome::Need(_)));
        let outer = ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/workspace/dep/BUILD.bazel").unwrap(),
                    PathObservationOperation::Lstat,
                ),
                result_operation: PathObservationOperation::FileBytes,
            },
        );
        let ControlFlow::Break(inspection_outer) = direct_local_horizon_observed_inspection(
            SourcePreparationOutcome::Complete(Err(outer.dupe())),
        ) else {
            panic!("inspection outer must stop")
        };
        assert!(
            matches!(&inspection_outer, SourcePreparationOutcome::Complete(Err(error)) if error == &outer)
        );

        let child_epoch = |slot| {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(format!("/workspace/dep/{slot}/BUILD.bazel")).unwrap(),
                PathObservationOperation::Lstat,
            );
            PathObservationEpoch::from_shared([(
                demand,
                Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
            )])
            .unwrap()
        };
        let success = |slot| {
            Ok(SourcePreparationOutcome::Complete(Ok((
                Arc::new(Ok(ExternalRepositoryPackageLookup::Package(
                    HostBuildFileName::BuildDotBazel,
                ))),
                child_epoch(slot),
            ))))
        };
        let batch = |slot, terminal: DirectLocalIncludePackageLookupOutcome| {
            packages
                .iter()
                .enumerate()
                .map(|(index, package)| {
                    (
                        package.clone(),
                        if index == slot {
                            terminal.clone()
                        } else {
                            success(index)
                        },
                    )
                })
                .collect()
        };
        let reduce = |outcomes| {
            finish_direct_local_include_package_horizon_observed(
                local_route(),
                vec![p.clone(), q.clone(), r.clone()],
                &packages,
                outcomes,
                prefix.dupe(),
            )
        };
        for slot in 0..3 {
            let compute = reduce(batch(slot, Err("lookup compute".into())));
            let observed = match &compute {
                SourcePreparationOutcome::Complete(Ok(observed)) => observed,
                _ => panic!("lookup compute is semantic"),
            };
            assert!(matches!(
                observed.result.as_ref(),
                Err(DirectLocalIncludePackageHorizonError::Package {
                    failure: DirectLocalIncludePackageFailure::LookupCompute { .. },
                    ..
                })
            ));
            assert_eq!(
                observed.observations.observations().len(),
                prefix.observations().len() + slot
            );
            assert!(DirectLocalIncludePackageHorizonObservationKey::validity(
                &compute
            ));
            assert!(DirectLocalIncludePackageHorizonObservationKey::equality(
                &compute, &compute
            ));
            assert!(matches!(
                reduce(batch(slot, Ok(SourcePreparationOutcome::Need(need.dupe())))),
                SourcePreparationOutcome::Need(_)
            ));
            assert!(
                matches!(reduce(batch(slot, Ok(SourcePreparationOutcome::Complete(Err(outer.dupe()))))), SourcePreparationOutcome::Complete(Err(error)) if error == outer)
            );
        }
        for (result, kind, sourced) in [
            (
                Ok(ExternalRepositoryPackageLookup::InvalidPackageName {
                    message: "invalid".into(),
                }),
                "InvalidPackageName",
                false,
            ),
            (
                Err(ExternalRepositoryPackageLookupError::Path(
                    RepositorySourceFileError::Cycle {
                        repo_relative_path: Arc::new(PathBuf::from("p/BUILD.bazel")),
                    },
                )),
                "Lookup",
                true,
            ),
        ] {
            let value = reduce(batch(
                1,
                Ok(SourcePreparationOutcome::Complete(Ok((
                    Arc::new(result),
                    child_epoch(1),
                )))),
            ));
            let observed = match &value {
                SourcePreparationOutcome::Complete(Ok(observed)) => observed,
                _ => panic!("lookup semantic must complete"),
            };
            let observed_result = observed.result.dupe();
            let error = observed_result.as_ref().as_ref().unwrap_err();
            assert!(format!("{error:?}").contains(kind));
            assert_eq!(std::error::Error::source(error).is_some(), sourced);
            let SourcePreparationOutcome::Complete(legacy) =
                project_legacy_direct_local_include_horizon(value)
            else {
                panic!("legacy projection must complete")
            };
            assert!(Arc::ptr_eq(&legacy, &observed_result));
        }
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
        assert!(key_owner.contains("drive_direct_local_include_horizon_key("));
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
        let builtin = HostDiscoveredModuleKey::try_new(
            workspace.dupe(),
            NonrootModuleKey::new("bazel_tools", ""),
        )
        .unwrap();
        let registry =
            HostDiscoveredModuleKey::try_new(workspace, NonrootModuleKey::new("dep", "1.0"))
                .unwrap();
        assert_ne!(builtin, registry);
        let normalized = HostDiscoveredModuleKey::try_new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", "1.0+client"),
        )
        .unwrap();
        assert!(normalized.to_string().ends_with(":dep@1.0"));
        assert!(
            HostDiscoveredModuleKey::try_new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                NonrootModuleKey::new("dep", "18446744073709551616"),
            )
            .is_err()
        );
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
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("RootModuleFilesKey"));
        let owner = source
            .split("async fn discover_nonregistry")
            .nth(1)
            .unwrap()
            .split("impl fmt::Display for RepositoryMaterializationKey")
            .next()
            .unwrap();
        for required in [
            "HostEffectiveModuleOverrideKey",
            "BuiltinBazelToolsModuleKey",
            "ModuleSourcePreparationKey",
            "HostNonregistryModuleClosureKey",
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
        effective: Mutex<Vec<ActivationKind>>,
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
            } else if key
                .downcast_ref::<HostEffectiveModuleOverrideKey>()
                .is_some()
            {
                self.effective.lock().unwrap().push(activation.kind());
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
            .compute(&HostDiscoveredModuleKey::try_new(workspace, module).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn host_discovered_module_dice_lifecycle_and_builtin_override_bypass() {
        let a = "https://a.invalid/modules/dep/1.0/MODULE.bazel";
        let b = "https://b.invalid/modules/dep/1.0/MODULE.bazel";
        let semantic_b_url = "https://a.invalid/modules/dep/2.0/MODULE.bazel";
        let source: Arc<[u8]> =
            Arc::from(&b"module(name = \"dep\", version = \"1.0+source\")\n"[..]);
        let semantic_b_source: Arc<[u8]> =
            Arc::from(&b"module(name = \"dep\", version = \"2.0\")\n"[..]);
        let mut builder = Dice::builder();
        crate::install_registry_io(
            &mut builder,
            Arc::new(HostDiscoveryRegistryIo(
                [
                    (a.to_owned(), source.clone()),
                    (b.to_owned(), source),
                    (semantic_b_url.to_owned(), semantic_b_source),
                ]
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

        let root = "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0+root\")\n";
        let equivalent_root =
            "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0+other\")\n";
        let first_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            1,
            NonrootModuleKey::new("dep", "1.0+request-a"),
        )
        .await;
        let selected_b = compute_host_discovered(
            &dice,
            tracker.clone(),
            equivalent_root,
            &["https://b.invalid"],
            2,
            NonrootModuleKey::new("dep", "1.0+request-b"),
        )
        .await;
        let second_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            3,
            NonrootModuleKey::new("dep", "1.0+request-restored"),
        )
        .await;
        let warm_a = compute_host_discovered(
            &dice,
            tracker.clone(),
            equivalent_root,
            &["https://a.invalid"],
            3,
            NonrootModuleKey::new("dep", "1.0+request-warm"),
        )
        .await;
        let semantic_b = compute_host_discovered(
            &dice,
            tracker.clone(),
            "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"2.0\")\n",
            &["https://a.invalid"],
            4,
            NonrootModuleKey::new("dep", "2.0"),
        )
        .await;
        let semantic_a_restored = compute_host_discovered(
            &dice,
            tracker.clone(),
            root,
            &["https://a.invalid"],
            5,
            NonrootModuleKey::new("dep", "1.0+final"),
        )
        .await;
        assert!(HostDiscoveredModuleKey::equality(&first_a, &second_a));
        assert!(HostDiscoveredModuleKey::equality(&second_a, &warm_a));
        assert!(!HostDiscoveredModuleKey::equality(&first_a, &selected_b));
        assert!(!HostDiscoveredModuleKey::equality(&first_a, &semantic_b));
        assert!(HostDiscoveredModuleKey::equality(
            &first_a,
            &semantic_a_restored
        ));
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
    fn inject_host_effective_inputs(
        updater: &mut dice::DiceTransactionUpdater,
        root_source: &str,
        mode: crate::LockfileMode,
        override_values: &[&str],
    ) {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
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
                    workspace: workspace.as_path().to_owned(),
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
            updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                workspace.as_path(),
                override_values.iter().copied(),
            )
            .unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            mode,
        )
        .unwrap();
    }

    async fn compute_host_effective(
        dice: &Arc<Dice>,
        root_source: &str,
        module_name: &str,
        override_values: &[&str],
    ) -> <HostEffectiveModuleOverrideKey as Key>::Value {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let mut updater = dice.updater();
        inject_host_effective_inputs(
            &mut updater,
            root_source,
            crate::LockfileMode::Update,
            override_values,
        );
        updater
            .commit()
            .await
            .compute(&HostEffectiveModuleOverrideKey::new(
                workspace,
                module_name.into(),
            ))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn effective_override_and_command_discovery_are_single_owned() {
        use crate::module_eval::HostEffectiveModuleOverride;

        let root = "module(name = \"root\")\nlocal_path_override(module_name = \"dep\", path = \"root-dep\")\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root_a = compute_host_effective(&dice, root, "dep", &[]).await;
        let command_b = compute_host_effective(&dice, root, "dep", &["dep=/workspace/dep"]).await;
        let root_a_restored = compute_host_effective(&dice, root, "dep", &[]).await;
        assert_eq!(root_a, root_a_restored);
        let path_a = compute_host_effective(&dice, root, "dep", &["dep=/workspace/dep-a"]).await;
        let path_b = compute_host_effective(&dice, root, "dep", &["dep=/workspace/dep-b"]).await;
        let path_a_restored =
            compute_host_effective(&dice, root, "dep", &["dep=/workspace/dep-a"]).await;
        assert_ne!(path_a, path_b);
        assert_eq!(path_a, path_a_restored);
        assert!(matches!(
            root_a.as_ref(),
            Ok(HostEffectiveModuleOverride::Root {
                override_: RootModuleOverride::NonRegistry(_),
            })
        ));
        assert_eq!(
            local_path_policy(root_a.as_ref().as_ref().unwrap()),
            HostRepositoryLocalPathPolicy::WorkspaceRelative
        );
        let command = command_b.as_ref().as_ref().unwrap();
        let HostEffectiveModuleOverride::Command { path, override_ } = command else {
            panic!("command must win over the root declaration")
        };
        assert_eq!(path.as_path(), Path::new("/workspace/dep"));
        let RootModuleOverride::NonRegistry(repo_spec) = override_ else {
            panic!("command paths project to local nonregistry specs")
        };
        assert_eq!(repo_spec.rule_id.rule_name, "local_repository");
        assert_eq!(
            repo_spec.rule_id.bzl_file,
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl").unwrap()
        );
        assert!(matches!(
            repo_spec.attributes.get("path"),
            Some(OverrideAttributeValue::String(path)) if path == "/workspace/dep"
        ));
        assert_eq!(
            local_path_policy(command),
            HostRepositoryLocalPathPolicy::CommandAbsolute
        );
        let same_spec_root = HostEffectiveModuleOverride::Root {
            override_: override_.clone(),
        };
        assert_eq!(same_spec_root.override_(), command.override_());
        assert_ne!(
            local_path_policy(&same_spec_root),
            local_path_policy(command)
        );
        assert_eq!(
            local_path_policy(&same_spec_root),
            HostRepositoryLocalPathPolicy::WorkspaceRelative
        );
        let root_error = compute_host_effective(
            &dice,
            "module(name = \"root\")\n",
            "dep",
            &["root=/workspace/replacement"],
        )
        .await;
        assert!(matches!(
            root_error.as_ref(),
            Err(crate::module_eval::HostEffectiveModuleOverrideError::RootModuleOverride {
                module_name,
            }) if module_name == "root"
        ));

        let tracker = Arc::new(HostDiscoveryTracker::default());
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let module_source = b"module(name = \"bazel_tools\")\n";
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(
                            "module(name = \"root\")\n".to_owned(),
                        )),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
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
            crate::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                workspace.as_path(),
                ["bazel_tools=/workspace/tools"],
            )
            .unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Update,
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
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let key = HostDiscoveredModuleKey::try_new(
            workspace.dupe(),
            NonrootModuleKey::new("bazel_tools", ""),
        )
        .unwrap();
        let mut observations = Vec::new();
        for _ in 0..8 {
            let outcome = transaction.compute(&key).await.unwrap();
            match outcome {
                SourcePreparationOutcome::Complete(value) => {
                    let discovered = value.as_ref().as_ref().unwrap();
                    assert_eq!(discovered.module.base.declared_name, "bazel_tools");
                    assert!(matches!(
                        &discovered.provenance,
                        HostDiscoveredModuleProvenance::NonRegistry { closure }
                            if closure.root.bytes.as_ref() == module_source
                    ));
                    assert!(tracker.builtin.lock().unwrap().is_empty());
                    let warm = transaction.compute(&key).await.unwrap();
                    assert!(HostDiscoveredModuleKey::equality(
                        &SourcePreparationOutcome::Complete(value.clone()),
                        &warm,
                    ));
                    assert!(
                        tracker
                            .host
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|(kind, _)| *kind == ActivationKind::Reused)
                    );
                    assert!(
                        tracker
                            .effective
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|kind| *kind == ActivationKind::Reused)
                    );
                    return;
                }
                SourcePreparationOutcome::Need(needs) => {
                    let requests = needs
                        .repository_materializations()
                        .values()
                        .cloned()
                        .collect::<Vec<_>>();
                    if let Some(demands) = needs.path_observations() {
                        observations.extend(demands.demands().iter().map(|demand| {
                            let result = match demand.operation() {
                                PathObservationOperation::Lstat => {
                                    let kind = if demand.path().as_path().ends_with("MODULE.bazel")
                                    {
                                        PathNodeKind::RegularFile
                                    } else {
                                        PathNodeKind::Directory
                                    };
                                    PathObservationResult::Lstat(PathOperationResult::Present(
                                        PathLstat::new(kind, 1, 1, 1, 1, 0o644),
                                    ))
                                }
                                PathObservationOperation::FileBytes => {
                                    PathObservationResult::FileBytes(PathOperationResult::Present(
                                        Arc::from(module_source.as_slice()),
                                    ))
                                }
                                operation => {
                                    panic!("unexpected command source demand: {operation:?}")
                                }
                            };
                            (demand.dupe(), result)
                        }));
                    }
                    let mut next = transaction.into_updater();
                    if !requests.is_empty() {
                        next.changed_to(vec![(
                            RepositoryMaterializationResultEpochKey {
                                workspace: workspace.dupe(),
                            },
                            RepositoryMaterializationResultEpoch::new(
                                workspace.dupe(),
                                requests.into_iter().map(|request| {
                                    RepositoryMaterializationEpochEntry {
                                        request,
                                        result: RepositoryMaterializationResult::Success(
                                            RepositoryMaterializationSuccess::Local,
                                        ),
                                    }
                                }),
                            )
                            .unwrap(),
                        )])
                        .unwrap();
                    }
                    if !observations.is_empty() {
                        next.changed_to(vec![(
                            PathObservationEpochKey,
                            PathObservationEpoch::new(observations.clone()).unwrap(),
                        )])
                        .unwrap();
                    }
                    transaction = next.commit().await;
                }
            }
        }
        panic!("command-overridden bazel_tools did not complete")
    }
    #[tokio::test]
    async fn command_source_preserves_missing_and_wrong_kind_terminals() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = "module(name = \"root\")\n";
        let effective = compute_host_effective(&dice, root, "dep", &["dep=/workspace/dep"]).await;
        assert!(matches!(
            effective.as_ref(),
            Ok(crate::module_eval::HostEffectiveModuleOverride::Command { .. })
        ));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let request = transaction
            .compute(&RepositoryMaterializationRequestKey {
                workspace: workspace.as_path().to_owned(),
                module_name: "dep".into(),
            })
            .await
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .clone();
        assert!(matches!(
            &request.kind,
            RepositoryMaterializationKind::Local { logical_root }
                if logical_root.as_path() == Path::new("/workspace/dep")
        ));
        let source_key = RepositorySourceFileKey {
            workspace: workspace.as_path().to_owned(),
            module_name: "dep".into(),
            repo_relative_path: "MODULE.bazel".into(),
        };
        let materialization = RepositoryMaterializationResultEpoch::new(
            workspace.dupe(),
            [RepositoryMaterializationEpochEntry {
                request: Arc::new(request),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap();
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                materialization.clone(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    root,
                    PathObservationNamespace::Host,
                    "/workspace/dep",
                    None,
                    None,
                    None,
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    901,
                ),
            )])
            .unwrap();
        transaction = updater.commit().await;
        assert!(matches!(
            transaction.compute(&source_key).await.unwrap(),
            SourcePreparationOutcome::Complete(Ok(RepositorySourceFileValue::Absent))
        ));
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey { workspace },
                materialization,
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    root,
                    PathObservationNamespace::Host,
                    "/workspace/dep",
                    None,
                    None,
                    Some(PathNodeKind::Directory),
                    None,
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    902,
                ),
            )])
            .unwrap();
        assert!(matches!(
            updater.commit().await.compute(&source_key).await.unwrap(),
            SourcePreparationOutcome::Complete(Err(RepositorySourceFileError::WrongKind {
                actual: PathNodeKind::Directory,
                ..
            }))
        ));
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
                    None,
                    ignore,
                    &[("pkg", false)],
                    &omitted,
                    &directories,
                    &fragments,
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
                    None,
                    &[("pkg", false)],
                    &[],
                    &[],
                    &[("pkg/BUILD.bazel", Some(b"exports_files([])\n"))],
                    &[],
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
    }

    async fn host_nonregistry_transaction(
        dice: &Arc<Dice>,
        root_module: Option<&[u8]>,
        root_wrong_kind: Option<PathNodeKind>,
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        immutable: Option<(&str, u64, &str)>,
        tracker: Option<Arc<NonregistryPreflightTracker>>,
        fragment_wrong_kind: Option<(&str, PathNodeKind)>,
        capture_events: bool,
    ) -> dice::DiceTransaction {
        let root_source = if immutable.is_some() {
            "bazel_dep(name = 'dep', version = '1')\narchive_override(module_name = 'dep')\n"
        } else {
            "bazel_dep(name = 'dep', version = '1')\nlocal_path_override(module_name = 'dep', path = 'dep')\n"
        };
        let (namespace, route_path, materialization) = match immutable {
            Some((generation, instance, source_identity)) => {
                let instance = PathObservationInstanceId::new(instance);
                (
                    PathObservationNamespace::Materialization(instance),
                    generation,
                    immutable_material_with_identity(generation, instance, source_identity),
                )
            }
            None => (
                PathObservationNamespace::Host,
                "/workspace/dep",
                material("dep"),
            ),
        };
        let mut data = UserComputationData {
            activation_tracker: tracker.map(|value| value as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture_events {
            data.data.set(CaptureEvaluationEvents);
        }
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
        let mut observed_fragments = vec![
            ("pkg/BUILD.bazel", Some(&b""[..])),
            ("other/BUILD.bazel", Some(&b""[..])),
        ];
        observed_fragments.extend_from_slice(fragments);
        let fragment_wrong_kinds = fragment_wrong_kind.into_iter().collect::<Vec<_>>();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                horizon_epoch(
                    root_source,
                    namespace,
                    route_path,
                    root_module,
                    None,
                    root_wrong_kind,
                    None,
                    &[("pkg", true), ("other", true)],
                    &[],
                    &[],
                    &observed_fragments,
                    fragment_needs,
                    &fragment_wrong_kinds,
                    variant,
                ),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                materialization,
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
    async fn host_nonregistry_closure_compute(
        dice: &Arc<Dice>,
        root_module: Option<&[u8]>,
        root_wrong_kind: Option<PathNodeKind>,
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        immutable: Option<(&str, u64, &str)>,
        tracker: Option<Arc<NonregistryPreflightTracker>>,
        fragment_wrong_kind: Option<(&str, PathNodeKind)>,
    ) -> HostNonregistryModuleClosureValue {
        host_nonregistry_transaction(
            dice,
            root_module,
            root_wrong_kind,
            fragments,
            fragment_needs,
            variant,
            immutable,
            tracker,
            fragment_wrong_kind,
            false,
        )
        .await
        .compute(&HostNonregistryModuleClosureKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            NonrootModuleKey::new("dep", "1"),
        ))
        .await
        .unwrap()
    }

    async fn host_nonregistry_discovered_compute(
        dice: &Arc<Dice>,
        root_module: Option<&[u8]>,
        fragments: &[(&str, Option<&[u8]>)],
        fragment_needs: &[&str],
        variant: i64,
        immutable: Option<(&str, u64, &str)>,
        tracker: Option<Arc<NonregistryPreflightTracker>>,
        module: NonrootModuleKey,
        capture_events: bool,
    ) -> <HostDiscoveredModuleKey as Key>::Value {
        host_nonregistry_transaction(
            dice,
            root_module,
            None,
            fragments,
            fragment_needs,
            variant,
            immutable,
            tracker,
            None,
            capture_events,
        )
        .await
        .compute(
            &HostDiscoveredModuleKey::try_new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                module,
            )
            .unwrap(),
        )
        .await
        .unwrap()
    }

    fn supported_host_closure(
        value: &HostNonregistryModuleClosureValue,
    ) -> &HostNonregistryPreparedClosure {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("expected complete closure")
        };
        let HostNonregistryModuleClosure::Supported(closure) = value.as_ref().as_ref().unwrap()
        else {
            panic!("expected supported closure")
        };
        closure
    }

    #[tokio::test]
    async fn host_discovered_nonregistry_composes_empty_key_order_and_lifecycle() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let root = b"module(name='dep',version='declared')\ninclude('//pkg:a.MODULE.bazel')\ninclude('//pkg:a.MODULE.bazel')\n";
        let fragment_a = [("pkg/a.MODULE.bazel", Some(&b""[..]))];
        let compute = |root, fragments, variant, immutable, tracker, module| {
            host_nonregistry_discovered_compute(
                &dice,
                root,
                fragments,
                &[],
                variant,
                immutable,
                tracker,
                module,
                true,
            )
        };
        let a = compute(
            Some(root),
            &fragment_a,
            90,
            None,
            Some(tracker.clone()),
            NonrootModuleKey::new("dep", ""),
        )
        .await;
        let b_fragments = [(
            "pkg/a.MODULE.bazel",
            Some(
                &b"# changed
"[..],
            ),
        )];
        let b = compute(
            Some(root),
            &b_fragments,
            91,
            None,
            None,
            NonrootModuleKey::new("dep", ""),
        )
        .await;
        let category_b = compute(
            Some(root),
            &fragment_a,
            92,
            Some(("/generation-discovery", 92, "fixed-content")),
            None,
            NonrootModuleKey::new("dep", ""),
        )
        .await;
        let restored = compute(
            Some(root),
            &fragment_a,
            93,
            None,
            Some(tracker.clone()),
            NonrootModuleKey::new("dep", ""),
        )
        .await;
        let warm = compute(
            Some(root),
            &fragment_a,
            93,
            None,
            Some(tracker.clone()),
            NonrootModuleKey::new("dep", ""),
        )
        .await;
        assert!(!HostDiscoveredModuleKey::equality(&a, &b));
        assert!(!HostDiscoveredModuleKey::equality(&a, &category_b));
        assert!(HostDiscoveredModuleKey::equality(&a, &restored));
        assert!(HostDiscoveredModuleKey::equality(&restored, &warm));
        let SourcePreparationOutcome::Complete(a_value) = &a else {
            panic!("expected complete discovered module")
        };
        let HostDiscoveredModuleProvenance::NonRegistry { closure } =
            &a_value.as_ref().as_ref().unwrap().provenance
        else {
            panic!("expected nonregistry provenance")
        };
        assert_eq!(
            closure.local_path_policy(),
            HostRepositoryLocalPathPolicy::WorkspaceRelative
        );
        let mut command_identity = closure.clone();
        command_identity.local_path_policy = HostRepositoryLocalPathPolicy::CommandAbsolute;
        assert_ne!(closure, &command_identity);
        command_identity.local_path_policy = HostRepositoryLocalPathPolicy::WorkspaceRelative;
        assert_eq!(closure, &command_identity);
        assert!(
            matches!(a, SourcePreparationOutcome::Complete(value) if matches!(
                value.as_ref(), Ok(HostDiscoveredModule {
                    module,
                    provenance: HostDiscoveredModuleProvenance::NonRegistry { closure },
                }) if module.base.expected_key.version.is_empty()
                    && module.base.declared_version == "declared"
                    && closure.fragments.len() == 2
            ))
        );
        let activations = tracker.discovered.lock().unwrap();
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

    #[tokio::test]
    async fn host_discovered_nonregistry_guards_need_cycle_and_recovers() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let root = b"module(name='dep',version='declared')\ninclude('//pkg:a.MODULE.bazel')\n";
        let nonempty = host_nonregistry_discovered_compute(
            &dice,
            Some(root),
            &[],
            &[],
            94,
            None,
            None,
            NonrootModuleKey::new("dep", "requested"),
            true,
        )
        .await;
        assert!(
            matches!(nonempty, SourcePreparationOutcome::Complete(value) if matches!(value.as_ref(), Err(HostDiscoveredModuleError::InvalidNonRegistryVersion { .. })))
        );
        let need = host_nonregistry_discovered_compute(
            &dice,
            Some(root),
            &[],
            &["pkg/a.MODULE.bazel"],
            95,
            None,
            None,
            NonrootModuleKey::new("dep", ""),
            true,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let cycle_fragments = [(
            "pkg/a.MODULE.bazel",
            Some(&b"include('//pkg:a.MODULE.bazel')\n"[..]),
        )];
        let cycle = host_nonregistry_discovered_compute(
            &dice,
            Some(root),
            &cycle_fragments,
            &[],
            96,
            None,
            None,
            NonrootModuleKey::new("dep", ""),
            true,
        )
        .await;
        assert!(
            matches!(cycle, SourcePreparationOutcome::Complete(value) if matches!(value.as_ref(), Err(HostDiscoveredModuleError::NonRegistryCycle { closure, .. }) if closure.fragments.len() == 2))
        );
        let invalid = host_nonregistry_discovered_compute(
            &dice,
            Some(b"module(name='wrong')\n"),
            &[],
            &[],
            97,
            None,
            None,
            NonrootModuleKey::new("dep", ""),
            true,
        )
        .await;
        assert!(
            matches!(invalid, SourcePreparationOutcome::Complete(value) if matches!(value.as_ref(), Err(HostDiscoveredModuleError::Evaluation(DirectNonregistryEvaluationError::DeclaredNameMismatch { .. }))))
        );
        let recovered = host_nonregistry_discovered_compute(
            &dice,
            Some(b"module(name='dep',version='1')\n"),
            &[],
            &[],
            98,
            None,
            None,
            NonrootModuleKey::new("dep", ""),
            true,
        )
        .await;
        assert!(
            matches!(recovered, SourcePreparationOutcome::Complete(value) if matches!(value.as_ref(), Ok(HostDiscoveredModule { provenance: HostDiscoveredModuleProvenance::NonRegistry { .. }, .. })))
        );
    }
    #[tokio::test]
    async fn host_nonregistry_module_closure_local_aba_order_need_and_reuse() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let tracker = Arc::new(NonregistryPreflightTracker::default());
        let root = b"module(name = 'dep', version = '1')\ninclude('//pkg:a.MODULE.bazel')\ninclude('//pkg:a.MODULE.bazel')\n";
        let a_fragments = [
            (
                "pkg/a.MODULE.bazel",
                Some(&b"include('//other:b.MODULE.bazel')\n"[..]),
            ),
            (
                "other/b.MODULE.bazel",
                Some(&b"bazel_dep(name='b',version='1')\n"[..]),
            ),
        ];
        let a = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &a_fragments,
            &[],
            60,
            None,
            Some(tracker.clone()),
            None,
        )
        .await;
        let b = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &[
                ("pkg/a.MODULE.bazel", Some(&b"include('//other:b.MODULE.bazel')\nbazel_dep(name='changed',version='1')\n"[..])),
                a_fragments[1],
            ],
            &[],
            61,
            None,
            None,
            None,
        )
        .await;
        let need = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &[],
            &["pkg/a.MODULE.bazel"],
            62,
            None,
            Some(tracker.clone()),
            None,
        )
        .await;
        let restored = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &a_fragments,
            &[],
            63,
            None,
            Some(tracker.clone()),
            None,
        )
        .await;
        let warm = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &a_fragments,
            &[],
            63,
            None,
            Some(tracker.clone()),
            None,
        )
        .await;
        let category_b = host_nonregistry_closure_compute(
            &dice,
            Some(root),
            None,
            &a_fragments,
            &[],
            64,
            Some(("/generation-category", 64, "fixed-content")),
            None,
            None,
        )
        .await;
        let multi_need = host_nonregistry_closure_compute(
            &dice,
            Some(b"module(name='dep',version='1')\ninclude('//pkg:a.MODULE.bazel')\ninclude('//other:b.MODULE.bazel')\n"),
            None,
            &[],
            &["pkg/a.MODULE.bazel", "other/b.MODULE.bazel"],
            65,
            None,
            None,
            None,
        )
        .await;
        assert!(!HostNonregistryModuleClosureKey::equality(&a, &category_b));
        assert!(matches!(
            multi_need,
            SourcePreparationOutcome::Need(need)
                if need.path_observations().unwrap().demands().len() == 2
        ));
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(HostNonregistryModuleClosureKey::equality(&a, &restored));
        assert!(HostNonregistryModuleClosureKey::equality(&restored, &warm));
        let closure = supported_host_closure(&a);
        assert_eq!(closure.fragments.len(), 4);
        assert_eq!(
            closure
                .fragments
                .iter()
                .map(|fragment| fragment.occurrence.target.as_str())
                .collect::<Vec<_>>(),
            [
                "a.MODULE.bazel",
                "a.MODULE.bazel",
                "b.MODULE.bazel",
                "b.MODULE.bazel"
            ]
        );
        assert!(!HostNonregistryModuleClosureKey::equality(&a, &b));
        assert!(
            tracker
                .closure
                .lock()
                .unwrap()
                .iter()
                .any(|kind| *kind == ActivationKind::Reused)
        );
        assert!(tracker.observed.lock().unwrap().is_empty());

        let changed = host_nonregistry_closure_compute(
            &dice,
            Some(b"module(name='dep',version='1')\n"),
            None,
            &[],
            &[],
            66,
            None,
            None,
            None,
        )
        .await;
        assert!(!HostNonregistryModuleClosureKey::equality(&a, &changed));
    }

    #[tokio::test]
    async fn host_nonregistry_module_closure_immutable_cycle_and_root_failures() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let cycle_root = b"module(name='dep',version='1')\ninclude('//pkg:a.MODULE.bazel')\n";
        let cycle_fragment = b"include('//pkg:a.MODULE.bazel')\n";
        let cycle = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", Some(cycle_fragment))],
            &[],
            70,
            Some(("/generation-a", 70, "fixed-content")),
            None,
            None,
        )
        .await;
        assert!(matches!(
            cycle,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(value.as_ref(), Ok(HostNonregistryModuleClosure::UnsupportedCycle {
                    closure,
                    ..
                }) if closure.fragments.len() == 2)
        ));

        let generation_b = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", Some(cycle_fragment))],
            &[],
            71,
            Some(("/generation-b", 71, "fixed-content")),
            None,
            None,
        )
        .await;
        let generation_a = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", Some(cycle_fragment))],
            &[],
            72,
            Some(("/generation-a", 72, "fixed-content")),
            None,
            None,
        )
        .await;
        assert!(HostNonregistryModuleClosureKey::equality(
            &cycle,
            &generation_b
        ));
        assert!(HostNonregistryModuleClosureKey::equality(
            &cycle,
            &generation_a
        ));

        let content_b = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[(
                "pkg/a.MODULE.bazel",
                Some(&b"include('//pkg:a.MODULE.bazel')\nbazel_dep(name='changed',version='1')\n"[..]),
            )],
            &[],
            73,
            Some(("/generation-c", 73, "fixed-content")),
            None,
            None,
        )
        .await;
        let identity_b = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", Some(cycle_fragment))],
            &[],
            74,
            Some(("/generation-d", 74, "different-content-id")),
            None,
            None,
        )
        .await;
        assert!(!HostNonregistryModuleClosureKey::equality(
            &cycle, &content_b
        ));
        assert!(!HostNonregistryModuleClosureKey::equality(
            &cycle,
            &identity_b
        ));
        let fragment_absent = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", None)],
            &[],
            75,
            Some(("/generation-e", 75, "fixed-content")),
            None,
            None,
        )
        .await;
        let fragment_wrong = host_nonregistry_closure_compute(
            &dice,
            Some(cycle_root),
            None,
            &[("pkg/a.MODULE.bazel", None)],
            &[],
            76,
            Some(("/generation-f", 76, "fixed-content")),
            None,
            Some(("pkg/a.MODULE.bazel", PathNodeKind::Directory)),
        )
        .await;
        assert!(matches!(
            fragment_absent,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostNonregistryModuleClosureError::Fragment {
                    failure: DirectLocalIncludeFragmentFailure::Absent,
                    ..
                }))
        ));
        assert!(matches!(
            fragment_wrong,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostNonregistryModuleClosureError::Fragment {
                    failure: DirectLocalIncludeFragmentFailure::Source(
                        RepositorySourceFileError::WrongKind { .. }
                    ),
                    ..
                }))
        ));
        let absent =
            host_nonregistry_closure_compute(&dice, None, None, &[], &[], 77, None, None, None)
                .await;
        let wrong_kind = host_nonregistry_closure_compute(
            &dice,
            None,
            Some(PathNodeKind::Directory),
            &[],
            &[],
            78,
            None,
            None,
            None,
        )
        .await;
        assert!(matches!(
            absent,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostNonregistryModuleClosureError::RootAbsent))
        ));
        assert!(matches!(
            wrong_kind,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostNonregistryModuleClosureError::RootSource(
                    RepositorySourceFileError::WrongKind { .. }
                )))
        ));
    }

    #[test]
    fn repository_relative_path_owns_the_exact_checked_shape() {
        use std::hash::Hash;
        use std::hash::Hasher;

        let valid = [
            "dep",
            "nested/BUILD.bazel",
            "two/ordered/components",
            "dep/./file",
            "dep//file",
            "dep/",
        ];
        for raw in valid {
            let path = PathBuf::from(raw);
            assert_eq!(checked_relative_path(&path).unwrap(), path.as_path());
            let owned = crate::host_repository_relative_path(path.clone()).unwrap();
            assert_eq!(owned.as_path(), path.as_path());
            assert_eq!(owned.path_arc().as_ref(), &path);
            let cloned = owned.clone();
            assert!(Arc::ptr_eq(owned.path_arc(), cloned.path_arc()));
        }
        for path in [
            PathBuf::new(),
            PathBuf::from("."),
            PathBuf::from("./dep"),
            PathBuf::from(".."),
            PathBuf::from("dep/../other"),
            PathBuf::from("/"),
            PathBuf::from("/absolute"),
        ] {
            assert!(checked_relative_path(&path).is_err());
            let error = crate::host_repository_relative_path(path.clone()).unwrap_err();
            assert_eq!(error.requested_path(), path.as_path());
            assert!(error.to_string().contains(&path.display().to_string()));
        }
        let a = crate::host_repository_relative_path(PathBuf::from("a/b")).unwrap();
        let b = crate::host_repository_relative_path(PathBuf::from("a/c")).unwrap();
        let restored = crate::host_repository_relative_path(PathBuf::from("a/b")).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, restored);
        let hash = |value: &HostRepositoryRelativePath| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&a), hash(&restored));
        assert_ne!(hash(&a), hash(&b));
        let source = include_str!("source_preparation.rs");
        let constructor = source
            .split("pub fn host_repository_relative_path")
            .nth(1)
            .unwrap()
            .split("pub fn source_identity")
            .next()
            .unwrap();
        assert_eq!(constructor.matches("checked_relative_path(").count(), 1);
        for forbidden in [
            ".compute(",
            "impl Key",
            "std::fs",
            "Materialization",
            "SourceFile",
        ] {
            assert!(
                !constructor.contains(forbidden),
                "forbidden edge: {forbidden}"
            );
        }
    }

    #[test]
    fn observed_preparation_projection_and_complete_algebra_are_exact() {
        let semantic = Arc::new(Err(DirectLocalModulePreparationError::InspectionCompute {
            message: "inspection".into(),
        }));
        let carrier =
            SourcePreparationOutcome::Complete(Ok(ObservedDirectLocalModulePreparation {
                result: semantic.dupe(),
                observations: PathObservationEpoch::empty(),
            }));
        let SourcePreparationOutcome::Complete(projected) =
            project_legacy_direct_local_preparation(carrier)
        else {
            panic!("legacy projection must complete")
        };
        assert!(Arc::ptr_eq(&semantic, &projected));
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::root_module_bootstrap(
            RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        ));
        assert!(!DirectLocalModulePreparationObservationKey::validity(&need));
        assert!(!DirectLocalModulePreparationObservationKey::equality(
            &need, &need
        ));
        let outer = SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::Epoch(
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/workspace/dep/p/a.MODULE.bazel").unwrap(),
                    PathObservationOperation::Lstat,
                ),
                result_operation: PathObservationOperation::FileBytes,
            },
        )));
        assert!(DirectLocalModulePreparationObservationKey::validity(&outer));
        assert!(DirectLocalModulePreparationObservationKey::equality(
            &outer, &outer
        ));
    }
    #[derive(Default)]
    struct MaterializationRequestTracker {
        rows: Mutex<Vec<(String, Vec<String>)>>,
        rich: Mutex<Vec<(String, ActivationKind, bool)>>,
    }
    impl ActivationTracker for MaterializationRequestTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            let name = key.to_string();
            if name.starts_with("repository-materialization-request:")
                || name.starts_with("observed-repository-materialization-request:")
                || name.starts_with("repository-materialization:")
                || name.starts_with("observed-repository-materialization:")
            {
                self.rows
                    .lock()
                    .unwrap()
                    .push((name, deps.map(ToString::to_string).collect()));
            }
        }
        fn tracks_rich_activations(&self) -> bool {
            true
        }
        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.rich.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                activation.evaluation_data().is_some(),
            ));
        }
    }
    fn materialization_request_epoch(source: &str, variant: i64) -> PathObservationEpoch {
        horizon_epoch(
            source,
            PathObservationNamespace::Host,
            "/workspace/dep",
            None,
            None,
            None,
            None,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            variant,
        )
    }

    fn inject_materialization_request_inputs(
        updater: &mut dice::DiceTransactionUpdater,
        source: &str,
        variant: i64,
        overrides: &[&str],
        observe: bool,
    ) -> PathObservationEpoch {
        let epoch = materialization_request_epoch(source, variant);
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                if observe {
                    epoch.dupe()
                } else {
                    PathObservationEpoch::empty()
                },
            )])
            .unwrap();
        inject_host_effective_inputs(updater, source, crate::LockfileMode::Off, overrides);
        inject_root_package_policy_inputs(
            updater,
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
        epoch
    }
    async fn materialization_request_case(
        dice: &Arc<Dice>,
        source: &str,
        variant: i64,
        overrides: &[&str],
        observe: bool,
    ) -> (
        MaterializationRequestDriverOutcome,
        MaterializationRequestResult,
        PathObservationEpoch,
        Vec<(String, ActivationKind, bool)>,
        Vec<(String, Vec<String>)>,
    ) {
        let tracker = Arc::new(MaterializationRequestTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        let epoch = inject_materialization_request_inputs(
            &mut updater,
            source,
            variant,
            overrides,
            observe,
        );
        let mut transaction = updater.commit().await;
        let observed = transaction
            .compute(&RepositoryMaterializationRequestObservationKey::new(
                PathBuf::from("/workspace"),
                "dep".into(),
            ))
            .await
            .unwrap();
        let trace = std::mem::take(&mut *tracker.rich.lock().unwrap());
        let legacy = transaction
            .compute(&RepositoryMaterializationRequestKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
            })
            .await
            .unwrap();
        let rows = tracker.rows.lock().unwrap().clone();
        (observed, legacy, epoch, trace, rows)
    }
    #[test]
    fn observed_materialization_request_reducer_and_projection_are_exact() {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
            PathObservationOperation::Lstat,
        );
        let shared = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), shared.dupe())]).unwrap();
        let child = ObservedHostEffectiveModuleOverride::new(
            Arc::new(Ok(HostEffectiveModuleOverride::None)),
            epoch.dupe(),
        );
        let ControlFlow::Continue((_, forwarded)) = finish_observed_materialization_effective(
            SourcePreparationOutcome::Complete(Ok(child)),
        ) else {
            panic!("success must continue")
        };
        assert!(Arc::ptr_eq(
            forwarded.observations().get(&demand).unwrap(),
            &shared
        ));
        let semantic = Arc::new(Err(
            crate::module_eval::HostEffectiveModuleOverrideError::CommandPolicy("policy".into()),
        ));
        let ControlFlow::Break(SourcePreparationOutcome::Complete(Ok(error))) =
            finish_observed_materialization_effective(SourcePreparationOutcome::Complete(Ok(
                ObservedHostEffectiveModuleOverride::new(semantic, epoch.dupe()),
            )))
        else {
            panic!("semantic error must retain a carrier")
        };
        assert_eq!(error.observations(), &epoch);
        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(demand.dupe()),
        ));
        let ControlFlow::Break(need) = finish_observed_materialization_effective(need) else {
            panic!("Need must stop")
        };
        assert!(
            !RepositoryMaterializationRequestObservationKey::validity(&need)
                && !RepositoryMaterializationRequestObservationKey::equality(&need, &need)
        );
        let outer = SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
            slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand),
        )));
        let ControlFlow::Break(outer) = finish_observed_materialization_effective(outer) else {
            panic!("outer must stop")
        };
        assert!(
            RepositoryMaterializationRequestObservationKey::validity(&outer)
                && RepositoryMaterializationRequestObservationKey::equality(&outer, &outer)
        );
        let held = Arc::new(Err(RepositoryMaterializationError::MissingOverride(
            "dep".into(),
        )));
        let legacy = project_legacy_materialization_request(SourcePreparationOutcome::Complete(
            Ok(ObservedRepositoryMaterializationRequest {
                result: held.dupe(),
                observations: epoch,
            }),
        ));
        assert!(Arc::ptr_eq(&held, &legacy));
    }

    #[tokio::test]
    async fn observed_materialization_request_lifecycle_families_events_and_cancellation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let a_source = "module(name='root')\nprint('root-a')\nlocal_path_override(module_name='dep', path='dep-a')\n";
        let b_source = "module(name='root')\nlocal_path_override(module_name='dep',path='b')\n";
        let (a, legacy, injected, cold, rows) =
            materialization_request_case(&dice, a_source, 1, &[], true).await;
        let key = RepositoryMaterializationRequestObservationKey::new(
            PathBuf::from("/workspace"),
            "dep".into(),
        );
        assert!(key.to_string().starts_with("observed-repository-"));
        let SourcePreparationOutcome::Complete(Ok(a_value)) = &a else {
            panic!("request must complete: {a:?}")
        };
        assert_eq!(a_value.result().as_ref(), legacy.as_ref());
        assert!(
            a_value
                .observations()
                .observations()
                .iter()
                .all(|(demand, result)| Arc::ptr_eq(
                    result,
                    injected.observations().get(demand).unwrap()
                ))
        );
        assert!(
            cold.iter()
                .any(|(name, _, event)| name == &key.to_string() && !event)
        );
        assert!(
            cold.iter()
                .any(|(name, _, event)| name.contains("root-module") && *event)
        );
        assert!(rows.iter().any(|(name, deps)| name == &key.to_string()
            && deps == &["observed-host-effective-module-override:\"/workspace\":dep"]));
        assert!(rows.iter().any(
            |(name, deps)| name == "repository-materialization-request:dep"
                && deps == &["host-effective-module-override:\"/workspace\":dep"]
        ));
        assert!(!cold.iter().any(|(name, _, _)| {
            [
                "repository-materialization:",
                "host-repository-source-file:",
                "host-nonregistry-module",
                "host-discovered-module:",
                "root-module-graph:",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        }));

        let (warm, _, _, reuse, _) =
            materialization_request_case(&dice, a_source, 1, &[], true).await;
        assert!(RepositoryMaterializationRequestObservationKey::equality(
            &a, &warm
        ));
        assert!(
            reuse
                .iter()
                .any(|(name, kind, event)| name == &key.to_string()
                    && *kind == ActivationKind::Reused
                    && !event)
        );
        let (b, _, _, _, _) = materialization_request_case(&dice, b_source, 2, &[], true).await;
        let (restored, _, _, _, _) =
            materialization_request_case(&dice, a_source, 1, &[], true).await;
        assert!(!RepositoryMaterializationRequestObservationKey::equality(
            &a, &b
        ));
        assert!(RepositoryMaterializationRequestObservationKey::equality(
            &a, &restored
        ));
        assert!(matches!(a_value.result().as_ref(), Ok(request)
            if matches!(&request.kind, RepositoryMaterializationKind::Local { logical_root }
                if logical_root.as_path() == Path::new("/workspace/dep-a"))));

        let (need, _, _, trace, rows) =
            materialization_request_case(&dice, a_source, 3, &[], false).await;
        assert!(
            !RepositoryMaterializationRequestObservationKey::validity(&need)
                && !RepositoryMaterializationRequestObservationKey::equality(&need, &need)
        );
        assert!(rows.iter().any(|(name, deps)| name.starts_with("observed-")
            && deps == &["observed-host-effective-module-override:\"/workspace\":dep"]));
        assert!(!trace.iter().any(
            |(name, _, _)| name.starts_with("repository-materialization:")
                || name.starts_with("host-repository-source-file:")
        ));

        let tracker = Arc::new(MaterializationRequestTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_materialization_request_inputs(&mut updater, b_source, 4, &[], true);
        let mut cancelled = updater.commit().await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(future.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        drop(cancelled);
        assert!(tracker.rich.lock().unwrap().is_empty());
        let (recovered, _, _, _, _) =
            materialization_request_case(&dice, b_source, 4, &[], true).await;
        assert!(RepositoryMaterializationRequestObservationKey::validity(
            &recovered
        ));
    }
    fn observed_materialization_value(
        outcome: &MaterializationRequestDriverOutcome,
    ) -> &ObservedRepositoryMaterializationRequest {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            _ => panic!("observed materialization request did not complete"),
        }
    }
    #[tokio::test]
    async fn observed_materialization_request_terminal_prefix_matrix_is_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction = dice.updater().commit().await;
        let invalid = transaction
            .compute(&RepositoryMaterializationRequestObservationKey::new(
                PathBuf::from("relative"),
                "dep".into(),
            ))
            .await
            .unwrap();
        let invalid = observed_materialization_value(&invalid);
        assert!(matches!(
            invalid.result().as_ref(),
            Err(RepositoryMaterializationError::InvalidWorkspace(_))
        ));
        assert!(invalid.observations().observations().is_empty());
        let compute = materialization_effective_compute_error("effective compute");
        assert!(
            observed_materialization_value(&compute)
                .observations()
                .observations()
                .is_empty()
        );
        let cases = [
            "module(name='root')\n",
            "module(name='root')\nsingle_version_override(module_name='dep',version='1.0')\n",
            "module(name='root')\nlocal_path_override(module_name='dep',path='../dep')\n",
            "module(name='root')\nlocal_path_override(module_name='dep',path='dep')\n",
            "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n",
            "module(name='root')\ngit_override(module_name='dep',remote='https://example.invalid/r.git',commit='deadbeef')\n",
        ];
        for (index, source) in cases.into_iter().enumerate() {
            let (observed, legacy, injected, _, _) =
                materialization_request_case(&dice, source, 20 + index as i64, &[], true).await;
            let value = observed_materialization_value(&observed);
            assert_eq!(value.result().as_ref(), legacy.as_ref());
            assert_eq!(value.observations().observations().len(), 4);
            assert!(value.observations().observations().iter().all(
                |(demand, result)| Arc::ptr_eq(
                    result,
                    injected.observations().get(demand).unwrap()
                )
            ));
            match (index, value.result().as_ref()) {
                (0, Err(RepositoryMaterializationError::MissingOverride(name)))
                    if name == "dep" => {}
                (1, Err(RepositoryMaterializationError::UnsupportedOverride(_))) => {}
                (2, Err(RepositoryMaterializationError::Spec(message)))
                    if message.contains("workspace-relative") => {}
                (
                    3,
                    Ok(RepositoryMaterializationRequest {
                        kind: RepositoryMaterializationKind::Local { logical_root },
                        ..
                    }),
                ) if logical_root.as_path() == Path::new("/workspace/dep") => {}
                (
                    4 | 5,
                    Ok(RepositoryMaterializationRequest {
                        kind: RepositoryMaterializationKind::Immutable,
                        ..
                    }),
                ) => {}
                other => panic!("unexpected observed request terminal: {other:?}"),
            }
        }
        let epoch = materialization_request_epoch("canonical", 40);
        let effective = HostEffectiveModuleOverride::Root {
            override_: RootModuleOverride::NonRegistry(local_route().repo_spec().clone()),
        };
        let canonical = materialization_request_complete(
            project_materialization_request(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                &"bad/name".into(),
                &effective,
            ),
            epoch.dupe(),
        );
        let canonical = observed_materialization_value(&canonical);
        assert!(matches!(
            canonical.result().as_ref(),
            Err(RepositoryMaterializationError::InvalidCanonicalRepository(
                _
            ))
        ));
        assert_eq!(canonical.observations(), &epoch);
    }
    #[tokio::test]
    async fn observed_materialization_request_command_and_kind_restore_exactly() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = "module(name='root')\n";
        let (command_a, _, _, _, _) =
            materialization_request_case(&dice, root, 60, &["dep=/workspace/dep-a"], true).await;
        let held = observed_materialization_value(&command_a);
        let held_result = held.result().dupe();
        let (held_demand, held_observation) = held
            .observations()
            .observations()
            .iter()
            .next()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .unwrap();
        let (command_b, _, _, _, _) =
            materialization_request_case(&dice, root, 61, &["dep=/workspace/dep-b"], true).await;
        let (command_restored, _, _, _, _) =
            materialization_request_case(&dice, root, 60, &["dep=/workspace/dep-a"], true).await;
        assert!(!RepositoryMaterializationRequestObservationKey::equality(
            &command_a, &command_b
        ));
        assert!(RepositoryMaterializationRequestObservationKey::equality(
            &command_a,
            &command_restored
        ));
        assert!(matches!(held_result.as_ref(), Ok(request)
            if matches!(&request.kind, RepositoryMaterializationKind::Local { logical_root }
                if logical_root.as_path() == Path::new("/workspace/dep-a"))));
        assert!(Arc::ptr_eq(
            held.observations()
                .observations()
                .get(&held_demand)
                .unwrap(),
            &held_observation
        ));
        let archive = "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n";
        let git = "module(name='root')\ngit_override(module_name='dep',remote='https://example.invalid/r.git',commit='deadbeef')\n";
        let (kind_a, _, _, _, _) =
            materialization_request_case(&dice, archive, 70, &[], true).await;
        let held_kind = observed_materialization_value(&kind_a).result().dupe();
        let (kind_b, _, _, _, _) = materialization_request_case(&dice, git, 71, &[], true).await;
        let (kind_restored, _, _, _, _) =
            materialization_request_case(&dice, archive, 70, &[], true).await;
        assert!(!RepositoryMaterializationRequestObservationKey::equality(
            &kind_a, &kind_b
        ));
        assert!(RepositoryMaterializationRequestObservationKey::equality(
            &kind_a,
            &kind_restored
        ));
        assert!(matches!(held_kind.as_ref(), Ok(request)
            if request.repo_spec.rule_id.rule_name == "http_archive"
                && request.kind == RepositoryMaterializationKind::Immutable));
    }

    fn observed_repository_materialization(
        outcome: &RepositoryMaterializationDriverOutcome,
    ) -> &ObservedRepositoryMaterialization {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            _ => panic!("observed materialization did not complete: {outcome:?}"),
        }
    }

    async fn repository_materialization_case(
        dice: &Arc<Dice>,
        source: &str,
        variant: i64,
        result: Option<RepositoryMaterializationResult>,
        generation: Option<RepositoryMaterializationGeneration>,
        mismatch_request: bool,
    ) -> (
        RepositoryMaterializationDriverOutcome,
        <RepositoryMaterializationKey as Key>::Value,
        PathObservationEpoch,
        Vec<(String, ActivationKind, bool)>,
        Vec<(String, Vec<String>)>,
    ) {
        let tracker = Arc::new(MaterializationRequestTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_materialization_request_inputs(&mut updater, source, variant, &[], true);
        let mut transaction = updater.commit().await;
        let request = transaction
            .compute(&RepositoryMaterializationRequestObservationKey::new(
                PathBuf::from("/workspace"),
                "dep".into(),
            ))
            .await
            .unwrap();
        let injected = observed_materialization_value(&request)
            .observations()
            .dupe();
        let request = observed_materialization_value(&request)
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .clone();
        let epoch_request = if mismatch_request {
            RepositoryMaterializationRequest {
                repo_spec: local_route_with_path("other").repo_spec().clone(),
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new("/workspace/other").unwrap(),
                },
                ..request.clone()
            }
        } else {
            request
        };
        let entries = result
            .into_iter()
            .map(|result| RepositoryMaterializationEpochEntry {
                request: Arc::new(epoch_request.clone()),
                result,
            });
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                },
                RepositoryMaterializationResultEpoch::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    entries,
                )
                .unwrap(),
            )])
            .unwrap();
        if let Some(generation) = generation {
            updater
                .changed_to(vec![(
                    RepositoryMaterializationGenerationKey {
                        workspace: PathBuf::from("/workspace"),
                    },
                    generation,
                )])
                .unwrap();
        }
        transaction = updater.commit().await;
        let observed = transaction
            .compute(&RepositoryMaterializationObservationKey::new(
                PathBuf::from("/workspace"),
                "dep".into(),
            ))
            .await
            .unwrap();
        let legacy = transaction
            .compute(&RepositoryMaterializationKey {
                workspace: PathBuf::from("/workspace"),
                module_name: "dep".into(),
            })
            .await
            .unwrap();
        (
            observed,
            legacy,
            injected,
            tracker.rich.lock().unwrap().clone(),
            tracker.rows.lock().unwrap().clone(),
        )
    }

    type MaterializationKey = RepositoryMaterializationObservationKey;
    type MResult = RepositoryMaterializationResult;
    type MError = RepositoryMaterializationError;

    #[test]
    fn observed_repository_materialization_reducer_projection_and_prefixes_are_exact() {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
            PathObservationOperation::Lstat,
        );
        let shared = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), shared.dupe())]).unwrap();
        let request = RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: local_route_with_path("dep").repo_spec().clone(),
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
            },
        };
        let ControlFlow::Continue((_forwarded, prefix)) = finish_observed_materialization_request(
            SourcePreparationOutcome::Complete(Ok(ObservedRepositoryMaterializationRequest {
                result: Arc::new(Ok(request)),
                observations: epoch.dupe(),
            })),
        ) else {
            panic!("request success must continue")
        };
        assert!(Arc::ptr_eq(
            prefix.observations().get(&demand).unwrap(),
            &shared
        ));
        let request_compute = repository_materialization_request_compute_error("request compute");
        let request_compute = observed_repository_materialization(&request_compute);
        assert!(
            matches!(
                request_compute.result().as_ref(),
                Err(MError::RootModuleFiles(message))
                    if message == "request compute"
            ) && request_compute.observations().observations().is_empty()
        );
        let result_compute =
            repository_materialization_result_compute_error("result compute", epoch.dupe());
        let result_compute = observed_repository_materialization(&result_compute);
        assert!(
            matches!(
                result_compute.result().as_ref(),
                Err(MError::ResultCompute(message))
                    if message == "result compute"
            ) && result_compute.observations() == &epoch
        );

        let request_error = Arc::new(Err(MError::MissingOverride("dep".into())));
        let ControlFlow::Break(error) = finish_observed_materialization_request(
            SourcePreparationOutcome::Complete(Ok(ObservedRepositoryMaterializationRequest {
                result: request_error,
                observations: epoch.dupe(),
            })),
        ) else {
            panic!("request semantic error must stop")
        };
        assert_eq!(
            observed_repository_materialization(&error).observations(),
            &epoch
        );

        for (input, valid) in [
            (
                SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
                    NeedPathObservations::singleton(demand.dupe()),
                )),
                false,
            ),
            (
                SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
                    slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand),
                ))),
                true,
            ),
        ] {
            let ControlFlow::Break(outcome) = finish_observed_materialization_request(input) else {
                panic!("request terminal must stop")
            };
            assert_eq!(MaterializationKey::validity(&outcome), valid);
            assert_eq!(MaterializationKey::equality(&outcome, &outcome), valid);
        }

        let held = Arc::new(Err(MError::Spec("spec".into())));
        let carrier = repository_materialization_complete(held.dupe(), epoch.dupe());
        assert!(Arc::ptr_eq(
            observed_repository_materialization(&carrier).result(),
            &held
        ));
        let SourcePreparationOutcome::Complete(projected) =
            project_legacy_repository_materialization(carrier)
        else {
            panic!("legacy projection must complete")
        };
        assert!(Arc::ptr_eq(&held, &projected));
    }

    #[tokio::test]
    async fn observed_repository_materialization_lifecycle_families_events_and_needs() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let a_source = "module(name='root')\nprint('root-a')\nlocal_path_override(module_name='dep',path='dep-a')\n";
        let local = MResult::Success(RepositoryMaterializationSuccess::Local);
        let (a, legacy, injected, cold, rows) =
            repository_materialization_case(&dice, a_source, 101, Some(local.clone()), None, false)
                .await;
        let key = MaterializationKey::new(PathBuf::from("/workspace"), "dep".into());
        let a_value = observed_repository_materialization(&a);
        let SourcePreparationOutcome::Complete(legacy) = &legacy else {
            panic!("legacy materialization must complete")
        };
        assert!(Arc::ptr_eq(a_value.result(), legacy));
        let result_dep = "repository-materialization-result:@@dep+".to_owned();
        assert!(rows.contains(&(
            key.to_string(),
            vec![
                "observed-repository-materialization-request:dep".into(),
                result_dep.clone()
            ],
        )));
        assert!(rows.contains(&(
            "repository-materialization:dep".into(),
            vec!["repository-materialization-request:dep".into(), result_dep],
        )));
        let parent_silent = cold
            .iter()
            .any(|(name, _, event)| name == &key.to_string() && !event);
        let child_event = cold
            .iter()
            .any(|(name, _, event)| name.contains("root-module") && *event);
        assert!(parent_silent && child_event);
        assert!(!cold.iter().any(|(name, _, _)| {
            [
                "repository-source-file:",
                "host-nonregistry-module",
                "host-discovered-module:",
                "host-selected-module-graph:",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        }));

        let (warm, _, _, reuse, _) =
            repository_materialization_case(&dice, a_source, 101, Some(local.clone()), None, false)
                .await;
        assert!(MaterializationKey::equality(&a, &warm));
        assert!(reuse.iter().any(|(name, kind, event)| {
            name == &key.to_string() && *kind == ActivationKind::Reused && !event
        }));
        let held_result = a_value.result().dupe();
        let changed = MResult::SpecError("changed".into());
        let (b, _, _, _, _) =
            repository_materialization_case(&dice, a_source, 101, Some(changed), None, false).await;
        let (restored, _, _, _, _) =
            repository_materialization_case(&dice, a_source, 101, Some(local.clone()), None, false)
                .await;
        assert!(!MaterializationKey::equality(&a, &b));
        assert!(MaterializationKey::equality(&a, &restored));
        assert!(matches!(
            held_result.as_ref(),
            Ok(RepositoryMaterialization::Local { source_root, .. })
                if source_root == Path::new("/workspace/dep-a")
        ));
        assert!(injected.observations().iter().all(|(demand, result)| {
            Arc::ptr_eq(
                result,
                a_value.observations().observations().get(demand).unwrap(),
            )
        }));

        let (missing, _, _, _, _) =
            repository_materialization_case(&dice, a_source, 103, None, None, false).await;
        assert!(
            !MaterializationKey::validity(&missing)
                && !MaterializationKey::equality(&missing, &missing)
        );
        let (mismatch, _, _, _, _) =
            repository_materialization_case(&dice, a_source, 104, Some(local), None, true).await;
        assert!(matches!(mismatch, SourcePreparationOutcome::Need(_)));

        for case in 0..5 {
            let result_generation = RepositoryMaterializationGeneration(case);
            let result = match case {
                0 => MResult::SpecError("spec".into()),
                2 | 4 => MResult::TransportError {
                    generation: result_generation,
                    message: "transport".into(),
                },
                _ => MResult::MaterializationError {
                    generation: result_generation,
                    message: "materialization".into(),
                },
            };
            let generation = match case {
                2 | 3 => Some(result_generation),
                4 => Some(RepositoryMaterializationGeneration(9)),
                _ => None,
            };
            let (outcome, _, prefix, _, _) = repository_materialization_case(
                &dice,
                a_source,
                105 + case as i64,
                Some(result),
                generation,
                false,
            )
            .await;
            if case < 4 {
                let observed = observed_repository_materialization(&outcome);
                assert_eq!(observed.observations(), &prefix);
                assert!(match (case, observed.result().as_ref()) {
                    (0, Err(MError::Spec(message))) if message == "spec" => true,
                    (1, Err(MError::MissingGeneration(_))) => true,
                    (2, Err(MError::Transport(message))) if message == "transport" => true,
                    (3, Err(MError::Materialization(message))) if message == "materialization" =>
                        true,
                    _ => false,
                });
            } else {
                assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
            }
        }
    }

    #[tokio::test]
    async fn observed_repository_materialization_immutable_error_and_cancellation_are_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let archive = "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n";
        let immutable = MResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("sha256-a"),
            generation_root: PathBuf::from("/immutable/a"),
            observation_instance: PathObservationInstanceId::new(7),
        });
        let (success, _, epoch, _, _) =
            repository_materialization_case(&dice, archive, 201, Some(immutable), None, false)
                .await;
        let success = observed_repository_materialization(&success);
        assert!(matches!(
            success.result().as_ref(),
            Ok(RepositoryMaterialization::Immutable {
                source_identity,
                generation_root,
                ..
            }) if source_identity.as_ref() == "sha256-a"
                && generation_root == Path::new("/immutable/a")
        ));
        assert_eq!(success.observations(), &epoch);

        let tracker = Arc::new(MaterializationRequestTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_materialization_request_inputs(&mut updater, archive, 203, &[], true);
        let mut cancelled = updater.commit().await;
        let key = MaterializationKey::new(PathBuf::from("/workspace"), "dep".into());
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(future.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        drop(cancelled);
        assert!(tracker.rich.lock().unwrap().is_empty());
        let result = Some(MResult::SpecError("recovered".into()));
        let (recovered, _, _, _, _) =
            repository_materialization_case(&dice, archive, 203, result, None, false).await;
        assert!(MaterializationKey::validity(&recovered));
    }

    fn repository_source_readlink_error_epoch() -> PathObservationEpoch {
        merge_path_observations(
            &host_path_epoch(
                PathObservationNamespace::Host,
                "/workspace/dep/file",
                Some(PathNodeKind::Symlink),
                None,
            ),
            &PathObservationEpoch::from_shared([(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/workspace/dep/file").unwrap(),
                    PathObservationOperation::ReadLink,
                ),
                Arc::new(PathObservationResult::ReadLink(PathOperationResult::Error(
                    PathObservationError::NotALink,
                ))),
            )])
            .unwrap(),
        )
        .unwrap()
    }

    fn test_hash<T: Hash>(value: &T) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }

    #[test]
    fn selected_frontier_preserves_path_and_nested_compute_polarity() {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
            PathObservationOperation::Lstat,
        );
        let path = ObservedPathFrontierError::from(
            slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand),
        );
        assert!(matches!(
            preflight_observation_frontier(
                &HostNonregistryPackagePreflightObservationError::EffectiveFrontier(path)
            ),
            HostSelectedObservationFrontier::Path(_)
        ));

        let compute_errors = [
            HostNonregistryPackagePreflightObservationError::EffectiveCompute("effective".into()),
            HostNonregistryPackagePreflightObservationError::PolicyCompute("policy".into()),
            HostNonregistryPackagePreflightObservationError::IgnoreCompute("ignore".into()),
            HostNonregistryPackagePreflightObservationError::MarkerCompute {
                marker: HostBuildFileName::Build,
                message: "marker".into(),
            },
        ];
        for error in compute_errors {
            let selected = HostDiscoveredModuleObservationError::ClosureFrontier(
                HostDiscoveredModuleClosureFrontier(
                    HostNonregistryModuleClosureObservationError::PreparationFrontier(
                        NonregistryPreparationFrontierError::Package(error),
                    ),
                ),
            )
            .selected_frontier();
            assert!(matches!(
                selected,
                HostSelectedObservationFrontier::Infrastructure(_)
            ));
        }
    }

    mod observation_tests {
        use super::*;
        include!("source_preparation_observation_tests.rs");
    }
}
