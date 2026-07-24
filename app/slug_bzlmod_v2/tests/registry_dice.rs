use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceTransaction;
use dice::UserComputationData;
use serde_json::json;
use sha2::Digest;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RegistryFileError;
use slug_bzlmod_v2::RegistryFileKey;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryFileValue;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryNotFoundSource;
use slug_bzlmod_v2::RegistryPolicyKey;
use slug_bzlmod_v2::RegistryRequestGeneration;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::RegistryUrls;
use slug_bzlmod_v2::RootModuleFilesKey;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::inject_registry_request_inputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::install_registry_io;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::sorted_map::SortedMap;

const REMOTE_URL: &str = "https://registry.example/modules/demo/1.0/MODULE.bazel";

#[derive(Debug, Clone)]
enum FakeResponse {
    Found(&'static [u8]),
    NotFound,
    Transport(&'static str),
}

#[derive(Debug)]
struct FakeRegistryIo {
    response: Mutex<FakeResponse>,
    calls: AtomicUsize,
}

impl FakeRegistryIo {
    fn new(response: FakeResponse) -> Self {
        Self {
            response: Mutex::new(response),
            calls: AtomicUsize::new(0),
        }
    }

    fn set_response(&self, response: FakeResponse) {
        *self.response.lock().unwrap() = response;
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RegistryIo for FakeRegistryIo {
    async fn read_exact(
        &self,
        _url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.response.lock().unwrap().clone() {
            FakeResponse::Found(bytes) => Ok(RegistryIoOutcome::Found(Arc::from(bytes))),
            FakeResponse::NotFound => Ok(RegistryIoOutcome::NotFound),
            FakeResponse::Transport(message) => Err(RegistryTransportError {
                message: message.into(),
            }),
        }
    }
}

fn workspace() -> PathBuf {
    PathBuf::from("/registry-dice-test")
}

fn lockfile(expectation: Option<&str>) -> String {
    let registry_file_hashes = expectation
        .map(|value| json!({ REMOTE_URL: value }))
        .unwrap_or_else(|| json!({}));
    json!({
        "lockFileVersion": 28,
        "registryFileHashes": registry_file_hashes,
    })
    .to_string()
}

fn snapshot(
    lockfile: Option<&str>,
    extra: impl IntoIterator<Item = (PathBuf, WorkspaceFileValue)>,
) -> Arc<WorkspaceSnapshot> {
    snapshot_with_root("module(name = 'root')", lockfile, extra)
}

fn snapshot_with_root(
    root_module: &str,
    lockfile: Option<&str>,
    extra: impl IntoIterator<Item = (PathBuf, WorkspaceFileValue)>,
) -> Arc<WorkspaceSnapshot> {
    let root = workspace();
    let mut files = vec![(
        root.join("MODULE.bazel"),
        WorkspaceFileValue::Present(Arc::new(root_module.to_owned())),
    )];
    if let Some(lockfile) = lockfile {
        files.push((
            root.join("MODULE.bazel.lock"),
            WorkspaceFileValue::Present(Arc::new(lockfile.to_owned())),
        ));
    }
    files.extend(extra);
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(files.into_iter().collect::<SortedMap<_, _>>()),
    })
}

fn dice_with_io(io: Arc<FakeRegistryIo>) -> Arc<Dice> {
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io);
    builder.build(DetectCycles::Enabled)
}

async fn transaction(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    mode: LockfileMode,
    generation: u64,
    urls: RegistryUrls,
) -> DiceTransaction {
    let root = workspace();
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: root.clone(),
            },
            files,
        )])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &root,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        mode,
    )
    .unwrap();
    inject_registry_request_inputs(
        &mut updater,
        &root,
        urls,
        RegistryRequestGeneration(generation),
    )
    .unwrap();
    updater.commit().await
}

async fn remote_value(
    transaction: &mut DiceTransaction,
) -> Arc<Result<RegistryFileValue, RegistryFileError>> {
    transaction
        .compute(&RegistryFileKey {
            workspace: workspace(),
            url: RegistryFileUrl::new(REMOTE_URL),
        })
        .await
        .unwrap()
}

async fn local_value(
    transaction: &mut DiceTransaction,
    url: &RegistryFileUrl,
) -> Arc<Result<RegistryFileValue, RegistryFileError>> {
    transaction
        .compute(&RegistryFileKey {
            workspace: workspace(),
            url: url.clone(),
        })
        .await
        .unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(bytes))
}

#[test]
fn registry_urls_trim_trailing_slashes_and_preserve_first_occurrence_order() {
    let urls = RegistryUrls::new([
        "https://first.example///",
        "https://second.example/",
        "https://first.example",
    ]);
    let actual = urls
        .as_slice()
        .iter()
        .map(|url| url.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec!["https://first.example", "https://second.example"]
    );
}

#[tokio::test]
async fn registry_policy_matches_lockfile_mode_matrix_before_io() {
    let cases = [
        (
            LockfileMode::Update,
            Some("not found"),
            false,
            RegistryNotFoundSource::RecordedAbsence,
        ),
        (
            LockfileMode::Refresh,
            Some("not found"),
            true,
            RegistryNotFoundSource::Io404,
        ),
        (
            LockfileMode::Off,
            Some("not found"),
            true,
            RegistryNotFoundSource::Io404,
        ),
    ];
    for (mode, expectation, should_fetch, source) in cases {
        let io = Arc::new(FakeRegistryIo::new(FakeResponse::NotFound));
        let dice = dice_with_io(io.clone());
        let lock = lockfile(expectation);
        let mut tx = transaction(
            &dice,
            snapshot(Some(&lock), []),
            mode,
            1,
            RegistryUrls::default_bazel_registry(),
        )
        .await;
        assert_eq!(
            remote_value(&mut tx).await.as_ref(),
            &Ok(RegistryFileValue::NotFound {
                source,
                recordable_remote_expectation: Some(
                    slug_bzlmod_v2::RegistryFileExpectation::RecordedAbsent
                ),
            })
        );
        assert_eq!(io.calls(), usize::from(should_fetch));
    }

    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Found(b"unused")));
    let dice = dice_with_io(io.clone());
    let lock = lockfile(None);
    let mut tx = transaction(
        &dice,
        snapshot(Some(&lock), []),
        LockfileMode::Error,
        1,
        RegistryUrls::default_bazel_registry(),
    )
    .await;
    assert!(matches!(
        remote_value(&mut tx).await.as_ref(),
        Err(RegistryFileError::MissingChecksumInError { .. })
    ));
    assert_eq!(io.calls(), 0);
}

#[tokio::test]
async fn unrecorded_remote_outcomes_retry_only_after_generation_changes() {
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Transport("offline")));
    let dice = dice_with_io(io.clone());
    let files = snapshot(Some(&lockfile(None)), []);
    let urls = RegistryUrls::default_bazel_registry();

    let mut tx = transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut tx).await.as_ref(),
        Err(RegistryFileError::Transport { .. })
    ));
    io.set_response(FakeResponse::Found(b"now available"));
    let mut same_generation =
        transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut same_generation).await.as_ref(),
        Err(RegistryFileError::Transport { .. })
    ));
    assert_eq!(io.calls(), 1);

    let mut next_generation = transaction(&dice, files, LockfileMode::Update, 2, urls).await;
    assert!(matches!(
        remote_value(&mut next_generation).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"now available"
    ));
    assert_eq!(io.calls(), 2);
}

#[tokio::test]
async fn known_sha_failure_acquires_generation_but_verified_success_drops_it() {
    let expected_bytes = b"expected content";
    let digest = sha256_hex(expected_bytes);
    let lock = lockfile(Some(&digest));
    let files = snapshot(Some(&lock), []);
    let urls = RegistryUrls::default_bazel_registry();
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::NotFound));
    let dice = dice_with_io(io.clone());

    let mut first = transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut first).await.as_ref(),
        Ok(RegistryFileValue::NotFound {
            source: RegistryNotFoundSource::Io404,
            ..
        })
    ));
    io.set_response(FakeResponse::Found(expected_bytes));
    let mut same_generation =
        transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut same_generation).await.as_ref(),
        Ok(RegistryFileValue::NotFound { .. })
    ));
    assert_eq!(io.calls(), 1);

    let mut retry = transaction(&dice, files.clone(), LockfileMode::Update, 2, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut retry).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected_bytes
    ));
    assert_eq!(io.calls(), 2);

    io.set_response(FakeResponse::Transport("would fail if retried"));
    let mut later_generation = transaction(&dice, files, LockfileMode::Update, 3, urls).await;
    assert!(matches!(
        remote_value(&mut later_generation).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected_bytes
    ));
    assert_eq!(io.calls(), 2);
}

#[tokio::test]
async fn known_sha_transport_failure_retries_after_generation_changes() {
    let expected_bytes = b"expected after transport recovery";
    let digest = sha256_hex(expected_bytes);
    let lock = lockfile(Some(&digest));
    let files = snapshot(Some(&lock), []);
    let urls = RegistryUrls::default_bazel_registry();
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Transport("offline")));
    let dice = dice_with_io(io.clone());

    let mut first = transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut first).await.as_ref(),
        Err(RegistryFileError::Transport { .. })
    ));
    io.set_response(FakeResponse::Found(expected_bytes));
    let mut same_generation =
        transaction(&dice, files.clone(), LockfileMode::Update, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut same_generation).await.as_ref(),
        Err(RegistryFileError::Transport { .. })
    ));
    assert_eq!(io.calls(), 1);

    let mut retry = transaction(&dice, files, LockfileMode::Update, 2, urls).await;
    assert!(matches!(
        remote_value(&mut retry).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected_bytes
    ));
    assert_eq!(io.calls(), 2);

    io.set_response(FakeResponse::Transport("would fail if retried"));
    let mut later_generation = transaction(
        &dice,
        snapshot(Some(&lockfile(Some(&digest))), []),
        LockfileMode::Update,
        3,
        RegistryUrls::default_bazel_registry(),
    )
    .await;
    assert!(matches!(
        remote_value(&mut later_generation).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected_bytes
    ));
    assert_eq!(io.calls(), 2);
}

#[tokio::test]
async fn checksum_mismatch_is_typed_and_stable_for_the_same_expectation() {
    let digest = sha256_hex(b"wanted");
    let lock = lockfile(Some(&digest));
    let files = snapshot(Some(&lock), []);
    let urls = RegistryUrls::default_bazel_registry();
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Found(b"wrong")));
    let dice = dice_with_io(io.clone());

    let mut first = transaction(&dice, files.clone(), LockfileMode::Error, 1, urls.clone()).await;
    assert!(matches!(
        remote_value(&mut first).await.as_ref(),
        Err(RegistryFileError::ChecksumMismatch { .. })
    ));
    io.set_response(FakeResponse::Found(b"wanted"));
    let mut next_generation = transaction(&dice, files, LockfileMode::Error, 2, urls).await;
    assert!(matches!(
        remote_value(&mut next_generation).await.as_ref(),
        Err(RegistryFileError::ChecksumMismatch { .. })
    ));
    assert_eq!(io.calls(), 1);
}

#[tokio::test]
async fn local_absence_and_read_error_retry_only_after_generation_changes() {
    let local_path = workspace().join("registry/modules/demo/MODULE.bazel");
    let local_url = RegistryFileUrl::new(format!("file://{}", local_path.display()));
    let urls = RegistryUrls::new(["file:///registry-dice-test/registry"]);
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::NotFound));
    let dice = dice_with_io(io.clone());

    let files = snapshot(None, []);
    let mut absent = transaction(&dice, files.clone(), LockfileMode::Off, 1, urls.clone()).await;
    assert!(matches!(
        local_value(&mut absent, &local_url).await.as_ref(),
        Ok(RegistryFileValue::NotFound {
            source: RegistryNotFoundSource::LocalAbsence,
            recordable_remote_expectation: None,
        })
    ));
    assert_eq!(io.calls(), 1);

    io.set_response(FakeResponse::Found(b"created"));
    let mut same_generation =
        transaction(&dice, files.clone(), LockfileMode::Off, 1, urls.clone()).await;
    assert!(matches!(
        local_value(&mut same_generation, &local_url).await.as_ref(),
        Ok(RegistryFileValue::NotFound {
            source: RegistryNotFoundSource::LocalAbsence,
            ..
        })
    ));
    assert_eq!(io.calls(), 1);

    let mut created = transaction(&dice, files.clone(), LockfileMode::Off, 2, urls.clone()).await;
    assert!(matches!(
        local_value(&mut created, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, recordable_remote_expectation: None, .. })
            if bytes.as_ref() == b"created"
    ));
    assert_eq!(io.calls(), 2);

    io.set_response(FakeResponse::Transport("permission denied"));
    let root_b = snapshot_with_root("module(name = 'root', version = '0.2')", None, []);
    let mut read_error =
        transaction(&dice, root_b.clone(), LockfileMode::Off, 3, urls.clone()).await;
    assert!(matches!(
        local_value(&mut read_error, &local_url).await.as_ref(),
        Err(RegistryFileError::LocalRead { message, .. }) if message == "permission denied"
    ));
    assert_eq!(io.calls(), 3);

    io.set_response(FakeResponse::Found(b"repaired"));
    let mut same_error_generation =
        transaction(&dice, root_b.clone(), LockfileMode::Off, 3, urls.clone()).await;
    assert!(matches!(
        local_value(&mut same_error_generation, &local_url)
            .await
            .as_ref(),
        Err(RegistryFileError::LocalRead { .. })
    ));
    assert_eq!(io.calls(), 3);

    let mut repaired = transaction(&dice, root_b, LockfileMode::Off, 4, urls).await;
    assert!(matches!(
        local_value(&mut repaired, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"repaired"
    ));
    assert_eq!(io.calls(), 4);
}

#[tokio::test]
async fn local_success_sticks_across_raw_mutations_with_equal_semantic_inputs() {
    let local_path = workspace().join("registry/modules/demo/MODULE.bazel");
    let local_url = RegistryFileUrl::new(format!("file://{}", local_path.display()));
    let urls = RegistryUrls::new(["file:///registry-dice-test/registry"]);
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Found(b"first")));
    let dice = dice_with_io(io.clone());
    let first_root = snapshot_with_root("module(name = 'root', version = '0.1')", None, []);

    let mut first = transaction(&dice, first_root, LockfileMode::Off, 1, urls.clone()).await;
    assert!(matches!(
        local_value(&mut first, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"first"
    ));

    io.set_response(FakeResponse::Found(b"def broken("));
    let comment_only_root = snapshot_with_root(
        "# changed comment\nmodule(name = 'root', version = '0.1')",
        None,
        [],
    );
    let mut malformed = transaction(
        &dice,
        comment_only_root.clone(),
        LockfileMode::Off,
        2,
        urls.clone(),
    )
    .await;
    assert!(matches!(
        local_value(&mut malformed, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"first"
    ));

    io.set_response(FakeResponse::NotFound);
    let mut deleted = transaction(&dice, comment_only_root, LockfileMode::Off, 3, urls).await;
    assert!(matches!(
        local_value(&mut deleted, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"first"
    ));
    assert_eq!(io.calls(), 1);
}

#[tokio::test]
async fn local_success_rereads_for_root_semantics_and_ordered_registry_inputs() {
    let local_path = workspace().join("registry/modules/demo/MODULE.bazel");
    let local_url = RegistryFileUrl::new(format!("file://{}", local_path.display()));
    let urls_ab = RegistryUrls::new([
        "file:///registry-dice-test/registry/a",
        "file:///registry-dice-test/registry/b",
    ]);
    let urls_ba = RegistryUrls::new([
        "file:///registry-dice-test/registry/b",
        "file:///registry-dice-test/registry/a",
    ]);
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Found(b"root-a")));
    let dice = dice_with_io(io.clone());

    let root_a = snapshot_with_root("module(name = 'root', version = '0.1')", None, []);
    let mut first = transaction(&dice, root_a, LockfileMode::Off, 1, urls_ab.clone()).await;
    assert!(matches!(
        local_value(&mut first, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"root-a"
    ));

    io.set_response(FakeResponse::Found(b"root-b"));
    let root_b = snapshot_with_root("module(name = 'root', version = '0.2')", None, []);
    let mut second = transaction(&dice, root_b, LockfileMode::Off, 2, urls_ab.clone()).await;
    assert!(matches!(
        local_value(&mut second, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"root-b"
    ));

    io.set_response(FakeResponse::Found(b"root-c"));
    let root_c = snapshot_with_root("module(name = 'root', version = '0.3')", None, []);
    let mut third = transaction(&dice, root_c.clone(), LockfileMode::Off, 3, urls_ab.clone()).await;
    assert!(matches!(
        local_value(&mut third, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"root-c"
    ));

    io.set_response(FakeResponse::Found(b"registry-ba"));
    let mut reordered = transaction(&dice, root_c.clone(), LockfileMode::Off, 4, urls_ba).await;
    assert!(matches!(
        local_value(&mut reordered, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"registry-ba"
    ));

    io.set_response(FakeResponse::Found(b"registry-ab"));
    let mut restored = transaction(&dice, root_c, LockfileMode::Off, 5, urls_ab).await;
    assert!(matches!(
        local_value(&mut restored, &local_url).await.as_ref(),
        Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"registry-ab"
    ));
    assert_eq!(io.calls(), 5);
}

#[tokio::test]
async fn remote_io_fails_closed_when_required_inputs_or_capability_are_missing() {
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::Found(b"unused")));
    let dice = dice_with_io(io.clone());
    let root = workspace();
    let lock = lockfile(None);
    let mut updater = dice.updater_with_data(UserComputationData::default());
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: root.clone(),
            },
            snapshot(Some(&lock), []),
        )])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &root,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater
        .changed_to(vec![(
            slug_bzlmod_v2::RootModuleRegistryUrlsKey {
                workspace: root.clone(),
            },
            slug_bzlmod_v2::RootModuleRegistryUrls::from(RegistryUrls::default_bazel_registry()),
        )])
        .unwrap();
    let mut missing_generation = updater.commit().await;
    assert!(matches!(
        remote_value(&mut missing_generation).await.as_ref(),
        Err(RegistryFileError::MissingRequestGeneration(_))
    ));
    assert_eq!(io.calls(), 0);

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut no_capability = transaction(
        &dice,
        snapshot(Some(&lock), []),
        LockfileMode::Update,
        1,
        RegistryUrls::default_bazel_registry(),
    )
    .await;
    assert!(matches!(
        remote_value(&mut no_capability).await.as_ref(),
        Err(RegistryFileError::MissingIoCapability)
    ));
}

#[tokio::test]
async fn registry_inputs_do_not_invalidate_root_module_files_or_graph() {
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::NotFound));
    let dice = dice_with_io(io);
    let files = snapshot(Some(&lockfile(None)), []);
    let mut first = transaction(
        &dice,
        files.clone(),
        LockfileMode::Update,
        1,
        RegistryUrls::new(["https://first.example"]),
    )
    .await;
    let first_files = first
        .compute(&RootModuleFilesKey {
            workspace: workspace(),
        })
        .await
        .unwrap();
    let first_graph = first
        .compute(&RootModuleGraphKey {
            workspace: workspace(),
        })
        .await
        .unwrap();

    let mut second = transaction(
        &dice,
        files,
        LockfileMode::Update,
        2,
        RegistryUrls::new(["https://second.example"]),
    )
    .await;
    let second_files = second
        .compute(&RootModuleFilesKey {
            workspace: workspace(),
        })
        .await
        .unwrap();
    let second_graph = second
        .compute(&RootModuleGraphKey {
            workspace: workspace(),
        })
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&first_files, &second_files));
    assert!(Arc::ptr_eq(&first_graph, &second_graph));
}

#[tokio::test]
async fn registry_policy_tracks_ordered_urls_across_a_b_a_requests() {
    let io = Arc::new(FakeRegistryIo::new(FakeResponse::NotFound));
    let dice = dice_with_io(io);
    let files = snapshot(Some(&lockfile(None)), []);
    let policy_key = RegistryPolicyKey {
        workspace: workspace(),
    };

    let mut first = transaction(
        &dice,
        files.clone(),
        LockfileMode::Update,
        1,
        RegistryUrls::new(["https://first.example", "https://second.example"]),
    )
    .await;
    let first = first.compute(&policy_key).await.unwrap();
    let first = first.as_ref().as_ref().unwrap();
    assert_eq!(first.urls().as_slice()[0].as_str(), "https://first.example");

    let mut middle = transaction(
        &dice,
        files.clone(),
        LockfileMode::Update,
        2,
        RegistryUrls::new(["https://second.example", "https://first.example"]),
    )
    .await;
    let middle = middle.compute(&policy_key).await.unwrap();
    let middle = middle.as_ref().as_ref().unwrap();
    assert_ne!(first, middle);
    assert_eq!(
        middle.urls().as_slice()[0].as_str(),
        "https://second.example"
    );

    let mut last = transaction(
        &dice,
        files,
        LockfileMode::Update,
        3,
        RegistryUrls::new(["https://first.example", "https://second.example"]),
    )
    .await;
    let last = last.compute(&policy_key).await.unwrap();
    let last = last.as_ref().as_ref().unwrap();
    assert_eq!(first, last);
}
