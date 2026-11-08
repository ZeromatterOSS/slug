//! Physical mechanism proof only. These extra views NEVER produce a semantic
//! Execute record or command epoch. Ordinary ExecuteLaunch still has no views.
use std::io::Read;

use super::super::super::readonly::PendingReadOnlyView;
use super::super::super::readonly::ReadOnlyViewAttempt;
use super::super::super::readonly::ReadOnlyViewInFlight;
use super::super::super::readonly::ViewUseBinding;
use super::*;
use crate::runtime::repository_process_wire as wire;

enum Root {
    First(ProcessQuotaTransition),
    Again {
        admission: NativeProcessAdmission,
        incoming: Option<AttachedMountClone>,
        pending: Option<AttachedMountClone>,
    },
}
struct Probe {
    root: Option<Root>,
    domain: Arc<NativeMountDomain>,
    view: Option<FinalizedRepositoryView>,
    attempt: Option<ReadOnlyViewAttempt>,
    inflight: Option<ReadOnlyViewInFlight>,
    pending: Option<PendingReadOnlyView>,
    entries: Vec<wire::ViewEntry>,
    stale_capture: Option<ReapedCapture>,
    binding: Option<Arc<ViewUseBinding>>,
    // Covers heap metadata and conservative cross-process old/incoming/new
    // descriptor overlap, held through actual namespace and monitor death.
    _credit: resources::ResourceCredit,
}
struct Observed {
    probe: Probe,
    capture: ReapedCapture,
}
impl Probe {
    fn new(root: Root, view: FinalizedRepositoryView, domain: Arc<NativeMountDomain>) -> Self {
        let bank = match &root {
            Root::First(q) => &q.builder().resources,
            Root::Again { admission, .. } => &admission.builder().resources,
        };
        let credit = bank.reserve(64 * 1024, 8, 0).unwrap();
        let mut result = Self {
            root: Some(root),
            domain,
            view: Some(view),
            attempt: None,
            inflight: None,
            pending: None,
            entries: Vec::new(),
            stale_capture: None,
            binding: None,
            _credit: credit,
        };
        result.arm();
        result
    }
    fn builder(&self) -> &RepositoryPhysicalRootBuilder {
        match self.root.as_ref().unwrap() {
            Root::First(q) => q.builder(),
            Root::Again { admission, .. } => admission.builder(),
        }
    }
    fn executor(&self) -> &RepositoryWorkspaceRequest {
        match self.root.as_ref().unwrap() {
            Root::First(q) => q.request(),
            Root::Again { admission, .. } => admission.request(),
        }
    }
    fn arm(&mut self) {
        let view = self.view.take().unwrap();
        assert!(Arc::ptr_eq(view.domain(), &self.domain));
        let id = view.attachment_identity();
        self.entries = vec![wire::ViewEntry {
            display: std::ffi::CString::new(
                view.display().as_path().as_os_str().as_encoded_bytes(),
            )
            .unwrap(),
            instance: view.instance().value(),
            device: id.device,
            inode: id.inode,
            mount: id.mount,
        }];
        self.attempt = Some(view.begin(self.scope(), &|| true).unwrap());
    }
}
impl ProcessLaunch for Probe {
    type Output = Observed;
    fn mode(&self) -> RunMode {
        match self.root.as_ref().unwrap() {
            Root::First(_) => RunMode::FirstExecute,
            Root::Again { .. } => RunMode::ContinueExecute,
        }
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder().reservation.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder().reservation.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        Some(self.executor())
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        Some(match self.root.as_ref().unwrap() {
            Root::First(q) => q.covered_request_owner(),
            Root::Again { admission, .. } => admission.request_cover(),
        })
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        Some(self.builder().identity)
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        match self.root.as_ref().unwrap() {
            Root::First(q) => q.scope(),
            Root::Again { admission, .. } => admission.scope(),
        }
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.builder().resources
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(Some(self.domain.borrow_namespace()))
    }
    fn views(&self) -> &[wire::ViewEntry] {
        &self.entries
    }
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        self.binding = Some(
            ViewUseBinding::new(
                self.covered_request_owner().unwrap(),
                self.builder().identity,
                *nonce,
                self.resources(),
            )
            .map_err(PhysicalRootError::mount)?,
        );
        Ok(())
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        match self.root.as_mut().unwrap() {
            Root::First(q) => q.transfer(socket, nonce, &|| true),
            Root::Again {
                admission,
                incoming,
                ..
            } => {
                let clone = incoming.as_ref().ok_or(PhysicalRootError::Mode)?;
                self.domain
                    .check_clone(clone, admission.scope(), &|| true)
                    .map_err(PhysicalRootError::mount)?;
                send_incoming(
                    socket,
                    &admission.builder().file,
                    &clone.token,
                    &clone.owner,
                    nonce,
                    admission.request().index(),
                )
                .map_err(PhysicalRootError::mount)
            }
        }
    }
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        let successor = received_successor(
            &attached,
            token,
            userns,
            words,
            &self.builder().resources,
            self.builder().backing.clone(),
        )?;
        self.domain
            .check_clone(&successor, self.scope(), &|| true)
            .map_err(PhysicalRootError::mount)?;
        match self.root.as_mut().unwrap() {
            Root::First(q) => q.verify_and_commit(
                &attached,
                successor,
                quota_words(quota.ok_or(PhysicalRootError::Mode)?),
                &|| true,
            ),
            Root::Again {
                admission,
                incoming,
                pending,
            } => {
                if quota.is_some()
                    || pending.is_some()
                    || successor.attached
                        != incoming.as_ref().ok_or(PhysicalRootError::Mode)?.detached
                    || !same_backing(successor.attached, admission.builder().identity)
                    || backing_identity(&attached, RepositoryWorkspaceMode::Process)?
                        != successor.attached
                    || backing_identity(
                        &admission.builder().file,
                        RepositoryWorkspaceMode::Process,
                    )? != admission.builder().identity
                {
                    return Err(PhysicalRootError::Backing);
                }
                *pending = Some(successor);
                Ok(())
            }
        }
    }
    fn transfer_view(
        &mut self,
        slot: usize,
        socket: BorrowedFd<'_>,
        _nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        if slot != 0 {
            return Err(PhysicalRootError::Mode);
        }
        let binding = self.binding.as_ref().unwrap().clone();
        let scope = self.scope().clone();
        let next = self
            .attempt
            .as_mut()
            .ok_or(PhysicalRootError::Mode)?
            .transfer(socket, binding, slot, &scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(self.attempt.take());
        self.inflight = Some(next);
        Ok(())
    }
    fn verify_view_successor(
        &mut self,
        slot: usize,
        attached: File,
        token: File,
        words: [u64; 8],
    ) -> Result<(), PhysicalRootError> {
        if slot != 0 || self.pending.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let scope = self.scope().clone();
        let next = self
            .inflight
            .as_mut()
            .ok_or(PhysicalRootError::Mode)?
            .stage_successor(attached, token, words, &scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(self.inflight.take());
        self.pending = Some(next);
        if let Some(stale) = &self.stale_capture {
            assert!(
                self.pending
                    .as_ref()
                    .unwrap()
                    .check_capture_for_test(stale)
                    .is_err(),
                "an earlier actual launch cannot release this new view token while its namespace is alive before ACK"
            );
        }
        Ok(())
    }
    fn finish_reaped(mut self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        capture
            .binding()
            .verify(self.executor(), self.builder().identity)?;
        let scope = self.scope().clone();
        let view = self
            .pending
            .as_mut()
            .ok_or(PhysicalRootError::Mode)?
            .accept_after_reap(&capture, &scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(self.pending.take());
        let (admission, clone) = match self.root.take().unwrap() {
            Root::First(q) => q.into_native_probe_after_reap(&capture)?,
            Root::Again {
                admission,
                incoming,
                pending,
            } => {
                drop(incoming);
                (admission, pending.ok_or(PhysicalRootError::Mode)?)
            }
        };
        self.root = Some(Root::Again {
            admission,
            incoming: Some(clone),
            pending: None,
        });
        self.view = Some(view);
        self.binding = None;
        Ok(Observed {
            probe: self,
            capture,
        })
    }
}

struct Fixture {
    probe: Probe,
    sealed: SealedWorkspaceMaterialization,
    a: Arc<RepositoryWorkspaceReservation>,
    b: Arc<RepositoryWorkspaceReservation>,
    bank: Arc<RepositoryRootResources>,
    baseline: usize,
    scope: Arc<SourceServiceScope>,
    runtime: tokio::runtime::Runtime,
}
fn final_request(
    command: &RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
) -> Arc<slug_bzlmod_v2::RepositoryMaterializationRequest> {
    let transcript = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation().as_ref() == selected)
        .unwrap();
    Arc::new(slug_bzlmod_v2::RepositoryMaterializationRequest {
        id: slug_bzlmod_v2::RepositoryMaterializationRequestId {
            workspace: selected.workspace().clone(),
            canonical_repo: selected.repository().clone(),
        },
        repo_spec: slug_bzlmod_v2::RepoSpec {
            rule_id: slug_bzlmod_v2::RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
                rule_name: "generic".into(),
            },
            attributes: Arc::new(SmallMap::new()),
        },
        kind: slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(
            transcript.realize(transcript.records().len()).unwrap(),
        ),
    })
}
fn fixture(large: bool) -> Fixture {
    let temporary = tempfile::tempdir().unwrap();
    let leaf = temporary.path().file_name().unwrap().to_str().unwrap();
    let a = reservation(
        &Path::new(wire::RESERVED_PREFIX).join(format!("{leaf}-a")),
        2201,
    );
    let b = Arc::new(
        RepositoryWorkspaceReservation::new(
            a.workspace().clone(),
            a.owner().clone(),
            slug_identity_v2::CanonicalRepoName::new(format!(
                "{}+executor",
                a.owner().unique_name().as_str()
            ))
            .unwrap(),
            1,
            a.expected_repo_spec().clone(),
            NormalizedAbsolutePath::new(Path::new(wire::RESERVED_PREFIX).join(format!("{leaf}-b")))
                .unwrap(),
            slug_workspace_v2::PathObservationInstanceId::new(2202),
            a.lease(),
            a.limits(),
        )
        .unwrap(),
    );
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope(&a);
    let runtime = runtime();
    let mut command = start(a.clone(), bank.clone(), scope.clone(), &runtime);
    for (path, body, mode) in [
        ("data", b"alpha".as_slice(), 0o666),
        ("tool", b"#!/bin/sh\nprintf tool".as_slice(), 0o755),
    ] {
        let request = command
            .request(
                &a,
                RepositoryWorkspaceOperation::File {
                    path: path.into(),
                    body: Arc::from(body),
                    mode,
                },
                &scope,
                &|| true,
            )
            .unwrap();
        command = command
            .apply(
                request,
                &runtime,
                &crate::runtime::ArchiveDistdirPolicy::default(),
                &scope,
                &|| true,
            )
            .unwrap();
    }
    // Add the large member LAST, so earlier immutable states do not charge
    // repeated occurrences. It belongs to A's Archive allowance, never B's.
    if large {
        let request = command
            .request(
                &a,
                RepositoryWorkspaceOperation::File {
                    path: "large".into(),
                    body: Arc::from(vec![0x5a; 65 * 1024 * 1024]),
                    mode: 0o644,
                },
                &scope,
                &|| true,
            )
            .unwrap();
        command = command
            .apply(
                request,
                &runtime,
                &crate::runtime::ArchiveDistdirPolicy::default(),
                &scope,
                &|| true,
            )
            .unwrap();
    }
    let materialization = final_request(&command, &a);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(materialization, &scope, &|| true))
        .unwrap();
    let domain = command.mount_domain().unwrap().clone();
    let producer = BootstrapLaunch::new_in_domain(
        b.clone(),
        bank.clone(),
        scope.clone(),
        Some(domain.clone()),
        &|| true,
    )
    .unwrap();
    let (builder, seed) = runtime
        .block_on(process_owner::run(producer, platform(), &|| true))
        .unwrap();
    domain
        .check_initial(&builder, &seed, &scope, &|| true)
        .unwrap();
    let command = command
        .reserve_root(builder, seed, &scope, &|| true)
        .unwrap();
    assert!(!a.display_root().as_path().exists());
    assert!(!b.display_root().as_path().exists());
    let operation = execute(
        &b,
        &[
            "python3",
            "-c",
            r#"import errno,os,pathlib,subprocess,sys
a=pathlib.Path(sys.argv[1])
assert (a/'data').stat().st_mode & 0o777 == 0o666
if (a/'large').exists(): assert (a/'large').stat().st_size == 65*1024*1024
assert os.statvfs(a).f_blocks*os.statvfs(a).f_frsize == 2147483648
assert os.statvfs('.').f_blocks*os.statvfs('.').f_frsize == 67108864
try: os.open(a/'data',os.O_WRONLY|os.O_APPEND)
except OSError as e: assert e.errno == errno.EROFS
else: raise AssertionError('readonly source was writable')
with open('repeated','ab') as f: f.write(b'x')
sys.stdout.buffer.write((a/'data').read_bytes()+b'|'+subprocess.check_output([a/'tool'])+b'|'+pathlib.Path('repeated').read_bytes())
"#,
            a.display_root().as_path().to_str().unwrap(),
        ],
        "",
    );
    let request = command.request(&b, operation, &scope, &|| true).unwrap();
    let ExecuteDecision::Launch(ExecuteLaunch::First(quota)) =
        command.admit_execute(request, &scope, &|| true).unwrap()
    else {
        panic!("first actual admission")
    };
    let probe = Probe::new(Root::First(quota), view, domain);
    Fixture {
        probe,
        sealed,
        a,
        b,
        bank,
        baseline,
        scope,
        runtime,
    }
}

#[test]
#[ignore = "actual compiled CLI shared-userns RO/RW renewal proof; finite native supervisor required"]
fn native_shared_domain_readonly_view_renews_while_same_executor_root_remains_writable() {
    let Fixture {
        mut probe,
        sealed,
        a,
        b,
        bank,
        baseline,
        scope,
        runtime,
    } = fixture(true);
    assert!(
        matches!(sealed.root().state().entry("large").unwrap().node(),slug_bzlmod_v2::RepositoryWorkspaceNode::File(body) if body.len()==65*1024*1024)
    );
    for expected in [b"alpha|tool|x".as_slice(), b"alpha|tool|xx".as_slice()] {
        let observed = runtime
            .block_on(process_owner::run(probe, platform(), &|| true))
            .unwrap();
        assert_eq!(observed.capture.return_code(), 0);
        assert_eq!(observed.capture.stdout(), expected);
        assert!(observed.capture.stderr().is_empty());
        let mut data = Vec::new();
        sealed
            .root()
            .open_member("data", &scope, &|| true)
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        assert_eq!(data, b"alpha");
        assert_eq!(
            observed.probe.builder().mode,
            RepositoryWorkspaceMode::Process
        );
        assert!(!a.display_root().as_path().exists());
        assert!(!b.display_root().as_path().exists());
        let Observed {
            probe: next,
            capture,
        } = observed;
        probe = next;
        probe.stale_capture = Some(capture);
        probe.arm();
    }
    let retirement = match probe.root.as_ref().unwrap() {
        Root::First(q) => q.retirement_observer(),
        Root::Again { admission, .. } => admission.retirement_observer(),
    };
    let renewed = probe
        .attempt
        .take()
        .expect("renewed view armed after both launches");
    drop(probe);
    assert!(
        !retirement.wait_until(std::time::Instant::now()),
        "renewed standalone view must retain original retirement custody after command drop"
    );
    drop(renewed);
    assert!(retirement.wait_until(std::time::Instant::now() + Duration::from_secs(1)));
    assert_eq!(bank.snapshot().backing_bytes, ARCHIVE_BYTES);
    assert_eq!(
        bank.snapshot().native_fds,
        1,
        "only sealed Archive data remains after renewed view retirement"
    );
    drop(retirement);
    drop(sealed);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI Process finalization/data-only lifetime proof; finite native supervisor required"]
fn native_process_finalization_preserves_actual_record_and_data_after_setup_authority_drops() {
    let temporary = tempfile::tempdir().unwrap();
    let selected = reservation(
        &Path::new(wire::RESERVED_PREFIX).join(temporary.path().file_name().unwrap()),
        2203,
    );
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope(&selected);
    let runtime = runtime();
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let command = run(
        command,
        &selected,
        execute(
            &selected,
            &["sh", "-c", "printf process > data; mkdir actual"],
            "",
        ),
        &scope,
        &runtime,
    );
    let count = command.epoch().transcripts().unwrap()[0].records().len();
    assert_eq!(count, 1);
    let request = final_request(&command, &selected);
    let retirement = command.retirement_observer();
    let finalized = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    let (send, receive) = tokio::sync::oneshot::channel();
    assert!(send.send(finalized).is_ok());
    assert!(!retirement.wait_until(std::time::Instant::now()));
    let (command, sealed, view) = runtime.block_on(receive).unwrap();
    assert_eq!(
        command.epoch().transcripts().unwrap()[0].records().len(),
        count
    );
    assert_eq!(sealed.root().mode, RepositoryWorkspaceMode::Process);
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    let mut reader = sealed.root().open_member("data", &scope, &|| true).unwrap();
    drop(command);
    assert!(
        !retirement.wait_until(std::time::Instant::now()),
        "standalone finalized view still owns setup descriptors/domain"
    );
    drop(view);
    assert!(retirement.wait_until(std::time::Instant::now() + Duration::from_secs(1)));
    drop(retirement);
    assert_eq!(
        bank.snapshot().backing_bytes,
        PROCESS_BYTES,
        "legitimate sealed data survives setup retirement"
    );
    drop(sealed);
    assert_eq!(
        bank.snapshot().native_fds,
        2,
        "data root+reader retain no setup namespace/token descriptors"
    );
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"process");
    assert!(!selected.display_root().as_path().exists());
    drop(reader);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI cancellation after real RO transfer; finite native supervisor required"]
fn native_readonly_transfer_cancellation_keeps_sole_source_backing_until_actual_cleanup() {
    let Fixture {
        probe,
        sealed,
        bank,
        baseline,
        scope: _,
        runtime,
        ..
    } = fixture(false);
    drop(sealed); // The transferred view is now A's sole native data owner.
    let stage = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    runtime.block_on(async {
        let active=||true;
        let mut future=Box::pin(process_owner::run(probe,platform().observe_view_transfer_for_test(stage.clone()),&active));
        tokio::time::timeout(Duration::from_secs(3),async {
            loop {
                tokio::select! {
                    result=&mut future=>panic!("view transfer gate returned before cancellation: {}",result.is_ok()),
                    _=tokio::time::sleep(Duration::from_millis(2))=>if stage.load(std::sync::atomic::Ordering::Acquire)==2 {break},
                }
            }
        }).await.unwrap();
        assert_eq!(bank.snapshot().backing_bytes,ARCHIVE_BYTES+PROCESS_BYTES+process_owner::temporary_backing_for_test());
        assert!(bank.snapshot().native_fds>0);
        drop(future);
        tokio::time::timeout(Duration::from_secs(6),async {
            loop {let now=bank.snapshot();if (now.native_fds,now.backing_bytes,now.retained_bytes)==(0,0,baseline){break}tokio::time::sleep(Duration::from_millis(2)).await;}
        }).await.unwrap();
    });
    release(&bank, baseline);
}
