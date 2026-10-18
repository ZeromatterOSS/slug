use std::fs;
use std::fs::File;
use std::fs::FileTimes;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use crate::runtime::dice::workspace_status_publication::WorkspaceStatusPublicationRoot;
use crate::runtime::dice::workspace_status_publication::WorkspaceStatusPublicationTerminal;
use crate::runtime::dice::workspace_status_publication::validate_descriptors;
use crate::runtime::dice::workspace_status_publication::validate_publication_namespace;
use crate::runtime::workspace_status_publication_file::StablePublication;
use crate::runtime::workspace_status_publication_file::Staging;

const STABLE: &str = "stable-status.txt";
const VOLATILE: &str = "volatile-status.txt";
const OLD_STABLE: &[u8] = b"old stable\n";
const OLD_VOLATILE: &[u8] = b"old volatile\n";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint(u64, u64, u32, u64, i64, i64, i64, i64);
fn fingerprint(path: &Path) -> Fingerprint {
    let m = fs::symlink_metadata(path).unwrap();
    Fingerprint(
        m.dev(),
        m.ino(),
        m.mode(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn final_path(root: &Path, name: &str) -> PathBuf {
    root.join("bazel-out").join(name)
}
fn past_time(path: &Path) {
    File::open(path)
        .unwrap()
        .set_times(
            FileTimes::new()
                .set_accessed(std::time::UNIX_EPOCH + std::time::Duration::from_secs(42))
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(42)),
        )
        .unwrap();
}
fn old_files(root: &Path) {
    fs::create_dir_all(root.join("bazel-out")).unwrap();
    fs::write(final_path(root, STABLE), OLD_STABLE).unwrap();
    fs::write(final_path(root, VOLATILE), OLD_VOLATILE).unwrap();
}
fn final_bytes(root: &Path) -> [Option<Vec<u8>>; 2] {
    [STABLE, VOLATILE].map(|name| match fs::read(final_path(root, name)) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("reading test final file: {error}"),
    })
}
fn stage_paths(root: &Path, stage: &Staging) -> [PathBuf; 2] {
    stage.names().map(|name| final_path(root, name))
}
fn no_stage_names(root: &Path) {
    for entry in fs::read_dir(root.join("bazel-out")).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(
            !name.to_string_lossy().starts_with(".slug-status"),
            "unretired stage: {name:?}"
        );
    }
}

#[derive(Clone)]
enum Intervention {
    None,
    EqualVolatile,
    SourceEdit(PathBuf, Vec<u8>),
    Abort,
    FailSecond,
    Stale(Arc<WorkspaceStatusCapture>),
    Copied,
}
struct PublicationAttempt {
    capture: Arc<WorkspaceStatusCapture>,
    stage: std::sync::Weak<Mutex<Staging>>,
    names: [PathBuf; 2],
    visible: [Option<Vec<u8>>; 2],
}
#[derive(Clone)]
struct PublicationProbe {
    root: WorkspaceStatusPublicationRoot,
    intervention: Arc<Mutex<Option<Intervention>>>,
    attempts: Arc<Mutex<Vec<PublicationAttempt>>>,
    publications: Arc<std::sync::atomic::AtomicUsize>,
    equal_before: Arc<Mutex<Option<Fingerprint>>>,
}
impl PublicationProbe {
    fn new(key: BuildInfoNativeInputsKey, intervention: Intervention) -> Self {
        Self {
            root: WorkspaceStatusPublicationRoot(key),
            intervention: Arc::new(Mutex::new(Some(intervention))),
            attempts: Arc::new(Mutex::new(Vec::new())),
            publications: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            equal_before: Arc::new(Mutex::new(None)),
        }
    }
}
#[async_trait]
impl NativeCommandRoot for PublicationProbe {
    type Terminal = WorkspaceStatusPublicationTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.root.observations(value)
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        self.root.source_certificate(value)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.root.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, value: &Self::Terminal) -> EventReconciliationPolicy {
        self.root.event_reconciliation_policy(value)
    }
    fn publication_completed(&self, value: &Self::Terminal) -> bool {
        self.root.publication_completed(value)
    }
    fn publish(&self, value: &mut Self::Terminal) -> std::io::Result<()> {
        self.publications
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.root.publish(value)
    }
    fn complete<'a>(
        &'a self,
        value: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            assert!(matches!(
                context
                    .command
                    .runtime
                    .begin_native_demand_command_with_inputs(
                        NativeDemandRequestInputBundle::normalized_initial()
                    ),
                Err(NativeDemandSessionError::Busy)
            ));
            let root = context.command.runtime.workspace.clone();
            let intervention = self
                .intervention
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Intervention::None);
            match &intervention {
                Intervention::Stale(capture) => validate_capture(capture, &context).await?,
                Intervention::Copied => {
                    validate_capture(
                        &Arc::new(value.inputs.status.capture.as_ref().clone()),
                        &context,
                    )
                    .await?
                }
                Intervention::EqualVolatile => {
                    fs::create_dir_all(root.join("bazel-out")).unwrap();
                    fs::set_permissions(
                        final_path(&root, VOLATILE),
                        fs::Permissions::from_mode(0o644),
                    )
                    .unwrap();
                    fs::write(final_path(&root, VOLATILE), &value.inputs.status.volatile).unwrap();
                    fs::set_permissions(
                        final_path(&root, VOLATILE),
                        fs::Permissions::from_mode(0o444),
                    )
                    .unwrap();
                    past_time(&final_path(&root, VOLATILE));
                    *self.equal_before.lock().unwrap() =
                        Some(fingerprint(&final_path(&root, VOLATILE)));
                }
                _ => {}
            }
            let visible = final_bytes(&root);
            self.root.complete(value, context).await?;
            let stage = value
                .pending
                .as_ref()
                .expect("real completion creates pending stage");
            let names = stage_paths(&root, &stage.lock().unwrap());
            assert_eq!(
                visible,
                final_bytes(&root),
                "preparation never changes visible status files"
            );
            {
                let attempts = self.attempts.lock().unwrap();
                for previous in attempts.iter() {
                    assert!(
                        previous.stage.upgrade().is_none(),
                        "prior attempt stage retired before retry completion"
                    );
                    assert!(previous.names.iter().all(|path| !path.exists()));
                    assert!(Arc::ptr_eq(&previous.capture, &value.inputs.status.capture));
                }
            }
            self.attempts.lock().unwrap().push(PublicationAttempt {
                capture: value.inputs.status.capture.clone(),
                stage: Arc::downgrade(stage),
                names,
                visible,
            });
            match intervention {
                Intervention::SourceEdit(path, bytes) => fs::write(path, bytes).unwrap(),
                Intervention::FailSecond => stage.lock().unwrap().fail_second(),
                Intervention::Abort => {
                    return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                        "publication completion aborted after real staging"
                    )));
                }
                _ => {}
            }
            Ok(())
        }
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        self.root.compute(tx).await
    }
}

#[test]
fn native_status_publication_creates_preserves_rewrites_and_shares_global_paths() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    assert!(
        workspace
            .path()
            .join("bazel-out/.slug-configurations")
            .is_dir()
    );
    assert_eq!(final_bytes(workspace.path()), [None, None]);
    let cold = PublicationProbe::new(keys[0].clone(), Intervention::None);
    let first = runtime
        .drive_command(native_request(&runtime), cold.clone())
        .unwrap();
    {
        let attempts = cold.attempts.lock().unwrap();
        assert!(
            !attempts.is_empty(),
            "real completion stages initially missing status files"
        );
        let terminal = first.accepted.terminal_for_test();
        assert!(
            validate_publication_namespace(
                &NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap(),
                cold.source_certificate(terminal).unwrap(),
            )
            .is_ok()
        );
        assert_eq!(
            cold.publications.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        for attempt in attempts.iter() {
            assert_eq!(attempt.visible, [None, None]);
            assert!(Arc::ptr_eq(
                &attempt.capture,
                &first.accepted.terminal_for_test().inputs.status.capture
            ));
            assert!(attempt.stage.upgrade().is_none());
            assert!(attempt.names.iter().all(|path| !path.exists()));
        }
    }
    let held = first.accepted.terminal_for_test().inputs.clone();
    assert!(first.accepted.terminal_for_test().pending.is_none());
    assert_eq!(
        first.accepted.terminal_for_test().outcome.unwrap().stable,
        StablePublication::Replaced
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), STABLE)).unwrap(),
        held.status.stable
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
        held.status.volatile
    );
    for name in [STABLE, VOLATILE] {
        assert_eq!(
            fs::metadata(final_path(workspace.path(), name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
    }
    no_stage_names(workspace.path());
    past_time(&final_path(workspace.path(), STABLE));
    let stable_before = fingerprint(&final_path(workspace.path(), STABLE));
    let probe = PublicationProbe::new(keys[3].clone(), Intervention::EqualVolatile);
    let equal = runtime
        .drive_command(native_request(&runtime), probe.clone())
        .unwrap();
    let current = equal.accepted.terminal_for_test();
    assert_eq!(
        current.outcome.unwrap().stable,
        StablePublication::Preserved
    );
    assert_eq!(
        fingerprint(&final_path(workspace.path(), STABLE)),
        stable_before
    );
    let volatile_after = fingerprint(&final_path(workspace.path(), VOLATILE));
    let volatile_before = probe.equal_before.lock().unwrap().clone().unwrap();
    assert_ne!(volatile_before.1, volatile_after.1);
    assert_ne!(volatile_before.4, volatile_after.4);
    assert_eq!(
        fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
        current.inputs.status.volatile
    );
    assert_ne!(
        held.template.action().context().owner(),
        current.inputs.template.action().context().owner()
    );
    assert_eq!(held.status.stable, current.inputs.status.stable);
    fs::set_permissions(
        final_path(workspace.path(), STABLE),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    fs::write(
        final_path(workspace.path(), STABLE),
        b"different old contents",
    )
    .unwrap();
    let changed = runtime
        .drive_command(
            native_request(&runtime),
            WorkspaceStatusPublicationRoot(keys[0].clone()),
        )
        .unwrap();
    assert_eq!(
        changed.accepted.terminal_for_test().outcome.unwrap().stable,
        StablePublication::Replaced
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), STABLE)).unwrap(),
        changed.accepted.terminal_for_test().inputs.status.stable
    );
    no_stage_names(workspace.path());
    revoked(&runtime);
    let expected_stable = held.status.stable.clone();
    let expected_volatile = held.status.volatile.clone();
    // NativeInputs owns immutable template/status metadata, never rendered output.
    let expected_template = held
        .template
        .source()
        .result()
        .as_ref()
        .unwrap()
        .bytes()
        .to_vec();
    let template_weak = Arc::downgrade(&held.template);
    let status_weak = Arc::downgrade(&held.status);
    let capture_weak = Arc::downgrade(&held.status.capture);
    let input_weak = Arc::downgrade(&held);
    drop(changed);
    drop(equal);
    drop(first);
    drop(cold);
    drop(probe);
    drop(keys);
    drop(runtime);
    assert_eq!(held.status.stable, expected_stable);
    assert_eq!(held.status.volatile, expected_volatile);
    assert_eq!(
        held.template.source().result().as_ref().unwrap().bytes(),
        expected_template
    );
    drop(held);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while (status_weak.upgrade().is_some()
        || capture_weak.upgrade().is_some()
        || input_weak.upgrade().is_some())
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(status_weak.upgrade().is_none());
    assert!(capture_weak.upgrade().is_none());
    assert!(input_weak.upgrade().is_none());
    assert!(template_weak.upgrade().is_none());
}

#[test]
fn native_status_publication_source_retry_drops_pending_before_any_visible_write() {
    let workspace = status_workspace();
    old_files(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let build = workspace.path().join("BUILD.bazel");
    let mut bytes = fs::read(&build).unwrap();
    bytes.extend_from_slice(b"# status publication retry\n");
    let probe = PublicationProbe::new(keys[0].clone(), Intervention::SourceEdit(build, bytes));
    let driven = runtime
        .drive_command(native_request(&runtime), probe.clone())
        .unwrap();
    let value = driven.accepted.terminal_for_test();
    let attempts = probe.attempts.lock().unwrap();
    assert!(attempts.len() > 1);
    assert_eq!(
        probe.publications.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    for attempt in attempts.iter() {
        assert_eq!(
            attempt.visible,
            [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
        );
        assert!(Arc::ptr_eq(&attempt.capture, &value.inputs.status.capture));
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|path| !path.exists()));
    }
    assert!(value.inputs.template.observations().observations().values().any(|result| matches!(result.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.ends_with(b"# status publication retry\n"))));
    assert_eq!(
        fs::read(final_path(workspace.path(), STABLE)).unwrap(),
        value.inputs.status.stable
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
        value.inputs.status.volatile
    );
    no_stage_names(workspace.path());
    revoked(&runtime);
}

fn stage_workspace() -> tempfile::TempDir {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp927/stage-fixtures");
    fs::create_dir_all(&parent).unwrap();
    let workspace = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    old_files(workspace.path());
    workspace
}

#[test]
fn status_stage_refuses_links_special_alias_unreadable_and_swapped_names() {
    for kind in 0..6 {
        let workspace = stage_workspace();
        let root = workspace.path();
        let victim = root.join("victim");
        fs::write(&victim, b"outside remains").unwrap();
        let stable = final_path(root, STABLE);
        fs::remove_file(&stable).unwrap();
        match kind {
            0 => symlink(&victim, &stable).unwrap(),
            1 => fs::create_dir(&stable).unwrap(),
            2 => nix::unistd::mkfifo(&stable, nix::sys::stat::Mode::from_bits_truncate(0o600))
                .unwrap(),
            3 => fs::hard_link(&victim, &stable).unwrap(),
            4 => {
                fs::write(&stable, OLD_STABLE).unwrap();
                fs::set_permissions(&stable, fs::Permissions::from_mode(0)).unwrap();
            }
            _ => {
                fs::rename(root.join("bazel-out"), root.join("original-bazel")).unwrap();
                symlink(root.join("original-bazel"), root.join("bazel-out")).unwrap();
            }
        }
        assert!(Staging::new(root, b"new stable", b"new volatile").is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"outside remains");
        assert_eq!(fs::read(final_path(root, VOLATILE)).unwrap(), OLD_VOLATILE);
        no_stage_names(root);
    }
    for mutation in 0..4 {
        let workspace = stage_workspace();
        let root = workspace.path();
        let mut stage = Staging::new(root, b"new stable", b"new volatile").unwrap();
        let paths = stage_paths(root, &stage);
        match mutation {
            0 => {
                fs::set_permissions(&paths[0], fs::Permissions::from_mode(0o644)).unwrap();
                fs::write(&paths[0], b"mutated stage").unwrap();
            }
            1 => {
                let saved = root.join("moved-owned-stage");
                fs::rename(&paths[0], &saved).unwrap();
                fs::write(&paths[0], b"foreign replacement").unwrap();
            }
            2 => {
                fs::rename(final_path(root, STABLE), root.join("moved-old-stable")).unwrap();
                fs::write(final_path(root, STABLE), OLD_STABLE).unwrap();
            }
            _ => fs::write(
                final_path(root, STABLE),
                b"old file changed after preparation",
            )
            .unwrap(),
        }
        assert!(stage.publish(root, b"new stable", b"new volatile").is_err());
        assert_eq!(fs::read(final_path(root, VOLATILE)).unwrap(), OLD_VOLATILE);
        if mutation < 2 {
            assert_eq!(fs::read(final_path(root, STABLE)).unwrap(), OLD_STABLE);
        }
        drop(stage);
        if mutation == 1 {
            assert_eq!(fs::read(&paths[0]).unwrap(), b"foreign replacement");
            assert!(!paths[1].exists());
            fs::remove_file(&paths[0]).unwrap();
        } else {
            assert!(paths.iter().all(|path| !path.exists()));
        }
        no_stage_names(root);
    }
    let workspace = stage_workspace();
    let root = workspace.path();
    let mut stage = Staging::new(root, b"new stable", b"new volatile").unwrap();
    let names = stage.names().map(str::to_owned);
    fs::rename(root.join("bazel-out"), root.join("old-bazel")).unwrap();
    old_files(root);
    assert!(stage.publish(root, b"new stable", b"new volatile").is_err());
    drop(stage);
    assert_eq!(
        final_bytes(root),
        [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
    );
    assert!(
        names
            .iter()
            .all(|name| !root.join("old-bazel").join(name).exists())
    );
    no_stage_names(root);
}

#[test]
fn status_stage_clone_cleanup_and_second_failure_are_attributed() {
    let workspace = stage_workspace();
    let root = workspace.path();
    let stage = Arc::new(Mutex::new(
        Staging::new(root, b"new stable", b"new volatile").unwrap(),
    ));
    let names = stage_paths(root, &stage.lock().unwrap());
    let weak = Arc::downgrade(&stage);
    let held = stage.clone();
    drop(stage);
    assert!(weak.upgrade().is_some());
    assert!(names.iter().all(|path| path.exists()));
    drop(held);
    assert!(weak.upgrade().is_none());
    assert!(names.iter().all(|path| !path.exists()));
    assert_eq!(
        final_bytes(root),
        [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
    );
    let mut stage = Staging::new(root, b"new stable", b"new volatile").unwrap();
    let names = stage_paths(root, &stage);
    stage.fail_second();
    assert!(stage.publish(root, b"new stable", b"new volatile").is_err());
    assert_eq!(fs::read(final_path(root, STABLE)).unwrap(), b"new stable");
    assert_eq!(fs::read(final_path(root, VOLATILE)).unwrap(), OLD_VOLATILE);
    drop(stage);
    assert!(names.iter().all(|path| !path.exists()));
    no_stage_names(root);
    let stage = Arc::new(Mutex::new(
        Staging::new(root, b"stable B", b"volatile B").unwrap(),
    ));
    let names = stage_paths(root, &stage.lock().unwrap());
    let held = stage.clone();
    assert_eq!(
        stage
            .lock()
            .unwrap()
            .publish(root, b"stable B", b"volatile B")
            .unwrap()
            .stable,
        StablePublication::Replaced
    );
    assert!(
        names.iter().all(|path| !path.exists()),
        "successful publication retires stage with scratch clone still held"
    );
    assert!(
        held.lock()
            .unwrap()
            .publish(root, b"stable B", b"volatile B")
            .is_err()
    );
    drop(stage);
    drop(held);
    no_stage_names(root);
    assert_eq!(
        final_bytes(root),
        [Some(b"stable B".to_vec()), Some(b"volatile B".to_vec())]
    );
}

#[test]
fn native_status_publication_abort_and_failed_publish_preserve_acceptance() {
    let workspace = status_workspace();
    old_files(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let abort = PublicationProbe::new(keys[0].clone(), Intervention::Abort);
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), abort.clone()),
        Err(NativeDemandSessionError::Computation(_))
    ));
    assert_eq!(
        abort.publications.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_snapshot_same(&runtime, &before);
    revoked(&runtime);
    assert_eq!(
        final_bytes(workspace.path()),
        [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
    );
    for attempt in abort.attempts.lock().unwrap().iter() {
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|path| !path.exists()));
    }
    let fail = PublicationProbe::new(keys[0].clone(), Intervention::FailSecond);
    let error = runtime
        .drive_command(native_request(&runtime), fail.clone())
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            NativeDemandSessionError::Revision(RequestRevisionError::OutputPublication(ref detail))
                if detail.contains("injected second status rename failure")
        ),
        "{error}"
    );
    assert_eq!(
        fail.publications.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_snapshot_same(&runtime, &before);
    revoked(&runtime);
    let capture = fail
        .attempts
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .capture
        .clone();
    let millis = capture.raw.millis.unwrap();
    let expected = stable_bytes(
        capture.raw.user.identity().unwrap(),
        capture
            .raw
            .hostname
            .identity()
            .unwrap()
            .split('.')
            .next()
            .unwrap(),
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), STABLE)).unwrap(),
        expected
    );
    assert_eq!(
        fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
        OLD_VOLATILE
    );
    assert!(millis <= MAX_MILLIS);
    for attempt in fail.attempts.lock().unwrap().iter() {
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|path| !path.exists()));
    }
    no_stage_names(workspace.path());
    let recovered = runtime
        .drive_command(
            native_request(&runtime),
            WorkspaceStatusPublicationRoot(keys[0].clone()),
        )
        .unwrap();
    assert!(recovered.accepted.terminal_for_test().outcome.is_some());
    revoked(&runtime);
}

#[test]
fn native_status_publication_refuses_foreign_capture_wrong_descriptor_and_observed_output_namespace()
 {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let first = runtime
        .drive_command(
            native_request(&runtime),
            WorkspaceStatusPublicationRoot(keys[0].clone()),
        )
        .unwrap();
    let old_capture = first
        .accepted
        .terminal_for_test()
        .inputs
        .status
        .capture
        .clone();
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let bytes = final_bytes(workspace.path());
    for intervention in [Intervention::Stale(old_capture), Intervention::Copied] {
        let rejected = PublicationProbe::new(keys[0].clone(), intervention);
        let error = runtime
            .drive_command(native_request(&runtime), rejected.clone())
            .err()
            .unwrap();
        assert!(error.to_string().contains("ForeignCapture"), "{error}");
        assert_eq!(
            rejected
                .publications
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert!(rejected.attempts.lock().unwrap().is_empty());
        assert_snapshot_same(&runtime, &before);
        assert_eq!(final_bytes(workspace.path()), bytes);
        no_stage_names(workspace.path());
        revoked(&runtime);
    }
    let normalized = NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap();
    let status = &first.accepted.terminal_for_test().inputs.status;
    assert!(validate_descriptors(&normalized, status).is_ok());
    let foreign = NormalizedAbsolutePath::new("/foreign").unwrap();
    assert!(validate_descriptors(&foreign, status).is_err());
    let wrong = WorkspaceStatusContent {
        artifacts: Arc::new(WorkspaceStatusArtifacts::new(foreign)),
        ..status.as_ref().clone()
    };
    assert!(validate_descriptors(&normalized, &wrong).is_err());
    for (suffix, admitted) in [
        ("", true),
        ("bazel-outside", true),
        ("bazel-out", false),
        ("bazel-out/stable-status.txt", false),
        ("bazel-out/private/stage", false),
    ] {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(workspace.path().join(suffix)).unwrap(),
            PathObservationOperation::Lstat,
        );
        let certificate = SourceCertificate::new(
            demand,
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        );
        assert_eq!(
            validate_publication_namespace(&normalized, &certificate).is_ok(),
            admitted,
            "{suffix}"
        );
    }
    assert_eq!(final_bytes(workspace.path()), bytes);
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod file_inputs {
    use super::*;
    include!("workspace_status_file_inputs_tests.rs");
}
