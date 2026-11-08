//! Source-only closed proof for same-backing read-only quota contraction.
//! Actual reap, complete digest-tree sealing and exact RO lineage precede the
//! atomic shared debit change. No General reservation or process is invented.
use super::super::super::source::RepositorySourceRoot;
use super::super::quota::QuotaFacts;
use super::super::quota::facts_for_quota;
use super::super::readonly::ReadOnlyMountClone;
use super::super::readonly::check_readonly;
use super::super::readonly_quota::FrozenArchiveQuota;
use super::*;

pub(in crate::runtime::repository_physical_root) struct PendingSourceReadOnlyCompaction {
    reservation: Arc<Reservation>,
    backing: Arc<RepositoryRootBacking>,
    claim: Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>,
    domain: Arc<domain::NativeMountDomain>,
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
impl PendingSourceReadOnlyCompaction {
    pub(super) fn receive(
        builder: &SourceRootBuilder,
        successor: &AttachedMountClone,
        attached: &File,
        words: [u64; 8],
        nonce: [u8; 16],
        scope: &Arc<SourceServiceScope>,
    ) -> Result<Self, PhysicalRootError> {
        super::super::super::check_origin(&builder.scope, scope, &|| true)?;
        let _scratch = builder.resources.reserve(IO_SCRATCH, 0, 0)?;
        builder
            .backing
            .matches(&builder.resources, RepositoryWorkspaceMode::Archive)?;
        let claim = builder
            .backing
            .source_snapshot_claim()
            .ok_or(PhysicalRootError::Reservation)?;
        if !claim.matches_reservation(&builder.reservation) || !claim.belongs_to(&builder.resources)
        {
            return Err(PhysicalRootError::Reservation);
        }
        builder
            .domain
            .check_source(&builder.reservation, &builder.resources, scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        builder
            .domain
            .check_clone(successor, scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        if builder.frozen_archive_quota.is_some() {
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
            claim,
            domain: builder.domain.clone(),
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
        builder: &mut SourceRootBuilder,
        capture: &crate::runtime::repository_physical_root::process_owner::ReapedCapture,
    ) -> Result<(), PhysicalRootError> {
        super::super::super::check_origin(&builder.scope, &self.scope, &|| true)?;
        capture.binding().verify_finalize_attempt(
            self.root,
            crate::runtime::repository_process_wire::RunMode::FinalizeSourceArchive,
            &self.nonce,
        )?;
        if builder.frozen_archive_quota.is_some()
            || builder.identity != self.root
            || !Arc::ptr_eq(&builder.backing, &self.backing)
            || !Arc::ptr_eq(&builder.reservation, &self.reservation)
            || !Arc::ptr_eq(&builder.domain, &self.domain)
            || !builder
                .backing
                .source_snapshot_claim()
                .is_some_and(|claim| Arc::ptr_eq(&claim, &self.claim))
            || facts_for_quota(&builder.file, self.class.quota())? != self.after
            || backing_identity_for_quota(&builder.file, self.class.quota())? != self.root
        {
            return Err(PhysicalRootError::Backing);
        }
        self.domain
            .check_source(
                &builder.reservation,
                &builder.resources,
                &self.scope,
                &|| true,
            )
            .map_err(PhysicalRootError::mount)?;
        builder.frozen_archive_quota = Some(self.class);
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn commit(
        &self,
        root: &Arc<RepositorySourceRoot>,
        mount: &ReadOnlyMountClone,
        capture: &crate::runtime::repository_physical_root::process_owner::ReapedCapture,
    ) -> Result<(), PhysicalRootError> {
        root.check_origin(&self.scope, &|| true)?;
        capture.binding().verify_finalize_attempt(
            self.root,
            crate::runtime::repository_process_wire::RunMode::FinalizeSourceArchive,
            &self.nonce,
        )?;
        if root.frozen_archive_quota != Some(self.class)
            || root.identity != self.root
            || !Arc::ptr_eq(&root.backing, &self.backing)
            || !Arc::ptr_eq(root.reservation(), &self.reservation)
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
        if !root
            .source_claim()
            .is_some_and(|claim| Arc::ptr_eq(&claim, &self.claim))
            || !self.claim.matches_reservation(root.reservation())
            || !self.claim.belongs_to(&root.resources)
        {
            return Err(PhysicalRootError::Reservation);
        }
        self.domain
            .check_source(root.reservation(), &root.resources, &self.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        self.domain
            .check_clone(&mount.clone, &self.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        check_readonly(&mount.clone.token).map_err(PhysicalRootError::mount)?;
        if namespace_identity(&mount.clone.owner).map_err(PhysicalRootError::mount)?
            != self.namespace
        {
            return Err(PhysicalRootError::Backing);
        }
        let verified = VerifiedSourceReadOnlyCompaction {
            pending: self,
            _root: root,
            _reap: capture,
        };
        self.backing.freeze_source_archive(&verified)
    }
}
/// Constructible only after actual bound reap + full sealed owner + exact RO
/// successor checks. Resource bookkeeping itself performs no IO under a lock.
pub(in crate::runtime::repository_physical_root) struct VerifiedSourceReadOnlyCompaction<'a> {
    pending: &'a PendingSourceReadOnlyCompaction,
    _root: &'a Arc<RepositorySourceRoot>,
    _reap: &'a crate::runtime::repository_physical_root::process_owner::ReapedCapture,
}
impl VerifiedSourceReadOnlyCompaction<'_> {
    pub(in crate::runtime::repository_physical_root) fn class(&self) -> FrozenArchiveQuota {
        self.pending.class
    }
    pub(in crate::runtime::repository_physical_root) fn matches_binding(
        &self,
        backing: &RepositoryRootBacking,
        identity: RootIdentity,
        reservation: &Arc<Reservation>,
        claim: &Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>,
    ) -> bool {
        std::ptr::eq(backing, self.pending.backing.as_ref())
            && identity == self.pending.root
            && Arc::ptr_eq(reservation, &self.pending.reservation)
            && Arc::ptr_eq(claim, &self.pending.claim)
    }
}
