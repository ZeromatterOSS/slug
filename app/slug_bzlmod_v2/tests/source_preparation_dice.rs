use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryIo;
use slug_bzlmod_v2::RepositoryIoOutcome;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationGenerationKey;
use slug_bzlmod_v2::RepositorySourceFileKey;
use slug_bzlmod_v2::RepositorySourceFileValue;
use slug_bzlmod_v2::RepositoryTransportError;
use slug_bzlmod_v2::inject_root_module_request_inputs;
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
