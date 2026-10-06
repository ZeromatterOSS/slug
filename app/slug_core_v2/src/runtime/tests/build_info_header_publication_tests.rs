use crate::runtime::action_output_staging::ActionOutputStaging;
use crate::runtime::dice::build_info_header_publication::MAX_HEADER_BYTES;
use crate::runtime::dice::build_info_header_publication::NativeBuildInfoHeaderPublicationRoot;
use crate::runtime::dice::build_info_header_publication::NativeBuildInfoHeaderPublicationTerminal;
use crate::runtime::dice::build_info_header_publication::PublishedBuildInfoHeader;
use crate::runtime::dice::build_info_header_publication::check_header_size;

const OLD_HEADER: &[u8] = b"previous declared header\n";
#[derive(Clone)]
enum HeaderMode {
    Normal,
    CloneOnce,
    BeforeCopy,
    BeforePhysical,
    BeforeOrigin,
    OldResult(Arc<PublishedBuildInfoHeader>),
    AfterCopy,
    AfterPhysical,
    AfterOrigin,
    ConsumedCopy,
    PendingSwap,
    WrongPhase,
    Abort,
    SourceEdit(PathBuf, Vec<u8>),
    Mutate(u8),
}
#[derive(Clone)]
struct HeaderTerminal {
    inner: NativeBuildInfoHeaderPublicationTerminal,
    other: NativeBuildInfoHeaderPublicationTerminal,
    original: Arc<BuildInfoNativeInputs>,
}
struct HeaderAttempt {
    capture: Arc<WorkspaceStatusCapture>,
    stage: std::sync::Weak<Mutex<Staging>>,
    names: [PathBuf; 2],
    status_before: [Option<Vec<u8>>; 2],
    header_before: Option<Vec<u8>>,
}
struct HeldScalar {
    cell: Arc<Mutex<Option<ActionOutputStaging>>>,
    fds: [i32; 6],
    name: PathBuf,
}
#[derive(Clone)]
struct HeaderProbe {
    root: NativeBuildInfoHeaderPublicationRoot,
    other: NativeBuildInfoHeaderPublicationRoot,
    mode: Arc<Mutex<Option<HeaderMode>>>,
    publish_mode: Arc<Mutex<Option<HeaderMode>>>,
    attempts: Arc<Mutex<Vec<HeaderAttempt>>>,
    scalars: Arc<Mutex<Vec<HeldScalar>>>,
    publications: Arc<std::sync::atomic::AtomicUsize>,
    seed: bool,
}
impl HeaderProbe {
    fn new(key: BuildInfoNativeInputsKey, mode: HeaderMode, seed: bool) -> Self {
        Self {
            root: NativeBuildInfoHeaderPublicationRoot::new(key.clone()),
            other: NativeBuildInfoHeaderPublicationRoot::new(key),
            mode: Arc::new(Mutex::new(Some(mode))),
            publish_mode: Arc::new(Mutex::new(None)),
            attempts: Arc::new(Mutex::new(Vec::new())),
            scalars: Arc::new(Mutex::new(Vec::new())),
            publications: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            seed,
        }
    }
}
fn header_path(input: &BuildInfoNativeInputs) -> PathBuf {
    let [output] = input.template.action().outputs() else {
        panic!("genuine retained action is scalar");
    };
    crate::runtime::configured_output::configured_output_root(
        input.status.capture.workspace.as_path(),
        output.root(),
    )
    .join(output.root_relative_path())
}
fn optional_header(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("test header read: {error}"),
    }
}
fn fd_retired(cell: &Arc<Mutex<Option<ActionOutputStaging>>>, fds: [i32; 6]) {
    assert!(
        cell.lock().unwrap().is_none(),
        "grant retirement empties cell despite retained scratch clone"
    );
    for fd in fds {
        assert!(
            !open_fd(fd),
            "scalar descriptor {fd} retained after phase exit"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(nix::libc::EBADF)
        );
    }
}
fn mutate_scalar(input: &BuildInfoNativeInputs, stage_path: &Path, mode: u8) {
    let destination = header_path(input);
    match mode {
        0 => {
            fs::set_permissions(stage_path, fs::Permissions::from_mode(0o644)).unwrap();
            fs::write(stage_path, b"bad staged header").unwrap();
            fs::set_permissions(stage_path, fs::Permissions::from_mode(0o444)).unwrap();
        }
        1 => {
            fs::rename(
                stage_path,
                input
                    .status
                    .capture
                    .workspace
                    .as_path()
                    .join("saved-header-stage"),
            )
            .unwrap();
            fs::write(stage_path, b"foreign hidden header").unwrap();
        }
        2 => fs::set_permissions(stage_path, fs::Permissions::from_mode(0o555)).unwrap(),
        3 => fs::hard_link(
            stage_path,
            input
                .status
                .capture
                .workspace
                .as_path()
                .join("header-stage-alias"),
        )
        .unwrap(),
        4 => mutate_file(input.status.capture.workspace.as_path(), input, 1),
        5 => {
            fs::rename(&destination, destination.with_extension("saved")).unwrap();
            fs::write(&destination, OLD_HEADER).unwrap();
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o555)).unwrap();
        }
        6 | 7 => {
            fs::rename(&destination, destination.with_extension("saved")).unwrap();
            if mode == 6 {
                symlink(destination.with_extension("saved"), &destination).unwrap();
            } else {
                fs::create_dir(&destination).unwrap();
            }
        }
        8 => fs::set_permissions(&destination, fs::Permissions::from_mode(0)).unwrap(),
        9 => fs::hard_link(&destination, destination.with_extension("alias")).unwrap(),
        10 => {
            let parent = destination.parent().unwrap();
            fs::rename(parent, parent.with_extension("saved-root")).unwrap();
            fs::create_dir(parent).unwrap();
            fs::write(&destination, OLD_HEADER).unwrap();
        }
        11 => {
            fs::rename(&destination, destination.with_extension("saved")).unwrap();
            nix::unistd::mkfifo(
                &destination,
                nix::sys::stat::Mode::from_bits_truncate(0o600),
            )
            .unwrap();
        }
        12 => fs::set_permissions(
            destination.parent().unwrap(),
            fs::Permissions::from_mode(0o555),
        )
        .unwrap(),
        _ => unreachable!(),
    }
}
#[async_trait]
impl NativeCommandRoot for HeaderProbe {
    type Terminal = HeaderTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(value.original.template.observations())
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        Some(value.original.template.certificate())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    fn publication_completed(&self, value: &Self::Terminal) -> bool {
        self.root.publication_completed(&value.inner)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let inner = match self.root.compute(tx).await? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(value) => value,
        };
        let other = match self.other.compute(tx).await? {
            PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
            PreparationOutcome::Complete(value) => value,
        };
        let original = inner.physical.publication.inputs.clone();
        assert!(
            Arc::ptr_eq(&original, &other.physical.publication.inputs),
            "foreign root terminal deliberately shares SAME actual DICE metadata"
        );
        assert_eq!(
            original.template.observations(),
            other.physical.publication.inputs.template.observations()
        );
        assert_eq!(
            original.template.certificate(),
            other.physical.publication.inputs.template.certificate()
        );
        Ok(PreparationOutcome::Complete(HeaderTerminal {
            inner,
            other,
            original,
        }))
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
            let mode = self
                .mode
                .lock()
                .unwrap()
                .take()
                .unwrap_or(HeaderMode::Normal);
            let root = context.command.runtime.workspace.clone();
            let path = header_path(&value.original);
            if self.seed && !path.exists() {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, OLD_HEADER).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            }
            match &mode {
                HeaderMode::BeforeCopy => {
                    value.inner.physical.publication.inputs = copied_metadata(&value.original)
                }
                HeaderMode::BeforePhysical => value.inner.physical = value.other.physical.clone(),
                HeaderMode::BeforeOrigin => {
                    return self.other.complete(&mut value.inner, context).await;
                }
                HeaderMode::OldResult(old) => value.inner.published = Some(old.clone()),
                _ => {}
            }
            let status_before = final_bytes(&root);
            let header_before = optional_header(&path);
            self.root.complete(&mut value.inner, context).await?;
            assert!(value.inner.pending.is_none());
            assert!(value.inner.published.is_none());
            assert_eq!(status_before, final_bytes(&root));
            assert_eq!(header_before, optional_header(&path));
            let status_stage = value.inner.physical.publication.pending.as_ref().unwrap();
            let names = stage_paths(&root, &status_stage.lock().unwrap());
            for attempt in self.attempts.lock().unwrap().iter() {
                assert!(attempt.stage.upgrade().is_none());
                assert!(attempt.names.iter().all(|p| !p.exists()));
                assert!(Arc::ptr_eq(
                    &attempt.capture,
                    &value.original.status.capture
                ));
            }
            self.attempts.lock().unwrap().push(HeaderAttempt {
                capture: value.original.status.capture.clone(),
                stage: Arc::downgrade(status_stage),
                names,
                status_before,
                header_before,
            });
            match &mode {
                HeaderMode::SourceEdit(path, bytes) => fs::write(path, bytes).unwrap(),
                HeaderMode::Abort => {
                    return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                        "abort after status completion before header effects"
                    )));
                }
                _ => {}
            }
            *self.publish_mode.lock().unwrap() = Some(mode);
            Ok(())
        }
    }
    fn publish(&self, value: &mut Self::Terminal) -> std::io::Result<()> {
        self.publications
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mode = self.publish_mode.lock().unwrap().take().unwrap();
        let mut clone = value.inner.clone();
        if matches!(mode, HeaderMode::WrongPhase) {
            let error = self.root.publish_header(&mut value.inner).err().unwrap();
            let spent = self.root.stage_header(&mut clone).err().unwrap();
            assert!(spent.to_string().contains("header permit spent"), "{spent}");
            return Err(error);
        }
        self.root.stage_header(&mut value.inner)?;
        clone = value.inner.clone();
        let cell = value.inner.pending.as_ref().unwrap().clone();
        let (fds, name) = {
            let guard = cell.lock().unwrap();
            let stage = guard.as_ref().unwrap();
            (
                stage.scalar_raw_fds(),
                header_path(&value.original)
                    .parent()
                    .unwrap()
                    .join(stage.scalar_stage_name()),
            )
        };
        // No stage guard/FD writer is kept across any phase effect.
        self.scalars.lock().unwrap().push(HeldScalar {
            cell: cell.clone(),
            fds,
            name: name.clone(),
        });
        let parent_permissions = fs::metadata(name.parent().unwrap()).unwrap().permissions();
        match &mode {
            HeaderMode::AfterCopy => {
                value.inner.physical.publication.inputs = copied_metadata(&value.original)
            }
            HeaderMode::AfterPhysical => value.inner.physical = value.other.physical.clone(),
            HeaderMode::ConsumedCopy => {
                let actual = value.inner.physical.consumed.as_ref().unwrap();
                value.inner.physical.consumed = Some(Arc::new(ConsumedBuildInfo {
                    inputs: actual.inputs.clone(),
                    files: actual.files.clone(),
                    header: actual.header.clone(),
                }));
            }
            HeaderMode::PendingSwap => value.inner.pending = Some(Arc::new(Mutex::new(None))),
            HeaderMode::Mutate(kind) => mutate_scalar(&value.original, &name, *kind),
            _ => {}
        }
        let result = if matches!(mode, HeaderMode::AfterOrigin) {
            self.other.publish_header(&mut value.inner)
        } else {
            self.root.publish_header(&mut value.inner)
        };
        fd_retired(&cell, fds);
        if matches!(mode, HeaderMode::Mutate(12)) {
            let error = result.as_ref().err().unwrap();
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            assert!(
                error.to_string().contains("publishing action outputs"),
                "{error}"
            );
            assert!(error.to_string().contains("os error 13"), "{error}");
            // Readable parent validates; rename and Drop unlink require write permission.
            // After immediate FD checks restore only the controlled fixture for cleanup.
            fs::set_permissions(name.parent().unwrap(), parent_permissions).unwrap();
            assert_eq!(fs::read(header_path(&value.original)).unwrap(), OLD_HEADER);
            assert_eq!(
                fs::read(&name).unwrap(),
                value.inner.physical.consumed.as_ref().unwrap().header
            );
            fs::remove_file(&name).unwrap();
        }
        if matches!(
            mode,
            HeaderMode::AfterCopy
                | HeaderMode::AfterPhysical
                | HeaderMode::AfterOrigin
                | HeaderMode::ConsumedCopy
                | HeaderMode::PendingSwap
        ) {
            assert!(result.is_err());
            clone.physical.publication.inputs = value.original.clone();
            let spent = self.root.publish_header(&mut clone).err().unwrap();
            assert!(spent.to_string().contains("header permit spent"), "{spent}");
            assert!(self.root.stage_header(&mut clone).is_err());
        }
        if matches!(mode, HeaderMode::CloneOnce) {
            assert!(result.is_ok());
            assert!(self.root.stage_header(&mut clone).is_err());
            assert!(self.root.publish_header(&mut clone).is_err());
        }
        result
    }
}

fn header_result(value: &HeaderTerminal) -> &Arc<PublishedBuildInfoHeader> {
    value
        .inner
        .published
        .as_ref()
        .expect("accepted header requires real publication")
}
fn header_failed(
    runtime: &WorkspaceRuntime,
    root: &HeaderProbe,
    before: &AcceptedNativeDemandSnapshot,
) {
    assert_snapshot_same(runtime, before);
    revoked(runtime);
    for attempt in root.attempts.lock().unwrap().iter() {
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|p| !p.exists()));
    }
    for scalar in root.scalars.lock().unwrap().iter() {
        assert!(scalar.cell.lock().unwrap().is_none());
        if scalar.name.exists() {
            assert_eq!(fs::read(&scalar.name).unwrap(), b"foreign hidden header");
        }
    }
}

#[test]
fn native_headers_publish_actual_info_version_paths_modes_metadata_and_configuration_source_aba() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let build = workspace.path().join("BUILD.bazel");
    let source_a = fs::read(&build).unwrap();
    let mut source_b = source_a.clone();
    source_b.extend_from_slice(b"# checked header source B\n");
    let mut held = Vec::new();
    let mut paths = Vec::new();
    for (index, source) in [
        (1, &source_a),
        (0, &source_a),
        (3, &source_b),
        (1, &source_a),
    ] {
        fs::write(&build, source).unwrap();
        let root = HeaderProbe::new(keys[index].clone(), HeaderMode::CloneOnce, false);
        let driven = runtime
            .drive_command(native_request(&runtime), root.clone())
            .unwrap();
        let terminal = driven.accepted.terminal_for_test();
        let result = header_result(terminal).clone();
        assert!(
            Arc::ptr_eq(
                &result.consumed,
                terminal.inner.physical.consumed.as_ref().unwrap()
            ),
            "published result must retain SAME physically rendered header Vec owner"
        );
        assert!(Arc::ptr_eq(&result.consumed.inputs, &terminal.original));
        let input = &result.consumed.inputs;
        let [output] = input.template.action().outputs() else {
            unreachable!()
        };
        let path = header_path(input);
        assert_eq!(output.kind(), ActionOutputKind::File);
        assert_eq!(
            output.metadata(),
            if index % 2 == 0 {
                ActionOutputMetadata::Constant
            } else {
                ActionOutputMetadata::Ordinary
            }
        );
        assert_eq!(result.outputs.outputs(), input.template.action().outputs());
        assert_eq!(
            result.outputs.root(),
            crate::runtime::configured_output::configured_output_root(
                workspace.path(),
                output.root()
            )
        );
        assert_eq!(result.consumed.header, expected_header(input));
        assert_eq!(fs::read(&path).unwrap(), expected_header(input));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o444
        );
        assert_ne!(result.consumed.header.as_ptr(), input.header.as_ptr());
        let other = path.parent().unwrap().join("unrelated-preserved");
        if !other.exists() {
            fs::write(&other, b"not a declared output").unwrap();
        }
        assert_eq!(fs::read(&other).unwrap(), b"not a declared output");
        assert!(input.template.observations().observations().values().any(|r|matches!(r.as_ref(),PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.as_ref()==source.as_slice())));
        for scalar in root.scalars.lock().unwrap().iter() {
            assert!(scalar.cell.lock().unwrap().is_none());
            assert!(!scalar.name.exists());
        }
        for earlier in &held {
            let earlier: &Arc<PublishedBuildInfoHeader> = earlier;
            assert_eq!(
                earlier.consumed.header,
                expected_header(&earlier.consumed.inputs)
            );
        }
        held.push(result);
        paths.push(path);
        revoked(&runtime);
    }
    assert_ne!(paths[0], paths[2]);
    assert_eq!(paths[0], paths[3]);
    assert_ne!(paths[0], paths[1]);
    // Readable old 0555 is admitted; replacement remains exactly nonexecutable0444.
    fs::set_permissions(&paths[0], fs::Permissions::from_mode(0o555)).unwrap();
    let replaced = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(keys[1].clone()),
        )
        .unwrap();
    assert_eq!(
        fs::metadata(&paths[0]).unwrap().permissions().mode() & 0o777,
        0o444
    );
    drop(replaced);
    let last = held.pop().unwrap();
    let expected = last.consumed.header.clone();
    let header_weak = Arc::downgrade(&last);
    let consumed_weak = Arc::downgrade(&last.consumed);
    let inputs_weak = Arc::downgrade(&last.consumed.inputs);
    drop(held);
    drop(keys);
    drop(runtime);
    assert_eq!(last.consumed.header, expected);
    drop(last);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while (header_weak.upgrade().is_some()
        || consumed_weak.upgrade().is_some()
        || inputs_weak.upgrade().is_some())
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(header_weak.upgrade().is_none());
    assert!(consumed_weak.upgrade().is_none());
    assert!(inputs_weak.upgrade().is_none());
}

#[test]
fn native_headers_copied_physical_consumed_pending_and_old_result_cannot_restore_grants() {
    let workspace = status_workspace();
    old_files(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let setup = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(keys[1].clone()),
        )
        .unwrap();
    let old = setup
        .accepted
        .terminal_for_test()
        .published
        .as_ref()
        .unwrap()
        .clone();
    let path = header_path(&old.consumed.inputs);
    let expected = fs::read(&path).unwrap();
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for mode in [
        HeaderMode::BeforeCopy,
        HeaderMode::BeforePhysical,
        HeaderMode::BeforeOrigin,
        HeaderMode::OldResult(old),
        HeaderMode::WrongPhase,
        HeaderMode::AfterCopy,
        HeaderMode::AfterPhysical,
        HeaderMode::AfterOrigin,
        HeaderMode::ConsumedCopy,
        HeaderMode::PendingSwap,
    ] {
        let root = HeaderProbe::new(keys[1].clone(), mode, true);
        assert!(
            runtime
                .drive_command(native_request(&runtime), root.clone())
                .is_err()
        );
        header_failed(&runtime, &root, &before);
        assert_eq!(fs::read(&path).unwrap(), expected);
    }
    let recovered = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(keys[1].clone()),
        )
        .unwrap();
    assert_eq!(
        recovered
            .accepted
            .terminal_for_test()
            .published
            .as_ref()
            .unwrap()
            .consumed
            .header,
        expected
    );
}

#[test]
fn native_headers_real_scalar_and_status_mutations_refuse_after_publication_and_retire_fds() {
    assert_ne!(
        nix::unistd::geteuid().as_raw(),
        0,
        "permission-denied rename discriminator requires non-root owner"
    );
    for mutation in 0..13 {
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
        let root = HeaderProbe::new(keys[1].clone(), HeaderMode::Mutate(mutation), true);
        let error = runtime
            .drive_command(native_request(&runtime), root.clone())
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                NativeDemandSessionError::Revision(RequestRevisionError::OutputPublication(_))
            ),
            "{error}"
        );
        header_failed(&runtime, &root, &before);
        assert_eq!(
            root.scalars.lock().unwrap().len(),
            1,
            "mutation follows real physical consume/write/nonexec seal"
        );
        let attempt = root.attempts.lock().unwrap();
        let raw = &attempt.last().unwrap().capture.raw;
        if mutation != 4 {
            assert_eq!(
                fs::read(final_path(workspace.path(), STABLE)).unwrap(),
                stable_bytes(
                    raw.user.identity().unwrap(),
                    raw.hostname.identity().unwrap().split('.').next().unwrap()
                )
            );
        }
        assert_ne!(
            fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
            OLD_VOLATILE
        );
    }
}

#[test]
fn native_headers_source_retry_and_abort_have_no_early_header_effects_and_recover() {
    let workspace = status_workspace();
    old_files(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let build = workspace.path().join("BUILD.bazel");
    let mut changed = fs::read(&build).unwrap();
    changed.extend_from_slice(b"# native header retry\n");
    let retry = HeaderProbe::new(
        keys[1].clone(),
        HeaderMode::SourceEdit(build, changed),
        true,
    );
    let driven = runtime
        .drive_command(native_request(&runtime), retry.clone())
        .unwrap();
    let value = driven.accepted.terminal_for_test();
    let published = header_result(value);
    assert_eq!(
        fs::read(header_path(&published.consumed.inputs)).unwrap(),
        expected_header(&published.consumed.inputs)
    );
    let attempts = retry.attempts.lock().unwrap();
    assert!(attempts.len() > 1);
    for attempt in attempts.iter() {
        assert_eq!(
            attempt.status_before,
            [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
        );
        assert_eq!(attempt.header_before, Some(OLD_HEADER.to_vec()));
        assert!(Arc::ptr_eq(
            &attempt.capture,
            &published.consumed.inputs.status.capture
        ));
        assert!(attempt.stage.upgrade().is_none());
    }
    assert_eq!(
        retry.publications.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(retry.scalars.lock().unwrap().len(), 1);
    drop(attempts);
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let files = final_bytes(workspace.path());
    let header = fs::read(header_path(&published.consumed.inputs)).unwrap();
    let abort = HeaderProbe::new(keys[1].clone(), HeaderMode::Abort, true);
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), abort.clone()),
        Err(NativeDemandSessionError::Computation(_))
    ));
    header_failed(&runtime, &abort, &before);
    assert_eq!(
        abort.publications.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(abort.scalars.lock().unwrap().is_empty());
    assert_eq!(final_bytes(workspace.path()), files);
    assert_eq!(
        fs::read(header_path(&published.consumed.inputs)).unwrap(),
        header
    );
    let recovered = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(keys[1].clone()),
        )
        .unwrap();
    assert_eq!(
        recovered
            .accepted
            .terminal_for_test()
            .published
            .as_ref()
            .unwrap()
            .consumed
            .header,
        header
    );
}

#[test]
fn native_header_bound_uses_real_catalog_ordered_markers_and_scalar_shape_guard() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let driven = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(keys[1].clone()),
        )
        .unwrap();
    let result = driven
        .accepted
        .terminal_for_test()
        .published
        .as_ref()
        .unwrap();
    let input = &result.consumed.inputs;
    let mut literal = raw();
    let user = "u".repeat(4096);
    let host = format!("{}xxxx", "{BUILD_USER}".repeat(341));
    assert_eq!(host.len(), 4096);
    literal.user = ObservedText::Text(user.clone().into());
    literal.hostname = ObservedText::Text(host.into());
    let pure = supplied(literal).unwrap();
    let view = ConfiguredBuildInfoActionView::from_configured_action(
        input.template.action(),
        &input.status.capture.workspace,
    )
    .unwrap();
    let source = input
        .template
        .source()
        .result()
        .as_ref()
        .map_err(|e| e.to_string())
        .unwrap()
        .as_ref()
        .as_ref()
        .map_err(|e| e.to_string())
        .unwrap();
    assert_eq!(source.bytes(), TEMPLATE_CASES[1].2);
    let actual = view
        .callback()
        .render_supplied_bytes(&pure.stable, source.bytes())
        .unwrap();
    let expected=format!("#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"{}xxxx\"\n#define BUILD_USER \"{user}\"\n",user.repeat(341)).into_bytes();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 1_400_909);
    use sha2::Digest as _;
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&actual)),
        "2ed28fa8eff386e888aa14627017413d9e8a83aebc4c6e755f23bea331dcc259"
    );
    assert!(actual.len() <= 1_405_044);
    assert!(check_header_size(actual.len()).is_ok());
    assert_eq!(MAX_HEADER_BYTES, 2 * 1024 * 1024);
    assert!(check_header_size(MAX_HEADER_BYTES).is_ok());
    assert!(check_header_size(MAX_HEADER_BYTES + 1).is_err());
    assert!(check_header_size(usize::MAX).is_err());
    let [output] = input.template.action().outputs() else {
        unreachable!()
    };
    let multi = [
        output.clone(),
        ActionOutput::new(
            output.root().clone(),
            "invalid-second-scalar",
            ActionOutputKind::File,
        ),
    ];
    let mut stage = ActionOutputStaging::new(workspace.path(), output.root(), &multi).unwrap();
    assert!(
        stage
            .seal_nonexecutable(&input.status.capture.workspace, b"leaf")
            .is_err()
    );
    drop(stage);
    let directory = [ActionOutput::new(
        output.root().clone(),
        "invalid-scalar-directory",
        ActionOutputKind::Directory,
    )];
    let mut stage = ActionOutputStaging::new(workspace.path(), output.root(), &directory).unwrap();
    assert!(
        stage
            .seal_nonexecutable(&input.status.capture.workspace, b"leaf")
            .is_err()
    );
    drop(stage);
    assert_eq!(
        fs::read(header_path(input)).unwrap(),
        expected_header(input)
    );
    revoked(&runtime);
}
