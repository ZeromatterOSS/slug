//! Physical compaction of an Archive that is being irreversibly sealed.
//! General and Source owners use distinct closed reservation proofs.
//! Semantic mode/history are unchanged. The old capacity debit remains armed
//! through actual reap, complete seal and exact readonly successor acceptance.
use super::quota::QuotaFacts;
use super::quota::facts_for_quota;
use super::readonly::ReadOnlyMountClone;
use super::readonly::check_readonly;
use super::readonly::check_writable;
use super::*;

const TINY_BYTES: u64 = 8 * 1024 * 1024;
const SMALL_BYTES: u64 = 16 * 1024 * 1024;
const MEDIUM_BYTES: u64 = 256 * 1024 * 1024;
const LARGE_BYTES: u64 = 1024 * 1024 * 1024;
const COMPACT_INODES: u64 = 8192;
const FULL_INODES: u64 = 262_144;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrozenArchiveSize {
    Tiny,
    Small,
    Standard,
    Medium,
    Large,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime::repository_physical_root) struct FrozenArchiveQuota {
    size: FrozenArchiveSize,
    wide_inodes: bool,
}

impl FrozenArchiveQuota {
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) const TINY: Self = Self {
        size: FrozenArchiveSize::Tiny,
        wide_inodes: false,
    };
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) const TINY_WIDE_INODES: Self = Self {
        size: FrozenArchiveSize::Tiny,
        wide_inodes: true,
    };
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) const SMALL: Self = Self {
        size: FrozenArchiveSize::Small,
        wide_inodes: false,
    };
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) const MEDIUM: Self = Self {
        size: FrozenArchiveSize::Medium,
        wide_inodes: false,
    };

    pub(in crate::runtime::repository_physical_root) fn bytes(self) -> u64 {
        match self.size {
            FrozenArchiveSize::Tiny => TINY_BYTES,
            FrozenArchiveSize::Small => SMALL_BYTES,
            FrozenArchiveSize::Standard => PROCESS_BYTES,
            FrozenArchiveSize::Medium => MEDIUM_BYTES,
            FrozenArchiveSize::Large => LARGE_BYTES,
            FrozenArchiveSize::Full => ARCHIVE_BYTES,
        }
    }
    pub(in crate::runtime::repository_physical_root) fn inodes(self) -> u64 {
        if self.wide_inodes {
            FULL_INODES
        } else {
            COMPACT_INODES
        }
    }
    pub(in crate::runtime::repository_physical_root) fn quota(self) -> (u64, u64) {
        (self.bytes(), self.inodes())
    }
    pub(in crate::runtime::repository_physical_root) fn select(facts: QuotaFacts) -> Self {
        let size = match facts.allocated {
            0..=TINY_BYTES => FrozenArchiveSize::Tiny,
            ..=SMALL_BYTES => FrozenArchiveSize::Small,
            ..=PROCESS_BYTES => FrozenArchiveSize::Standard,
            ..=MEDIUM_BYTES => FrozenArchiveSize::Medium,
            ..=LARGE_BYTES => FrozenArchiveSize::Large,
            _ => FrozenArchiveSize::Full,
        };
        Self {
            size,
            wide_inodes: facts.used_inodes > COMPACT_INODES,
        }
    }
}

/// Kernel setup only, with the same owning-namespace/attached-root checks as
/// Process shrink. No Execute admission, state cap or result is manufactured.
pub(super) fn compact_attached(
    root: &AttachedSetupRoot,
    display: &Path,
    freeze_superblock: bool,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(QuotaFacts, QuotaFacts), MountLeaseError> {
    check(scope, active)?;
    let _scratch = resources.reserve(IO_SCRATCH, 3, 0)?;
    let before_id = identity(&root.file)?;
    path_identity(display, before_id)?;
    let _owner = current_owner(resources)?;
    if freeze_superblock {
        check_writable(&root.file)?;
    }
    let before = facts_for_quota(&root.file, quota(RepositoryWorkspaceMode::Archive))?;
    let class = FrozenArchiveQuota::select(before);
    let (bytes, inodes) = class.quota();
    if let Some(flags) = remount_flags(
        (bytes, inodes) != (before.bytes, before.inodes),
        freeze_superblock,
    ) {
        let path = std::ffi::CString::new(display.as_os_str().as_bytes())
            .map_err(|_| MountLeaseError::AttachedRoot)?;
        let options = std::ffi::CString::new(format!("size={bytes},nr_inodes={inodes}"))
            .map_err(|_| MountLeaseError::AttachedRoot)?;
        if unsafe {
            libc::mount(
                std::ptr::null(),
                path.as_ptr(),
                c"tmpfs".as_ptr(),
                flags,
                options.as_ptr().cast(),
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
    if freeze_superblock {
        check_readonly(&root.file)?;
    }
    check(scope, active)?;
    Ok((before, after))
}

fn remount_flags(quota_changed: bool, freeze_superblock: bool) -> Option<libc::c_ulong> {
    let flags = libc::MS_REMOUNT | libc::MS_NOSUID | libc::MS_NODEV;
    if freeze_superblock {
        // A non-bind remount changes the superblock as well as this mount.
        Some(flags | libc::MS_RDONLY)
    } else {
        quota_changed.then_some(flags)
    }
}

/// The parent's original mount was writable before launching the finalizer.
/// The helper can remount only its distinct attached alias of this backing.
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub(super) struct WritableArchiveParent {
    original: RootIdentity,
    helper: RootIdentity,
}

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
impl WritableArchiveParent {
    pub(super) fn before_launch(
        builder: &RepositoryPhysicalRootBuilder,
        incoming: &AttachedMountClone,
    ) -> Result<Self, PhysicalRootError> {
        let proof = Self {
            original: builder.identity,
            helper: incoming.detached,
        };
        if builder.mode != RepositoryWorkspaceMode::Archive
            || !same_backing(proof.original, proof.helper)
            || proof.original.mount == proof.helper.mount
        {
            return Err(PhysicalRootError::Backing);
        }
        proof.check_before_transfer(builder)?;
        Ok(proof)
    }

    pub(super) fn check_before_transfer(
        &self,
        builder: &RepositoryPhysicalRootBuilder,
    ) -> Result<(), PhysicalRootError> {
        if builder.identity != self.original
            || identity(&builder.file).map_err(PhysicalRootError::mount)? != self.original
        {
            return Err(PhysicalRootError::Backing);
        }
        check_writable(&builder.file).map_err(PhysicalRootError::mount)
    }
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
        parent: &WritableArchiveParent,
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
            || parent.original != builder.identity
            || parent.helper != successor.attached
            || parent.original.mount == successor.detached.mount
            || backing_identity_for_quota(&builder.file, class.quota())? != builder.identity
            || backing_identity_for_quota(attached, class.quota())? != successor.attached
            || !same_backing(builder.identity, successor.attached)
        {
            return Err(PhysicalRootError::Backing);
        }
        // This FD's distinct mount was writable before the helper remounted
        // its own alias. Its transition proves the backing is now read-only.
        check_readonly(&builder.file).map_err(PhysicalRootError::mount)?;
        check_readonly(attached).map_err(PhysicalRootError::mount)?;
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
        check_readonly(&builder.file).map_err(PhysicalRootError::mount)?;
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
        check_readonly(&root.file).map_err(PhysicalRootError::mount)?;
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
    fn general_archive_freezes_even_when_quota_is_unchanged() {
        let flags = libc::MS_REMOUNT | libc::MS_NOSUID | libc::MS_NODEV;
        assert_eq!(remount_flags(false, true), Some(flags | libc::MS_RDONLY));
        assert_eq!(remount_flags(true, true), Some(flags | libc::MS_RDONLY));
        assert_eq!(remount_flags(false, false), None);
        assert_eq!(remount_flags(true, false), Some(flags));
    }

    #[test]
    fn immutable_compaction_uses_actual_allocated_bytes_and_charged_inode_units() {
        let facts = |allocated, used_inodes| QuotaFacts {
            bytes: ARCHIVE_BYTES,
            inodes: 262_144,
            allocated,
            used_inodes,
        };
        for (allocated_mib, overshoot, capacity_mib) in [
            (0, 0, 8),
            (8, 0, 8),
            (8, 4096, 16),
            (16, 0, 16),
            (16, 4096, 64),
            (64, 0, 64),
            (64, 4096, 256),
            (256, 0, 256),
            (256, 4096, 1024),
            (1024, 0, 1024),
            (1024, 4096, 2048),
            (2048, 0, 2048),
        ] {
            let allocated = allocated_mib * 1024 * 1024 + overshoot;
            for (used, inodes) in [(1, 8192), (8192, 8192), (8193, 262_144), (262_144, 262_144)] {
                assert_eq!(
                    FrozenArchiveQuota::select(facts(allocated, used)).quota(),
                    (capacity_mib * 1024 * 1024, inodes),
                );
            }
        }
    }
}
