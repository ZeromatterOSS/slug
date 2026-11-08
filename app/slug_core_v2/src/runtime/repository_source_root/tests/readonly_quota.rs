//! Actual Source bootstrap/capture/finalize proof. Logical digest-tree limits
//! remain independent of the physical read-only backing class.
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use super::*;
use crate::runtime::repository_io::RepositorySessionToken;
use crate::runtime::repository_io::source_snapshots::ReservedSourceSnapshot;
use crate::runtime::repository_physical_root::initialize as native_initialize;
use crate::runtime::repository_physical_root::mount_lease::readonly_quota::FrozenArchiveQuota;
use crate::runtime::repository_physical_root::mount_lease::source::FinalizedSource;
use crate::runtime::repository_physical_root::mount_lease::source::SourceBootstrap;
use crate::runtime::repository_physical_root::mount_lease::source::SourceFinalize;
use crate::runtime::repository_physical_root::process_owner;
use crate::runtime::repository_physical_root::process_owner::ProcessLaunch;
use crate::runtime::repository_physical_root::process_owner::ReapedCapture;
use crate::runtime::repository_process_wire::RunMode;

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
// A frozen Source is data-only even when its physical class still equals the
// original Archive quota. Same canonical association cannot reauthorize setup.
pub(super) fn assert_frozen_source(root: &RepositorySourceRoot, expected: FrozenArchiveQuota) {
    assert_eq!(root.frozen_archive_quota(), Some(expected));
    let actual = facts(root.file.as_raw_fd());
    assert_eq!((actual.bytes, actual.inodes), expected.quota());
    let before = root.resources.snapshot();
    assert!(matches!(
        root.backing
            .matches(&root.resources, RepositoryWorkspaceMode::Archive),
        Err(PhysicalRootError::Backing)
    ));
    assert!(
        root.backing
            .bind_source(root.identity, root.reservation())
            .is_err()
    );
    assert!(
        root.backing
            .bind_source_claim(root.reservation(), root.source_claim().unwrap())
            .is_err()
    );
    let after = root.resources.snapshot();
    assert_eq!(
        (after.native_fds, after.backing_bytes, after.retained_bytes),
        (
            before.native_fds,
            before.backing_bytes,
            before.retained_bytes
        )
    );
    assert_eq!(root.frozen_archive_quota(), Some(expected));
    assert_eq!(facts(root.file.as_raw_fd()), actual);
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    WrongNonce,
    WrongFacts,
    RefuseAfterSuccessor,
    CancelAfterSuccessor,
    DriftBeforeSeal,
}
struct CheckedFinalize {
    inner: SourceFinalize,
    original_fd: i32,
    original_identity: RootIdentity,
    before: Facts,
    expected: FrozenArchiveQuota,
    verified: Arc<AtomicBool>,
    fault: Fault,
}
impl ProcessLaunch for CheckedFinalize {
    type Output = FinalizedSource;
    fn mode(&self) -> RunMode {
        self.inner.mode()
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.inner.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.inner.display_root()
    }
    fn request(&self) -> Option<&slug_bzlmod_v2::RepositoryWorkspaceRequest> {
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
    fn retirement_observer(
        &self,
    ) -> Option<crate::runtime::repository_physical_root::RepositoryWorkspaceRetirementObserver>
    {
        self.inner.retirement_observer()
    }
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        let mut bound = *nonce;
        if self.fault == Fault::WrongNonce {
            bound[0] ^= 1;
        }
        self.inner.bind_attempt(&bound)
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
        mut quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        // The real helper already remounted, but is blocked before ACK. Neither
        // physical success nor a received successor may refund the old debit.
        assert_eq!(
            self.resources().snapshot().backing_bytes,
            ARCHIVE_BYTES + process_owner::temporary_backing_for_test()
        );
        let after = facts(self.original_fd);
        assert_eq!((after.bytes, after.inodes), self.expected.quota());
        assert_eq!(
            (after.allocated, after.used_inodes),
            (self.before.allocated, self.before.used_inodes)
        );
        assert_eq!(
            backing_identity_for_quota(&attached, self.expected.quota())?.device,
            self.original_identity.device
        );
        assert_eq!(
            backing_identity_for_quota(&attached, self.expected.quota())?.inode,
            self.original_identity.inode
        );
        let mut original_stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        assert_eq!(
            unsafe { libc::fstat(self.original_fd, original_stat.as_mut_ptr()) },
            0
        );
        let original_stat = unsafe { original_stat.assume_init() };
        assert_eq!(
            (original_stat.st_dev as u64, original_stat.st_ino as u64),
            (self.original_identity.device, self.original_identity.inode)
        );
        if self.fault == Fault::WrongFacts {
            quota
                .as_mut()
                .expect("Source finalization requires exactly one quota receipt")[6] += 4096;
        }
        self.inner
            .verify_successor(attached, token, userns, words, quota)?;
        self.verified.store(true, Ordering::Release);
        if self.fault == Fault::DriftBeforeSeal {
            let _duplicate_credit = self.resources().reserve(1024, 1, 0)?;
            let duplicate = unsafe { libc::fcntl(self.original_fd, libc::F_DUPFD_CLOEXEC, 0) };
            if duplicate < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let original = unsafe { File::from_raw_fd(duplicate) };
            let io = RootIo::new(
                &original,
                RootIoLimits {
                    max_path_bytes: 256,
                    max_name_bytes: 255,
                    max_depth: 32,
                    max_entries: 8192,
                    max_directory_name_bytes: 1024 * 1024,
                    max_file_bytes: 64 * 1024 * 1024,
                    max_link_bytes: 256,
                },
            )?;
            io.write_file(
                Path::new("payload"),
                b"changed after captured tree",
                0o644,
                crate::runtime::repository_root_io::WriteMode::ReplaceRegular,
                &|| true,
            )?;
        }
        if self.fault == Fault::CancelAfterSuccessor {
            self.scope().cancel();
        }
        if self.fault == Fault::RefuseAfterSuccessor {
            return Err(PhysicalRootError::Inactive);
        }
        Ok(())
    }
    fn finish_reaped(self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        self.inner.finish_reaped(capture)
    }
}

struct Fixture {
    builder: Option<SourceRootBuilder>,
    state: Option<Arc<State>>,
    state_credit: Option<ResourceCredit>,
    command: RepositoryWorkspaceCommand,
    ticket: ReservedSourceSnapshot,
    actor: RepositoryMaterializer,
    token: RepositorySessionToken,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    baseline: usize,
    runtime: tokio::runtime::Runtime,
}
impl Fixture {
    fn new(path: &Path) -> Self {
        let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(path).unwrap();
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        let scope = scope(&normalized, &resources);
        let actor = RepositoryMaterializer::new(normalized.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let token = actor.begin_with_command_scope(Some(scope.clone())).unwrap();
        actor.preflight_native(token, []).unwrap();
        let bounds = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
            128,
            16 * 1024 * 1024,
            1024 * 1024 * 1024,
        )
        .unwrap();
        let lease = actor
            .begin_general_workspace(token, resources.clone(), bounds, &scope)
            .unwrap();
        let request = request(path, lease);
        let ticket = actor
            .reserve_source_snapshot(token, request.clone(), &scope)
            .unwrap();
        let command = RepositoryWorkspaceCommand::new(
            normalized,
            lease,
            bounds,
            resources.clone(),
            scope.clone(),
            &|| true,
        )
        .unwrap();
        let input = SourceRootInput::capture(
            request,
            &resources,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
        let launch = SourceBootstrap::new(
            ticket.reservation().clone(),
            ticket.claim().clone(),
            resources.clone(),
            scope.clone(),
            None,
            command.retirement_observer(),
            &|| true,
        )
        .unwrap();
        let builder = runtime
            .block_on(process_owner::run(
                launch,
                native_initialize::platform().unwrap(),
                &|| true,
            ))
            .unwrap();
        let captured = input.populate(&builder, &scope, &|| true).unwrap();
        let (state, state_credit) = captured
            .into_state(ticket.reservation().clone(), &resources)
            .unwrap();
        Self {
            builder: Some(builder),
            state: Some(state),
            state_credit: Some(state_credit),
            command,
            ticket,
            actor,
            token,
            scope,
            resources,
            baseline,
            runtime,
        }
    }
    fn finalize(
        &mut self,
        expected: FrozenArchiveQuota,
        fault: Fault,
    ) -> (
        Result<FinalizedSource, PhysicalRootError>,
        Arc<AtomicBool>,
        i32,
        RootIdentity,
    ) {
        let builder = self.builder.take().unwrap();
        let fd = builder.file.as_raw_fd();
        let identity = builder.identity;
        let before = facts(fd);
        assert_eq!((before.bytes, before.inodes), (ARCHIVE_BYTES, 262144));
        let verified = Arc::new(AtomicBool::new(false));
        let launch = CheckedFinalize {
            inner: SourceFinalize::new(
                builder,
                self.state.take().unwrap(),
                self.state_credit.take().unwrap(),
                self.command.retirement_observer(),
            )
            .unwrap(),
            original_fd: fd,
            original_identity: identity,
            before,
            expected,
            verified: verified.clone(),
            fault,
        };
        let output = self.runtime.block_on(process_owner::run(
            launch,
            native_initialize::platform().unwrap(),
            &|| true,
        ));
        (output, verified, fd, identity)
    }
    fn cleanup(self) {
        let Self {
            builder,
            state,
            state_credit,
            command,
            ticket,
            actor,
            token,
            scope,
            resources,
            baseline,
            runtime,
        } = self;
        drop(builder);
        drop(state);
        drop(state_credit);
        let observer = command.retirement_observer();
        drop(command);
        assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
        drop(observer);
        drop(ticket);
        actor.discard(token).unwrap();
        drop(actor);
        drop(scope);
        drop(runtime);
        assert_eq!(
            (
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes,
                resources.snapshot().retained_bytes
            ),
            (0, 0, baseline)
        );
    }
}

#[test]
#[ignore = "actual CLI Source contraction, original-FD/shared debit and last-reader lifetime"]
fn native_small_source_contraction_keeps_original_identity_old_debit_until_reap_and_last_reader() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("payload"), b"source bytes").unwrap();
    let mut fixture = Fixture::new(workspace.path());
    let (output, verified, fd, identity) =
        fixture.finalize(FrozenArchiveQuota::CompactArchive, Fault::None);
    let output = output.unwrap();
    assert!(verified.load(Ordering::Acquire));
    let root = output.receipt.root();
    assert_eq!(root.file.as_raw_fd(), fd);
    assert_eq!(root.identity, identity);
    assert_eq!(
        root.frozen_archive_quota(),
        Some(FrozenArchiveQuota::CompactArchive)
    );
    assert_frozen_source(root, FrozenArchiveQuota::CompactArchive);
    assert_eq!(fixture.resources.snapshot().backing_bytes, PROCESS_BYTES);
    assert!(!root.anchor().root().as_path().exists());
    let mut reader = root
        .open_member("payload", &fixture.scope, &|| true)
        .unwrap();
    let resources = fixture.resources.clone();
    let baseline = fixture.baseline;
    let scope = fixture.scope.clone();
    drop(output);
    let Fixture {
        builder,
        state,
        state_credit,
        command,
        ticket,
        actor,
        token,
        scope: fixture_scope,
        resources: fixture_resources,
        runtime,
        ..
    } = fixture;
    drop(builder);
    drop(state);
    drop(state_credit);
    let observer = command.retirement_observer();
    drop(command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    drop(ticket);
    actor.discard(token).unwrap();
    drop(actor);
    drop(fixture_scope);
    drop(fixture_resources);
    drop(runtime);
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    let mut body = Vec::new();
    reader.read_to_end(&mut body).unwrap();
    assert_eq!(body, b"source bytes");
    drop(reader);
    drop(scope);
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual CLI Source quota nonce/facts/refusal/cancel/full-seal failure retirement"]
fn native_source_contraction_refusals_reap_without_publishing_or_refunding_live_backing() {
    for fault in [
        Fault::WrongNonce,
        Fault::WrongFacts,
        Fault::RefuseAfterSuccessor,
        Fault::CancelAfterSuccessor,
        Fault::DriftBeforeSeal,
    ] {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("payload"), b"source bytes").unwrap();
        let mut fixture = Fixture::new(workspace.path());
        let (output, verified, _, _) = fixture.finalize(FrozenArchiveQuota::CompactArchive, fault);
        assert!(
            output.is_err(),
            "no sealed Source/view may escape a refused attempt"
        );
        if matches!(
            fault,
            Fault::RefuseAfterSuccessor | Fault::CancelAfterSuccessor | Fault::DriftBeforeSeal
        ) {
            assert!(verified.load(Ordering::Acquire));
        }
        assert_eq!(fixture.resources.snapshot().backing_bytes, 0);
        fixture.cleanup();
    }
}

#[test]
#[ignore = "actual CLI Source physical inode fallback inside unchanged logical Source limits"]
fn native_source_contraction_keeps_full_quota_when_actual_charged_inodes_cross_compact_allowance() {
    let workspace = tempfile::tempdir().unwrap();
    // Long names increase real tmpfs metadata charges without increasing logical
    // entries beyond8192 or path bytes beyond1MiB. No one-name/one-unit assumption.
    for index in 0..7900 {
        std::fs::create_dir(
            workspace
                .path()
                .join(format!("{index:04}-{}", "n".repeat(115))),
        )
        .unwrap();
    }
    let mut fixture = Fixture::new(workspace.path());
    let state = fixture.state.as_ref().unwrap();
    assert!(state.entries().len() <= 8192);
    assert!(
        state
            .entries()
            .iter()
            .map(|e| e.path().len())
            .sum::<usize>()
            <= 1024 * 1024
    );
    let before = facts(fixture.builder.as_ref().unwrap().file.as_raw_fd());
    assert!(
        before.used_inodes > 8192,
        "actual charges must cross compact allowance: {before:?}"
    );
    assert!(
        before.allocated <= PROCESS_BYTES,
        "isolate inode fallback: {before:?}"
    );
    let (output, verified, _, _) = fixture.finalize(FrozenArchiveQuota::FullArchive, Fault::None);
    let output = output.unwrap();
    assert!(verified.load(Ordering::Acquire));
    assert_eq!(
        output.receipt.root().frozen_archive_quota(),
        Some(FrozenArchiveQuota::FullArchive)
    );
    assert_frozen_source(output.receipt.root(), FrozenArchiveQuota::FullArchive);
    assert_eq!(fixture.resources.snapshot().backing_bytes, ARCHIVE_BYTES);
    eprintln!(
        "source logical entries={} observed inode units={} allocated={}",
        output.receipt.state().entries().len(),
        before.used_inodes,
        before.allocated
    );
    drop(output);
    fixture.cleanup();
}

#[test]
#[ignore = "actual CLI compact Source read-only view renewal across two genuine Execute records"]
fn native_compact_source_views_remain_readonly_across_two_executes_and_data_outlives_controllers() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("payload"), b"small source").unwrap();
    let normalized = slug_workspace_v2::NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope(&normalized, &resources);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized);
    let (token, command, source) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &scope,
        &runtime,
        false,
    )
    .unwrap();
    assert_eq!(
        source.frozen_archive_quota(),
        Some(FrozenArchiveQuota::CompactArchive)
    );
    assert_eq!(
        (
            facts(source.file.as_raw_fd()).bytes,
            facts(source.file.as_raw_fd()).inodes
        ),
        (PROCESS_BYTES, 8192)
    );
    let (mut command, executor) =
        source_links::general(&actor, token, command, &resources, &scope, &runtime);
    let view =
        slug_bzlmod_v2::RepositoryWorkspaceReadView::from_source(source.state().clone()).unwrap();
    let manifest =
        slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::new(&executor, vec![view]).unwrap();
    let selected = actor.selected_epoch(token, &[]).unwrap();
    let code = r#"import errno,os,pathlib,sys
p=pathlib.Path(sys.argv[1])
v=os.statvfs(p)
assert v.f_blocks*v.f_frsize==67108864 and v.f_files==8192
assert (p/'payload').read_bytes()==b'small source'
for name in ['payload','new']:
 try: os.open(p/name,os.O_WRONLY|os.O_CREAT,0o644)
 except OSError as e: assert e.errno==errno.EROFS
 else: raise AssertionError('Source view writable')
c=pathlib.Path('counter')
n=int(c.read_text())+1 if c.exists() else 1
c.write_text(str(n))
print(n)
"#;
    for index in 0..2 {
        let execute = slug_bzlmod_v2::RepositoryWorkspaceExecute::new(
            [
                "/usr/bin/python3",
                "-c",
                code,
                source
                    .reservation()
                    .display_root()
                    .as_path()
                    .to_str()
                    .unwrap(),
            ]
            .iter()
            .map(|s| slug_bzlmod_v2::RepositoryWorkspaceArgument::String((*s).into()))
            .collect::<Vec<_>>()
            .into(),
            slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
            starlark_map::small_map::SmallMap::new(),
            600,
            true,
            slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                executor.display_root().clone(),
                PathObservationNamespace::Materialization(executor.instance()),
            )
            .unwrap(),
            manifest.clone(),
        )
        .unwrap();
        let request = command
            .request(
                &executor,
                slug_bzlmod_v2::RepositoryWorkspaceOperation::Execute(execute),
                &scope,
                &|| true,
            )
            .unwrap();
        command = runtime
            .block_on(actor.execute_workspace_with_sources(
                token,
                command,
                request,
                &[],
                &selected,
                std::slice::from_ref(source.state()),
                &scope,
                &|| true,
            ))
            .unwrap();
        let transcript = command
            .epoch()
            .transcripts()
            .unwrap()
            .iter()
            .find(|t| t.reservation() == &executor)
            .unwrap();
        let record = &transcript.records()[index];
        let slug_bzlmod_v2::RepositoryWorkspaceResult::Execute(result) = record.result() else {
            panic!("actual Execute record required")
        };
        assert_eq!(result.return_code(), 0);
        assert_eq!(result.stdout(), format!("{}\n", index + 1).as_bytes());
    }
    let mut reader = source.open_member("payload", &scope, &|| true).unwrap();
    let observer = command.retirement_observer();
    drop(command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    drop(executor);
    drop(manifest);
    drop(selected);
    actor.discard(token).unwrap();
    drop(actor);
    drop(source);
    assert_eq!(resources.snapshot().backing_bytes, PROCESS_BYTES);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"small source");
    drop(reader);
    drop(scope);
    drop(runtime);
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual claimed Source bootstrap refuses a different current reservation claim before creation"]
fn native_source_contraction_cannot_borrow_another_reservations_claim() {
    let workspace = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(workspace.path());
    let other = RepositoryMaterializer::new(fixture.actor.workspace().clone());
    let other_token = other
        .begin_with_command_scope(Some(fixture.scope.clone()))
        .unwrap();
    other.preflight_native(other_token, []).unwrap();
    let bounds = slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits::new(
        128,
        16 * 1024 * 1024,
        1024 * 1024 * 1024,
    )
    .unwrap();
    let lease = other
        .begin_general_workspace(
            other_token,
            fixture.resources.clone(),
            bounds,
            &fixture.scope,
        )
        .unwrap();
    let ticket = other
        .reserve_source_snapshot(
            other_token,
            request(workspace.path(), lease),
            &fixture.scope,
        )
        .unwrap();
    let before = fixture.resources.snapshot();
    assert!(matches!(
        SourceBootstrap::new(
            fixture.ticket.reservation().clone(),
            ticket.claim().clone(),
            fixture.resources.clone(),
            fixture.scope.clone(),
            None,
            fixture.command.retirement_observer(),
            &|| true
        ),
        Err(PhysicalRootError::Reservation)
    ));
    let after = fixture.resources.snapshot();
    assert_eq!(
        (after.native_fds, after.backing_bytes, after.retained_bytes),
        (
            before.native_fds,
            before.backing_bytes,
            before.retained_bytes
        )
    );
    assert_eq!(after.peak_backing_bytes, before.peak_backing_bytes);
    drop(ticket);
    other.discard(other_token).unwrap();
    drop(other);
    fixture.cleanup();
}
