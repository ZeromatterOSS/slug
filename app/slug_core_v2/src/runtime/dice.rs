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
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use anyhow::Context;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
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
use slug_bzlmod_v2::RootModuleGraph;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::LoadedPackage;
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
use slug_query_v2::QueryPolicy;
use slug_query_v2::evaluate_loading_query_with_policy;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
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
use super::events::AttemptEffectTracker;
use super::events::CommandEffectError;
use super::events::CommandEffectOwner;
use super::events::SealedCommandAttempt;
use super::events::SelectedCommandSidecars;
use super::events::SelectedEventBatches;
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
#[derive(Clone)]
struct AcceptedNativeDemandSnapshot {
    generations: NativeDemandGenerationBundle,
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
    Injection(anyhow::Error),
    Restoration(anyhow::Error),
}

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
    generations: NativeDemandGenerationBundle,
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
                    generations,
                    repository_results: RepositoryMaterializationResultEpoch::new(workspace, [])
                        .expect("empty repository epoch is valid"),
                    path_observations: PathObservationEpoch::empty(),
                    selected: SelectedWorkspaceDemands::empty(),
                },
                #[cfg(test)]
                fail_next_restoration: false,
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
        Ok(data)
    }

    #[allow(dead_code)]
    fn begin_native_demand_command(
        &self,
    ) -> Result<NativeDemandPreflight<'_>, NativeDemandSessionError> {
        let (lease, prior) = self.native_demand_sessions.acquire()?;
        // Busy is decided before any member of this fixed command bundle is
        // allocated.
        let generations = NativeDemandGenerationBundle {
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
                generations,
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
            evaluate_loading_query_with_policy(
                &mut transaction,
                self.workspace.clone(),
                expression,
                order,
                policy,
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
        self.command.generations
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
            self.generations,
            self.repository_results.clone(),
            self.path_observations.clone(),
        )
        .map_err(NativeDemandSessionError::Injection)
    }

    fn progress(
        mut self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<(Self, NativeDemandProgress), NativeDemandSessionError> {
        match self.progress_inner(needs) {
            Ok(progress) => Ok((self, progress)),
            Err(error) => self.restore_after(error),
        }
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
                        self.generations.repository,
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

    fn discard(self) -> Result<(), NativeDemandSessionError> {
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
                prior.generations,
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

    fn accept(
        self,
        selected: NativeDemandTerminalSelection,
    ) -> Result<SelectedEventBatches, NativeDemandSessionError> {
        if !Arc::ptr_eq(&self.effects, &selected.effects) {
            return self.restore_after(NativeDemandSessionError::ForeignEffects);
        }
        let events = selected.sidecars.events().clone();
        self.accept_selected(selected.sidecars.demands().clone())?;
        Ok(events)
    }

    fn accept_selected(
        self,
        selected: SelectedWorkspaceDemands,
    ) -> Result<(), NativeDemandSessionError> {
        match self.selected_snapshot(selected) {
            Ok((snapshot, validation)) => {
                if let Err(error) = self.runtime.runtime.block_on(async {
                    let mut updater = self.runtime.dice.updater_with_data(
                        self.runtime
                            .user_computation_data(None)
                            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                    );
                    inject_native_demand_snapshot(
                        &mut updater,
                        self.runtime,
                        snapshot.generations,
                        snapshot.repository_results.clone(),
                        snapshot.path_observations.clone(),
                    )?;
                    let transaction = updater.commit().await;
                    drop(transaction);
                    Ok::<_, anyhow::Error>(())
                }) {
                    return self.restore_after(NativeDemandSessionError::Injection(error));
                }
                if let Err(error) = self.runtime.repository_materializer.accept(
                    self.repository_session,
                    snapshot.selected.repository_requests(),
                    validation,
                ) {
                    return self.restore_after(NativeDemandSessionError::Repository(error));
                }
                self.runtime
                    .native_demand_sessions
                    .replace_accepted(self.lease, snapshot)?;
                self.runtime.native_demand_sessions.close(self.lease)
            }
            Err(error) => self.restore_after(error),
        }
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
                generations: self.generations,
                repository_results,
                path_observations,
                selected,
            },
            validation,
        ))
    }

    fn restore_after<T>(
        self,
        original: NativeDemandSessionError,
    ) -> Result<T, NativeDemandSessionError> {
        match self.discard() {
            Ok(()) => Err(original),
            Err(error) => Err(error),
        }
    }
}

#[allow(dead_code)]
impl NativeDemandAttempt {
    fn seal_retry(self) -> Result<(), NativeDemandSessionError> {
        self.tracker
            .seal_retry()
            .map_err(NativeDemandSessionError::Effect)
    }

    fn seal_terminal(self) -> Result<NativeDemandSealedAttempt, NativeDemandSessionError> {
        let sealed = self
            .tracker
            .seal_terminal()
            .map_err(NativeDemandSessionError::Effect)?;
        Ok(NativeDemandSealedAttempt {
            effects: self.effects,
            sealed,
        })
    }
}

#[allow(dead_code)]
impl NativeDemandSealedAttempt {
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

#[allow(dead_code)]
fn inject_native_demand_snapshot(
    updater: &mut DiceTransactionUpdater,
    runtime: &WorkspaceRuntime,
    generations: NativeDemandGenerationBundle,
    repository_results: RepositoryMaterializationResultEpoch,
    path_observations: PathObservationEpoch,
) -> anyhow::Result<()> {
    let workspace = NormalizedAbsolutePath::new(runtime.workspace.clone())
        .context("normalizing native-demand workspace")?;
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: runtime.workspace.clone(),
            },
            generations.repository,
        )])
        .context("injecting fixed repository generation")?;
    updater
        .changed_to(vec![(
            RegistryRequestGenerationKey {
                workspace: runtime.workspace.clone(),
            },
            generations.registry,
        )])
        .context("injecting fixed registry generation")?;
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
    use std::thread;

    use slug_events_v2::CaptureEvaluationEvents;
    use slug_workspace_v2::PathObservationKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
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
        assert_eq!(preflight.generations(), preflight.command.generations);
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
        let command = preflight.into_command();

        let first = command.begin_attempt().unwrap();
        let first_need = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data(&first).unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            let first_outcome = transaction.compute(&key).await.unwrap();
            let first_need = match first_outcome {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => need,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) => {
                    panic!("first attempt unexpectedly completed: {value:?}")
                }
            };
            first.seal_retry().unwrap();
            drop(transaction);
            first_need
        });
        assert_eq!(first_need.repository_materializations().len(), 2);
        assert_eq!(
            first_need.path_observations().unwrap().demands(),
            &[path.clone()]
        );
        assert!(command.path_observations.get(&path).is_none());
        let (command, progress) = command.progress(&first_need).unwrap();
        assert_eq!(progress, NativeDemandProgress::Repositories);
        assert!(
            command.path_observations.get(&path).is_none(),
            "repository priority must not observe a simultaneous path Need"
        );
        assert_eq!(command.generations, fixed);

        let second = command.begin_attempt().unwrap();
        let second_need = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data(&second).unwrap());
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
            second.seal_retry().unwrap();
            drop(transaction);
            second_need
        });
        let (command, progress) = command.progress(&second_need).unwrap();
        assert_eq!(progress, NativeDemandProgress::Paths);
        assert_eq!(command.generations, fixed);

        let terminal = command.begin_attempt().unwrap();
        let sidecars = runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data(&terminal).unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            let terminal_outcome = transaction.compute(&key).await.unwrap();
            assert!(matches!(
                terminal_outcome,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref() == "complete"
            ));
            let sealed = terminal.seal_terminal().unwrap();
            let sidecars = sealed.select(&transaction).await.unwrap();
            assert_eq!(
                sidecars.sidecars().demands().repository_requests(),
                &[request.clone()]
            );
            assert_eq!(
                sidecars.sidecars().demands().unscoped_paths(),
                &[path.clone()]
            );
            drop(terminal_outcome);
            drop(transaction);
            sidecars
        });
        let events = command.accept(sidecars).unwrap();
        assert!(events.batches().is_empty());

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
        let accepted = accepted.into_command();
        let probe = accepted.begin_attempt().unwrap();
        runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(accepted.attempt_user_computation_data(&probe).unwrap());
            accepted.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(matches!(
                transaction.compute(&next_key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(_)
            ));
            probe.seal_retry().unwrap();
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
        assert!(matches!(
            accepted.progress(&inherited_with_path),
            Err(NativeDemandSessionError::RepositoryInternalNonProgress)
        ));

        let replacement = local_native_request(&normalized, "dep+", "vendor-next");
        let reopened = runtime.begin_native_demand_command().unwrap();
        assert!(
            reopened
                .path_observations()
                .get(&unprocessed_path)
                .is_none()
        );
        let reopened = reopened.into_command();
        let (reopened, progress) = reopened
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
        assert!(matches!(
            reopened.progress(&conflict_with_path),
            Err(NativeDemandSessionError::ConflictingRepository(repo)) if repo.as_str() == "dep+"
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
        let restored = restored.into_command();
        let (restored, progress) = restored
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                replacement.as_ref().clone(),
            ))
            .unwrap();
        assert_eq!(progress, NativeDemandProgress::Repositories);
        restored.discard().unwrap();

        let repeated_path = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        assert!(matches!(
            repeated_path.progress(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(path.clone()),
            )),
            Err(NativeDemandSessionError::PathInternalNonProgress)
        ));
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command()
            .discard()
            .unwrap();
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

        let initial = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        let (initial, _) = initial
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                accepted_request.as_ref().clone(),
            ))
            .unwrap();
        let (initial, _) = initial
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(path.clone()),
            ))
            .unwrap();
        initial
            .accept_selected(SelectedWorkspaceDemands::for_test(
                Arc::from([accepted_request.clone()]),
                Arc::from([path.clone()]),
            ))
            .unwrap();

        let failing = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        let (failing, _) = failing
            .progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                unselected_request.as_ref().clone(),
            ))
            .unwrap();
        let error = failing
            .accept_selected(SelectedWorkspaceDemands::for_test_with_validation(
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
        let restored = restored.into_command();
        assert!(matches!(
            restored.progress(&slug_bzlmod_v2::SourcePreparationNeeds::repository(
                accepted_request.as_ref().clone()
            )),
            Err(NativeDemandSessionError::RepositoryInternalNonProgress)
        ));
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command()
            .discard()
            .unwrap();
    }

    #[test]
    fn native_demand_restoration_failure_keeps_lease_and_materializer_fail_closed() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let command = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
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
        let command = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        let attempt = command.begin_attempt().unwrap();

        runtime.runtime.block_on(async {
            let mut updater = runtime
                .dice
                .updater_with_data(command.attempt_user_computation_data(&attempt).unwrap());
            command.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(transaction.compute(&NativeTerminalProbeKey).await.unwrap());
            let sealed = attempt.seal_terminal().unwrap();

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
        reopened.into_command().discard().unwrap();
    }

    #[test]
    fn native_demand_accept_rejects_foreign_command_sidecars_and_restores() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let foreign_runtime = WorkspaceRuntime::new(workspace.path()).unwrap();
        let foreign = foreign_runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        let attempt = foreign.begin_attempt().unwrap();
        let foreign_selection = foreign_runtime.runtime.block_on(async {
            let mut updater = foreign_runtime
                .dice
                .updater_with_data(foreign.attempt_user_computation_data(&attempt).unwrap());
            foreign.inject_attempt(&mut updater).unwrap();
            let mut transaction = updater.commit().await;
            assert!(transaction.compute(&NativeTerminalProbeKey).await.unwrap());
            let sealed = attempt.seal_terminal().unwrap();
            let selected = sealed.select(&transaction).await.unwrap();
            drop(transaction);
            selected
        });

        let command = runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command();
        assert!(matches!(
            command.accept(foreign_selection),
            Err(NativeDemandSessionError::ForeignEffects)
        ));
        runtime
            .begin_native_demand_command()
            .unwrap()
            .into_command()
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
}
