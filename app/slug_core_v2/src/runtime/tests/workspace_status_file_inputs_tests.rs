use slug_build_api_v2::WorkspaceStatusArtifact;
use slug_build_api_v2::WorkspaceStatusArtifactKind;

use crate::runtime::dice::build_info_file_inputs::ConsumedBuildInfo;
use crate::runtime::dice::build_info_file_inputs::NativeBuildInfoFileInputsRoot;
use crate::runtime::dice::build_info_file_inputs::NativeBuildInfoFileInputsTerminal;
use crate::runtime::dice::workspace_status_file_inputs::WorkspaceStatusFileInputs;

#[derive(Clone)]
enum FileMode {
    Normal,
    BeforeCopy,
    AfterCopy,
    BeforeOther,
    AfterOther,
    BeforeOrigin,
    AfterOrigin,
    CloneOnce,
    SourceEdit(PathBuf, Vec<u8>),
    Abort,
    FailSecond,
    SmallScratch,
    Mutate(u8),
    OldReceipt(Arc<WorkspaceStatusFileInputs>),
    ReadBeforePublish(Arc<WorkspaceStatusFileInputs>),
}
#[derive(Clone)]
struct FileTerminal {
    inner: NativeBuildInfoFileInputsTerminal,
    metadata: Vec<Arc<BuildInfoNativeInputs>>,
    observations: PathObservationEpoch,
    certificate: SourceCertificate,
    rendered: Vec<Vec<u8>>,
}
struct FileAttempt {
    capture: Arc<WorkspaceStatusCapture>,
    stage: std::sync::Weak<Mutex<Staging>>,
    names: [PathBuf; 2],
    visible: [Option<Vec<u8>>; 2],
}
#[derive(Clone)]
struct FileProbe {
    root: NativeBuildInfoFileInputsRoot,
    other: NativeBuildInfoFileInputsRoot,
    keys: Arc<[BuildInfoNativeInputsKey]>,
    batch: bool,
    mode: Arc<Mutex<Option<FileMode>>>,
    publish_mode: Arc<Mutex<Option<FileMode>>>,
    attempts: Arc<Mutex<Vec<FileAttempt>>>,
    issued: Arc<Mutex<Vec<std::sync::Weak<WorkspaceStatusFileInputs>>>>,
    publications: Arc<std::sync::atomic::AtomicUsize>,
}
impl FileProbe {
    fn new(
        keys: Arc<[BuildInfoNativeInputsKey]>,
        index: usize,
        mode: FileMode,
        batch: bool,
    ) -> Self {
        Self {
            root: NativeBuildInfoFileInputsRoot::new(keys[index].clone()),
            other: NativeBuildInfoFileInputsRoot::new(keys[index].clone()),
            keys,
            batch,
            mode: Arc::new(Mutex::new(Some(mode))),
            publish_mode: Arc::new(Mutex::new(None)),
            attempts: Arc::new(Mutex::new(Vec::new())),
            issued: Arc::new(Mutex::new(Vec::new())),
            publications: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}
fn copied_metadata(input: &Arc<BuildInfoNativeInputs>) -> Arc<BuildInfoNativeInputs> {
    let copied = Arc::new(input.as_ref().clone());
    assert_eq!(copied, *input);
    assert!(Arc::ptr_eq(&copied.status, &input.status));
    assert!(!Arc::ptr_eq(&copied, input));
    copied
}
fn selected_descriptor(input: &BuildInfoNativeInputs) -> &WorkspaceStatusArtifact {
    match ConfiguredBuildInfoActionView::from_configured_action(
        input.template.action(),
        &input.status.capture.workspace,
    )
    .unwrap()
    .status()
    .kind()
    {
        WorkspaceStatusArtifactKind::Stable => input.status.artifacts.stable(),
        WorkspaceStatusArtifactKind::Volatile => input.status.artifacts.volatile(),
    }
}
fn read_checked<'a>(
    files: &WorkspaceStatusFileInputs,
    input: &BuildInfoNativeInputs,
    scratch: &'a mut [u8],
) -> std::io::Result<&'a [u8]> {
    let pointer = scratch.as_ptr();
    let actual = files.read(&input.status, selected_descriptor(input), scratch)?;
    assert_eq!(
        actual.as_ptr(),
        pointer,
        "read must return actual scratch, not expected producer buffer"
    );
    Ok(actual)
}
fn mutate_file(root: &Path, input: &BuildInfoNativeInputs, kind: u8) {
    let selected = root.join(selected_descriptor(input).path());
    let bytes = input.status.bytes(selected_descriptor(input).kind());
    match kind {
        0 => {
            fs::rename(&selected, root.join("saved-status")).unwrap();
            fs::write(&selected, bytes).unwrap();
            fs::set_permissions(&selected, fs::Permissions::from_mode(0o444)).unwrap();
        }
        1 | 2 => {
            fs::set_permissions(&selected, fs::Permissions::from_mode(0o644)).unwrap();
            fs::write(
                &selected,
                if kind == 1 {
                    b"different status bytes".as_slice()
                } else {
                    &bytes[..bytes.len() - 1]
                },
            )
            .unwrap();
            fs::set_permissions(&selected, fs::Permissions::from_mode(0o444)).unwrap();
        }
        3 => fs::set_permissions(&selected, fs::Permissions::from_mode(0o644)).unwrap(),
        4 => fs::hard_link(&selected, root.join("status-alias")).unwrap(),
        5 | 6 => {
            fs::rename(&selected, root.join("saved-status")).unwrap();
            if kind == 5 {
                symlink(root.join("saved-status"), &selected).unwrap();
            } else {
                nix::unistd::mkfifo(&selected, nix::sys::stat::Mode::from_bits_truncate(0o600))
                    .unwrap();
            }
        }
        7 => fs::set_permissions(&selected, fs::Permissions::from_mode(0)).unwrap(),
        8 => {
            fs::rename(root.join("bazel-out"), root.join("saved-bazel-out")).unwrap();
            fs::create_dir(root.join("bazel-out")).unwrap();
            fs::write(final_path(root, STABLE), &input.status.stable).unwrap();
            fs::write(final_path(root, VOLATILE), &input.status.volatile).unwrap();
        }
        _ => unreachable!(),
    }
}
#[async_trait]
impl NativeCommandRoot for FileProbe {
    type Terminal = FileTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(&value.observations)
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        Some(&value.certificate)
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
        let mut metadata = vec![inner.publication.inputs.clone()];
        if self.batch {
            for key in self.keys.iter() {
                match NativeCommandRoot::compute(key, tx).await? {
                    PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                    PreparationOutcome::Complete(value) => metadata.push(value),
                }
            }
        }
        let observations = PathObservationEpoch::from_shared(metadata.iter().flat_map(|input| {
            input
                .template
                .observations()
                .observations()
                .iter()
                .map(|(demand, value)| (demand.dupe(), value.clone()))
        }))
        .unwrap();
        let certificate = SourceCertificate::from_epoch(
            PathObservationEpoch::from_shared(metadata.iter().flat_map(|input| {
                input
                    .template
                    .certificate()
                    .observations()
                    .observations()
                    .iter()
                    .map(|(demand, value)| (demand.dupe(), value.clone()))
            }))
            .unwrap(),
        )
        .unwrap();
        for input in &metadata {
            assert!(Arc::ptr_eq(&input.status, &metadata[0].status));
            for (demand, value) in input.template.observations().observations().iter() {
                assert_eq!(observations.get(demand).unwrap(), value);
            }
            for (demand, value) in input
                .template
                .certificate()
                .observations()
                .observations()
                .iter()
            {
                assert_eq!(certificate.observations().get(demand).unwrap(), value);
            }
        }
        Ok(PreparationOutcome::Complete(FileTerminal {
            inner,
            metadata,
            observations,
            certificate,
            rendered: Vec::new(),
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
            for input in &value.metadata {
                validate_capture(&input.status.capture, &context).await?;
            }
            let root = context.command.runtime.workspace.clone();
            let mode = self.mode.lock().unwrap().take().unwrap_or(FileMode::Normal);
            if matches!(mode, FileMode::BeforeCopy) {
                value.inner.publication.inputs = copied_metadata(&value.inner.publication.inputs);
            }
            if matches!(mode, FileMode::BeforeOther) {
                value.inner.publication.inputs = value
                    .metadata
                    .iter()
                    .find(|input| !Arc::ptr_eq(input, &value.metadata[0]))
                    .unwrap()
                    .clone();
            }
            if matches!(mode, FileMode::BeforeOrigin) {
                return self.other.complete(&mut value.inner, context).await;
            }
            let visible = final_bytes(&root);
            self.root.complete(&mut value.inner, context).await?;
            assert_eq!(visible, final_bytes(&root));
            let pending = value.inner.publication.pending.as_ref().unwrap();
            let names = stage_paths(&root, &pending.lock().unwrap());
            {
                let attempts = self.attempts.lock().unwrap();
                for previous in attempts.iter() {
                    assert!(previous.stage.upgrade().is_none());
                    assert!(previous.names.iter().all(|p| !p.exists()));
                    assert!(Arc::ptr_eq(
                        &previous.capture,
                        &value.inner.publication.inputs.status.capture
                    ));
                }
            }
            self.attempts.lock().unwrap().push(FileAttempt {
                capture: value.inner.publication.inputs.status.capture.clone(),
                stage: Arc::downgrade(pending),
                names,
                visible,
            });
            match &mode {
                FileMode::AfterCopy => {
                    value.inner.publication.inputs =
                        copied_metadata(&value.inner.publication.inputs)
                }
                FileMode::AfterOther => {
                    value.inner.publication.inputs = value
                        .metadata
                        .iter()
                        .find(|input| !Arc::ptr_eq(input, &value.metadata[0]))
                        .unwrap()
                        .clone()
                }
                FileMode::SourceEdit(path, bytes) => fs::write(path, bytes).unwrap(),
                FileMode::FailSecond => pending.lock().unwrap().fail_second(),
                FileMode::Abort => {
                    return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                        "abort after actual status stage and read permit"
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
        if matches!(
            mode,
            FileMode::AfterCopy | FileMode::AfterOther | FileMode::AfterOrigin
        ) {
            let error = if matches!(mode, FileMode::AfterOrigin) {
                self.other.publish_files(&mut value.inner).err().unwrap()
            } else {
                self.root.publish_files(&mut value.inner).err().unwrap()
            };
            clone.publication.inputs = value.metadata[0].clone();
            let spent = self.root.publish_files(&mut clone).err().unwrap();
            assert!(spent.to_string().contains("read permit spent"), "{spent}");
            assert!(clone.consumed.is_none());
            return Err(error);
        }
        if let FileMode::ReadBeforePublish(old) = mode {
            let error = self
                .root
                .consume_files(&mut value.inner, old)
                .err()
                .unwrap();
            assert!(
                error.to_string().contains("read permit phase differs"),
                "{error}"
            );
            let spent = self.root.publish_files(&mut clone).err().unwrap();
            assert!(spent.to_string().contains("read permit spent"), "{spent}");
            assert!(clone.consumed.is_none());
            return Err(error);
        }
        let files = self.root.publish_files(&mut value.inner)?;
        self.issued.lock().unwrap().push(Arc::downgrade(&files));
        if let FileMode::Mutate(kind) = mode {
            mutate_file(
                value
                    .inner
                    .publication
                    .inputs
                    .status
                    .capture
                    .workspace
                    .as_path(),
                &value.inner.publication.inputs,
                kind,
            );
        }
        if matches!(mode, FileMode::SmallScratch) {
            let mut scratch = [0; 1];
            return read_checked(&files, &value.inner.publication.inputs, &mut scratch).map(|_| ());
        }
        if let FileMode::OldReceipt(old) = mode {
            return self.root.consume_files(&mut value.inner, old);
        }
        self.root.consume_files(&mut value.inner, files.clone())?;
        if matches!(mode, FileMode::CloneOnce) {
            assert!(
                self.root.publish_files(&mut clone).is_err(),
                "clone cannot spend shared publication permit twice"
            );
            assert!(
                self.root.consume_files(&mut clone, files.clone()).is_err(),
                "clone cannot reuse shared read permit"
            );
        }
        if self.batch {
            for input in &value.metadata {
                let mut scratch = [0; 8236];
                let physical = read_checked(&files, input, &mut scratch)?;
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
                    .map_err(|error| std::io::Error::other(error.to_string()))?
                    .as_ref()
                    .as_ref()
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
                value.rendered.push(
                    view.callback()
                        .render_supplied_bytes(physical, source.bytes())
                        .unwrap(),
                );
            }
        }
        Ok(())
    }
}

fn consumed(value: &FileTerminal) -> &Arc<ConsumedBuildInfo> {
    value
        .inner
        .consumed
        .as_ref()
        .expect("accepted result requires actual file consumption")
}
fn expected_header(input: &BuildInfoNativeInputs) -> Vec<u8> {
    let raw = &input.status.capture.raw;
    header_bytes(
        if selected_descriptor(input).kind() == WorkspaceStatusArtifactKind::Volatile {
            0
        } else {
            1
        },
        raw.user.identity().unwrap(),
        raw.hostname.identity().unwrap().split('.').next().unwrap(),
        i128::from(raw.millis.unwrap() / 1000),
    )
}
fn assert_no_file_grant(
    runtime: &WorkspaceRuntime,
    root: &FileProbe,
    before: &AcceptedNativeDemandSnapshot,
) {
    assert_snapshot_same(runtime, before);
    revoked(runtime);
    for attempt in root.attempts.lock().unwrap().iter() {
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|path| !path.exists()));
    }
    assert!(
        root.issued
            .lock()
            .unwrap()
            .iter()
            .all(|weak| weak.upgrade().is_none())
    );
    no_stage_names(&runtime.workspace);
}
fn open_fd(fd: i32) -> bool {
    unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFD) >= 0 }
}

#[test]
fn native_file_inputs_read_both_roles_cover_configurations_and_release_four_readonly_fds() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let root = FileProbe::new(keys.clone(), 0, FileMode::CloneOnce, true);
    let driven = runtime
        .drive_command(native_request(&runtime), root.clone())
        .unwrap();
    let value = driven.accepted.terminal_for_test();
    let held = consumed(value).clone();
    assert_eq!(held.header, expected_header(&held.inputs));
    assert_ne!(
        held.header.as_ptr(),
        held.inputs.header.as_ptr(),
        "physical header independently owned"
    );
    assert_eq!(value.rendered.len(), value.metadata.len());
    let snapshot = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for (input, header) in value.metadata.iter().zip(&value.rendered) {
        assert!(Arc::ptr_eq(&input.status, &held.inputs.status));
        assert_eq!(header, &expected_header(input));
        let mut scratch = [0; 8236];
        assert_eq!(
            read_checked(&held.files, input, &mut scratch).unwrap(),
            input.status.bytes(selected_descriptor(input).kind())
        );
        for (demand, result) in input.template.observations().observations().iter() {
            assert_eq!(snapshot.path_observations.get(demand).unwrap(), result);
        }
    }
    assert_ne!(
        value.metadata[1].template.action().context().owner(),
        value.metadata[3].template.action().context().owner()
    );
    for flags in held.files.readonly_flags().unwrap() {
        assert_eq!(flags & nix::libc::O_ACCMODE, nix::libc::O_RDONLY);
    }
    let identities = held.files.identities().unwrap();
    for (index, name) in [STABLE, VOLATILE].iter().enumerate() {
        let fp = fingerprint(&final_path(workspace.path(), name));
        assert_eq!(identities[index], (fp.0, fp.1));
        assert_eq!(
            fs::read(final_path(workspace.path(), name)).unwrap(),
            held.inputs.status.bytes(if index == 0 {
                WorkspaceStatusArtifactKind::Stable
            } else {
                WorkspaceStatusArtifactKind::Volatile
            })
        );
    }
    let direct = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoFileInputsRoot::new(keys[1].clone()),
        )
        .unwrap();
    let stable = direct
        .accepted
        .terminal_for_test()
        .consumed
        .as_ref()
        .unwrap();
    assert_eq!(stable.header, expected_header(&stable.inputs));
    assert_eq!(
        direct
            .accepted
            .terminal_for_test()
            .publication
            .outcome
            .unwrap()
            .stable,
        StablePublication::Preserved
    );
    let expected_stable = held.inputs.status.stable.clone();
    let expected_volatile = held.inputs.status.volatile.clone();
    let expected_header = held.header.clone();
    let receipt = held.files.clone();
    let fds = receipt.raw_fds();
    let receipt_weak = Arc::downgrade(&receipt);
    let inputs_weak = Arc::downgrade(&held.inputs);
    let capture_weak = Arc::downgrade(&held.inputs.status.capture);
    drop(direct);
    drop(driven);
    drop(root);
    drop(keys);
    drop(runtime);
    assert_eq!(held.header, expected_header);
    assert_eq!(
        receipt
            .historical_bytes(WorkspaceStatusArtifactKind::Stable)
            .unwrap(),
        expected_stable
    );
    assert_eq!(
        receipt
            .historical_bytes(WorkspaceStatusArtifactKind::Volatile)
            .unwrap(),
        expected_volatile
    );
    assert!(fds.iter().copied().all(open_fd));
    drop(held);
    assert!(receipt_weak.upgrade().is_some());
    assert!(fds.iter().copied().all(open_fd));
    drop(receipt);
    assert!(receipt_weak.upgrade().is_none());
    for fd in fds {
        assert!(
            !open_fd(fd),
            "issued descriptor remains open after last receipt Drop"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(nix::libc::EBADF)
        );
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while (inputs_weak.upgrade().is_some() || capture_weak.upgrade().is_some())
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(inputs_weak.upgrade().is_none());
    assert!(capture_weak.upgrade().is_none());
}

#[test]
fn native_file_inputs_old_receipts_are_history_and_cannot_grant_equal_new_status() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let first = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoFileInputsRoot::new(keys[1].clone()),
        )
        .unwrap();
    let old = first
        .accepted
        .terminal_for_test()
        .consumed
        .as_ref()
        .unwrap()
        .clone();
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let old_ids = old.files.identities().unwrap();
    let visible = final_bytes(workspace.path());
    let premature = FileProbe::new(
        keys.clone(),
        1,
        FileMode::ReadBeforePublish(old.files.clone()),
        false,
    );
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), premature.clone()),
        Err(NativeDemandSessionError::Revision(
            RequestRevisionError::OutputPublication(_)
        ))
    ));
    assert!(premature.issued.lock().unwrap().is_empty());
    assert_eq!(final_bytes(workspace.path()), visible);
    assert_no_file_grant(&runtime, &premature, &before);
    let rejected = FileProbe::new(
        keys.clone(),
        1,
        FileMode::OldReceipt(old.files.clone()),
        false,
    );
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), rejected.clone()),
        Err(NativeDemandSessionError::Revision(
            RequestRevisionError::OutputPublication(_)
        ))
    ));
    assert_no_file_grant(&runtime, &rejected, &before);
    let second = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoFileInputsRoot::new(keys[3].clone()),
        )
        .unwrap();
    let new = second
        .accepted
        .terminal_for_test()
        .consumed
        .as_ref()
        .unwrap();
    assert!(!Arc::ptr_eq(&new.inputs.status, &old.inputs.status));
    assert_eq!(old.inputs.status.stable, new.inputs.status.stable);
    assert_eq!(old_ids[0], new.files.identities().unwrap()[0]);
    assert_ne!(old_ids[1], new.files.identities().unwrap()[1]);
    let mut scratch = [0; 8236];
    assert!(
        old.files
            .read(
                &new.inputs.status,
                new.inputs.status.artifacts.stable(),
                &mut scratch
            )
            .is_err()
    );
    assert!(
        old.files
            .read(
                &old.inputs.status,
                old.inputs.status.artifacts.stable(),
                &mut scratch
            )
            .is_err(),
        "current correspondence checks both roles; newer volatile retires the old pair"
    );
    assert!(
        old.files
            .read(
                &old.inputs.status,
                old.inputs.status.artifacts.volatile(),
                &mut scratch
            )
            .is_err()
    );
    assert_eq!(
        old.files
            .historical_bytes(WorkspaceStatusArtifactKind::Stable)
            .unwrap(),
        old.inputs.status.stable
    );
    assert_eq!(
        old.files
            .historical_bytes(WorkspaceStatusArtifactKind::Volatile)
            .unwrap(),
        old.inputs.status.volatile
    );
    let foreign = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/foreign").unwrap());
    assert!(
        new.files
            .read(&new.inputs.status, foreign.stable(), &mut scratch)
            .is_err()
    );
    assert!(
        new.files
            .read(
                &Arc::new(new.inputs.status.as_ref().clone()),
                new.inputs.status.artifacts.stable(),
                &mut scratch
            )
            .is_err()
    );
    revoked(&runtime);
}

#[test]
fn native_file_inputs_refuse_same_capture_metadata_and_foreign_root_before_effects() {
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
    for mode in [
        FileMode::BeforeCopy,
        FileMode::AfterCopy,
        FileMode::BeforeOther,
        FileMode::AfterOther,
        FileMode::BeforeOrigin,
        FileMode::AfterOrigin,
    ] {
        let root = FileProbe::new(keys.clone(), 0, mode, true);
        assert!(
            runtime
                .drive_command(native_request(&runtime), root.clone())
                .is_err()
        );
        assert_no_file_grant(&runtime, &root, &before);
        assert!(root.issued.lock().unwrap().is_empty());
        assert_eq!(
            final_bytes(workspace.path()),
            [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
        );
    }
}

#[test]
fn native_file_inputs_actual_read_refuses_inode_bytes_modes_alias_links_and_parent_changes() {
    for mutation in 0..9 {
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
        let root = FileProbe::new(keys, 1, FileMode::Mutate(mutation), false);
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
        assert_eq!(
            root.issued.lock().unwrap().len(),
            1,
            "mutation follows genuine two-role publication"
        );
        assert_no_file_grant(&runtime, &root, &before);
    }
}

#[test]
fn native_file_inputs_source_retry_aborts_partial_reads_and_publication_errors_grant_nothing() {
    let workspace = status_workspace();
    old_files(workspace.path());
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let build = workspace.path().join("BUILD.bazel");
    let mut bytes = fs::read(&build).unwrap();
    bytes.extend_from_slice(b"# physical file consumption retry\n");
    let retry = FileProbe::new(keys.clone(), 0, FileMode::SourceEdit(build, bytes), false);
    let driven = runtime
        .drive_command(native_request(&runtime), retry.clone())
        .unwrap();
    let value = driven.accepted.terminal_for_test();
    assert_eq!(
        consumed(value).header,
        expected_header(&consumed(value).inputs)
    );
    let attempts = retry.attempts.lock().unwrap();
    assert!(attempts.len() > 1);
    assert_eq!(retry.issued.lock().unwrap().len(), 1);
    assert_eq!(
        retry.publications.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    for attempt in attempts.iter() {
        assert_eq!(
            attempt.visible,
            [Some(OLD_STABLE.to_vec()), Some(OLD_VOLATILE.to_vec())]
        );
        assert!(Arc::ptr_eq(
            &attempt.capture,
            &consumed(value).inputs.status.capture
        ));
        assert!(attempt.stage.upgrade().is_none());
        assert!(attempt.names.iter().all(|p| !p.exists()));
    }
    assert!(value.observations.observations().values().any(|result| matches!(result.as_ref(),PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.ends_with(b"# physical file consumption retry\n"))));
    drop(attempts);
    drop(driven);
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let abort = FileProbe::new(keys.clone(), 0, FileMode::Abort, false);
    let existing = final_bytes(workspace.path());
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), abort.clone()),
        Err(NativeDemandSessionError::Computation(_))
    ));
    assert_eq!(final_bytes(workspace.path()), existing);
    assert_eq!(
        abort.publications.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_no_file_grant(&runtime, &abort, &before);
    for mode in [FileMode::FailSecond, FileMode::SmallScratch] {
        let fail_second = matches!(mode, FileMode::FailSecond);
        let old_volatile = fs::read(final_path(workspace.path(), VOLATILE)).unwrap();
        if fail_second {
            fs::set_permissions(
                final_path(workspace.path(), STABLE),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap();
            fs::write(final_path(workspace.path(), STABLE), OLD_STABLE).unwrap();
        }
        let root = FileProbe::new(keys.clone(), 0, mode, false);
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
        assert_eq!(root.issued.lock().unwrap().len(), usize::from(!fail_second));
        if fail_second {
            assert!(
                matches!(error, NativeDemandSessionError::Revision(RequestRevisionError::OutputPublication(ref detail)) if detail.contains("injected second status rename failure"))
            );
            let attempts = root.attempts.lock().unwrap();
            let raw = &attempts.last().unwrap().capture.raw;
            assert_eq!(
                fs::read(final_path(workspace.path(), STABLE)).unwrap(),
                stable_bytes(
                    raw.user.identity().unwrap(),
                    raw.hostname.identity().unwrap().split('.').next().unwrap()
                )
            );
            assert_eq!(
                fs::read(final_path(workspace.path(), VOLATILE)).unwrap(),
                old_volatile
            );
        }
        assert_no_file_grant(&runtime, &root, &before);
    }
    let recovered = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoFileInputsRoot::new(keys[0].clone()),
        )
        .unwrap();
    assert_eq!(
        recovered
            .accepted
            .terminal_for_test()
            .consumed
            .as_ref()
            .unwrap()
            .header,
        expected_header(&recovered.accepted.terminal_for_test().publication.inputs)
    );
    revoked(&runtime);
}
