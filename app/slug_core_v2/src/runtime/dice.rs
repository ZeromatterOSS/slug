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
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::RootConfiguredTargetAnalysisKey;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
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
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetName;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::RootPackageLoadError;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::bzl_load_cycle_detector;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_query_v2::QueryError;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryOutput;
use slug_query_v2::QueryOutputCompletion;
use slug_query_v2::QueryPolicy;
use slug_query_v2::RootQueryCommandKey;
use slug_query_v2::evaluate_loading_query_with_policy_and_output_completion;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::WorkspaceFileKey;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::small_map::SmallMap;

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
    collect_workspace_observations(&workspace, &mut observation);
    Ok(observation)
}

/// The legacy focused-test adapter. Production callers should use
/// 'observe_workspace' so direct directory observations travel with files.
pub fn observe_workspace_files(workspace: &Path) -> anyhow::Result<Vec<WorkspaceFileObservation>> {
    Ok(observe_workspace(workspace)?.files)
}

fn collect_workspace_observations(directory: &Path, observation: &mut WorkspaceObservation) {
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
        collect_workspace_observations(&child, observation);
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
    pub analysis: Option<AnalysisResult>,
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
#[allow(dead_code)] // Activated by the later shared command driver.
struct BuildCommandRootKey {
    workspace: NormalizedAbsolutePath,
    targets: Arc<[Arc<str>]>,
    configuration: ConfigurationKey,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
#[allow(dead_code)] // Constructor diagnostics remain private until activation.
enum BuildCommandRequestError {
    ExternalRepository { pattern: Arc<str> },
    RecursivePattern { pattern: Arc<str> },
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct BuildCommandEvaluation {
    anchor: RootModuleLoadingAnchor,
    targets: Arc<[BuildRequestedTarget]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
#[allow(dead_code)]
struct BuildRequestedTarget {
    pattern: Arc<str>,
    package: LoadedPackage,
    analysis: Option<AnalysisResult>,
}

#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct BuildCommandError {
    kind: BuildCommandErrorKind,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum BuildCommandErrorKind {
    RootAnchor(RootModuleLoadingAnchorError),
    Package {
        pattern: Arc<str>,
        error: RootPackageLoadError,
    },
    TargetNotFound {
        pattern: Arc<str>,
        package: PackagePath,
        target: TargetName,
        build_file: PathBuf,
    },
    Analysis {
        pattern: Arc<str>,
        error: AnalysisError,
    },
    ExternalRepository {
        pattern: Arc<str>,
    },
    RecursivePattern {
        pattern: Arc<str>,
    },
    Infrastructure(Arc<str>),
}

#[allow(dead_code)]
type BuildCommandOutcome = slug_bzlmod_v2::SourcePreparationOutcome<
    Arc<Result<BuildCommandEvaluation, BuildCommandError>>,
>;

#[allow(dead_code)]
enum BuildBranchResult {
    Outcome(
        slug_bzlmod_v2::SourcePreparationOutcome<Result<BuildRequestedTarget, BuildCommandError>>,
    ),
    Infrastructure(Arc<str>),
}

impl BuildCommandRootKey {
    #[allow(dead_code)]
    fn new(
        workspace: NormalizedAbsolutePath,
        targets: &[TargetPattern],
        configuration: ConfigurationKey,
    ) -> Result<Self, BuildCommandRequestError> {
        let mut canonical = Vec::with_capacity(targets.len());
        for target in targets {
            let pattern: Arc<str> = Arc::from(target.to_string());
            let repo = match target {
                TargetPattern::Single(label) => label.repo(),
                TargetPattern::PackageAll { repo, .. } | TargetPattern::Recursive { repo, .. } => {
                    repo
                }
            };
            if !repo.is_root() {
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
            configuration,
        })
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

    pub fn analyses(&self) -> impl Iterator<Item = &AnalysisResult> {
        self.targets
            .iter()
            .filter_map(|target| target.analysis.as_ref())
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
    fn root_anchor(error: RootModuleLoadingAnchorError) -> Self {
        Self {
            kind: BuildCommandErrorKind::RootAnchor(error),
        }
    }

    fn package(pattern: Arc<str>, error: RootPackageLoadError) -> Self {
        Self {
            kind: BuildCommandErrorKind::Package { pattern, error },
        }
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

    fn analysis(pattern: Arc<str>, error: AnalysisError) -> Self {
        Self {
            kind: BuildCommandErrorKind::Analysis { pattern, error },
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
}

impl fmt::Display for BuildCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            BuildCommandErrorKind::RootAnchor(error) => error.fmt(f),
            BuildCommandErrorKind::Package { error, .. } => error.fmt(f),
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
            BuildCommandErrorKind::Analysis { error, .. } => error.fmt(f),
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

impl std::error::Error for BuildCommandError {}

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

#[allow(dead_code)]
fn build_complete(
    result: Result<BuildCommandEvaluation, BuildCommandError>,
) -> BuildCommandOutcome {
    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Arc::new(result))
}

#[allow(dead_code)]
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

#[allow(dead_code)]
async fn compute_build_branch(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    pattern: Arc<str>,
    configuration: ConfigurationKey,
) -> BuildBranchResult {
    let parsed = TargetPattern::parse(&pattern)
        .expect("BuildCommandRootKey stores validated canonical target patterns");
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
                        BuildCommandError::package(pattern, error.clone()),
                    )),
                );
            }
        },
    };
    let analysis = match parsed {
        TargetPattern::PackageAll { .. } => None,
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
                slug_loading_v2::PackageTargetKind::StarlarkRule(_)
            ) {
                let canonical =
                    CanonicalLabel::parse(&format!("@@//{}:{}", label.package(), label.target()))
                        .expect("validated root apparent label has a canonical projection");
                let configured_target = ConfiguredTargetKey::new(canonical, configuration);
                match ctx
                    .compute(&RootConfiguredTargetAnalysisKey::new(
                        workspace,
                        configured_target,
                    ))
                    .await
                {
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
                            Ok(analysis) => Some(analysis.clone()),
                            Err(error) => {
                                return BuildBranchResult::Outcome(
                                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                                        BuildCommandError::analysis(pattern, error.clone()),
                                    )),
                                );
                            }
                        }
                    }
                }
            } else {
                None
            }
        }
        TargetPattern::Recursive { .. } => unreachable!(),
    };
    BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(
        BuildRequestedTarget {
            pattern,
            package: package_value,
            analysis,
        },
    )))
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
                    return build_complete(Err(BuildCommandError::root_anchor(error.clone())));
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
                build_complete(Ok(BuildCommandEvaluation { anchor, targets }))
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
    pub fn new(workspace: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let workspace = workspace.into();
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
        let normalized_workspace = NormalizedAbsolutePath::new(workspace.clone())
            .context("normalizing retained workspace")?;
        let repository_materializer = Arc::new(super::repository_io::RepositoryMaterializer::new(
            normalized_workspace.clone(),
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
                        let sealed = guard.seal_terminal()?;
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
    ) -> Result<
        AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
        BuildCommandError,
    > {
        let registry_urls = RegistryUrls::from_request(&self.workspace, registry_urls)
            .map_err(BuildCommandError::infrastructure)?;
        let configuration =
            ConfigurationKey::target("first-build").map_err(BuildCommandError::infrastructure)?;
        let root = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new(self.workspace.clone())
                .map_err(BuildCommandError::infrastructure)?,
            targets,
            configuration,
        )
        .map_err(BuildCommandError::request)?;
        let request = NativeDemandRequestInputBundle {
            command_policy,
            environment_policy,
            lockfile_mode,
            registry_urls,
        };
        self.drive_command(request, root)
            .map(|result| result.accepted)
            .map_err(|error| {
                BuildCommandError::infrastructure(format!("typed build command failed: {error}"))
            })
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
                            let configured_target = ConfiguredTargetKey::new(
                                canonical,
                                ConfigurationKey::target("first-build")
                                    .map_err(anyhow::Error::msg)?,
                            );
                            let value = transaction
                                .compute(&ConfiguredTargetAnalysisKey {
                                    workspace: self.workspace.clone(),
                                    configured_target,
                                })
                                .await
                                .context("computing configured-target analysis through DICE")?;
                            Some(
                                value
                                    .as_ref()
                                    .as_ref()
                                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
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
        let sealed = self
            .attempt
            .as_ref()
            .expect("native-demand terminal attempt is live")
            .seal_terminal()?;
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
    let runtime = WorkspaceRuntime::new(&workspace)?;
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
    let runtime = WorkspaceRuntime::new(&workspace)?;
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
    use crate::runtime::events::CommandEffectOwner;

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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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

        let missing_runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
            "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\", srcs = [\"target.txt\", \"missing_input.txt\"])\nalias(name = \"files_alias\", actual = \":files\")\nconfig_setting(name = \"is_k8\", values = {\"cpu\": \"k8\"})\ntest_suite(name = \"suite_omitted\")\ntest_suite(name = \"suite_empty\", tests = [], tags = [\"manual\", \"a\"])\ntest_suite(name = \"suite_parent\", tests = [\":suite_empty\"])\ntest_suite(name = \"suite_cycle_a\", tests = [\":suite_cycle_b\"])\ntest_suite(name = \"suite_cycle_b\", tests = [\":suite_cycle_a\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("dep/target.txt"), "target").unwrap();

        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = WorkspaceRuntime::new(workspace.path())
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
                "package_group(name = \"unsupported\", packages = [\"//...\"])\n",
                "external repository rule graph is deferred",
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
            let stopped = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let stopped = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "print(\"BZL_EVENT\")\ndef _impl(ctx):\n    print(\"ANALYSIS_EVENT\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprint(\"BUILD_EVENT\")\nprobe(name = \"probe\")\n",
        )
        .unwrap();
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let target = TargetPattern::parse("//pkg:probe").unwrap();
        let build = |runtime: &WorkspaceRuntime, targets: &[TargetPattern]| {
            runtime.build_command_with_bzlmod_inputs(
                targets,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
            )
        };

        let accepted = build(&runtime, std::slice::from_ref(&target)).unwrap();
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(evaluation.loaded_package_count(), 1);
        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 0);
        assert_eq!(
            accepted_output_text(&accepted),
            ["MODULE_EVENT", "BZL_EVENT", "BUILD_EVENT", "ANALYSIS_EVENT"]
        );

        let warm = build(&runtime, std::slice::from_ref(&target)).unwrap();
        assert!(warm.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&warm).is_empty());

        let empty = build(&runtime, &[]).unwrap();
        let evaluation = empty.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(evaluation.loaded_package_count(), 0);
        assert_eq!(evaluation.analyzed_target_count(), 0);

        let missing_runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let missing_target = TargetPattern::parse("//pkg:missing").unwrap();
        let missing = build(&missing_runtime, &[missing_target]).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();

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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
            let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let first = WorkspaceRuntime::new(workspace.path()).unwrap();
        let second = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();

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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let foreign_runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let foreign_runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();

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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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

        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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
        let runtime = WorkspaceRuntime::new(&root).unwrap();
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

    fn build_test_configuration(value: &str) -> ConfigurationKey {
        ConfigurationKey::target(value).unwrap()
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
                || key.downcast_ref::<ConfiguredTargetAnalysisKey>().is_some()
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
                build_test_configuration("other"),
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
        assert!(matches!(
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse("@repo//pkg:t").unwrap()],
                configuration.clone(),
            ),
            Err(BuildCommandRequestError::ExternalRepository { pattern })
                if pattern.as_ref() == "@repo//pkg:t"
        ));
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
                        kind: BuildCommandErrorKind::Package { .. },
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
                        kind: BuildCommandErrorKind::Package { .. },
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
                        kind: BuildCommandErrorKind::Package { .. },
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
