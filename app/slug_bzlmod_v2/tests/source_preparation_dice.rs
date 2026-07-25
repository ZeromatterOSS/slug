use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use dice::UserComputationData;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::ModuleSourcePreparation;
use slug_bzlmod_v2::ModuleSourcePreparationError;
use slug_bzlmod_v2::ModuleSourcePreparationKey;
use slug_bzlmod_v2::RegistryFileError;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryIo;
use slug_bzlmod_v2::RepositoryIoOutcome;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationGenerationKey;
use slug_bzlmod_v2::RepositorySourceFileKey;
use slug_bzlmod_v2::RepositorySourceFileValue;
use slug_bzlmod_v2::RepositoryTransportError;
use slug_bzlmod_v2::apply_unified_patch;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::install_registry_io;
use slug_bzlmod_v2::install_repository_io;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
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

async fn source(
    dice: &Arc<Dice>,
    raw: Arc<WorkspaceRawSnapshot>,
    generation: u64,
    repo_relative_path: &str,
) -> RepositorySourceFileValue {
    let workspace = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            (WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            }),
            text_snapshot(),
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
    transaction
        .compute(&RepositorySourceFileKey {
            workspace,
            module_name: "dep".into(),
            repo_relative_path: PathBuf::from(repo_relative_path),
        })
        .await
        .unwrap()
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
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            text_snapshot_with(root_source),
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
    transaction
        .compute(&ModuleSourcePreparationKey {
            workspace,
            module_name: "dep".into(),
            version: version.into(),
        })
        .await
        .unwrap()
}

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
}

fn lstat(kind: PathNodeKind) -> PathLstat {
    PathLstat::new(kind, 1, 2, 3, 4, 0o644)
}

fn complete_epoch_for_raw(raw: &WorkspaceRawSnapshot) -> PathObservationEpoch {
    let workspace = workspace();
    let mut observations = vec![
        (
            demand("/", PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                PathNodeKind::Directory,
            ))),
        ),
        (
            demand(workspace.to_str().unwrap(), PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                PathNodeKind::Directory,
            ))),
        ),
    ];
    for (file, value) in raw.files.iter() {
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
        observations.push((
            demand(file, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(lstat_result),
        ));
        if let WorkspaceRawFileValue::Present(bytes) = value {
            observations.push((
                demand(file, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes.dupe())),
            ));
        }
    }
    PathObservationEpoch::new(observations).unwrap()
}

fn lstat_observation(
    path: &str,
    result: PathOperationResult<PathLstat>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand(path, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(result),
    )
}

fn file_bytes_observation(
    path: &str,
    result: PathOperationResult<Arc<[u8]>>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand(path, PathObservationOperation::FileBytes),
        PathObservationResult::FileBytes(result),
    )
}

fn read_link_observation(
    path: &str,
    result: PathOperationResult<Arc<PathBuf>>,
) -> (PathObservationDemand, PathObservationResult) {
    (
        demand(path, PathObservationOperation::ReadLink),
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
        Ok(RepositorySourceFileValue::Present(Arc::from(
            &b"module(name = 'first')"[..]
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
            2,
            "parts/dep.MODULE.bazel",
        )
        .await,
        RepositorySourceFileValue::Present(Arc::from(&b"bazel_dep(name = 'b')"[..]))
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
        RepositorySourceFileValue::Present(Arc::from(&b"module(name = 'second')"[..]))
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
        RepositorySourceFileValue::Present(Arc::from(&b"module(name = 'first')"[..]))
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 1);

    assert_eq!(
        source(
            &dice,
            raw_snapshot([("MODULE.bazel", WorkspaceRawFileValue::Absent)]),
            5,
            "MODULE.bazel",
        )
        .await,
        RepositorySourceFileValue::Absent
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
        RepositorySourceFileValue::ReadError(message) if message.as_ref() == "denied"
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

    assert!(matches!(
        source(&dice, raw.clone(), 1, "MODULE.bazel").await,
        RepositorySourceFileValue::ReadError(message) if message.as_ref().contains("Transport")
    ));
    assert_eq!(
        source(&dice, raw, 2, "MODULE.bazel").await,
        RepositorySourceFileValue::Present(Arc::from(&b"module(name = 'recovered')"[..]))
    );
    assert_eq!(io.calls.load(Ordering::SeqCst), 2);
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
    assert_eq!(repository_io.calls.load(Ordering::SeqCst), 1);
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
            let need = PathOutcome::Need(need);
            assert!(!ModuleSourcePreparationKey::validity(&need));
            assert!(!ModuleSourcePreparationKey::equality(&need, &need));
        } else {
            let PathOutcome::Complete(prepared) = outcome else {
                panic!("complete epoch unexpectedly needs observations");
            };
            let complete = PathOutcome::Complete(prepared.clone());
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
