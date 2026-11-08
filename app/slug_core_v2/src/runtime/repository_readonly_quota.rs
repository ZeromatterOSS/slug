//! Physical compaction of an Archive that is being irreversibly sealed.
//! General and Source owners use distinct closed reservation proofs.
//! Semantic mode/history are unchanged. The old capacity debit remains armed
//! through actual reap, complete seal and exact readonly successor acceptance.
use super::quota::QuotaFacts;
use super::quota::facts_for_quota;
use super::readonly::ReadOnlyMountClone;
use super::readonly::check_readonly;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime::repository_physical_root) enum FrozenArchiveQuota {
    FullArchive,
    CompactArchive,
}
impl FrozenArchiveQuota {
    pub(in crate::runtime::repository_physical_root) fn bytes(self) -> u64 {
        self.quota().0
    }
    pub(in crate::runtime::repository_physical_root) fn inodes(self) -> u64 {
        self.quota().1
    }
    pub(in crate::runtime::repository_physical_root) fn quota(self) -> (u64, u64) {
        match self {
            Self::FullArchive => (ARCHIVE_BYTES, 262_144),
            Self::CompactArchive => (PROCESS_BYTES, 8192),
        }
    }
    pub(in crate::runtime::repository_physical_root) fn select(facts: QuotaFacts) -> Self {
        if facts.allocated <= PROCESS_BYTES && facts.used_inodes <= 8192 {
            Self::CompactArchive
        } else {
            Self::FullArchive
        }
    }
}

/// Kernel setup only, with the same owning-namespace/attached-root checks as
/// Process shrink. No Execute admission, state cap or result is manufactured.
pub(super) fn compact_attached(
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
    let before = facts_for_quota(&root.file, quota(RepositoryWorkspaceMode::Archive))?;
    let class = FrozenArchiveQuota::select(before);
    if class == FrozenArchiveQuota::CompactArchive {
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
    }
    let after = facts_for_quota(&root.file, class.quota())?;
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

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(in crate::runtime::repository_physical_root) struct PendingReadOnlyCompaction {
    reservation: Arc<RepositoryWorkspaceReservation>,
    backing: Arc<RepositoryRootBacking>,
    scope: Arc<SourceServiceScope>,
    root: RootIdentity,
    attached: RootIdentity,
    detached: RootIdentity,
    namespace: NamespaceIdentity,
    nonce: [u8; 16],
    class: FrozenArchiveQuota,
    after: QuotaFacts,
    _credit: ResourceCredit,
}
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
impl PendingReadOnlyCompaction {
    pub(super) fn receive(
        builder: &RepositoryPhysicalRootBuilder,
        successor: &AttachedMountClone,
        attached: &File,
        words: [u64; 8],
        nonce: [u8; 16],
        scope: &Arc<SourceServiceScope>,
    ) -> Result<Self, PhysicalRootError> {
        builder.check_origin(scope, &|| true)?;
        let _scratch = builder.resources.reserve(IO_SCRATCH, 0, 0)?;
        if builder.mode != RepositoryWorkspaceMode::Archive
            || builder.frozen_archive_quota.is_some()
        {
            return Err(PhysicalRootError::Mode);
        }
        let before = QuotaFacts {
            bytes: words[0],
            inodes: words[1],
            allocated: words[2],
            used_inodes: words[3],
        };
        let after = QuotaFacts {
            bytes: words[4],
            inodes: words[5],
            allocated: words[6],
            used_inodes: words[7],
        };
        let class = FrozenArchiveQuota::select(before);
        if (before.bytes, before.inodes) != quota(RepositoryWorkspaceMode::Archive)
            || (after.bytes, after.inodes) != class.quota()
            || before.allocated > before.bytes
            || before.used_inodes > before.inodes
            || before.allocated != after.allocated
            || before.used_inodes != after.used_inodes
            || facts_for_quota(&builder.file, class.quota())? != after
            || facts_for_quota(attached, class.quota())? != after
            || backing_identity_for_quota(&builder.file, class.quota())? != builder.identity
            || backing_identity_for_quota(attached, class.quota())? != successor.attached
            || !same_backing(builder.identity, successor.attached)
        {
            return Err(PhysicalRootError::Backing);
        }
        let credit = builder.resources.reserve(
            add(
                std::mem::size_of::<Self>(),
                builder
                    .reservation
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )?,
            0,
            0,
        )?;
        Ok(Self {
            reservation: builder.reservation.clone(),
            backing: builder.backing.clone(),
            scope: scope.clone(),
            root: builder.identity,
            attached: successor.attached,
            detached: successor.detached,
            namespace: successor.namespace,
            nonce,
            class,
            after,
            _credit: credit,
        })
    }
    pub(in crate::runtime::repository_physical_root) fn stage_after_reap(
        &self,
        builder: &mut RepositoryPhysicalRootBuilder,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(), PhysicalRootError> {
        builder.check_origin(&self.scope, &|| true)?;
        capture.binding().verify_finalize_attempt(
            self.root,
            crate::runtime::repository_process_wire::RunMode::FinalizeArchive,
            &self.nonce,
        )?;
        if builder.mode != RepositoryWorkspaceMode::Archive
            || builder.frozen_archive_quota.is_some()
            || builder.identity != self.root
            || !Arc::ptr_eq(&builder.backing, &self.backing)
            || !Arc::ptr_eq(&builder.reservation, &self.reservation)
            || facts_for_quota(&builder.file, self.class.quota())? != self.after
            || backing_identity_for_quota(&builder.file, self.class.quota())? != self.root
        {
            return Err(PhysicalRootError::Backing);
        }
        builder.frozen_archive_quota = Some(self.class);
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn commit(
        &self,
        root: &Arc<RepositoryPhysicalRoot>,
        mount: &ReadOnlyMountClone,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<(), PhysicalRootError> {
        root.check_origin(&self.scope, &|| true)?;
        capture.binding().verify_finalize_attempt(
            self.root,
            crate::runtime::repository_process_wire::RunMode::FinalizeArchive,
            &self.nonce,
        )?;
        if root.mode != RepositoryWorkspaceMode::Archive
            || root.frozen_archive_quota != Some(self.class)
            || root.identity != self.root
            || !Arc::ptr_eq(&root.backing, &self.backing)
            || !Arc::ptr_eq(&root.reservation, &self.reservation)
            || mount.clone.attached != self.attached
            || mount.clone.detached != self.detached
            || mount.clone.namespace != self.namespace
            || identity(&mount.clone.token).map_err(PhysicalRootError::mount)? != self.detached
            || !mount
                .clone
                .backing
                .as_ref()
                .is_some_and(|backing| Arc::ptr_eq(backing, &self.backing))
            || facts_for_quota(&root.file, self.class.quota())? != self.after
            || facts_for_quota(&mount.clone.token, self.class.quota())? != self.after
            || backing_identity_for_quota(&root.file, self.class.quota())? != self.root
        {
            return Err(PhysicalRootError::Backing);
        }
        check_readonly(&mount.clone.token).map_err(PhysicalRootError::mount)?;
        if namespace_identity(&mount.clone.owner).map_err(PhysicalRootError::mount)?
            != self.namespace
        {
            return Err(PhysicalRootError::Backing);
        }
        let verified = VerifiedReadOnlyCompaction {
            pending: self,
            _root: root,
            _reap: capture,
        };
        self.backing.freeze_archive(&verified)
    }
}
/// Constructible only after actual bound reap + full sealed owner + exact RO
/// successor checks. Resource bookkeeping itself performs no IO under a lock.
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(in crate::runtime::repository_physical_root) struct VerifiedReadOnlyCompaction<'a> {
    pending: &'a PendingReadOnlyCompaction,
    _root: &'a Arc<RepositoryPhysicalRoot>,
    _reap: &'a super::super::process_owner::ReapedCapture,
}
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
impl VerifiedReadOnlyCompaction<'_> {
    pub(in crate::runtime::repository_physical_root) fn class(&self) -> FrozenArchiveQuota {
        self.pending.class
    }
    pub(in crate::runtime::repository_physical_root) fn matches_binding(
        &self,
        backing: &RepositoryRootBacking,
        identity: RootIdentity,
        reservation: &Arc<RepositoryWorkspaceReservation>,
    ) -> bool {
        std::ptr::eq(backing, self.pending.backing.as_ref())
            && identity == self.pending.root
            && Arc::ptr_eq(reservation, &self.pending.reservation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_compaction_uses_actual_allocated_bytes_and_charged_inode_units() {
        let facts = |allocated, used_inodes| QuotaFacts {
            bytes: ARCHIVE_BYTES,
            inodes: 262_144,
            allocated,
            used_inodes,
        };
        for (allocated, used) in [(0, 1), (PROCESS_BYTES, 8192)] {
            assert_eq!(
                FrozenArchiveQuota::select(facts(allocated, used)),
                FrozenArchiveQuota::CompactArchive
            );
        }
        for (allocated, used) in [(PROCESS_BYTES + 4096, 1), (0, 8193)] {
            assert_eq!(
                FrozenArchiveQuota::select(facts(allocated, used)),
                FrozenArchiveQuota::FullArchive
            );
        }
        assert_eq!(
            FrozenArchiveQuota::CompactArchive.quota(),
            (64 * 1024 * 1024, 8192)
        );
        assert_eq!(
            FrozenArchiveQuota::FullArchive.quota(),
            (2 * 1024 * 1024 * 1024, 262_144)
        );
    }
}
