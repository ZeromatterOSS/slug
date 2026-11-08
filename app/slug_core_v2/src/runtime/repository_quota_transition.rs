//! Inactive one-way quota transition. Semantic admission comes only from the
//! consuming whole-command owner; kernel quota facts cannot establish history.
use super::super::progress::command::NativeProcessAdmission;
use super::super::progress::command::PendingFirstExecute;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime::repository_physical_root) struct QuotaFacts {
    pub bytes: u64,
    pub inodes: u64,
    pub allocated: u64,
    pub used_inodes: u64,
}
fn facts(file: &File, mode: RepositoryWorkspaceMode) -> Result<QuotaFacts, PhysicalRootError> {
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    let mut vfs = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    if unsafe { libc::fstatfs(file.as_raw_fd(), fs.as_mut_ptr()) } != 0
        || unsafe { libc::fstatvfs(file.as_raw_fd(), vfs.as_mut_ptr()) } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let fs = unsafe { fs.assume_init() };
    let vfs = unsafe { vfs.assume_init() };
    let (bytes, inodes) = quota(mode);
    if unsafe { libc::sysconf(libc::_SC_PAGESIZE) } != 4096
        || fs.f_type != libc::TMPFS_MAGIC
        || fs.f_bsize != 4096
        || vfs.f_frsize != 4096
        || vfs.f_bsize != 4096
        || vfs.f_bfree > vfs.f_blocks
        || vfs.f_ffree > vfs.f_files
        || (vfs.f_blocks as u64).checked_mul(vfs.f_frsize as u64) != Some(bytes)
        || vfs.f_files as u64 != inodes
    {
        return Err(PhysicalRootError::Backing);
    }
    Ok(QuotaFacts {
        bytes,
        inodes,
        allocated: (vfs.f_blocks as u64 - vfs.f_bfree as u64)
            .checked_mul(4096)
            .ok_or(PhysicalRootError::Resource)?,
        used_inodes: vfs.f_files as u64 - vfs.f_ffree as u64,
    })
}
fn admit_physical(value: QuotaFacts) -> Result<(), PhysicalRootError> {
    if value.allocated > PROCESS_BYTES || value.used_inodes > 8192 {
        return Err(PhysicalRootError::Backing);
    }
    Ok(())
}

/// Kernel-only trusted setup stage. The parent has already authenticated a
/// NativeProcessAdmission for this exact incoming setup/nonce. This function
/// does not pretend to reconstruct command history from an FD or wire digest.
pub(super) fn shrink_attached(
    root: &AttachedSetupRoot,
    display: &Path,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(QuotaFacts, QuotaFacts), MountLeaseError> {
    check(scope, active)?;
    let _scratch = resources.reserve(IO_SCRATCH, 3, 0)?;
    let before_id = identity(&root.file)?;
    path_identity(display, before_id)?;
    let _owner = current_owner(resources)?;
    let before = facts(&root.file, RepositoryWorkspaceMode::Archive)?;
    admit_physical(before)?;
    let path = std::ffi::CString::new(display.as_os_str().as_bytes())
        .map_err(|_| MountLeaseError::AttachedRoot)?;
    if unsafe {
        libc::mount(
            std::ptr::null(),
            path.as_ptr(),
            c"tmpfs".as_ptr(),
            libc::MS_REMOUNT | libc::MS_NOSUID | libc::MS_NODEV,
            c"size=67108864,nr_inodes=8192".as_ptr().cast(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let after = facts(&root.file, RepositoryWorkspaceMode::Process)?;
    if identity(&root.file)? != before_id
        || before.allocated != after.allocated
        || before.used_inodes != after.used_inodes
    {
        return Err(MountLeaseError::AttachedRoot);
    }
    path_identity(display, before_id)?;
    check(scope, active)?;
    Ok((before, after))
}

/// A current parent-side physical verification, not an execution completion.
/// Constructor is private and all retained guards precede their own credit.
pub(in crate::runtime::repository_physical_root) struct VerifiedProcessQuota {
    request: Arc<RepositoryWorkspaceRequest>,
    root: RootIdentity,
    backing: Arc<RepositoryRootBacking>,
    scope: Arc<SourceServiceScope>,
    _credit: resources::ResourceCredit,
}
impl VerifiedProcessQuota {
    pub(in crate::runtime::repository_physical_root) fn matches_binding(
        &self,
        backing: &RepositoryRootBacking,
        identity: RootIdentity,
        reservation: &Arc<RepositoryWorkspaceReservation>,
    ) -> bool {
        std::ptr::eq(backing, self.backing.as_ref())
            && self.root == identity
            && self.request.reservation() == reservation
    }
    pub(in crate::runtime::repository_physical_root) fn commit(
        &self,
        builder: &mut RepositoryPhysicalRootBuilder,
        request: &RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        crate::runtime::repository_io::source_service::check_identity(
            Some(&self.scope),
            Some(scope),
        )?;
        check(scope, active)?;
        if request != self.request.as_ref()
            || builder.mode != RepositoryWorkspaceMode::Archive
            || builder.identity != self.root
            || !Arc::ptr_eq(&builder.backing, &self.backing)
        {
            return Err(PhysicalRootError::Reservation);
        }
        if backing_identity(&builder.file, RepositoryWorkspaceMode::Process)? != self.root {
            return Err(PhysicalRootError::RootIdentity);
        }
        builder.backing.shrink_to_process(self)?;
        // No fallible operation follows the bank change. The caller keeps this
        // owner quarantined until ACK and actual setup/child lifecycle settles.
        builder.mode = RepositoryWorkspaceMode::Process;
        Ok(())
    }
}

pub(in crate::runtime::repository_physical_root) struct ProcessQuotaTransition {
    attempt: Option<RepositoryMountAttempt>,
    inflight: Option<MountInFlight>,
    pending: Option<PendingMountLease>,
    admission: NativeProcessAdmission,
    before: QuotaFacts,
    confirmed: bool,
    poisoned: bool,
    _credit: resources::ResourceCredit,
}
pub(in crate::runtime::repository_physical_root) struct ProcessQuotaReady {
    pending_mount: PendingMountLease,
    owner: PendingFirstExecute,
}
impl ProcessQuotaReady {
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn finish_reaped(
        self,
        capture: super::super::process_owner::ReapedCapture,
    ) -> Result<super::super::progress::command::RepositoryWorkspaceCommand, PhysicalRootError>
    {
        self.owner.finish_reaped(self.pending_mount, capture)
    }

    pub(in crate::runtime::repository_physical_root) fn builder(
        &self,
    ) -> &RepositoryPhysicalRootBuilder {
        self.owner.builder()
    }
    pub(in crate::runtime::repository_physical_root) fn request(
        &self,
    ) -> &RepositoryWorkspaceRequest {
        self.owner.request()
    }
}
impl ProcessQuotaTransition {
    pub(in crate::runtime::repository_physical_root) fn new(
        mut admission: NativeProcessAdmission,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(admission.scope(), active)?;
        let root = admission.builder();
        let credit = root
            .resources
            .reserve(add(std::mem::size_of::<Self>(), IO_SCRATCH)?, 0, 0)?;
        if backing_identity(&root.file, RepositoryWorkspaceMode::Archive)? != root.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        let before = facts(&root.file, RepositoryWorkspaceMode::Archive)?;
        admit_physical(before)?;
        let seed = admission.take_initial_seed()?;
        seed.check_binding(admission.builder(), admission.scope(), active)
            .map_err(PhysicalRootError::mount)?;
        let mut clone = seed.clone;
        clone
            .credit
            .bytes
            .try_grow(
                admission
                    .request()
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )
            .map_err(|_| PhysicalRootError::Resource)?;
        let lease = RepositoryMountLease {
            reservation: admission.request().reservation().clone(),
            root: admission.builder().identity,
            index: admission.request().index(),
            predecessor: admission.request().predecessor().clone(),
            clone,
        };
        let request = admission.request().clone();
        let attempt = lease
            .begin(admission.builder(), request, admission.scope(), active)
            .map_err(PhysicalRootError::mount)?;
        Ok(Self {
            attempt: Some(attempt),
            inflight: None,
            pending: None,
            admission,
            before,
            confirmed: false,
            poisoned: false,
            _credit: credit,
        })
    }
    pub(super) fn scope(&self) -> &Arc<SourceServiceScope> {
        self.admission.scope()
    }
    pub(super) fn request_owner(&self) -> Arc<RepositoryWorkspaceRequest> {
        self.admission.request_owner()
    }
    pub(in crate::runtime::repository_physical_root) fn builder(
        &self,
    ) -> &RepositoryPhysicalRootBuilder {
        self.admission.builder()
    }
    pub(in crate::runtime::repository_physical_root) fn request(
        &self,
    ) -> &RepositoryWorkspaceRequest {
        self.admission.request()
    }
    /// Called by the owning driver only after its local Reap has settled. The
    /// whole command stays quarantined until Drop; no authority survives error.
    pub(super) fn poison(&mut self) {
        self.poisoned = true;
        drop(self.pending.take());
        drop(self.attempt.take());
        drop(self.inflight.take());
    }
    pub(super) fn namespace(&self) -> Result<BorrowedFd<'_>, PhysicalRootError> {
        if self.poisoned {
            return Err(PhysicalRootError::Mode);
        }
        self.attempt
            .as_ref()
            .map(|attempt| attempt.owning_user_namespace())
            .ok_or(PhysicalRootError::Mode)
    }
    pub(super) fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        if self.poisoned {
            return Err(PhysicalRootError::Mode);
        }
        let attempt = self.attempt.take().ok_or(PhysicalRootError::Mode)?;
        self.inflight = Some(
            attempt
                .transfer(
                    self.admission.builder(),
                    socket,
                    nonce,
                    self.admission.scope(),
                    active,
                )
                .map_err(PhysicalRootError::mount)?,
        );
        Ok(())
    }
    pub(super) fn verify_and_commit(
        &mut self,
        attached: &File,
        successor: AttachedMountClone,
        wire: (QuotaFacts, QuotaFacts),
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        if self.poisoned {
            return Err(PhysicalRootError::Mode);
        }
        check(self.admission.scope(), active)?;
        let root = self.admission.builder();
        if self.confirmed
            || wire.0 != self.before
            || wire.1 != facts(attached, RepositoryWorkspaceMode::Process)?
            || wire.1 != facts(&root.file, RepositoryWorkspaceMode::Process)?
            || wire.1.allocated != self.before.allocated
            || wire.1.used_inodes != self.before.used_inodes
            || identity(&root.file).map_err(PhysicalRootError::mount)? != root.identity
            || !same_backing(
                identity(attached).map_err(PhysicalRootError::mount)?,
                root.identity,
            )
        {
            return Err(PhysicalRootError::Backing);
        }
        let credit = root.resources.reserve(
            add(
                std::mem::size_of::<VerifiedProcessQuota>(),
                self.request()
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )?,
            0,
            0,
        )?;
        let proof = VerifiedProcessQuota {
            request: self.admission.request_owner(),
            root: root.identity,
            backing: root.backing.clone(),
            scope: self.admission.scope().clone(),
            _credit: credit,
        };
        let pending = self
            .inflight
            .take()
            .ok_or(PhysicalRootError::Mode)?
            .stage(successor, self.admission.scope(), active)
            .map_err(PhysicalRootError::mount)?;
        self.pending = Some(pending);
        self.admission.commit_process_in_place(&proof, active)?;
        self.confirmed = true;
        Ok(())
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn into_ready_after_reap(
        self,
        _proof: &super::super::process_owner::ReapedCapture,
    ) -> Result<ProcessQuotaReady, PhysicalRootError> {
        self.into_ready()
    }
    /// Physical transport probe only: retain the entire real admission and its
    /// guards without accepting a semantic result, record, epoch or replay.
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn into_native_probe_after_reap(
        mut self,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(NativeProcessAdmission, AttachedMountClone), PhysicalRootError> {
        capture
            .binding()
            .verify(self.request(), self.builder().identity)?;
        check(self.scope(), &|| true)?;
        if self.poisoned || !self.confirmed || self.attempt.is_some() || self.inflight.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let pending = self.pending.take().ok_or(PhysicalRootError::Mode)?;
        if pending.request != *self.request()
            || pending.root != self.builder().identity
            || pending.reservation != *self.request().reservation()
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok((self.admission, pending.successor))
    }
    // The general runner enters through into_ready_after_reap and retains its
    // exact capture proof. Fixed native tests call this private readiness-only
    // conversion after actual helper reap; it mints no completion receipt.
    fn into_ready(mut self) -> Result<ProcessQuotaReady, PhysicalRootError> {
        if self.poisoned || !self.confirmed {
            return Err(PhysicalRootError::Mode);
        }
        let pending_mount = self.pending.take().ok_or(PhysicalRootError::Mode)?;
        let owner = self.admission.into_pending_first_execute()?;
        Ok(ProcessQuotaReady {
            pending_mount,
            owner,
        })
    }
}

#[cfg(test)]
#[path = "repository_quota_transition/tests.rs"]
mod tests;

/// Test-only actual initialized Archive backing. This does not mint admission
/// or execute a payload; the command must own the returned never-used seed.
#[cfg(test)]
pub(in crate::runtime::repository_physical_root) fn initialized_archive_for_test(
    reservation: Arc<RepositoryWorkspaceReservation>,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
) -> Result<(RepositoryPhysicalRootBuilder, InitialRepositoryMount), PhysicalRootError> {
    let receipt = super::native_fixture::archive_seed(
        reservation.display_root().as_path(),
        resources,
        scope,
    )?;
    let builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs_with_backing(
        receipt.root,
        reservation,
        RepositoryWorkspaceMode::Archive,
        resources.clone(),
        receipt.backing.clone(),
        scope,
        &|| true,
    )?;
    let seed = InitialRepositoryMount::bind(&builder, receipt.successor, scope, &|| true)
        .map_err(PhysicalRootError::mount)?;
    Ok((builder, seed))
}
