//! Real, consuming General Archive finalization. No Process result or semantic
//! state is fabricated to obtain the smaller physical read-only quota.
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use sha2::Digest;

use super::*;
use crate::runtime::repository_physical_root::RepositoryWorkspaceRetirementObserver;
use crate::runtime::repository_physical_root::mount_lease::readonly_quota::FrozenArchiveQuota;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Facts {
    bytes: u64,
    inodes: u64,
    allocated: u64,
    used_inodes: u64,
}
fn facts(fd: i32) -> Facts {
    let mut value = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    assert_eq!(unsafe { libc::fstatvfs(fd, value.as_mut_ptr()) }, 0);
    let value = unsafe { value.assume_init() };
    assert_eq!((value.f_bsize, value.f_frsize), (4096, 4096));
    assert!(value.f_bfree <= value.f_blocks && value.f_ffree <= value.f_files);
    Facts {
        bytes: value.f_blocks as u64 * 4096,
        inodes: value.f_files as u64,
        allocated: (value.f_blocks - value.f_bfree) as u64 * 4096,
        used_inodes: (value.f_files - value.f_ffree) as u64,
    }
}
fn initialized(
    id: u64,
) -> (
    tempfile::TempDir,
    Arc<RepositoryWorkspaceReservation>,
    Arc<RepositoryRootResources>,
    usize,
    Arc<SourceServiceScope>,
    tokio::runtime::Runtime,
    RepositoryWorkspaceCommand,
    i32,
    RootIdentity,
) {
    let temporary = tempfile::tempdir().unwrap();
    let selected = reservation(
        &Path::new(wire::RESERVED_PREFIX).join(temporary.path().file_name().unwrap()),
        id,
    );
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope(&selected);
    let runtime = runtime();
    let launch =
        BootstrapLaunch::new(selected.clone(), bank.clone(), scope.clone(), &|| true).unwrap();
    let (builder, seed) = runtime
        .block_on(process_owner::run(launch, platform(), &|| true))
        .unwrap();
    let fd = builder.file.as_raw_fd();
    let identity = builder.identity;
    let command = RepositoryWorkspaceCommand::new(
        selected.workspace().clone(),
        selected.lease(),
        selected.limits(),
        bank.clone(),
        scope.clone(),
        &|| true,
    )
    .unwrap()
    .reserve_root(builder, seed, &scope, &|| true)
    .unwrap();
    (
        temporary, selected, bank, baseline, scope, runtime, command, fd, identity,
    )
}
fn apply(
    command: RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
    operation: RepositoryWorkspaceOperation,
    runtime: &tokio::runtime::Runtime,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceCommand {
    let request = command
        .request(selected, operation, scope, &|| true)
        .unwrap();
    command
        .apply(
            request,
            runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap()
}
struct CheckedFinalize {
    inner: FinalizeLaunch,
    original_fd: i32,
    before: Facts,
    verified: Arc<AtomicBool>,
    refuse_after_successor: bool,
}
impl ProcessLaunch for CheckedFinalize {
    type Output = <FinalizeLaunch as ProcessLaunch>::Output;
    fn mode(&self) -> RunMode {
        self.inner.mode()
    }
    fn workspace(&self) -> &NormalizedAbsolutePath {
        self.inner.workspace()
    }
    fn display_root(&self) -> &NormalizedAbsolutePath {
        self.inner.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        self.inner.request()
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        self.inner.covered_request_owner()
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        self.inner.root_identity()
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        self.inner.scope()
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        self.inner.resources()
    }
    fn retirement_observer(&self) -> Option<RepositoryWorkspaceRetirementObserver> {
        self.inner.retirement_observer()
    }
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        self.inner.bind_attempt(nonce)
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        self.inner.namespace()
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        self.inner.transfer(socket, nonce)
    }
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        self.inner
            .verify_successor(attached, token, userns, words, quota)?;
        // The helper has remounted, but it is blocked before ACK and has not
        // reaped. Physical observation alone MUST NOT refund the old debit.
        assert_eq!(
            self.resources().snapshot().backing_bytes,
            ARCHIVE_BYTES + process_owner::temporary_backing_for_test()
        );
        let after = facts(self.original_fd);
        assert_eq!((after.bytes, after.inodes), (PROCESS_BYTES, 8192));
        assert_eq!(
            (after.allocated, after.used_inodes),
            (self.before.allocated, self.before.used_inodes)
        );
        self.verified.store(true, Ordering::Release);
        if self.refuse_after_successor {
            return Err(PhysicalRootError::Inactive);
        }
        Ok(())
    }
    fn finish_reaped(self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        self.inner.finish_reaped(capture)
    }
}

#[test]
#[ignore = "actual compiled CLI General Archive RO same-backing contraction and preACK debit proof"]
fn native_small_archive_finalization_contracts_same_original_root_only_after_reap() {
    let (_temporary, selected, bank, baseline, scope, runtime, command, fd, identity) =
        initialized(2401);
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::File {
            path: "data".into(),
            body: Arc::from(&b"small archive"[..]),
            mode: 0o644,
        },
        &runtime,
        &scope,
    );
    let before = facts(fd);
    assert_eq!((before.bytes, before.inodes), (ARCHIVE_BYTES, 262144));
    let request = final_request(&command, &selected);
    let control = bank.reserve(4096, 0, 0).unwrap();
    let verified = Arc::new(AtomicBool::new(false));
    let admission = command
        .admit_view_finalization_for_test(request, &scope, &|| true)
        .unwrap();
    let checked = CheckedFinalize {
        inner: FinalizeLaunch::new(admission, &|| true).unwrap(),
        original_fd: fd,
        before,
        verified: verified.clone(),
        refuse_after_successor: false,
    };
    let (command, sealed, view) = runtime
        .block_on(process_owner::run(checked, platform(), &|| true))
        .unwrap();
    assert!(verified.load(Ordering::Acquire));
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(
        sealed.root().frozen_archive_quota(),
        Some(FrozenArchiveQuota::CompactArchive)
    );
    assert_eq!(
        sealed.root().state().mode(),
        RepositoryWorkspaceMode::Archive
    );
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
        &sealed.request().kind
    else {
        unreachable!()
    };
    assert_eq!(realization.transcript().records().len(), 1);
    assert!(matches!(
        realization.transcript().records()[0].request().operation(),
        RepositoryWorkspaceOperation::File { .. }
    ));
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    let mut reader = sealed.root().open_member("data", &scope, &|| true).unwrap();
    drop(command);
    drop(view);
    drop(sealed);
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    assert_eq!(
        bank.snapshot().native_fds,
        2,
        "only original data root and reader remain"
    );
    let mut body = Vec::new();
    reader.read_to_end(&mut body).unwrap();
    assert_eq!(body, b"small archive");
    drop(reader);
    drop(verified);
    drop(control);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI compact Archive RO child read/execute/EROFS and last-reader lifetime"]
fn native_compact_archive_view_is_readonly_and_data_outlives_all_controllers() {
    let Fixture {
        probe,
        sealed,
        bank,
        baseline,
        scope,
        runtime,
        ..
    } = fixture(false);
    assert_eq!(
        sealed.root().frozen_archive_quota(),
        Some(FrozenArchiveQuota::CompactArchive)
    );
    assert_eq!(
        sealed.root().state().mode(),
        RepositoryWorkspaceMode::Archive
    );
    let observed = runtime
        .block_on(process_owner::run(probe, platform(), &|| true))
        .unwrap();
    assert_eq!(observed.capture.return_code(), 0);
    assert_eq!(observed.capture.stdout(), b"alpha|tool|x");
    assert!(observed.capture.stderr().is_empty());
    let mut reader = sealed.root().open_member("data", &scope, &|| true).unwrap();
    drop(observed);
    drop(sealed);
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    assert_eq!(
        bank.snapshot().native_fds,
        2,
        "only original data root and reader remain"
    );
    let mut body = Vec::new();
    reader.read_to_end(&mut body).unwrap();
    assert_eq!(body, b"alpha");
    drop(reader);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI compact empty Archive preserves deleted65MiB non-Process history"]
fn native_archive_compaction_does_not_promote_deleted_large_history_to_process() {
    let (_temporary, selected, bank, baseline, scope, runtime, command, fd, identity) =
        initialized(2402);
    let producer = bank.reserve(2 * 65 * 1024 * 1024 + 4096, 0, 0).unwrap();
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::File {
            path: "large".into(),
            body: Arc::from(vec![0x5a; 65 * 1024 * 1024]),
            mode: 0o644,
        },
        &runtime,
        &scope,
    );
    drop(producer);
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Delete("large".into()),
        &runtime,
        &scope,
    );
    let before = facts(fd);
    assert!(before.allocated <= PROCESS_BYTES && before.used_inodes <= 8192);
    let request = final_request(&command, &selected);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(
        sealed.root().frozen_archive_quota(),
        Some(FrozenArchiveQuota::CompactArchive)
    );
    assert_eq!(
        sealed.root().state().mode(),
        RepositoryWorkspaceMode::Archive
    );
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
        &sealed.request().kind
    else {
        unreachable!()
    };
    let transcript = realization.transcript();
    assert_eq!(transcript.records().len(), 2);
    assert_eq!(transcript.retention().body_bytes(), 65 * 1024 * 1024);
    assert!(transcript.records().iter().all(|record| !matches!(
        record.request().operation(),
        RepositoryWorkspaceOperation::Execute(_)
    )));
    assert!(matches!(
        transcript.request(execute(&selected, &["/bin/true"], "")),
        Err(slug_bzlmod_v2::RepositoryWorkspaceError::RetentionLimit)
    ));
    assert_eq!((facts(fd).bytes, facts(fd).inodes), (PROCESS_BYTES, 8192));
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    drop(command);
    drop(view);
    drop(sealed);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI Archive extraction measured inode crossing retains full frozen quota"]
fn native_archive_measured_inode_crossing_keeps_full_quota_without_name_cost_assumption() {
    use std::io::Write;
    let (_temporary, selected, bank, baseline, scope, runtime, mut command, fd, identity) =
        initialized(2403);
    let directory = tempfile::tempdir().unwrap();
    let mut last_admitted = facts(fd);
    let mut crossing = None;
    let mut names = 0;
    // Existing archive decoder admits at most1024 entries per capture. Eight
    // independent genuine archives stay below128 state versions and64 source
    // attempts; directory entries avoid retaining file-body dedup scratch for
    // thousands of empty files. No cap or inode charging assumption changes.
    for batch in 0..8 {
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for index in 0..1024 {
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_entry_type(tar::EntryType::Directory);
            header.set_mode(0o755);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            archive
                .append_data(
                    &mut header,
                    format!("inode-{batch:02}-{index:04}"),
                    std::io::empty(),
                )
                .unwrap();
        }
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let mut file = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();
        let archive = slug_bzlmod_v2::GeneratedRepositoryArchiveEffect::new(
            Arc::from([url::Url::from_file_path(file.path())
                .unwrap()
                .to_string()
                .into()]),
            sha2::Sha256::digest(&bytes).into(),
            "".into(),
            slug_bzlmod_v2::GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            "".into(),
        )
        .unwrap();
        command = apply(
            command,
            &selected,
            RepositoryWorkspaceOperation::Archive(archive),
            &runtime,
            &scope,
        );
        names += 1024;
        let after = facts(fd);
        assert_eq!((after.bytes, after.inodes), (ARCHIVE_BYTES, 262144));
        assert!(
            after.used_inodes > last_admitted.used_inodes,
            "nonpositive charged-unit increase after archive batch {batch}"
        );
        assert!(
            after.allocated <= PROCESS_BYTES,
            "inode fallback must not be a byte fallback"
        );
        if after.used_inodes > 8192 {
            crossing = Some(after);
            break;
        }
        last_admitted = after;
    }
    let crossed = crossing.expect("bounded complete extraction must cross8192 actual inode units");
    assert!(last_admitted.used_inodes <= 8192 && crossed.used_inodes > 8192);
    eprintln!(
        "Archive compaction inode facts: named_entries={names}, last_sample_fitting={last_admitted:?}, first_sample_crossed={crossed:?}"
    );
    let request = final_request(&command, &selected);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(
        sealed.root().frozen_archive_quota(),
        Some(FrozenArchiveQuota::FullArchive)
    );
    assert_eq!(
        sealed.root().state().mode(),
        RepositoryWorkspaceMode::Archive
    );
    assert_eq!(facts(fd), crossed);
    assert_eq!(bank.snapshot().backing_bytes, ARCHIVE_BYTES);
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
        &sealed.request().kind
    else {
        unreachable!()
    };
    assert!(realization.transcript().records().len() <= 8);
    assert!(
        realization
            .transcript()
            .records()
            .iter()
            .all(|record| matches!(
                record.request().operation(),
                RepositoryWorkspaceOperation::Archive(_)
            ))
    );
    assert_eq!(realization.transcript().retention().body_bytes(), 0);
    drop(command);
    drop(view);
    drop(sealed);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI refusal after physical Archive contraction/successor and beforeACK must reap before refund"]
fn native_archive_compaction_refusal_after_successor_reaps_without_publishing_or_early_refund() {
    let (_temporary, selected, bank, baseline, scope, runtime, command, fd, _identity) =
        initialized(2404);
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::File {
            path: "data".into(),
            body: Arc::from(&b"unpublished"[..]),
            mode: 0o644,
        },
        &runtime,
        &scope,
    );
    let before = facts(fd);
    assert_eq!((before.bytes, before.inodes), (ARCHIVE_BYTES, 262144));
    let request = final_request(&command, &selected);
    let control = bank.reserve(4096, 0, 0).unwrap();
    let verified = Arc::new(AtomicBool::new(false));
    let admission = command
        .admit_view_finalization_for_test(request, &scope, &|| true)
        .unwrap();
    let checked = CheckedFinalize {
        inner: FinalizeLaunch::new(admission, &|| true).unwrap(),
        original_fd: fd,
        before,
        verified: verified.clone(),
        refuse_after_successor: true,
    };
    let retirement = checked.retirement_observer().unwrap();
    let result = runtime.block_on(process_owner::run(checked, platform(), &|| true));
    assert!(
        verified.load(Ordering::Acquire),
        "must observe actual compact quota and still-full old debit before refusing"
    );
    assert!(
        matches!(result, Err(PhysicalRootError::Inactive)),
        "a refused successor must publish no sealed/view tuple"
    );
    assert!(
        retirement.wait_until(Instant::now() + Duration::from_secs(1)),
        "owned worker must finish actual namespace cleanup/reap before generation retirement"
    );
    assert_eq!(
        (bank.snapshot().native_fds, bank.snapshot().backing_bytes),
        (0, 0)
    );
    scope.check().unwrap();
    drop(retirement);
    drop(verified);
    drop(control);
    release(&bank, baseline);
}
