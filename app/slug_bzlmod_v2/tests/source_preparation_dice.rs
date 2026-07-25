use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleSourcePreparation;
use slug_bzlmod_v2::ModuleSourcePreparationError;
use slug_bzlmod_v2::ModuleSourcePreparationKey;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RegistryFileError;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryIo;
use slug_bzlmod_v2::RepositoryIoOutcome;
use slug_bzlmod_v2::RepositoryMaterialization;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationError;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationGenerationKey;
use slug_bzlmod_v2::RepositoryMaterializationKey;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochError;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RepositorySourceFileError;
use slug_bzlmod_v2::RepositorySourceFileKey;
use slug_bzlmod_v2::RepositorySourceFileValue;
use slug_bzlmod_v2::RepositoryTransportError;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationNeedsError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::apply_unified_patch;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::install_registry_io;
use slug_bzlmod_v2::install_repository_io;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
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
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::sorted_map::SortedMap;

fn workspace() -> PathBuf {
    PathBuf::from("/source-preparation-dice-test")
}

struct LocalIo {
    calls: AtomicUsize,
}

struct FlakyIo {
    calls: AtomicUsize,
}

struct ImmutableIo {
    calls: AtomicUsize,
    root: tempfile::TempDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
struct SemanticMaterializationInputKey;

impl fmt::Display for SemanticMaterializationInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("semantic-materialization-input")
    }
}

impl InjectedKey for SemanticMaterializationInputKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>,
    >;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        <RepositoryMaterializationKey as Key>::equality(x, y)
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct MaterializationCounterKey {
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct RepositoryMaterializationCounterKey {
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

impl PartialEq for RepositoryMaterializationCounterKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.counter, &other.counter)
    }
}

impl Eq for RepositoryMaterializationCounterKey {}

impl Hash for RepositoryMaterializationCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.counter).hash(state);
    }
}

impl fmt::Display for RepositoryMaterializationCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-counter:{:p}",
            Arc::as_ptr(&self.counter)
        )
    }
}

#[async_trait]
impl Key for RepositoryMaterializationCounterKey {
    type Value = usize;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let outcome = ctx
            .compute(&RepositoryMaterializationKey {
                workspace: workspace(),
                module_name: "dep".into(),
            })
            .await
            .expect("fixed repository materialization must compute");
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()
        ));
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

impl PartialEq for MaterializationCounterKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.counter, &other.counter)
    }
}

impl Eq for MaterializationCounterKey {}

impl Hash for MaterializationCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.counter).hash(state);
    }
}

impl fmt::Display for MaterializationCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "semantic-materialization-counter:{:p}",
            Arc::as_ptr(&self.counter)
        )
    }
}

#[async_trait]
impl Key for MaterializationCounterKey {
    type Value = usize;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&SemanticMaterializationInputKey)
            .await
            .expect("injected semantic materialization must compute");
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct PreparationCounterKey {
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

impl PartialEq for PreparationCounterKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.counter, &other.counter)
    }
}

impl Eq for PreparationCounterKey {}

impl Hash for PreparationCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.counter).hash(state);
    }
}

impl fmt::Display for PreparationCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "module-source-preparation-counter:{:p}",
            Arc::as_ptr(&self.counter)
        )
    }
}

#[async_trait]
impl Key for PreparationCounterKey {
    type Value = PathOutcome<usize>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        source_outcome_to_path(
            ctx.compute(&ModuleSourcePreparationKey {
                workspace: workspace(),
                module_name: "dep".into(),
                version: "1.0.0".into(),
            })
            .await
            .expect("fixed module-source preparation must compute")
            .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1),
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct RootObservationCounterKey {
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
    namespace: PathObservationNamespace,
}

impl PartialEq for RootObservationCounterKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.counter, &other.counter) && self.namespace == other.namespace
    }
}

impl Eq for RootObservationCounterKey {}

impl Hash for RootObservationCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.counter).hash(state);
        self.namespace.hash(state);
    }
}

impl fmt::Display for RootObservationCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-observation-counter:{:?}:{:p}",
            self.namespace,
            Arc::as_ptr(&self.counter),
        )
    }
}

#[async_trait]
impl Key for RootObservationCounterKey {
    type Value = PathOutcome<usize>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&PathObservationKey::new(demand_in(
            self.namespace,
            "/",
            PathObservationOperation::Lstat,
        )))
        .await
        .expect("fixed root observation must compute")
        .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Clone)]
enum FakeRegistryResponse {
    Found(Arc<[u8]>),
    NotFound,
    Error(&'static str),
}

struct FakeRegistryIo {
    responses: Mutex<std::collections::BTreeMap<String, FakeRegistryResponse>>,
    calls: Mutex<Vec<String>>,
}

impl FakeRegistryIo {
    fn new(responses: impl IntoIterator<Item = (impl Into<String>, FakeRegistryResponse)>) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(url, response)| (url.into(), response))
                    .collect(),
            ),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl RegistryIo for FakeRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        self.calls.lock().unwrap().push(url.as_str().to_owned());
        match self.responses.lock().unwrap().get(url.as_str()).cloned() {
            Some(FakeRegistryResponse::Found(bytes)) => Ok(RegistryIoOutcome::Found(bytes)),
            Some(FakeRegistryResponse::NotFound) | None => Ok(RegistryIoOutcome::NotFound),
            Some(FakeRegistryResponse::Error(message)) => Err(RegistryTransportError {
                message: message.into(),
            }),
        }
    }
}

#[async_trait]
impl RepositoryIo for FlakyIo {
    async fn materialize(
        &self,
        workspace: &Path,
        _: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(RepositoryTransportError {
                message: "temporary source failure".into(),
            });
        }
        Ok(RepositoryIoOutcome::Local {
            source_root: workspace.join("vendor/dep"),
        })
    }
}

#[async_trait]
impl RepositoryIo for LocalIo {
    async fn materialize(
        &self,
        workspace: &Path,
        _: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(RepositoryIoOutcome::Local {
            source_root: workspace.join("vendor/dep"),
        })
    }
}

#[async_trait]
impl RepositoryIo for ImmutableIo {
    async fn materialize(
        &self,
        _: &Path,
        _: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(RepositoryIoOutcome::Immutable {
            source_identity: Arc::from("retained-immutable-source"),
            generation_root: self.root.path().to_owned(),
            observation_instance: PathObservationInstanceId::new(1),
        })
    }
}

fn text_snapshot() -> Arc<WorkspaceSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(SortedMap::from_iter([(
            workspace.join("MODULE.bazel"),
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n"
                    .to_owned(),
            )),
        )])),
    })
}

fn text_snapshot_with(source: &str) -> Arc<WorkspaceSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(SortedMap::from_iter([(
            workspace.join("MODULE.bazel"),
            WorkspaceFileValue::Present(Arc::new(source.to_owned())),
        )])),
    })
}

fn raw_workspace_snapshot(
    values: impl IntoIterator<Item = (&'static str, WorkspaceRawFileValue)>,
) -> Arc<WorkspaceRawSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(SortedMap::from_iter(
            values
                .into_iter()
                .map(|(path, value)| (workspace.join(path), value)),
        )),
    })
}

fn raw_snapshot(
    values: impl IntoIterator<Item = (&'static str, WorkspaceRawFileValue)>,
) -> Arc<WorkspaceRawSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(SortedMap::from_iter(
            values
                .into_iter()
                .map(|(path, value)| (workspace.join("vendor/dep").join(path), value)),
        )),
    })
}

async fn source_with_epoch(
    dice: &Arc<Dice>,
    raw: Arc<WorkspaceRawSnapshot>,
    epoch: PathObservationEpoch,
    generation: u64,
    repo_relative_path: &str,
) -> PathResult<RepositorySourceFileValue, RepositorySourceFileError> {
    let workspace = workspace();
    let immutable = epoch
        .observations()
        .keys()
        .find_map(|demand| match demand.namespace() {
            PathObservationNamespace::Materialization(instance)
                if demand.path().as_path().file_name().is_some_and(|name| {
                    name == "MODULE.bazel" || name == ".materialization-root"
                }) =>
            {
                Some((
                    demand
                        .path()
                        .as_path()
                        .parent()
                        .unwrap_or(Path::new("/"))
                        .to_owned(),
                    instance,
                ))
            }
            PathObservationNamespace::Materialization(_) => None,
            PathObservationNamespace::Host => None,
        });
    let text = immutable
        .as_ref()
        .map(|_| immutable_text_snapshot())
        .unwrap_or_else(text_snapshot);
    let materialization_epoch = match immutable {
        Some((root, instance)) => immutable_materialization_epoch_input(&workspace, root, instance),
        None => local_materialization_epoch_input(&workspace),
    };
    source_with_result_epoch(
        dice,
        text,
        raw,
        epoch,
        materialization_epoch,
        generation,
        repo_relative_path,
    )
    .await
}

async fn source_with_result_epoch(
    dice: &Arc<Dice>,
    text: Arc<WorkspaceSnapshot>,
    raw: Arc<WorkspaceRawSnapshot>,
    epoch: PathObservationEpoch,
    materialization_epoch: (
        RepositoryMaterializationResultEpochKey,
        RepositoryMaterializationResultEpoch,
    ),
    generation: u64,
    repo_relative_path: &str,
) -> PathResult<RepositorySourceFileValue, RepositorySourceFileError> {
    let workspace = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            (WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            }),
            text,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            (WorkspaceRawSnapshotKey {
                workspace: workspace.clone(),
            }),
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater.changed_to(vec![materialization_epoch]).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater
        .changed_to(vec![(
            (RepositoryMaterializationGenerationKey {
                workspace: workspace.clone(),
            }),
            RepositoryMaterializationGeneration(generation),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let outcome = transaction
        .compute(&RepositorySourceFileKey {
            workspace,
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from(repo_relative_path),
        })
        .await
        .unwrap();
    source_outcome_to_path(outcome)
}

fn immutable_text_snapshot() -> Arc<WorkspaceSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(SortedMap::from_iter([(
            workspace.join("MODULE.bazel"),
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root')\narchive_override(module_name = 'dep')\n".to_owned(),
            )),
        )])),
    })
}

fn immutable_materialization_epoch_input(
    workspace: &Path,
    root: PathBuf,
    instance: PathObservationInstanceId,
) -> (
    RepositoryMaterializationResultEpochKey,
    RepositoryMaterializationResultEpoch,
) {
    let workspace = NormalizedAbsolutePath::new(workspace).unwrap();
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace.dupe(),
            canonical_repo: slug_identity_v2::CanonicalRepoName::new("dep+").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
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
    (
        RepositoryMaterializationResultEpochKey {
            workspace: workspace.dupe(),
        },
        RepositoryMaterializationResultEpoch::new(
            workspace,
            [RepositoryMaterializationEpochEntry {
                request,
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        source_identity: Arc::from("immutable-test-source"),
                        generation_root: root,
                        observation_instance: instance,
                    },
                ),
            }],
        )
        .unwrap(),
    )
}

fn source_outcome_to_path<T>(outcome: SourcePreparationOutcome<T>) -> PathOutcome<T> {
    match outcome {
        SourcePreparationOutcome::Complete(value) => PathOutcome::Complete(value),
        SourcePreparationOutcome::Need(need) => {
            assert!(need.repository_materializations().is_empty());
            PathOutcome::Need(
                need.path_observations()
                    .expect("path-only test need")
                    .dupe(),
            )
        }
    }
}

fn local_materialization_epoch_input(
    workspace: &Path,
) -> (
    RepositoryMaterializationResultEpochKey,
    RepositoryMaterializationResultEpoch,
) {
    let workspace = NormalizedAbsolutePath::new(workspace).unwrap();
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace.dupe(),
            canonical_repo: slug_identity_v2::CanonicalRepoName::new("dep+").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse(
                    "@@bazel_tools//tools/build_defs/repo:local.bzl",
                )
                .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(starlark_map::small_map::SmallMap::from_iter([(
                compact_str::CompactString::new("path"),
                OverrideAttributeValue::String("vendor/dep".into()),
            )])),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new(workspace.as_path().join("vendor/dep"))
                .unwrap(),
        },
    });
    (
        RepositoryMaterializationResultEpochKey {
            workspace: workspace.dupe(),
        },
        RepositoryMaterializationResultEpoch::new(
            workspace,
            [RepositoryMaterializationEpochEntry {
                request,
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap(),
    )
}

fn local_request_for(
    workspace: &Path,
    module_name: &str,
    relative_path: &str,
) -> Arc<RepositoryMaterializationRequest> {
    let workspace = NormalizedAbsolutePath::new(workspace).unwrap();
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace.dupe(),
            canonical_repo: slug_identity_v2::CanonicalRepoName::new(format!("{module_name}+"))
                .unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse(
                    "@@bazel_tools//tools/build_defs/repo:local.bzl",
                )
                .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(starlark_map::small_map::SmallMap::from_iter([(
                compact_str::CompactString::new("path"),
                OverrideAttributeValue::String(relative_path.into()),
            )])),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new(workspace.as_path().join(relative_path))
                .unwrap(),
        },
    })
}

fn materialization_epoch_input(
    workspace: &Path,
    entries: impl IntoIterator<Item = RepositoryMaterializationEpochEntry>,
) -> (
    RepositoryMaterializationResultEpochKey,
    RepositoryMaterializationResultEpoch,
) {
    let workspace = NormalizedAbsolutePath::new(workspace).unwrap();
    (
        RepositoryMaterializationResultEpochKey {
            workspace: workspace.dupe(),
        },
        RepositoryMaterializationResultEpoch::new(workspace, entries).unwrap(),
    )
}

async fn materialization_with_epoch(
    dice: &Arc<Dice>,
    root_source: &str,
    epoch: (
        RepositoryMaterializationResultEpochKey,
        RepositoryMaterializationResultEpoch,
    ),
    generation: u64,
    module_name: &str,
) -> SourcePreparationOutcome<Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>>
{
    let workspace = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            text_snapshot_with(root_source),
        )])
        .unwrap();
    updater.changed_to(vec![epoch]).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationGeneration(generation),
        )])
        .unwrap();
    updater
        .commit()
        .await
        .compute(&RepositoryMaterializationKey {
            workspace,
            module_name: module_name.into(),
        })
        .await
        .unwrap()
}

async fn count_repository_materialization_with_epoch(
    dice: &Arc<Dice>,
    root_source: &str,
    epoch: (
        RepositoryMaterializationResultEpochKey,
        RepositoryMaterializationResultEpoch,
    ),
    generation: u64,
    counter_key: &RepositoryMaterializationCounterKey,
) -> usize {
    let workspace = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            text_snapshot_with(root_source),
        )])
        .unwrap();
    updater.changed_to(vec![epoch]).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationGeneration(generation),
        )])
        .unwrap();
    updater.commit().await.compute(counter_key).await.unwrap()
}

async fn source(
    dice: &Arc<Dice>,
    raw: Arc<WorkspaceRawSnapshot>,
    generation: u64,
    repo_relative_path: &str,
) -> Result<RepositorySourceFileValue, RepositorySourceFileError> {
    let epoch = complete_epoch_for_raw(&raw);
    let outcome = source_with_epoch(dice, raw, epoch, generation, repo_relative_path).await;
    let PathOutcome::Complete(value) = outcome else {
        panic!("complete raw snapshot unexpectedly needs path observations");
    };
    value
}

async fn prepare(
    dice: &Arc<Dice>,
    root_source: &str,
    raw: Arc<WorkspaceRawSnapshot>,
    registries: &[&str],
    generation: u64,
    version: &str,
) -> Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>> {
    let outcome = prepare_with_epoch(
        dice,
        root_source,
        raw.clone(),
        complete_epoch_for_raw(&raw),
        registries,
        generation,
        version,
    )
    .await;
    let PathOutcome::Complete(value) = outcome else {
        panic!("complete raw snapshot unexpectedly needs path observations");
    };
    value
}

async fn prepare_with_epoch(
    dice: &Arc<Dice>,
    root_source: &str,
    raw: Arc<WorkspaceRawSnapshot>,
    epoch: PathObservationEpoch,
    registries: &[&str],
    generation: u64,
    version: &str,
) -> PathOutcome<Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>> {
    let workspace = workspace();
    let immutable = epoch
        .observations()
        .keys()
        .find_map(|demand| match demand.namespace() {
            PathObservationNamespace::Materialization(instance)
                if demand.path().as_path().file_name().is_some_and(|name| {
                    name == "MODULE.bazel" || name == ".materialization-root"
                }) =>
            {
                Some((
                    demand
                        .path()
                        .as_path()
                        .parent()
                        .unwrap_or(Path::new("/"))
                        .to_owned(),
                    instance,
                ))
            }
            PathObservationNamespace::Materialization(_) | PathObservationNamespace::Host => None,
        });
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            immutable
                .as_ref()
                .map(|_| immutable_text_snapshot())
                .unwrap_or_else(|| text_snapshot_with(root_source)),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.clone(),
            },
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater
        .changed_to(vec![match immutable {
            Some((root, instance)) => {
                immutable_materialization_epoch_input(&workspace, root, instance)
            }
            None => local_materialization_epoch_input(&workspace),
        }])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    inject_registry_request_inputs(
        &mut updater,
        &workspace,
        RegistryUrls::new(registries.iter().copied()),
        RegistryRequestGeneration(generation),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationGeneration(generation),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    source_outcome_to_path(
        transaction
            .compute(&ModuleSourcePreparationKey {
                workspace,
                module_name: "dep".into(),
                version: version.into(),
            })
            .await
            .unwrap(),
    )
}

async fn count_preparation_with_epoch(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    generation: u64,
    counter_key: &PreparationCounterKey,
) -> PathOutcome<usize> {
    let workspace = workspace();
    let immutable = epoch
        .observations()
        .keys()
        .find_map(|demand| match demand.namespace() {
            PathObservationNamespace::Materialization(instance)
                if demand
                    .path()
                    .as_path()
                    .file_name()
                    .is_some_and(|name| name == "MODULE.bazel") =>
            {
                Some((
                    demand.path().as_path().parent().unwrap().to_owned(),
                    instance,
                ))
            }
            _ => None,
        });
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            immutable
                .as_ref()
                .map(|_| immutable_text_snapshot())
                .unwrap_or_else(text_snapshot),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.clone(),
            },
            raw_snapshot([]),
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater
        .changed_to(vec![match immutable {
            Some((root, instance)) => {
                immutable_materialization_epoch_input(&workspace, root, instance)
            }
            None => local_materialization_epoch_input(&workspace),
        }])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    inject_registry_request_inputs(
        &mut updater,
        &workspace,
        RegistryUrls::new(std::iter::empty::<&str>()),
        RegistryRequestGeneration(generation),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            RepositoryMaterializationGenerationKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationGeneration(generation),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction.compute(counter_key).await.unwrap()
}

async fn count_root_observation_with_epoch(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    counter_key: &RootObservationCounterKey,
) -> PathOutcome<usize> {
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction.compute(counter_key).await.unwrap()
}

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn demand_in(
    namespace: PathObservationNamespace,
    value: &str,
    operation: PathObservationOperation,
) -> PathObservationDemand {
    PathObservationDemand::new(namespace, path(value), operation)
}

fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
    demand_in(PathObservationNamespace::Host, value, operation)
}

fn lstat(kind: PathNodeKind) -> PathLstat {
    PathLstat::new(kind, 1, 2, 3, 4, 0o644)
}

fn lstat_variant(kind: PathNodeKind, value: i64) -> PathLstat {
    PathLstat::new(kind, value, value + 1, value + 2, value + 3, 0o600)
}

fn complete_epoch_for_raw(raw: &WorkspaceRawSnapshot) -> PathObservationEpoch {
    let workspace = workspace();
    let mut observations = std::collections::BTreeMap::new();
    let directory =
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Directory)));
    observations.insert(
        demand("/", PathObservationOperation::Lstat),
        directory.clone(),
    );
    observations.insert(
        demand(workspace.to_str().unwrap(), PathObservationOperation::Lstat),
        directory.clone(),
    );
    for (file, value) in raw.files.iter() {
        let mut ancestor = file.parent();
        while let Some(path) = ancestor {
            observations
                .entry(demand(
                    path.to_str().unwrap(),
                    PathObservationOperation::Lstat,
                ))
                .or_insert_with(|| directory.clone());
            ancestor = path.parent();
        }
        let file = file.to_str().unwrap();
        let lstat_result = match value {
            WorkspaceRawFileValue::Present(_) => {
                PathOperationResult::Present(lstat(PathNodeKind::RegularFile))
            }
            WorkspaceRawFileValue::Absent => PathOperationResult::Missing,
            WorkspaceRawFileValue::ReadError(_) => {
                PathOperationResult::Error(PathObservationError::Io {
                    kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                    raw_os_error: Some(13),
                })
            }
        };
        observations.insert(
            demand(file, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(lstat_result),
        );
        if let WorkspaceRawFileValue::Present(bytes) = value {
            observations.insert(
                demand(file, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes.dupe())),
            );
        }
    }
    PathObservationEpoch::new(observations).unwrap()
}

fn lstat_observation(
    path: &str,
    result: PathOperationResult<PathLstat>,
) -> (PathObservationDemand, PathObservationResult) {
    lstat_observation_in(PathObservationNamespace::Host, path, result)
}

fn lstat_observation_in(
    namespace: PathObservationNamespace,
    path: &str,
    result: PathOperationResult<PathLstat>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand_in(namespace, path, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(result),
    )
}

fn file_bytes_observation(
    path: &str,
    result: PathOperationResult<Arc<[u8]>>,
) -> (PathObservationDemand, PathObservationResult) {
    file_bytes_observation_in(PathObservationNamespace::Host, path, result)
}

fn file_bytes_observation_in(
    namespace: PathObservationNamespace,
    path: &str,
    result: PathOperationResult<Arc<[u8]>>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand_in(namespace, path, PathObservationOperation::FileBytes),
        PathObservationResult::FileBytes(result),
    )
}

fn read_link_observation(
    path: &str,
    result: PathOperationResult<Arc<PathBuf>>,
) -> (PathObservationDemand, PathObservationResult) {
    read_link_observation_in(PathObservationNamespace::Host, path, result)
}

fn read_link_observation_in(
    namespace: PathObservationNamespace,
    path: &str,
    result: PathOperationResult<Arc<PathBuf>>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand_in(namespace, path, PathObservationOperation::ReadLink),
        PathObservationResult::ReadLink(result),
    )
}

fn root_patch_source(patches: &str) -> String {
    format!(
        "module(name = 'root')\n\
         bazel_dep(name = 'dep', version = '1.0.0')\n\
         single_version_override(module_name = 'dep', patches = [{patches}], patch_strip = 1)\n"
    )
}

fn local_source_observations(
    terminal: PathObservationResult,
) -> Vec<(PathObservationDemand, PathObservationResult)> {
    let workspace = workspace();
    let root = workspace.join("vendor/dep");
    vec![
        lstat_observation(
            "/",
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.join("vendor").to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            root.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        (
            demand(
                root.join("MODULE.bazel").to_str().unwrap(),
                PathObservationOperation::Lstat,
            ),
            terminal,
        ),
    ]
}

fn immutable_source_observations(
    root: &Path,
    instance: PathObservationInstanceId,
    terminal: PathObservationResult,
) -> Vec<(PathObservationDemand, PathObservationResult)> {
    let namespace = PathObservationNamespace::Materialization(instance);
    let directory = PathOperationResult::Present(lstat(PathNodeKind::Directory));
    let mut ancestors = root.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    let mut observations = ancestors
        .into_iter()
        .map(|ancestor| {
            lstat_observation_in(namespace, ancestor.to_str().unwrap(), directory.clone())
        })
        .collect::<Vec<_>>();
    observations.push((
        demand_in(
            namespace,
            root.join("MODULE.bazel").to_str().unwrap(),
            PathObservationOperation::Lstat,
        ),
        terminal,
    ));
    observations
}

fn semantic_materialization(
    root: &str,
    instance: u64,
    source_identity: &str,
    canonical_repo: &str,
    rule_name: &str,
) -> SourcePreparationOutcome<Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>>
{
    SourcePreparationOutcome::Complete(Arc::new(Ok(RepositoryMaterialization::Immutable {
        canonical_repo: slug_identity_v2::CanonicalRepoName::new(canonical_repo).unwrap(),
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse(
                    "@@bazel_tools//tools/build_defs/repo:http.bzl",
                )
                .unwrap(),
                rule_name: rule_name.into(),
            },
            attributes: Arc::default(),
        },
        source_identity: Arc::from(source_identity),
        generation_root: PathBuf::from(root),
        observation_instance: PathObservationInstanceId::new(instance),
    })))
}

fn materialization_request(
    workspace: &str,
    canonical_repo: &str,
    rule_name: &str,
    kind: RepositoryMaterializationKind,
) -> RepositoryMaterializationRequest {
    RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new(workspace).unwrap(),
            canonical_repo: slug_identity_v2::CanonicalRepoName::new(canonical_repo).unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse(
                    "@@bazel_tools//tools/build_defs/repo:http.bzl",
                )
                .unwrap(),
                rule_name: rule_name.into(),
            },
            attributes: Arc::default(),
        },
        kind,
    }
}

#[test]
fn source_needs_union_deduplicates_and_rejects_conflicts_while_need_is_transient() {
    let dep = materialization_request(
        "/workspace",
        "dep+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    );
    let other = materialization_request(
        "/workspace",
        "other+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    );
    let conflicting_dep = materialization_request(
        "/workspace",
        "dep+",
        "git_repository",
        RepositoryMaterializationKind::Immutable,
    );
    let path_demand = demand(
        "/workspace/MODULE.bazel",
        PathObservationOperation::FileBytes,
    );
    let path = SourcePreparationNeeds::path(NeedPathObservations::singleton(path_demand.dupe()));
    let dep_need = SourcePreparationNeeds::repository(dep.clone());
    let combined = path
        .try_union(&path)
        .unwrap()
        .try_union(&dep_need)
        .unwrap()
        .try_union(&dep_need)
        .unwrap()
        .try_union(&SourcePreparationNeeds::repository(other))
        .unwrap();

    assert_eq!(
        combined
            .path_observations()
            .expect("path demand must be retained")
            .demands(),
        &[path_demand]
    );
    assert_eq!(combined.repository_materializations().len(), 2);
    assert!(matches!(
        combined.try_union(&SourcePreparationNeeds::repository(conflicting_dep)),
        Err(SourcePreparationNeedsError::ConflictingRepositoryRequest {
            canonical_repo
        }) if canonical_repo.as_str() == "dep+"
    ));

    let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::repository(dep));
    assert!(!<RepositoryMaterializationKey as Key>::equality(
        &need, &need
    ));
    assert!(!<RepositoryMaterializationKey as Key>::validity(&need));
}

#[test]
fn result_epoch_rejects_wrong_workspace_duplicates_conflicts_and_success_kind_mismatch() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let immutable = Arc::new(materialization_request(
        "/workspace",
        "dep+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    ));
    let other_workspace = Arc::new(materialization_request(
        "/other-workspace",
        "dep+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    ));
    let conflicting = Arc::new(materialization_request(
        "/workspace",
        "dep+",
        "git_repository",
        RepositoryMaterializationKind::Immutable,
    ));
    let local = Arc::new(materialization_request(
        "/workspace",
        "local+",
        "local_repository",
        RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/workspace/vendor/local").unwrap(),
        },
    ));
    let immutable_success = || {
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity: Arc::from("source"),
            generation_root: PathBuf::from("/immutable"),
            observation_instance: PathObservationInstanceId::new(1),
        })
    };
    let entry =
        |request: Arc<RepositoryMaterializationRequest>| RepositoryMaterializationEpochEntry {
            request,
            result: immutable_success(),
        };

    assert!(matches!(
        RepositoryMaterializationResultEpoch::new(
            workspace.dupe(),
            [entry(other_workspace)]
        ),
        Err(RepositoryMaterializationResultEpochError::WrongWorkspace {
            canonical_repo
        }) if canonical_repo.as_str() == "dep+"
    ));
    assert!(matches!(
        RepositoryMaterializationResultEpoch::new(
            workspace.dupe(),
            [entry(immutable.dupe()), entry(immutable.dupe())]
        ),
        Err(RepositoryMaterializationResultEpochError::DuplicateRepository {
            canonical_repo
        }) if canonical_repo.as_str() == "dep+"
    ));
    assert!(matches!(
        RepositoryMaterializationResultEpoch::new(
            workspace.dupe(),
            [entry(immutable), entry(conflicting)]
        ),
        Err(
            RepositoryMaterializationResultEpochError::ConflictingRepositoryRequest {
                canonical_repo
            }
        ) if canonical_repo.as_str() == "dep+"
    ));
    assert!(matches!(
        RepositoryMaterializationResultEpoch::new(workspace, [entry(local)]),
        Err(
            RepositoryMaterializationResultEpochError::SuccessKindMismatch {
                canonical_repo
            }
        ) if canonical_repo.as_str() == "local+"
    ));
}

#[tokio::test]
async fn materialization_result_key_collision_and_projection_handle_errors_retries_and_omissions() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let workspace = workspace();
    let source = |path: &str| {
        format!(
            "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = '{path}')\n"
        )
    };
    let empty_epoch = || materialization_epoch_input(&workspace, []);
    let old = local_request_for(&workspace, "dep", "vendor/old");
    let current = local_request_for(&workspace, "dep", "vendor/current");
    let entry = |request, result| RepositoryMaterializationEpochEntry { request, result };

    let SourcePreparationOutcome::Need(empty_need) =
        materialization_with_epoch(&dice, &source("vendor/old"), empty_epoch(), 1, "dep").await
    else {
        panic!("an empty result epoch must demand materialization");
    };
    assert_eq!(
        empty_need
            .repository_materializations()
            .values()
            .map(|request| request.as_ref())
            .collect::<Vec<_>>(),
        [old.as_ref()]
    );

    let old_success = materialization_epoch_input(
        &workspace,
        [entry(
            old.dupe(),
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
        )],
    );
    assert!(matches!(
        materialization_with_epoch(&dice, &source("vendor/old"), old_success, 1, "dep").await,
        SourcePreparationOutcome::Complete(value)
            if matches!(
                value.as_ref(),
                Ok(RepositoryMaterialization::Local { source_root, .. })
                    if source_root == &workspace.join("vendor/old")
            )
    ));

    let old_success = materialization_epoch_input(
        &workspace,
        [entry(
            old,
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
        )],
    );
    let SourcePreparationOutcome::Need(changed_spec_need) =
        materialization_with_epoch(&dice, &source("vendor/current"), old_success, 1, "dep").await
    else {
        panic!(
            "same workspace/repository with a distinct RepoSpec must not hit the old result key"
        );
    };
    assert_eq!(
        changed_spec_need
            .repository_materializations()
            .values()
            .map(|request| request.as_ref())
            .collect::<Vec<_>>(),
        [current.as_ref()]
    );

    let persistent = materialization_epoch_input(
        &workspace,
        [entry(
            current.dupe(),
            RepositoryMaterializationResult::SpecError("bad spec".into()),
        )],
    );
    assert!(matches!(
        materialization_with_epoch(&dice, &source("vendor/current"), persistent, 1, "dep").await,
        SourcePreparationOutcome::Complete(value)
            if matches!(
                value.as_ref(),
                Err(RepositoryMaterializationError::Spec(message))
                    if message.as_str() == "bad spec"
            )
    ));

    for result in [
        RepositoryMaterializationResult::TransportError {
            generation: RepositoryMaterializationGeneration(7),
            message: "offline".into(),
        },
        RepositoryMaterializationResult::MaterializationError {
            generation: RepositoryMaterializationGeneration(7),
            message: "unpack failed".into(),
        },
    ] {
        let matching =
            materialization_epoch_input(&workspace, [entry(current.dupe(), result.clone())]);
        assert!(matches!(
            materialization_with_epoch(&dice, &source("vendor/current"), matching, 7, "dep").await,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    (result, value.as_ref()),
                    (
                        RepositoryMaterializationResult::TransportError { .. },
                        Err(RepositoryMaterializationError::Transport(_))
                    ) | (
                        RepositoryMaterializationResult::MaterializationError { .. },
                        Err(RepositoryMaterializationError::Materialization(_))
                    )
                )
        ));
    }

    for stale_result in [
        RepositoryMaterializationResult::TransportError {
            generation: RepositoryMaterializationGeneration(6),
            message: "stale transport".into(),
        },
        RepositoryMaterializationResult::MaterializationError {
            generation: RepositoryMaterializationGeneration(6),
            message: "stale materialization".into(),
        },
    ] {
        let stale = materialization_epoch_input(&workspace, [entry(current.dupe(), stale_result)]);
        assert!(matches!(
            materialization_with_epoch(&dice, &source("vendor/current"), stale, 7, "dep").await,
            SourcePreparationOutcome::Need(_)
        ));
    }

    let spare = local_request_for(&workspace, "spare", "vendor/spare");
    let two_overrides = "module(name = 'root')\n\
        local_path_override(module_name = 'dep', path = 'vendor/current')\n\
        local_path_override(module_name = 'spare', path = 'vendor/spare')\n";
    let only_dep = || {
        materialization_epoch_input(
            &workspace,
            [entry(
                current.dupe(),
                RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
            )],
        )
    };
    assert!(matches!(
        materialization_with_epoch(&dice, two_overrides, only_dep(), 7, "dep").await,
        SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()
    ));
    let SourcePreparationOutcome::Need(spare_need) =
        materialization_with_epoch(&dice, two_overrides, only_dep(), 7, "spare").await
    else {
        panic!("an omitted second repository must demand only its own result");
    };
    assert_eq!(
        spare_need
            .repository_materializations()
            .values()
            .map(|request| request.as_ref())
            .collect::<Vec<_>>(),
        [spare.as_ref()]
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn result_projection_prunes_unrelated_repositories_but_tracks_exact_immutable_selection() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_key = RepositoryMaterializationCounterKey {
        counter: counter.dupe(),
    };
    let workspace = workspace();
    let root_source = "module(name = 'root')\narchive_override(module_name = 'dep')\n";
    let dep = Arc::new(materialization_request(
        workspace.to_str().unwrap(),
        "dep+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    ));
    let spare = Arc::new(materialization_request(
        workspace.to_str().unwrap(),
        "spare+",
        "http_archive",
        RepositoryMaterializationKind::Immutable,
    ));
    let immutable = |request: Arc<RepositoryMaterializationRequest>, root: &str, instance| {
        RepositoryMaterializationEpochEntry {
            request,
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    source_identity: Arc::from("source"),
                    generation_root: PathBuf::from(root),
                    observation_instance: PathObservationInstanceId::new(instance),
                },
            ),
        }
    };
    let epoch = |entries| materialization_epoch_input(&workspace, entries);

    assert_eq!(
        count_repository_materialization_with_epoch(
            &dice,
            root_source,
            epoch(vec![immutable(dep.dupe(), "/generation/a", 1)]),
            1,
            &counter_key,
        )
        .await,
        1
    );
    assert_eq!(
        count_repository_materialization_with_epoch(
            &dice,
            root_source,
            epoch(vec![
                immutable(dep.dupe(), "/generation/a", 1),
                immutable(spare.dupe(), "/generation/spare-a", 1),
            ]),
            1,
            &counter_key,
        )
        .await,
        1,
        "adding an unrelated result must not invalidate dep's projection"
    );
    assert_eq!(
        count_repository_materialization_with_epoch(
            &dice,
            root_source,
            epoch(vec![
                immutable(dep.dupe(), "/generation/a", 1),
                immutable(spare, "/generation/spare-b", 2),
            ]),
            1,
            &counter_key,
        )
        .await,
        1,
        "changing an unrelated result must not invalidate dep's projection"
    );
    assert_eq!(
        count_repository_materialization_with_epoch(
            &dice,
            root_source,
            epoch(vec![immutable(dep.dupe(), "/generation/b", 1)]),
            1,
            &counter_key,
        )
        .await,
        2,
        "changing dep's exact immutable root must invalidate its projection"
    );
    assert_eq!(
        count_repository_materialization_with_epoch(
            &dice,
            root_source,
            epoch(vec![immutable(dep, "/generation/b", 2)]),
            1,
            &counter_key,
        )
        .await,
        3,
        "changing dep's observation instance must invalidate its projection"
    );
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn materialization_need_and_pure_spec_error_precede_path_observation() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let workspace = workspace();
    let compute = |root_source: &str| {
        let dice = dice.dupe();
        let workspace = workspace.clone();
        let root_source = root_source.to_owned();
        async move {
            let mut updater = dice.updater_with_data(UserComputationData::default());
            updater
                .changed_to(vec![(
                    WorkspaceSnapshotKey {
                        workspace: workspace.clone(),
                    },
                    text_snapshot_with(&root_source),
                )])
                .unwrap();
            updater
                .changed_to(vec![(
                    PathObservationEpochKey,
                    PathObservationEpoch::empty(),
                )])
                .unwrap();
            updater
                .changed_to(vec![materialization_epoch_input(&workspace, [])])
                .unwrap();
            inject_root_module_request_inputs(
                &mut updater,
                &workspace,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .unwrap();
            updater
                .changed_to(vec![(
                    RepositoryMaterializationGenerationKey {
                        workspace: workspace.clone(),
                    },
                    RepositoryMaterializationGeneration(1),
                )])
                .unwrap();
            updater
                .commit()
                .await
                .compute(&RepositorySourceFileKey {
                    workspace,
                    module_name: "dep".into(),
                    repo_relative_path: PathBuf::from("MODULE.bazel"),
                })
                .await
                .unwrap()
        }
    };

    let SourcePreparationOutcome::Need(need) = compute(
        "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
    )
    .await
    else {
        panic!("an absent materialization result must demand materialization");
    };
    assert!(need.path_observations().is_none());
    assert_eq!(need.repository_materializations().len(), 1);

    let invalid = compute(
        "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = '/absolute')\n",
    )
    .await;
    assert!(matches!(
        invalid,
        SourcePreparationOutcome::Complete(Err(RepositorySourceFileError::Materialization {
            error,
            ..
        })) if matches!(
            error.as_ref(),
            RepositoryMaterializationError::Spec(message)
                if message.as_str()
                    == "local_repository path must be normalized and workspace-relative"
        )
    ));
}

#[tokio::test]
async fn immutable_materialization_equality_prunes_only_operational_root_and_instance() {
    let a1 = semantic_materialization("/generation/a", 1, "A", "dep+", "http_archive");
    let root_only = semantic_materialization("/generation/b", 1, "A", "dep+", "http_archive");
    let instance_only = semantic_materialization("/generation/a", 2, "A", "dep+", "http_archive");
    let a2 = semantic_materialization("/generation/b", 2, "A", "dep+", "http_archive");
    let identity_b = semantic_materialization("/generation/b", 2, "B", "dep+", "http_archive");
    let other_repo = semantic_materialization("/generation/a", 1, "A", "other+", "http_archive");
    let other_spec = semantic_materialization("/generation/a", 1, "A", "dep+", "git_repository");

    for operationally_distinct in [&root_only, &instance_only, &a2] {
        assert!(!<RepositoryMaterializationKey as Key>::equality(
            &a1,
            operationally_distinct,
        ));
    }
    for semantically_distinct in [&identity_b, &other_repo, &other_spec] {
        assert!(!<RepositoryMaterializationKey as Key>::equality(
            &a1,
            semantically_distinct,
        ));
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_key = MaterializationCounterKey {
        counter: counter.dupe(),
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (value, expected_count) in [(a1.dupe(), 1), (a2, 2), (identity_b, 3), (a1, 4)] {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(SemanticMaterializationInputKey, value)])
            .unwrap();
        let mut transaction = updater.commit().await;
        assert_eq!(
            transaction.compute(&counter_key).await.unwrap(),
            expected_count
        );
        assert_eq!(counter.load(Ordering::SeqCst), expected_count);
    }
}

#[test]
fn repository_source_error_schema_compares_every_shared_semantic_field() {
    let path = Arc::new(PathBuf::from("MODULE.bazel"));
    let other_path = Arc::new(PathBuf::from("parts/dep.MODULE.bazel"));
    let message: Arc<str> = Arc::from("compute failed");
    let other_message: Arc<str> = Arc::from("different failure");
    let materialization = Arc::new(RepositoryMaterializationError::Transport("offline".into()));
    let other_materialization =
        Arc::new(RepositoryMaterializationError::Transport("online".into()));
    let io_error = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let other_io_error = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(14),
    };
    let not_a_link = PathObservationError::NotALink;
    let invalid_relative =
        |requested_path| RepositorySourceFileError::InvalidRepoRelativePath { requested_path };
    let materialization_compute =
        |repo_relative_path, message| RepositorySourceFileError::MaterializationCompute {
            repo_relative_path,
            message,
        };
    let materialization_error =
        |repo_relative_path, error| RepositorySourceFileError::Materialization {
            repo_relative_path,
            error,
        };
    let invalid_materialized = |repo_relative_path| {
        RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path }
    };
    let observation =
        |repo_relative_path, operation, error| RepositorySourceFileError::Observation {
            repo_relative_path,
            operation,
            error,
        };
    let inconsistent = |repo_relative_path, operation, before, after| {
        RepositorySourceFileError::InconsistentState {
            repo_relative_path,
            operation,
            before,
            after,
        }
    };
    let wrong_kind = |repo_relative_path, actual| RepositorySourceFileError::WrongKind {
        repo_relative_path,
        actual,
    };
    let cycle = |repo_relative_path| RepositorySourceFileError::Cycle { repo_relative_path };
    let expansion =
        |repo_relative_path| RepositorySourceFileError::InfiniteExpansion { repo_relative_path };
    let resolution_compute =
        |repo_relative_path, message| RepositorySourceFileError::ResolutionCompute {
            repo_relative_path,
            message,
        };
    let file_compute = |repo_relative_path, message| RepositorySourceFileError::FileCompute {
        repo_relative_path,
        message,
    };
    macro_rules! unequal {
        ($left:expr, $right:expr) => {{
            let left = $left;
            assert_eq!(left, left.dupe());
            assert_ne!(left, $right);
        }};
    }

    unequal!(
        invalid_relative(path.dupe()),
        invalid_relative(other_path.dupe())
    );
    unequal!(
        materialization_compute(path.dupe(), message.dupe()),
        materialization_compute(other_path.dupe(), message.dupe())
    );
    unequal!(
        materialization_compute(path.dupe(), message.dupe()),
        materialization_compute(path.dupe(), other_message.dupe())
    );
    unequal!(
        materialization_error(path.dupe(), materialization.dupe()),
        materialization_error(other_path.dupe(), materialization.dupe())
    );
    unequal!(
        materialization_error(path.dupe(), materialization.dupe()),
        materialization_error(path.dupe(), other_materialization)
    );
    unequal!(
        invalid_materialized(path.dupe()),
        invalid_materialized(other_path.dupe())
    );
    unequal!(
        observation(path.dupe(), PathObservationOperation::Lstat, io_error),
        observation(other_path.dupe(), PathObservationOperation::Lstat, io_error)
    );
    unequal!(
        observation(path.dupe(), PathObservationOperation::Lstat, io_error),
        observation(path.dupe(), PathObservationOperation::ReadLink, io_error)
    );
    unequal!(
        observation(path.dupe(), PathObservationOperation::Lstat, io_error),
        observation(path.dupe(), PathObservationOperation::Lstat, other_io_error)
    );
    assert_eq!(
        observation(path.dupe(), PathObservationOperation::ReadLink, not_a_link,),
        observation(
            path.dupe(),
            PathObservationOperation::ReadLink,
            PathObservationError::NotALink,
        )
    );
    unequal!(
        observation(path.dupe(), PathObservationOperation::ReadLink, not_a_link,),
        observation(path.dupe(), PathObservationOperation::ReadLink, io_error)
    );
    let before = Some(lstat(PathNodeKind::Symlink));
    unequal!(
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            None
        ),
        inconsistent(
            other_path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            None
        )
    );
    unequal!(
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            None
        ),
        inconsistent(
            path.dupe(),
            PathObservationOperation::FileBytes,
            before,
            None
        )
    );
    unequal!(
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            None
        ),
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            Some(lstat(PathNodeKind::RegularFile)),
            None
        )
    );
    unequal!(
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            None
        ),
        inconsistent(
            path.dupe(),
            PathObservationOperation::ReadLink,
            before,
            Some(lstat(PathNodeKind::RegularFile))
        )
    );
    unequal!(
        wrong_kind(path.dupe(), PathNodeKind::Directory),
        wrong_kind(other_path.dupe(), PathNodeKind::Directory)
    );
    unequal!(
        wrong_kind(path.dupe(), PathNodeKind::Directory),
        wrong_kind(path.dupe(), PathNodeKind::SpecialFile)
    );
    unequal!(cycle(path.dupe()), cycle(other_path.dupe()));
    unequal!(expansion(path.dupe()), expansion(other_path.dupe()));
    unequal!(
        resolution_compute(path.dupe(), message.dupe()),
        resolution_compute(other_path.dupe(), message.dupe())
    );
    unequal!(
        resolution_compute(path.dupe(), message.dupe()),
        resolution_compute(path.dupe(), other_message.dupe())
    );
    unequal!(
        file_compute(path.dupe(), message.dupe()),
        file_compute(other_path.dupe(), message.dupe())
    );
    unequal!(
        file_compute(path.dupe(), message.dupe()),
        file_compute(path.dupe(), other_message.dupe())
    );

    let source_error = cycle(path);
    assert_ne!(
        ModuleSourcePreparationError::Source(source_error.dupe()),
        ModuleSourcePreparationError::SourceCompute(Arc::from("source key failed"))
    );
    assert!(matches!(
        ModuleSourcePreparationError::Source(source_error),
        ModuleSourcePreparationError::Source(RepositorySourceFileError::Cycle { .. })
    ));
    assert!(matches!(
        ModuleSourcePreparationError::SourceCompute(Arc::from("source key failed")),
        ModuleSourcePreparationError::SourceCompute(message)
            if message.as_ref() == "source key failed"
    ));
}

async fn assert_cumulative_source_and_preparation_needs(
    dice: &Arc<Dice>,
    observations: &[(PathObservationDemand, PathObservationResult)],
    generation_base: u64,
    expected_bytes: &[u8],
) {
    let immutable_context = observations
        .iter()
        .find_map(|(demand, _)| match demand.namespace() {
            PathObservationNamespace::Materialization(instance)
                if demand
                    .path()
                    .as_path()
                    .file_name()
                    .is_some_and(|name| name == "MODULE.bazel") =>
            {
                Some((
                    demand.path().as_path().parent().unwrap().to_owned(),
                    instance,
                ))
            }
            _ => None,
        });
    for index in 0..observations.len() {
        let mut prefix = observations[..index].to_vec();
        if let Some((root, instance)) = &immutable_context {
            prefix.push((
                demand_in(
                    PathObservationNamespace::Materialization(*instance),
                    root.join(".materialization-root").to_str().unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            ));
        }
        let outcome = source_with_epoch(
            dice,
            raw_snapshot([]),
            PathObservationEpoch::new(prefix.clone()).unwrap(),
            generation_base + index as u64,
            "MODULE.bazel",
        )
        .await;
        let PathOutcome::Need(need) = outcome else {
            panic!("prefix {index} unexpectedly completed");
        };
        assert_eq!(need.demands(), &[observations[index].0.dupe()]);
        let need_outcome =
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need.dupe()));
        assert!(!<RepositorySourceFileKey as Key>::validity(&need_outcome));
        assert!(!<RepositorySourceFileKey as Key>::equality(
            &need_outcome,
            &SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need)),
        ));

        let preparation = prepare_with_epoch(
            dice,
            "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
            raw_snapshot([]),
            PathObservationEpoch::new(prefix).unwrap(),
            &[],
            generation_base + index as u64,
            "1.0.0",
        )
        .await;
        let PathOutcome::Need(preparation_need) = preparation else {
            panic!("preparation prefix {index} unexpectedly completed");
        };
        assert_eq!(preparation_need.demands(), &[observations[index].0.dupe()]);
        let preparation_need =
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(preparation_need));
        assert!(!<ModuleSourcePreparationKey as Key>::validity(
            &preparation_need
        ));
        assert!(!<ModuleSourcePreparationKey as Key>::equality(
            &preparation_need,
            &preparation_need
        ));
    }

    let complete_epoch = PathObservationEpoch::new(observations.to_vec()).unwrap();
    let source_complete = source_with_epoch(
        dice,
        raw_snapshot([]),
        complete_epoch.clone(),
        generation_base + observations.len() as u64,
        "MODULE.bazel",
    )
    .await;
    let PathOutcome::Complete(value) = source_complete else {
        panic!("complete epoch needs observations");
    };
    assert_eq!(
        &value,
        &Ok(RepositorySourceFileValue::Present(Arc::from(
            expected_bytes
        )))
    );
    let source_complete = SourcePreparationOutcome::Complete(value);
    assert!(<RepositorySourceFileKey as Key>::validity(&source_complete));
    assert!(<RepositorySourceFileKey as Key>::equality(
        &source_complete,
        &source_complete
    ));

    let preparation_complete = prepare_with_epoch(
        dice,
        "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
        raw_snapshot([]),
        complete_epoch,
        &[],
        generation_base + observations.len() as u64,
        "1.0.0",
    )
    .await;
    let PathOutcome::Complete(prepared) = preparation_complete else {
        panic!("complete preparation epoch needs observations");
    };
    assert!(matches!(
        prepared.as_ref(),
        Ok(ModuleSourcePreparation::NonRegistry { bytes })
            if bytes.as_ref() == expected_bytes
    ));
    let preparation_complete = SourcePreparationOutcome::Complete(prepared);
    assert!(<ModuleSourcePreparationKey as Key>::validity(
        &preparation_complete
    ));
    assert!(<ModuleSourcePreparationKey as Key>::equality(
        &preparation_complete,
        &preparation_complete
    ));
}

#[tokio::test]
async fn local_source_demands_cumulatively_and_propagates_need_to_preparation() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let module_path = workspace().join("vendor/dep/MODULE.bazel");

    let mut direct = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    direct.push(file_bytes_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"direct"[..])),
    ));
    assert_cumulative_source_and_preparation_needs(&dice, &direct, 1, b"direct").await;

    let mut relative = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    relative.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("target"))),
    ));
    let relative_target = workspace().join("vendor/dep/target");
    relative.push(lstat_observation(
        relative_target.to_str().unwrap(),
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    relative.push(file_bytes_observation(
        relative_target.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"relative"[..])),
    ));
    assert_cumulative_source_and_preparation_needs(&dice, &relative, 20, b"relative").await;

    let mut escaping = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    escaping.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("/outside"))),
    ));
    escaping.push(lstat_observation(
        "/outside",
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    escaping.push(file_bytes_observation(
        "/outside",
        PathOperationResult::Present(Arc::from(&b"escaping"[..])),
    ));
    assert_cumulative_source_and_preparation_needs(&dice, &escaping, 40, b"escaping").await;

    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn local_source_accepts_special_and_projects_resolver_and_file_terminals() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let relative = Arc::new(PathBuf::from("MODULE.bazel"));
    let module_path = workspace().join("vendor/dep/MODULE.bazel");

    let mut special = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
    ));
    special.push(file_bytes_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b""[..])),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(special).unwrap(),
        1,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("special-file epoch needs observations")
    };
    assert_eq!(
        value,
        Ok(RepositorySourceFileValue::Present(Arc::from(&b""[..])))
    );

    let mut relative_link = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    relative_link.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("target"))),
    ));
    let target = workspace().join("vendor/dep/target");
    relative_link.push(lstat_observation(
        target.to_str().unwrap(),
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    relative_link.push(file_bytes_observation(
        target.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"linked"[..])),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(relative_link).unwrap(),
        2,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("relative symlink epoch needs observations")
    };
    assert_eq!(
        value,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"linked"[..]
        )))
    );

    let directory = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Directory)),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(directory).unwrap(),
        3,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("directory epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::WrongKind {
            repo_relative_path: relative.dupe(),
            actual: PathNodeKind::Directory,
        })
    );

    let mut denied = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    denied.push(file_bytes_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Error(PathObservationError::Io {
            kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(denied).unwrap(),
        4,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("denied epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: relative,
            operation: PathObservationOperation::FileBytes,
            error: PathObservationError::Io {
                kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            },
        })
    );

    let missing =
        local_source_observations(PathObservationResult::Lstat(PathOperationResult::Missing));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(missing).unwrap(),
        5,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("missing epoch needs observations")
    };
    assert_eq!(value, Ok(RepositorySourceFileValue::Absent));

    let mut readlink_io = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    readlink_io.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Error(PathObservationError::Io {
            kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(readlink_io).unwrap(),
        10,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("readlink-io epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
            operation: PathObservationOperation::ReadLink,
            error: PathObservationError::Io {
                kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13)
            },
        })
    );

    let mut escape = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    escape.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("/escape"))),
    ));
    escape.push(lstat_observation(
        "/escape",
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    escape.push(file_bytes_observation(
        "/escape",
        PathOperationResult::Present(Arc::from(&b"escape"[..])),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(escape).unwrap(),
        11,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("escaping epoch needs observations")
    };
    assert_eq!(
        value,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"escape"[..]
        )))
    );

    let mut cycle = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    cycle.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("MODULE.bazel"))),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(cycle).unwrap(),
        12,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("cycle epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Cycle {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
        })
    );

    let mut expansion = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    expansion.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("MODULE.bazel/child"))),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(expansion).unwrap(),
        13,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("descendant-expansion epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InfiniteExpansion {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
        })
    );

    let denied_lstat = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Error(PathObservationError::Io {
            kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(denied_lstat).unwrap(),
        6,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("lstat-error epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
            operation: PathObservationOperation::Lstat,
            error: PathObservationError::Io {
                kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            },
        })
    );

    let mut bytes_missing = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    bytes_missing.push(file_bytes_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(bytes_missing).unwrap(),
        7,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("file-missing epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InconsistentState {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
            operation: PathObservationOperation::FileBytes,
            before: Some(lstat(PathNodeKind::RegularFile)),
            after: None,
        })
    );

    let mut readlink_missing = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    readlink_missing.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(readlink_missing).unwrap(),
        8,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("readlink-missing epoch needs observations")
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InconsistentState {
            repo_relative_path: Arc::new(PathBuf::from("MODULE.bazel")),
            operation: PathObservationOperation::ReadLink,
            before: Some(lstat(PathNodeKind::Symlink)),
            after: None,
        })
    );

    let mut dangling = local_source_observations(PathObservationResult::Lstat(
        PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
    ));
    dangling.push(read_link_observation(
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("gone"))),
    ));
    dangling.push(lstat_observation(
        workspace().join("vendor/dep/gone").to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        PathObservationEpoch::new(dangling).unwrap(),
        9,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("dangling epoch needs observations")
    };
    assert_eq!(value, Ok(RepositorySourceFileValue::Absent));
}

#[tokio::test]
async fn local_source_semantics_prune_preparation_and_restore_on_one_retained_engine() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_key = PreparationCounterKey {
        counter: counter.dupe(),
    };
    let root_counter = Arc::new(AtomicUsize::new(0));
    let root_counter_key = RootObservationCounterKey {
        counter: root_counter.dupe(),
        namespace: PathObservationNamespace::Host,
    };
    let module_path = workspace().join("vendor/dep/MODULE.bazel");

    let direct_bytes = |bytes: &'static [u8], metadata, root_metadata| {
        let mut observations = local_source_observations(PathObservationResult::Lstat(
            PathOperationResult::Present(lstat_variant(PathNodeKind::RegularFile, metadata)),
        ));
        observations[0] = lstat_observation(
            "/",
            PathOperationResult::Present(lstat_variant(PathNodeKind::Directory, root_metadata)),
        );
        observations.push(file_bytes_observation(
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::from(bytes)),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let routed_bytes = |target_name: &'static str, bytes: &'static [u8], metadata| {
        let mut observations = local_source_observations(PathObservationResult::Lstat(
            PathOperationResult::Present(lstat_variant(PathNodeKind::Symlink, metadata)),
        ));
        observations.push(read_link_observation(
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from(target_name))),
        ));
        let target = workspace().join("vendor/dep").join(target_name);
        observations.push(lstat_observation(
            target.to_str().unwrap(),
            PathOperationResult::Present(lstat_variant(PathNodeKind::RegularFile, metadata + 10)),
        ));
        observations.push(file_bytes_observation(
            target.to_str().unwrap(),
            PathOperationResult::Present(Arc::from(bytes)),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let direct_missing = PathObservationEpoch::new(local_source_observations(
        PathObservationResult::Lstat(PathOperationResult::Missing),
    ))
    .unwrap();
    let dangling_missing = {
        let mut observations = local_source_observations(PathObservationResult::Lstat(
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ));
        observations.push(read_link_observation(
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from("missing-target"))),
        ));
        observations.push(lstat_observation(
            workspace()
                .join("vendor/dep/missing-target")
                .to_str()
                .unwrap(),
            PathOperationResult::Missing,
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let permission_denied = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let direct_error = PathObservationEpoch::new(local_source_observations(
        PathObservationResult::Lstat(PathOperationResult::Error(permission_denied)),
    ))
    .unwrap();
    let routed_error = {
        let mut observations = local_source_observations(PathObservationResult::Lstat(
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ));
        observations.push(read_link_observation(
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from("error-target"))),
        ));
        observations.push(lstat_observation(
            workspace()
                .join("vendor/dep/error-target")
                .to_str()
                .unwrap(),
            PathOperationResult::Error(permission_denied),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };

    enum Expected {
        Bytes(&'static [u8]),
        Missing,
        Error,
    }
    let steps = [
        (direct_bytes(b"A", 1, 1), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"A", 1, 1_000), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"A", 100, 1_000), 1, Expected::Bytes(b"A")),
        (routed_bytes("route-a", b"A", 200), 1, Expected::Bytes(b"A")),
        (routed_bytes("route-b", b"A", 300), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"B", 400, 1), 2, Expected::Bytes(b"B")),
        (direct_missing, 3, Expected::Missing),
        (dangling_missing, 3, Expected::Missing),
        (direct_error, 4, Expected::Error),
        (routed_error, 4, Expected::Error),
        (direct_bytes(b"A", 500, 1), 5, Expected::Bytes(b"A")),
    ];
    let mut first_a = None;
    for (index, (epoch, expected_count, expected)) in steps.into_iter().enumerate() {
        let counted =
            count_preparation_with_epoch(&dice, epoch.clone(), index as u64 + 1, &counter_key)
                .await;
        assert!(matches!(
            counted,
            PathOutcome::Complete(actual) if actual == expected_count
        ));
        assert_eq!(counter.load(Ordering::SeqCst), expected_count);
        if index < 2 {
            let root_counted =
                count_root_observation_with_epoch(&dice, epoch.clone(), &root_counter_key).await;
            assert!(matches!(
                root_counted,
                PathOutcome::Complete(actual) if actual == index + 1
            ));
            assert_eq!(root_counter.load(Ordering::SeqCst), index + 1);
        }

        let prepared = prepare_with_epoch(
            &dice,
            "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
            raw_snapshot([]),
            epoch,
            &[],
            index as u64 + 1,
            "1.0.0",
        )
        .await;
        let PathOutcome::Complete(prepared) = prepared else {
            panic!("lifecycle step {index} needs observations");
        };
        match expected {
            Expected::Bytes(expected) => assert!(matches!(
                prepared.as_ref(),
                Ok(ModuleSourcePreparation::NonRegistry { bytes })
                    if bytes.as_ref() == expected
            )),
            Expected::Missing => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::ModuleNotFound {
                    module_file_attempts
                }) if module_file_attempts.is_empty()
            )),
            Expected::Error => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::Source(
                    RepositorySourceFileError::Observation {
                        repo_relative_path,
                        operation: PathObservationOperation::Lstat,
                        error,
                    }
                )) if repo_relative_path.as_path() == Path::new("MODULE.bazel")
                    && *error == permission_denied
            )),
        }
        if index == 0 {
            first_a = Some(prepared);
        } else if index == 10 {
            assert_eq!(Some(prepared), first_a);
        }
    }
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
    assert_eq!(root_counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn logical_local_root_and_immutable_instance_moves_select_new_paths_but_prune_equal_bytes() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let local_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let local_counter = Arc::new(AtomicUsize::new(0));
    let local_counter_key = PreparationCounterKey {
        counter: local_counter.dupe(),
    };
    let workspace = workspace();
    let logical_root = workspace.join("vendor/dep");
    let local_route = |target_name: &str| {
        let target = workspace.join("vendor").join(target_name);
        PathObservationEpoch::new([
            lstat_observation(
                "/",
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                workspace.to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                workspace.join("vendor").to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                logical_root.to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
            ),
            read_link_observation(
                logical_root.to_str().unwrap(),
                PathOperationResult::Present(Arc::new(PathBuf::from(target_name))),
            ),
            lstat_observation(
                target.to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                target.join("MODULE.bazel").to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
            ),
            file_bytes_observation(
                target.join("MODULE.bazel").to_str().unwrap(),
                PathOperationResult::Present(Arc::from(&b"same"[..])),
            ),
        ])
        .unwrap()
    };
    for route in ["dep-a", "dep-b"] {
        assert!(matches!(
            count_preparation_with_epoch(&local_dice, local_route(route), 1, &local_counter_key)
                .await,
            PathOutcome::Complete(1)
        ));
    }
    assert_eq!(local_counter.load(Ordering::SeqCst), 1);
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);

    let immutable_io = Arc::new(ImmutableIo {
        calls: AtomicUsize::new(0),
        root: tempfile::tempdir().unwrap(),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, immutable_io.clone());
    let immutable_dice = Arc::new(builder.build(DetectCycles::Enabled));
    let immutable_counter = Arc::new(AtomicUsize::new(0));
    let immutable_counter_key = PreparationCounterKey {
        counter: immutable_counter.dupe(),
    };
    let immutable_epoch =
        |root: &Path, instance: PathObservationInstanceId, bytes: &'static [u8]| {
            let namespace = PathObservationNamespace::Materialization(instance);
            let mut observations = immutable_source_observations(
                root,
                instance,
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::RegularFile,
                ))),
            );
            observations.push(file_bytes_observation_in(
                namespace,
                root.join("MODULE.bazel").to_str().unwrap(),
                PathOperationResult::Present(Arc::from(bytes)),
            ));
            PathObservationEpoch::new(observations).unwrap()
        };
    let root_a = PathBuf::from("/immutable/a");
    let root_b = PathBuf::from("/immutable/b");
    let instance_1 = PathObservationInstanceId::new(11);
    let instance_2 = PathObservationInstanceId::new(12);
    let steps = [
        (immutable_epoch(&root_a, instance_1, b"A"), 1),
        (immutable_epoch(&root_b, instance_2, b"A"), 1),
        (immutable_epoch(&root_b, instance_2, b"B"), 2),
        (immutable_epoch(&root_a, instance_1, b"A"), 3),
    ];
    for (index, (epoch, expected_count)) in steps.into_iter().enumerate() {
        let expected_namespace = if index == 0 || index == 3 {
            PathObservationNamespace::Materialization(instance_1)
        } else {
            PathObservationNamespace::Materialization(instance_2)
        };
        assert!(
            epoch
                .observations()
                .keys()
                .all(|demand| demand.namespace() == expected_namespace)
        );
        assert!(matches!(
            count_preparation_with_epoch(
                &immutable_dice,
                epoch,
                index as u64 + 1,
                &immutable_counter_key
            )
            .await,
            PathOutcome::Complete(actual) if actual == expected_count
        ));
    }
    assert_eq!(immutable_counter.load(Ordering::SeqCst), 3);
    assert_eq!(immutable_io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn immutable_source_demands_exact_instance_cumulatively_through_preparation() {
    let io = Arc::new(ImmutableIo {
        calls: AtomicUsize::new(0),
        root: tempfile::tempdir().unwrap(),
    });
    let root = io.root.path().to_owned();
    let instance = PathObservationInstanceId::new(1);
    let namespace = PathObservationNamespace::Materialization(instance);
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let module_path = root.join("MODULE.bazel");

    let mut direct = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(
            PathNodeKind::RegularFile,
        ))),
    );
    direct.push(file_bytes_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"direct"[..])),
    ));
    assert!(
        direct
            .iter()
            .all(|(demand, _)| demand.namespace() == namespace)
    );
    assert_cumulative_source_and_preparation_needs(&dice, &direct, 1, b"direct").await;

    let mut relative = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    relative.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("target"))),
    ));
    relative.push(lstat_observation_in(
        namespace,
        root.join("target").to_str().unwrap(),
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    relative.push(file_bytes_observation_in(
        namespace,
        root.join("target").to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"relative"[..])),
    ));
    assert!(
        relative
            .iter()
            .all(|(demand, _)| demand.namespace() == namespace)
    );
    assert_cumulative_source_and_preparation_needs(&dice, &relative, 20, b"relative").await;

    let mut escaping = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    escaping.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("/immutable-outside"))),
    ));
    escaping.push(lstat_observation_in(
        namespace,
        "/immutable-outside",
        PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
    ));
    escaping.push(file_bytes_observation_in(
        namespace,
        "/immutable-outside",
        PathOperationResult::Present(Arc::from(&b"escaping"[..])),
    ));
    assert!(
        escaping
            .iter()
            .all(|(demand, _)| demand.namespace() == namespace)
    );
    assert_cumulative_source_and_preparation_needs(&dice, &escaping, 40, b"escaping").await;

    let wrong_instance = PathObservationInstanceId::new(2);
    let wrong_namespace = PathObservationNamespace::Materialization(wrong_instance);
    let mut wrong = immutable_source_observations(
        &root,
        wrong_instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(
            PathNodeKind::RegularFile,
        ))),
    );
    wrong.push(file_bytes_observation_in(
        wrong_namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b"wrong-instance"[..])),
    ));
    let PathOutcome::Need(need) = source_with_result_epoch(
        &dice,
        immutable_text_snapshot(),
        raw_snapshot([]),
        PathObservationEpoch::new(wrong).unwrap(),
        immutable_materialization_epoch_input(&workspace(), root.clone(), instance),
        60,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("wrong materialization instance unexpectedly satisfied the source read");
    };
    assert_eq!(
        need.demands(),
        &[demand_in(namespace, "/", PathObservationOperation::Lstat)]
    );
    assert!(
        need.demands()
            .iter()
            .all(|demand| demand.namespace() != PathObservationNamespace::Host)
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn immutable_source_accepts_special_and_projects_all_observed_terminals() {
    let io = Arc::new(ImmutableIo {
        calls: AtomicUsize::new(0),
        root: tempfile::tempdir().unwrap(),
    });
    let root = io.root.path().to_owned();
    let instance = PathObservationInstanceId::new(1);
    let namespace = PathObservationNamespace::Materialization(instance);
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let module_path = root.join("MODULE.bazel");
    let relative = Arc::new(PathBuf::from("MODULE.bazel"));
    let permission_denied = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };

    let complete = |observations| PathObservationEpoch::new(observations).unwrap();

    let mut special = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(
            PathNodeKind::SpecialFile,
        ))),
    );
    special.push(file_bytes_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::from(&b""[..])),
    ));
    let special_epoch = complete(special);
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        special_epoch.clone(),
        1,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("special-file epoch needs observations");
    };
    assert_eq!(
        value,
        Ok(RepositorySourceFileValue::Present(Arc::from(&b""[..])))
    );
    let PathOutcome::Complete(prepared) = prepare_with_epoch(
        &dice,
        "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
        raw_snapshot([]),
        special_epoch,
        &[],
        1,
        "1.0.0",
    )
    .await
    else {
        panic!("special-file preparation needs observations");
    };
    assert!(matches!(
        prepared.as_ref(),
        Ok(ModuleSourcePreparation::NonRegistry { bytes }) if bytes.is_empty()
    ));

    let directory = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Directory))),
    );
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(directory),
        2,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("directory epoch requested FileBytes");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::WrongKind {
            repo_relative_path: relative.dupe(),
            actual: PathNodeKind::Directory,
        })
    );

    let missing = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Missing),
    );
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(missing),
        3,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("missing epoch needs observations");
    };
    assert_eq!(value, Ok(RepositorySourceFileValue::Absent));

    let denied_lstat = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Error(permission_denied)),
    );
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(denied_lstat),
        4,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("lstat-error epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: relative.dupe(),
            operation: PathObservationOperation::Lstat,
            error: permission_denied,
        })
    );

    let mut denied_readlink = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    denied_readlink.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Error(permission_denied),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(denied_readlink),
        5,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("readlink-error epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: relative.dupe(),
            operation: PathObservationOperation::ReadLink,
            error: permission_denied,
        })
    );

    let mut not_a_link_readlink = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    not_a_link_readlink.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Error(PathObservationError::NotALink),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(not_a_link_readlink),
        5,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("not-a-link readlink epoch needs observations");
    };
    let expected = Err(RepositorySourceFileError::Observation {
        repo_relative_path: relative.dupe(),
        operation: PathObservationOperation::ReadLink,
        error: PathObservationError::NotALink,
    });
    assert_eq!(value, expected);
    let complete_value = SourcePreparationOutcome::Complete(value.dupe());
    assert!(<RepositorySourceFileKey as Key>::validity(&complete_value));
    assert!(<RepositorySourceFileKey as Key>::equality(
        &complete_value,
        &SourcePreparationOutcome::Complete(expected),
    ));

    let mut denied_bytes = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(
            PathNodeKind::RegularFile,
        ))),
    );
    denied_bytes.push(file_bytes_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Error(permission_denied),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(denied_bytes),
        6,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("file-error epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Observation {
            repo_relative_path: relative.dupe(),
            operation: PathObservationOperation::FileBytes,
            error: permission_denied,
        })
    );

    let mut missing_readlink = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    missing_readlink.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(missing_readlink),
        7,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("readlink-missing epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InconsistentState {
            repo_relative_path: relative.dupe(),
            operation: PathObservationOperation::ReadLink,
            before: Some(lstat(PathNodeKind::Symlink)),
            after: None,
        })
    );

    let mut missing_bytes = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(
            PathNodeKind::RegularFile,
        ))),
    );
    missing_bytes.push(file_bytes_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(missing_bytes),
        8,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("file-missing epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InconsistentState {
            repo_relative_path: relative.dupe(),
            operation: PathObservationOperation::FileBytes,
            before: Some(lstat(PathNodeKind::RegularFile)),
            after: None,
        })
    );

    let mut dangling = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    dangling.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("gone"))),
    ));
    dangling.push(lstat_observation_in(
        namespace,
        root.join("gone").to_str().unwrap(),
        PathOperationResult::Missing,
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(dangling),
        9,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("dangling epoch needs observations");
    };
    assert_eq!(value, Ok(RepositorySourceFileValue::Absent));

    let mut cycle = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    cycle.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("MODULE.bazel"))),
    ));
    let PathOutcome::Complete(value) =
        source_with_epoch(&dice, raw_snapshot([]), complete(cycle), 10, "MODULE.bazel").await
    else {
        panic!("cycle epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::Cycle {
            repo_relative_path: relative.dupe(),
        })
    );

    let mut expansion = immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Present(lstat(PathNodeKind::Symlink))),
    );
    expansion.push(read_link_observation_in(
        namespace,
        module_path.to_str().unwrap(),
        PathOperationResult::Present(Arc::new(PathBuf::from("MODULE.bazel/child"))),
    ));
    let PathOutcome::Complete(value) = source_with_epoch(
        &dice,
        raw_snapshot([]),
        complete(expansion),
        11,
        "MODULE.bazel",
    )
    .await
    else {
        panic!("expansion epoch needs observations");
    };
    assert_eq!(
        value,
        Err(RepositorySourceFileError::InfiniteExpansion {
            repo_relative_path: relative,
        })
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn immutable_source_prunes_operational_changes_and_restores_on_fixed_instance() {
    let io = Arc::new(ImmutableIo {
        calls: AtomicUsize::new(0),
        root: tempfile::tempdir().unwrap(),
    });
    let root = io.root.path().to_owned();
    let instance = PathObservationInstanceId::new(1);
    let namespace = PathObservationNamespace::Materialization(instance);
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_key = PreparationCounterKey {
        counter: counter.dupe(),
    };
    let root_counter = Arc::new(AtomicUsize::new(0));
    let root_counter_key = RootObservationCounterKey {
        counter: root_counter.dupe(),
        namespace,
    };
    let module_path = root.join("MODULE.bazel");

    let direct_bytes = |bytes: &'static [u8], metadata, root_metadata| {
        let mut observations = immutable_source_observations(
            &root,
            instance,
            PathObservationResult::Lstat(PathOperationResult::Present(lstat_variant(
                PathNodeKind::RegularFile,
                metadata,
            ))),
        );
        observations[0] = lstat_observation_in(
            namespace,
            "/",
            PathOperationResult::Present(lstat_variant(PathNodeKind::Directory, root_metadata)),
        );
        observations.push(file_bytes_observation_in(
            namespace,
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::from(bytes)),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let routed_bytes = |target_name: &'static str, bytes: &'static [u8], metadata| {
        let mut observations = immutable_source_observations(
            &root,
            instance,
            PathObservationResult::Lstat(PathOperationResult::Present(lstat_variant(
                PathNodeKind::Symlink,
                metadata,
            ))),
        );
        observations.push(read_link_observation_in(
            namespace,
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from(target_name))),
        ));
        let target = root.join(target_name);
        observations.push(lstat_observation_in(
            namespace,
            target.to_str().unwrap(),
            PathOperationResult::Present(lstat_variant(PathNodeKind::RegularFile, metadata + 10)),
        ));
        observations.push(file_bytes_observation_in(
            namespace,
            target.to_str().unwrap(),
            PathOperationResult::Present(Arc::from(bytes)),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let direct_missing = PathObservationEpoch::new(immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Missing),
    ))
    .unwrap();
    let dangling_missing = {
        let mut observations = immutable_source_observations(
            &root,
            instance,
            PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                PathNodeKind::Symlink,
            ))),
        );
        observations.push(read_link_observation_in(
            namespace,
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from("missing-target"))),
        ));
        observations.push(lstat_observation_in(
            namespace,
            root.join("missing-target").to_str().unwrap(),
            PathOperationResult::Missing,
        ));
        PathObservationEpoch::new(observations).unwrap()
    };
    let permission_denied = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let direct_error = PathObservationEpoch::new(immutable_source_observations(
        &root,
        instance,
        PathObservationResult::Lstat(PathOperationResult::Error(permission_denied)),
    ))
    .unwrap();
    let routed_error = {
        let mut observations = immutable_source_observations(
            &root,
            instance,
            PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                PathNodeKind::Symlink,
            ))),
        );
        observations.push(read_link_observation_in(
            namespace,
            module_path.to_str().unwrap(),
            PathOperationResult::Present(Arc::new(PathBuf::from("error-target"))),
        ));
        observations.push(lstat_observation_in(
            namespace,
            root.join("error-target").to_str().unwrap(),
            PathOperationResult::Error(permission_denied),
        ));
        PathObservationEpoch::new(observations).unwrap()
    };

    enum Expected {
        Bytes(&'static [u8]),
        Missing,
        Error,
    }
    let steps = [
        (direct_bytes(b"A", 1, 1), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"A", 1, 1_000), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"A", 100, 1_000), 1, Expected::Bytes(b"A")),
        (routed_bytes("route-a", b"A", 200), 1, Expected::Bytes(b"A")),
        (routed_bytes("route-b", b"A", 300), 1, Expected::Bytes(b"A")),
        (direct_bytes(b"B", 400, 1), 2, Expected::Bytes(b"B")),
        (direct_missing, 3, Expected::Missing),
        (dangling_missing, 3, Expected::Missing),
        (direct_error, 4, Expected::Error),
        (routed_error, 4, Expected::Error),
        (direct_bytes(b"A", 500, 1), 5, Expected::Bytes(b"A")),
    ];
    let mut first_a = None;
    for (index, (epoch, expected_count, expected)) in steps.into_iter().enumerate() {
        assert!(
            epoch
                .observations()
                .keys()
                .all(|demand| demand.namespace() == namespace)
        );
        let counted =
            count_preparation_with_epoch(&dice, epoch.clone(), index as u64 + 1, &counter_key)
                .await;
        assert!(matches!(
            counted,
            PathOutcome::Complete(actual) if actual == expected_count
        ));
        assert_eq!(counter.load(Ordering::SeqCst), expected_count);
        if index < 2 {
            let root_counted =
                count_root_observation_with_epoch(&dice, epoch.clone(), &root_counter_key).await;
            assert!(matches!(
                root_counted,
                PathOutcome::Complete(actual) if actual == index + 1
            ));
            assert_eq!(root_counter.load(Ordering::SeqCst), index + 1);
        }

        let prepared = prepare_with_epoch(
            &dice,
            "module(name = 'root')\nlocal_path_override(module_name = 'dep', path = 'vendor/dep')\n",
            raw_snapshot([]),
            epoch,
            &[],
            index as u64 + 1,
            "1.0.0",
        )
        .await;
        let PathOutcome::Complete(prepared) = prepared else {
            panic!("immutable lifecycle step {index} needs observations");
        };
        match expected {
            Expected::Bytes(expected) => assert!(matches!(
                prepared.as_ref(),
                Ok(ModuleSourcePreparation::NonRegistry { bytes })
                    if bytes.as_ref() == expected
            )),
            Expected::Missing => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::ModuleNotFound {
                    module_file_attempts
                }) if module_file_attempts.is_empty()
            )),
            Expected::Error => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::Source(
                    RepositorySourceFileError::Observation {
                        repo_relative_path,
                        operation: PathObservationOperation::Lstat,
                        error,
                    }
                )) if repo_relative_path.as_path() == Path::new("MODULE.bazel")
                    && *error == permission_denied
            )),
        }
        if index == 0 {
            first_a = Some(prepared);
        } else if index == 10 {
            assert_eq!(Some(prepared), first_a);
        }
    }
    assert_eq!(counter.load(Ordering::SeqCst), 5);
    assert_eq!(root_counter.load(Ordering::SeqCst), 2);
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn local_root_and_include_replay_independently_without_a_dice_cycle() {
    let io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));

    assert_eq!(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            source(
                &dice,
                raw_snapshot([
                    (
                        "MODULE.bazel",
                        WorkspaceRawFileValue::Present(Arc::from(&b"module(name = 'first')"[..],)),
                    ),
                    (
                        "parts/dep.MODULE.bazel",
                        WorkspaceRawFileValue::Present(Arc::from(&b"bazel_dep(name = 'a')"[..])),
                    ),
                ]),
                1,
                "MODULE.bazel",
            ),
        )
        .await,
        Ok(Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'first')"[..]
        ))))
    );
    assert_eq!(
        source(
            &dice,
            raw_snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"module(name = 'first')"[..],)),
                ),
                (
                    "parts/dep.MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"bazel_dep(name = 'b')"[..])),
                ),
            ]),
            2,
            "parts/dep.MODULE.bazel",
        )
        .await,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"bazel_dep(name = 'b')"[..]
        )))
    );
    assert_eq!(
        source(
            &dice,
            raw_snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"module(name = 'second')"[..],)),
                ),
                (
                    "parts/dep.MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"bazel_dep(name = 'b')"[..])),
                ),
            ]),
            3,
            "MODULE.bazel",
        )
        .await,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'second')"[..]
        )))
    );
    assert_eq!(
        source(
            &dice,
            raw_snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"module(name = 'first')"[..],)),
                ),
                (
                    "parts/dep.MODULE.bazel",
                    WorkspaceRawFileValue::Present(Arc::from(&b"bazel_dep(name = 'b')"[..])),
                ),
            ]),
            4,
            "MODULE.bazel",
        )
        .await,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'first')"[..]
        )))
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);

    assert_eq!(
        source(
            &dice,
            raw_snapshot([("MODULE.bazel", WorkspaceRawFileValue::Absent)]),
            5,
            "MODULE.bazel",
        )
        .await,
        Ok(RepositorySourceFileValue::Absent)
    );
    assert!(matches!(
        source(
            &dice,
            raw_snapshot([(
                "MODULE.bazel",
                WorkspaceRawFileValue::ReadError(Arc::new("denied".to_owned())),
            )]),
            6,
            "MODULE.bazel",
        )
        .await,
        Err(RepositorySourceFileError::Observation {
            operation: PathObservationOperation::Lstat,
            error: PathObservationError::Io {
                kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13)
            },
            ..
        })
    ));
}

#[tokio::test]
async fn materialization_failure_retries_when_the_generation_changes() {
    let io = Arc::new(FlakyIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_repository_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let raw = raw_snapshot([(
        "MODULE.bazel",
        WorkspaceRawFileValue::Present(Arc::from(&b"module(name = 'recovered')"[..])),
    )]);

    assert_eq!(
        source(&dice, raw.clone(), 1, "MODULE.bazel").await,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'recovered')"[..]
        )))
    );
    assert_eq!(
        source(&dice, raw, 2, "MODULE.bazel").await,
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'recovered')"[..]
        )))
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn registry_preparation_falls_through_only_not_found_and_preserves_raw_bytes() {
    let first = "https://first.invalid/modules/dep/1.0.0/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1.0.0/MODULE.bazel";
    let raw_module: Arc<[u8]> = Arc::from(&b"\xffraw module bytes\r\n"[..]);
    let io = Arc::new(FakeRegistryIo::new([
        (first, FakeRegistryResponse::NotFound),
        (second, FakeRegistryResponse::Found(raw_module.clone())),
    ]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));

    let prepared = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        prepare(
            &dice,
            "module(name = 'root')\nbazel_dep(name = 'dep', version = '1.0.0')\n",
            raw_workspace_snapshot([]),
            &["https://first.invalid", "https://second.invalid"],
            1,
            "1.0.0",
        ),
    )
    .await
    .expect("source preparation must not introduce a DICE cycle");
    let Ok(ModuleSourcePreparation::Registry {
        bytes,
        selected_registry,
        module_file_attempts,
    }) = prepared.as_ref()
    else {
        panic!("expected registry preparation")
    };
    assert_eq!(bytes.as_ref(), raw_module.as_ref());
    assert_eq!(selected_registry.as_str(), "https://second.invalid");
    assert_eq!(module_file_attempts.len(), 2);
    assert_eq!(module_file_attempts[0].url.as_str(), first);
    assert_eq!(module_file_attempts[0].sha256, None);
    assert_eq!(module_file_attempts[1].url.as_str(), second);
    assert_eq!(
        module_file_attempts[1].sha256,
        Some(Sha256::digest(raw_module.as_ref()).into())
    );
    assert_eq!(io.calls(), [first, second]);

    let third = "https://third.invalid/modules/dep/1.0.0/MODULE.bazel";
    let io = Arc::new(FakeRegistryIo::new([
        (first, FakeRegistryResponse::NotFound),
        (second, FakeRegistryResponse::Error("fatal transport")),
        (
            third,
            FakeRegistryResponse::Found(Arc::from(&b"must not be read"[..])),
        ),
    ]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let prepared = prepare(
        &dice,
        "module(name = 'root')\nbazel_dep(name = 'dep', version = '1.0.0')\n",
        raw_workspace_snapshot([]),
        &[
            "https://first.invalid",
            "https://second.invalid",
            "https://third.invalid",
        ],
        1,
        "1.0.0",
    )
    .await;
    let Err(ModuleSourcePreparationError::RegistryFile {
        url,
        prior_not_found_attempts,
        error,
    }) = prepared.as_ref()
    else {
        panic!("expected typed fatal registry error: {prepared:?}");
    };
    assert_eq!(url.as_str(), second);
    assert_eq!(prior_not_found_attempts.len(), 1);
    assert_eq!(prior_not_found_attempts[0].url.as_str(), first);
    assert_eq!(prior_not_found_attempts[0].sha256, None);
    assert!(matches!(
        error,
        RegistryFileError::Transport { url, message }
            if url.as_str() == second && message.as_str() == "fatal transport"
    ));
    assert_eq!(io.calls(), [first, second]);
}

#[tokio::test]
async fn selected_registry_provenance_replays_a_b_a_structurally() {
    let first = "https://first.invalid/modules/dep/1.0.0/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1.0.0/MODULE.bazel";
    let module_bytes: Arc<[u8]> = Arc::from(&b"identical module bytes"[..]);
    let io = Arc::new(FakeRegistryIo::new([
        (first, FakeRegistryResponse::Found(module_bytes.clone())),
        (second, FakeRegistryResponse::Found(module_bytes.clone())),
    ]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name = 'root')\nbazel_dep(name = 'dep', version = '1.0.0')\n";

    let first_a = prepare(
        &dice,
        root,
        raw_workspace_snapshot([]),
        &["https://first.invalid"],
        1,
        "1.0.0",
    )
    .await;
    let selected_b = prepare(
        &dice,
        root,
        raw_workspace_snapshot([]),
        &["https://second.invalid"],
        2,
        "1.0.0",
    )
    .await;
    let second_a = prepare(
        &dice,
        root,
        raw_workspace_snapshot([]),
        &["https://first.invalid"],
        3,
        "1.0.0",
    )
    .await;

    assert_eq!(first_a, second_a);
    assert_ne!(first_a, selected_b);
    assert!(matches!(
        first_a.as_ref(),
        Ok(ModuleSourcePreparation::Registry {
            bytes,
            selected_registry,
            module_file_attempts,
        }) if bytes == &module_bytes
            && selected_registry.as_str() == "https://first.invalid"
            && module_file_attempts.len() == 1
            && module_file_attempts[0].url.as_str() == first
            && module_file_attempts[0].sha256
                == Some(Sha256::digest(module_bytes.as_ref()).into())
    ));
    assert!(matches!(
        selected_b.as_ref(),
        Ok(ModuleSourcePreparation::Registry {
            bytes,
            selected_registry,
            module_file_attempts,
        }) if bytes == &module_bytes
            && selected_registry.as_str() == "https://second.invalid"
            && module_file_attempts.len() == 1
            && module_file_attempts[0].url.as_str() == second
            && module_file_attempts[0].sha256
                == Some(Sha256::digest(module_bytes.as_ref()).into())
    ));
}

#[tokio::test]
async fn override_registry_uses_the_effective_version_and_bypasses_defaults() {
    let expected = "https://override.invalid/modules/dep/9.0.0/MODULE.bazel";
    let override_bytes: Arc<[u8]> = Arc::from(&b"effective override"[..]);
    let io = Arc::new(FakeRegistryIo::new([(
        expected,
        FakeRegistryResponse::Found(override_bytes.clone()),
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let prepared = prepare(
        &dice,
        "module(name = 'root')\n\
         bazel_dep(name = 'dep', version = '1.0.0')\n\
         single_version_override(module_name = 'dep', version = '9.0.0', registry = 'https://override.invalid/')\n",
        raw_workspace_snapshot([]),
        &["https://default.invalid"],
        1,
        "9.0.0",
    )
    .await;

    let Ok(ModuleSourcePreparation::Registry {
        bytes,
        selected_registry,
        module_file_attempts,
    }) = prepared.as_ref()
    else {
        panic!("expected override registry preparation: {prepared:?}");
    };
    assert_eq!(bytes.as_ref(), override_bytes.as_ref());
    assert_eq!(selected_registry.as_str(), "https://override.invalid");
    assert_eq!(module_file_attempts.len(), 1);
    assert_eq!(module_file_attempts[0].url.as_str(), expected);
    assert_eq!(
        module_file_attempts[0].sha256,
        Some(Sha256::digest(override_bytes.as_ref()).into())
    );
    assert_eq!(io.calls(), [expected]);

    let io = Arc::new(FakeRegistryIo::new([(
        expected,
        FakeRegistryResponse::NotFound,
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let missing = prepare(
        &dice,
        "module(name = 'root')\n\
         bazel_dep(name = 'dep', version = '1.0.0')\n\
         single_version_override(module_name = 'dep', version = '9.0.0', registry = 'https://override.invalid/')\n",
        raw_workspace_snapshot([]),
        &["https://default.invalid"],
        1,
        "9.0.0",
    )
    .await;
    let Err(ModuleSourcePreparationError::ModuleNotFound {
        module_file_attempts,
    }) = missing.as_ref()
    else {
        panic!("expected typed override registry miss: {missing:?}");
    };
    assert_eq!(module_file_attempts.len(), 1);
    assert_eq!(module_file_attempts[0].url.as_str(), expected);
    assert_eq!(module_file_attempts[0].sha256, None);
    assert_eq!(io.calls(), [expected]);
}

#[tokio::test]
async fn nonregistry_preparation_bypasses_registry_io() {
    let registry_io = Arc::new(FakeRegistryIo::new([(
        "https://default.invalid/modules/dep/1.0.0/MODULE.bazel",
        FakeRegistryResponse::Error("must not be called"),
    )]));
    let repository_io = Arc::new(LocalIo {
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, registry_io.clone());
    install_repository_io(&mut builder, repository_io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let prepared = prepare(
        &dice,
        "module(name = 'root')\n\
         bazel_dep(name = 'dep')\n\
         local_path_override(module_name = 'dep', path = 'vendor/dep')\n",
        raw_workspace_snapshot([(
            "vendor/dep/MODULE.bazel",
            WorkspaceRawFileValue::Present(Arc::from(&b"local module"[..])),
        )]),
        &["https://default.invalid"],
        1,
        "",
    )
    .await;

    assert!(matches!(
        prepared.as_ref(),
        Ok(ModuleSourcePreparation::NonRegistry { bytes }) if bytes.as_ref() == b"local module"
    ));
    assert!(registry_io.calls().is_empty());
    assert_eq!(repository_io.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn selected_patch_replays_a_b_errors_recovery_and_a_without_refetch() {
    let module_url = "file:///registry/modules/dep/1.0.0/MODULE.bazel";
    let original = b"module(name = 'dep', version = '1.0.0')\n\
bazel_dep(name = 'base', version = '1.0.0')\n";
    let patch = |leaf: &'static str| {
        Arc::from(
            format!(
                concat!(
                    "--- a/MODULE.bazel\n",
                    "+++ b/MODULE.bazel\n",
                    "@@ -1,2 +1,2 @@\n",
                    " module(name = 'dep', version = '1.0.0')\n",
                    "-bazel_dep(name = 'base', version = '1.0.0')\n",
                    "+bazel_dep(name = '{}', version = '1.0.0')\n",
                ),
                leaf
            )
            .into_bytes(),
        )
    };
    let io = Arc::new(FakeRegistryIo::new([(
        module_url,
        FakeRegistryResponse::Found(Arc::from(&original[..])),
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name = 'root')\n\
                bazel_dep(name = 'dep', version = '1.0.0')\n\
                single_version_override(module_name = 'dep', patches = ['//:route.patch'], patch_strip = 1)\n";

    let workspace = workspace();
    let route = workspace.join("route.patch");
    let route = route.to_str().unwrap();
    let base = [
        lstat_observation(
            "/",
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
    ];
    enum PatchStep {
        Bytes(Arc<[u8]>, Option<&'static str>),
        Missing,
        FileError,
        Malformed,
    }
    let mut first_a = None;
    for (generation, step) in [
        (1, PatchStep::Bytes(patch("leaf_a"), Some("leaf_a"))),
        (2, PatchStep::Bytes(patch("leaf_b"), Some("leaf_b"))),
        (3, PatchStep::Missing),
        (4, PatchStep::FileError),
        (5, PatchStep::Malformed),
        (6, PatchStep::Bytes(patch("leaf_b"), Some("leaf_b"))),
        (7, PatchStep::Bytes(patch("leaf_a"), Some("leaf_a"))),
    ] {
        let mut observations = base.to_vec();
        let expected = match step {
            PatchStep::Bytes(bytes, leaf) => {
                observations.push(lstat_observation(
                    route,
                    PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
                ));
                observations.push(file_bytes_observation(
                    route,
                    PathOperationResult::Present(bytes),
                ));
                leaf
            }
            PatchStep::Missing => {
                observations.push(lstat_observation(route, PathOperationResult::Missing));
                None
            }
            PatchStep::FileError => {
                observations.push(lstat_observation(
                    route,
                    PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
                ));
                observations.push(file_bytes_observation(
                    route,
                    PathOperationResult::Error(PathObservationError::Io {
                        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                        raw_os_error: Some(13),
                    }),
                ));
                None
            }
            PatchStep::Malformed => {
                observations.push(lstat_observation(
                    route,
                    PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
                ));
                observations.push(file_bytes_observation(
                    route,
                    PathOperationResult::Present(Arc::from(&b"not a patch"[..])),
                ));
                None
            }
        };
        let outcome = prepare_with_epoch(
            &dice,
            root,
            raw_workspace_snapshot([]),
            PathObservationEpoch::new(observations).unwrap(),
            &["file:///registry"],
            generation,
            "1.0.0",
        )
        .await;
        let PathOutcome::Complete(prepared) = outcome else {
            panic!("complete lifecycle epoch unexpectedly needs observations");
        };
        match expected {
            Some(leaf) => {
                let Ok(ModuleSourcePreparation::Registry {
                    bytes,
                    module_file_attempts,
                    ..
                }) = prepared.as_ref()
                else {
                    panic!("expected patched registry preparation: {prepared:?}");
                };
                assert!(String::from_utf8_lossy(bytes).contains(leaf));
                assert_eq!(module_file_attempts.len(), 1);
                assert_eq!(
                    module_file_attempts[0].sha256,
                    Some(Sha256::digest(original).into())
                );
                if generation == 1 {
                    first_a = Some(prepared.clone());
                }
                if generation == 7 {
                    assert_eq!(Some(prepared.clone()), first_a);
                }
            }
            None if generation == 3 => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::PatchMissing { logical_path })
                    if logical_path.as_path() == Path::new(route)
            )),
            None if generation == 4 => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::PatchFileObservation { demand: actual_demand, error })
                    if actual_demand == &demand(route, PathObservationOperation::FileBytes)
                        && *error == PathObservationError::Io {
                            kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
                            raw_os_error: Some(13),
                        }
            )),
            None => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::Patch(_))
            )),
        }
    }
    assert_eq!(io.calls(), [module_url]);
}

#[tokio::test]
async fn root_patches_validate_all_paths_before_reading_or_applying() {
    let module_url = "file:///registry/modules/dep/1.0.0/MODULE.bazel";
    let original: Arc<[u8]> = Arc::from(&b"value = 'base'\n"[..]);
    let io = Arc::new(FakeRegistryIo::new([(
        module_url,
        FakeRegistryResponse::Found(original.clone()),
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = root_patch_source("'//:first.patch', '//:second.patch'");
    let workspace = workspace();
    let first = workspace.join("first.patch");
    let second = workspace.join("second.patch");
    let first = first.to_str().unwrap();
    let second = second.to_str().unwrap();
    let first_patch: Arc<[u8]> = Arc::from(
        &b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-value = 'base'\n+value = 'first'\n"[..],
    );
    let second_patch: Arc<[u8]> = Arc::from(
        &b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-value = 'first'\n+value = 'final'\n"[..],
    );
    let observations = vec![
        lstat_observation(
            "/",
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            first,
            PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
        ),
        lstat_observation(
            second,
            PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
        ),
        file_bytes_observation(first, PathOperationResult::Present(first_patch)),
        file_bytes_observation(second, PathOperationResult::Present(second_patch)),
    ];
    for prefix_len in 0..=observations.len() {
        let epoch = PathObservationEpoch::new(observations[..prefix_len].iter().cloned()).unwrap();
        let outcome = prepare_with_epoch(
            &dice,
            &root,
            raw_workspace_snapshot([]),
            epoch,
            &["file:///registry"],
            prefix_len as u64 + 1,
            "1.0.0",
        )
        .await;
        if prefix_len < observations.len() {
            let PathOutcome::Need(need) = outcome else {
                panic!("prefix {prefix_len} unexpectedly completed: {outcome:?}");
            };
            assert_eq!(need.demands(), &[observations[prefix_len].0.clone()]);
            let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need));
            assert!(!ModuleSourcePreparationKey::validity(&need));
            assert!(!ModuleSourcePreparationKey::equality(&need, &need));
        } else {
            let PathOutcome::Complete(prepared) = outcome else {
                panic!("complete epoch unexpectedly needs observations");
            };
            let complete = SourcePreparationOutcome::Complete(prepared.clone());
            assert!(ModuleSourcePreparationKey::validity(&complete));
            assert!(ModuleSourcePreparationKey::equality(&complete, &complete));
            let Ok(ModuleSourcePreparation::Registry { bytes, .. }) = prepared.as_ref() else {
                panic!("expected patched registry bytes: {prepared:?}");
            };
            assert_eq!(bytes.as_ref(), b"value = 'final'\n");
        }
    }

    let missing_second = PathObservationEpoch::new([
        lstat_observation(
            "/",
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            first,
            PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
        ),
        lstat_observation(second, PathOperationResult::Missing),
    ])
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        missing_second,
        &["file:///registry"],
        20,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchMissing { logical_path }) if logical_path.as_path() == Path::new(second))
    ));

    let malformed_first_then_missing_second =
        root_patch_source("'//:malformed.patch', '//:missing.patch'");
    let malformed = workspace.join("malformed.patch");
    let missing = workspace.join("missing.patch");
    let malformed = malformed.to_str().unwrap();
    let missing = missing.to_str().unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &malformed_first_then_missing_second,
        raw_workspace_snapshot([]),
        PathObservationEpoch::new([
            lstat_observation(
                "/",
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                workspace.to_str().unwrap(),
                PathOperationResult::Present(lstat(PathNodeKind::Directory)),
            ),
            lstat_observation(
                malformed,
                PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
            ),
            lstat_observation(missing, PathOperationResult::Missing),
        ])
        .unwrap(),
        &["file:///registry"],
        21,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchMissing { logical_path }) if logical_path.as_path() == Path::new(missing))
    ));
    assert_eq!(io.calls(), [module_url, module_url]);
}

#[tokio::test]
async fn root_patch_special_files_and_filebytes_failures_are_typed() {
    let module_url = "file:///registry/modules/dep/1.0.0/MODULE.bazel";
    let original: Arc<[u8]> = Arc::from(&b"value = 'base'\n"[..]);
    let io = Arc::new(FakeRegistryIo::new([(
        module_url,
        FakeRegistryResponse::Found(original.clone()),
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = root_patch_source("'//:special.patch'");
    let workspace = workspace();
    let special = workspace.join("special.patch");
    let special = special.to_str().unwrap();
    let base = [
        lstat_observation(
            "/",
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
        lstat_observation(
            workspace.to_str().unwrap(),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ),
    ];
    let special_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
        ),
        file_bytes_observation(special, PathOperationResult::Present(Arc::from(&b""[..]))),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        special_epoch,
        &["file:///registry"],
        1,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Ok(ModuleSourcePreparation::Registry { bytes, .. }) if bytes.as_ref() == original.as_ref())
    ));

    let relative_target = workspace.join("relative-special.patch");
    let relative_target = relative_target.to_str().unwrap();
    let relative_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ),
        read_link_observation(
            special,
            PathOperationResult::Present(Arc::new(PathBuf::from("relative-special.patch"))),
        ),
        lstat_observation(
            relative_target,
            PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
        ),
        file_bytes_observation(
            relative_target,
            PathOperationResult::Present(Arc::from(&b""[..])),
        ),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        relative_epoch,
        &["file:///registry"],
        2,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Ok(ModuleSourcePreparation::Registry { bytes, .. }) if bytes.as_ref() == original.as_ref())
    ));

    let escaped_special = "/outside-special.patch";
    let escaped_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ),
        read_link_observation(
            special,
            PathOperationResult::Present(Arc::new(PathBuf::from(escaped_special))),
        ),
        lstat_observation(
            escaped_special,
            PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
        ),
        file_bytes_observation(
            escaped_special,
            PathOperationResult::Present(Arc::from(&b""[..])),
        ),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        escaped_epoch,
        &["file:///registry"],
        3,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Ok(ModuleSourcePreparation::Registry { bytes, .. }) if bytes.as_ref() == original.as_ref())
    ));

    let directory_epoch =
        PathObservationEpoch::new(base.iter().cloned().chain([lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        )]))
        .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        directory_epoch,
        &["file:///registry"],
        4,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchWrongKind {
                logical_path,
                actual: PathNodeKind::Directory,
            }) if logical_path.as_path() == Path::new(special))
    ));

    let missing_target = workspace.join("missing-target.patch");
    let missing_target = missing_target.to_str().unwrap();
    let dangling_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ),
        read_link_observation(
            special,
            PathOperationResult::Present(Arc::new(PathBuf::from("missing-target.patch"))),
        ),
        lstat_observation(missing_target, PathOperationResult::Missing),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        dangling_epoch,
        &["file:///registry"],
        5,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchMissing { logical_path })
                if logical_path.as_path() == Path::new(special))
    ));

    let readlink_missing_race = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ),
        read_link_observation(special, PathOperationResult::Missing),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        readlink_missing_race,
        &["file:///registry"],
        6,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchResolution(
                PathResolutionError::InconsistentState {
                    namespace,
                    requested_path,
                    demand: actual_demand,
                    before: Some(before),
                    after: None,
                }
            )) if *namespace == PathObservationNamespace::Host
                && requested_path.as_path() == Path::new(special)
                && actual_demand == &demand(special, PathObservationOperation::ReadLink)
                && *before == lstat(PathNodeKind::Symlink))
    ));

    let lstat_error = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let lstat_error_epoch =
        PathObservationEpoch::new(base.iter().cloned().chain([lstat_observation(
            special,
            PathOperationResult::Error(lstat_error),
        )]))
        .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        lstat_error_epoch,
        &["file:///registry"],
        7,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchResolution(
                PathResolutionError::Observation { namespace, requested_path, demand: actual_demand, error }
            )) if *namespace == PathObservationNamespace::Host
                && requested_path.as_path() == Path::new(special)
                && actual_demand == &demand(special, PathObservationOperation::Lstat)
                && *error == lstat_error)
    ));

    let readlink_error = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::InvalidData,
        raw_os_error: Some(22),
    };
    let readlink_error_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::Symlink)),
        ),
        read_link_observation(special, PathOperationResult::Error(readlink_error)),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        readlink_error_epoch,
        &["file:///registry"],
        8,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchResolution(
                PathResolutionError::Observation { namespace, requested_path, demand: actual_demand, error }
            )) if *namespace == PathObservationNamespace::Host
                && requested_path.as_path() == Path::new(special)
                && actual_demand == &demand(special, PathObservationOperation::ReadLink)
                && *error == readlink_error)
    ));

    let error = PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    let regular_epoch = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
        ),
        file_bytes_observation(special, PathOperationResult::Error(error)),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        regular_epoch,
        &["file:///registry"],
        9,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchFileObservation { error: actual, .. }) if *actual == error)
    ));

    let missing_after_lstat = PathObservationEpoch::new(base.iter().cloned().chain([
        lstat_observation(
            special,
            PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
        ),
        file_bytes_observation(special, PathOperationResult::Missing),
    ]))
    .unwrap();
    let outcome = prepare_with_epoch(
        &dice,
        &root,
        raw_workspace_snapshot([]),
        missing_after_lstat,
        &["file:///registry"],
        10,
        "1.0.0",
    )
    .await;
    assert!(matches!(
        outcome,
        PathOutcome::Complete(prepared)
            if matches!(prepared.as_ref(), Err(ModuleSourcePreparationError::PatchFileInconsistentState {
                demand: actual_demand,
                before: Some(before),
                after: None,
            }) if actual_demand == &demand(special, PathObservationOperation::FileBytes)
                && *before == lstat(PathNodeKind::RegularFile))
    ));
}

#[tokio::test]
async fn ordered_patches_apply_strip_while_nonmain_and_commands_stay_inactive() {
    let module_url = "https://registry.invalid/modules/dep/1.0.0/MODULE.bazel";
    let original: Arc<[u8]> = Arc::from(&b"value = 'base'\n"[..]);
    let io = Arc::new(FakeRegistryIo::new([(
        module_url,
        FakeRegistryResponse::Found(original.clone()),
    )]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io);
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let prepared = prepare(
        &dice,
        "module(name = 'root')\n\
         bazel_dep(name = 'dep', version = '1.0.0')\n\
         bazel_dep(name = 'visible', version = '1.0.0')\n\
         single_version_override(module_name = 'dep', patches = ['//:one.patch', '@@visible//:ignored.patch', '//:two.patch'], patch_cmds = ['exit 37'], patch_strip = 1)\n",
        raw_workspace_snapshot([
            (
                "one.patch",
                WorkspaceRawFileValue::Present(Arc::from(
                    &b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-value = 'base'\n+value = 'middle'\n"[..],
                )),
            ),
            (
                "two.patch",
                WorkspaceRawFileValue::Present(Arc::from(
                    &b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-value = 'middle'\n+value = 'final'\n"[..],
                )),
            ),
        ]),
        &["https://registry.invalid"],
        1,
        "1.0.0",
    )
    .await;

    let Ok(ModuleSourcePreparation::Registry {
        bytes,
        module_file_attempts,
        ..
    }) = prepared.as_ref()
    else {
        panic!("expected patched registry preparation: {prepared:?}");
    };
    assert_eq!(bytes.as_ref(), b"value = 'final'\n");
    assert_eq!(module_file_attempts.len(), 1);
    assert_eq!(module_file_attempts[0].url.as_str(), module_url);
    assert_eq!(
        module_file_attempts[0].sha256,
        Some(Sha256::digest(original.as_ref()).into())
    );
}

#[tokio::test]
async fn empty_version_and_registry_exhaustion_are_typed_failures() {
    let first = "https://first.invalid/modules/dep/1.0.0/MODULE.bazel";
    let second = "https://second.invalid/modules/dep/1.0.0/MODULE.bazel";
    let io = Arc::new(FakeRegistryIo::new([
        (first, FakeRegistryResponse::NotFound),
        (second, FakeRegistryResponse::NotFound),
    ]));
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = Arc::new(builder.build(DetectCycles::Enabled));
    let root = "module(name = 'root')\nbazel_dep(name = 'dep', version = '1.0.0')\n";

    let missing_version = prepare(
        &dice,
        root,
        raw_workspace_snapshot([]),
        &["https://first.invalid", "https://second.invalid"],
        1,
        "",
    )
    .await;
    assert!(matches!(
        missing_version.as_ref(),
        Err(ModuleSourcePreparationError::MissingVersion)
    ));
    assert!(io.calls().is_empty());

    let exhausted = prepare(
        &dice,
        root,
        raw_workspace_snapshot([]),
        &["https://first.invalid", "https://second.invalid"],
        2,
        "1.0.0",
    )
    .await;
    let Err(ModuleSourcePreparationError::ModuleNotFound {
        module_file_attempts,
    }) = exhausted.as_ref()
    else {
        panic!("expected ordered registry exhaustion: {exhausted:?}");
    };
    assert_eq!(module_file_attempts.len(), 2);
    assert_eq!(module_file_attempts[0].url.as_str(), first);
    assert_eq!(module_file_attempts[0].sha256, None);
    assert_eq!(module_file_attempts[1].url.as_str(), second);
    assert_eq!(module_file_attempts[1].sha256, None);
    assert_eq!(io.calls(), [first, second]);
}

#[test]
fn bounded_patcher_preserves_raw_bytes_without_a_patch_and_applies_ordered_hunks() {
    let original: Arc<[u8]> = Arc::from(&b"module(name = 'a')\n"[..]);
    let first = apply_unified_patch(
        original.clone(),
        b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-module(name = 'a')\n+module(name = 'b')\n",
        1,
    )
    .unwrap();
    let second = apply_unified_patch(
        first,
        b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-module(name = 'b')\n+module(name = 'c')\n",
        1,
    )
    .unwrap();
    assert_eq!(second.as_ref(), b"module(name = 'c')\n");
    assert!(apply_unified_patch(original, b"not a patch", 0).is_err());
    assert!(apply_unified_patch(
        Arc::from(&b"module(name = 'a')\n"[..]),
        b"--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1 +1 @@\n-module(name = 'a')\n+module(name = 'b')\n",
        0,
    )
    .is_err());
    assert!(apply_unified_patch(
        Arc::from(&b"module(name = 'a')\n"[..]),
        b"--- MODULE.bazel\n+++ MODULE.bazel\n@@ -1,2 +1,1 @@\n-module(name = 'a')\n+module(name = 'b')\n",
        -1,
    )
    .is_err());
    assert_eq!(
        apply_unified_patch(
            Arc::from(&b"module(name = 'a')\n"[..]),
            b"--- MODULE.bazel\n+++ MODULE.bazel\n@@ -1 +1 @@\n-module(name = 'a')\n+module(name = 'b')\n",
            -1,
        )
        .unwrap()
        .as_ref(),
        b"module(name = 'b')\n"
    );
    assert_eq!(
        apply_unified_patch(
            Arc::from(&b"module(name = 'a')\n"[..]),
            b"--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
            1,
        )
        .unwrap()
        .as_ref(),
        b"module(name = 'a')\n"
    );
    assert_eq!(
        apply_unified_patch(
            Arc::from(&b"module(name = 'a')\n"[..]),
            b"--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
            0,
        )
        .unwrap()
        .as_ref(),
        b"module(name = 'a')\n"
    );
    assert!(
        apply_unified_patch(
            Arc::from(&b"module(name = 'a')\n"[..]),
            b"--- a/README.md\n+++ b/README.md\n@@ -1,2 +1 @@\n-old\n+new\n",
            1,
        )
        .is_err()
    );
    assert!(apply_unified_patch(
        Arc::from(&b"module(name = 'a')\n"[..]),
        b"--- MODULE.bazel\n+++ MODULE.bazel\n@@ -1 +1 bogus @@\n-module(name = 'a')\n+module(name = 'b')\n",
        0,
    )
    .is_err());
}
