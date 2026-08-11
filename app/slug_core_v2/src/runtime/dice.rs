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
use std::future::Future;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Poll;

use allocative::Allocative;
use anyhow::Context;
use async_trait::async_trait;
#[cfg(test)]
use dice::ActivationData;
#[cfg(test)]
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
#[cfg(test)]
use dice::DynKey;
use dice::InjectedKey;
use dice::Key;
#[cfg(test)]
use dice::RichActivation;
#[cfg(test)]
use dice::RootActivation;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_analysis_v2::AnalysisError;
use slug_analysis_v2::AnalysisErrorKind;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeKind;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::prepare_configured_node_analysis;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::HostRepositorySourceFileKey;
use slug_bzlmod_v2::HostRepositorySourceFileValue;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryRequestGenerationKey;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationGenerationKey;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositorySourceFileError;
use slug_bzlmod_v2::RootModuleCommandPolicyKey;
use slug_bzlmod_v2::RootModuleEnvironmentPolicyKey;
use slug_bzlmod_v2::RootModuleGraph;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootModuleLoadingAnchor;
use slug_bzlmod_v2::RootModuleLoadingAnchorError;
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_bzlmod_v2::RootModuleLockfileModeKey;
use slug_bzlmod_v2::RootModuleRegistryUrlsKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::RootRepositoryRouteError;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_configuration_v2::RootStringSettingValue;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::SlugConfigurationProjection;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetName;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::RepositoryPackageLoadError;
use slug_loading_v2::RepositoryPackageLoadKey;
use slug_loading_v2::RootPackageLoadError;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::bzl_load_cycle_detector;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_query_v2::CqueryQueryEnvironment;
use slug_query_v2::QueryError;
use slug_query_v2::QueryExpression;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryOutput;
use slug_query_v2::QueryOutputCompletion;
use slug_query_v2::QueryPolicy;
use slug_query_v2::RootQueryCommandKey;
use slug_query_v2::TargetSet;
use slug_query_v2::cquery_literals;
use slug_query_v2::evaluate_cquery_query;
use slug_query_v2::evaluate_loading_query_with_policy_and_output_completion;
use slug_query_v2::preflight_cquery_query;
use slug_query_v2::render_unfactored_dot;
use slug_query_v2::validate_cquery_query;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::WorkspaceFileKey;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use super::RuntimeMode;
use super::demands::SelectedWorkspaceDemands;
use super::demands::WorkspaceDemandOwner;
use super::events::AcceptedCommand;
use super::events::AttemptEffectTracker;
use super::events::CommandEffectError;
use super::events::CommandEffectOwner;
use super::events::SealedCommandAttempt;
use super::events::SelectedCommandSidecars;
use super::starlark::evaluate_file;

pub trait IncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OneShotIncrementalEngine;

impl IncrementalEngine for OneShotIncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode {
        RuntimeMode::OneShot
    }
}

/// The result of evaluating the root Starlark files for one workspace.
///
/// This is intentionally just the Stage 2 runtime boundary. Stage 4 owns
/// Bazel file loading and Stage 5 owns the full `MODULE.bazel` global surface.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceEvaluation {
    pub module: EvaluatedFile,
    pub build: EvaluatedFile,
    pub revision: WorkspaceRevision,
}

/// The one committed request revision shared by root and package loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub struct WorkspaceRevision(u64);

/// A complete external observation of one workspace file.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceFileObservation {
    pub path: PathBuf,
    pub value: WorkspaceFileValue,
}

impl WorkspaceFileObservation {
    /// Read one path outside DICE and retain missing and failed reads distinctly.
    pub fn read(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let value = match std::fs::read_to_string(&path) {
            Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                WorkspaceFileValue::Absent
            }
            Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
        };
        Self { path, value }
    }
}

/// A complete raw-byte observation of one workspace file.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceRawFileObservation {
    pub path: PathBuf,
    pub value: WorkspaceRawFileValue,
}

impl WorkspaceRawFileObservation {
    fn from_text(observation: &WorkspaceFileObservation) -> Self {
        Self {
            path: observation.path.clone(),
            value: match &observation.value {
                WorkspaceFileValue::Present(source) => {
                    WorkspaceRawFileValue::Present(Arc::from(source.as_bytes()))
                }
                WorkspaceFileValue::Absent => WorkspaceRawFileValue::Absent,
                WorkspaceFileValue::ReadError(error) => {
                    WorkspaceRawFileValue::ReadError(error.clone())
                }
            },
        }
    }
}

/// A complete external observation of one direct workspace directory.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceDirectoryObservation {
    pub path: PathBuf,
    pub value: WorkspaceDirectoryValue,
}

/// Externally observed workspace state supplied to one runtime request.
///
/// Files-only callers remain convenient through 'Self::from_files', but the
/// resulting request still injects an explicit empty directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceObservation {
    pub files: Vec<WorkspaceFileObservation>,
    pub raw_files: Vec<WorkspaceRawFileObservation>,
    pub directories: Vec<WorkspaceDirectoryObservation>,
}

impl WorkspaceObservation {
    pub fn from_files(files: impl IntoIterator<Item = WorkspaceFileObservation>) -> Self {
        let files = files.into_iter().collect::<Vec<_>>();
        Self {
            raw_files: files
                .iter()
                .map(WorkspaceRawFileObservation::from_text)
                .collect(),
            files,
            directories: Vec::new(),
        }
    }
}

/// Read a complete workspace snapshot outside DICE.
///
/// This initial M1 adapter observes every regular file, including hidden
/// paths, so a missing requested `.bzl` is represented by `Absent` rather than
/// an uninitialized DICE input. It deliberately makes no freshness decision;
/// `WorkspaceRuntime` owns that through `changed_to` equality.
pub fn observe_workspace(workspace: &Path) -> anyhow::Result<WorkspaceObservation> {
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
    let mut observation = WorkspaceObservation::from_files([]);
    collect_workspace_observations(&workspace, &workspace, &mut observation);
    Ok(observation)
}

/// The legacy focused-test adapter. Production callers should use
/// 'observe_workspace' so direct directory observations travel with files.
pub fn observe_workspace_files(workspace: &Path) -> anyhow::Result<Vec<WorkspaceFileObservation>> {
    Ok(observe_workspace(workspace)?.files)
}

fn collect_workspace_observations(
    workspace: &Path,
    directory: &Path,
    observation: &mut WorkspaceObservation,
) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            observation.directories.push(WorkspaceDirectoryObservation {
                path: directory.to_path_buf(),
                value: WorkspaceDirectoryValue::Absent,
            });
            return;
        }
        Err(error) => {
            observation.directories.push(WorkspaceDirectoryObservation {
                path: directory.to_path_buf(),
                value: WorkspaceDirectoryValue::ReadError(Arc::new(error.to_string())),
            });
            return;
        }
    };
    let mut direct_entries = Vec::new();
    let mut children = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                observation.directories.push(WorkspaceDirectoryObservation {
                    path: directory.to_path_buf(),
                    value: WorkspaceDirectoryValue::ReadError(Arc::new(error.to_string())),
                });
                return;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                observation.directories.push(WorkspaceDirectoryObservation {
                    path: directory.to_path_buf(),
                    value: WorkspaceDirectoryValue::ReadError(Arc::new(error.to_string())),
                });
                return;
            }
        };
        let kind = if file_type.is_file() {
            WorkspaceDirectoryEntryKind::RegularFile
        } else if file_type.is_dir() {
            WorkspaceDirectoryEntryKind::Directory
        } else if file_type.is_symlink() {
            WorkspaceDirectoryEntryKind::Symlink
        } else {
            WorkspaceDirectoryEntryKind::Other
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            observation.directories.push(WorkspaceDirectoryObservation {
                path: directory.to_path_buf(),
                value: WorkspaceDirectoryValue::ReadError(Arc::new(format!(
                    "directory entry name is not valid UTF-8: {}",
                    path.display()
                ))),
            });
            return;
        };
        // Configured outputs and collision sidecars are build products, never
        // source inputs. Excluding the root-owned tree also prevents its
        // creation from fabricating a workspace invalidation in a retained
        // daemon.
        if directory == workspace && name == "bazel-out" {
            continue;
        }
        direct_entries.push(WorkspaceDirectoryEntry {
            name: name.into(),
            kind,
        });
        match kind {
            WorkspaceDirectoryEntryKind::RegularFile => {
                let (text, raw) = read_file_observations(path);
                observation.files.push(text);
                observation.raw_files.push(raw);
            }
            WorkspaceDirectoryEntryKind::Directory => children.push(path),
            WorkspaceDirectoryEntryKind::Symlink | WorkspaceDirectoryEntryKind::Other => {}
        }
    }
    observation.directories.push(WorkspaceDirectoryObservation {
        path: directory.to_path_buf(),
        value: WorkspaceDirectoryValue::present(direct_entries),
    });
    for child in children {
        // 'file_type()' identified a directory. Symlinks were already recorded
        // above and deliberately never arrive here.
        collect_workspace_observations(workspace, &child, observation);
    }
}

fn read_file_observations(
    path: PathBuf,
) -> (WorkspaceFileObservation, WorkspaceRawFileObservation) {
    let (text, raw) = match std::fs::read(&path) {
        Ok(bytes) => {
            let text = match String::from_utf8(bytes.clone()) {
                Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
                Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
            };
            (text, WorkspaceRawFileValue::Present(Arc::from(bytes)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (WorkspaceFileValue::Absent, WorkspaceRawFileValue::Absent)
        }
        Err(error) => {
            let error = Arc::new(error.to_string());
            (
                WorkspaceFileValue::ReadError(error.clone()),
                WorkspaceRawFileValue::ReadError(error),
            )
        }
    };
    (
        WorkspaceFileObservation {
            path: path.clone(),
            value: text,
        },
        WorkspaceRawFileObservation { path, value: raw },
    )
}

/// The sole DICE owner for one canonical workspace identity.
pub struct WorkspaceRuntime {
    workspace: PathBuf,
    // Retained now so the request-projection packet cannot accidentally create
    // a second process owner at observation time.
    #[allow(dead_code)]
    process_host: Arc<super::ProcessHostOwner>,
    configured_output: Arc<super::configured_output::ConfiguredOutputOwner>,
    dice: Arc<Dice>,
    demand_owner: Arc<WorkspaceDemandOwner>,
    loader: BzlModuleEvaluator,
    runtime: tokio::runtime::Runtime,
    next_revision: AtomicU64,
    next_registry_generation: AtomicU64,
    next_repository_materialization_generation: AtomicU64,
    #[allow(dead_code)] // Activated by the later retry-driver packet.
    repository_materializer: Arc<super::repository_io::RepositoryMaterializer>,
    #[allow(dead_code)] // Dormant until the shared retry-driver packet.
    native_demand_sessions: NativeDemandSessionOwner,
    #[cfg(test)]
    activation_audit: Option<Arc<ExternalQueryActivationAudit>>,
}

#[cfg(test)]
#[derive(Default)]
struct ExternalQueryActivationAudit {
    forbidden: Mutex<Vec<String>>,
    typed_roots: AtomicUsize,
    configured_roots: Mutex<Vec<ConfiguredTargetKey>>,
}

#[cfg(test)]
impl ExternalQueryActivationAudit {
    fn checkpoint(&self) -> (usize, usize) {
        (
            self.forbidden.lock().unwrap().len(),
            self.typed_roots.load(Ordering::Relaxed),
        )
    }

    fn assert_phase_clean(
        &self,
        checkpoint: (usize, usize),
        minimum_typed_root_activations: usize,
    ) {
        let forbidden = self.forbidden.lock().unwrap();
        assert_eq!(
            forbidden.len(),
            checkpoint.0,
            "external query activated forbidden legacy keys: {:?}",
            &forbidden[checkpoint.0..]
        );
        assert!(
            self.typed_roots.load(Ordering::Relaxed)
                >= checkpoint.1 + minimum_typed_root_activations,
            "external query did not activate the typed root enough times to prove the requested phase"
        );
    }

    fn record_key(&self, key: &DynKey) {
        let key_text = key.to_string();
        if key
            .downcast_ref::<slug_bzlmod_v2::RepositoryMaterializationKey>()
            .is_some()
            || key.downcast_ref::<RootModuleGraphKey>().is_some()
            || key
                .downcast_ref::<slug_bzlmod_v2::RootModuleFilesKey>()
                .is_some()
            || key.downcast_ref::<WorkspaceEvaluationKey>().is_some()
            || key
                .downcast_ref::<slug_loading_v2::keys::PackageLoadKey>()
                .is_some()
            || key
                .downcast_ref::<slug_loading_v2::keys::WorkspaceDirectorySnapshotKey>()
                .is_some()
            || key
                .downcast_ref::<slug_loading_v2::keys::WorkspaceDirectoryKey>()
                .is_some()
            || key.downcast_ref::<WorkspaceSnapshotKey>().is_some()
            || key.downcast_ref::<WorkspaceRawSnapshotKey>().is_some()
            || key.downcast_ref::<WorkspaceFileKey>().is_some()
            || key
                .downcast_ref::<slug_workspace_v2::WorkspaceRawFileKey>()
                .is_some()
            || key_text.starts_with("root-module-evaluation:")
        {
            self.forbidden.lock().unwrap().push(key_text);
        }
    }

    fn record_root(&self, key: &DynKey) {
        if key.downcast_ref::<RootQueryCommandKey>().is_some() {
            self.typed_roots.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() {
            if let Some(configured_target) = key.configured_target() {
                self.configured_roots
                    .lock()
                    .unwrap()
                    .push(configured_target.clone());
            }
        }
    }

    fn take_configured_roots(&self) -> Vec<ConfiguredTargetKey> {
        std::mem::take(&mut *self.configured_roots.lock().unwrap())
    }
}

#[cfg(test)]
struct AuditedRuntimeActivationTracker {
    runtime: Arc<dyn ActivationTracker>,
    audit: Arc<ExternalQueryActivationAudit>,
}

#[cfg(test)]
impl ActivationTracker for AuditedRuntimeActivationTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        // RuntimeActivationTracker's legacy callback is deliberately empty;
        // preserve its rich callback below and give the audit the one legacy
        // dependency iterator.
        let _ = (deps, activation);
        self.audit.record_key(key);
    }

    fn tracks_rich_activations(&self) -> bool {
        self.runtime.tracks_rich_activations()
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.audit.record_key(key);
        self.runtime.key_activated_rich(key, activation);
    }

    fn root_activated(&self, key: &DynKey, activation: RootActivation) {
        self.runtime.root_activated(key, activation);
        self.audit.record_root(key);
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeDemandLeaseToken(u64);

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
struct NativeDemandGenerationBundle {
    workspace_revision: WorkspaceRevision,
    registry: RegistryRequestGeneration,
    repository: RepositoryMaterializationGeneration,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeDemandRequestInputBundle {
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
    lockfile_mode: LockfileMode,
    registry_urls: RegistryUrls,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeDemandInputBundle {
    request: NativeDemandRequestInputBundle,
    generations: NativeDemandGenerationBundle,
}

#[allow(dead_code)]
impl NativeDemandRequestInputBundle {
    fn normalized_initial() -> Self {
        Self {
            command_policy: BzlmodCommandPolicyKey::from_flags(None, false)
                .expect("default bzlmod command policy is valid"),
            environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy is valid"),
            lockfile_mode: LockfileMode::Update,
            registry_urls: RegistryUrls::new(std::iter::empty::<&str>()),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct NativeDemandWorkspaceRevisionKey {
    workspace: PathBuf,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
struct NativeDemandWorkspaceRevision(WorkspaceRevision);

impl Dupe for NativeDemandWorkspaceRevision {}

impl fmt::Display for NativeDemandWorkspaceRevisionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "native-demand-workspace-revision:{}",
            self.workspace.display()
        )
    }
}

#[async_trait]
impl InjectedKey for NativeDemandWorkspaceRevisionKey {
    type Value = NativeDemandWorkspaceRevision;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[allow(dead_code)]
#[derive(Clone)]
struct AcceptedNativeDemandSnapshot {
    inputs: NativeDemandInputBundle,
    repository_results: RepositoryMaterializationResultEpoch,
    path_observations: PathObservationEpoch,
    selected: SelectedWorkspaceDemands,
}

#[allow(dead_code)]
enum NativeDemandLeasePhase {
    Idle,
    Open {
        lease: NativeDemandLeaseToken,
        repository: Option<super::repository_io::RepositorySessionToken>,
    },
}

#[allow(dead_code)]
struct NativeDemandSessionState {
    next_lease: u64,
    phase: NativeDemandLeasePhase,
    accepted: AcceptedNativeDemandSnapshot,
    #[cfg(test)]
    fail_next_restoration: bool,
    #[cfg(test)]
    fail_next_selected_injection: bool,
    #[cfg(test)]
    fail_next_replace_accepted: bool,
    #[cfg(test)]
    fail_next_close: bool,
    #[cfg(test)]
    trace: Vec<NativeDemandTestTrace>,
}

#[allow(dead_code)]
struct NativeDemandSessionOwner {
    state: Mutex<NativeDemandSessionState>,
}

#[allow(dead_code)]
#[derive(Debug)]
enum NativeDemandSessionError {
    Busy,
    LeaseTokenExhausted,
    StaleLease,
    Repository(super::repository_io::RepositorySessionError),
    ConflictingRepository(slug_identity_v2::CanonicalRepoName),
    RepositoryInternalNonProgress,
    PathInternalNonProgress,
    MissingSelectedPath(PathObservationDemand),
    PathEpoch(slug_workspace_v2::PathObservationEpochError),
    Effect(CommandEffectError),
    ForeignEffects,
    Computation(anyhow::Error),
    Injection(anyhow::Error),
    Restoration(anyhow::Error),
}

impl fmt::Display for NativeDemandSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("another workspace command is already active"),
            Self::LeaseTokenExhausted => f.write_str("workspace command lease tokens exhausted"),
            Self::StaleLease => f.write_str("workspace command lease is stale"),
            Self::Repository(_) => f.write_str("repository session failed"),
            Self::ConflictingRepository(repository) => {
                write!(f, "conflicting repository requests for {repository}")
            }
            Self::RepositoryInternalNonProgress => {
                f.write_str("repository preparation made no progress")
            }
            Self::PathInternalNonProgress => f.write_str("path preparation made no progress"),
            Self::MissingSelectedPath(_) => {
                f.write_str("a selected path observation was not materialized")
            }
            Self::PathEpoch(error) => write!(f, "path observation epoch failed: {error}"),
            Self::Effect(error) => write!(f, "command effect selection failed: {error}"),
            Self::ForeignEffects => f.write_str("command effects belong to another command"),
            Self::Computation(error) => write!(f, "command computation failed: {error}"),
            Self::Injection(error) => write!(f, "command input injection failed: {error}"),
            Self::Restoration(error) => write!(f, "command restoration failed: {error}"),
        }
    }
}

impl std::error::Error for NativeDemandSessionError {}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeDemandProgress {
    Repositories,
    Paths,
}

#[allow(dead_code)]
struct NativeDemandCommand<'a> {
    runtime: &'a WorkspaceRuntime,
    lease: NativeDemandLeaseToken,
    repository_session: super::repository_io::RepositorySessionToken,
    inputs: NativeDemandInputBundle,
    effects: Arc<CommandEffectOwner>,
    prior: AcceptedNativeDemandSnapshot,
    reusable_requests:
        SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>>,
    issued_requests:
        SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>>,
    repository_results: RepositoryMaterializationResultEpoch,
    path_observations: PathObservationEpoch,
}

#[allow(dead_code)]
struct NativeDemandPreflight<'a> {
    command: NativeDemandCommand<'a>,
}

#[allow(dead_code)]
struct NativeDemandAttempt {
    effects: Arc<CommandEffectOwner>,
    tracker: Arc<AttemptEffectTracker>,
}

#[allow(dead_code)]
struct NativeDemandSealedAttempt {
    effects: Arc<CommandEffectOwner>,
    sealed: SealedCommandAttempt,
}

#[allow(dead_code)]
struct NativeDemandTerminalSelection {
    effects: Arc<CommandEffectOwner>,
    sidecars: SelectedCommandSidecars,
}

#[allow(dead_code)]
struct NativeDemandPreparedAcceptance {
    events: super::events::SelectedEventBatches,
    snapshot: AcceptedNativeDemandSnapshot,
    validation: Vec<super::repository_io::RepositoryValidation>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeDemandAbortPhase {
    Restorable,
    Irreversible,
    FailClosed,
    Closed,
}

#[allow(dead_code)]
struct NativeDemandAbortGuard<'a> {
    command: Option<NativeDemandCommand<'a>>,
    attempt: Option<NativeDemandAttempt>,
    phase: NativeDemandAbortPhase,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
enum SyntheticCommandValue {
    Build(Arc<str>),
    Query(Arc<[Arc<str>]>),
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
struct SyntheticCommandError(Arc<str>);

#[allow(dead_code)]
type SyntheticCommandOutcome =
    slug_bzlmod_v2::SourcePreparationOutcome<Result<SyntheticCommandValue, SyntheticCommandError>>;

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
struct SyntheticCommandPlan {
    id: u64,
    workspace: NormalizedAbsolutePath,
    repositories: Arc<[Arc<RepositoryMaterializationRequest>]>,
    paths: Arc<[PathObservationDemand]>,
    terminal: Result<SyntheticCommandValue, SyntheticCommandError>,
    retry_event: Option<Arc<str>>,
    terminal_event: Option<Arc<str>>,
    behavior: SyntheticRootBehavior,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Allocative)]
struct SyntheticBuildRootKey {
    plan: Arc<SyntheticCommandPlan>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Allocative)]
struct SyntheticQueryRootKey {
    plan: Arc<SyntheticCommandPlan>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum SyntheticCommandRoot {
    Build(SyntheticBuildRootKey),
    Query(SyntheticQueryRootKey),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
enum SyntheticRootBehavior {
    Normal,
    PanicAfterInputs,
    PendForCancellation,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Allocative)]
struct SyntheticRepositoryDemandKey {
    plan_id: u64,
    index: usize,
    request: Arc<RepositoryMaterializationRequest>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Allocative)]
enum SyntheticEventKind {
    Retry,
    Terminal,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, Hash, PartialEq, Allocative)]
struct SyntheticEventKey {
    plan_id: u64,
    kind: SyntheticEventKind,
    text: Arc<str>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct DrivenCommand<T> {
    accepted: AcceptedCommand<T>,
    attempts: usize,
    terminal_root_count: usize,
}

#[allow(dead_code)]
enum CommandAttemptResult<T> {
    Retry(slug_bzlmod_v2::SourcePreparationNeeds),
    Terminal(T, NativeDemandPreparedAcceptance, usize),
}

type SyntheticCommandResult = DrivenCommand<Result<SyntheticCommandValue, SyntheticCommandError>>;

#[async_trait]
trait NativeCommandRoot: Clone {
    type Terminal: Clone;

    fn allows_empty_terminal(&self) -> bool {
        false
    }

    fn allows_unavailable_terminal_roots(&self, _terminal: &Self::Terminal) -> bool {
        false
    }

    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError>;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeDemandTestTrace {
    SelectedInjectionCommitted,
    TerminalTransactionDropped,
    MaterializerAccepted,
    AcceptedSnapshotReplaced,
    OutputBufferMoved,
    LeaseClosed,
    AttemptTransactionDroppedBeforeAbort,
}

macro_rules! impl_synthetic_root_identity {
    ($key:ty, $name:literal) => {
        impl PartialEq for $key {
            fn eq(&self, other: &Self) -> bool {
                self.plan == other.plan
            }
        }

        impl Eq for $key {}

        impl Hash for $key {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.plan.id.hash(state);
            }
        }

        impl fmt::Display for $key {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $name, self.plan.id)
            }
        }
    };
}

impl_synthetic_root_identity!(SyntheticBuildRootKey, "synthetic-build-root");
impl_synthetic_root_identity!(SyntheticQueryRootKey, "synthetic-query-root");

impl PartialEq for SyntheticRepositoryDemandKey {
    fn eq(&self, other: &Self) -> bool {
        self.plan_id == other.plan_id && self.index == other.index && self.request == other.request
    }
}

impl Eq for SyntheticRepositoryDemandKey {}

impl Hash for SyntheticRepositoryDemandKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.plan_id.hash(state);
        self.index.hash(state);
    }
}

impl fmt::Display for SyntheticRepositoryDemandKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "synthetic-repository-demand:{}:{}",
            self.plan_id, self.index
        )
    }
}

impl fmt::Display for SyntheticEventKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "synthetic-{:?}-event:{}", self.kind, self.plan_id)
    }
}

#[async_trait]
impl Key for SyntheticRepositoryDemandKey {
    type Value = ();

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }

    fn provide<'a>(&'a self, demand: &mut dice::Demand<'a>) {
        demand.provide_value_with(|| self.request.clone());
    }
}

#[async_trait]
impl Key for SyntheticEventKey {
    type Value = ();

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::from_events([EvaluationEvent::StarlarkPrint {
            location: slug_events_v2::StarlarkSourceLocation::new(Arc::from("synthetic.bzl"), 1, 6),
            text: self.text.as_ref().into(),
        }]))
        .expect("synthetic event capture is installed");
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }
}

async fn compute_synthetic_command_root(
    plan: &SyntheticCommandPlan,
    ctx: &mut DiceComputations<'_>,
) -> SyntheticCommandOutcome {
    let workspace_path = plan.workspace.as_path().to_path_buf();
    // Every attempt depends on the entire immutable command input bundle.
    // The values themselves are consumed by real production keys later; this
    // private root proves the retry driver does not silently mix snapshots.
    ctx.compute(&NativeDemandWorkspaceRevisionKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic command workspace revision is injected");
    ctx.compute(&RegistryRequestGenerationKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic command registry generation is injected");
    ctx.compute(&RepositoryMaterializationGenerationKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic command repository generation is injected");
    ctx.compute(&RootModuleCommandPolicyKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic command policy is injected");
    ctx.compute(&RootModuleEnvironmentPolicyKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic environment policy is injected");
    ctx.compute(&RootModuleLockfileModeKey {
        workspace: workspace_path.clone(),
    })
    .await
    .expect("synthetic lockfile mode is injected");
    ctx.compute(&RootModuleRegistryUrlsKey {
        workspace: workspace_path,
    })
    .await
    .expect("synthetic registry URLs are injected");
    match plan.behavior {
        SyntheticRootBehavior::Normal => {}
        SyntheticRootBehavior::PanicAfterInputs => {
            panic!("forced synthetic command root panic");
        }
        SyntheticRootBehavior::PendForCancellation => {
            std::future::pending::<()>().await;
            unreachable!("a pending synthetic command root cannot complete");
        }
    }

    ctx.store_evaluation_data(EventBatch::empty())
        .expect("synthetic root event capture is installed");
    for (index, request) in plan.repositories.iter().enumerate() {
        ctx.compute(&SyntheticRepositoryDemandKey {
            plan_id: plan.id,
            index,
            request: request.clone(),
        })
        .await
        .expect("synthetic repository demand computes");
    }

    let epoch = ctx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: plan.workspace.clone(),
        })
        .await
        .expect("synthetic command repository epoch is injected");
    let expected_epoch = RepositoryMaterializationResultEpoch::new(
        plan.workspace.clone(),
        plan.repositories.iter().map(|request| {
            slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                request: request.clone(),
                result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                    slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
                ),
            }
        }),
    )
    .expect("synthetic repository requests are distinct");
    if epoch != expected_epoch {
        let mut needs: Option<slug_bzlmod_v2::SourcePreparationNeeds> = None;
        for request in plan.repositories.iter() {
            let repository_need =
                slug_bzlmod_v2::SourcePreparationNeeds::repository(request.as_ref().clone());
            needs = Some(match needs {
                Some(existing) => existing
                    .try_union(&repository_need)
                    .expect("synthetic repository requests are distinct"),
                None => repository_need,
            });
        }
        if let Some(path) = plan.paths.first() {
            if let PathOutcome::Need(path_need) = ctx
                .compute(&PathObservationKey::new(path.clone()))
                .await
                .expect("synthetic path projection computes")
            {
                let path_need = slug_bzlmod_v2::SourcePreparationNeeds::path(path_need);
                needs = Some(match needs {
                    Some(existing) => existing
                        .try_union(&path_need)
                        .expect("repository and path needs compose"),
                    None => path_need,
                });
            }
        }
        if let Some(text) = &plan.retry_event {
            ctx.compute(&SyntheticEventKey {
                plan_id: plan.id,
                kind: SyntheticEventKind::Retry,
                text: text.clone(),
            })
            .await
            .expect("synthetic retry event computes");
        }
        return slug_bzlmod_v2::SourcePreparationOutcome::Need(
            needs.expect("a mismatched repository epoch has a repository need"),
        );
    }

    for path in plan.paths.iter() {
        if let PathOutcome::Need(need) = ctx
            .compute(&PathObservationKey::new(path.clone()))
            .await
            .expect("synthetic path projection computes")
        {
            if let Some(text) = &plan.retry_event {
                ctx.compute(&SyntheticEventKey {
                    plan_id: plan.id,
                    kind: SyntheticEventKind::Retry,
                    text: text.clone(),
                })
                .await
                .expect("synthetic retry event computes");
            }
            return slug_bzlmod_v2::SourcePreparationOutcome::Need(
                slug_bzlmod_v2::SourcePreparationNeeds::path(need),
            );
        }
    }

    if let Some(text) = &plan.terminal_event {
        ctx.compute(&SyntheticEventKey {
            plan_id: plan.id,
            kind: SyntheticEventKind::Terminal,
            text: text.clone(),
        })
        .await
        .expect("synthetic terminal event computes");
    }
    slug_bzlmod_v2::SourcePreparationOutcome::Complete(plan.terminal.clone())
}

macro_rules! impl_synthetic_root_key {
    ($key:ty) => {
        #[async_trait]
        impl Key for $key {
            type Value = SyntheticCommandOutcome;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                _cancellations: &CancellationContext,
            ) -> Self::Value {
                compute_synthetic_command_root(&self.plan, ctx).await
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                x.complete_eq(y)
            }

            fn validity(value: &Self::Value) -> bool {
                value.is_complete()
            }
        }
    };
}

impl_synthetic_root_key!(SyntheticBuildRootKey);
impl_synthetic_root_key!(SyntheticQueryRootKey);

#[async_trait]
impl NativeCommandRoot for SyntheticCommandRoot {
    type Terminal = Result<SyntheticCommandValue, SyntheticCommandError>;

    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<SyntheticCommandOutcome, NativeDemandSessionError> {
        if self.plan().behavior == SyntheticRootBehavior::PendForCancellation {
            return match self {
                Self::Build(key) => poll_then_cancel_synthetic_compute(transaction, key).await,
                Self::Query(key) => poll_then_cancel_synthetic_compute(transaction, key).await,
            };
        }
        match self {
            Self::Build(key) => transaction.compute(key).await,
            Self::Query(key) => transaction.compute(key).await,
        }
        .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error:#}")))
    }
}

impl SyntheticCommandRoot {
    fn plan(&self) -> &SyntheticCommandPlan {
        match self {
            Self::Build(key) => &key.plan,
            Self::Query(key) => &key.plan,
        }
    }
}

#[async_trait]
impl NativeCommandRoot for RootQueryCommandKey {
    type Terminal = Arc<Result<QueryOutput, QueryError>>;

    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError>
    {
        transaction
            .compute(self)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error:#}")))
    }
}

#[async_trait]
impl NativeCommandRoot for BuildCommandRootKey {
    type Terminal = Arc<Result<BuildCommandEvaluation, BuildCommandError>>;

    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        matches!(
            terminal.as_ref(),
            Err(BuildCommandError {
                kind: BuildCommandErrorKind::Analysis(_)
            })
        )
    }

    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError>
    {
        transaction
            .compute(self)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error:#}")))
    }
}

#[async_trait]
impl NativeCommandRoot for CqueryCommandRoot {
    type Terminal = Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>;

    fn allows_empty_terminal(&self) -> bool {
        true
    }

    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        matches!(
            terminal.as_ref(),
            Err(CqueryCommandError::MissingTarget { .. }
                | CqueryCommandError::ExecutableRuleMissingExecutable(_)
                | CqueryCommandError::Analysis(_))
        )
    }

    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError>
    {
        if let Err(error) = preflight_cquery_query(&self.expression) {
            return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                Arc::new(Err(CqueryCommandError::from_evaluator_error(error))),
            ));
        }
        let mut needs: Option<slug_bzlmod_v2::SourcePreparationNeeds> = None;
        let mut results = Vec::with_capacity(self.roots.len());
        for root in self.roots.iter() {
            let prepared = prepare_configured_node_analysis(
                transaction,
                root.workspace.dupe(),
                root.canonical.clone(),
                root.base_configuration.clone(),
                root.explicit_root_string_setting.clone(),
            )
            .await;
            match prepared {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(next) => {
                    needs = Some(match needs {
                        Some(current) => current.try_union(&next).map_err(|error| {
                            NativeDemandSessionError::Computation(anyhow::anyhow!(
                                "incompatible cquery preparation needs: {error:?}"
                            ))
                        })?,
                        None => next,
                    });
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error)) => {
                    results.push(Arc::new(Err(error)));
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(analysis_key)) => {
                    match transaction.compute(&analysis_key).await.map_err(|error| {
                        NativeDemandSessionError::Computation(anyhow::anyhow!("{error:#}"))
                    })? {
                        slug_bzlmod_v2::SourcePreparationOutcome::Need(next) => {
                            needs = Some(match needs {
                                Some(current) => current.try_union(&next).map_err(|error| {
                                    NativeDemandSessionError::Computation(anyhow::anyhow!(
                                        "incompatible cquery analysis needs: {error:?}"
                                    ))
                                })?,
                                None => next,
                            });
                        }
                        slug_bzlmod_v2::SourcePreparationOutcome::Complete(result) => {
                            results.push(result)
                        }
                    }
                }
            }
        }
        if let Some(needs) = needs {
            return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(needs));
        }

        let mut targets = Vec::with_capacity(self.roots.len());
        let mut analyses = Vec::with_capacity(self.roots.len());
        for (root, result) in self.roots.iter().zip(results) {
            let analysis = match result.as_ref() {
                Ok(analysis) => analysis,
                Err(error) => {
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                        Arc::new(Err(root.map_analysis_error(error))),
                    ));
                }
            };
            let target = match cquery_result_target(analysis.dupe(), root.requested.clone()) {
                Ok(target) => target,
                Err(error) => {
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                        Arc::new(Err(error)),
                    ));
                }
            };
            targets.push(target);
            analyses.push(analysis.dupe());
        }

        let mut node_indices = SmallMap::new();
        for (index, analysis) in analyses.iter().enumerate() {
            node_indices.insert(analysis.key().clone(), index);
        }
        if let Some(deps) = self.expression.cquery_preactivation_deps_spec() {
            if self.include_implicit {
                return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                    Arc::new(Err(CqueryCommandError::request(
                        "cquery deps() currently requires --noimplicit_deps",
                    ))),
                ));
            }
            let root_index = self
                .literal_roots
                .iter()
                .find(|(literal, _)| literal.as_ref() == deps.target())
                .map(|(_, index)| *index)
                .expect("validated cquery deps root literal");
            let root = &self.roots[root_index];
            match compute_cquery_deps_closure(
                transaction,
                &root.workspace,
                analyses[root_index].dupe(),
                deps.depth(),
                self.include_tool,
            )
            .await?
            {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) => {
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(needs));
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error)) => {
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                        Arc::new(Err(error)),
                    ));
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok((nodes, indices))) => {
                    analyses = nodes;
                    node_indices = indices;
                }
            }
        }
        if let Some(raw_seed) = self.expression.cquery_rdeps_seed() {
            let TargetPattern::Single(seed) =
                TargetPattern::parse(raw_seed).expect("validated cquery rdeps seed is one target")
            else {
                unreachable!("validated cquery rdeps seed is concrete")
            };
            if !seed.repo().is_root() {
                return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                    Arc::new(Err(CqueryCommandError::request(
                        "cquery rdeps() seed must be in the root repository",
                    ))),
                ));
            }
            let workspace = self
                .roots
                .first()
                .expect("rdeps universe has one root")
                .workspace
                .dupe();
            let package = match transaction
                .compute(&RootPackageLoadKey::new(workspace, seed.package().clone()))
                .await
                .map_err(|error| {
                    NativeDemandSessionError::Computation(anyhow::anyhow!("{error:#}"))
                })? {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need));
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                    Ok(package) => package.clone(),
                    Err(error) => {
                        return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                            Arc::new(Err(CqueryCommandError::Analysis(AnalysisError::message(
                                error.to_string(),
                            )))),
                        ));
                    }
                },
            };
            if !package
                .targets
                .iter()
                .any(|target| target.name == seed.target().as_str())
            {
                let label =
                    CanonicalLabel::parse(&format!("@@//{}:{}", seed.package(), seed.target()))
                        .expect("validated root seed has a canonical projection");
                return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                    Arc::new(Err(CqueryCommandError::MissingTarget {
                        requested: Arc::from(raw_seed),
                        label,
                        build_file: package.build_file.clone(),
                    })),
                ));
            }
        }
        let mut environment =
            CquerySetEnvironment::new(&self.literal_roots, &targets, analyses.into(), node_indices);
        let terminal = evaluate_cquery_query(&mut environment, &self.expression)
            .await
            .map(|targets| CqueryCommandEvaluation { targets })
            .map_err(CqueryCommandError::from_evaluator_error);
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
            Arc::new(terminal),
        ))
    }
}

type CqueryDepsClosureOutcome = slug_bzlmod_v2::SourcePreparationOutcome<
    Result<
        (
            Vec<Arc<ConfiguredNodeResult>>,
            SmallMap<ConfiguredNodeKey, usize>,
        ),
        CqueryCommandError,
    >,
>;

fn cquery_result_target(
    analysis: Arc<ConfiguredNodeResult>,
    display_label: Arc<str>,
) -> Result<CqueryResultTarget, CqueryCommandError> {
    let projection = match analysis.key().configured_target() {
        Some(configured) => Some(
            configured
                .configuration()
                .slug_configuration()
                .ok_or_else(|| {
                    CqueryCommandError::infrastructure(
                        "production cquery analysis returned an opaque configuration",
                    )
                })?
                .projection(),
        ),
        None => None,
    };
    Ok(CqueryResultTarget {
        key: analysis.key().clone(),
        analysis,
        display_label,
        projection,
    })
}

fn cquery_label_mode_label(key: &ConfiguredNodeKey) -> Arc<str> {
    let canonical = key.label().to_string();
    Arc::from(canonical.strip_prefix("@@").unwrap_or(&canonical))
}

async fn compute_cquery_deps_closure(
    transaction: &mut dice::DiceTransaction,
    workspace: &NormalizedAbsolutePath,
    root: Arc<ConfiguredNodeResult>,
    depth: Option<i32>,
    include_tool: bool,
) -> Result<CqueryDepsClosureOutcome, NativeDemandSessionError> {
    let mut nodes = vec![root];
    let mut node_indices = SmallMap::new();
    node_indices.insert(nodes[0].key().clone(), 0);
    let mut frontier = vec![nodes[0].key().clone()];
    let mut level = 0;

    while !frontier.is_empty() {
        // Bazel's deps(root, 0) returns the root without observing its edges.
        if depth.is_some_and(|limit| level >= limit) {
            break;
        }

        let mut next = Vec::new();
        let mut next_seen = SmallSet::new();
        for node in &frontier {
            let index = *node_indices
                .get(node)
                .expect("cquery frontier must refer to an activated node");
            for edge in nodes[index].edges() {
                if edge.tool() {
                    let request_mode = if include_tool {
                        "requested"
                    } else {
                        "filtered"
                    };
                    return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                        CqueryCommandError::infrastructure(format!(
                            "cquery deps() tool dependency traversal is unsupported ({request_mode})"
                        )),
                    )));
                }
                if edge.implicit() {
                    continue;
                }
                let target = edge.target().clone();
                if node_indices.contains_key(&target) || !next_seen.insert(target.clone()) {
                    continue;
                }
                next.push(target);
            }
        }
        if next.is_empty() {
            break;
        }

        let outcomes = transaction
            .compute_join(next, |ctx, node| {
                Box::pin(async move {
                    let key = ConfiguredNodeAnalysisKey::new(workspace.dupe(), node.clone())
                        .map_err(|error| error.to_string());
                    let outcome = match key {
                        Ok(key) => ctx.compute(&key).await.map_err(|error| error.to_string()),
                        Err(error) => Err(error),
                    };
                    (node, outcome)
                })
            })
            .await;

        let mut needs: Option<slug_bzlmod_v2::SourcePreparationNeeds> = None;
        let mut first_error = None;
        let mut completed = Vec::with_capacity(outcomes.len());
        for (node, outcome) in outcomes {
            let outcome = outcome.map_err(|error| {
                NativeDemandSessionError::Computation(anyhow::anyhow!(
                    "cquery deps analysis failed: {error}"
                ))
            })?;
            match outcome {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                    needs = Some(match needs {
                        Some(current) => current.try_union(&need).map_err(|error| {
                            NativeDemandSessionError::Computation(anyhow::anyhow!(
                                "incompatible cquery deps analysis needs: {error:?}"
                            ))
                        })?,
                        None => need,
                    });
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                    Ok(analysis) => completed.push((node, analysis.dupe())),
                    Err(error) if first_error.is_none() => first_error = Some(error.clone()),
                    Err(_) => {}
                },
            }
        }
        if let Some(needs) = needs {
            return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(needs));
        }
        if let Some(error) = first_error {
            return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                CqueryCommandError::Analysis(error),
            )));
        }

        frontier = Vec::with_capacity(completed.len());
        for (node, analysis) in completed {
            assert_eq!(analysis.key(), &node, "cquery deps analysis key mismatch");
            let index = nodes.len();
            node_indices.insert(node.clone(), index);
            nodes.push(analysis);
            frontier.push(node);
        }
        level += 1;
    }

    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok((
        nodes,
        node_indices,
    ))))
}

impl CqueryRootTarget {
    fn map_analysis_error(&self, error: &AnalysisError) -> CqueryCommandError {
        match error.kind() {
            AnalysisErrorKind::TargetNotFound { label, build_file } if label == &self.canonical => {
                CqueryCommandError::MissingTarget {
                    requested: self.requested.clone(),
                    label: label.clone(),
                    build_file: build_file.clone(),
                }
            }
            AnalysisErrorKind::ExecutableRuleMissingExecutable { .. } => {
                CqueryCommandError::ExecutableRuleMissingExecutable(error.clone())
            }
            _ => CqueryCommandError::Analysis(error.clone()),
        }
    }
}

async fn poll_then_cancel_synthetic_compute<K>(
    transaction: &mut dice::DiceTransaction,
    key: &K,
) -> Result<SyntheticCommandOutcome, NativeDemandSessionError>
where
    K: Key<Value = SyntheticCommandOutcome>,
{
    let mut compute = Box::pin(transaction.compute(key));
    std::future::poll_fn(|context| match compute.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("the cancellation seam's synthetic root unexpectedly completed"),
    })
    .await;
    drop(compute);
    Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
        "synthetic root compute future cancelled after its first pending poll"
    )))
}

#[allow(dead_code)]
impl NativeDemandSessionOwner {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        let generations = NativeDemandGenerationBundle {
            workspace_revision: WorkspaceRevision(0),
            registry: RegistryRequestGeneration(0),
            repository: RepositoryMaterializationGeneration(0),
        };
        Self {
            state: Mutex::new(NativeDemandSessionState {
                next_lease: 1,
                phase: NativeDemandLeasePhase::Idle,
                accepted: AcceptedNativeDemandSnapshot {
                    inputs: NativeDemandInputBundle {
                        request: NativeDemandRequestInputBundle::normalized_initial(),
                        generations,
                    },
                    repository_results: RepositoryMaterializationResultEpoch::new(workspace, [])
                        .expect("empty repository epoch is valid"),
                    path_observations: PathObservationEpoch::empty(),
                    selected: SelectedWorkspaceDemands::empty(),
                },
                #[cfg(test)]
                fail_next_restoration: false,
                #[cfg(test)]
                fail_next_selected_injection: false,
                #[cfg(test)]
                fail_next_replace_accepted: false,
                #[cfg(test)]
                fail_next_close: false,
                #[cfg(test)]
                trace: Vec::new(),
            }),
        }
    }

    fn acquire(
        &self,
    ) -> Result<(NativeDemandLeaseToken, AcceptedNativeDemandSnapshot), NativeDemandSessionError>
    {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        if !matches!(state.phase, NativeDemandLeasePhase::Idle) {
            return Err(NativeDemandSessionError::Busy);
        }
        let current = state.next_lease;
        if current == 0 {
            return Err(NativeDemandSessionError::LeaseTokenExhausted);
        }
        state.next_lease = current
            .checked_add(1)
            .ok_or(NativeDemandSessionError::LeaseTokenExhausted)?;
        let lease = NativeDemandLeaseToken(current);
        state.phase = NativeDemandLeasePhase::Open {
            lease,
            repository: None,
        };
        Ok((lease, state.accepted.clone()))
    }

    fn attach_repository(
        &self,
        lease: NativeDemandLeaseToken,
        repository: super::repository_io::RepositorySessionToken,
    ) -> Result<(), NativeDemandSessionError> {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        match &mut state.phase {
            NativeDemandLeasePhase::Open {
                lease: active,
                repository: slot,
            } if *active == lease && slot.is_none() => {
                *slot = Some(repository);
                Ok(())
            }
            _ => Err(NativeDemandSessionError::StaleLease),
        }
    }

    fn replace_accepted(
        &self,
        lease: NativeDemandLeaseToken,
        snapshot: AcceptedNativeDemandSnapshot,
    ) -> Result<(), NativeDemandSessionError> {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        #[cfg(test)]
        if std::mem::take(&mut state.fail_next_replace_accepted) {
            return Err(NativeDemandSessionError::StaleLease);
        }
        match state.phase {
            NativeDemandLeasePhase::Open { lease: active, .. } if active == lease => {
                state.accepted = snapshot;
                Ok(())
            }
            _ => Err(NativeDemandSessionError::StaleLease),
        }
    }

    fn close(&self, lease: NativeDemandLeaseToken) -> Result<(), NativeDemandSessionError> {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        #[cfg(test)]
        if std::mem::take(&mut state.fail_next_close) {
            return Err(NativeDemandSessionError::StaleLease);
        }
        match state.phase {
            NativeDemandLeasePhase::Open { lease: active, .. } if active == lease => {
                state.phase = NativeDemandLeasePhase::Idle;
                Ok(())
            }
            _ => Err(NativeDemandSessionError::StaleLease),
        }
    }

    #[cfg(test)]
    fn force_next_restoration_failure(&self) {
        self.state
            .lock()
            .expect("native demand session mutex poisoned")
            .fail_next_restoration = true;
    }

    #[cfg(test)]
    fn force_next_selected_injection_failure(&self) {
        self.state
            .lock()
            .expect("native demand session mutex poisoned")
            .fail_next_selected_injection = true;
    }

    #[cfg(test)]
    fn force_next_replace_accepted_failure(&self) {
        self.state
            .lock()
            .expect("native demand session mutex poisoned")
            .fail_next_replace_accepted = true;
    }

    #[cfg(test)]
    fn force_next_close_failure(&self) {
        self.state
            .lock()
            .expect("native demand session mutex poisoned")
            .fail_next_close = true;
    }

    #[cfg(test)]
    fn take_selected_injection_failure(
        &self,
        lease: NativeDemandLeaseToken,
    ) -> Result<bool, NativeDemandSessionError> {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        if !matches!(
            state.phase,
            NativeDemandLeasePhase::Open { lease: active, .. } if active == lease
        ) {
            return Err(NativeDemandSessionError::StaleLease);
        }
        Ok(std::mem::take(&mut state.fail_next_selected_injection))
    }

    #[cfg(test)]
    fn record_trace(&self, event: NativeDemandTestTrace) {
        self.state
            .lock()
            .expect("native demand session mutex poisoned")
            .trace
            .push(event);
    }

    #[cfg(test)]
    fn take_trace(&self) -> Vec<NativeDemandTestTrace> {
        std::mem::take(
            &mut self
                .state
                .lock()
                .expect("native demand session mutex poisoned")
                .trace,
        )
    }

    #[cfg(test)]
    fn take_restoration_failure(
        &self,
        lease: NativeDemandLeaseToken,
    ) -> Result<bool, NativeDemandSessionError> {
        let mut state = self
            .state
            .lock()
            .expect("native demand session mutex poisoned");
        if !matches!(
            state.phase,
            NativeDemandLeasePhase::Open { lease: active, .. } if active == lease
        ) {
            return Err(NativeDemandSessionError::StaleLease);
        }
        Ok(std::mem::take(&mut state.fail_next_restoration))
    }
}

/// Stage 4 package-loading evidence attached to a requested target pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedPackageEvaluation {
    pub target_pattern: String,
    pub package: LoadedPackage,
    pub analysis: Option<ConfiguredNodeResult>,
    pub revision: WorkspaceRevision,
}

/// The V2 runtime result after the first configured-rule analysis packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBuildEvaluation {
    pub workspace: WorkspaceEvaluation,
    pub root_module_graph: Arc<RootModuleGraph>,
    pub packages: Vec<RequestedPackageEvaluation>,
    pub revision: WorkspaceRevision,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct BuildCommandRootKey {
    workspace: NormalizedAbsolutePath,
    targets: Arc<[Arc<str>]>,
    configuration: ConfigurationKey,
    base_configuration: ConfigurationKey,
    explicit_root_string_setting: Option<RootStringSettingValue>,
}

#[derive(Clone)]
struct CqueryCommandRoot {
    expression: QueryExpression,
    roots: Arc<[CqueryRootTarget]>,
    literal_roots: Arc<[(Arc<str>, usize)]>,
    include_implicit: bool,
    include_tool: bool,
}

#[derive(Clone)]
struct CqueryRootTarget {
    requested: Arc<str>,
    canonical: CanonicalLabel,
    workspace: NormalizedAbsolutePath,
    base_configuration: ConfigurationKey,
    explicit_root_string_setting: Option<RootStringSettingValue>,
}

#[derive(Debug, Clone, Allocative)]
pub struct CqueryCommandEvaluation {
    targets: TargetSet<CqueryResultTarget>,
}

#[derive(Debug, Clone, Allocative)]
struct CqueryResultTarget {
    key: ConfiguredNodeKey,
    analysis: Arc<ConfiguredNodeResult>,
    display_label: Arc<str>,
    projection: Option<SlugConfigurationProjection>,
}

impl PartialEq for CqueryResultTarget {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for CqueryResultTarget {}

impl Hash for CqueryResultTarget {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

struct CquerySetEnvironment {
    targets: SmallMap<Arc<str>, TargetSet<CqueryResultTarget>>,
    nodes: Arc<[Arc<ConfiguredNodeResult>]>,
    node_indices: SmallMap<ConfiguredNodeKey, usize>,
}

impl CquerySetEnvironment {
    fn new(
        literal_roots: &[(Arc<str>, usize)],
        roots: &[CqueryResultTarget],
        nodes: Arc<[Arc<ConfiguredNodeResult>]>,
        node_indices: SmallMap<ConfiguredNodeKey, usize>,
    ) -> Self {
        let mut targets = SmallMap::new();
        for (literal, root) in literal_roots {
            targets.insert(literal.clone(), TargetSet::singleton(roots[*root].clone()));
        }
        Self {
            targets,
            nodes,
            node_indices,
        }
    }

    fn union_all(sets: &[TargetSet<CqueryResultTarget>]) -> TargetSet<CqueryResultTarget> {
        let mut result = TargetSet::default();
        for set in sets {
            for target in set.iter() {
                result.insert(target.clone());
            }
        }
        result
    }
}

fn is_cquery_executable_non_test(capability: Option<&slug_loading_v2::RuleCapability>) -> bool {
    capability.is_some_and(|capability| {
        capability.executable && !capability.rule_class.ends_with("_test")
    })
}

fn filter_cquery_executable_non_tests<T>(
    targets: &TargetSet<T>,
    is_executable_non_test: impl Fn(&T) -> bool,
) -> TargetSet<T>
where
    T: Clone + Eq + Hash,
{
    let mut result = TargetSet::default();
    for target in targets.iter() {
        if is_executable_non_test(target) {
            result.insert(target.clone());
        }
    }
    result
}

fn filter_cquery_kind<T>(
    targets: &TargetSet<T>,
    matches: impl Fn(&T) -> Result<bool, QueryError>,
) -> Result<TargetSet<T>, QueryError>
where
    T: Clone + Eq + Hash,
{
    let mut result = TargetSet::default();
    for target in targets.iter() {
        if matches(target)? {
            result.insert(target.clone());
        }
    }
    Ok(result)
}

fn cquery_post_analysis_siblings<T>(targets: &TargetSet<T>) -> Result<TargetSet<T>, QueryError>
where
    T: Clone + Eq + Hash,
{
    if targets.iter().next().is_none() {
        Ok(targets.clone())
    } else {
        Err(QueryError::evaluation(
            "siblings() not supported for post analysis queries",
        ))
    }
}

fn cquery_post_analysis_visible<T>(
    callers: &TargetSet<T>,
    targets: &TargetSet<T>,
) -> Result<TargetSet<T>, QueryError>
where
    T: Clone + Eq + Hash,
{
    if callers.iter().next().is_none() || targets.iter().next().is_none() {
        Ok(targets.clone())
    } else {
        Err(QueryError::evaluation(
            "visible() is not supported on configured targets",
        ))
    }
}

#[async_trait]
impl CqueryQueryEnvironment for CquerySetEnvironment {
    type Set = TargetSet<CqueryResultTarget>;
    type VisibleCallers = TargetSet<CqueryResultTarget>;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
        Self::union_all(sets)
    }

    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
        Self::union_all(&[left, right])
    }

    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        let mut result = TargetSet::default();
        for target in left.iter() {
            if right.contains(target) {
                result.insert(target.clone());
            }
        }
        result
    }

    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        let mut result = TargetSet::default();
        for target in left.iter() {
            if !right.contains(target) {
                result.insert(target.clone());
            }
        }
        result
    }

    fn select_some(&self, targets: &Self::Set, count: i32) -> Result<Self::Set, QueryError> {
        let mut selected = TargetSet::default();
        if count > 0 {
            for target in targets.iter().take(count as usize) {
                selected.insert(target.clone());
            }
        }
        if selected.iter().next().is_none() {
            Err(QueryError::evaluation("argument set is empty"))
        } else {
            Ok(selected)
        }
    }

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
        self.targets
            .get(literal)
            .cloned()
            .ok_or_else(|| QueryError::evaluation(format!("unresolved cquery literal '{literal}'")))
    }

    async fn deps(
        &mut self,
        targets: &Self::Set,
        depth: Option<i32>,
    ) -> Result<Self::Set, QueryError> {
        let mut result = targets.clone();
        if depth == Some(0) {
            return Ok(result);
        }

        let mut seen = SmallSet::new();
        let mut frontier = Vec::new();
        for target in targets.iter() {
            if seen.insert(target.key.clone()) {
                frontier.push(target.key.clone());
            }
        }
        let mut level = 0;
        while !frontier.is_empty() {
            if depth.is_some_and(|limit| level >= limit) {
                break;
            }
            let mut next = Vec::new();
            for key in frontier {
                let index = *self.node_indices.get(&key).ok_or_else(|| {
                    QueryError::evaluation(format!(
                        "cquery deps() target '{key}' was not preactivated"
                    ))
                })?;
                for edge in self.nodes[index].edges() {
                    if edge.tool() {
                        return Err(QueryError::evaluation(
                            "cquery deps() tool dependency traversal is unsupported",
                        ));
                    }
                    if edge.implicit() {
                        continue;
                    }
                    let child = edge.target().clone();
                    if !seen.insert(child.clone()) {
                        continue;
                    }
                    let child_index = *self.node_indices.get(&child).ok_or_else(|| {
                        QueryError::evaluation(format!(
                            "cquery deps() target '{child}' was not preactivated"
                        ))
                    })?;
                    let child_target = cquery_result_target(
                        self.nodes[child_index].dupe(),
                        cquery_label_mode_label(&child),
                    )
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;
                    result.insert(child_target);
                    next.push(child);
                }
            }
            frontier = next;
            level += 1;
        }
        Ok(result)
    }

    async fn rdeps(
        &mut self,
        universe: &Self::Set,
        from: &str,
        depth: Option<i32>,
    ) -> Result<Self::Set, QueryError> {
        let TargetPattern::Single(from) =
            TargetPattern::parse(from).map_err(QueryError::evaluation)?
        else {
            return Err(QueryError::evaluation(
                "cquery rdeps() seed must be one target",
            ));
        };
        if !from.repo().is_root() {
            return Err(QueryError::evaluation(
                "cquery rdeps() seed must be in the root repository",
            ));
        }
        let from = CanonicalLabel::parse(&format!("@@//{}:{}", from.package(), from.target()))
            .map_err(QueryError::evaluation)?;
        let mut result = TargetSet::default();
        if depth.is_some_and(|depth| depth < 0) {
            return Ok(result);
        }
        let mut frontier = SmallSet::new();
        for target in universe.iter() {
            if target.key.label() == &from {
                result.insert(target.clone());
                frontier.insert(target.key.clone());
            }
        }
        let mut level = 0;
        while !frontier.is_empty() && depth.is_none_or(|depth| level < depth as usize) {
            let mut next = SmallSet::new();
            for parent in universe.iter() {
                if result.contains(parent) {
                    continue;
                }
                for edge in parent.analysis.edges() {
                    if edge.tool() {
                        return Err(QueryError::evaluation(
                            "cquery rdeps() tool dependency traversal is unsupported",
                        ));
                    }
                    if !edge.implicit() && frontier.contains(edge.target()) {
                        result.insert(parent.clone());
                        next.insert(parent.key.clone());
                        break;
                    }
                }
            }
            frontier = next;
            level += 1;
        }
        Ok(result)
    }

    async fn siblings(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        cquery_post_analysis_siblings(targets)
    }

    fn materialize_visible_callers(&self, callers: &Self::Set) -> Self::VisibleCallers {
        callers.clone()
    }

    async fn visible(
        &mut self,
        callers: &Self::VisibleCallers,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        cquery_post_analysis_visible(callers, targets)
    }

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        Ok(filter_cquery_executable_non_tests(targets, |target| {
            is_cquery_executable_non_test(target.analysis.rule_capability())
        }))
    }

    async fn kind(
        &mut self,
        regex: &regex::Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        filter_cquery_kind(targets, |target| {
            let candidate = cquery_target_kind_for_query(
                target.analysis.kind(),
                target.analysis.rule_capability(),
            )?;
            Ok(regex.find(candidate.as_str()).is_some())
        })
    }

    async fn filter(
        &mut self,
        regex: &regex::Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut result = TargetSet::default();
        for target in targets.iter() {
            if regex.find(&target.display_label).is_some() {
                result.insert(target.clone());
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum CqueryCommandError {
    MissingTarget {
        requested: Arc<str>,
        label: CanonicalLabel,
        build_file: PathBuf,
    },
    Evaluation(Arc<str>),
    ExecutableRuleMissingExecutable(AnalysisError),
    Analysis(AnalysisError),
    Request(Arc<str>),
    Infrastructure(Arc<str>),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum BuildCommandRequestError {
    ExternalRepository { pattern: Arc<str> },
    RecursivePattern { pattern: Arc<str> },
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct BuildCommandEvaluation {
    anchor: RootModuleLoadingAnchor,
    targets: Arc<[BuildRequestedTarget]>,
    action_closure: Arc<[Arc<ConfiguredNodeResult>]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct BuildRequestedTarget {
    pattern: Arc<str>,
    package: LoadedPackage,
    analysis: Option<Arc<ConfiguredNodeResult>>,
    completion: BuildTargetCompletion,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
enum BuildTargetCompletion {
    Analyzed,
    ObservedExportedSource,
    LoadedOnly,
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct BuildCommandError {
    kind: BuildCommandErrorKind,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum BuildCommandErrorKind {
    RootAnchor(RootModuleLoadingAnchorError),
    Package(RootPackageLoadError),
    RepositoryRoute(RootRepositoryRouteError),
    RepositoryPackage(RepositoryPackageLoadError),
    RepositorySource(RepositorySourceFileError),
    SourceMissing(CanonicalLabel),
    RootSource(PathObservationResult),
    ExternalTargetKind,
    TargetNotFound {
        pattern: Arc<str>,
        package: PackagePath,
        target: TargetName,
        build_file: PathBuf,
    },
    Analysis(AnalysisError),
    ExternalRepository {
        pattern: Arc<str>,
    },
    RecursivePattern {
        pattern: Arc<str>,
    },
    Infrastructure(Arc<str>),
}

type BuildCommandOutcome = slug_bzlmod_v2::SourcePreparationOutcome<
    Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
>;

type BuildActionClosureOutcome = slug_bzlmod_v2::SourcePreparationOutcome<
    Result<Arc<[Arc<ConfiguredNodeResult>]>, BuildCommandError>,
>;
type BuildActionAnalysisOutcome =
    slug_bzlmod_v2::SourcePreparationOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;
type BuildActionFrontierOutcome =
    slug_bzlmod_v2::SourcePreparationOutcome<Result<Vec<Arc<ConfiguredNodeResult>>, AnalysisError>>;

enum BuildBranchResult {
    Outcome(
        slug_bzlmod_v2::SourcePreparationOutcome<Result<BuildRequestedTarget, BuildCommandError>>,
    ),
    Infrastructure(Arc<str>),
}

impl BuildCommandRootKey {
    fn new(
        workspace: NormalizedAbsolutePath,
        targets: &[TargetPattern],
        configuration: ConfigurationKey,
    ) -> Result<Self, BuildCommandRequestError> {
        let permits_one_external_single = matches!(
            targets,
            [TargetPattern::Single(label)] if !label.repo().is_root()
        );
        let mut canonical = Vec::with_capacity(targets.len());
        for target in targets {
            let pattern: Arc<str> = Arc::from(target.to_string());
            let repo = match target {
                TargetPattern::Single(label) => label.repo(),
                TargetPattern::PackageAll { repo, .. } | TargetPattern::Recursive { repo, .. } => {
                    repo
                }
            };
            if !repo.is_root() && !permits_one_external_single {
                return Err(BuildCommandRequestError::ExternalRepository { pattern });
            }
            if matches!(target, TargetPattern::Recursive { .. }) {
                return Err(BuildCommandRequestError::RecursivePattern { pattern });
            }
            canonical.push(pattern);
        }
        Ok(Self {
            workspace,
            targets: canonical.into(),
            base_configuration: configuration.clone(),
            configuration,
            explicit_root_string_setting: None,
        })
    }

    fn new_with_root_string_setting(
        workspace: NormalizedAbsolutePath,
        targets: &[TargetPattern],
        base_configuration: ConfigurationKey,
        explicit: Option<RootStringSettingValue>,
    ) -> Result<Self, BuildCommandRequestError> {
        let configuration = explicit.as_ref().map_or_else(
            || base_configuration.clone(),
            |value| base_configuration.with_root_string_setting(value.clone()),
        );
        let mut key = Self::new(workspace, targets, configuration)?;
        key.base_configuration = base_configuration;
        key.explicit_root_string_setting = explicit;
        Ok(key)
    }
}

impl BuildCommandEvaluation {
    pub fn loaded_package_count(&self) -> usize {
        self.targets.len()
    }

    pub fn analyzed_target_count(&self) -> usize {
        self.targets
            .iter()
            .filter(|target| target.analysis.is_some())
            .count()
    }

    pub fn declared_action_count(&self) -> usize {
        self.analyses()
            .map(|analysis| analysis.actions().len())
            .sum()
    }

    pub fn analyses(&self) -> impl Iterator<Item = &ConfiguredNodeResult> {
        self.action_closure.iter().map(Arc::as_ref)
    }

    pub fn is_observed_exported_source(&self) -> bool {
        matches!(
            self.targets.as_ref(),
            [BuildRequestedTarget {
                completion: BuildTargetCompletion::ObservedExportedSource,
                ..
            }]
        )
    }
}

impl CqueryCommandEvaluation {
    pub fn label_stdout(&self) -> String {
        self.targets
            .iter()
            .map(|target| match &target.projection {
                Some(projection) => format!("{} ({projection})\n", target.display_label),
                None => format!("{} (null)\n", target.display_label),
            })
            .collect()
    }

    pub fn label_kind_stdout(&self) -> Result<String, CqueryCommandError> {
        self.targets
            .iter()
            .map(|target| {
                Ok(format!(
                    "{} {}\n",
                    cquery_target_kind(target.analysis.kind(), target.analysis.rule_capability())?,
                    cquery_graph_label(target)
                ))
            })
            .collect()
    }

    pub fn starlark_label_stdout(&self) -> String {
        self.targets
            .iter()
            .map(|target| format!("{}\n", target.analysis.key().label()))
            .collect()
    }

    /// Render the selected configured targets through the shared unfactored
    /// DOT writer. The writer asks for successors by cursor; this callback
    /// scans the result-owned edge slice directly and retains no adjacency.
    pub fn graph_stdout(&self) -> String {
        let mut ordered = self
            .targets
            .iter()
            .map(|target| (target, cquery_graph_label(target)))
            .collect::<Vec<_>>();
        ordered.sort_unstable_by(|(left_target, left_label), (right_target, right_label)| {
            left_label
                .cmp(right_label)
                .then_with(|| left_target.key.cmp(&right_target.key))
        });

        let mut node_indices = SmallMap::with_capacity(ordered.len());
        let mut targets = Vec::with_capacity(ordered.len());
        let mut labels = Vec::with_capacity(ordered.len());
        for (index, (target, label)) in ordered.into_iter().enumerate() {
            node_indices.insert(
                target.key.clone(),
                u32::try_from(index).expect("cquery graph exceeds u32 node capacity"),
            );
            targets.push(target);
            labels.push(label);
        }
        render_unfactored_dot(&labels, |node, cursor| {
            cquery_graph_successor(&targets, &node_indices, node, cursor)
        })
    }

    pub fn analyses(&self) -> impl Iterator<Item = &ConfiguredNodeResult> {
        self.targets.iter().map(|target| target.analysis.as_ref())
    }
}

fn cquery_target_kind(
    kind: &ConfiguredNodeKind,
    capability: Option<&slug_loading_v2::RuleCapability>,
) -> Result<String, CqueryCommandError> {
    match capability {
        Some(capability) => Ok(format!("{} rule", capability.rule_class)),
        None => match kind {
            ConfiguredNodeKind::SourceFile => Ok("source file".to_owned()),
            ConfiguredNodeKind::GeneratedFile => Ok("generated file".to_owned()),
            ConfiguredNodeKind::PackageGroup => Ok("package group".to_owned()),
            kind => Err(CqueryCommandError::infrastructure(format!(
                "cquery label_kind has no target kind for {kind:?} without a rule capability"
            ))),
        },
    }
}

fn cquery_target_kind_for_query(
    kind: &ConfiguredNodeKind,
    capability: Option<&slug_loading_v2::RuleCapability>,
) -> Result<String, QueryError> {
    cquery_target_kind(kind, capability).map_err(|error| QueryError::syntax(error.to_string()))
}

fn cquery_graph_label(target: &CqueryResultTarget) -> String {
    match &target.projection {
        Some(projection) => format!("{} ({projection})", target.display_label),
        None => format!("{} (null)", target.display_label),
    }
}

fn cquery_graph_successor(
    targets: &[&CqueryResultTarget],
    node_indices: &SmallMap<ConfiguredNodeKey, u32>,
    node: u32,
    cursor: usize,
) -> Option<u32> {
    let target = targets.get(node as usize)?;
    let mut previous = None;
    for _ in 0..=cursor {
        let next = target
            .analysis
            .edges()
            .iter()
            .filter(|edge| !edge.implicit())
            .filter_map(|edge| node_indices.get(edge.target()).copied())
            .filter(|candidate| previous.is_none_or(|previous| *candidate > previous))
            .min()?;
        previous = Some(next);
    }
    previous
}

impl CqueryCommandError {
    fn request(message: impl Into<String>) -> Self {
        Self::Request(Arc::from(message.into()))
    }

    pub fn infrastructure(error: impl fmt::Display) -> Self {
        Self::Infrastructure(Arc::from(error.to_string()))
    }

    fn from_evaluator_error(error: QueryError) -> Self {
        if error.is_evaluation_failure() {
            Self::Evaluation(error.message)
        } else {
            Self::Request(error.message)
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::MissingTarget { .. }
            | Self::Evaluation(_)
            | Self::ExecutableRuleMissingExecutable(_) => 1,
            Self::Request(_) | Self::Analysis(_) | Self::Infrastructure(_) => 2,
        }
    }

    pub fn missing_stderr(&self) -> Option<String> {
        let Self::MissingTarget {
            requested,
            label,
            build_file,
        } = self
        else {
            return None;
        };
        let message = format!(
            "no such target '{requested}': target '{}' not declared in package '{}' defined by {}",
            label.target().as_str(),
            label.package().package(),
            build_file.display(),
        );
        Some(format!(
            "ERROR: Skipping '{requested}': {message}\nERROR: {message}\nERROR: Build did NOT complete successfully\n"
        ))
    }
}

impl fmt::Display for CqueryCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget {
                label, build_file, ..
            } => {
                write!(
                    f,
                    "target `{label}` was not found in {}",
                    build_file.display()
                )
            }
            Self::ExecutableRuleMissingExecutable(error) | Self::Analysis(error) => error.fmt(f),
            Self::Evaluation(message) | Self::Request(message) | Self::Infrastructure(message) => {
                f.write_str(message)
            }
        }
    }
}

impl fmt::Debug for BuildCommandEvaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildCommandEvaluation")
            .field("loaded_package_count", &self.loaded_package_count())
            .field("analyzed_target_count", &self.analyzed_target_count())
            .field("declared_action_count", &self.declared_action_count())
            .finish()
    }
}

impl BuildCommandError {
    fn new(kind: BuildCommandErrorKind) -> Self {
        Self { kind }
    }

    fn target_not_found(
        pattern: Arc<str>,
        package: PackagePath,
        target: TargetName,
        build_file: PathBuf,
    ) -> Self {
        Self {
            kind: BuildCommandErrorKind::TargetNotFound {
                pattern,
                package,
                target,
                build_file,
            },
        }
    }

    fn request(error: BuildCommandRequestError) -> Self {
        let kind = match error {
            BuildCommandRequestError::ExternalRepository { pattern } => {
                BuildCommandErrorKind::ExternalRepository { pattern }
            }
            BuildCommandRequestError::RecursivePattern { pattern } => {
                BuildCommandErrorKind::RecursivePattern { pattern }
            }
        };
        Self { kind }
    }

    pub(super) fn infrastructure(error: impl fmt::Display) -> Self {
        Self {
            kind: BuildCommandErrorKind::Infrastructure(Arc::from(error.to_string())),
        }
    }

    pub fn terminal_error(&self) -> (&'static str, i32) {
        match &self.kind {
            BuildCommandErrorKind::RepositoryPackage(error) if error.is_unsupported_feature() => {
                ("unsupported_feature", 7)
            }
            _ => ("build_runtime_error", 2),
        }
    }
}

impl fmt::Display for BuildCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            BuildCommandErrorKind::RootAnchor(error) => error.fmt(f),
            BuildCommandErrorKind::Package(error) => error.fmt(f),
            BuildCommandErrorKind::RepositoryRoute(error) => error.fmt(f),
            BuildCommandErrorKind::RepositoryPackage(error) => error.fmt(f),
            BuildCommandErrorKind::RepositorySource(error) => write!(f, "{error:?}"),
            BuildCommandErrorKind::SourceMissing(label) => {
                write!(f, "{label}: missing input file '{label}'")
            }
            BuildCommandErrorKind::RootSource(result) => {
                write!(f, "source observation failed: {result:?}")
            }
            BuildCommandErrorKind::ExternalTargetKind => {
                f.write_str("external build target is not an exported source file")
            }
            BuildCommandErrorKind::TargetNotFound {
                pattern,
                build_file,
                ..
            } => {
                write!(
                    f,
                    "target `{pattern}` was not found in {}",
                    build_file.display()
                )
            }
            BuildCommandErrorKind::Analysis(error) => error.fmt(f),
            BuildCommandErrorKind::ExternalRepository { pattern } => write!(
                f,
                "external repository target patterns are not supported before Stage 5 repository mapping: {pattern}"
            ),
            BuildCommandErrorKind::RecursivePattern { pattern } => write!(
                f,
                "recursive target patterns are not supported before Stage 6 analysis: {pattern}"
            ),
            BuildCommandErrorKind::Infrastructure(error) => f.write_str(error),
        }
    }
}

impl fmt::Debug for BuildCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BuildCommandError")
            .field(&self.to_string())
            .finish()
    }
}

impl std::error::Error for BuildCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            BuildCommandErrorKind::RootAnchor(error) => Some(error),
            BuildCommandErrorKind::Package(error) => Some(error),
            BuildCommandErrorKind::RepositoryRoute(error) => Some(error),
            BuildCommandErrorKind::RepositoryPackage(error) => Some(error),
            BuildCommandErrorKind::Analysis(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for BuildCommandRootKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "build-command-root:{}:{}",
            self.workspace,
            self.targets
                .iter()
                .map(|target| target.as_ref())
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn build_complete(
    result: Result<BuildCommandEvaluation, BuildCommandError>,
) -> BuildCommandOutcome {
    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Arc::new(result))
}

fn collect_build_branches(
    branches: Vec<BuildBranchResult>,
) -> Result<
    slug_bzlmod_v2::SourcePreparationOutcome<
        Result<Arc<[BuildRequestedTarget]>, BuildCommandError>,
    >,
    Arc<str>,
> {
    let mut needs: Option<slug_bzlmod_v2::SourcePreparationNeeds> = None;
    let mut first_error = None;
    let mut targets = Vec::with_capacity(branches.len());
    for branch in branches {
        match branch {
            BuildBranchResult::Infrastructure(error) => return Err(error),
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
                needs = Some(match needs {
                    Some(current) => current
                        .try_union(&need)
                        .map_err(|error| Arc::from(format!("{error:?}")))?,
                    None => need,
                });
            }
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
                target,
            ))) => targets.push(target),
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                Err(error),
            )) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(need) = needs {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need))
    } else if let Some(error) = first_error {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
            error,
        )))
    } else {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
            targets.into(),
        )))
    }
}

fn collect_build_action_frontier(
    outcomes: Vec<(ConfiguredTargetKey, BuildActionAnalysisOutcome)>,
) -> Result<BuildActionFrontierOutcome, Arc<str>> {
    let mut needs: Option<slug_bzlmod_v2::SourcePreparationNeeds> = None;
    let mut first_error = None;
    let mut layer = Vec::with_capacity(outcomes.len());
    for (configured_target, outcome) in outcomes {
        match outcome {
            slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                needs = Some(match needs {
                    Some(current) => current
                        .try_union(&need)
                        .map_err(|error| Arc::from(format!("{error:?}")))?,
                    None => need,
                });
            }
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(analysis) => {
                    assert_eq!(
                        analysis.configured_target_key(),
                        Some(&configured_target),
                        "root analysis returned a mismatched configured target"
                    );
                    layer.push(analysis.dupe());
                }
                Err(error) if first_error.is_none() => {
                    first_error = Some(error.clone());
                }
                Err(_) => {}
            },
        }
    }
    if let Some(need) = needs {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need))
    } else if let Some(error) = first_error {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
            error,
        )))
    } else {
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
            layer,
        )))
    }
}

async fn compute_build_action_closure(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    targets: &[BuildRequestedTarget],
) -> Result<BuildActionClosureOutcome, Arc<str>> {
    let mut seen = SmallSet::new();
    let mut closure = Vec::new();
    let mut frontier = Vec::new();

    for analysis in targets.iter().filter_map(|target| target.analysis.as_ref()) {
        let configured_key = analysis
            .configured_target_key()
            .expect("current build analysis only contains configured nodes");
        if !seen.insert(configured_key.clone()) {
            continue;
        }
        closure.push(analysis.dupe());
    }
    for analysis in &closure {
        for dependency in analysis.configured_dependencies() {
            if seen.insert(dependency.clone()) {
                frontier.push(dependency.clone());
            }
        }
    }

    while !frontier.is_empty() {
        let outcomes = ctx
            .compute_join(frontier, |ctx, configured_target| {
                Box::pin(async move {
                    let value = ctx
                        .compute(
                            &ConfiguredNodeAnalysisKey::new(
                                workspace.dupe(),
                                configured_target.clone(),
                            )
                            .expect(
                                "cquery traversal inherits a prepared structural configuration",
                            ),
                        )
                        .await;
                    (configured_target, value)
                })
            })
            .await;
        let mut completed = Vec::with_capacity(outcomes.len());
        for (configured_target, outcome) in outcomes {
            match outcome {
                Err(error) => return Err(Arc::from(error.to_string())),
                Ok(outcome) => completed.push((configured_target, outcome)),
            }
        }
        let layer = match collect_build_action_frontier(completed)? {
            slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need));
            }
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error)) => {
                return Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                    BuildCommandError::new(BuildCommandErrorKind::Analysis(error)),
                )));
            }
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(layer)) => layer,
        };

        let mut next = Vec::new();
        for analysis in layer {
            for dependency in analysis.configured_dependencies() {
                if seen.insert(dependency.clone()) {
                    next.push(dependency.clone());
                }
            }
            closure.push(analysis);
        }
        frontier = next;
    }

    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
        closure.into(),
    )))
}

#[allow(dead_code)]
async fn compute_build_branch(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    pattern: Arc<str>,
    _configuration: ConfigurationKey,
    base_configuration: ConfigurationKey,
    explicit_root_string_setting: Option<RootStringSettingValue>,
) -> BuildBranchResult {
    let parsed = TargetPattern::parse(&pattern)
        .expect("BuildCommandRootKey stores validated canonical target patterns");
    if let TargetPattern::Single(label) = &parsed {
        if !label.repo().is_root() {
            return compute_external_exported_source_build_branch(ctx, workspace, pattern, label)
                .await;
        }
    }
    let package = match &parsed {
        TargetPattern::Single(label) => label.package().clone(),
        TargetPattern::PackageAll { package, .. } => package.clone(),
        TargetPattern::Recursive { .. } => {
            unreachable!("BuildCommandRootKey rejects recursive patterns")
        }
    };
    let package_value = match ctx
        .compute(&RootPackageLoadKey::new(workspace.clone(), package.clone()))
        .await
    {
        Err(error) => return BuildBranchResult::Infrastructure(Arc::from(error.to_string())),
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
            return BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                need,
            ));
        }
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(package) => package.clone(),
            Err(error) => {
                return BuildBranchResult::Outcome(
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                        BuildCommandError::new(BuildCommandErrorKind::Package(error.clone())),
                    )),
                );
            }
        },
    };
    let (analysis, completion) = match parsed {
        TargetPattern::PackageAll { .. } => (None, BuildTargetCompletion::LoadedOnly),
        TargetPattern::Single(label) => {
            let Some(target) = package_value
                .targets
                .iter()
                .find(|candidate| candidate.name == label.target().as_str())
            else {
                return BuildBranchResult::Outcome(
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                        BuildCommandError::target_not_found(
                            pattern,
                            package,
                            label.target().clone(),
                            package_value.build_file.clone(),
                        ),
                    )),
                );
            };
            if matches!(
                target.kind,
                slug_loading_v2::PackageTargetKind::ExportedFile
            ) {
                let path = workspace
                    .as_path()
                    .join(label.package().as_str())
                    .join(label.target().as_str());
                let demand = PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(path)
                        .expect("root package target stays within the workspace"),
                    PathObservationOperation::FileBytes,
                );
                match ctx.compute(&PathObservationKey::new(demand)).await {
                    Err(error) => {
                        return BuildBranchResult::Infrastructure(Arc::from(error.to_string()));
                    }
                    Ok(PathOutcome::Need(need)) => {
                        return BuildBranchResult::Outcome(
                            slug_bzlmod_v2::SourcePreparationOutcome::Need(
                                slug_bzlmod_v2::SourcePreparationNeeds::path(need),
                            ),
                        );
                    }
                    Ok(PathOutcome::Complete(result)) => match result.as_ref() {
                        PathObservationResult::FileBytes(PathOperationResult::Present(_)) => {
                            (None, BuildTargetCompletion::ObservedExportedSource)
                        }
                        _ => {
                            return BuildBranchResult::Outcome(
                                slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                                    BuildCommandError::new(BuildCommandErrorKind::RootSource(
                                        (*result).clone(),
                                    )),
                                )),
                            );
                        }
                    },
                }
            } else if let slug_loading_v2::PackageTargetKind::StarlarkRule(_) = &target.kind {
                let canonical =
                    CanonicalLabel::parse(&format!("@@//{}:{}", label.package(), label.target()))
                        .expect("validated root apparent label has a canonical projection");
                let analysis_key = match prepare_configured_node_analysis(
                    ctx,
                    workspace,
                    canonical,
                    base_configuration,
                    explicit_root_string_setting,
                )
                .await
                {
                    LoadingPreparationOutcome::Need(need) => {
                        return BuildBranchResult::Outcome(
                            slug_bzlmod_v2::SourcePreparationOutcome::Need(need),
                        );
                    }
                    LoadingPreparationOutcome::Complete(Ok(key)) => key,
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return BuildBranchResult::Outcome(
                            slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                                BuildCommandError::new(BuildCommandErrorKind::Analysis(error)),
                            )),
                        );
                    }
                };
                match ctx.compute(&analysis_key).await {
                    Err(error) => {
                        return BuildBranchResult::Infrastructure(Arc::from(error.to_string()));
                    }
                    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
                        return BuildBranchResult::Outcome(
                            slug_bzlmod_v2::SourcePreparationOutcome::Need(need),
                        );
                    }
                    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)) => {
                        match value.as_ref() {
                            Ok(analysis) => {
                                (Some(analysis.clone()), BuildTargetCompletion::Analyzed)
                            }
                            Err(error) => {
                                return BuildBranchResult::Outcome(
                                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                                        BuildCommandError::new(BuildCommandErrorKind::Analysis(
                                            error.clone(),
                                        )),
                                    )),
                                );
                            }
                        }
                    }
                }
            } else {
                (None, BuildTargetCompletion::LoadedOnly)
            }
        }
        TargetPattern::Recursive { .. } => unreachable!(),
    };
    BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
        BuildRequestedTarget {
            pattern,
            package: package_value,
            analysis,
            completion,
        },
    )))
}

async fn compute_external_exported_source_build_branch(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    pattern: Arc<str>,
    label: &slug_identity_v2::ApparentLabel,
) -> BuildBranchResult {
    let route = match RootRepositoryRouteKey::new(workspace, label.repo().clone())
        .expect("external single target has a nonroot repository route")
    {
        key => match ctx.compute(&key).await {
            Err(error) => return BuildBranchResult::Infrastructure(Arc::from(error.to_string())),
            Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
                return BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    need,
                ));
            }
            Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(route) => route.clone(),
                Err(error) => {
                    return BuildBranchResult::Outcome(
                        slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                            BuildCommandError::new(BuildCommandErrorKind::RepositoryRoute(
                                error.clone(),
                            )),
                        )),
                    );
                }
            },
        },
    };
    let package = match ctx
        .compute(&RepositoryPackageLoadKey::new(
            route.clone(),
            label.package().clone(),
        ))
        .await
    {
        Err(error) => return BuildBranchResult::Infrastructure(Arc::from(error.to_string())),
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
            return BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                need,
            ));
        }
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(package) => package.clone(),
            Err(error) => {
                return BuildBranchResult::Outcome(
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                        BuildCommandError::new(BuildCommandErrorKind::RepositoryPackage(
                            error.clone(),
                        )),
                    )),
                );
            }
        },
    };
    let Some(target) = package
        .targets
        .iter()
        .find(|candidate| candidate.name == label.target().as_str())
    else {
        return BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
            Err(BuildCommandError::target_not_found(
                pattern,
                label.package().clone(),
                label.target().clone(),
                package.build_file.clone(),
            )),
        ));
    };
    if !matches!(
        target.kind,
        slug_loading_v2::PackageTargetKind::ExportedFile
    ) {
        return BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
            Err(BuildCommandError::new(
                BuildCommandErrorKind::ExternalTargetKind,
            )),
        ));
    }
    let source_label = CanonicalLabel::parse(&format!(
        "{}//{}:{}",
        route.canonical_repo(),
        label.package(),
        label.target()
    ))
    .expect("validated routed external label has a canonical projection");
    let source = PathBuf::from(label.package().as_str()).join(label.target().as_str());
    match ctx
        .compute(&HostRepositorySourceFileKey::new(route, source))
        .await
    {
        Err(error) => BuildBranchResult::Infrastructure(Arc::from(error.to_string())),
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need))
        }
        Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(HostRepositorySourceFileValue::Present { .. })
            | Err(RepositorySourceFileError::WrongKind {
                actual: PathNodeKind::Directory,
                ..
            }) => BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(
                Ok(BuildRequestedTarget {
                    pattern,
                    package,
                    analysis: None,
                    completion: BuildTargetCompletion::ObservedExportedSource,
                }),
            )),
            Ok(HostRepositorySourceFileValue::Absent) => {
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                    BuildCommandError::new(BuildCommandErrorKind::SourceMissing(source_label)),
                )))
            }
            Err(error) => {
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                    BuildCommandError::new(BuildCommandErrorKind::RepositorySource(error.clone())),
                )))
            }
        },
    }
}

#[async_trait]
impl Key for BuildCommandRootKey {
    type Value = BuildCommandOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let anchor = match ctx
            .compute(&RootModuleLoadingAnchorKey::new(self.workspace.clone()))
            .await
            .expect("build root-module anchor DICE invariant")
        {
            slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                return slug_bzlmod_v2::SourcePreparationOutcome::Need(need);
            }
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Ok(anchor) => anchor.clone(),
                Err(error) => {
                    return build_complete(Err(BuildCommandError::new(
                        BuildCommandErrorKind::RootAnchor(error.clone()),
                    )));
                }
            },
        };
        let workspace = &self.workspace;
        let configuration = &self.configuration;
        let branches = ctx
            .compute_join(self.targets.iter().cloned(), |ctx, pattern| {
                Box::pin(compute_build_branch(
                    ctx,
                    workspace.clone(),
                    pattern,
                    configuration.clone(),
                    self.base_configuration.clone(),
                    self.explicit_root_string_setting.clone(),
                ))
            })
            .await;
        match collect_build_branches(branches) {
            Err(error) => panic!("build command-root infrastructure invariant failed: {error}"),
            Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need)
            }
            Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error))) => {
                build_complete(Err(error))
            }
            Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(targets))) => {
                match compute_build_action_closure(ctx, &self.workspace, &targets).await {
                    Err(error) => {
                        panic!("build action-closure infrastructure invariant failed: {error}")
                    }
                    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Need(need)) => {
                        slug_bzlmod_v2::SourcePreparationOutcome::Need(need)
                    }
                    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(error))) => {
                        build_complete(Err(error))
                    }
                    Ok(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(action_closure))) => {
                        build_complete(Ok(BuildCommandEvaluation {
                            anchor,
                            targets,
                            action_closure,
                        }))
                    }
                }
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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct EvaluatedFile {
    pub path: String,
    pub error: Option<String>,
}

impl EvaluatedFile {
    fn success(path: &Path) -> Self {
        Self {
            path: path.display().to_string(),
            error: None,
        }
    }

    fn failure(path: &Path, error: impl fmt::Display) -> Self {
        Self {
            path: path.display().to_string(),
            error: Some(error.to_string()),
        }
    }
}

impl WorkspaceEvaluation {
    fn into_result(self) -> anyhow::Result<Self> {
        for file in [&self.module, &self.build] {
            if let Some(error) = &file.error {
                anyhow::bail!("failed to evaluate {}: {error}", file.path);
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct WorkspaceEvaluationKey {
    workspace: PathBuf,
}

impl fmt::Display for WorkspaceEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-evaluation:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for WorkspaceEvaluationKey {
    type Value = Arc<WorkspaceEvaluation>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(evaluate_workspace_files(ctx, &self.workspace).await)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

async fn evaluate_workspace_files(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
) -> WorkspaceEvaluation {
    WorkspaceEvaluation {
        module: evaluate_root_module_graph(ctx, workspace).await,
        build: evaluate_workspace_build_file(ctx, workspace).await,
        revision: WorkspaceRevision(0),
    }
}

async fn evaluate_root_module_graph(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
) -> EvaluatedFile {
    let module_path = workspace.join("MODULE.bazel");
    match ctx
        .compute(&RootModuleGraphKey {
            workspace: workspace.to_path_buf(),
        })
        .await
    {
        Ok(graph) => match graph.as_ref() {
            Ok(_) => EvaluatedFile::success(&module_path),
            Err(error) => EvaluatedFile::failure(&module_path, error),
        },
        Err(error) => EvaluatedFile::failure(&module_path, error),
    }
}

async fn evaluate_workspace_build_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
) -> EvaluatedFile {
    let primary = workspace.join("BUILD.bazel");
    let observed = match ctx
        .compute(&WorkspaceFileKey {
            workspace: workspace.to_path_buf(),
            path: primary.clone(),
        })
        .await
    {
        Ok(observed) => observed,
        Err(error) => return EvaluatedFile::failure(&primary, error),
    };
    match observed {
        WorkspaceFileValue::Present(source) => evaluate_workspace_source(&primary, &source, false),
        WorkspaceFileValue::Absent => {
            let fallback = workspace.join("BUILD");
            evaluate_workspace_file(ctx, workspace, &fallback, false).await
        }
        WorkspaceFileValue::ReadError(error) => EvaluatedFile::failure(&primary, error),
    }
}

async fn evaluate_workspace_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    path: &Path,
    is_module: bool,
) -> EvaluatedFile {
    let observed = match ctx
        .compute(&WorkspaceFileKey {
            workspace: workspace.to_path_buf(),
            path: path.to_path_buf(),
        })
        .await
    {
        Ok(observed) => observed,
        Err(error) => return EvaluatedFile::failure(path, error),
    };
    let source = match observed {
        WorkspaceFileValue::Present(source) => source,
        WorkspaceFileValue::Absent => {
            return EvaluatedFile::failure(path, "workspace file is absent");
        }
        WorkspaceFileValue::ReadError(error) => return EvaluatedFile::failure(path, error),
    };
    evaluate_workspace_source(path, &source, is_module)
}

fn evaluate_workspace_source(path: &Path, source: &str, is_module: bool) -> EvaluatedFile {
    match evaluate_file(path, source, is_module) {
        Ok(()) => EvaluatedFile::success(path),
        Err(error) => EvaluatedFile::failure(path, error),
    }
}

impl WorkspaceRuntime {
    pub fn new(
        workspace: impl Into<PathBuf>,
        process_host: Arc<super::ProcessHostOwner>,
    ) -> anyhow::Result<Self> {
        let workspace = workspace.into();
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
        let normalized_workspace = NormalizedAbsolutePath::new(workspace.clone())
            .context("normalizing retained workspace")?;
        let repository_materializer = Arc::new(super::repository_io::RepositoryMaterializer::new(
            normalized_workspace.clone(),
        ));
        let configured_output = Arc::new(super::configured_output::ConfiguredOutputOwner::new(
            workspace.clone(),
        ));
        let loader = BzlModuleEvaluator::new(&workspace)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("creating workspace DICE runtime")?;
        let mut dice_builder = Dice::builder();
        super::registry_io::install(&mut dice_builder);
        super::repository_io::install(&mut dice_builder);
        let dice = dice_builder.build(DetectCycles::Enabled);
        let native_demand_sessions = NativeDemandSessionOwner::new(normalized_workspace.clone());
        let demand_owner = WorkspaceDemandOwner::new(&dice, normalized_workspace);
        Ok(Self {
            workspace,
            process_host,
            configured_output,
            dice,
            demand_owner,
            loader,
            runtime,
            next_revision: AtomicU64::new(1),
            next_registry_generation: AtomicU64::new(1),
            next_repository_materialization_generation: AtomicU64::new(1),
            repository_materializer,
            native_demand_sessions,
            #[cfg(test)]
            activation_audit: None,
        })
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    #[cfg(test)]
    pub(crate) fn process_host(&self) -> &Arc<super::ProcessHostOwner> {
        &self.process_host
    }

    fn user_computation_data(
        &self,
        effects: Option<Arc<AttemptEffectTracker>>,
    ) -> Result<UserComputationData, CommandEffectError> {
        let mut data = UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            ..Default::default()
        };
        self.demand_owner.install(&self.dice, &mut data, effects)?;
        #[cfg(test)]
        if let Some(audit) = &self.activation_audit {
            let runtime = data
                .activation_tracker
                .take()
                .expect("the runtime demand owner installed its tracker");
            data.activation_tracker = Some(Arc::new(AuditedRuntimeActivationTracker {
                runtime,
                audit: audit.dupe(),
            }));
        }
        Ok(data)
    }

    #[cfg(test)]
    fn with_activation_audit(mut self, audit: Arc<ExternalQueryActivationAudit>) -> Self {
        self.activation_audit = Some(audit);
        self
    }

    #[allow(dead_code)]
    fn begin_native_demand_command(
        &self,
    ) -> Result<NativeDemandPreflight<'_>, NativeDemandSessionError> {
        self.begin_native_demand_command_with_inputs(
            NativeDemandRequestInputBundle::normalized_initial(),
        )
    }

    #[allow(dead_code)]
    fn begin_native_demand_command_with_inputs(
        &self,
        request: NativeDemandRequestInputBundle,
    ) -> Result<NativeDemandPreflight<'_>, NativeDemandSessionError> {
        let (lease, prior) = self.native_demand_sessions.acquire()?;
        // Busy is decided before any member of this fixed command bundle is
        // allocated.
        let inputs = NativeDemandInputBundle {
            request,
            generations: NativeDemandGenerationBundle {
                workspace_revision: WorkspaceRevision(
                    self.next_revision.fetch_add(1, Ordering::Relaxed),
                ),
                registry: RegistryRequestGeneration(
                    self.next_registry_generation
                        .fetch_add(1, Ordering::Relaxed),
                ),
                repository: RepositoryMaterializationGeneration(
                    self.next_repository_materialization_generation
                        .fetch_add(1, Ordering::Relaxed),
                ),
            },
        };
        let repository_session = match self.repository_materializer.begin() {
            Ok(token) => token,
            Err(error) => {
                // Once the workspace lease is Open, an unexpected materializer
                // owner is evidence of incoherent state. Fail closed rather
                // than exposing Idle beside that active owner.
                return Err(NativeDemandSessionError::Repository(error));
            }
        };
        self.native_demand_sessions
            .attach_repository(lease, repository_session)?;
        let preflight = match self.repository_materializer.preflight_native(
            repository_session,
            prior.selected.unscoped_paths().iter().cloned(),
        ) {
            Ok(preflight) => preflight,
            Err(error) => {
                if let Err(discard) = self.repository_materializer.discard(repository_session) {
                    // Keep the lease open when cleanup cannot restore a
                    // coherent owner/materializer pair.
                    return Err(NativeDemandSessionError::Repository(discard));
                }
                self.native_demand_sessions.close(lease)?;
                return Err(NativeDemandSessionError::Repository(error));
            }
        };
        Ok(NativeDemandPreflight {
            command: NativeDemandCommand {
                runtime: self,
                lease,
                repository_session,
                inputs,
                effects: CommandEffectOwner::new(),
                prior,
                reusable_requests: preflight
                    .reusable_requests()
                    .iter()
                    .map(|request| (request.id.clone(), request.clone()))
                    .collect(),
                issued_requests: SmallMap::new(),
                repository_results: preflight.repository_results().clone(),
                path_observations: preflight.path_observations().clone(),
            },
        })
    }

    #[allow(dead_code)]
    fn drive_command<R>(
        &self,
        request: NativeDemandRequestInputBundle,
        root: R,
    ) -> Result<DrivenCommand<R::Terminal>, NativeDemandSessionError>
    where
        R: NativeCommandRoot,
    {
        let preflight = self.begin_native_demand_command_with_inputs(request)?;
        let mut guard = NativeDemandAbortGuard::new(preflight.into_command());
        let allows_empty_terminal = root.allows_empty_terminal();
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            if let Err(error) = guard.begin_attempt() {
                return guard.abort(error);
            }
            let attempt_root = root.clone();
            let attempt = self.runtime.block_on(async {
                let data = guard.attempt_user_computation_data()?;
                let mut updater = self.dice.updater_with_data(data);
                guard.inject_attempt(&mut updater)?;
                let mut transaction = updater.commit().await;
                let root_outcome = attempt_root.compute(&mut transaction).await?;
                match &root_outcome {
                    slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) => {
                        let needs = needs.clone();
                        guard.seal_retry()?;
                        drop(root_outcome);
                        drop(attempt_root);
                        drop(transaction);
                        Ok(CommandAttemptResult::Retry(needs))
                    }
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(terminal) => {
                        let terminal = terminal.clone();
                        let sealed = if attempt_root.allows_unavailable_terminal_roots(&terminal) {
                            guard.seal_terminal_allowing_unavailable_roots()?
                        } else if allows_empty_terminal {
                            guard.seal_terminal_allowing_empty_roots()?
                        } else {
                            guard.seal_terminal()?
                        };
                        let terminal_root_count = sealed.root_count();
                        let selected = sealed.select(&transaction).await?;
                        let prepared = guard.prepare_accept(selected, &transaction).await;
                        drop(root_outcome);
                        drop(attempt_root);
                        drop(transaction);
                        #[cfg(test)]
                        self.native_demand_sessions
                            .record_trace(NativeDemandTestTrace::TerminalTransactionDropped);
                        let prepared = prepared?;
                        Ok(CommandAttemptResult::Terminal(
                            terminal,
                            prepared,
                            terminal_root_count,
                        ))
                    }
                }
            });
            let attempt = match attempt {
                Ok(attempt) => attempt,
                Err(error) => {
                    #[cfg(test)]
                    self.native_demand_sessions
                        .record_trace(NativeDemandTestTrace::AttemptTransactionDroppedBeforeAbort);
                    return guard.abort(error);
                }
            };
            match attempt {
                CommandAttemptResult::Retry(needs) => {
                    if let Err(error) = guard.progress(&needs) {
                        return guard.abort(error);
                    }
                }
                CommandAttemptResult::Terminal(terminal, prepared, terminal_root_count) => {
                    let accepted = guard.accept_prepared(prepared, terminal)?;
                    return Ok(DrivenCommand {
                        accepted,
                        attempts,
                        terminal_root_count,
                    });
                }
            }
        }
    }

    #[allow(dead_code)]
    fn drive_synthetic_command(
        &self,
        request: NativeDemandRequestInputBundle,
        root: SyntheticCommandRoot,
    ) -> Result<SyntheticCommandResult, NativeDemandSessionError> {
        self.drive_command(request, root)
    }

    fn inject_bzlmod_request_inputs(
        &self,
        updater: &mut DiceTransactionUpdater,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: RegistryUrls,
    ) -> anyhow::Result<()> {
        inject_root_module_request_inputs(
            updater,
            &self.workspace,
            command_policy,
            environment_policy,
            lockfile_mode,
        )
        .context("injecting normalized root module request inputs")?;
        inject_registry_request_inputs(
            updater,
            &self.workspace,
            registry_urls,
            RegistryRequestGeneration(
                self.next_registry_generation
                    .fetch_add(1, Ordering::Relaxed),
            ),
        )
        .context("injecting registry request inputs")?;
        updater
            .changed_to(vec![(
                RepositoryMaterializationGenerationKey {
                    workspace: self.workspace.clone(),
                },
                RepositoryMaterializationGeneration(
                    self.next_repository_materialization_generation
                        .fetch_add(1, Ordering::Relaxed),
                ),
            )])
            .context("injecting repository materialization generation")
    }

    /// Evaluate one typed loading/analysis build command through the retained
    /// native demand, terminal-selection, and publication owner.
    pub fn build_command_with_bzlmod_inputs(
        &self,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        root_string_setting: Option<&str>,
    ) -> Result<
        AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
        BuildCommandError,
    > {
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(BuildCommandError::infrastructure)?;
        let host = self
            .process_host
            .default_configuration_inputs()
            .map_err(BuildCommandError::infrastructure)?;
        let base_configuration =
            SlugConfiguration::default_target(&host).map_err(BuildCommandError::infrastructure)?;
        let explicit_root_string_setting = root_string_setting.map(RootStringSettingValue::new);
        let configuration = explicit_root_string_setting.as_ref().map_or_else(
            || base_configuration.clone(),
            |value| base_configuration.with_root_string_setting(value.clone()),
        );
        self.configured_output
            .claim(&configuration)
            .map_err(BuildCommandError::infrastructure)?;
        let root = BuildCommandRootKey::new_with_root_string_setting(
            NormalizedAbsolutePath::new(self.workspace.clone())
                .map_err(BuildCommandError::infrastructure)?,
            targets,
            ConfigurationKey::from_slug(base_configuration),
            explicit_root_string_setting,
        )
        .map_err(BuildCommandError::request)?;
        let request = NativeDemandRequestInputBundle {
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
        };
        let driven = self.drive_command(request, root).map_err(|error| {
            BuildCommandError::infrastructure(format!("typed build command failed: {error}"))
        })?;
        if let Ok(evaluation) = driven.accepted.terminal().as_ref().as_ref() {
            for analysis in evaluation.analyses() {
                let configuration = analysis
                    .configured_target_key()
                    .expect("production analysis only contains configured nodes")
                    .configuration()
                    .slug_configuration()
                    .ok_or_else(|| {
                        BuildCommandError::infrastructure(
                            "production analysis returned an opaque configuration",
                        )
                    })?;
                self.configured_output
                    .claim(configuration)
                    .map_err(BuildCommandError::infrastructure)?;
            }
        }
        Ok(driven.accepted)
    }

    pub fn cquery_command_with_bzlmod_inputs(
        &self,
        expression: &str,
        include_implicit: bool,
        include_tool: bool,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        root_string_setting: Option<&str>,
    ) -> Result<
        AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
        CqueryCommandError,
    > {
        let expression = QueryExpression::parse(expression)
            .map_err(|error| CqueryCommandError::request(error.to_string()))?;
        validate_cquery_query(&expression)
            .map_err(|error| CqueryCommandError::request(error.to_string()))?;
        if expression.cquery_preactivation_deps_spec().is_some() && include_implicit {
            return Err(CqueryCommandError::request(
                "cquery deps() currently requires --noimplicit_deps",
            ));
        }
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(CqueryCommandError::infrastructure)?;
        let host = self
            .process_host
            .default_configuration_inputs()
            .map_err(CqueryCommandError::infrastructure)?;
        let base_configuration =
            SlugConfiguration::default_target(&host).map_err(CqueryCommandError::infrastructure)?;
        let explicit_root_string_setting = root_string_setting.map(RootStringSettingValue::new);
        let configuration = explicit_root_string_setting.as_ref().map_or_else(
            || base_configuration.clone(),
            |value| base_configuration.with_root_string_setting(value.clone()),
        );
        self.configured_output
            .claim(&configuration)
            .map_err(CqueryCommandError::infrastructure)?;
        let workspace = NormalizedAbsolutePath::new(self.workspace.clone())
            .map_err(CqueryCommandError::infrastructure)?;
        let mut roots = Vec::new();
        let mut root_indices = SmallMap::new();
        let mut literal_roots = Vec::new();
        for literal in cquery_literals(&expression) {
            let target = TargetPattern::parse(literal)
                .map_err(|error| CqueryCommandError::request(error))?;
            let TargetPattern::Single(ref label) = target else {
                return Err(CqueryCommandError::request(
                    "target patterns are not supported by this cquery",
                ));
            };
            if !label.repo().is_root() {
                return Err(CqueryCommandError::request(
                    "external repository labels are not supported by this cquery",
                ));
            }
            let canonical =
                CanonicalLabel::parse(&format!("@@//{}:{}", label.package(), label.target()))
                    .map_err(CqueryCommandError::infrastructure)?;
            let requested: Arc<str> = Arc::from(target.to_string());
            let index = match root_indices.get(&canonical) {
                Some(index) => *index,
                None => {
                    let index = roots.len();
                    roots.push(CqueryRootTarget {
                        requested,
                        canonical: canonical.clone(),
                        workspace: workspace.clone(),
                        base_configuration: ConfigurationKey::from_slug(base_configuration.clone()),
                        explicit_root_string_setting: explicit_root_string_setting.clone(),
                    });
                    root_indices.insert(canonical, index);
                    index
                }
            };
            literal_roots.push((Arc::from(literal), index));
        }
        let root = CqueryCommandRoot {
            expression,
            roots: roots.into(),
            literal_roots: literal_roots.into(),
            include_implicit,
            include_tool,
        };
        let request = NativeDemandRequestInputBundle {
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
        };
        let driven = self.drive_command(request, root).map_err(|error| {
            CqueryCommandError::infrastructure(format!("typed cquery command failed: {error}"))
        })?;
        if let Ok(evaluation) = driven.accepted.terminal().as_ref().as_ref() {
            for analysis in evaluation.analyses() {
                let Some(configured) = analysis.configured_target_key() else {
                    continue;
                };
                let configuration =
                    configured
                        .configuration()
                        .slug_configuration()
                        .ok_or_else(|| {
                            CqueryCommandError::infrastructure(
                                "production cquery analysis returned an opaque configuration",
                            )
                        })?;
                self.configured_output
                    .claim(configuration)
                    .map_err(CqueryCommandError::infrastructure)?;
            }
        }
        Ok(driven.accepted)
    }

    /// Evaluate one typed loading-query command through the retained native
    /// demand, terminal-selection, and publication owner.
    pub fn query_command_with_policy_and_bzlmod_inputs_and_output_completion(
        &self,
        expression: &str,
        order: QueryOrder,
        policy: QueryPolicy,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        completion: QueryOutputCompletion,
    ) -> Result<AcceptedCommand<Arc<Result<QueryOutput, QueryError>>>, QueryError> {
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let root = RootQueryCommandKey::new(
            NormalizedAbsolutePath::new(self.workspace.clone())
                .map_err(|error| QueryError::evaluation(error.to_string()))?,
            expression,
            order,
            policy,
            completion,
        )?;
        let request = NativeDemandRequestInputBundle {
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
        };
        self.drive_command(request, root)
            .map(|result| result.accepted)
            .map_err(|error| QueryError::evaluation(format!("typed query command failed: {error}")))
    }

    /// Evaluate one loading query in this runtime's retained DICE graph.
    ///
    /// Parsing, registry validation, literal resolution, and traversal all
    /// happen after the observation batch is committed, in the same
    /// transaction used by loading keys.
    pub fn query_observations(
        &self,
        observations: WorkspaceObservation,
        expression: &str,
        order: QueryOrder,
    ) -> Result<QueryOutput, QueryError> {
        self.query_observations_with_policy(observations, expression, order, QueryPolicy::default())
    }

    pub fn query_observations_with_policy(
        &self,
        observations: WorkspaceObservation,
        expression: &str,
        order: QueryOrder,
        policy: QueryPolicy,
    ) -> Result<QueryOutput, QueryError> {
        self.query_observations_with_policy_and_bzlmod_inputs(
            observations,
            expression,
            order,
            policy,
            BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy"),
            LockfileMode::Update,
            &[],
        )
    }

    /// Evaluate a query with explicit normalized bzlmod request inputs.
    pub fn query_observations_with_policy_and_bzlmod_inputs(
        &self,
        observations: WorkspaceObservation,
        expression: &str,
        order: QueryOrder,
        policy: QueryPolicy,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
    ) -> Result<QueryOutput, QueryError> {
        self.query_observations_with_policy_and_bzlmod_inputs_and_output_completion(
            observations,
            expression,
            order,
            policy,
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
            QueryOutputCompletion::Standard,
        )
    }

    pub fn query_observations_with_policy_and_bzlmod_inputs_and_output_completion(
        &self,
        observations: WorkspaceObservation,
        expression: &str,
        order: QueryOrder,
        policy: QueryPolicy,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
        completion: QueryOutputCompletion,
    ) -> Result<QueryOutput, QueryError> {
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let files = observations
            .files
            .into_iter()
            .map(|observation| {
                self.validate_file_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<_>>()
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let raw_files = observations
            .raw_files
            .into_iter()
            .map(|observation| {
                self.validate_raw_file_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<_>>()
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let directories = observations
            .directories
            .into_iter()
            .map(|observation| {
                self.validate_directory_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let snapshot = Arc::new(WorkspaceSnapshot {
            files: Arc::new(files),
        });
        let raw_snapshot = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(raw_files),
        });
        let directory_snapshot = Arc::new(WorkspaceDirectorySnapshot {
            directories: Arc::new(directories.into_iter().collect()),
        });
        self.runtime.block_on(async {
            let data = self
                .user_computation_data(None)
                .map_err(|error| QueryError::evaluation(error.to_string()))?;
            let mut updater = self.dice.updater_with_data(data);
            updater
                .changed_to(vec![(
                    WorkspaceSnapshotKey {
                        workspace: self.workspace.clone(),
                    },
                    snapshot,
                )])
                .map_err(|error| QueryError::evaluation(error.to_string()))?;
            updater
                .changed_to(vec![(
                    (WorkspaceRawSnapshotKey {
                        workspace: self.workspace.clone(),
                    }),
                    raw_snapshot,
                )])
                .map_err(|error| QueryError::evaluation(error.to_string()))?;
            updater
                .changed_to(vec![(
                    WorkspaceDirectorySnapshotKey {
                        workspace: self.workspace.clone(),
                    },
                    directory_snapshot,
                )])
                .map_err(|error| QueryError::evaluation(error.to_string()))?;
            self.inject_bzlmod_request_inputs(
                &mut updater,
                command_policy,
                environment_policy,
                lockfile_mode,
                registry_urls,
            )
            .map_err(|error| {
                QueryError::evaluation(format!("injecting bzlmod request inputs: {error}"))
            })?;
            let mut transaction = updater.commit().await;
            evaluate_loading_query_with_policy_and_output_completion(
                &mut transaction,
                self.workspace.clone(),
                expression,
                order,
                policy,
                completion,
            )
            .await
        })
    }

    /// Commit all external file observations as one DICE version, then evaluate
    /// root files and packages from that exact transaction.
    pub fn evaluate(
        &self,
        observations: impl IntoIterator<Item = WorkspaceFileObservation>,
        targets: &[TargetPattern],
    ) -> anyhow::Result<WorkspaceBuildEvaluation> {
        self.evaluate_observations(WorkspaceObservation::from_files(observations), targets)
    }

    /// Commit file and direct-directory observations together, then evaluate
    /// root files and packages from that one request revision.
    pub fn evaluate_observations(
        &self,
        observations: WorkspaceObservation,
        targets: &[TargetPattern],
    ) -> anyhow::Result<WorkspaceBuildEvaluation> {
        self.evaluate_observations_with_bzlmod_inputs(
            observations,
            targets,
            BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy"),
            LockfileMode::Update,
            &[],
        )
    }

    /// Evaluate with explicit normalized bzlmod request inputs on this retained graph.
    pub fn evaluate_observations_with_bzlmod_inputs(
        &self,
        observations: WorkspaceObservation,
        targets: &[TargetPattern],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
    ) -> anyhow::Result<WorkspaceBuildEvaluation> {
        self.evaluate_observations_with_directory_probes_and_bzlmod_inputs(
            observations,
            targets,
            &[],
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
        )
        .map(|(evaluation, _)| evaluation)
    }

    /// Internal evidence hook for selected directory keys.
    ///
    /// Production requests pass no probes. Keeping this private prevents the
    /// migration observer from turning every directory into an eager semantic
    /// dependency before a real glob consumer exists.
    #[allow(dead_code)] // Retained only for legacy observation-path tests.
    fn evaluate_observations_with_directory_probes(
        &self,
        observations: WorkspaceObservation,
        targets: &[TargetPattern],
        directory_probes: &[PathBuf],
    ) -> anyhow::Result<(
        WorkspaceBuildEvaluation,
        Vec<(PathBuf, WorkspaceDirectoryValue, WorkspaceRevision)>,
    )> {
        self.evaluate_observations_with_directory_probes_and_bzlmod_inputs(
            observations,
            targets,
            directory_probes,
            BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy"),
            LockfileMode::Update,
            &[],
        )
    }

    fn evaluate_observations_with_directory_probes_and_bzlmod_inputs(
        &self,
        observations: WorkspaceObservation,
        targets: &[TargetPattern],
        directory_probes: &[PathBuf],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: &[String],
    ) -> anyhow::Result<(
        WorkspaceBuildEvaluation,
        Vec<(PathBuf, WorkspaceDirectoryValue, WorkspaceRevision)>,
    )> {
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(anyhow::Error::msg)?;
        let files = observations
            .files
            .into_iter()
            .map(|observation| {
                self.validate_file_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<_>>()?;
        let raw_files = observations
            .raw_files
            .into_iter()
            .map(|observation| {
                self.validate_raw_file_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<_>>()?;
        let directories = observations
            .directories
            .into_iter()
            .map(|observation| {
                self.validate_directory_observation(observation)
                    .map(|observation| (observation.path, observation.value))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let directory_probes = directory_probes
            .iter()
            .map(|path| self.validate_observation_path(path))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let snapshot = Arc::new(WorkspaceSnapshot {
            files: Arc::new(files),
        });
        let raw_snapshot = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(raw_files),
        });
        let directory_snapshot = Arc::new(WorkspaceDirectorySnapshot {
            directories: Arc::new(directories.into_iter().collect()),
        });
        let host = self
            .process_host
            .default_configuration_inputs()
            .map_err(anyhow::Error::msg)?;
        let structural_configuration =
            SlugConfiguration::default_target(&host).map_err(anyhow::Error::msg)?;
        self.configured_output
            .claim(&structural_configuration)
            .map_err(anyhow::Error::msg)?;
        let configuration = ConfigurationKey::from_slug(structural_configuration);
        let revision = WorkspaceRevision(self.next_revision.fetch_add(1, Ordering::Relaxed));
        self.runtime.block_on(async {
            let data = self
                .user_computation_data(None)
                .context("installing passive workspace activation tracking")?;
            let mut updater = self.dice.updater_with_data(data);
            updater
                .changed_to(vec![(
                    (WorkspaceSnapshotKey {
                        workspace: self.workspace.clone(),
                    }),
                    snapshot,
                )])
                .context("injecting workspace-file observations")?;
            updater
                .changed_to(vec![(
                    (WorkspaceRawSnapshotKey {
                        workspace: self.workspace.clone(),
                    }),
                    raw_snapshot,
                )])
                .context("injecting raw workspace-file observations")?;
            // DICE's typed 'changed_to' batches one key type per call. Both
            // snapshots are scheduled on this single updater before its sole
            // commit, so no transaction can see one without the other.
            updater
                .changed_to(vec![(
                    (WorkspaceDirectorySnapshotKey {
                        workspace: self.workspace.clone(),
                    }),
                    directory_snapshot,
                )])
                .context("injecting workspace-directory observations")?;
            self.inject_bzlmod_request_inputs(
                &mut updater,
                command_policy,
                environment_policy,
                lockfile_mode,
                registry_urls,
            )
            .context("injecting bzlmod request inputs")?;
            let mut transaction = updater.commit().await;
            let root_module_graph = transaction
                .compute(&RootModuleGraphKey {
                    workspace: self.workspace.clone(),
                })
                .await
                .context("computing root module graph through DICE")?
                .as_ref()
                .as_ref()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .clone();
            let mut workspace = transaction
                .compute(&WorkspaceEvaluationKey {
                    workspace: self.workspace.clone(),
                })
                .await
                .context("computing root workspace evaluation through DICE")?
                .as_ref()
                .clone()
                .into_result()?;
            workspace.revision = revision;
            let mut probed_directories = Vec::with_capacity(directory_probes.len());
            for path in directory_probes {
                let value = transaction
                    .compute(&WorkspaceDirectoryKey {
                        workspace: self.workspace.clone(),
                        directory: path.clone(),
                    })
                    .await
                    .context("computing observed workspace directory through DICE")?;
                probed_directories.push((path, value, revision));
            }
            let mut packages = Vec::with_capacity(targets.len());
            for target in targets {
                let package_path = package_path_for_target(&self.workspace, target)?;
                let package = self
                    .loader
                    .evaluate_package(&mut transaction, package_path)
                    .await?;
                let analysis = match target {
                    TargetPattern::Single(label) => {
                        let package_target = package
                            .targets
                            .iter()
                            .find(|candidate| candidate.name == label.target().as_str())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "target `{target}` was not found in {}",
                                    package.build_file.display()
                                )
                            })?;
                        if matches!(
                            package_target.kind,
                            slug_loading_v2::PackageTargetKind::StarlarkRule(_)
                        ) {
                            let canonical = CanonicalLabel::parse(&format!(
                                "@@//{}:{}",
                                label.package().as_str(),
                                label.target().as_str()
                            ))
                            .map_err(anyhow::Error::msg)?;
                            let configured_target =
                                ConfiguredTargetKey::new(canonical, configuration.clone());
                            let outcome = transaction
                                .compute(
                                    &ConfiguredNodeAnalysisKey::new(
                                        NormalizedAbsolutePath::new(self.workspace.clone())
                                            .map_err(anyhow::Error::msg)?,
                                        configured_target,
                                    )
                                    .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                                )
                                .await
                                .context("computing configured-target analysis through DICE")?;
                            let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome
                            else {
                                anyhow::bail!(
                                    "legacy workspace evaluation retained configured-analysis Needs"
                                );
                            };
                            Some(
                                value
                                    .as_ref()
                                    .as_ref()
                                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
                                    .as_ref()
                                    .clone(),
                            )
                        } else {
                            None
                        }
                    }
                    TargetPattern::PackageAll { .. } | TargetPattern::Recursive { .. } => None,
                };
                packages.push(RequestedPackageEvaluation {
                    target_pattern: target.to_string(),
                    package,
                    analysis,
                    revision,
                });
            }
            Ok((
                WorkspaceBuildEvaluation {
                    workspace,
                    root_module_graph: Arc::new(root_module_graph),
                    packages,
                    revision,
                },
                probed_directories,
            ))
        })
    }

    #[cfg(test)]
    fn current_root_module_graph_for_test(&self) -> anyhow::Result<Arc<RootModuleGraph>> {
        self.runtime.block_on(async {
            let updater = self.dice.updater_with_data(
                self.user_computation_data(None)
                    .context("installing passive workspace activation tracking")?,
            );
            let mut transaction = updater.existing_state().await;
            let graph = transaction
                .compute(&RootModuleGraphKey {
                    workspace: self.workspace.clone(),
                })
                .await
                .context("reading retained root module graph through DICE")?;
            Ok(Arc::new(
                graph
                    .as_ref()
                    .as_ref()
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
                    .clone(),
            ))
        })
    }

    fn validate_file_observation(
        &self,
        observation: WorkspaceFileObservation,
    ) -> anyhow::Result<WorkspaceFileObservation> {
        Ok(WorkspaceFileObservation {
            path: self.validate_observation_path(&observation.path)?,
            value: observation.value,
        })
    }

    fn validate_raw_file_observation(
        &self,
        observation: WorkspaceRawFileObservation,
    ) -> anyhow::Result<WorkspaceRawFileObservation> {
        Ok(WorkspaceRawFileObservation {
            path: self.validate_observation_path(&observation.path)?,
            value: observation.value,
        })
    }

    fn validate_directory_observation(
        &self,
        observation: WorkspaceDirectoryObservation,
    ) -> anyhow::Result<WorkspaceDirectoryObservation> {
        Ok(WorkspaceDirectoryObservation {
            path: self.validate_observation_path(&observation.path)?,
            value: observation.value,
        })
    }

    fn validate_observation_path(&self, path: &Path) -> anyhow::Result<PathBuf> {
        if !path.is_absolute() {
            anyhow::bail!(
                "workspace observation path must be absolute: {}",
                path.display()
            );
        }
        if path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            anyhow::bail!(
                "workspace observation path is not normalized: {}",
                path.display()
            );
        }
        let path = path.to_path_buf();
        if !path.starts_with(&self.workspace) {
            anyhow::bail!(
                "workspace observation is outside {}: {}",
                self.workspace.display(),
                path.display()
            );
        }
        let existing_ancestor = path
            .ancestors()
            .find(|candidate| std::fs::symlink_metadata(candidate).is_ok())
            .expect("the canonical workspace is an existing observation ancestor");
        let canonical_ancestor = existing_ancestor.canonicalize().with_context(|| {
            format!(
                "canonicalizing observation ancestor {}",
                existing_ancestor.display()
            )
        })?;
        if canonical_ancestor != existing_ancestor {
            anyhow::bail!(
                "workspace observation path aliases through {}: {}",
                existing_ancestor.display(),
                path.display()
            );
        }
        Ok(path)
    }
}

#[allow(dead_code)]
impl<'a> NativeDemandPreflight<'a> {
    fn generations(&self) -> NativeDemandGenerationBundle {
        self.command.inputs.generations
    }

    fn repository_results(&self) -> &RepositoryMaterializationResultEpoch {
        &self.command.repository_results
    }

    fn path_observations(&self) -> &PathObservationEpoch {
        &self.command.path_observations
    }

    fn into_command(self) -> NativeDemandCommand<'a> {
        self.command
    }
}

#[allow(dead_code)]
impl NativeDemandCommand<'_> {
    fn begin_attempt(&self) -> Result<NativeDemandAttempt, NativeDemandSessionError> {
        let tracker = self
            .effects
            .begin_attempt()
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandAttempt {
            effects: self.effects.clone(),
            tracker,
        })
    }

    fn attempt_user_computation_data(
        &self,
        attempt: &NativeDemandAttempt,
    ) -> Result<UserComputationData, NativeDemandSessionError> {
        if !Arc::ptr_eq(&self.effects, &attempt.effects) {
            return Err(NativeDemandSessionError::ForeignEffects);
        }
        self.runtime
            .user_computation_data(Some(attempt.tracker.clone()))
            .map_err(NativeDemandSessionError::Effect)
    }

    fn inject_attempt(
        &self,
        updater: &mut DiceTransactionUpdater,
    ) -> Result<(), NativeDemandSessionError> {
        inject_native_demand_snapshot(
            updater,
            self.runtime,
            &self.inputs,
            self.repository_results.clone(),
            self.path_observations.clone(),
        )
        .map_err(NativeDemandSessionError::Injection)
    }

    fn progress(
        &mut self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<NativeDemandProgress, NativeDemandSessionError> {
        self.progress_inner(needs)
    }

    fn progress_inner(
        &mut self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<NativeDemandProgress, NativeDemandSessionError> {
        let mut repository_needs = needs
            .repository_materializations()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        repository_needs.sort_by(|left, right| {
            left.id
                .workspace
                .cmp(&right.id.workspace)
                .then_with(|| left.id.canonical_repo.cmp(&right.id.canonical_repo))
        });
        if !repository_needs.is_empty() {
            let mut new_requests = Vec::new();
            for request in repository_needs {
                match self.issued_requests.get(&request.id) {
                    Some(known) if known.as_ref() != request.as_ref() => {
                        return Err(NativeDemandSessionError::ConflictingRepository(
                            request.id.canonical_repo.clone(),
                        ));
                    }
                    Some(_) => {}
                    None => match self.reusable_requests.get(&request.id) {
                        // An equal inherited result is already known. A
                        // changed exact request is permitted to replace it
                        // once; after issuance, another changed request above
                        // is a strict conflict.
                        Some(known) if known.as_ref() == request.as_ref() => {}
                        Some(_) | None => new_requests.push(request),
                    },
                }
            }
            if new_requests.is_empty() {
                return Err(NativeDemandSessionError::RepositoryInternalNonProgress);
            }
            for request in new_requests {
                self.repository_results = self
                    .runtime
                    .repository_materializer
                    .materialize_native(
                        self.repository_session,
                        request.clone(),
                        self.inputs.generations.repository,
                    )
                    .map_err(NativeDemandSessionError::Repository)?;
                self.reusable_requests.shift_remove(&request.id);
                self.issued_requests.insert(request.id.clone(), request);
            }
            return Ok(NativeDemandProgress::Repositories);
        }

        let Some(path_needs) = needs.path_observations() else {
            return Err(NativeDemandSessionError::PathInternalNonProgress);
        };
        let new_demands = path_needs
            .demands()
            .iter()
            .filter(|demand| self.path_observations.get(demand).is_none())
            .cloned()
            .collect::<Vec<_>>();
        if new_demands.is_empty() {
            return Err(NativeDemandSessionError::PathInternalNonProgress);
        }
        let observed = self
            .runtime
            .repository_materializer
            .observe_native(self.repository_session, new_demands)
            .map_err(NativeDemandSessionError::Repository)?;
        let merged = self
            .path_observations
            .observations()
            .iter()
            .map(|(demand, result)| (demand.clone(), result.as_ref().clone()))
            .chain(
                observed
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.clone(), result.as_ref().clone())),
            );
        self.path_observations =
            PathObservationEpoch::new(merged).map_err(NativeDemandSessionError::PathEpoch)?;
        Ok(NativeDemandProgress::Paths)
    }

    fn discard_in_place(&mut self) -> Result<(), NativeDemandSessionError> {
        #[cfg(test)]
        if self
            .runtime
            .native_demand_sessions
            .take_restoration_failure(self.lease)?
        {
            return Err(NativeDemandSessionError::Restoration(anyhow::anyhow!(
                "forced restoration failure"
            )));
        }
        let prior = self.prior.clone();
        self.runtime.runtime.block_on(async {
            let mut updater = self.runtime.dice.updater_with_data(
                self.runtime.user_computation_data(None).map_err(|error| {
                    NativeDemandSessionError::Restoration(anyhow::anyhow!(error.to_string()))
                })?,
            );
            inject_native_demand_snapshot(
                &mut updater,
                self.runtime,
                &prior.inputs,
                prior.repository_results,
                prior.path_observations,
            )
            .map_err(NativeDemandSessionError::Restoration)?;
            let transaction = updater.commit().await;
            drop(transaction);
            Ok::<_, NativeDemandSessionError>(())
        })?;
        self.runtime
            .repository_materializer
            .discard(self.repository_session)
            .map_err(NativeDemandSessionError::Repository)?;
        self.runtime.native_demand_sessions.close(self.lease)
    }

    fn selected_snapshot(
        &self,
        selected: SelectedWorkspaceDemands,
    ) -> Result<
        (
            AcceptedNativeDemandSnapshot,
            Vec<super::repository_io::RepositoryValidation>,
        ),
        NativeDemandSessionError,
    > {
        let repository_results = self
            .runtime
            .repository_materializer
            .selected_epoch(self.repository_session, selected.repository_requests())
            .map_err(NativeDemandSessionError::Repository)?;
        let mut selected_paths = selected.unscoped_paths().to_vec();
        for validation in selected.repository_validations() {
            selected_paths.extend(validation.paths().iter().cloned());
        }
        selected_paths.sort_unstable();
        selected_paths.dedup();
        let path_observations = PathObservationEpoch::new(
            selected_paths
                .iter()
                .map(|demand| {
                    let result = self.path_observations.get(demand).ok_or_else(|| {
                        NativeDemandSessionError::MissingSelectedPath(demand.clone())
                    })?;
                    Ok((demand.clone(), result.as_ref().clone()))
                })
                .collect::<Result<Vec<_>, NativeDemandSessionError>>()?,
        )
        .map_err(NativeDemandSessionError::PathEpoch)?;

        let mut grouped: Vec<(
            Arc<RepositoryMaterializationRequest>,
            Vec<(
                PathObservationDemand,
                slug_workspace_v2::PathObservationResult,
            )>,
        )> = Vec::new();
        for scope in selected.repository_validations() {
            let index = grouped
                .iter()
                .position(|(request, _)| request.id == scope.request().id);
            let observations = scope
                .paths()
                .iter()
                .map(|demand| {
                    let result = path_observations.get(demand).ok_or_else(|| {
                        NativeDemandSessionError::MissingSelectedPath(demand.clone())
                    })?;
                    Ok((demand.clone(), result.as_ref().clone()))
                })
                .collect::<Result<Vec<_>, NativeDemandSessionError>>()?;
            if let Some(index) = index {
                if grouped[index].0 != *scope.request() {
                    return Err(NativeDemandSessionError::ConflictingRepository(
                        scope.request().id.canonical_repo.clone(),
                    ));
                }
                grouped[index].1.extend(observations);
            } else {
                grouped.push((scope.request().clone(), observations));
            }
        }
        let validation = grouped
            .into_iter()
            .map(|(request, mut observations)| {
                observations.sort_by(|left, right| left.0.cmp(&right.0));
                observations.dedup_by(|left, right| left.0 == right.0);
                super::repository_io::RepositoryValidation::new(request, observations)
            })
            .collect();
        Ok((
            AcceptedNativeDemandSnapshot {
                inputs: self.inputs.clone(),
                repository_results,
                path_observations,
                selected,
            },
            validation,
        ))
    }
}

#[allow(dead_code)]
impl<'a> NativeDemandAbortGuard<'a> {
    fn new(command: NativeDemandCommand<'a>) -> Self {
        Self {
            command: Some(command),
            attempt: None,
            phase: NativeDemandAbortPhase::Restorable,
        }
    }

    fn command(&self) -> &NativeDemandCommand<'a> {
        self.command
            .as_ref()
            .expect("an armed native-demand guard owns its command")
    }

    fn begin_attempt(&mut self) -> Result<(), NativeDemandSessionError> {
        assert!(
            self.attempt.is_none(),
            "a native-demand guard begins only one live attempt"
        );
        self.attempt = Some(self.command().begin_attempt()?);
        Ok(())
    }

    fn attempt_user_computation_data(
        &self,
    ) -> Result<UserComputationData, NativeDemandSessionError> {
        self.command().attempt_user_computation_data(
            self.attempt
                .as_ref()
                .expect("native-demand attempt is live"),
        )
    }

    fn inject_attempt(
        &self,
        updater: &mut DiceTransactionUpdater,
    ) -> Result<(), NativeDemandSessionError> {
        self.command().inject_attempt(updater)
    }

    fn seal_retry(&mut self) -> Result<(), NativeDemandSessionError> {
        self.attempt
            .as_ref()
            .expect("native-demand retry attempt is live")
            .seal_retry()?;
        self.attempt = None;
        Ok(())
    }

    fn seal_terminal(&mut self) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let attempt = self
            .attempt
            .as_ref()
            .expect("native-demand terminal attempt is live");
        let sealed = attempt.seal_terminal()?;
        self.attempt = None;
        Ok(sealed)
    }

    fn seal_terminal_allowing_empty_roots(
        &mut self,
    ) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .attempt
            .as_ref()
            .expect("native-demand terminal attempt is live")
            .seal_terminal_allowing_empty_roots()?;
        self.attempt = None;
        Ok(sealed)
    }

    fn seal_terminal_allowing_unavailable_roots(
        &mut self,
    ) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .attempt
            .as_ref()
            .expect("native-demand terminal attempt is live")
            .seal_terminal_allowing_unavailable_roots()?;
        self.attempt = None;
        Ok(sealed)
    }

    fn progress(
        &mut self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<NativeDemandProgress, NativeDemandSessionError> {
        self.command
            .as_mut()
            .expect("native-demand command is armed")
            .progress(needs)
    }

    fn suppress_attempt(&mut self) -> Result<(), NativeDemandSessionError> {
        let Some(attempt) = self.attempt.as_ref() else {
            return Ok(());
        };
        let result = attempt
            .tracker
            .finish_suppressed()
            .map_err(NativeDemandSessionError::Effect);
        self.attempt = None;
        result
    }

    fn abort<T>(
        &mut self,
        original: NativeDemandSessionError,
    ) -> Result<T, NativeDemandSessionError> {
        if self.phase != NativeDemandAbortPhase::Restorable {
            self.phase = NativeDemandAbortPhase::FailClosed;
            return Err(original);
        }
        let suppression = self.suppress_attempt();
        let restoration = self
            .command
            .as_mut()
            .expect("restorable native-demand guard owns its command")
            .discard_in_place();
        match restoration {
            Err(error) => {
                self.phase = NativeDemandAbortPhase::FailClosed;
                Err(error)
            }
            Ok(()) => {
                self.phase = NativeDemandAbortPhase::Closed;
                self.command = None;
                match suppression {
                    Ok(()) => Err(original),
                    Err(error) => Err(error),
                }
            }
        }
    }

    #[cfg(test)]
    fn discard(&mut self) -> Result<(), NativeDemandSessionError> {
        let suppression = self.suppress_attempt();
        let restoration = self
            .command
            .as_mut()
            .expect("restorable native-demand guard owns its command")
            .discard_in_place();
        match restoration {
            Err(error) => {
                self.phase = NativeDemandAbortPhase::FailClosed;
                Err(error)
            }
            Ok(()) => {
                self.phase = NativeDemandAbortPhase::Closed;
                self.command = None;
                suppression
            }
        }
    }

    #[cfg(test)]
    fn accept_selected_for_test(
        &mut self,
        demands: SelectedWorkspaceDemands,
    ) -> Result<AcceptedCommand<()>, NativeDemandSessionError> {
        let selected = NativeDemandTerminalSelection {
            effects: self.command().effects.clone(),
            sidecars: SelectedCommandSidecars::for_test(demands),
        };
        let prepared = {
            let command = self.command();
            command.runtime.runtime.block_on(async {
                let updater = command.runtime.dice.updater_with_data(
                    command
                        .runtime
                        .user_computation_data(None)
                        .map_err(NativeDemandSessionError::Effect)?,
                );
                let terminal_authority = updater.existing_state().await;
                let prepared = self.prepare_accept(selected, &terminal_authority).await?;
                drop(terminal_authority);
                Ok::<_, NativeDemandSessionError>(prepared)
            })
        };
        match prepared {
            Ok(prepared) => self.accept_prepared(prepared, ()),
            Err(error) => self.abort(error),
        }
    }

    async fn prepare_accept(
        &self,
        selected: NativeDemandTerminalSelection,
        terminal_authority: &dice::DiceTransaction,
    ) -> Result<NativeDemandPreparedAcceptance, NativeDemandSessionError> {
        if !Arc::ptr_eq(&self.command().effects, &selected.effects) {
            return Err(NativeDemandSessionError::ForeignEffects);
        }
        let (events, demands) = selected.sidecars.into_parts();
        let (snapshot, validation) = self.command().selected_snapshot(demands)?;
        commit_selected_native_demand_snapshot(self.command(), terminal_authority, &snapshot)
            .await?;
        Ok(NativeDemandPreparedAcceptance {
            events,
            snapshot,
            validation,
        })
    }

    fn accept_prepared<T>(
        &mut self,
        prepared: NativeDemandPreparedAcceptance,
        terminal: T,
    ) -> Result<AcceptedCommand<T>, NativeDemandSessionError> {
        let NativeDemandPreparedAcceptance {
            events,
            snapshot,
            validation,
        } = prepared;
        let materializer_accept = {
            let command = self.command();
            command.runtime.repository_materializer.accept(
                command.repository_session,
                snapshot.selected.repository_requests(),
                validation,
            )
        };
        if let Err(error) = materializer_accept {
            return self.abort(NativeDemandSessionError::Repository(error));
        }
        #[cfg(test)]
        self.command()
            .runtime
            .native_demand_sessions
            .record_trace(NativeDemandTestTrace::MaterializerAccepted);
        self.phase = NativeDemandAbortPhase::Irreversible;

        let replace = {
            let command = self.command();
            command
                .runtime
                .native_demand_sessions
                .replace_accepted(command.lease, snapshot)
        };
        if let Err(error) = replace {
            self.phase = NativeDemandAbortPhase::FailClosed;
            return Err(error);
        }
        #[cfg(test)]
        self.command()
            .runtime
            .native_demand_sessions
            .record_trace(NativeDemandTestTrace::AcceptedSnapshotReplaced);
        let output = events.into_output_buffer();
        #[cfg(test)]
        self.command()
            .runtime
            .native_demand_sessions
            .record_trace(NativeDemandTestTrace::OutputBufferMoved);
        let close = {
            let command = self.command();
            command.runtime.native_demand_sessions.close(command.lease)
        };
        if let Err(error) = close {
            self.phase = NativeDemandAbortPhase::FailClosed;
            return Err(error);
        }
        #[cfg(test)]
        self.command()
            .runtime
            .native_demand_sessions
            .record_trace(NativeDemandTestTrace::LeaseClosed);
        self.phase = NativeDemandAbortPhase::Closed;
        self.command = None;
        Ok(AcceptedCommand::new(terminal, output))
    }
}

impl Drop for NativeDemandAbortGuard<'_> {
    fn drop(&mut self) {
        if self.phase != NativeDemandAbortPhase::Restorable {
            return;
        }
        let _suppressed = self.suppress_attempt();
        match self
            .command
            .as_mut()
            .expect("restorable native-demand guard owns its command")
            .discard_in_place()
        {
            Ok(()) => {
                self.phase = NativeDemandAbortPhase::Closed;
                self.command = None;
            }
            Err(_) => {
                self.phase = NativeDemandAbortPhase::FailClosed;
            }
        }
    }
}

#[allow(dead_code)]
impl NativeDemandAttempt {
    fn seal_retry(&self) -> Result<(), NativeDemandSessionError> {
        self.tracker
            .seal_retry()
            .map_err(NativeDemandSessionError::Effect)
    }

    fn seal_terminal(&self) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .tracker
            .seal_terminal()
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandSealedAttempt {
            effects: self.effects.clone(),
            sealed,
        })
    }

    fn seal_terminal_allowing_empty_roots(
        &self,
    ) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .tracker
            .seal_terminal_allowing_empty_roots()
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandSealedAttempt {
            effects: self.effects.clone(),
            sealed,
        })
    }

    fn seal_terminal_allowing_unavailable_roots(
        &self,
    ) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .tracker
            .seal_terminal_allowing_unavailable_roots()
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandSealedAttempt {
            effects: self.effects.clone(),
            sealed,
        })
    }
}

#[allow(dead_code)]
impl NativeDemandSealedAttempt {
    fn root_count(&self) -> usize {
        self.sealed.root_count()
    }

    async fn select(
        self,
        transaction: &dice::DiceTransaction,
    ) -> Result<NativeDemandTerminalSelection, NativeDemandSessionError> {
        let sidecars = self
            .sealed
            .select(transaction)
            .await
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandTerminalSelection {
            effects: self.effects,
            sidecars,
        })
    }
}

#[allow(dead_code)]
impl NativeDemandTerminalSelection {
    fn sidecars(&self) -> &SelectedCommandSidecars {
        &self.sidecars
    }
}

async fn commit_selected_native_demand_snapshot(
    command: &NativeDemandCommand<'_>,
    terminal_authority: &dice::DiceTransaction,
    snapshot: &AcceptedNativeDemandSnapshot,
) -> Result<(), NativeDemandSessionError> {
    #[cfg(test)]
    if command
        .runtime
        .native_demand_sessions
        .take_selected_injection_failure(command.lease)?
    {
        return Err(NativeDemandSessionError::Injection(anyhow::anyhow!(
            "forced selected snapshot injection failure"
        )));
    }
    let mut updater = command.runtime.dice.updater_with_data(
        command
            .runtime
            .user_computation_data(None)
            .map_err(|error| {
                NativeDemandSessionError::Injection(anyhow::anyhow!(error.to_string()))
            })?,
    );
    inject_native_demand_snapshot(
        &mut updater,
        command.runtime,
        &snapshot.inputs,
        snapshot.repository_results.clone(),
        snapshot.path_observations.clone(),
    )
    .map_err(NativeDemandSessionError::Injection)?;
    let selected_snapshot_transaction = updater.commit().await;
    drop(selected_snapshot_transaction);
    // This explicit use after the selected commit makes the terminal
    // transaction's authority lifetime part of the helper contract.
    std::hint::black_box(terminal_authority);
    #[cfg(test)]
    command
        .runtime
        .native_demand_sessions
        .record_trace(NativeDemandTestTrace::SelectedInjectionCommitted);
    Ok(())
}

#[allow(dead_code)]
fn inject_native_demand_snapshot(
    updater: &mut DiceTransactionUpdater,
    runtime: &WorkspaceRuntime,
    inputs: &NativeDemandInputBundle,
    repository_results: RepositoryMaterializationResultEpoch,
    path_observations: PathObservationEpoch,
) -> anyhow::Result<()> {
    let workspace = NormalizedAbsolutePath::new(runtime.workspace.clone())
        .context("normalizing native-demand workspace")?;
    inject_root_module_request_inputs(
        updater,
        &runtime.workspace,
        inputs.request.command_policy.clone(),
        inputs.request.environment_policy.clone(),
        inputs.request.lockfile_mode.clone(),
    )
    .context("injecting fixed root module request inputs")?;
    inject_root_package_policy_inputs(
        updater,
        RootPackagePolicyInputs::new(
            workspace.clone(),
            [workspace.clone()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .context("constructing fixed root package policy inputs")?,
    )
    .context("injecting fixed root package policy inputs")?;
    inject_registry_request_inputs(
        updater,
        &runtime.workspace,
        inputs.request.registry_urls.clone(),
        inputs.generations.registry,
    )
    .context("injecting fixed registry request inputs")?;
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: runtime.workspace.clone(),
            },
            inputs.generations.repository,
        )])
        .context("injecting fixed repository generation")?;
    updater
        .changed_to(vec![(
            NativeDemandWorkspaceRevisionKey {
                workspace: runtime.workspace.clone(),
            },
            NativeDemandWorkspaceRevision(inputs.generations.workspace_revision),
        )])
        .context("injecting fixed workspace revision")?;
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey { workspace },
            repository_results,
        )])
        .context("injecting complete repository workset")?;
    updater
        .changed_to(vec![(PathObservationEpochKey, path_observations)])
        .context("injecting complete path workset")
}

/// Open a one-shot workspace runtime and evaluate injected root observations.
pub fn evaluate_workspace(workspace: impl Into<PathBuf>) -> anyhow::Result<WorkspaceEvaluation> {
    let workspace = workspace.into();
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
    let runtime = WorkspaceRuntime::new(&workspace, super::ProcessHostOwner::native())?;
    let evaluation = runtime.evaluate_observations(observe_workspace(&workspace)?, &[])?;
    Ok(evaluation.workspace)
}

/// Evaluate root files and each requested root-repository BUILD package.
///
/// Single custom-rule targets are analyzed through the retained DICE
/// configured-target graph in the same committed transaction as package
/// loading. Package-wide and recursive patterns remain loading-only.
pub fn evaluate_workspace_targets(
    workspace: impl Into<PathBuf>,
    targets: &[TargetPattern],
) -> anyhow::Result<WorkspaceBuildEvaluation> {
    evaluate_workspace_targets_with_bzlmod_inputs(
        workspace,
        targets,
        BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
            .expect("default bzlmod environment policy"),
        LockfileMode::Update,
        &[],
    )
}

/// Evaluate root files and requested packages with explicit normalized
/// bzlmod inputs on the one-shot retained runtime.
pub fn evaluate_workspace_targets_with_bzlmod_inputs(
    workspace: impl Into<PathBuf>,
    targets: &[TargetPattern],
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
    lockfile_mode: LockfileMode,
    registry_urls: &[String],
) -> anyhow::Result<WorkspaceBuildEvaluation> {
    let workspace = workspace.into();
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
    let runtime = WorkspaceRuntime::new(&workspace, super::ProcessHostOwner::native())?;
    runtime.evaluate_observations_with_bzlmod_inputs(
        observe_workspace(&workspace)?,
        targets,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
    )
}

/// Resolve a target pattern to its workspace-relative package directory.
pub fn package_path_for_target(
    workspace: &Path,
    target: &TargetPattern,
) -> anyhow::Result<PathBuf> {
    let (repo, package) = match target {
        TargetPattern::Single(label) => (label.repo(), label.package()),
        TargetPattern::PackageAll { repo, package } => (repo, package),
        TargetPattern::Recursive { .. } => {
            anyhow::bail!(
                "recursive target patterns are not supported before Stage 6 analysis: {target}"
            );
        }
    };
    if !repo.is_root() {
        anyhow::bail!(
            "external repository target patterns are not supported before Stage 5 repository mapping: {target}"
        );
    }
    Ok(workspace.join(package.as_str()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DynKey;
    use dice::RootActivation;
    use slug_analysis_v2::key::RootStringSettingValue;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::PathOutcome;

    use super::*;
    use crate::runtime::ProcessHostOwner;
    use crate::runtime::events::CommandEffectOwner;

    fn test_runtime(workspace: impl Into<PathBuf>) -> anyhow::Result<WorkspaceRuntime> {
        WorkspaceRuntime::new(workspace, ProcessHostOwner::native())
    }

    #[test]
    fn runtime_keeps_the_explicit_process_host_arc() {
        let workspace = tempfile::tempdir().unwrap();
        let owner = ProcessHostOwner::unsupported();
        let runtime = WorkspaceRuntime::new(workspace.path(), owner.clone()).unwrap();
        assert!(Arc::ptr_eq(runtime.process_host(), &owner));
        assert_eq!(Arc::strong_count(&owner), 2);
    }

    #[test]
    fn cquery_evaluator_terminal_classification_is_narrow() {
        let evaluation = CqueryCommandError::from_evaluator_error(QueryError::evaluation(
            "argument set is empty",
        ));
        assert_eq!(evaluation.exit_code(), 1);
        assert_eq!(evaluation.to_string(), "argument set is empty");
        assert!(evaluation.missing_stderr().is_none());

        let syntax = CqueryCommandError::from_evaluator_error(QueryError::syntax("bad count"));
        assert!(matches!(syntax, CqueryCommandError::Request(_)));
        assert_eq!(syntax.exit_code(), 2);
        assert_eq!(
            CqueryCommandError::infrastructure("infrastructure").exit_code(),
            2
        );
    }

    #[test]
    fn cquery_loading_files_post_analysis_terminal_is_an_evaluation_error() {
        let terminal = CqueryCommandError::from_evaluator_error(QueryError::evaluation(
            "buildfiles() doesn't make sense for the configured target graph",
        ));
        assert_eq!(terminal.exit_code(), 1);
        assert_eq!(
            terminal.to_string(),
            "buildfiles() doesn't make sense for the configured target graph"
        );
    }

    #[test]
    fn cquery_siblings_post_analysis_terminal_only_checks_emptiness() {
        let empty = TargetSet::<&str>::default();
        let result = cquery_post_analysis_siblings(&empty).unwrap();
        assert!(result.iter().next().is_none());

        let mut nonempty = TargetSet::default();
        nonempty.insert("configured-rule");
        let error = cquery_post_analysis_siblings(&nonempty).unwrap_err();
        assert_eq!(
            error.to_string(),
            "siblings() not supported for post analysis queries"
        );
        let terminal = CqueryCommandError::from_evaluator_error(error);
        assert_eq!(terminal.exit_code(), 1);
        assert_eq!(
            terminal.to_string(),
            "siblings() not supported for post analysis queries"
        );
    }

    #[test]
    fn cquery_visible_post_analysis_terminal_preserves_vacuous_target_sets() {
        let empty = TargetSet::<&str>::default();
        let mut targets = TargetSet::default();
        targets.insert("target-b");
        targets.insert("target-a");
        targets.insert("target-b");
        let vacuous = cquery_post_analysis_visible(&empty, &targets).unwrap();
        assert_eq!(
            vacuous.iter().copied().collect::<Vec<_>>(),
            ["target-b", "target-a"]
        );

        let mut callers = TargetSet::default();
        callers.insert("caller");
        let empty_targets = cquery_post_analysis_visible(&callers, &empty).unwrap();
        assert!(empty_targets.iter().next().is_none());

        let error = cquery_post_analysis_visible(&callers, &targets).unwrap_err();
        assert_eq!(
            error.to_string(),
            "visible() is not supported on configured targets"
        );
        let terminal = CqueryCommandError::from_evaluator_error(error);
        assert_eq!(terminal.exit_code(), 1);
    }

    #[test]
    fn cquery_kind_maps_structural_kinds_and_fails_closed_as_a_request_terminal() {
        assert_eq!(
            cquery_target_kind_for_query(&ConfiguredNodeKind::SourceFile, None).unwrap(),
            "source file"
        );
        assert_eq!(
            cquery_target_kind_for_query(&ConfiguredNodeKind::GeneratedFile, None).unwrap(),
            "generated file"
        );
        assert_eq!(
            cquery_target_kind_for_query(&ConfiguredNodeKind::PackageGroup, None).unwrap(),
            "package group"
        );
        let error = cquery_target_kind_for_query(&ConfiguredNodeKind::Platform, None).unwrap_err();
        let terminal = CqueryCommandError::from_evaluator_error(error);
        assert!(matches!(terminal, CqueryCommandError::Request(_)));
        assert_eq!(terminal.exit_code(), 2);
        assert!(terminal.to_string().contains("Platform"));
    }

    #[test]
    fn cquery_label_kind_fails_closed_for_unadmitted_capability_free_nodes() {
        assert_eq!(
            cquery_target_kind(&ConfiguredNodeKind::SourceFile, None).unwrap(),
            "source file"
        );
        let error = cquery_target_kind(&ConfiguredNodeKind::Platform, None).unwrap_err();
        assert!(matches!(error, CqueryCommandError::Infrastructure(_)));
        assert!(error.to_string().contains("Platform"));
    }

    #[test]
    fn cquery_executables_uses_rule_capability_order_and_full_key_dedupe() {
        use slug_loading_v2::RuleCapability;

        #[derive(Clone)]
        struct MatrixTarget {
            key: ConfiguredTargetKey,
            capability: Option<RuleCapability>,
        }

        impl PartialEq for MatrixTarget {
            fn eq(&self, other: &Self) -> bool {
                self.key == other.key
            }
        }

        impl Eq for MatrixTarget {}

        impl Hash for MatrixTarget {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.key.hash(state);
            }
        }

        let capability = |rule_class: &str, executable| RuleCapability {
            rule_class: rule_class.into(),
            executable,
            test_kind: None,
        };
        assert!(!is_cquery_executable_non_test(None));
        assert!(!is_cquery_executable_non_test(Some(&capability(
            "exec_rule",
            false
        ))));
        assert!(!is_cquery_executable_non_test(Some(&capability(
            "exported_test",
            true
        ))));
        assert!(is_cquery_executable_non_test(Some(&capability(
            "exec_rule",
            true
        ))));
        let target = |name: &str, capability| MatrixTarget {
            key: ConfiguredTargetKey::new(
                CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap(),
                ConfigurationKey::target("matrix").unwrap(),
            ),
            capability,
        };
        let nonexec = target("nonexec", Some(capability("exec_rule", false)));
        let target_named_test = target("target_named_test", Some(capability("exec_rule", true)));
        let exported_test = target(
            "exported_test_target",
            Some(capability("exported_test", true)),
        );
        let executable = target("executable_non_test", Some(capability("exec_rule", true)));
        let no_capability = target("no_capability", None);
        let mut operand = TargetSet::default();
        for target in [
            nonexec,
            target_named_test,
            exported_test,
            executable.clone(),
            executable,
            no_capability,
        ] {
            operand.insert(target);
        }
        assert_eq!(operand.iter().count(), 5, "full configured key dedupes");
        let result = filter_cquery_executable_non_tests(&operand, |target| {
            is_cquery_executable_non_test(target.capability.as_ref())
        });
        assert_eq!(
            result
                .iter()
                .map(|target| target.key.label().target().as_str())
                .collect::<Vec<_>>(),
            ["target_named_test", "executable_non_test"],
        );
    }

    #[test]
    fn cquery_only_typed_missing_executable_analysis_uses_exit_one() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"typed\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _missing(ctx): return [DefaultInfo()]\ndef _ordinary(ctx): return \"not a provider list\"\nmissing = rule(implementation = _missing, executable = True)\nordinary = rule(implementation = _ordinary)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"missing\", \"ordinary\")\nmissing(name = \"missing\")\nordinary(name = \"ordinary\")\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let run = |expression: &str| {
            runtime
                .cquery_command_with_bzlmod_inputs(
                    expression,
                    true,
                    true,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap()
        };
        let missing = run("//pkg:missing");
        let missing = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(matches!(
            missing,
            CqueryCommandError::ExecutableRuleMissingExecutable(_)
        ));
        assert_eq!(missing.exit_code(), 1);
        assert!(missing.missing_stderr().is_none());
        assert_eq!(
            missing.to_string(),
            "The rule 'missing' is executable. It needs to create an executable File and pass it as the 'executable' parameter to the DefaultInfo it returns."
        );

        let ordinary = run("//pkg:ordinary");
        let ordinary = ordinary.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(matches!(ordinary, CqueryCommandError::Analysis(_)));
        assert_eq!(ordinary.exit_code(), 2);
    }

    #[derive(Debug, Clone, Allocative)]
    struct NativeDemandHandshakeKey {
        requests: Arc<[Arc<RepositoryMaterializationRequest>]>,
        path: PathObservationDemand,
        generations: NativeDemandGenerationBundle,
    }

    impl PartialEq for NativeDemandHandshakeKey {
        fn eq(&self, other: &Self) -> bool {
            self.requests == other.requests
                && self.path == other.path
                && self.generations == other.generations
        }
    }

    impl Eq for NativeDemandHandshakeKey {}

    impl Hash for NativeDemandHandshakeKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            for request in self.requests.iter() {
                request.id.workspace.hash(state);
                request.id.canonical_repo.hash(state);
            }
            self.path.hash(state);
            self.generations.workspace_revision.0.hash(state);
            self.generations.registry.0.hash(state);
            self.generations.repository.0.hash(state);
        }
    }

    impl fmt::Display for NativeDemandHandshakeKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "native-demand-handshake:{}:{:?}",
                self.requests
                    .iter()
                    .map(|request| request.id.canonical_repo.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                self.path
            )
        }
    }

    #[async_trait]
    impl Key for NativeDemandHandshakeKey {
        type Value = slug_bzlmod_v2::SourcePreparationOutcome<Arc<str>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let workspace = self
                .requests
                .first()
                .expect("handshake has at least one repository")
                .id
                .workspace
                .clone();
            assert_eq!(
                ctx.compute(&RegistryRequestGenerationKey {
                    workspace: workspace.as_path().to_path_buf(),
                })
                .await
                .expect("the command injects its registry generation"),
                self.generations.registry
            );
            assert_eq!(
                ctx.compute(&RepositoryMaterializationGenerationKey {
                    workspace: workspace.as_path().to_path_buf(),
                })
                .await
                .expect("the command injects its repository generation"),
                self.generations.repository
            );
            let epoch = ctx
                .compute(&RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                })
                .await
                .expect("the command injects a complete repository epoch");
            let empty = RepositoryMaterializationResultEpoch::new(workspace.clone(), []).unwrap();
            if epoch == empty {
                let mut needs = slug_bzlmod_v2::SourcePreparationNeeds::path(
                    slug_workspace_v2::NeedPathObservations::singleton(self.path.clone()),
                );
                for request in self.requests.iter() {
                    needs = needs
                        .try_union(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                            request.as_ref().clone(),
                        ))
                        .expect("distinct handshake repositories");
                }
                return slug_bzlmod_v2::SourcePreparationOutcome::Need(needs);
            }
            let expected = RepositoryMaterializationResultEpoch::new(
                workspace,
                self.requests.iter().map(|request| {
                    slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                        request: request.clone(),
                        result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                            slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
                        ),
                    }
                }),
            )
            .unwrap();
            assert_eq!(
                epoch, expected,
                "every retry injects the exact cumulative repository epoch"
            );
            match ctx
                .compute(&PathObservationKey::new(self.path.clone()))
                .await
                .expect("path observation projection computes")
            {
                PathOutcome::Need(need) => slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    slug_bzlmod_v2::SourcePreparationNeeds::path(need),
                ),
                PathOutcome::Complete(_) => {
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete("complete".into())
                }
            }
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }

        fn provide<'a>(&'a self, demand: &mut dice::Demand<'a>) {
            demand.provide_value_with(|| self.requests[0].clone());
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
    struct NativeTerminalProbeKey;

    impl fmt::Display for NativeTerminalProbeKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("native-terminal-probe")
        }
    }

    #[async_trait]
    impl Key for NativeTerminalProbeKey {
        type Value = bool;

        async fn compute(
            &self,
            _ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            true
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x == y
        }
    }

    fn local_native_request(
        workspace: &NormalizedAbsolutePath,
        repository: &str,
        relative: &str,
    ) -> Arc<RepositoryMaterializationRequest> {
        use compact_str::CompactString;
        use slug_bzlmod_v2::OverrideAttributeValue;
        use slug_bzlmod_v2::RepoRuleId;
        use slug_bzlmod_v2::RepoSpec;
        use slug_bzlmod_v2::RepositoryMaterializationKind;
        use slug_bzlmod_v2::RepositoryMaterializationRequestId;
        use starlark_map::small_map::SmallMap;

        let logical_root = NormalizedAbsolutePath::new(workspace.as_path().join(relative)).unwrap();
        Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: slug_identity_v2::CanonicalRepoName::new(repository).unwrap(),
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
                    CompactString::new("path"),
                    OverrideAttributeValue::String(relative.into()),
                )])),
            },
            kind: RepositoryMaterializationKind::Local { logical_root },
        })
    }

    fn native_host_file_demand(path: PathBuf) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationOperation::FileBytes,
        )
    }

    fn synthetic_plan(
        id: u64,
        workspace: &NormalizedAbsolutePath,
        repositories: impl IntoIterator<Item = Arc<RepositoryMaterializationRequest>>,
        paths: impl IntoIterator<Item = PathObservationDemand>,
        terminal: Result<SyntheticCommandValue, SyntheticCommandError>,
    ) -> Arc<SyntheticCommandPlan> {
        Arc::new(SyntheticCommandPlan {
            id,
            workspace: workspace.clone(),
            repositories: repositories.into_iter().collect::<Vec<_>>().into(),
            paths: paths.into_iter().collect::<Vec<_>>().into(),
            terminal,
            retry_event: Some("retry-only".into()),
            terminal_event: Some("terminal-only".into()),
            behavior: SyntheticRootBehavior::Normal,
        })
    }

    fn accepted_output_text<T>(accepted: &AcceptedCommand<T>) -> Vec<&str> {
        accepted
            .batches_for_test()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => {
                    unreachable!("diagnostic events are not produced by this packet")
                }
            })
            .collect()
    }

    fn projected_output_text<T>(output: &crate::runtime::CommandOutput<T>) -> Vec<&str> {
        output
            .batches_for_test()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => {
                    unreachable!("diagnostic events are not produced by this packet")
                }
            })
            .collect()
    }

    #[test]
    fn real_query_command_drives_typed_results_and_cold_events_without_warm_replay() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"driver\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "print(\"BZL_EVENT\")\nNAME = \"probe\"\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_EVENT\")\nfilegroup(name = NAME)\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let query = |runtime: &WorkspaceRuntime, expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };

        let accepted = query(&runtime, "deps(//pkg:probe)").unwrap();
        assert_eq!(
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "//pkg:probe\n"
        );
        assert_eq!(
            accepted_output_text(&accepted),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT"]
        );

        let warm = query(&runtime, "deps(//pkg:probe)").unwrap();
        assert!(warm.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&warm).is_empty());

        let empty = query(&runtime, "set()").unwrap();
        assert_eq!(
            empty
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            ""
        );

        let missing_runtime = test_runtime(workspace.path()).unwrap();
        let missing = query(&missing_runtime, "//pkg:missing").unwrap();
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such target '//pkg:missing': target 'missing' not declared in package 'pkg'"
        );
        assert_eq!(
            accepted_output_text(&missing),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT"]
        );
    }

    #[test]
    fn direct_external_query_uses_host_route_native_materialization_and_apparent_output() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"driver\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("dep")).unwrap();
        fs::write(
            workspace.path().join("dep/MODULE.bazel"),
            "module(name = \"dep\", version = \"1.0.0\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\", srcs = [\"target.txt\", \"missing_input.txt\"])\nalias(name = \"files_alias\", actual = \":files\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"})\ntest_suite(name = \"suite_omitted\")\ntest_suite(name = \"suite_empty\", tests = [], tags = [\"manual\", \"a\"])\ntest_suite(name = \"suite_parent\", tests = [\":suite_empty\"])\ntest_suite(name = \"suite_cycle_a\", tests = [\":suite_cycle_b\"])\ntest_suite(name = \"suite_cycle_b\", tests = [\":suite_cycle_a\"])\npackage_group(name = \"pg_empty\")\npackage_group(name = \"pg_nonempty\", packages = [\"//pkg\", \"//tree/...\", \"-//blocked\", \"-//blocked_tree/...\", \"public\", \"private\"])\npackage_group(name = \"pg_leaf\", packages = [\"//leaf\"])\npackage_group(name = \"pg_parent\", includes = [\":pg_leaf\"])\npackage_group(name = \"pg_cycle_a\", includes = [\":pg_cycle_b\"])\npackage_group(name = \"pg_cycle_b\", includes = [\":pg_cycle_a\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/target.txt"), "target").unwrap();

        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(activation_audit.clone());
        let query = |expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
        };
        let query_label_kind = |expression: &str| {
            runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::LabelKind,
            )
        };

        let phase = activation_audit.checkpoint();
        let first = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert_eq!(
            first
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(
            accepted_output_text(&first),
            ["MODULE_EVENT", "EXTERNAL_BUILD_EVENT"]
        );

        let files = query("@dep//:files").unwrap();
        assert_eq!(
            files
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n"
        );
        assert_eq!(
            query("labels(srcs, @dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("deps(@dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("@dep//:files_alias")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files_alias\n"
        );
        assert_eq!(
            query("labels(actual, @dep//:files_alias)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n"
        );
        assert_eq!(
            query("deps(@dep//:files_alias)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:files\n@dep//:files_alias\n@dep//:missing_input.txt\n@dep//:target.txt\n"
        );
        assert_eq!(
            query("@dep//:is_k8")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:is_k8\n"
        );
        assert_eq!(
            query("deps(@dep//:is_k8)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:is_k8\n"
        );
        assert_eq!(
            query_label_kind("@dep//:is_k8")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "config_setting rule @dep//:is_k8\n"
        );
        assert!(
            query("labels(visibility, @dep//:files)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("@dep//:suite_omitted")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_omitted\n"
        );
        assert_eq!(
            query("@dep//:suite_empty")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n"
        );
        assert_eq!(
            query_label_kind("@dep//:suite_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "test_suite rule @dep//:suite_parent\n"
        );
        assert!(
            query("labels($implicit_tests, @dep//:suite_omitted)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("labels(tests, @dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n"
        );
        assert_eq!(
            query("deps(@dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_empty\n@dep//:suite_parent\n"
        );
        assert!(
            query("tests(@dep//:suite_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("deps(@dep//:suite_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_cycle_a\n@dep//:suite_cycle_b\n"
        );
        assert!(
            query("tests(@dep//:suite_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );
        assert_eq!(
            query("@dep//:pg_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_parent\n"
        );
        assert_eq!(
            query_label_kind("@dep//:pg_parent")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_kind_stdout(),
            "package group @dep//:pg_parent\n"
        );
        assert_eq!(
            query("deps(@dep//:pg_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_leaf\n@dep//:pg_parent\n"
        );
        assert_eq!(
            query("deps(@dep//:pg_cycle_a)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_cycle_a\n@dep//:pg_cycle_b\n"
        );
        assert!(
            query("labels(visibility, @dep//:pg_parent)")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout()
                .is_empty()
        );

        // An attribute-created external source is semantic loading state, not
        // a source-file observation. It remains addressable while absent.
        fs::write(workspace.path().join("dep/missing_input.txt"), "present").unwrap();
        let created = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&created).is_empty());
        let suite_after_source_create = query("@dep//:suite_parent").unwrap();
        assert_eq!(
            suite_after_source_create
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:suite_parent\n"
        );
        assert!(accepted_output_text(&suite_after_source_create).is_empty());
        let group_after_source_create = query("@dep//:pg_parent").unwrap();
        assert_eq!(
            group_after_source_create
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:pg_parent\n"
        );
        assert!(accepted_output_text(&group_after_source_create).is_empty());
        let setting_after_source_create = query("@dep//:is_k8").unwrap();
        assert!(accepted_output_text(&setting_after_source_create).is_empty());
        fs::write(workspace.path().join("dep/missing_input.txt"), "edited").unwrap();
        let edited_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&edited_source).is_empty());
        fs::remove_file(workspace.path().join("dep/missing_input.txt")).unwrap();
        let deleted_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&deleted_source).is_empty());
        fs::write(workspace.path().join("dep/missing_input.txt"), "recreated").unwrap();
        let recreated_source = query("deps(@dep//:files)").unwrap();
        assert!(accepted_output_text(&recreated_source).is_empty());
        fs::remove_file(workspace.path().join("dep/missing_input.txt")).unwrap();

        for (build, expected) in [
            (
                "filegroup(name = \"member\")\ntest_suite(name = \"other\", tests = [\":member\"])\n",
                "external repository test_suite non-suite member is deferred",
            ),
            (
                "package_group(name = \"group\", includes = [\":missing\"])\n",
                "external repository package_group missing include is deferred",
            ),
            (
                "filegroup(name = \"member\")\npackage_group(name = \"group\", includes = [\":member\"])\n",
                "external repository package_group non-package-group include is deferred",
            ),
            (
                "exports_files([\"target.txt\"])\nalias(name = \"member\", actual = \":target.txt\")\npackage_group(name = \"group\", includes = [\":member\"])\n",
                "external repository package_group alias include is deferred",
            ),
            (
                "package_group(name = \"group\", includes = [\"//other:member\"])\n",
                "external repository package_group cross-package include is deferred",
            ),
            (
                "filegroup(name = \"files\", srcs = [\"//other:item\"])\n",
                "external repository filegroup cross-package srcs are deferred",
            ),
            (
                "filegroup(name = \"group\")\nfilegroup(name = \"files\", visibility = [\":group\"])\n",
                "external repository visibility edges are deferred",
            ),
            (
                "filegroup(name = \"group\")\nfilegroup(name = \"files\")\nalias(name = \"files_alias\", actual = \":files\", visibility = [\":group\"])\n",
                "external repository visibility edges are deferred",
            ),
            (
                "filegroup(name = \"group\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"}, visibility = [\":group\"])\n",
                "external repository visibility edges are deferred",
            ),
            (
                "filegroup(name = \"group\")\ntest_suite(name = \"suite\", visibility = [\":group\"])\n",
                "external repository visibility edges are deferred",
            ),
            (
                "filegroup(name = \"BUILD.bazel\")\n",
                "collides with active BUILD file",
            ),
            (
                "config_setting(name = \"BUILD.bazel\", values = {\"cpu\": \"k8\"})\n",
                "collides with active BUILD file",
            ),
            (
                "filegroup(name = \"files\")\nalias(name = \"first\", actual = \":second\")\nalias(name = \"second\", actual = \":files\")\n",
                "external repository alias chains are deferred",
            ),
            (
                "alias(name = \"to_build\", actual = \":BUILD.bazel\")\n",
                "external repository alias actual destination is deferred",
            ),
            (
                "alias(name = \"cross\", actual = \"//other:item\")\n",
                "external repository alias cross-package actual is deferred",
            ),
        ] {
            fs::write(workspace.path().join("dep/BUILD.bazel"), build).unwrap();
            let stopped = test_runtime(workspace.path()).unwrap();
            let error = stopped
                .query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                    "@dep//:files",
                    QueryOrder::Auto,
                    QueryPolicy::default(),
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    QueryOutputCompletion::Standard,
                )
                .unwrap();
            let failure = error
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string();
            assert!(
                failure.contains(expected),
                "expected {expected:?}: {failure}"
            );
        }
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\", srcs = [\"target.txt\", \"missing_input.txt\"])\nalias(name = \"files_alias\", actual = \":files\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"})\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "alias(name = \"files_alias\", actual = \"@other//:item\")\n",
        )
        .unwrap();
        let stopped = test_runtime(workspace.path()).unwrap();
        let named_repository = stopped
            .query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                "@dep//:files",
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                QueryOutputCompletion::Standard,
            )
            .unwrap();
        assert!(
            named_repository
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository dependency labels are not supported"),
            "{named_repository:?}"
        );
        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let restored_after_stop_gates = query("@dep//:target.txt").unwrap();
        assert_eq!(
            accepted_output_text(&restored_after_stop_gates),
            ["EXTERNAL_BUILD_EVENT"]
        );
        let phase = activation_audit.checkpoint();
        let warm = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        assert!(accepted_output_text(&warm).is_empty());

        fs::rename(
            workspace.path().join("dep/BUILD.bazel"),
            workspace.path().join("dep/BUILD"),
        )
        .unwrap();
        let fallback = query("@dep//:target.txt").unwrap();
        assert_eq!(
            fallback
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(accepted_output_text(&fallback), ["EXTERNAL_BUILD_EVENT"]);

        fs::write(
            workspace.path().join("dep/BUILD"),
            "print(\"EXTERNAL_BUILD_EDITED\")\nexports_files([\"edited.txt\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/edited.txt"), "edited").unwrap();
        let edited = query("@dep//:edited.txt").unwrap();
        assert_eq!(
            edited
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:edited.txt\n"
        );
        assert_eq!(accepted_output_text(&edited), ["EXTERNAL_BUILD_EDITED"]);

        fs::remove_file(workspace.path().join("dep/BUILD")).unwrap();
        let phase = activation_audit.checkpoint();
        let deleted = query("@dep//:edited.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert!(
            deleted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("BUILD file not found")
        );

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let phase = activation_audit.checkpoint();
        let restored = query("@dep//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 2);
        assert_eq!(
            restored
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .stdout(),
            "@dep//:target.txt\n"
        );
        assert_eq!(accepted_output_text(&restored), ["EXTERNAL_BUILD_EVENT"]);

        let phase = activation_audit.checkpoint();
        let missing = query("@dep//:missing").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such target '@@dep+//:missing': target 'missing' not declared in package '' defined by <output_base>/external/dep+/BUILD.bazel"
        );
        assert_eq!(error.exit_code, 7);

        let missing_package = query("@dep//nope:missing").unwrap();
        let error = missing_package
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such package '@@dep+//nope': BUILD file not found in directory 'nope' of external repository @@dep+. Add a BUILD file to a directory to mark it as a package."
        );
        assert_eq!(error.exit_code, 7);

        let phase = activation_audit.checkpoint();
        let unknown = query("@missing//:target.txt").unwrap();
        activation_audit.assert_phase_clean(phase, 1);
        let error = unknown.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.to_string(),
            "no such package '@@[unknown repo 'missing' requested from @@]//': The repository '@@[unknown repo 'missing' requested from @@]' could not be resolved: No repository visible as '@missing' from main repository"
        );
        assert_eq!(error.exit_code, 7);

        for pattern in ["@dep//:all", "@dep//:*", "@dep//..."] {
            let pattern_error = query(pattern).unwrap();
            assert_eq!(
                pattern_error
                    .terminal_for_test()
                    .as_ref()
                    .as_ref()
                    .unwrap_err()
                    .to_string(),
                format!("external repository query patterns are deferred: {pattern}")
            );
        }

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "load(\":defs.bzl\", \"defs\")\nexports_files([\"target.txt\"])\n",
        )
        .unwrap();
        let load = query("@dep//:target.txt").unwrap();
        assert!(
            load.terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD loads are deferred")
        );

        fs::write(
            workspace.path().join("dep/BUILD.bazel"),
            "exports_files(glob([\"*.txt\"]))\n",
        )
        .unwrap();
        let glob = query("@dep//:target.txt").unwrap();
        assert!(
            glob.terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD globs are deferred")
        );

        fs::write(workspace.path().join("dep/BUILD.bazel"), [0xff]).unwrap();
        let invalid_utf8 = query("@dep//:target.txt").unwrap();
        assert!(
            invalid_utf8
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("external repository BUILD file is not UTF-8")
        );
    }

    #[test]
    fn real_build_command_drives_typed_analysis_and_cold_events_without_warm_replay() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"driver\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\"//pkg:defs.bzl\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "print(\"BZL_EVENT\")\ndef _impl(ctx):\n    print(\"ANALYSIS_EVENT\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\ndef _setting(ctx): return []\nstring_setting = rule(implementation = _setting, build_setting = config.string(flag = True))\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprint(\"BUILD_EVENT\")\nprobe(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let target = TargetPattern::parse("//pkg:probe").unwrap();
        let build =
            |runtime: &WorkspaceRuntime, targets: &[TargetPattern], setting: Option<&str>| {
                runtime.build_command_with_bzlmod_inputs(
                    targets,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    setting,
                )
            };

        let accepted = build(&runtime, std::slice::from_ref(&target), None).unwrap();
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(evaluation.loaded_package_count(), 1);
        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 0);
        assert_eq!(
            accepted_output_text(&accepted),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT", "ANALYSIS_EVENT"]
        );
        let c0 = evaluation
            .analyses()
            .next()
            .unwrap()
            .configured_target_key()
            .expect("current build analysis only contains configured nodes")
            .configuration()
            .clone();

        let warm = build(&runtime, std::slice::from_ref(&target), None).unwrap();
        assert!(warm.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&warm).is_empty());

        let transitioned = build(
            &runtime,
            std::slice::from_ref(&target),
            Some("transitioned"),
        )
        .unwrap();
        let c1 = transitioned
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .analyses()
            .next()
            .unwrap()
            .configured_target_key()
            .expect("current build analysis only contains configured nodes")
            .configuration()
            .clone();
        assert_ne!(c0, c1);
        assert_eq!(accepted_output_text(&transitioned), ["ANALYSIS_EVENT"]);
        assert_ne!(
            crate::runtime::configured_output_root(
                workspace.path(),
                c0.slug_configuration().unwrap()
            ),
            crate::runtime::configured_output_root(
                workspace.path(),
                c1.slug_configuration().unwrap()
            )
        );

        let restored = build(&runtime, std::slice::from_ref(&target), None).unwrap();
        let restored_configuration = restored
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .analyses()
            .next()
            .unwrap()
            .configured_target_key()
            .expect("current build analysis only contains configured nodes")
            .configuration();
        assert_eq!(&c0, restored_configuration);
        assert!(accepted_output_text(&restored).is_empty());

        let empty = build(&runtime, &[], None).unwrap();
        let evaluation = empty.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(evaluation.loaded_package_count(), 0);
        assert_eq!(evaluation.analyzed_target_count(), 0);

        let missing_runtime = test_runtime(workspace.path()).unwrap();
        let missing_target = TargetPattern::parse("//pkg:missing").unwrap();
        let missing = build(&missing_runtime, &[missing_target], None).unwrap();
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(matches!(
            error.kind,
            BuildCommandErrorKind::TargetNotFound { .. }
        ));
        assert_eq!(
            error.to_string(),
            format!(
                "target `//pkg:missing` was not found in {}",
                workspace.path().join("pkg/BUILD.bazel").display()
            )
        );
        assert_eq!(
            accepted_output_text(&missing),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT"]
        );
    }

    #[test]
    fn real_build_analysis_error_publishes_and_recovers_without_validating_the_error_node() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"driver\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            "def _impl(ctx): return [DefaultInfo()]\nprobe_rule = rule(implementation = _impl, attrs = {\"dep\": attr.label()})\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\":defs.bzl\", \"probe_rule\")\nfilegroup(name = \"native\")\nalias(name = \"broken\", actual = \":native\")\nprobe_rule(name = \"probe\", dep = \":broken\")\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let target = TargetPattern::parse("//:probe").unwrap();
        let build = |runtime: &WorkspaceRuntime| {
            runtime.build_command_with_bzlmod_inputs(
                std::slice::from_ref(&target),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
        };

        let failed = build(&runtime).unwrap();
        assert!(
            failed
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("not a Starlark rule")
        );

        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\":defs.bzl\", \"probe_rule\")\nprobe_rule(name = \"leaf\")\nalias(name = \"broken\", actual = \":leaf\")\nprobe_rule(name = \"probe\", dep = \":broken\")\n",
        )
        .unwrap();
        let recovered = build(&runtime).unwrap();
        assert!(recovered.terminal_for_test().as_ref().is_ok());
    }

    #[test]
    fn retained_runtime_restores_default_transition_configuration_after_explicit_override() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print(\"MODULE_EVENT\")\nmodule(name = \"configuration_driver\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            r#"SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx):
    print("CONSUMER_ANALYSIS")
    out = ctx.actions.declare_file("consumer.txt")
    ctx.actions.write(out, ctx.attr._setting[SettingInfo].value + "\n")
    return [DefaultInfo(files = depset([out]))]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
def _left(settings, attr):
    return {"//:setting": "left"}
left = transition(implementation = _left, inputs = [], outputs = ["//:setting"])
def _parent(ctx):
    print("PARENT_ANALYSIS")
    out = ctx.actions.declare_file("parent.txt")
    ctx.actions.write(out, "parent\n")
    return [DefaultInfo(files = depset([out]))]
parent = rule(implementation = _parent, attrs = {"child": attr.label(cfg = left)})
def _top(ctx):
    print("TOP_ANALYSIS")
    out = ctx.actions.declare_file("top.txt")
    ctx.actions.write(out, "top\n")
    return [DefaultInfo(files = depset([out]))]
top = rule(implementation = _top, attrs = {"child": attr.label()})
"#,
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\", \"top\")\nprint(\"BUILD_EVENT\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", child = \":consumer\")\ntop(name = \"top\", child = \":parent\")\n",
        )
        .unwrap();

        let target = TargetPattern::parse("//:parent").unwrap();
        let build = |runtime: &WorkspaceRuntime, setting: Option<&str>| {
            runtime.build_command_with_bzlmod_inputs(
                std::slice::from_ref(&target),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                setting,
            )
        };
        let configuration_for =
            |accepted: &AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
             label: &str| {
                accepted
                    .terminal_for_test()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .analyses()
                    .find(|analysis| analysis.key().label().to_string() == label)
                    .unwrap()
                    .configured_target_key()
                    .expect("current build analysis only contains configured nodes")
                    .configuration()
                    .clone()
            };
        let topology_for =
            |accepted: &AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>| {
                accepted
                    .terminal_for_test()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .analyses()
                    .map(|analysis| {
                        (
                            analysis.key().label().to_string(),
                            analysis
                                .providers()
                                .names()
                                .map(|name| name.to_string())
                                .collect::<Vec<_>>(),
                            analysis.declared_outputs().to_vec(),
                        )
                    })
                    .collect::<Vec<_>>()
            };

        let retained = test_runtime(workspace.path()).unwrap();
        let c0_build = build(&retained, None).unwrap();
        let c0 = configuration_for(&c0_build, "@@//:parent");
        assert_eq!(
            c0.root_string_setting().map(|value| value.as_str()),
            Some("default")
        );
        assert!(accepted_output_text(&c0_build).contains(&"PARENT_ANALYSIS"));
        assert!(accepted_output_text(&c0_build).contains(&"CONSUMER_ANALYSIS"));
        let c0_topology = topology_for(&c0_build);

        let c1_build = build(&retained, Some("command")).unwrap();
        let c1 = configuration_for(&c1_build, "@@//:parent");
        assert_eq!(
            c1.root_string_setting().map(|value| value.as_str()),
            Some("command")
        );
        assert_ne!(c0, c1);
        assert!(accepted_output_text(&c1_build).contains(&"PARENT_ANALYSIS"));
        assert!(!accepted_output_text(&c1_build).contains(&"CONSUMER_ANALYSIS"));
        assert_eq!(c0_topology, topology_for(&c1_build));
        assert_ne!(
            crate::runtime::configured_output_root(
                workspace.path(),
                c0.slug_configuration().unwrap()
            ),
            crate::runtime::configured_output_root(
                workspace.path(),
                c1.slug_configuration().unwrap()
            )
        );

        let restored_build = build(&retained, None).unwrap();
        let restored = configuration_for(&restored_build, "@@//:parent");
        assert_eq!(c0, restored);
        assert_eq!(c0_topology, topology_for(&restored_build));
        assert!(accepted_output_text(&restored_build).is_empty());

        let fresh = test_runtime(workspace.path()).unwrap();
        let one_shot_build = build(&fresh, None).unwrap();
        let one_shot = configuration_for(&one_shot_build, "@@//:parent");
        assert_eq!(c0, one_shot);
        assert_eq!(
            c0.slug_configuration().unwrap().projection(),
            one_shot.slug_configuration().unwrap().projection()
        );

        let transitive_runtime = test_runtime(workspace.path()).unwrap();
        let top = TargetPattern::parse("//:top").unwrap();
        let transitive_c0 = transitive_runtime
            .build_command_with_bzlmod_inputs(
                std::slice::from_ref(&top),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        let transitive_parent = configuration_for(&transitive_c0, "@@//:parent");
        let transitive_consumer = configuration_for(&transitive_c0, "@@//:consumer");
        assert_eq!(transitive_parent, c0);
        assert_eq!(
            transitive_consumer
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("left")
        );

        let transitive_c1 = transitive_runtime
            .build_command_with_bzlmod_inputs(
                std::slice::from_ref(&top),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                Some("command"),
            )
            .unwrap();
        assert!(accepted_output_text(&transitive_c1).contains(&"TOP_ANALYSIS"));
        assert!(accepted_output_text(&transitive_c1).contains(&"PARENT_ANALYSIS"));
        assert!(!accepted_output_text(&transitive_c1).contains(&"CONSUMER_ANALYSIS"));

        let setting_runtime = test_runtime(workspace.path()).unwrap();
        let setting_target = TargetPattern::parse("//:setting").unwrap();
        let setting_build = setting_runtime
            .build_command_with_bzlmod_inputs(
                std::slice::from_ref(&setting_target),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        assert_eq!(
            configuration_for(&setting_build, "@@//:setting")
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("default")
        );
    }

    #[test]
    fn cquery_drives_the_existing_root_analysis_once() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"driver\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n",
        )
        .unwrap();
        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(activation_audit.clone());
        let run = |target: &str| {
            runtime.cquery_command_with_bzlmod_inputs(
                target,
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
        };
        let assert_roots = |label: &str, expected_count| {
            let roots = activation_audit.take_configured_roots();
            assert_eq!(
                roots.len(),
                expected_count,
                "cquery analysis root activation count"
            );
            for root in roots {
                let serialized = root.stable_serialize();
                assert!(serialized.starts_with(&format!("{label} [target:slugcfg-v1:")));
                assert!(serialized.ends_with(']'));
            }
        };
        let target = "//pkg:probe";
        let cold = run(target).unwrap();
        let cold_evaluation = cold.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(cold_evaluation.starlark_label_stdout(), "@@//pkg:probe\n");
        assert!(
            cold_evaluation
                .analyses()
                .next()
                .unwrap()
                .actions()
                .is_empty()
        );
        assert!(accepted_output_text(&cold).is_empty());
        assert_roots("@@//pkg:probe", 1);

        let warm = run(target).unwrap();
        assert!(warm.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&warm).is_empty());
        assert_roots("@@//pkg:probe", 1);

        let missing = "//pkg:missing";
        let missing_result = run(missing).unwrap();
        let error = missing_result
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert_eq!(error.missing_stderr().unwrap().lines().count(), 3);
        assert!(accepted_output_text(&missing_result).is_empty());
        assert_roots("@@//pkg:missing", 0);

        let recovered = run(target).unwrap();
        assert_eq!(
            recovered
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .starlark_label_stdout(),
            "@@//pkg:probe\n"
        );
        assert!(accepted_output_text(&recovered).is_empty());
        assert_roots("@@//pkg:probe", 1);

        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n# cquery edit\n",
        )
        .unwrap();
        let build_edit = run(target).unwrap();
        assert!(build_edit.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&build_edit).is_empty());
        assert_roots("@@//pkg:probe", 1);

        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n# cquery edit\n",
        )
        .unwrap();
        let bzl_edit = run(target).unwrap();
        assert!(bzl_edit.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&bzl_edit).is_empty());
        assert_roots("@@//pkg:probe", 1);
    }

    #[test]
    fn cquery_analysis_error_retains_sidecars_from_successful_sibling_roots() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"driver\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _impl(ctx):\n    print(ctx.label.name)\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"ok\")\nfilegroup(name = \"native\")\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let accepted = runtime
            .cquery_command_with_bzlmod_inputs(
                "//pkg:ok + //pkg:native",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();

        assert!(
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("not a Starlark rule")
        );
        assert_eq!(accepted_output_text(&accepted), ["ok"]);
    }

    #[test]
    fn cquery_deps_uses_the_retained_noimplicit_graph_with_null_sources() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"deps\")\n",
        )
        .unwrap();
        fs::write(workspace.path().join("defs.bzl"), CQUERY_DELEGATING_DEFS).unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            CQUERY_DELEGATING_BUILD,
        )
        .unwrap();
        fs::write(workspace.path().join("source.txt"), "source\n").unwrap();

        let runtime = test_runtime(workspace.path()).unwrap();
        let run = |expression: &str, include_implicit: bool, include_tool: bool| {
            runtime.cquery_command_with_bzlmod_inputs(
                expression,
                include_implicit,
                include_tool,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
        };

        let default_error = run("deps(//:root)", true, true).unwrap_err();
        assert!(matches!(default_error, CqueryCommandError::Request(_)));
        assert!(default_error.to_string().contains("--noimplicit_deps"));

        let depth_zero = run("deps(//:root, 0)", false, true).unwrap();
        let depth_zero = depth_zero.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(depth_zero.starlark_label_stdout(), "@@//:root\n");
        assert!(
            depth_zero
                .label_kind_stdout()
                .unwrap()
                .starts_with("ordinary_rule rule //:root (slugcfg-v1:")
        );

        let full = run("deps(//:root)", false, true).unwrap();
        let full = full.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            full.starlark_label_stdout(),
            "@@//:root\n@@//:ordinary\n@@//:ordinary\n@@//:alias_outer\n@@//:source.txt\n@@//:producer.out\n@@//:vis_top\n@@//:alias_inner\n@@//:producer\n"
        );
        assert_eq!(
            full.label_stdout()
                .lines()
                .map(|line| line.split_once(" (").unwrap().0)
                .collect::<Vec<_>>(),
            [
                "//:root",
                "//:ordinary",
                "//:ordinary",
                "//:alias_outer",
                "//:source.txt",
                "//:producer.out",
                "//:vis_top",
                "//:alias_inner",
                "//:producer",
            ]
        );
        assert!(full.label_stdout().contains("//:source.txt (null)\n"));
        assert!(full.label_stdout().contains("//:vis_top (null)\n"));
        assert_eq!(
            full.label_kind_stdout()
                .unwrap()
                .lines()
                .map(|line| line.split_once(" (").unwrap().0)
                .collect::<Vec<_>>(),
            [
                "ordinary_rule rule //:root",
                "ordinary_rule rule //:ordinary",
                "ordinary_rule rule //:ordinary",
                "alias rule //:alias_outer",
                "source file //:source.txt",
                "generated file //:producer.out",
                "package group //:vis_top",
                "alias rule //:alias_inner",
                "producer rule //:producer",
            ]
        );
        assert!(
            full.label_kind_stdout()
                .unwrap()
                .contains("source file //:source.txt (null)\n")
        );
        assert!(
            full.label_kind_stdout()
                .unwrap()
                .contains("package group //:vis_top (null)\n")
        );
        assert!(!full.starlark_label_stdout().contains("vis_leaf"));
        let ordinary = full
            .analyses()
            .filter(|analysis| analysis.key().label().target().as_str() == "ordinary")
            .map(|analysis| analysis.key().clone())
            .collect::<Vec<_>>();
        assert_eq!(ordinary.len(), 2);
        assert_ne!(ordinary[0], ordinary[1]);

        let structural = run(
            "kind('^(source file|generated file|package group)$', deps(//:root))",
            false,
            true,
        )
        .unwrap();
        let structural = structural.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            structural.starlark_label_stdout(),
            "@@//:source.txt\n@@//:producer.out\n@@//:vis_top\n"
        );
        assert!(structural.label_stdout().contains("//:source.txt (null)\n"));
        assert!(structural.label_stdout().contains("//:vis_top (null)\n"));
        assert_eq!(
            structural
                .label_kind_stdout()
                .unwrap()
                .lines()
                .map(|line| line.split_once(" (").unwrap().0)
                .collect::<Vec<_>>(),
            [
                "source file //:source.txt",
                "generated file //:producer.out",
                "package group //:vis_top",
            ]
        );
        assert_eq!(
            structural
                .graph_stdout()
                .lines()
                .filter(|line| line.contains(" -> "))
                .count(),
            0
        );
        let structural_chain = run(
            "filter('^(//:source\\.txt|//:producer\\.out|//:vis_top)$', kind('^(source file|generated file|package group)$', deps(//:root)))",
            false,
            true,
        )
        .unwrap();
        let structural_chain = structural_chain
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap();
        assert_eq!(structural_chain.label_stdout(), structural.label_stdout());
        assert_eq!(
            structural_chain.starlark_label_stdout(),
            structural.starlark_label_stdout()
        );
        assert_eq!(
            structural_chain.label_kind_stdout().unwrap(),
            structural.label_kind_stdout().unwrap()
        );
        assert_eq!(structural_chain.graph_stdout(), structural.graph_stdout());

        let duplicates = run(
            "filter('^//:ordinary$', kind('^ordinary_rule rule$', deps(//:root)))",
            false,
            true,
        )
        .unwrap();
        let duplicates = duplicates.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            duplicates.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n"
        );
        assert_eq!(duplicates.analyses().count(), 2);
        assert_eq!(
            duplicates
                .graph_stdout()
                .lines()
                .filter(|line| line.contains(" -> "))
                .count(),
            0
        );

        let topology = |evaluation: &CqueryCommandEvaluation| {
            let mut graph = evaluation.graph_stdout();
            for analysis in evaluation.analyses() {
                let Some(configuration) = analysis
                    .configured_target_key()
                    .and_then(|key| key.configuration().slug_configuration())
                else {
                    continue;
                };
                let name = if configuration
                    .root_string_setting()
                    .is_some_and(|setting| setting.as_str() == "transitioned")
                {
                    "transition"
                } else {
                    "base"
                };
                graph = graph.replace(&configuration.projection().to_string(), name);
            }
            let mut nodes = graph
                .lines()
                .filter(|line| line.starts_with("  \"") && !line.contains(" -> "))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            nodes.sort_unstable();
            let mut edges = graph
                .lines()
                .filter(|line| line.contains(" -> "))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            edges.sort_unstable();
            (nodes, edges)
        };
        let assert_topology = |evaluation: &CqueryCommandEvaluation,
                               expected_nodes: &[&str],
                               expected_edges: &[&str]| {
            let (nodes, edges) = topology(evaluation);
            assert_eq!(
                nodes,
                expected_nodes
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                edges,
                expected_edges
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect::<Vec<_>>()
            );
        };
        assert_topology(depth_zero, &["  \"//:root (base)\""], &[]);

        let reverse = run("rdeps(deps(//:root), //:ordinary)", false, true).unwrap();
        let reverse = reverse.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            reverse.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n@@//:root\n@@//:alias_inner\n@@//:alias_outer\n"
        );
        assert_eq!(reverse.analyses().count(), 5);
        assert_eq!(reverse.label_stdout().lines().count(), 5);
        assert_eq!(reverse.label_kind_stdout().unwrap().lines().count(), 5);
        assert_topology(
            reverse,
            &[
                "  \"//:alias_inner (base)\"",
                "  \"//:alias_outer (base)\"",
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
                "  \"//:root (base)\"",
            ],
            &[
                "  \"//:alias_inner (base)\" -> \"//:ordinary (base)\"",
                "  \"//:alias_outer (base)\" -> \"//:alias_inner (base)\"",
                "  \"//:root (base)\" -> \"//:alias_outer (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (transition)\"",
            ],
        );
        let reverse_zero = run("rdeps(deps(//:root), //:ordinary, 0)", false, true).unwrap();
        let reverse_zero = reverse_zero.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            reverse_zero.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n"
        );
        assert_topology(
            reverse_zero,
            &["  \"//:ordinary (base)\"", "  \"//:ordinary (transition)\""],
            &[],
        );
        let reverse_one = run("rdeps(deps(//:root), //:ordinary, 1)", false, true).unwrap();
        let reverse_one = reverse_one.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            reverse_one.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n@@//:root\n@@//:alias_inner\n"
        );
        assert_topology(
            reverse_one,
            &[
                "  \"//:alias_inner (base)\"",
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
                "  \"//:root (base)\"",
            ],
            &[
                "  \"//:alias_inner (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (transition)\"",
            ],
        );
        for depth in ["2", "2147483647"] {
            let bounded = run(
                &format!("rdeps(deps(//:root), //:ordinary, {depth})"),
                false,
                true,
            )
            .unwrap();
            assert_eq!(
                bounded
                    .terminal_for_test()
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .graph_stdout(),
                reverse.graph_stdout(),
                "depth {depth}"
            );
        }
        let negative = run(
            "rdeps(deps(//:root), //:ordinary, '-2147483648')",
            false,
            true,
        )
        .unwrap();
        let negative = negative.terminal_for_test().as_ref().as_ref().unwrap();
        assert!(negative.label_stdout().is_empty());
        assert!(negative.starlark_label_stdout().is_empty());
        assert!(negative.label_kind_stdout().unwrap().is_empty());
        assert_eq!(
            negative.graph_stdout(),
            "digraph mygraph {\n  node [shape=box];\n}\n"
        );
        for depth in ["", ", '-2147483648'", ", 0", ", 1", ", 2147483647"] {
            let direct = run(&format!("rdeps(//:root, //:ordinary{depth})"), false, true).unwrap();
            let direct = direct.terminal_for_test().as_ref().as_ref().unwrap();
            let normalized = run(
                &format!("rdeps(deps(//:root), //:ordinary{depth})"),
                false,
                true,
            )
            .unwrap();
            let normalized = normalized.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(direct.label_stdout(), normalized.label_stdout(), "{depth}");
            assert_eq!(
                direct.label_kind_stdout().unwrap(),
                normalized.label_kind_stdout().unwrap(),
                "{depth}"
            );
            assert_eq!(
                direct.starlark_label_stdout(),
                normalized.starlark_label_stdout(),
                "{depth}"
            );
            assert_eq!(direct.graph_stdout(), normalized.graph_stdout(), "{depth}");
            assert_eq!(
                direct
                    .analyses()
                    .map(|analysis| analysis.key().clone())
                    .collect::<Vec<_>>(),
                normalized
                    .analyses()
                    .map(|analysis| analysis.key().clone())
                    .collect::<Vec<_>>(),
                "{depth} configured keys"
            );
        }
        for (depth, expected) in [
            ("", reverse),
            (", '-2147483648'", negative),
            (", 0", reverse_zero),
            (", 1", reverse_one),
            (", 2147483647", reverse),
        ] {
            let filtered = run(
                &format!("filter('.*', rdeps(//:root, //:ordinary{depth}))"),
                false,
                true,
            )
            .unwrap();
            let filtered = filtered.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(filtered.label_stdout(), expected.label_stdout(), "{depth}");
            assert_eq!(
                filtered.label_kind_stdout().unwrap(),
                expected.label_kind_stdout().unwrap(),
                "{depth}"
            );
            assert_eq!(
                filtered.starlark_label_stdout(),
                expected.starlark_label_stdout(),
                "{depth}"
            );
            assert_eq!(filtered.graph_stdout(), expected.graph_stdout(), "{depth}");
            assert_eq!(
                filtered
                    .analyses()
                    .map(|analysis| analysis.key().clone())
                    .collect::<Vec<_>>(),
                expected
                    .analyses()
                    .map(|analysis| analysis.key().clone())
                    .collect::<Vec<_>>(),
                "{depth} configured keys"
            );
        }
        let selected = run(
            "filter('ordinary|alias_', rdeps(//:root, //:ordinary))",
            false,
            true,
        )
        .unwrap();
        let selected = selected.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(selected.analyses().count(), 4);
        assert_topology(
            selected,
            &[
                "  \"//:alias_inner (base)\"",
                "  \"//:alias_outer (base)\"",
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
            ],
            &[
                "  \"//:alias_inner (base)\" -> \"//:ordinary (base)\"",
                "  \"//:alias_outer (base)\" -> \"//:alias_inner (base)\"",
            ],
        );
        let filtered_empty = run(
            "filter('never-matches', rdeps(//:root, //:ordinary))",
            false,
            true,
        )
        .unwrap();
        let filtered_empty = filtered_empty
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap();
        assert!(filtered_empty.label_stdout().is_empty());
        assert_eq!(
            filtered_empty.graph_stdout(),
            "digraph mygraph {\n  node [shape=box];\n}\n"
        );
        let kind_zero = run(
            "kind('^ordinary_rule rule$', rdeps(//:root, //:ordinary, 0))",
            false,
            true,
        )
        .unwrap();
        let kind_zero = kind_zero.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            kind_zero.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n"
        );
        let zero_keys = kind_zero
            .analyses()
            .map(|analysis| analysis.key().clone())
            .collect::<Vec<_>>();
        assert_eq!(zero_keys.len(), 2);
        assert_ne!(zero_keys[0], zero_keys[1]);
        let kind_full = run(
            "kind('^ordinary_rule rule$', rdeps(//:root, //:ordinary))",
            false,
            true,
        )
        .unwrap();
        let kind_full = kind_full.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            kind_full.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n@@//:root\n"
        );
        assert_topology(
            kind_full,
            &[
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
                "  \"//:root (base)\"",
            ],
            &[
                "  \"//:root (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (transition)\"",
            ],
        );
        let kind_zero_graph = kind_zero.graph_stdout();
        let kind_full_graph = kind_full.graph_stdout();
        for (depth, expected) in [
            ("'-1'", "digraph mygraph {\n  node [shape=box];\n}\n"),
            ("0", kind_zero_graph.as_str()),
            ("1", kind_full_graph.as_str()),
            ("2147483647", kind_full_graph.as_str()),
        ] {
            let bounded = run(
                &format!("kind('^ordinary_rule rule$', rdeps(//:root, //:ordinary, {depth}))"),
                false,
                true,
            )
            .unwrap();
            let bounded = bounded.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(bounded.graph_stdout(), expected, "depth {depth}");
        }
        let aliases = run(
            "kind('^alias rule$', rdeps(//:root, //:ordinary))",
            false,
            true,
        )
        .unwrap();
        let aliases = aliases.terminal_for_test().as_ref().as_ref().unwrap();
        assert_topology(
            aliases,
            &["  \"//:alias_inner (base)\"", "  \"//:alias_outer (base)\""],
            &["  \"//:alias_outer (base)\" -> \"//:alias_inner (base)\""],
        );
        let kind_empty = run(
            "kind('^producer rule$', rdeps(//:root, //:ordinary))",
            false,
            true,
        )
        .unwrap();
        assert!(
            kind_empty
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_stdout()
                .is_empty()
        );
        let reverse_keys = reverse
            .analyses()
            .map(|analysis| analysis.key().clone())
            .collect::<Vec<_>>();
        for inner in ["0", "1", "2", "2147483647"] {
            let normalized = run(
                &format!("rdeps(deps(//:root, {inner}), //:ordinary)"),
                false,
                true,
            )
            .unwrap();
            let normalized = normalized.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                normalized.graph_stdout(),
                reverse.graph_stdout(),
                "inner {inner} graph"
            );
            assert_eq!(
                normalized
                    .analyses()
                    .map(|analysis| analysis.key().clone())
                    .collect::<Vec<_>>(),
                reverse_keys,
                "inner {inner} configured keys"
            );
        }
        let composed = run("rdeps(deps(//:root, 0), //:ordinary, 0)", false, true).unwrap();
        assert_eq!(
            composed
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .graph_stdout(),
            reverse_zero.graph_stdout()
        );
        let max_negative = run(
            "rdeps(deps(//:root, 2147483647), //:ordinary, '-1')",
            false,
            true,
        )
        .unwrap();
        assert!(
            max_negative
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_stdout()
                .is_empty()
        );
        let broken = run("//:broken", false, true).unwrap();
        assert!(
            broken
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("seed must not be analyzed")
        );
        let unreachable = run("rdeps(//:root, //:broken)", false, true).unwrap();
        let unreachable = unreachable.terminal_for_test().as_ref().as_ref().unwrap();
        assert!(unreachable.label_stdout().is_empty());
        assert_eq!(unreachable.analyses().count(), 0);
        let missing = run("rdeps(//:root, //:missing)", false, true).unwrap();
        assert!(matches!(
            missing.terminal_for_test().as_ref().as_ref().unwrap_err(),
            CqueryCommandError::MissingTarget { requested, .. } if requested.as_ref() == "//:missing"
        ));
        for expression in [
            "filter('(', rdeps(//:universe_missing, //:ordinary))",
            "filter('(', rdeps(//:root, //:missing))",
            "kind('(', rdeps(//:universe_missing, //:ordinary))",
            "kind('(', rdeps(//:root, //:missing))",
        ] {
            let invalid = run(expression, false, true).unwrap();
            assert!(matches!(
                invalid.terminal_for_test().as_ref().as_ref().unwrap_err(),
                CqueryCommandError::Request(message) if message.contains("invalid Slug regex")
            ));
        }
        let bad_universe = run("rdeps(//:universe_missing, //pending:seed)", false, true).unwrap();
        assert!(matches!(
            bad_universe.terminal_for_test().as_ref().as_ref().unwrap_err(),
            CqueryCommandError::MissingTarget { requested, .. }
                if requested.as_ref() == "//:universe_missing"
        ));
        let default_seed = run("//:transition_only", false, true).unwrap();
        assert!(
            default_seed
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("default seed must not be analyzed")
        );
        let transitioned = run(
            "rdeps(//:transition_root, //:transition_only, 0)",
            false,
            true,
        )
        .unwrap();
        let transitioned = transitioned.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            transitioned.starlark_label_stdout(),
            "@@//:transition_only\n"
        );
        let expected_reverse_graph = reverse.graph_stdout();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            CQUERY_DELEGATING_BUILD
                .replace("aliased = \":alias_outer\"", "aliased = \":ordinary\""),
        )
        .unwrap();
        let bypass = run("rdeps(deps(//:root), //:ordinary)", false, true).unwrap();
        let bypass = bypass.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            bypass.starlark_label_stdout(),
            "@@//:ordinary\n@@//:ordinary\n@@//:root\n"
        );
        fs::write(
            workspace.path().join("BUILD.bazel"),
            CQUERY_DELEGATING_BUILD,
        )
        .unwrap();
        let restored = run("rdeps(deps(//:root), //:ordinary)", false, true).unwrap();
        assert_eq!(
            restored
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .graph_stdout(),
            expected_reverse_graph
        );

        let depth_one = run("deps(//:root, 1)", false, true).unwrap();
        let depth_one = depth_one.terminal_for_test().as_ref().as_ref().unwrap();
        assert_topology(
            depth_one,
            &[
                "  \"//:alias_outer (base)\"",
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
                "  \"//:producer.out (base)\"",
                "  \"//:root (base)\"",
                "  \"//:source.txt (null)\"",
                "  \"//:vis_top (null)\"",
            ],
            &[
                "  \"//:root (base)\" -> \"//:alias_outer (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (transition)\"",
                "  \"//:root (base)\" -> \"//:producer.out (base)\"",
                "  \"//:root (base)\" -> \"//:source.txt (null)\"",
                "  \"//:root (base)\" -> \"//:vis_top (null)\"",
            ],
        );

        let depth_two = run("deps(//:root, 2)", false, true).unwrap();
        let depth_two = depth_two.terminal_for_test().as_ref().as_ref().unwrap();
        assert_topology(
            depth_two,
            &[
                "  \"//:alias_inner (base)\"",
                "  \"//:alias_outer (base)\"",
                "  \"//:ordinary (base)\"",
                "  \"//:ordinary (transition)\"",
                "  \"//:producer (base)\"",
                "  \"//:producer.out (base)\"",
                "  \"//:root (base)\"",
                "  \"//:source.txt (null)\"",
                "  \"//:vis_top (null)\"",
            ],
            &[
                "  \"//:alias_inner (base)\" -> \"//:ordinary (base)\"",
                "  \"//:alias_outer (base)\" -> \"//:alias_inner (base)\"",
                "  \"//:producer.out (base)\" -> \"//:producer (base)\"",
                "  \"//:root (base)\" -> \"//:alias_outer (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (base)\"",
                "  \"//:root (base)\" -> \"//:ordinary (transition)\"",
                "  \"//:root (base)\" -> \"//:producer.out (base)\"",
                "  \"//:root (base)\" -> \"//:source.txt (null)\"",
                "  \"//:root (base)\" -> \"//:vis_top (null)\"",
            ],
        );
        assert_eq!(topology(depth_two), topology(full));

        let depth_max = run("deps(//:root, 2147483647)", false, true).unwrap();
        let depth_max = depth_max.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(topology(depth_max), topology(full));
        assert_eq!(
            depth_max.label_kind_stdout().unwrap(),
            full.label_kind_stdout().unwrap()
        );

        let without_tools = run("deps(//:root)", false, false).unwrap();
        let without_tools = without_tools.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            without_tools.starlark_label_stdout(),
            full.starlark_label_stdout()
        );
        assert_eq!(without_tools.graph_stdout(), full.graph_stdout());
    }

    #[test]
    fn cquery_executables_deps_filters_complete_closure_and_induces_edges() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"executable_deps\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            format!(
                r##"{CQUERY_DELEGATING_DEFS}
def _executable(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/sh\n")
    return [DefaultInfo(executable = out)]
executable_rule = rule(implementation = _executable, executable = True, attrs = {{
    "normal": attr.label(),
    "transitioned": attr.label(cfg = to_transition),
    "bridge": attr.label(),
}})
"##
            ),
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            r#"load(":defs.bzl", "executable_rule", "ordinary_rule", "string_setting")
string_setting(name = "setting", build_setting_default = "default")
executable_rule(name = "direct")
executable_rule(name = "leaf")
ordinary_rule(name = "bridge", normal = ":leaf")
executable_rule(
    name = "root",
    normal = ":direct",
    transitioned = ":direct",
    bridge = ":bridge",
)
"#,
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let run = |expression: &str| {
            runtime
                .cquery_command_with_bzlmod_inputs(
                    expression,
                    false,
                    true,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap()
        };

        let depth_zero = run("executables(deps(//:root, 0))");
        let depth_zero = depth_zero.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(depth_zero.starlark_label_stdout(), "@@//:root\n");

        let depth_one = run("executables(deps(//:root, 1))");
        let depth_one = depth_one.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            depth_one.starlark_label_stdout(),
            "@@//:root\n@@//:direct\n@@//:direct\n"
        );

        let full = run("executables(deps(//:root))");
        let full = full.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            full.starlark_label_stdout(),
            "@@//:root\n@@//:direct\n@@//:direct\n@@//:leaf\n"
        );
        assert_eq!(full.analyses().count(), 4);
        assert_eq!(full.label_kind_stdout().unwrap().lines().count(), 4);
        assert!(
            full.label_kind_stdout()
                .unwrap()
                .lines()
                .all(|line| line.starts_with("executable_rule rule "))
        );
        let graph = full.graph_stdout();
        let nodes = graph
            .lines()
            .filter(|line| line.starts_with("  \"") && !line.contains(" -> "))
            .count();
        let edges = graph
            .lines()
            .filter(|line| line.contains(" -> "))
            .collect::<Vec<_>>();
        assert_eq!(nodes, 4);
        assert_eq!(edges.len(), 2);
        assert!(
            edges
                .iter()
                .all(|edge| edge.contains("//:root") && edge.contains("//:direct"))
        );
        assert!(edges.iter().all(|edge| !edge.contains("//:leaf")));

        let reverse_self = run("executables(rdeps(//:root, //:root))");
        let reverse_self = reverse_self.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(reverse_self.starlark_label_stdout(), "@@//:root\n");

        let reverse_zero = run("executables(rdeps(//:root, //:direct, 0))");
        let reverse_zero = reverse_zero.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            reverse_zero.starlark_label_stdout(),
            "@@//:direct\n@@//:direct\n"
        );
        let reverse_full = run("executables(rdeps(//:root, //:direct))");
        let reverse_full = reverse_full.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(
            reverse_full.starlark_label_stdout(),
            "@@//:direct\n@@//:direct\n@@//:root\n"
        );
        let reverse_keys = reverse_full
            .analyses()
            .map(|analysis| analysis.key().clone())
            .collect::<Vec<_>>();
        assert_eq!(reverse_keys.len(), 3);
        assert_ne!(reverse_keys[0], reverse_keys[1]);
        let reverse_graph = reverse_full.graph_stdout();
        assert_eq!(
            reverse_graph
                .lines()
                .filter(|line| line.contains(" -> "))
                .count(),
            2
        );
        assert!(
            reverse_graph
                .lines()
                .filter(|line| line.contains(" -> "))
                .all(|line| line.contains("//:root") && line.contains("//:direct"))
        );
        for (depth, expected) in [
            (
                "'-1'",
                "digraph mygraph {\n  node [shape=box];\n}\n".to_owned(),
            ),
            ("0", reverse_zero.graph_stdout()),
            ("1", reverse_full.graph_stdout()),
            ("2147483647", reverse_full.graph_stdout()),
        ] {
            let bounded = run(&format!("executables(rdeps(//:root, //:direct, {depth}))"));
            let bounded = bounded.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(bounded.graph_stdout(), expected, "depth {depth}");
        }
        let reverse_empty = run("executables(rdeps(//:root, //:bridge, 0))");
        let reverse_empty = reverse_empty.terminal_for_test().as_ref().as_ref().unwrap();
        assert!(reverse_empty.label_stdout().is_empty());
        assert_eq!(
            reverse_empty.graph_stdout(),
            "digraph mygraph {\n  node [shape=box];\n}\n"
        );

        let chained_full = run("filter(':(root|direct|leaf)$', executables(deps(//:root)))");
        let chained_full = chained_full.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(chained_full.label_stdout(), full.label_stdout());
        assert_eq!(
            chained_full.starlark_label_stdout(),
            full.starlark_label_stdout()
        );
        assert_eq!(
            chained_full.label_kind_stdout().unwrap(),
            full.label_kind_stdout().unwrap()
        );
        assert_eq!(chained_full.graph_stdout(), full.graph_stdout());

        let depth_two = run("executables(deps(//:root, 2))");
        let depth_max = run("executables(deps(//:root, 2147483647))");
        assert_eq!(
            depth_two
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .graph_stdout(),
            full.graph_stdout()
        );
        assert_eq!(
            depth_max
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_stdout(),
            full.label_stdout()
        );

        let filtered = run("filter(':(root|direct|leaf)$', deps(//:root))");
        let filtered = filtered.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(filtered.label_stdout(), full.label_stdout());
        assert_eq!(
            filtered.starlark_label_stdout(),
            full.starlark_label_stdout()
        );
        assert_eq!(
            filtered.label_kind_stdout().unwrap(),
            full.label_kind_stdout().unwrap()
        );
        assert_eq!(filtered.graph_stdout(), full.graph_stdout());

        let kind = run("kind('^executable_rule rule$', deps(//:root))");
        let kind = kind.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(kind.label_stdout(), full.label_stdout());
        assert_eq!(kind.starlark_label_stdout(), full.starlark_label_stdout());
        assert_eq!(
            kind.label_kind_stdout().unwrap(),
            full.label_kind_stdout().unwrap()
        );
        assert_eq!(kind.graph_stdout(), full.graph_stdout());

        let named_kind_full =
            run("filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root)))");
        let named_kind_full = named_kind_full
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap();
        assert_eq!(named_kind_full.label_stdout(), full.label_stdout());
        assert_eq!(
            named_kind_full.starlark_label_stdout(),
            full.starlark_label_stdout()
        );
        assert_eq!(
            named_kind_full.label_kind_stdout().unwrap(),
            full.label_kind_stdout().unwrap()
        );
        assert_eq!(named_kind_full.graph_stdout(), full.graph_stdout());

        for (depth, expected) in [(0, depth_zero), (1, depth_one)] {
            let filtered = run(&format!(
                "filter(':(root|direct|leaf)$', deps(//:root, {depth}))"
            ));
            let filtered = filtered.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                filtered.graph_stdout(),
                expected.graph_stdout(),
                "depth {depth}"
            );
        }
        for depth in [2, i32::MAX] {
            let filtered = run(&format!(
                "filter(':(root|direct|leaf)$', deps(//:root, {depth}))"
            ));
            let filtered = filtered.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                filtered.graph_stdout(),
                full.graph_stdout(),
                "depth {depth}"
            );
        }
        for (depth, expected) in [(0, depth_zero), (1, depth_one)] {
            let kind = run(&format!(
                "kind('^executable_rule rule$', deps(//:root, {depth}))"
            ));
            let kind = kind.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                kind.graph_stdout(),
                expected.graph_stdout(),
                "depth {depth}"
            );
        }
        for depth in [2, i32::MAX] {
            let kind = run(&format!(
                "kind('^executable_rule rule$', deps(//:root, {depth}))"
            ));
            let kind = kind.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(kind.graph_stdout(), full.graph_stdout(), "depth {depth}");
        }
        for (depth, expected) in [(0, depth_zero), (1, depth_one)] {
            let chained = run(&format!(
                "filter(':(root|direct|leaf)$', executables(deps(//:root, {depth})))"
            ));
            let chained = chained.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                chained.graph_stdout(),
                expected.graph_stdout(),
                "depth {depth}"
            );
        }
        for depth in [2, i32::MAX] {
            let chained = run(&format!(
                "filter(':(root|direct|leaf)$', executables(deps(//:root, {depth})))"
            ));
            let chained = chained.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(chained.graph_stdout(), full.graph_stdout(), "depth {depth}");
        }
        for (depth, expected) in [(0, depth_zero), (1, depth_one), (2, full), (i32::MAX, full)] {
            let named_kind = run(&format!(
                "filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root, {depth})))"
            ));
            let named_kind = named_kind.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(
                named_kind.graph_stdout(),
                expected.graph_stdout(),
                "depth {depth}"
            );
        }
        let empty = run("filter('^//:missing$', deps(//:root))");
        let empty = empty.terminal_for_test().as_ref().as_ref().unwrap();
        assert!(empty.label_stdout().is_empty());
        assert_eq!(
            empty.graph_stdout(),
            "digraph mygraph {\n  node [shape=box];\n}\n"
        );
        let chained_empty = run("filter('^//:missing$', executables(deps(//:root)))");
        let chained_empty = chained_empty.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(chained_empty.label_stdout(), empty.label_stdout());
        assert_eq!(chained_empty.graph_stdout(), empty.graph_stdout());
        let named_kind_empty =
            run("filter('^//:missing$', kind('^executable_rule rule$', deps(//:root)))");
        let named_kind_empty = named_kind_empty
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap();
        assert_eq!(named_kind_empty.label_stdout(), empty.label_stdout());
        assert_eq!(named_kind_empty.graph_stdout(), empty.graph_stdout());
    }

    #[tokio::test]
    async fn cquery_deps_frontier_need_precedes_an_earlier_child_analysis_error() {
        let expression =
            QueryExpression::parse("filter('root$', kind('rule$', deps(//:root, 1)))").unwrap();
        let deps = expression
            .cquery_preactivation_deps_spec()
            .expect("chained closure spec");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let configuration = build_test_configuration("target");
        let configured = |label: &str| {
            ConfiguredTargetKey::new(CanonicalLabel::parse(label).unwrap(), configuration.clone())
        };
        let mut transaction =
            build_root_transaction(&dice, delegating_action_closure_epoch(1)).await;
        let root_key = ConfiguredNodeAnalysisKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            configured("@@//:root"),
        )
        .unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(root) =
            transaction.compute(&root_key).await.unwrap()
        else {
            panic!("observed root fixture returned Need")
        };
        let root = Arc::new(
            root.as_ref()
                .as_ref()
                .unwrap()
                .as_ref()
                .clone()
                .with_edges(vec![
                    slug_analysis_v2::ConfiguredEdge::new(
                        configured("@@//:missing").into(),
                        slug_analysis_v2::ConfiguredEdgeKind::OrdinaryAttribute {
                            attribute: "error".into(),
                            index: 0,
                        },
                    ),
                    slug_analysis_v2::ConfiguredEdge::new(
                        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap()),
                        slug_analysis_v2::ConfiguredEdgeKind::Source,
                    ),
                ]),
        );
        let mut missing_source_epoch = BuildRootEpoch::base(2);
        missing_source_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 2);
        missing_source_epoch.package("", DELEGATING_BUILD, 2);
        let mut transaction = build_root_transaction(&dice, missing_source_epoch.build()).await;
        let outcome = compute_cquery_deps_closure(
            &mut transaction,
            &NormalizedAbsolutePath::new("/workspace").unwrap(),
            root.dupe(),
            deps.depth(),
            true,
        )
        .await
        .unwrap();
        assert!(
            matches!(outcome, slug_bzlmod_v2::SourcePreparationOutcome::Need(_)),
            "{outcome:?}"
        );
        let mut restored_epoch = BuildRootEpoch::base(3);
        restored_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 3);
        restored_epoch.package(
            "",
            &format!("{DELEGATING_BUILD}\nroot_rule(name = \"missing\")\n"),
            3,
        );
        restored_epoch.file("/workspace/source.txt", "source\n", 3);
        let mut restored = build_root_transaction(&dice, restored_epoch.build()).await;
        let restored = compute_cquery_deps_closure(
            &mut restored,
            &NormalizedAbsolutePath::new("/workspace").unwrap(),
            root,
            deps.depth(),
            true,
        )
        .await
        .unwrap();
        assert!(matches!(
            restored,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(_))
        ));
    }

    #[tokio::test]
    async fn cquery_rdeps_universe_need_precedes_seed_validation() {
        let expression = QueryExpression::parse("rdeps(//:root, //:missing)").unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = CqueryCommandRoot {
            expression,
            roots: Arc::from([CqueryRootTarget {
                requested: Arc::from("//:root"),
                canonical: CanonicalLabel::parse("@@//:root").unwrap(),
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                base_configuration: build_test_configuration("target"),
                explicit_root_string_setting: None,
            }]),
            literal_roots: Arc::from([(Arc::from("//:root"), 0)]),
            include_implicit: false,
            include_tool: true,
        };
        let mut missing_source_epoch = BuildRootEpoch::base(2);
        missing_source_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 2);
        missing_source_epoch.package("", DELEGATING_BUILD, 2);
        let mut transaction = build_root_transaction(&dice, missing_source_epoch.build()).await;
        let invalid_root = CqueryCommandRoot {
            expression: QueryExpression::parse("filter('(', rdeps(//:root, //:missing))").unwrap(),
            roots: root.roots.clone(),
            literal_roots: root.literal_roots.clone(),
            include_implicit: root.include_implicit,
            include_tool: root.include_tool,
        };
        let invalid = invalid_root.compute(&mut transaction).await.unwrap();
        assert!(matches!(
            invalid,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(CqueryCommandError::Request(message)) if message.contains("invalid Slug regex"))
        ));
        let outcome = root.compute(&mut transaction).await.unwrap();
        assert!(
            matches!(outcome, slug_bzlmod_v2::SourcePreparationOutcome::Need(_)),
            "{outcome:?}"
        );
    }

    #[test]
    fn cquery_evaluates_ordered_function_free_set_expressions_over_shared_roots() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"sets\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"bin\")\nprobe(name = \"lib\")\n",
        )
        .unwrap();
        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(activation_audit.clone());
        let empty = |expression: &str| {
            runtime
                .cquery_command_with_bzlmod_inputs(
                    expression,
                    true,
                    true,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap()
        };
        for expression in ["set()", "let x = set() in $x"] {
            let accepted = empty(expression);
            let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
            assert!(evaluation.label_stdout().is_empty());
            assert_eq!(evaluation.analyses().count(), 0);
            assert!(evaluation.starlark_label_stdout().is_empty());
            assert!(activation_audit.take_configured_roots().is_empty());
        }
        let invalid_count = runtime
            .cquery_command_with_bzlmod_inputs(
                "some(//pkg:missing, 2147483648)",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap_err();
        assert_eq!(invalid_count.exit_code(), 2);
        assert!(
            invalid_count
                .to_string()
                .contains("expected an integer literal: '2147483648'")
        );
        assert!(activation_audit.take_configured_roots().is_empty());
        let run = |expression: &str| {
            runtime
                .cquery_command_with_bzlmod_inputs(
                    expression,
                    true,
                    true,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .label_stdout()
        };
        let labels = |output: String| {
            output
                .lines()
                .map(|line| line.split_once(" (").unwrap().0.to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(run("//pkg:bin union //pkg:lib")),
            ["//pkg:bin", "//pkg:lib"]
        );
        assert_eq!(
            labels(run("set(//pkg:bin //pkg:lib //pkg:bin)")),
            ["//pkg:bin", "//pkg:lib"]
        );
        assert_eq!(
            labels(run("let x = //pkg:bin in $x union //pkg:lib")),
            ["//pkg:bin", "//pkg:lib"]
        );
        assert!(run("//pkg:bin intersect //pkg:lib").is_empty());
        assert_eq!(labels(run("//pkg:bin except //pkg:lib")), ["//pkg:bin"]);
        assert_eq!(
            labels(run(
                "filter('^//pkg:bin$', set(//pkg:lib //pkg:bin //pkg:lib))"
            )),
            ["//pkg:bin"]
        );
        assert_eq!(
            labels(run("filter('^//pkg:', set(//pkg:lib //pkg:bin //pkg:lib))")),
            ["//pkg:lib", "//pkg:bin"]
        );
        assert!(run("filter('^//missing:', set(//pkg:lib //pkg:bin))").is_empty());
        assert_eq!(labels(run("some(set(//pkg:lib //pkg:bin))")), ["//pkg:lib"]);
        assert_eq!(
            labels(run("some(set(//pkg:lib //pkg:bin //pkg:lib), 2)")),
            ["//pkg:lib", "//pkg:bin"]
        );
        assert_eq!(
            labels(run(
                "some(filter('^//pkg:bin$', set(//pkg:lib //pkg:bin)), 10)"
            )),
            ["//pkg:bin"]
        );
        for expression in ["some(set())", "some(//pkg:bin, 0)", "some(//pkg:bin, '-1')"] {
            let accepted = runtime
                .cquery_command_with_bzlmod_inputs(
                    expression,
                    true,
                    true,
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap();
            let error = accepted.terminal_for_test().as_ref().as_ref().unwrap_err();
            assert!(
                error.to_string().contains("argument set is empty"),
                "{expression}"
            );
        }
        let starlark = runtime
            .cquery_command_with_bzlmod_inputs(
                "let x = set(//pkg:bin //pkg:lib //pkg:bin) in ($x except //pkg:lib) union //pkg:lib",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        assert_eq!(
            starlark
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .starlark_label_stdout(),
            "@@//pkg:bin\n@@//pkg:lib\n"
        );

        let missing = runtime
            .cquery_command_with_bzlmod_inputs(
                "//pkg:missing union //pkg:also_missing",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(error.missing_stderr().unwrap().contains("//pkg:missing"));

        let missing_before_malformed = runtime
            .cquery_command_with_bzlmod_inputs(
                "filter('(', //pkg:missing)",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        let error = missing_before_malformed
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert!(error.missing_stderr().unwrap().contains("//pkg:missing"));

        let malformed = runtime
            .cquery_command_with_bzlmod_inputs(
                "filter('(', //pkg:bin)",
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        let error = malformed.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(error.to_string().contains("invalid Slug regex"));
    }

    #[test]
    fn cquery_restores_structural_configuration_and_display_projection() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"cquery_configuration\")\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            r#"SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx):
    print("CONSUMER_ANALYSIS")
    return [DefaultInfo(files = depset([]))]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
def _left(settings, attr):
    return {"//:setting": "left"}
left = transition(implementation = _left, inputs = [], outputs = ["//:setting"])
def _parent(ctx):
    return [DefaultInfo(files = depset([]))]
parent = rule(implementation = _parent, attrs = {"child": attr.label(cfg = left)})
"#,
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", child = \":consumer\")\n",
        )
        .unwrap();

        let target = "//:consumer";
        let run = |runtime: &WorkspaceRuntime, target: &str, setting: Option<&str>| {
            runtime.cquery_command_with_bzlmod_inputs(
                target,
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                setting,
            )
        };
        let evaluation = |accepted: &AcceptedCommand<
            Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>,
        >| {
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .clone()
        };
        let topology = |evaluation: &CqueryCommandEvaluation| {
            let analysis = evaluation.analyses().next().unwrap();
            (
                analysis.key().label().to_string(),
                analysis
                    .providers()
                    .names()
                    .map(|name| name.to_string())
                    .collect::<Vec<_>>(),
                analysis.declared_outputs().to_vec(),
                analysis.actions().len(),
                analysis
                    .configured_dependencies()
                    .map(|dependency| dependency.label().to_string())
                    .collect::<Vec<_>>(),
            )
        };

        let retained = test_runtime(workspace.path()).unwrap();
        let c0_command = run(&retained, target, None).unwrap();
        let c0 = evaluation(&c0_command);
        let c0_analysis = c0.analyses().next().unwrap();
        assert_eq!(
            c0_analysis
                .configured_target_key()
                .expect("current cquery analysis only contains configured nodes")
                .configuration()
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("default")
        );
        assert!(c0.label_stdout().starts_with("//:consumer (slugcfg-v1:"));
        assert_eq!(
            c0.label_stdout().len(),
            "//:consumer (slugcfg-v1:)\n".len() + 64
        );
        assert_eq!(c0.starlark_label_stdout(), "@@//:consumer\n");
        assert_eq!(accepted_output_text(&c0_command), ["CONSUMER_ANALYSIS"]);
        let c0_stdout = c0.label_stdout();
        let c0_topology = topology(&c0);

        let c1_command = run(&retained, target, Some("command")).unwrap();
        let c1 = evaluation(&c1_command);
        let c1_analysis = c1.analyses().next().unwrap();
        assert_eq!(
            c1_analysis
                .configured_target_key()
                .expect("current cquery analysis only contains configured nodes")
                .configuration()
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("command")
        );
        assert_ne!(c0_stdout, c1.label_stdout());
        assert_eq!(c1.starlark_label_stdout(), "@@//:consumer\n");
        assert_eq!(c0_topology, topology(&c1));
        assert_eq!(accepted_output_text(&c1_command), ["CONSUMER_ANALYSIS"]);

        let restored_command = run(&retained, target, None).unwrap();
        let restored = evaluation(&restored_command);
        assert_eq!(c0_stdout, restored.label_stdout());
        assert_eq!(c0_topology, topology(&restored));
        assert!(accepted_output_text(&restored_command).is_empty());

        let missing = "//:missing";
        let missing_command = run(&retained, missing, Some("command")).unwrap();
        let missing_error = missing_command
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert_eq!(missing_error.missing_stderr().unwrap().lines().count(), 3);

        let fresh = test_runtime(workspace.path()).unwrap();
        let one_shot = run(&fresh, target, None).unwrap();
        assert_eq!(c0_stdout, evaluation(&one_shot).label_stdout());

        let setting = "//:setting";
        let setting_command = run(&retained, setting, None).unwrap();
        assert_eq!(
            evaluation(&setting_command)
                .analyses()
                .next()
                .unwrap()
                .configured_target_key()
                .expect("current cquery analysis only contains configured nodes")
                .configuration()
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("default")
        );

        let parent = "//:parent";
        let parent_command = run(&retained, parent, None).unwrap();
        let parent = evaluation(&parent_command);
        let child = parent
            .analyses()
            .next()
            .unwrap()
            .configured_dependencies()
            .next()
            .unwrap();
        assert_eq!(child.label().to_string(), "@@//:consumer");
        assert_eq!(
            child
                .configuration()
                .root_string_setting()
                .map(|value| value.as_str()),
            Some("left")
        );
    }

    fn accepted_native_snapshot(runtime: &WorkspaceRuntime) -> AcceptedNativeDemandSnapshot {
        runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .clone()
    }

    fn assert_current_native_snapshot(
        runtime: &WorkspaceRuntime,
        expected: &AcceptedNativeDemandSnapshot,
    ) {
        let retained = accepted_native_snapshot(runtime);
        assert_eq!(retained.inputs, expected.inputs);
        assert_eq!(retained.repository_results, expected.repository_results);
        assert_eq!(retained.path_observations, expected.path_observations);
        assert_eq!(retained.selected, expected.selected);
        runtime.runtime.block_on(async {
            let updater = runtime
                .dice
                .updater_with_data(runtime.user_computation_data(None).unwrap());
            let mut transaction = updater.existing_state().await;
            let workspace = runtime.workspace.clone();
            assert_eq!(
                transaction
                    .compute(&NativeDemandWorkspaceRevisionKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                NativeDemandWorkspaceRevision(expected.inputs.generations.workspace_revision)
            );
            assert_eq!(
                transaction
                    .compute(&RegistryRequestGenerationKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                expected.inputs.generations.registry
            );
            assert_eq!(
                transaction
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                expected.inputs.generations.repository
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleCommandPolicyKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleCommandPolicy::from(
                    expected.inputs.request.command_policy.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleEnvironmentPolicyKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleEnvironmentPolicy::from(
                    expected.inputs.request.environment_policy.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleLockfileModeKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleLockfileMode::from(
                    expected.inputs.request.lockfile_mode.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleRegistryUrlsKey {
                        workspace: workspace.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleRegistryUrls::from(
                    expected.inputs.request.registry_urls.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RepositoryMaterializationResultEpochKey {
                        workspace: NormalizedAbsolutePath::new(workspace).unwrap(),
                    })
                    .await
                    .unwrap(),
                expected.repository_results
            );
            assert_eq!(
                transaction.compute(&PathObservationEpochKey).await.unwrap(),
                expected.path_observations
            );
        });
    }

    #[test]
    fn synthetic_driver_enforces_strict_progress_terminal_output_and_fresh_nonreplay() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::create_dir(root.join("vendor-aux")).unwrap();
        fs::write(root.join("first.txt"), "first").unwrap();
        fs::write(root.join("second.txt"), "second").unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let first_repo = local_native_request(&normalized, "dep+", "vendor");
        let second_repo = local_native_request(&normalized, "aux+", "vendor-aux");
        let first_path = native_host_file_demand(root.join("first.txt"));
        let second_path = native_host_file_demand(root.join("second.txt"));
        let plan = synthetic_plan(
            100,
            &normalized,
            [first_repo, second_repo],
            [first_path, second_path],
            Ok(SyntheticCommandValue::Build("built".into())),
        );
        let root_key = SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan });
        let runtime = test_runtime(&root).unwrap();

        let first = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                root_key.clone(),
            )
            .unwrap();
        assert_eq!(
            first.accepted.terminal_for_test(),
            &Ok(SyntheticCommandValue::Build("built".into()))
        );
        assert_eq!(first.attempts, 4);
        assert_eq!(first.terminal_root_count, 1);
        assert_eq!(accepted_output_text(&first.accepted), ["terminal-only"]);
        assert_eq!(
            runtime.native_demand_sessions.take_trace(),
            [
                NativeDemandTestTrace::SelectedInjectionCommitted,
                NativeDemandTestTrace::TerminalTransactionDropped,
                NativeDemandTestTrace::MaterializerAccepted,
                NativeDemandTestTrace::AcceptedSnapshotReplaced,
                NativeDemandTestTrace::OutputBufferMoved,
                NativeDemandTestTrace::LeaseClosed,
            ]
        );
        let mut projections = 0;
        let projected = first.accepted.project(|terminal| {
            projections += 1;
            assert_eq!(terminal, &Ok(SyntheticCommandValue::Build("built".into())));
            crate::runtime::TerminalOutput::new(0, "stdout".into(), "stderr".into())
        });
        assert_eq!(projections, 1);
        assert_eq!(
            projected.terminal_for_test(),
            &Ok(SyntheticCommandValue::Build("built".into()))
        );
        assert_eq!(
            projected.output_for_test(),
            &crate::runtime::TerminalOutput::new(0, "stdout".into(), "stderr".into())
        );
        assert_eq!(projected_output_text(&projected), ["terminal-only"]);

        let fresh = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                root_key,
            )
            .unwrap();
        assert_eq!(fresh.attempts, 1);
        assert_eq!(fresh.terminal_root_count, 1);
        assert!(
            fresh.accepted.batches_for_test().is_empty(),
            "a cached terminal child must not replay an earlier command's event"
        );
    }

    #[test]
    fn synthetic_root_identity_rejects_same_id_different_plan_reuse() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::create_dir(root.join("vendor-next")).unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let runtime = test_runtime(&root).unwrap();
        let first_request = local_native_request(&normalized, "dep+", "vendor");
        let first_plan = synthetic_plan(
            105,
            &normalized,
            [first_request],
            [],
            Ok(SyntheticCommandValue::Build("first".into())),
        );
        let first = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan: first_plan }),
            )
            .unwrap();
        assert_eq!(
            first.accepted.terminal_for_test(),
            &Ok(SyntheticCommandValue::Build("first".into()))
        );
        runtime.native_demand_sessions.take_trace();

        let second_request = local_native_request(&normalized, "dep+", "vendor-next");
        let mut second_plan = synthetic_plan(
            105,
            &normalized,
            [second_request.clone()],
            [],
            Ok(SyntheticCommandValue::Build("second".into())),
        );
        Arc::get_mut(&mut second_plan).unwrap().terminal_event = Some("second-event".into());
        let second = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan: second_plan }),
            )
            .unwrap();
        assert_eq!(
            second.accepted.terminal_for_test(),
            &Ok(SyntheticCommandValue::Build("second".into()))
        );
        assert_eq!(accepted_output_text(&second.accepted), ["second-event"]);
        assert_eq!(
            accepted_native_snapshot(&runtime)
                .selected
                .repository_requests(),
            &[second_request]
        );
    }

    #[test]
    fn synthetic_driver_has_no_retry_cap_and_accepts_complete_error_and_empty_query() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let mut paths = Vec::new();
        for index in 0..65 {
            let path = root.join(format!("path-{index}.txt"));
            fs::write(&path, index.to_string()).unwrap();
            paths.push(native_host_file_demand(path));
        }
        let runtime = test_runtime(&root).unwrap();
        let long_plan = synthetic_plan(
            110,
            &normalized,
            [],
            paths,
            Ok(SyntheticCommandValue::Build("many-paths".into())),
        );
        let long = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan: long_plan }),
            )
            .unwrap();
        assert_eq!(long.attempts, 66);
        assert_eq!(accepted_output_text(&long.accepted), ["terminal-only"]);

        let error_plan = synthetic_plan(
            111,
            &normalized,
            [],
            [],
            Err(SyntheticCommandError("terminal error".into())),
        );
        let completed_error = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan: error_plan }),
            )
            .unwrap();
        assert_eq!(
            completed_error.accepted.terminal_for_test(),
            &Err(SyntheticCommandError("terminal error".into()))
        );
        assert_eq!(
            accepted_output_text(&completed_error.accepted),
            ["terminal-only"]
        );
        let completed_error = completed_error.accepted.project(|terminal| {
            assert_eq!(
                terminal,
                &Err(SyntheticCommandError("terminal error".into()))
            );
            crate::runtime::TerminalOutput::new(2, String::new(), "semantic".into())
        });
        assert_eq!(
            completed_error.terminal_for_test(),
            &Err(SyntheticCommandError("terminal error".into()))
        );
        assert_eq!(projected_output_text(&completed_error), ["terminal-only"]);

        let mut query_plan = synthetic_plan(
            112,
            &normalized,
            [],
            [],
            Ok(SyntheticCommandValue::Query(Arc::from([]))),
        );
        Arc::get_mut(&mut query_plan).unwrap().terminal_event = None;
        let query = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Query(SyntheticQueryRootKey { plan: query_plan }),
            )
            .unwrap();
        assert_eq!(
            query.accepted.terminal_for_test(),
            &Ok(SyntheticCommandValue::Query(Arc::from([])))
        );
        assert_eq!(query.attempts, 1);
        assert_eq!(query.terminal_root_count, 1);
        assert!(query.accepted.batches_for_test().is_empty());
        let query = query.accepted.project(|terminal| {
            assert_eq!(terminal, &Ok(SyntheticCommandValue::Query(Arc::from([]))));
            crate::runtime::TerminalOutput::new(0, String::new(), String::new())
        });
        assert_eq!(
            query.terminal_for_test(),
            &Ok(SyntheticCommandValue::Query(Arc::from([])))
        );
        assert!(query.batches_for_test().is_empty());

        let mut reopened = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        reopened.discard().unwrap();
    }

    #[test]
    fn synthetic_driver_unwind_restores_the_complete_accepted_input_snapshot() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::write(root.join("probe.txt"), "probe").unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let request = local_native_request(&normalized, "dep+", "vendor");
        let path = native_host_file_demand(root.join("probe.txt"));
        let runtime = test_runtime(&root).unwrap();
        let accepted_inputs = NativeDemandRequestInputBundle {
            command_policy: BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
            environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(
                Some("all"),
            )
            .unwrap(),
            lockfile_mode: LockfileMode::Refresh,
            registry_urls: RegistryUrls::new(["https://accepted.example/"]),
        };
        let accepted_plan = synthetic_plan(
            120,
            &normalized,
            [request],
            [path],
            Ok(SyntheticCommandValue::Build("accepted".into())),
        );
        runtime
            .drive_synthetic_command(
                accepted_inputs.clone(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey {
                    plan: accepted_plan,
                }),
            )
            .unwrap();
        let before = runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .clone();

        let rejected_inputs = NativeDemandRequestInputBundle {
            command_policy: BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .unwrap(),
            lockfile_mode: LockfileMode::Off,
            registry_urls: RegistryUrls::new(["https://rejected.example/"]),
        };
        let mut panic_plan = synthetic_plan(
            121,
            &normalized,
            [],
            [],
            Ok(SyntheticCommandValue::Build("unreachable".into())),
        );
        Arc::get_mut(&mut panic_plan).unwrap().behavior = SyntheticRootBehavior::PanicAfterInputs;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime
                .drive_synthetic_command(
                    rejected_inputs,
                    SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan: panic_plan }),
                )
                .unwrap();
        }));
        assert!(panic.is_err());

        let after = runtime
            .native_demand_sessions
            .state
            .lock()
            .unwrap()
            .accepted
            .clone();
        assert_eq!(after.inputs, before.inputs);
        assert_eq!(after.repository_results, before.repository_results);
        assert_eq!(after.path_observations, before.path_observations);
        assert_eq!(after.selected, before.selected);
        assert_eq!(after.inputs.request, accepted_inputs);

        runtime.runtime.block_on(async {
            let updater = runtime
                .dice
                .updater_with_data(runtime.user_computation_data(None).unwrap());
            let mut transaction = updater.existing_state().await;
            assert_eq!(
                transaction
                    .compute(&NativeDemandWorkspaceRevisionKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                NativeDemandWorkspaceRevision(before.inputs.generations.workspace_revision)
            );
            assert_eq!(
                transaction
                    .compute(&RegistryRequestGenerationKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                before.inputs.generations.registry
            );
            assert_eq!(
                transaction
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                before.inputs.generations.repository
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleCommandPolicyKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleCommandPolicy::from(
                    accepted_inputs.command_policy.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleEnvironmentPolicyKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleEnvironmentPolicy::from(
                    accepted_inputs.environment_policy.clone()
                )
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleLockfileModeKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleLockfileMode::from(accepted_inputs.lockfile_mode.clone())
            );
            assert_eq!(
                transaction
                    .compute(&RootModuleRegistryUrlsKey {
                        workspace: root.clone(),
                    })
                    .await
                    .unwrap(),
                slug_bzlmod_v2::RootModuleRegistryUrls::from(accepted_inputs.registry_urls.clone())
            );
        });

        let mut reopened = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        reopened.discard().unwrap();
    }

    #[test]
    fn synthetic_compute_cancellation_restores_before_explicit_abort_and_reopens() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::write(root.join("probe.txt"), "probe").unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let request = local_native_request(&normalized, "dep+", "vendor");
        let path = native_host_file_demand(root.join("probe.txt"));
        let runtime = test_runtime(&root).unwrap();
        let accepted_inputs = NativeDemandRequestInputBundle {
            command_policy: BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
            environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(
                Some("all"),
            )
            .unwrap(),
            lockfile_mode: LockfileMode::Refresh,
            registry_urls: RegistryUrls::new(["https://accepted.example/"]),
        };
        let accepted_plan = synthetic_plan(
            125,
            &normalized,
            [request],
            [path],
            Ok(SyntheticCommandValue::Build("accepted".into())),
        );
        runtime
            .drive_synthetic_command(
                accepted_inputs,
                SyntheticCommandRoot::Build(SyntheticBuildRootKey {
                    plan: accepted_plan,
                }),
            )
            .unwrap();
        let before = accepted_native_snapshot(&runtime);
        runtime.native_demand_sessions.take_trace();

        let mut cancelled_plan = synthetic_plan(
            126,
            &normalized,
            [],
            [],
            Ok(SyntheticCommandValue::Build("unreachable".into())),
        );
        Arc::get_mut(&mut cancelled_plan).unwrap().behavior =
            SyntheticRootBehavior::PendForCancellation;
        let error = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey {
                    plan: cancelled_plan,
                }),
            )
            .unwrap_err();
        assert!(matches!(error, NativeDemandSessionError::Computation(_)));
        assert_eq!(
            runtime.native_demand_sessions.take_trace(),
            [NativeDemandTestTrace::AttemptTransactionDroppedBeforeAbort]
        );
        assert_current_native_snapshot(&runtime, &before);

        let mut reopened = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        reopened.discard().unwrap();
    }

    #[test]
    fn synthetic_selected_injection_failure_restores_and_exposes_no_output() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let runtime = test_runtime(&root).unwrap();
        let accepted_plan = synthetic_plan(
            130,
            &normalized,
            [],
            [],
            Ok(SyntheticCommandValue::Build("accepted".into())),
        );
        runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey {
                    plan: accepted_plan,
                }),
            )
            .unwrap();
        let before = accepted_native_snapshot(&runtime);
        runtime.native_demand_sessions.take_trace();

        runtime
            .native_demand_sessions
            .force_next_selected_injection_failure();
        let rejected_plan = synthetic_plan(
            131,
            &normalized,
            [],
            [],
            Ok(SyntheticCommandValue::Build("must-not-publish".into())),
        );
        let error = runtime
            .drive_synthetic_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                SyntheticCommandRoot::Build(SyntheticBuildRootKey {
                    plan: rejected_plan,
                }),
            )
            .unwrap_err();
        assert!(matches!(error, NativeDemandSessionError::Injection(_)));
        assert_eq!(
            runtime.native_demand_sessions.take_trace(),
            [
                NativeDemandTestTrace::TerminalTransactionDropped,
                NativeDemandTestTrace::AttemptTransactionDroppedBeforeAbort,
            ]
        );
        assert_current_native_snapshot(&runtime, &before);
        let mut reopened = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        reopened.discard().unwrap();
    }

    #[test]
    fn synthetic_irreversible_owner_failures_are_workspace_fail_closed_without_output() {
        for fail_close in [false, true] {
            let workspace = tempfile::tempdir().unwrap();
            let root = workspace.path().canonicalize().unwrap();
            let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
            let runtime = test_runtime(&root).unwrap();
            if fail_close {
                runtime.native_demand_sessions.force_next_close_failure();
            } else {
                runtime
                    .native_demand_sessions
                    .force_next_replace_accepted_failure();
            }
            let plan = synthetic_plan(
                if fail_close { 141 } else { 140 },
                &normalized,
                [],
                [],
                Ok(SyntheticCommandValue::Build("must-not-publish".into())),
            );
            let error = runtime
                .drive_synthetic_command(
                    NativeDemandRequestInputBundle::normalized_initial(),
                    SyntheticCommandRoot::Build(SyntheticBuildRootKey { plan }),
                )
                .unwrap_err();
            assert!(matches!(error, NativeDemandSessionError::StaleLease));
            let trace = runtime.native_demand_sessions.take_trace();
            let expected: &[NativeDemandTestTrace] = if fail_close {
                &[
                    NativeDemandTestTrace::SelectedInjectionCommitted,
                    NativeDemandTestTrace::TerminalTransactionDropped,
                    NativeDemandTestTrace::MaterializerAccepted,
                    NativeDemandTestTrace::AcceptedSnapshotReplaced,
                    NativeDemandTestTrace::OutputBufferMoved,
                ]
            } else {
                &[
                    NativeDemandTestTrace::SelectedInjectionCommitted,
                    NativeDemandTestTrace::TerminalTransactionDropped,
                    NativeDemandTestTrace::MaterializerAccepted,
                ]
            };
            assert_eq!(trace, expected);
            assert!(matches!(
                runtime.begin_native_demand_command(),
                Err(NativeDemandSessionError::Busy)
            ));

            // Materializer acceptance consumed its token and returned that
            // owner to Idle. Only the workspace owner deliberately remains
            // Busy after the irreversible bookkeeping fault.
            let materializer = runtime.repository_materializer.begin().unwrap();
            runtime
                .repository_materializer
                .discard(materializer)
                .unwrap();
        }
    }

    #[test]
    fn runtime_user_data_factory_installs_one_passive_or_eventful_tracker() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        runtime.runtime.block_on(async {
            let passive = runtime.user_computation_data(None).unwrap();
            assert!(passive.activation_tracker.is_some());
            assert!(
                passive
                    .activation_tracker
                    .as_ref()
                    .unwrap()
                    .tracks_rich_activations()
            );
            assert!(passive.cycle_detector.is_some());
            assert!(passive.data.get::<CaptureEvaluationEvents>().is_err());

            let effects = CommandEffectOwner::new();
            let attempt = effects.begin_attempt().unwrap();
            let eventful = runtime
                .user_computation_data(Some(attempt.clone()))
                .unwrap();
            assert!(eventful.activation_tracker.is_some());
            assert!(
                eventful
                    .activation_tracker
                    .as_ref()
                    .unwrap()
                    .tracks_rich_activations()
            );
            assert!(eventful.cycle_detector.is_some());
            assert!(eventful.data.get::<CaptureEvaluationEvents>().is_ok());
            attempt.finish_suppressed().unwrap();
        });
    }

    #[test]
    fn workspace_runtime_owns_distinct_exact_repository_materializers() {
        let workspace = tempfile::tempdir().unwrap();
        let first = test_runtime(workspace.path()).unwrap();
        let second = test_runtime(workspace.path()).unwrap();
        let expected =
            NormalizedAbsolutePath::new(workspace.path().canonicalize().unwrap()).unwrap();

        assert_eq!(first.repository_materializer.workspace(), &expected);
        assert_eq!(second.repository_materializer.workspace(), &expected);
        assert!(!Arc::ptr_eq(
            &first.repository_materializer,
            &second.repository_materializer,
        ));
    }

    #[test]
    fn native_demand_session_drives_real_dice_repository_then_path_then_complete() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::create_dir(root.join("vendor-aux")).unwrap();
        fs::create_dir(root.join("vendor-next")).unwrap();
        fs::create_dir(root.join("vendor-third")).unwrap();
        fs::write(root.join("probe.txt"), "observed").unwrap();
        fs::write(root.join("unprocessed.txt"), "must remain unobserved").unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let request = local_native_request(&normalized, "dep+", "vendor");
        let auxiliary = local_native_request(&normalized, "aux+", "vendor-aux");
        let path = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(root.join("probe.txt")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let unprocessed_path = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(root.join("unprocessed.txt")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let runtime = test_runtime(&root).unwrap();
        let preflight = runtime.begin_native_demand_command().unwrap();
        assert_eq!(
            preflight.generations(),
            preflight.command.inputs.generations
        );
        assert_eq!(
            preflight.repository_results(),
            &RepositoryMaterializationResultEpoch::new(normalized.clone(), []).unwrap()
        );
        assert!(preflight.path_observations().observations().is_empty());
        assert!(matches!(
            runtime.begin_native_demand_command(),
            Err(NativeDemandSessionError::Busy)
        ));
        let fixed = preflight.generations();
        let key = NativeDemandHandshakeKey {
            requests: Arc::from([request.clone(), auxiliary]),
            path: path.clone(),
            generations: fixed,
        };
        let mut command = NativeDemandAbortGuard::new(preflight.into_command());

        command.begin_attempt().unwrap();
        let first_need = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data().unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            let first_outcome = transaction.compute(&key).await.unwrap();
            let first_need = match first_outcome {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => need,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => {
                    panic!("first attempt unexpectedly completed: {value:?}")
                }
            };
            command.seal_retry().unwrap();
            drop(transaction);
            first_need
        });
        assert_eq!(first_need.repository_materializations().len(), 2);
        assert_eq!(
            first_need.path_observations().unwrap().demands(),
            &[path.clone()]
        );
        assert!(command.command().path_observations.get(&path).is_none());
        let progress = command.progress(&first_need).unwrap();
        assert_eq!(progress, NativeDemandProgress::Repositories);
        assert!(
            command.command().path_observations.get(&path).is_none(),
            "repository priority must not observe a simultaneous path Need"
        );
        assert_eq!(command.command().inputs.generations, fixed);

        command.begin_attempt().unwrap();
        let second_need = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data().unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            let second_outcome = transaction.compute(&key).await.unwrap();
            let second_need = match second_outcome {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => need,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => {
                    panic!("second attempt unexpectedly completed: {value:?}")
                }
            };
            assert!(second_need.repository_materializations().is_empty());
            assert_eq!(
                second_need.path_observations().unwrap().demands(),
                &[path.clone()]
            );
            command.seal_retry().unwrap();
            drop(transaction);
            second_need
        });
        let progress = command.progress(&second_need).unwrap();
        assert_eq!(progress, NativeDemandProgress::Paths);
        assert_eq!(command.command().inputs.generations, fixed);

        command.begin_attempt().unwrap();
        let prepared = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data().unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            let terminal_outcome = transaction.compute(&key).await.unwrap();
            assert!(matches!(
                terminal_outcome,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref() == "complete"
            ));
            let sealed = command.seal_terminal().unwrap();
            let sidecars = sealed.select(&transaction).await.unwrap();
            assert_eq!(
                sidecars.sidecars().demands().repository_requests(),
                &[request.clone()]
            );
            assert_eq!(
                sidecars.sidecars().demands().unscoped_paths(),
                &[path.clone()]
            );
            let prepared = command
                .prepare_accept(sidecars, &transaction)
                .await
                .unwrap();
            drop(terminal_outcome);
            drop(transaction);
            prepared
        });
        let accepted_terminal = command.accept_prepared(prepared, "complete").unwrap();
        assert_eq!(accepted_terminal.terminal_for_test(), &"complete");
        assert!(accepted_terminal.batches_for_test().is_empty());

        let accepted = runtime.begin_native_demand_command().unwrap();
        assert_ne!(
            accepted.repository_results(),
            &RepositoryMaterializationResultEpoch::new(normalized.clone(), []).unwrap()
        );
        assert!(accepted.path_observations().get(&path).is_some());
        assert_eq!(
            accepted.generations().workspace_revision.0,
            fixed.workspace_revision.0 + 1
        );
        assert_eq!(accepted.generations().registry.0, fixed.registry.0 + 1);
        assert_eq!(accepted.generations().repository.0, fixed.repository.0 + 1);

        // The next command's changed generations are the values actually
        // visible inside its fresh DICE attempt, not merely counters held by
        // the native owner.
        let next_key = NativeDemandHandshakeKey {
            requests: Arc::from([request.clone()]),
            path: path.clone(),
            generations: accepted.generations(),
        };
        let mut accepted = NativeDemandAbortGuard::new(accepted.into_command());
        accepted.begin_attempt().unwrap();
        runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(accepted.attempt_user_computation_data().unwrap());
            accepted.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(matches!(
                transaction.compute(&next_key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(_)
            ));
            accepted.seal_retry().unwrap();
            drop(transaction);
        });

        // Even with new path work present, an equal inherited repository Need
        // wins with typed repository nonprogress and consumes the command via
        // restore-before-discard.
        let inherited =
            slug_bzlmod_v2::SourcePreparationNeeds::repository(request.as_ref().clone());
        let inherited_with_path = inherited
            .try_union(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(unprocessed_path.clone()),
            ))
            .unwrap();
        let error = accepted.progress(&inherited_with_path).unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::RepositoryInternalNonProgress
        ));
        let restored_error = accepted.abort::<()>(error).unwrap_err();
        assert!(matches!(
            restored_error,
            NativeDemandSessionError::RepositoryInternalNonProgress
        ));

        let replacement = local_native_request(&normalized, "dep+", "vendor-next");
        let reopened = runtime.begin_native_demand_command().unwrap();
        assert!(
            reopened
                .path_observations()
                .get(&unprocessed_path)
                .is_none()
        );
        let mut reopened = NativeDemandAbortGuard::new(reopened.into_command());
        let progress = reopened
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                replacement.as_ref().clone(),
            ))
            .unwrap();
        assert_eq!(progress, NativeDemandProgress::Repositories);
        let competing = local_native_request(&normalized, "dep+", "vendor-third");
        let conflict_with_path =
            slug_bzlmod_v2::SourcePreparationNeeds::repository(competing.as_ref().clone())
                .try_union(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                    slug_workspace_v2::NeedPathObservations::singleton(unprocessed_path.clone()),
                ))
                .unwrap();
        let error = reopened.progress(&conflict_with_path).unwrap_err();
        assert!(matches!(
            &error,
            NativeDemandSessionError::ConflictingRepository(repo) if repo.as_str() == "dep+"
        ));
        let restored_error = reopened.abort::<()>(error).unwrap_err();
        assert!(matches!(
            restored_error,
            NativeDemandSessionError::ConflictingRepository(repo) if repo.as_str() == "dep+"
        ));

        // Both failure paths restored the accepted A/path snapshot before
        // discard and reopened the lease; neither observed the masked path.
        let restored = runtime.begin_native_demand_command().unwrap();
        assert!(restored.path_observations().get(&path).is_some());
        assert!(
            restored
                .path_observations()
                .get(&unprocessed_path)
                .is_none()
        );
        let mut restored = NativeDemandAbortGuard::new(restored.into_command());
        let progress = restored
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                replacement.as_ref().clone(),
            ))
            .unwrap();
        assert_eq!(progress, NativeDemandProgress::Repositories);
        restored.discard().unwrap();

        let mut repeated_path = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let error = repeated_path
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(path.clone()),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::PathInternalNonProgress
        ));
        repeated_path.abort::<()>(error).unwrap_err();
        let mut final_command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        final_command.discard().unwrap();
    }

    #[test]
    fn native_demand_acceptance_failure_restores_prior_snapshot_and_reopens_lease() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::create_dir(root.join("vendor")).unwrap();
        fs::create_dir(root.join("other")).unwrap();
        fs::write(root.join("probe.txt"), "observed").unwrap();
        let normalized = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let accepted_request = local_native_request(&normalized, "dep+", "vendor");
        let unselected_request = local_native_request(&normalized, "other+", "other");
        let path = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(root.join("probe.txt")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let runtime = test_runtime(&root).unwrap();

        let mut initial = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        initial
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                accepted_request.as_ref().clone(),
            ))
            .unwrap();
        initial
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(path.clone()),
            ))
            .unwrap();
        let accepted = initial
            .accept_selected_for_test(SelectedWorkspaceDemands::for_test(
                Arc::from([accepted_request.clone()]),
                Arc::from([path.clone()]),
            ))
            .unwrap();
        assert_eq!(accepted.terminal_for_test(), &());

        let mut failing = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        failing
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                unselected_request.as_ref().clone(),
            ))
            .unwrap();
        let error = failing
            .accept_selected_for_test(SelectedWorkspaceDemands::for_test_with_validation(
                Arc::from([accepted_request.clone()]),
                unselected_request,
                path.clone(),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::Repository(
                super::super::repository_io::RepositorySessionError::InvalidValidation(repo)
            ) if repo.as_str() == "other+"
        ));

        let restored = runtime.begin_native_demand_command().unwrap();
        assert!(restored.path_observations().get(&path).is_some());
        let mut restored = NativeDemandAbortGuard::new(restored.into_command());
        let error = restored
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                accepted_request.as_ref().clone(),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::RepositoryInternalNonProgress
        ));
        restored.abort::<()>(error).unwrap_err();
        let mut final_command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        final_command.discard().unwrap();
    }

    #[test]
    fn native_demand_restoration_failure_keeps_lease_and_materializer_fail_closed() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let mut command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        runtime
            .native_demand_sessions
            .force_next_restoration_failure();
        let error = command.discard().unwrap_err();
        assert!(matches!(error, NativeDemandSessionError::Restoration(_)));
        assert!(matches!(
            runtime.begin_native_demand_command(),
            Err(NativeDemandSessionError::Busy)
        ));
        assert!(matches!(
            runtime.repository_materializer.begin(),
            Err(super::super::repository_io::RepositorySessionError::Busy)
        ));
    }

    #[test]
    fn native_demand_materializer_begin_conflict_keeps_workspace_lease_fail_closed() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let foreign = runtime.repository_materializer.begin().unwrap();
        assert!(matches!(
            runtime.begin_native_demand_command(),
            Err(NativeDemandSessionError::Repository(
                super::super::repository_io::RepositorySessionError::Busy
            ))
        ));
        assert!(matches!(
            runtime.begin_native_demand_command(),
            Err(NativeDemandSessionError::Busy)
        ));
        runtime.repository_materializer.discard(foreign).unwrap();
    }

    #[test]
    fn native_demand_closure_selection_failure_discards_then_reopens_lease() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let foreign_runtime = test_runtime(workspace.path()).unwrap();
        let mut command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        command.begin_attempt().unwrap();

        runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data().unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(transaction.compute(&NativeTerminalProbeKey).await.unwrap());
            let sealed = command.seal_terminal().unwrap();

            let updater = foreign_runtime
                .dice
                .updater_with_data(foreign_runtime.user_computation_data(None).unwrap());
            let foreign_transaction = updater.commit().await;
            assert!(sealed.select(&foreign_transaction).await.is_err());
            drop(foreign_transaction);
            drop(transaction);
        });
        command.discard().unwrap();
        let reopened = runtime.begin_native_demand_command().unwrap();
        NativeDemandAbortGuard::new(reopened.into_command())
            .discard()
            .unwrap();
    }

    #[test]
    fn native_demand_accept_rejects_foreign_command_sidecars_and_restores() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let foreign_runtime = test_runtime(workspace.path()).unwrap();
        let mut foreign = NativeDemandAbortGuard::new(
            foreign_runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        foreign.begin_attempt().unwrap();
        let foreign_selection = foreign_runtime.runtime.block_on(async {
            let mut updater = foreign_runtime
                .dice
                .updater_with_data(foreign.attempt_user_computation_data().unwrap());
            foreign.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(transaction.compute(&NativeTerminalProbeKey).await.unwrap());
            let sealed = foreign.seal_terminal().unwrap();
            let selected = sealed.select(&transaction).await.unwrap();
            drop(transaction);
            selected
        });

        let mut command = NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let error = runtime.runtime.block_on(async {
            let updater = runtime
                .dice
                .updater_with_data(runtime.user_computation_data(None).unwrap());
            let transaction = updater.existing_state().await;
            let error = match command
                .prepare_accept(foreign_selection, &transaction)
                .await
            {
                Ok(_) => panic!("foreign command sidecars unexpectedly prepared"),
                Err(error) => error,
            };
            drop(transaction);
            error
        });
        assert!(matches!(error, NativeDemandSessionError::ForeignEffects));
        command.abort::<()>(error).unwrap_err();
        NativeDemandAbortGuard::new(
            runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        )
        .discard()
        .unwrap();
        foreign.discard().unwrap();
    }

    fn serve_registry_once(body: &'static [u8]) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        (format!("http://{address}/registry-file"), handle)
    }

    fn current_registry_inputs(
        runtime: &WorkspaceRuntime,
    ) -> (
        slug_bzlmod_v2::RootModuleRegistryUrls,
        slug_bzlmod_v2::RegistryRequestGeneration,
    ) {
        runtime.runtime.block_on(async {
            let updater = runtime.dice.updater_with_data(
                runtime
                    .user_computation_data(None)
                    .expect("install passive workspace activation tracking"),
            );
            let mut transaction = updater.existing_state().await;
            let urls = transaction
                .compute(&slug_bzlmod_v2::RootModuleRegistryUrlsKey {
                    workspace: runtime.workspace.clone(),
                })
                .await
                .unwrap();
            let generation = transaction
                .compute(&slug_bzlmod_v2::RegistryRequestGenerationKey {
                    workspace: runtime.workspace.clone(),
                })
                .await
                .unwrap();
            (urls, generation)
        })
    }

    fn fetch_registry_file(
        runtime: &WorkspaceRuntime,
        url: &str,
    ) -> Arc<Result<slug_bzlmod_v2::RegistryFileValue, slug_bzlmod_v2::RegistryFileError>> {
        runtime.runtime.block_on(async {
            let updater = runtime.dice.updater_with_data(
                runtime
                    .user_computation_data(None)
                    .expect("install passive workspace activation tracking"),
            );
            let mut transaction = updater.existing_state().await;
            transaction
                .compute(&slug_bzlmod_v2::RegistryFileKey {
                    workspace: runtime.workspace.clone(),
                    url: slug_bzlmod_v2::RegistryFileUrl::new(url),
                })
                .await
                .unwrap()
        })
    }

    #[test]
    fn query_and_build_share_registry_request_inputs_and_io_capability() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::write(root.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        fs::write(root.join("BUILD.bazel"), "").unwrap();
        fs::create_dir(root.join("pkg")).unwrap();
        fs::write(
            root.join("pkg/BUILD.bazel"),
            "filegroup(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = test_runtime(&root).unwrap();

        runtime
            .query_observations(
                observe_workspace(&root).unwrap(),
                "//pkg:probe",
                QueryOrder::Auto,
            )
            .unwrap();
        let (query_urls, query_generation) = current_registry_inputs(&runtime);
        assert_eq!(
            query_urls,
            slug_bzlmod_v2::RootModuleRegistryUrls::from(RegistryUrls::default_bazel_registry())
        );
        let (query_url, query_server) = serve_registry_once(b"query");
        assert!(matches!(
            fetch_registry_file(&runtime, &query_url).as_ref(),
            Ok(slug_bzlmod_v2::RegistryFileValue::Found { bytes, .. })
                if bytes.as_ref() == b"query"
        ));
        query_server.join().unwrap();

        runtime
            .evaluate_observations(observe_workspace(&root).unwrap(), &[])
            .unwrap();
        let (build_urls, build_generation) = current_registry_inputs(&runtime);
        assert_eq!(query_urls, build_urls);
        assert_ne!(query_generation, build_generation);
        let (build_url, build_server) = serve_registry_once(b"build");
        assert!(matches!(
            fetch_registry_file(&runtime, &build_url).as_ref(),
            Ok(slug_bzlmod_v2::RegistryFileValue::Found { bytes, .. })
                if bytes.as_ref() == b"build"
        ));
        build_server.join().unwrap();
    }

    #[test]
    fn registry_request_inputs_restore_a_b_a_and_malformed_input_consumes_nothing() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::write(root.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        fs::create_dir(root.join("pkg")).unwrap();
        fs::write(
            root.join("pkg/BUILD.bazel"),
            "filegroup(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = test_runtime(&root).unwrap();
        let query = |registry_urls: &[String]| {
            runtime.query_observations_with_policy_and_bzlmod_inputs(
                observe_workspace(&root).unwrap(),
                "//pkg:probe",
                QueryOrder::Auto,
                QueryPolicy::default(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                registry_urls,
            )
        };

        query(&[]).unwrap();
        let (default_urls, default_generation) = current_registry_inputs(&runtime);
        let override_urls = vec!["https://registry.example/a/".to_owned()];
        query(&override_urls).unwrap();
        let (override_value, override_generation) = current_registry_inputs(&runtime);
        assert_ne!(default_urls, override_value);
        assert_ne!(default_generation, override_generation);

        let malformed = query(&["file://bad".to_owned()]).unwrap_err().to_string();
        assert!(
            malformed.contains("Unsupported non-local file URL"),
            "{malformed}"
        );
        let (_, after_malformed_generation) = current_registry_inputs(&runtime);
        assert_eq!(after_malformed_generation, override_generation);

        query(&[]).unwrap();
        let (last_urls, last_generation) = current_registry_inputs(&runtime);
        assert_eq!(last_urls, default_urls);
        assert_ne!(last_generation, after_malformed_generation);
    }

    #[test]
    fn selected_directory_keys_preserve_absent_read_error_and_request_revision() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        let unknown = root.join("unknown");
        let unreadable = root.join("unreadable");
        fs::write(root.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        fs::write(root.join("BUILD.bazel"), "").unwrap();
        assert_eq!(
            WorkspaceDirectorySnapshot::empty().value(&unknown),
            WorkspaceDirectoryValue::Absent
        );

        let runtime = test_runtime(&root).unwrap();
        let (evaluation, directories) = runtime
            .evaluate_observations_with_directory_probes(
                WorkspaceObservation {
                    files: vec![
                        WorkspaceFileObservation::read(root.join("MODULE.bazel")),
                        WorkspaceFileObservation::read(root.join("BUILD.bazel")),
                    ],
                    raw_files: Vec::new(),
                    directories: vec![
                        WorkspaceDirectoryObservation {
                            path: unknown.clone(),
                            value: WorkspaceDirectoryValue::Absent,
                        },
                        WorkspaceDirectoryObservation {
                            path: unreadable.clone(),
                            value: WorkspaceDirectoryValue::ReadError(Arc::new(
                                "permission denied".to_owned(),
                            )),
                        },
                    ],
                },
                &[],
                &[unknown.clone(), unreadable.clone()],
            )
            .unwrap();

        assert_eq!(
            probed_directory_value(&directories, &unknown),
            WorkspaceDirectoryValue::Absent
        );
        assert_eq!(
            probed_directory_value(&directories, &unreadable),
            WorkspaceDirectoryValue::ReadError(Arc::new("permission denied".to_owned()))
        );
        assert!(
            directories
                .iter()
                .all(|(_, _, revision)| *revision == evaluation.revision)
        );
    }

    #[test]
    fn selected_directory_key_observes_create_rename_delete() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        let package = root.join("pkg");
        let unrelated = root.join("unrelated");
        fs::write(root.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        fs::write(root.join("BUILD.bazel"), "").unwrap();
        fs::create_dir(&package).unwrap();
        fs::create_dir(&unrelated).unwrap();
        let runtime = test_runtime(&root).unwrap();
        let probes = [package.clone(), unrelated.clone()];

        let (empty_evaluation, empty_directories) = runtime
            .evaluate_observations_with_directory_probes(
                observe_workspace(&root).unwrap(),
                &[],
                &probes,
            )
            .unwrap();
        let unchanged = probed_directory_value(&empty_directories, &unrelated);
        assert_directory_names(&probed_directory_value(&empty_directories, &package), &[]);

        fs::write(package.join("before"), "").unwrap();
        let (created_evaluation, created_directories) = runtime
            .evaluate_observations_with_directory_probes(
                observe_workspace(&root).unwrap(),
                &[],
                &probes,
            )
            .unwrap();
        assert_eq!(
            created_evaluation.revision,
            created_evaluation.workspace.revision
        );
        assert!(
            created_directories
                .iter()
                .all(|(_, _, revision)| *revision == created_evaluation.revision)
        );
        assert_directory_names(
            &probed_directory_value(&created_directories, &package),
            &["before"],
        );
        assert_eq!(
            unchanged,
            probed_directory_value(&created_directories, &unrelated)
        );

        fs::rename(package.join("before"), package.join("after")).unwrap();
        let (renamed_evaluation, renamed_directories) = runtime
            .evaluate_observations_with_directory_probes(
                observe_workspace(&root).unwrap(),
                &[],
                &probes,
            )
            .unwrap();
        assert_eq!(
            renamed_evaluation.revision,
            renamed_evaluation.workspace.revision
        );
        assert_directory_names(
            &probed_directory_value(&renamed_directories, &package),
            &["after"],
        );
        assert_eq!(
            unchanged,
            probed_directory_value(&renamed_directories, &unrelated)
        );

        fs::remove_file(package.join("after")).unwrap();
        let (deleted_evaluation, deleted_directories) = runtime
            .evaluate_observations_with_directory_probes(
                observe_workspace(&root).unwrap(),
                &[],
                &probes,
            )
            .unwrap();
        assert_ne!(deleted_evaluation.revision, empty_evaluation.revision);
        assert_eq!(
            deleted_evaluation.revision,
            deleted_evaluation.workspace.revision
        );
        assert_directory_names(&probed_directory_value(&deleted_directories, &package), &[]);
        assert_eq!(
            unchanged,
            probed_directory_value(&deleted_directories, &unrelated)
        );
    }

    #[test]
    fn explicit_query_inputs_restore_all_root_module_values_a_b_a() {
        use slug_identity_v2::ApparentRepoName;

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().canonicalize().unwrap();
        fs::write(
            root.join("MODULE.bazel"),
            "module(name = \"root\")\nbazel_dep(name = \"dev_dep\", version = \"1.0\", dev_dependency = True)\n",
        )
        .unwrap();
        fs::create_dir(root.join("pkg")).unwrap();
        fs::write(
            root.join("pkg/BUILD.bazel"),
            "filegroup(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = test_runtime(&root).unwrap();
        let query = |command, environment, mode| {
            runtime
                .query_observations_with_policy_and_bzlmod_inputs(
                    observe_workspace(&root).unwrap(),
                    "//pkg:probe",
                    QueryOrder::Auto,
                    QueryPolicy::default(),
                    command,
                    environment,
                    mode,
                    &[],
                )
                .unwrap()
        };

        assert_eq!(
            query(
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .stdout(),
            "//pkg:probe\n"
        );
        let first = runtime.current_root_module_graph_for_test().unwrap();
        fs::write(
            root.join("MODULE.bazel.lock"),
            "{\"lockFileVersion\":28, nope\n",
        )
        .unwrap();
        assert_eq!(
            query(
                BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
                LockfileMode::Off,
            )
            .stdout(),
            "//pkg:probe\n"
        );
        let middle = runtime.current_root_module_graph_for_test().unwrap();
        fs::remove_file(root.join("MODULE.bazel.lock")).unwrap();
        assert_eq!(
            query(
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .stdout(),
            "//pkg:probe\n"
        );
        let last = runtime.current_root_module_graph_for_test().unwrap();

        let dev_dep = ApparentRepoName::new("dev_dep").unwrap();
        assert_eq!(
            first.repository_mapping.resolve(&dev_dep).as_str(),
            "dev_dep+"
        );
        assert_eq!(
            middle.repository_mapping.resolve(&dev_dep).as_str(),
            "dev_dep"
        );
        assert_ne!(first.command_policy, middle.command_policy);
        assert_ne!(first.environment_policy, middle.environment_policy);
        assert_ne!(first.lockfile_mode, middle.lockfile_mode);
        assert!(matches!(
            first.visible_lockfile,
            slug_bzlmod_v2::VisibleLockfileRead::Parsed(_)
        ));
        assert_eq!(
            middle.visible_lockfile,
            slug_bzlmod_v2::VisibleLockfileRead::Ignored
        );
        assert_eq!(first.as_ref(), last.as_ref());
    }

    fn probed_directory_value(
        directories: &[(PathBuf, WorkspaceDirectoryValue, WorkspaceRevision)],
        path: &Path,
    ) -> WorkspaceDirectoryValue {
        directories
            .iter()
            .find(|(directory, _, _)| directory == path)
            .unwrap_or_else(|| panic!("missing evaluated directory for {}", path.display()))
            .1
            .clone()
    }

    fn assert_directory_names(value: &WorkspaceDirectoryValue, expected: &[&str]) {
        let WorkspaceDirectoryValue::Present(entries) = value else {
            panic!("expected present directory value: {value:?}");
        };
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }

    fn build_test_configuration(_value: &str) -> ConfigurationKey {
        ConfigurationKey::from_slug(
            SlugConfiguration::default_target(
                &HostConversionInputs::new(
                    Some(AutoCpuToken::K8),
                    Some(HostPathFlavor::Unix),
                    None,
                    Arc::from([]),
                    Arc::from([]),
                )
                .unwrap(),
            )
            .unwrap(),
        )
    }

    fn build_test_configuration_with_root_setting(value: &str) -> ConfigurationKey {
        build_test_configuration("base")
            .with_root_string_setting(RootStringSettingValue::new(value))
    }

    #[derive(Default)]
    struct BuildRootEpoch {
        entries: SmallMap<PathObservationDemand, PathObservationResult>,
    }

    impl BuildRootEpoch {
        fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
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

        fn file(&mut self, path: &str, source: &str, variant: i64) {
            self.node(path, PathNodeKind::RegularFile, variant);
            self.entries.insert(
                Self::demand(path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    source.as_bytes(),
                ))),
            );
        }

        fn base(variant: i64) -> Self {
            let mut epoch = Self::default();
            epoch.node("/", PathNodeKind::Directory, variant);
            epoch.node("/workspace", PathNodeKind::Directory, variant);
            epoch.file("/workspace/MODULE.bazel", "", variant);
            epoch.missing("/workspace/REPO.bazel");
            epoch.missing("/workspace/.bazelignore");
            epoch
        }

        fn package(&mut self, name: &str, source: &str, variant: i64) {
            let directory = format!("/workspace/{name}");
            self.node(&directory, PathNodeKind::Directory, variant);
            self.file(&format!("{directory}/BUILD.bazel"), source, variant);
        }

        fn deleted_package(&mut self, name: &str, variant: i64) {
            let directory = format!("/workspace/{name}");
            self.node(&directory, PathNodeKind::Directory, variant);
            self.missing(&format!("{directory}/BUILD.bazel"));
            self.missing(&format!("{directory}/BUILD"));
        }

        fn build(self) -> PathObservationEpoch {
            PathObservationEpoch::new(self.entries).unwrap()
        }
    }

    async fn build_root_transaction(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
    ) -> dice::DiceTransaction {
        let user_data = UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            ..Default::default()
        };
        build_root_transaction_with_data(dice, epoch, user_data).await
    }

    async fn build_root_transaction_with_data(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        mut user_data: UserComputationData,
    ) -> dice::DiceTransaction {
        user_data.cycle_detector = Some(bzl_load_cycle_detector());
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            slug_bzlmod_v2::RootPackagePolicyInputs::new(
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
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater.commit().await
    }

    #[tokio::test]
    async fn root_string_setting_preparation_keeps_distinct_transitioned_children() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut epoch = BuildRootEpoch::base(1);
        epoch.file(
            "/workspace/defs.bzl",
            r#"ConsumerInfo = provider(fields = {"value": "value"})
ParentInfo = provider(fields = {"value": "value"})
SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx): return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx): return [ConsumerInfo(value = ctx.attr._setting[SettingInfo].value)]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
def _left(settings, attr): return {"//:setting": "left"}
def _right(settings, attr): return {"//:setting": "right"}
left = transition(implementation = _left, inputs = [], outputs = ["//:setting"])
right = transition(implementation = _right, inputs = [], outputs = ["//:setting"])
def _parent(ctx): return [ParentInfo(value = "%s,%s" % (ctx.attr.left[0][ConsumerInfo].value, ctx.attr.right[0][ConsumerInfo].value))]
parent = rule(implementation = _parent, attrs = {"left": attr.label(cfg = left), "right": attr.label(cfg = right)})
"#,
            1,
        );
        epoch.package("", "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", left = \":consumer\", right = \":consumer\")\n", 1);
        let mut transaction = build_root_transaction(&dice, epoch.build()).await;
        let build_key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//:parent").unwrap()],
            build_test_configuration("first-build")
                .with_root_string_setting(RootStringSettingValue::new("default")),
        )
        .unwrap();
        let outcome = transaction.compute(&build_key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            [
                "@@//:parent",
                "@@//:consumer",
                "@@//:consumer",
                "@@//:setting",
                "@@//:setting",
            ]
        );
        let configured = evaluation
            .analyses()
            .map(|analysis| analysis.key())
            .collect::<Vec<_>>();
        assert_ne!(configured[1], configured[2]);
        assert_eq!(configured[1].label(), configured[2].label());
        assert_ne!(configured[3], configured[4]);
        assert_eq!(configured[3].label(), configured[4].label());
    }

    #[derive(Default)]
    struct ActionClosureTracker {
        roots: Mutex<Vec<(String, dice::ActivationKind, Option<EventBatch>)>>,
    }

    impl ActionClosureTracker {
        fn take(&self) -> Vec<(String, dice::ActivationKind, Option<EventBatch>)> {
            std::mem::take(&mut *self.roots.lock().unwrap())
        }
    }

    impl ActivationTracker for ActionClosureTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() else {
                return;
            };
            self.roots.lock().unwrap().push((
                key.node().label().to_string(),
                activation.kind(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
        }
    }

    #[derive(Default)]
    struct LegacyBuildTracker {
        typed_roots: AtomicUsize,
        forbidden: AtomicUsize,
    }

    impl ActivationTracker for LegacyBuildTracker {
        fn root_activated(&self, key: &DynKey, _activation: RootActivation) {
            if key.downcast_ref::<BuildCommandRootKey>().is_some() {
                self.typed_roots.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn key_activated(
            &self,
            key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<RootModuleGraphKey>().is_some()
                || key.downcast_ref::<WorkspaceEvaluationKey>().is_some()
                || key
                    .downcast_ref::<slug_loading_v2::keys::PackageLoadKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_workspace_v2::WorkspaceSnapshotKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_workspace_v2::WorkspaceRawSnapshotKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_loading_v2::keys::WorkspaceDirectorySnapshotKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_workspace_v2::WorkspaceFileKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_workspace_v2::WorkspaceRawFileKey>()
                    .is_some()
                || key
                    .downcast_ref::<slug_workspace_v2::WorkspaceDirectoryKey>()
                    .is_some()
            {
                self.forbidden.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn compute_build_root(
        transaction: &mut dice::DiceTransaction,
        key: &BuildCommandRootKey,
        tracker: &LegacyBuildTracker,
    ) -> BuildCommandOutcome {
        let before = tracker.typed_roots.load(Ordering::Relaxed);
        let value = transaction.compute(key).await.unwrap();
        assert_eq!(
            tracker.typed_roots.load(Ordering::Relaxed),
            before + 1,
            "each command compute must activate exactly one typed build root"
        );
        assert_eq!(tracker.forbidden.load(Ordering::Relaxed), 0);
        value
    }

    fn build_test_error(pattern: &str) -> BuildCommandError {
        BuildCommandError::target_not_found(
            Arc::from(pattern),
            PackagePath::parse("pkg").unwrap(),
            TargetName::parse("missing").unwrap(),
            PathBuf::from("/workspace/pkg/BUILD.bazel"),
        )
    }

    fn build_test_need(path: &str) -> slug_bzlmod_v2::SourcePreparationNeeds {
        slug_bzlmod_v2::SourcePreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            )),
        )
    }

    const ACTION_CLOSURE_DEFS: &str = r#"NodeInfo = provider(fields = {"value": "target name"})
def _node(ctx):
    print(ctx.label.name)
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.attr.marker + "\n")
    return [DefaultInfo(files = depset([out])), NodeInfo(value = ctx.label.name)]
node = rule(implementation = _node, attrs = {"deps": attr.label_list(), "marker": attr.string()})
"#;

    fn action_closure_epoch(
        variant: i64,
        shared_marker: &str,
        link_shared: bool,
        shared_present: bool,
    ) -> PathObservationEpoch {
        let mut epoch = BuildRootEpoch::base(variant);
        epoch.package("rules", "", variant);
        epoch.file("/workspace/rules/defs.bzl", ACTION_CLOSURE_DEFS, variant);
        epoch.package(
            "top",
            "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"top\", deps = [\"//left:left\", \"//right:right\"], marker = \"top\")\n",
            variant,
        );
        for side in ["left", "right"] {
            let deps = if link_shared {
                "deps = [\"//shared:shared\"], "
            } else {
                ""
            };
            epoch.package(
                side,
                &format!(
                    "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"{side}\", {deps}marker = \"{side}\")\n"
                ),
                variant,
            );
        }
        if shared_present {
            epoch.package(
                "shared",
                &format!(
                    "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"shared\", marker = \"{shared_marker}\")\n"
                ),
                variant,
            );
        } else {
            epoch.deleted_package("shared", variant);
        }
        epoch.build()
    }

    const DELEGATING_DEFS: &str = r#"def _producer(ctx):
    out = ctx.actions.declare_file("producer.out")
    ctx.actions.write(out, "producer\n")
    return [DefaultInfo(files = depset([out]))]
producer = rule(implementation = _producer, attrs = {"out": attr.output()})
def _root(ctx): return [DefaultInfo()]
root_rule = rule(implementation = _root, attrs = {
    "aliased": attr.label(),
    "src": attr.label(allow_single_file = True),
    "generated": attr.label(allow_single_file = True),
})
"#;

    const DELEGATING_BUILD: &str = r#"load(":defs.bzl", "producer", "root_rule")
producer(name = "producer", out = "producer.out")
alias(name = "alias_inner", actual = ":producer")
alias(name = "alias_outer", actual = ":alias_inner")
package_group(name = "vis_leaf", packages = ["//..."])
package_group(name = "vis_top", includes = [":vis_leaf"])
root_rule(
    name = "root",
    aliased = ":alias_outer",
    src = "source.txt",
    generated = ":producer.out",
    visibility = [":vis_top"],
)
"#;

    const CQUERY_DELEGATING_DEFS: &str = r#"SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _transition(settings, attr):
    return {"//:setting": "transitioned"}
to_transition = transition(implementation = _transition, inputs = [], outputs = ["//:setting"])
def _ordinary(ctx):
    return [DefaultInfo()]
ordinary_rule = rule(implementation = _ordinary, attrs = {
    "normal": attr.label(),
    "transitioned": attr.label(cfg = to_transition),
    "aliased": attr.label(),
    "src": attr.label(allow_single_file = True),
    "generated": attr.label(allow_single_file = True),
})
def _broken(ctx): fail("seed must not be analyzed")
broken_rule = rule(implementation = _broken)
def _transition_only(ctx):
    if ctx.attr._setting[SettingInfo].value != "transitioned": fail("default seed must not be analyzed")
    return [DefaultInfo()]
transition_only_rule = rule(implementation = _transition_only, attrs = {"_setting": attr.label(default = "//:setting")})
def _producer(ctx):
    out = ctx.actions.declare_file("producer.out")
    ctx.actions.write(out, "producer\n")
    return [DefaultInfo(files = depset([out]))]
producer = rule(implementation = _producer, attrs = {"out": attr.output()})
"#;

    const CQUERY_DELEGATING_BUILD: &str = r#"load(":defs.bzl", "broken_rule", "ordinary_rule", "producer", "string_setting", "transition_only_rule")
string_setting(name = "setting", build_setting_default = "default")
ordinary_rule(name = "ordinary")
broken_rule(name = "broken")
transition_only_rule(name = "transition_only")
ordinary_rule(name = "transition_root", transitioned = ":transition_only")
producer(name = "producer", out = "producer.out")
alias(name = "alias_inner", actual = ":ordinary")
alias(name = "alias_outer", actual = ":alias_inner")
package_group(name = "vis_leaf", packages = ["//..."])
package_group(name = "vis_top", includes = [":vis_leaf"])
ordinary_rule(
    name = "root",
    normal = ":ordinary",
    transitioned = ":ordinary",
    aliased = ":alias_outer",
    src = "source.txt",
    generated = ":producer.out",
    visibility = [":vis_top"],
)
"#;

    fn delegating_action_closure_epoch(variant: i64) -> PathObservationEpoch {
        let mut epoch = BuildRootEpoch::base(variant);
        epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, variant);
        epoch.package("", DELEGATING_BUILD, variant);
        epoch.file("/workspace/source.txt", "source\n", variant);
        epoch.build()
    }

    fn complete_build_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("build command retained Needs")
        };
        value.as_ref().as_ref().unwrap()
    }

    #[test]
    fn build_command_root_identity_is_canonical_ordered_and_preflighted() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let shorthand = BuildCommandRootKey::new(
            workspace.clone(),
            &[TargetPattern::parse("//pkg").unwrap()],
            configuration.clone(),
        )
        .unwrap();
        let explicit = BuildCommandRootKey::new(
            workspace.clone(),
            &[TargetPattern::parse("//pkg:pkg").unwrap()],
            configuration.clone(),
        )
        .unwrap();
        assert_eq!(shorthand, explicit);
        assert_eq!(shorthand.targets.as_ref(), [Arc::<str>::from("//pkg:pkg")]);

        let duplicate = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//pkg:pkg").unwrap(),
                TargetPattern::parse("//pkg:pkg").unwrap(),
            ],
            configuration.clone(),
        )
        .unwrap();
        let reversed = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//other:t").unwrap(),
                TargetPattern::parse("//pkg:pkg").unwrap(),
            ],
            configuration.clone(),
        )
        .unwrap();
        assert_ne!(duplicate, explicit);
        assert_ne!(reversed, explicit);
        assert_ne!(
            explicit,
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse("//pkg:pkg").unwrap()],
                build_test_configuration_with_root_setting("other"),
            )
            .unwrap()
        );
        assert_ne!(
            explicit,
            BuildCommandRootKey::new(
                NormalizedAbsolutePath::new("/other").unwrap(),
                &[TargetPattern::parse("//pkg:pkg").unwrap()],
                configuration.clone(),
            )
            .unwrap()
        );
        assert!(
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse("@repo//pkg:t").unwrap()],
                configuration.clone(),
            )
            .is_ok()
        );
        for targets in [
            vec![TargetPattern::parse("@repo//pkg:all").unwrap()],
            vec![TargetPattern::parse("@repo//pkg/...").unwrap()],
            vec![
                TargetPattern::parse("//pkg:t").unwrap(),
                TargetPattern::parse("@repo//pkg:t").unwrap(),
            ],
            vec![
                TargetPattern::parse("@repo//pkg:one").unwrap(),
                TargetPattern::parse("@repo//pkg:two").unwrap(),
            ],
        ] {
            assert!(matches!(
                BuildCommandRootKey::new(workspace.clone(), &targets, configuration.clone()),
                Err(BuildCommandRequestError::ExternalRepository { .. })
            ));
        }
        assert!(matches!(
            BuildCommandRootKey::new(
                workspace,
                &[TargetPattern::parse("//pkg/...").unwrap()],
                configuration,
            ),
            Err(BuildCommandRequestError::RecursivePattern { pattern })
                if pattern.as_ref() == "//pkg/..."
        ));
    }

    #[tokio::test]
    async fn build_command_root_anchors_empty_and_preserves_ordered_package_results() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LegacyBuildTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let empty_key =
            BuildCommandRootKey::new(workspace.clone(), &[], configuration.clone()).unwrap();
        let mut empty =
            build_root_transaction_with_data(&dice, BuildRootEpoch::base(1).build(), user_data)
                .await;
        let empty_outcome = compute_build_root(&mut empty, &empty_key, &tracker).await;
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(empty_value) = &empty_outcome else {
            panic!("complete root anchor returned Need");
        };
        assert!(empty_value.as_ref().as_ref().unwrap().targets.is_empty());
        assert!(BuildCommandRootKey::validity(&empty_outcome));
        assert!(BuildCommandRootKey::equality(
            &empty_outcome,
            &empty_outcome
        ));

        let targets = [
            TargetPattern::parse("//second:all").unwrap(),
            TargetPattern::parse("//first:t").unwrap(),
            TargetPattern::parse("//first:t").unwrap(),
        ];
        let key = BuildCommandRootKey::new(workspace, &targets, configuration).unwrap();
        let mut epoch = BuildRootEpoch::base(2);
        epoch.package("first", "filegroup(name = \"t\")\n", 2);
        epoch.package("second", "filegroup(name = \"other\")\n", 2);
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let value = compute_build_root(&mut transaction, &key, &tracker).await;
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = value else {
            panic!("complete package bundle returned Need");
        };
        let targets = &value.as_ref().as_ref().unwrap().targets;
        assert_eq!(
            targets
                .iter()
                .map(|target| target.pattern.as_ref())
                .collect::<Vec<_>>(),
            ["//second:all", "//first:t", "//first:t"]
        );
        assert!(targets.iter().all(|target| target.analysis.is_none()));
    }

    #[tokio::test]
    async fn build_action_closure_traverses_alias_and_generated_nodes_but_excludes_null_nodes() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//:root").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut transaction =
            build_root_transaction(&dice, delegating_action_closure_epoch(1)).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);

        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 1);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            [
                "@@//:root",
                "@@//:alias_outer",
                "@@//:producer.out",
                "@@//:alias_inner",
                "@@//:producer",
            ]
        );
        assert!(
            evaluation
                .analyses()
                .all(|analysis| analysis.configured_target_key().is_some())
        );
        let root = evaluation.analyses().next().unwrap();
        assert_eq!(root.edges().len(), 4);
        assert_eq!(
            root.edges()
                .iter()
                .filter(|edge| edge.target().configured_target().is_none())
                .count(),
            2,
            "source and declaring-visibility null nodes stay outside the build action closure"
        );
        let producer = evaluation.analyses().last().unwrap();
        assert_eq!(producer.actions().len(), 1);
        assert_eq!(producer.actions()[0].outputs()[0].path(), "producer.out");
    }

    #[tokio::test]
    async fn build_action_closure_is_roots_first_breadth_first_and_deduplicates_diamonds() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[
                TargetPattern::parse("//top:top").unwrap(),
                TargetPattern::parse("//left:left").unwrap(),
                TargetPattern::parse("//top:top").unwrap(),
            ],
            build_test_configuration("target"),
        )
        .unwrap();
        let tracker = Arc::new(ActionClosureTracker::default());
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut transaction = build_root_transaction_with_data(
            &dice,
            action_closure_epoch(1, "shared-a", true, true),
            user_data,
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(evaluation.analyzed_target_count(), 3);
        assert_eq!(evaluation.declared_action_count(), 4);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            [
                "@@//top:top",
                "@@//left:left",
                "@@//right:right",
                "@@//shared:shared",
            ]
        );
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.actions()[0].outputs()[0].path())
                .collect::<Vec<_>>(),
            [
                "top/top.txt",
                "left/left.txt",
                "right/right.txt",
                "shared/shared.txt",
            ]
        );
        assert!(Arc::ptr_eq(
            evaluation.targets[0].analysis.as_ref().unwrap(),
            &evaluation.action_closure[0],
        ));
        assert!(Arc::ptr_eq(
            evaluation.targets[1].analysis.as_ref().unwrap(),
            &evaluation.action_closure[1],
        ));
        assert!(Arc::ptr_eq(
            evaluation.targets[0].analysis.as_ref().unwrap(),
            evaluation.targets[2].analysis.as_ref().unwrap(),
        ));
        let activations = tracker.take();
        let mut evaluated = activations
            .iter()
            .filter(|(_, kind, _)| *kind == dice::ActivationKind::Evaluated)
            .map(|(label, _, batch)| {
                assert_eq!(
                    batch.as_ref().map(|batch| batch.events().len()),
                    Some(1),
                    "target-local event batch for {label}"
                );
                label.as_str()
            })
            .collect::<Vec<_>>();
        evaluated.sort();
        assert_eq!(
            evaluated,
            [
                "@@//left:left",
                "@@//right:right",
                "@@//shared:shared",
                "@@//top:top",
            ]
        );

        let mut warm_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        warm_data.data.set(CaptureEvaluationEvents);
        let mut warm_transaction = build_root_transaction_with_data(
            &dice,
            action_closure_epoch(1, "shared-a", true, true),
            warm_data,
        )
        .await;
        let warm = warm_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&outcome, &warm));
        assert!(tracker.take().is_empty());
    }

    #[tokio::test]
    async fn build_action_closure_retains_accepted_parent_second_first_actions() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut epoch = BuildRootEpoch::base(5);
        epoch.package("rules", "", 5);
        epoch.file("/workspace/rules/defs.bzl", ACTION_CLOSURE_DEFS, 5);
        epoch.package(
            "leaf",
            "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"first\", marker = \"first\")\nnode(name = \"second\", marker = \"second\")\n",
            5,
        );
        epoch.package(
            "parent",
            "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"parent\", deps = [\"//leaf:second\", \"//leaf:first\"], marker = \"parent\")\n",
            5,
        );
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//parent:parent").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut transaction = build_root_transaction(&dice, epoch.build()).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 3);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            ["@@//parent:parent", "@@//leaf:second", "@@//leaf:first"]
        );
    }

    #[tokio::test]
    async fn build_action_frontier_need_precedes_an_earlier_sibling_analysis_error() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let configuration = build_test_configuration("target");
        let error_target = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//error:missing").unwrap(),
            configuration.clone(),
        );
        let mut epoch = BuildRootEpoch::base(6);
        epoch.package("error", "", 6);
        let mut transaction = build_root_transaction(&dice, epoch.build()).await;
        let error = transaction
            .compute(
                &ConfiguredNodeAnalysisKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    error_target.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            error,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().is_err()
        ));

        let need_target = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//need:child").unwrap(),
            configuration,
        );
        let reduced = collect_build_action_frontier(vec![
            (error_target, error),
            (
                need_target,
                slug_bzlmod_v2::SourcePreparationOutcome::Need(build_test_need("/workspace/need")),
            ),
        ])
        .unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = reduced else {
            panic!("same-frontier analysis error won over sibling Need")
        };
        assert_eq!(
            needs.path_observations().unwrap().demands()[0]
                .path()
                .as_path(),
            Path::new("/workspace/need")
        );
    }

    #[tokio::test]
    async fn build_action_closure_tracks_child_actions_prunes_orphans_and_restores_equality() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//top:top").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut first_transaction =
            build_root_transaction(&dice, action_closure_epoch(10, "shared-a", true, true)).await;
        let first = first_transaction.compute(&key).await.unwrap();
        let first_evaluation = complete_build_evaluation(&first);
        let first_parent = first_evaluation.action_closure[0].dupe();
        let first_shared = first_evaluation.action_closure[3].dupe();

        let mut warm_transaction =
            build_root_transaction(&dice, action_closure_epoch(10, "shared-a", true, true)).await;
        let warm = warm_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &warm));

        let mut edited_transaction =
            build_root_transaction(&dice, action_closure_epoch(11, "shared-b", true, true)).await;
        let edited = edited_transaction.compute(&key).await.unwrap();
        let edited_evaluation = complete_build_evaluation(&edited);
        assert!(!BuildCommandRootKey::equality(&first, &edited));
        assert_eq!(
            first_parent.as_ref(),
            edited_evaluation.action_closure[0].as_ref()
        );
        assert_ne!(
            first_shared.as_ref(),
            edited_evaluation.action_closure[3].as_ref()
        );

        let mut orphaned_transaction =
            build_root_transaction(&dice, action_closure_epoch(12, "shared-b", false, true)).await;
        let orphaned = orphaned_transaction.compute(&key).await.unwrap();
        assert_eq!(
            complete_build_evaluation(&orphaned).declared_action_count(),
            3
        );
        let mut pruned_transaction =
            build_root_transaction(&dice, action_closure_epoch(12, "shared-c", false, true)).await;
        let pruned = pruned_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&orphaned, &pruned));

        let mut deleted_transaction =
            build_root_transaction(&dice, action_closure_epoch(13, "shared-b", true, false)).await;
        let deleted = deleted_transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().is_err()
        ));

        let mut restored_transaction =
            build_root_transaction(&dice, action_closure_epoch(14, "shared-a", true, true)).await;
        let restored = restored_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &restored));
    }

    #[tokio::test]
    async fn build_command_root_selects_each_terminal_producer_once_for_duplicate_targets() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt().unwrap();
        let demands = WorkspaceDemandOwner::new(&dice, workspace.clone());
        let mut user_data = UserComputationData::default();
        demands
            .install(&dice, &mut user_data, Some(attempt.clone()))
            .unwrap();

        let definitions = r#"
print("BZL")
def _impl(ctx):
    print("ANALYSIS")
    return [DefaultInfo(files = depset([]))]
probe = rule(implementation = _impl)
"#;
        let mut epoch = BuildRootEpoch::base(3);
        epoch.file("/workspace/MODULE.bazel", "print(\"MODULE\")\n", 3);
        epoch.package("rules", "", 3);
        epoch.file("/workspace/rules/defs.bzl", definitions, 3);
        epoch.package(
            "app",
            "print(\"BUILD\")\nload(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
            3,
        );
        let targets = [
            TargetPattern::parse("//app:t").unwrap(),
            TargetPattern::parse("//app:t").unwrap(),
        ];
        let key = BuildCommandRootKey::new(workspace, &targets, build_test_configuration("target"))
            .unwrap();
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let value = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            value,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().as_ref().unwrap().targets.len() == 2
        ));
        let sealed = attempt.seal_terminal().unwrap();
        assert_eq!(sealed.root_count(), 1);
        let sidecars = sealed.select(&transaction).await.unwrap();
        let texts = sidecars
            .events()
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, ["MODULE", "BZL", "BUILD", "ANALYSIS"]);
    }

    #[tokio::test]
    async fn build_command_root_terminal_closure_retains_reused_and_clears_retry_only_batches() {
        for (retain_prints, expected) in [
            (true, vec!["MODULE", "BZL", "BUILD", "ANALYSIS"]),
            (false, Vec::new()),
        ] {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
            let effects = CommandEffectOwner::new();
            let demands = WorkspaceDemandOwner::new(&dice, workspace.clone());
            let key = BuildCommandRootKey::new(
                workspace,
                &[
                    TargetPattern::parse("//app:t").unwrap(),
                    TargetPattern::parse("//later:all").unwrap(),
                ],
                build_test_configuration("target"),
            )
            .unwrap();
            let epoch = |variant: i64, prints: bool, later: bool| {
                let mut epoch = BuildRootEpoch::base(variant);
                epoch.file(
                    "/workspace/MODULE.bazel",
                    if prints { "print(\"MODULE\")\n" } else { "" },
                    variant,
                );
                epoch.package("rules", "", variant);
                let definition_name = if prints { "old.bzl" } else { "new.bzl" };
                epoch.file(
                    &format!("/workspace/rules/{definition_name}"),
                    if prints {
                        "print(\"BZL\")\ndef _impl(ctx):\n    print(\"ANALYSIS\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                    } else {
                        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                    },
                    variant,
                );
                epoch.package(
                    "app",
                    if prints {
                        "print(\"BUILD\")\nload(\"//rules:old.bzl\", \"probe\")\nprobe(name = \"t\")\n"
                    } else {
                        "load(\"//rules:new.bzl\", \"probe\")\nprobe(name = \"t\")\n"
                    },
                    variant,
                );
                if later {
                    epoch.package("later", "filegroup(name = \"t\")\n", variant);
                }
                epoch.build()
            };

            let retry = effects.begin_attempt().unwrap();
            let mut retry_data = UserComputationData::default();
            demands
                .install(&dice, &mut retry_data, Some(retry.clone()))
                .unwrap();
            let mut retry_transaction =
                build_root_transaction_with_data(&dice, epoch(60, true, false), retry_data).await;
            assert!(matches!(
                retry_transaction.compute(&key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
            ));
            retry.seal_retry().unwrap();

            let terminal = effects.begin_attempt().unwrap();
            let mut terminal_data = UserComputationData::default();
            demands
                .install(&dice, &mut terminal_data, Some(terminal.clone()))
                .unwrap();
            let terminal_variant = if retain_prints { 60 } else { 61 };
            let mut terminal_transaction = build_root_transaction_with_data(
                &dice,
                epoch(terminal_variant, retain_prints, true),
                terminal_data,
            )
            .await;
            assert!(matches!(
                terminal_transaction.compute(&key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref().is_ok()
            ));
            let selected = terminal
                .seal_terminal()
                .unwrap()
                .select(&terminal_transaction)
                .await
                .unwrap();
            if expected.is_empty() {
                assert!(
                    selected.events().batches().is_empty(),
                    "empty terminal producers retained event batches"
                );
            }
            let texts = selected
                .events()
                .batches()
                .iter()
                .flat_map(EventBatch::events)
                .map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                    EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
                })
                .collect::<Vec<_>>();
            assert_eq!(texts, expected);
        }
    }

    #[tokio::test]
    async fn build_command_root_unions_target_needs_and_replays_typed_analysis_lifecycle() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let union_key = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//left:t").unwrap(),
                TargetPattern::parse("//right:t").unwrap(),
            ],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut need_transaction =
            build_root_transaction(&dice, BuildRootEpoch::base(10).build()).await;
        let need = need_transaction.compute(&union_key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = &need else {
            panic!("independent missing packages did not return Need");
        };
        let paths = needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .map(|demand| demand.path().as_path())
            .collect::<Vec<_>>();
        assert!(paths.contains(&Path::new("/workspace/left")), "{paths:?}");
        assert!(paths.contains(&Path::new("/workspace/right")), "{paths:?}");
        assert!(!BuildCommandRootKey::validity(&need));
        assert!(!BuildCommandRootKey::equality(&need, &need));

        let key = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//app:t").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let epoch = |variant: i64, marker: &str, deleted: bool| {
            let mut epoch = BuildRootEpoch::base(variant);
            epoch.package("rules", "", variant);
            epoch.file(
                "/workspace/rules/defs.bzl",
                &format!(
                    "def _impl(ctx):\n    print(\"{marker}\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                ),
                variant,
            );
            if deleted {
                epoch.deleted_package("app", variant);
            } else {
                epoch.package(
                    "app",
                    "load(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
                    variant,
                );
            }
            epoch.build()
        };
        let mut first_transaction = build_root_transaction(&dice, epoch(11, "V1", false)).await;
        let first = first_transaction.compute(&key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("first typed analysis returned Need");
        };
        assert!(
            first_value.as_ref().as_ref().unwrap().targets[0]
                .analysis
                .is_some()
        );

        let mut edited_transaction = build_root_transaction(&dice, epoch(12, "V2", false)).await;
        let edited = edited_transaction.compute(&key).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&first, &edited));

        let mut deleted_transaction = build_root_transaction(&dice, epoch(13, "V2", true)).await;
        let deleted = deleted_transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));

        let mut restored_transaction = build_root_transaction(&dice, epoch(14, "V1", false)).await;
        let restored = restored_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &restored));
    }

    #[tokio::test]
    async fn build_command_root_anchor_need_and_error_suppress_target_branches() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//app:t").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut missing_epoch = BuildRootEpoch::default();
        missing_epoch.node("/", PathNodeKind::Directory, 19);
        missing_epoch.node("/workspace", PathNodeKind::Directory, 19);
        let mut missing_anchor = build_root_transaction(&dice, missing_epoch.build()).await;
        let missing = missing_anchor.compute(&key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = missing else {
            panic!("missing anchor did not return Need");
        };
        let paths = needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .map(|demand| demand.path().as_path())
            .collect::<Vec<_>>();
        assert!(
            paths.contains(&Path::new("/workspace/MODULE.bazel")),
            "{paths:?}"
        );
        assert!(!paths.contains(&Path::new("/workspace/app")), "{paths:?}");

        let mut invalid_epoch = BuildRootEpoch::base(20);
        invalid_epoch.file("/workspace/MODULE.bazel", "this is invalid (", 20);
        let mut invalid_anchor = build_root_transaction(&dice, invalid_epoch.build()).await;
        let invalid = invalid_anchor.compute(&key).await.unwrap();
        assert!(matches!(
            invalid,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::RootAnchor(_),
                    })
                )
        ));
    }

    #[tokio::test]
    async fn build_command_root_real_branches_use_no_legacy_keys_and_structure_missing_target() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LegacyBuildTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut epoch = BuildRootEpoch::base(30);
        epoch.package("native", "filegroup(name = \"t\")\n", 30);
        epoch.package("rules", "", 30);
        epoch.file(
            "/workspace/rules/defs.bzl",
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
            30,
        );
        epoch.package(
            "custom",
            "load(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
            30,
        );
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        for pattern in ["//native:all", "//native:t", "//custom:t"] {
            let key = BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse(pattern).unwrap()],
                configuration.clone(),
            )
            .unwrap();
            let value = compute_build_root(&mut transaction, &key, &tracker).await;
            assert!(matches!(
                value,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref().is_ok()
            ));
        }
        let missing_key = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//native:missing").unwrap()],
            configuration,
        )
        .unwrap();
        let missing = compute_build_root(&mut transaction, &missing_key, &tracker).await;
        assert!(matches!(
            missing,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::TargetNotFound {
                            pattern,
                            package,
                            target,
                            build_file,
                        },
                    }) if pattern.as_ref() == "//native:missing"
                        && package.as_str() == "native"
                        && target.as_str() == "missing"
                        && build_file == Path::new("/workspace/native/BUILD.bazel")
                )
        ));
        assert_eq!(tracker.forbidden.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn build_command_root_replays_root_module_and_build_create_edit_delete_restore() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let empty =
            BuildCommandRootKey::new(workspace.clone(), &[], configuration.clone()).unwrap();
        let module_epoch = |variant: i64, source: Option<&str>| {
            let mut epoch = BuildRootEpoch::base(variant);
            match source {
                Some(source) => epoch.file("/workspace/MODULE.bazel", source, variant),
                None => epoch.missing("/workspace/MODULE.bazel"),
            }
            epoch.build()
        };
        let mut missing_module = build_root_transaction(&dice, module_epoch(40, None)).await;
        let module_missing = missing_module.compute(&empty).await.unwrap();
        assert!(matches!(
            module_missing,
            slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
        ));
        assert!(!BuildCommandRootKey::validity(&module_missing));
        let mut created_module =
            build_root_transaction(&dice, module_epoch(41, Some("module(name = \"one\")\n"))).await;
        let module_v1 = created_module.compute(&empty).await.unwrap();
        let mut edited_module =
            build_root_transaction(&dice, module_epoch(42, Some("module(name = \"two\")\n"))).await;
        let module_v2 = edited_module.compute(&empty).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&module_v1, &module_v2));
        let mut deleted_module = build_root_transaction(&dice, module_epoch(43, None)).await;
        let module_deleted = deleted_module.compute(&empty).await.unwrap();
        assert!(matches!(
            module_deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
        ));
        let mut restored_module =
            build_root_transaction(&dice, module_epoch(44, Some("module(name = \"one\")\n"))).await;
        let module_restored = restored_module.compute(&empty).await.unwrap();
        assert!(BuildCommandRootKey::equality(&module_v1, &module_restored));

        let package = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//app:all").unwrap()],
            configuration,
        )
        .unwrap();
        let build_epoch = |variant: i64, source: Option<&str>| {
            let mut epoch = BuildRootEpoch::base(variant);
            match source {
                Some(source) => epoch.package("app", source, variant),
                None => epoch.deleted_package("app", variant),
            }
            epoch.build()
        };
        let mut missing_build = build_root_transaction(&dice, build_epoch(45, None)).await;
        assert!(matches!(
            missing_build.compute(&package).await.unwrap(),
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));
        let mut created_build =
            build_root_transaction(&dice, build_epoch(46, Some("filegroup(name = \"v1\")\n")))
                .await;
        let build_v1 = created_build.compute(&package).await.unwrap();
        let mut edited_build =
            build_root_transaction(&dice, build_epoch(47, Some("filegroup(name = \"v2\")\n")))
                .await;
        let build_v2 = edited_build.compute(&package).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&build_v1, &build_v2));
        let mut deleted_build = build_root_transaction(&dice, build_epoch(48, None)).await;
        assert!(matches!(
            deleted_build.compute(&package).await.unwrap(),
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));
        let mut restored_build =
            build_root_transaction(&dice, build_epoch(49, Some("filegroup(name = \"v1\")\n")))
                .await;
        let build_restored = restored_build.compute(&package).await.unwrap();
        assert!(BuildCommandRootKey::equality(&build_v1, &build_restored));
    }

    #[test]
    fn build_branch_collection_has_total_infrastructure_need_and_error_precedence() {
        let first = build_test_error("//first:missing");
        let second = build_test_error("//second:missing");
        let complete_error = |error| {
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                error,
            )))
        };

        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(selected)) =
            collect_build_branches(vec![
                complete_error(first.clone()),
                complete_error(second.clone()),
            ])
            .unwrap()
        else {
            panic!("Complete errors did not remain terminal");
        };
        assert_eq!(selected, first);
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(selected)) =
            collect_build_branches(vec![
                complete_error(second.clone()),
                complete_error(first.clone()),
            ])
            .unwrap()
        else {
            panic!("reversed Complete errors did not remain terminal");
        };
        assert_eq!(selected, second);

        let need_a = build_test_need("/workspace/a");
        let need_b = build_test_need("/workspace/b");
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(combined) =
            collect_build_branches(vec![
                complete_error(first),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need_a)),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need_b)),
            ])
            .unwrap()
        else {
            panic!("reached Needs did not dominate a Complete error");
        };
        let paths = combined.path_observations().unwrap().demands();
        assert_eq!(paths.len(), 2);

        let infrastructure: Arc<str> = Arc::from("cancelled");
        assert_eq!(
            collect_build_branches(vec![
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    build_test_need("/workspace/need"),
                )),
                BuildBranchResult::Infrastructure(infrastructure.clone()),
            ])
            .unwrap_err(),
            infrastructure
        );

        let conflicting_a = slug_bzlmod_v2::SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        );
        let conflicting_b = slug_bzlmod_v2::SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/other").unwrap(),
            },
        );
        assert!(
            collect_build_branches(vec![
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    conflicting_a,
                )),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    conflicting_b,
                )),
            ])
            .is_err()
        );
    }
}
