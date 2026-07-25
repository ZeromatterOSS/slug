use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;
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
        WorkspaceRawFileValue::Present(Arc::from(
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
        ))
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

    for (generation, value, expected) in [
        (1, patch("leaf_a"), Some("leaf_a")),
        (2, patch("leaf_b"), Some("leaf_b")),
        (3, WorkspaceRawFileValue::Absent, None),
        (
            4,
            WorkspaceRawFileValue::Present(Arc::from(&b"not a patch"[..])),
            None,
        ),
        (5, patch("leaf_b"), Some("leaf_b")),
        (6, patch("leaf_a"), Some("leaf_a")),
    ] {
        let prepared = prepare(
            &dice,
            root,
            raw_workspace_snapshot([("route.patch", value)]),
            &["file:///registry"],
            generation,
            "1.0.0",
        )
        .await;
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
            }
            None => assert!(matches!(
                prepared.as_ref(),
                Err(ModuleSourcePreparationError::Patch(_))
            )),
        }
    }
    assert_eq!(io.calls(), [module_url]);
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
