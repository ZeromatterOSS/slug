//! Closed semantic/native join for the general runner. Only an owning worker's
//! actual reaped capture can restore a command or initialize a new root.
use super::super::process_owner::ProcessLaunch;
use super::super::process_owner::ReapedCapture;
use super::super::progress::command::NativeProcessAdmission;
use super::super::progress::command::RepositoryWorkspaceCommand;
use super::super::progress::command::finalize::NativeViewFinalization;
use super::batch::NativeViewBatch;
use super::domain::NativeMountDomain;
use super::quota::ProcessQuotaTransition;
use super::quota::QuotaFacts;
use super::readonly::FinalizedRepositoryView;
use super::readonly::ReadOnlyMountClone;
use super::readonly_quota::PendingReadOnlyCompaction;
use super::*;
use crate::runtime::repository_process_wire::RunMode;

pub(in crate::runtime::repository_physical_root) enum ExecuteDecision {
    Replay(RepositoryWorkspaceCommand),
    Launch(ExecuteLaunch),
}
pub(in crate::runtime::repository_physical_root) enum SelectedExecuteDecision {
    Replay(RepositoryWorkspaceCommand),
    Launch(SelectedExecuteLaunch),
}
pub(in crate::runtime::repository_physical_root) struct SelectedExecuteLaunch {
    root: ExecuteLaunch,
    views: NativeViewBatch,
}
impl SelectedExecuteLaunch {
    pub(in crate::runtime::repository_physical_root) fn new(
        root: ExecuteLaunch,
        views: NativeViewBatch,
    ) -> Result<Self, PhysicalRootError> {
        if !Arc::ptr_eq(
            &root
                .covered_request_owner()
                .ok_or(PhysicalRootError::Mode)?,
            views.request_cover(),
        ) || root.root_identity() != Some(views.executor())
            || !Arc::ptr_eq(root.scope(), views.scope())
            || !Arc::ptr_eq(root.resources(), views.resources())
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(Self { root, views })
    }
}
impl ProcessLaunch for SelectedExecuteLaunch {
    type Output = RepositoryWorkspaceCommand;
    #[cfg(test)]
    fn process_probe(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceProcessProbe> {
        self.root.process_probe()
    }
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        self.root.retirement_observer()
    }
    fn mode(&self) -> RunMode {
        self.root.mode()
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.root.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.root.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        self.root.request()
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        self.root.covered_request_owner()
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        self.root.root_identity()
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        self.root.scope()
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        self.root.resources()
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        self.root.namespace()
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        self.root.transfer(socket, nonce)
    }
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        self.root
            .verify_successor(attached, token, userns, words, quota)
    }
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        self.root.bind_attempt(nonce)?;
        self.views.bind_attempt(nonce)
    }
    fn views(&self) -> &[crate::runtime::repository_process_wire::ViewEntry] {
        self.views.entries()
    }
    fn transfer_view(
        &mut self,
        slot: usize,
        socket: BorrowedFd<'_>,
        _: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        self.views.transfer(slot, socket)
    }
    fn verify_view_successor(
        &mut self,
        slot: usize,
        attached: File,
        token: File,
        words: [u64; 8],
    ) -> Result<(), PhysicalRootError> {
        self.views.stage(slot, attached, token, words)
    }
    fn finish_reaped(mut self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        let restored = self.views.complete(&capture)?;
        let scope = self.root.scope().clone();
        let command = self.root.finish_reaped(capture)?;
        command.restore_process_views(restored, &scope, &|| true)
    }
}
pub(in crate::runtime::repository_physical_root) enum ExecuteLaunch {
    First(ProcessQuotaTransition),
    Continue {
        attempt: Option<RepositoryMountAttempt>,
        inflight: Option<MountInFlight>,
        pending: Option<PendingMountLease>,
        admission: NativeProcessAdmission,
    },
}
impl ExecuteLaunch {
    pub(in crate::runtime::repository_physical_root) fn new(
        mut admission: NativeProcessAdmission,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        if admission.builder().mode == RepositoryWorkspaceMode::Archive {
            Ok(Self::First(ProcessQuotaTransition::new(admission, active)?))
        } else {
            let attempt = admission.begin_rolling(active)?;
            Ok(Self::Continue {
                attempt: Some(attempt),
                inflight: None,
                pending: None,
                admission,
            })
        }
    }
    fn builder(&self) -> &RepositoryPhysicalRootBuilder {
        match self {
            Self::First(q) => q.builder(),
            Self::Continue { admission, .. } => admission.builder(),
        }
    }
}
fn quota_words(words: [u64; 8]) -> (QuotaFacts, QuotaFacts) {
    let one = |s: &[u64]| QuotaFacts {
        bytes: s[0],
        inodes: s[1],
        allocated: s[2],
        used_inodes: s[3],
    };
    (one(&words[..4]), one(&words[4..]))
}
/// All incoming descriptors are already owned and covered by the driver's
/// transient allowance. These guards cover the successor's retained lifetime.
pub(super) fn received_successor(
    root: &File,
    token: File,
    owner: File,
    words: [u64; 8],
    resources: &Arc<RepositoryRootResources>,
    backing: Arc<RepositoryRootBacking>,
) -> Result<AttachedMountClone, PhysicalRootError> {
    let credit = resources.reserve(std::mem::size_of::<AttachedMountClone>(), 2, 0)?;
    let attached = identity(root).map_err(PhysicalRootError::mount)?;
    let detached = identity(&token).map_err(PhysicalRootError::mount)?;
    let namespace = namespace_identity(&owner).map_err(PhysicalRootError::mount)?;
    if words
        != [
            attached.device,
            attached.inode,
            attached.mount,
            detached.device,
            detached.inode,
            detached.mount,
            namespace.device,
            namespace.inode,
        ]
        || !same_backing(attached, detached)
        || attached.mount == detached.mount
    {
        return Err(PhysicalRootError::Backing);
    }
    Ok(AttachedMountClone {
        token,
        owner,
        attached,
        detached,
        namespace,
        resources: resources.clone(),
        backing: Some(backing),
        credit,
    })
}
impl ProcessLaunch for ExecuteLaunch {
    type Output = RepositoryWorkspaceCommand;
    #[cfg(test)]
    fn process_probe(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceProcessProbe> {
        match self {
            Self::First(quota) => quota.process_probe(),
            Self::Continue { admission, .. } => admission.process_probe(),
        }
    }
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        Some(match self {
            Self::First(quota) => quota.retirement_observer(),
            Self::Continue { admission, .. } => admission.retirement_observer(),
        })
    }
    fn mode(&self) -> RunMode {
        match self {
            Self::First(_) => RunMode::FirstExecute,
            Self::Continue { .. } => RunMode::ContinueExecute,
        }
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder().reservation.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder().reservation.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        Some(match self {
            Self::First(q) => q.request(),
            Self::Continue { admission, .. } => admission.request(),
        })
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        Some(match self {
            Self::First(q) => q.covered_request_owner(),
            Self::Continue { admission, .. } => admission.request_cover(),
        })
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        Some(self.builder().identity)
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        match self {
            Self::First(q) => q.scope(),
            Self::Continue { admission, .. } => admission.scope(),
        }
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.builder().resources
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(Some(match self {
            Self::First(q) => q.namespace()?,
            Self::Continue { attempt, .. } => attempt
                .as_ref()
                .ok_or(PhysicalRootError::Mode)?
                .owning_user_namespace(),
        }))
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        match self {
            Self::First(q) => q.transfer(socket, nonce, &|| true),
            Self::Continue {
                attempt,
                inflight,
                admission,
                ..
            } => {
                if inflight.is_some() {
                    return Err(PhysicalRootError::Mode);
                }
                *inflight = Some(
                    attempt
                        .take()
                        .ok_or(PhysicalRootError::Mode)?
                        .transfer(
                            admission.builder(),
                            socket,
                            nonce,
                            admission.scope(),
                            &|| true,
                        )
                        .map_err(PhysicalRootError::mount)?,
                );
                Ok(())
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
        let builder = self.builder();
        let successor = received_successor(
            &attached,
            token,
            userns,
            words,
            &builder.resources,
            builder.backing.clone(),
        )?;
        match self {
            Self::First(q) => q.verify_and_commit(
                &attached,
                successor,
                quota_words(quota.ok_or(PhysicalRootError::Mode)?),
                &|| true,
            ),
            Self::Continue {
                inflight,
                pending,
                admission,
                ..
            } => {
                if quota.is_some()
                    || pending.is_some()
                    || !same_backing(successor.attached, admission.builder().identity)
                    || backing_identity(&attached, RepositoryWorkspaceMode::Process)?.device
                        != admission.builder().identity.device
                    || backing_identity(
                        &admission.builder().file,
                        RepositoryWorkspaceMode::Process,
                    )? != admission.builder().identity
                {
                    return Err(PhysicalRootError::Backing);
                }
                *pending = Some(
                    inflight
                        .take()
                        .ok_or(PhysicalRootError::Mode)?
                        .stage(successor, admission.scope(), &|| true)
                        .map_err(PhysicalRootError::mount)?,
                );
                Ok(())
            }
        }
    }
    fn finish_reaped(self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        match self {
            Self::First(q) => {
                capture
                    .binding()
                    .verify(q.request(), q.builder().identity)?;
                q.into_ready_after_reap(&capture)?.finish_reaped(capture)
            }
            Self::Continue {
                attempt,
                inflight,
                pending,
                admission,
            } => {
                if attempt.is_some() || inflight.is_some() {
                    return Err(PhysicalRootError::Mode);
                }
                admission.finish_reaped(pending.ok_or(PhysicalRootError::Mode)?, capture)
            }
        }
    }
}

/// No fabricated Execute request participates in trusted empty-root bootstrap.
/// The Archive capacity allowance is paid before the worker can create tmpfs.
pub(in crate::runtime::repository_physical_root) struct BootstrapLaunch {
    successor: Option<AttachedMountClone>,
    root: Option<File>,
    reservation: Arc<RepositoryWorkspaceReservation>,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    backing: Arc<RepositoryRootBacking>,
    domain: Option<Arc<NativeMountDomain>>,
    retirement: Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver>,
    #[cfg(test)]
    process_probe: Option<super::super::progress::command::RepositoryWorkspaceProcessProbe>,
    _credit: resources::ResourceCredit,
}
impl BootstrapLaunch {
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) fn with_process_probe(
        mut self,
        probe: Option<super::super::progress::command::RepositoryWorkspaceProcessProbe>,
    ) -> Self {
        self.process_probe = probe;
        self
    }
    pub(in crate::runtime::repository_physical_root) fn with_retirement_observer(
        mut self,
        observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
    ) -> Self {
        self.retirement = Some(observer);
        self
    }
    pub(in crate::runtime::repository_physical_root) fn new_claimed(
        reservation: Arc<RepositoryWorkspaceReservation>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        claim: Arc<crate::runtime::repository_io::reservations::SourceRootClaim>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        Self::new_claimed_in_domain(reservation, resources, scope, claim, None, active)
    }
    pub(in crate::runtime::repository_physical_root) fn new_claimed_in_domain(
        reservation: Arc<RepositoryWorkspaceReservation>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        claim: Arc<crate::runtime::repository_io::reservations::SourceRootClaim>,
        domain: Option<Arc<NativeMountDomain>>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let owner = Self::new_in_domain(reservation, resources, scope, domain, active)?;
        // Arm the claim in shared backing BEFORE the worker can create a root.
        // Caller cancellation cannot retire spelling while bootstrap still owns
        // descriptors, even if no builder ever reaches the awaiting caller.
        owner.backing.bind_claim(&owner.reservation, claim)?;
        Ok(owner)
    }
    pub(in crate::runtime::repository_physical_root) fn new(
        reservation: Arc<RepositoryWorkspaceReservation>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        Self::new_in_domain(reservation, resources, scope, None, active)
    }
    pub(in crate::runtime::repository_physical_root) fn new_in_domain(
        reservation: Arc<RepositoryWorkspaceReservation>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        domain: Option<Arc<NativeMountDomain>>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(&scope, active)?;
        if reservation.workspace() != scope.workspace() {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some(domain) = &domain {
            domain
                .check_root(&reservation, &resources, &scope, active)
                .map_err(PhysicalRootError::mount)?;
        }
        let credit = resources.reserve(
            add(
                std::mem::size_of::<Self>(),
                reservation
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )?,
            1,
            0,
        )?;
        let backing = RepositoryRootBacking::new(&resources, RepositoryWorkspaceMode::Archive)?;
        Ok(Self {
            successor: None,
            root: None,
            reservation,
            resources,
            scope,
            backing,
            domain,
            retirement: None,
            #[cfg(test)]
            process_probe: None,
            _credit: credit,
        })
    }
}
impl ProcessLaunch for BootstrapLaunch {
    type Output = (RepositoryPhysicalRootBuilder, InitialRepositoryMount);
    #[cfg(test)]
    fn process_probe(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceProcessProbe> {
        self.process_probe.clone()
    }
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        self.retirement.clone()
    }
    fn mode(&self) -> RunMode {
        RunMode::InitializeArchive
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.reservation.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.reservation.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        None
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        None
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        self.successor.as_ref().map(|s| s.attached)
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.scope
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.resources
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(self.domain.as_ref().map(|domain| domain.borrow_namespace()))
    }
    fn transfer(&mut self, _: BorrowedFd<'_>, _: &[u8; 16]) -> Result<(), PhysicalRootError> {
        Err(PhysicalRootError::Mode)
    }
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        if self.root.is_some() || self.successor.is_some() || quota.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let successor = received_successor(
            &attached,
            token,
            userns,
            words,
            &self.resources,
            self.backing.clone(),
        )?;
        if let Some(domain) = &self.domain {
            domain
                .check_clone(&successor, &self.scope, &|| true)
                .map_err(PhysicalRootError::mount)?;
        }
        if backing_identity(&attached, RepositoryWorkspaceMode::Archive)? != successor.attached {
            return Err(PhysicalRootError::Backing);
        }
        self.root = Some(attached);
        self.successor = Some(successor);
        Ok(())
    }
    fn finish_reaped(mut self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        capture.binding().verify_bootstrap(
            self.root_identity()
                .ok_or(PhysicalRootError::RootIdentity)?,
        )?;
        if capture.return_code() != 0
            || !capture.stdout().is_empty()
            || !capture.stderr().is_empty()
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs_with_backing(
            self.root.take().ok_or(PhysicalRootError::Mode)?,
            self.reservation.clone(),
            RepositoryWorkspaceMode::Archive,
            self.resources.clone(),
            self.backing.clone(),
            &self.scope,
            &|| true,
        )?;
        builder.retirement = self
            .retirement
            .as_ref()
            .map(|observer| observer.worker_guard());
        let mut seed = InitialRepositoryMount::bind(
            &builder,
            self.successor.take().ok_or(PhysicalRootError::Mode)?,
            &self.scope,
            &|| true,
        )
        .map_err(PhysicalRootError::mount)?;
        if let Some(observer) = &self.retirement {
            seed = seed.with_retirement_guard(observer.worker_guard());
        }
        drop(capture);
        Ok((builder, seed))
    }
}

/// No payload or semantic Execute record participates in one-way view sealing.
/// The admission retains the whole command through the owning worker's cleanup.
pub(in crate::runtime::repository_physical_root) struct FinalizeLaunch {
    admission: NativeViewFinalization,
    incoming: Option<AttachedMountClone>,
    pending: Option<ReadOnlyMountClone>,
    compaction: Option<PendingReadOnlyCompaction>,
    nonce: Option<[u8; 16]>,
}
impl FinalizeLaunch {
    pub(in crate::runtime::repository_physical_root) fn new(
        mut admission: NativeViewFinalization,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let incoming = admission.take_clone(active)?;
        admission
            .domain()
            .check_clone(&incoming, admission.scope(), active)
            .map_err(PhysicalRootError::mount)?;
        Ok(Self {
            admission,
            incoming: Some(incoming),
            pending: None,
            compaction: None,
            nonce: None,
        })
    }
}
impl ProcessLaunch for FinalizeLaunch {
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        if self.nonce.replace(*nonce).is_some() {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    #[cfg(test)]
    fn process_probe(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceProcessProbe> {
        self.admission.process_probe()
    }
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        Some(self.admission.retirement_observer())
    }
    type Output = (
        RepositoryWorkspaceCommand,
        SealedWorkspaceMaterialization,
        FinalizedRepositoryView,
    );
    fn mode(&self) -> RunMode {
        match self.admission.builder().mode {
            RepositoryWorkspaceMode::Archive => RunMode::FinalizeArchive,
            RepositoryWorkspaceMode::Process => RunMode::FinalizeProcess,
        }
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.admission.builder().reservation.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.admission.builder().reservation.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        None
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        None
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        Some(self.admission.builder().identity)
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        self.admission.scope()
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.admission.builder().resources
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(Some(self.admission.domain().borrow_namespace()))
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        let incoming = self.incoming.as_ref().ok_or(PhysicalRootError::Mode)?;
        self.admission
            .domain()
            .check_clone(incoming, self.scope(), &|| true)
            .map_err(PhysicalRootError::mount)?;
        send_incoming(
            socket,
            &self.admission.builder().file,
            &incoming.token,
            &incoming.owner,
            nonce,
            0,
        )
        .map_err(PhysicalRootError::mount)
    }
    fn verify_successor(
        &mut self,
        attached: File,
        token: File,
        userns: File,
        words: [u64; 8],
        quota: Option<[u64; 8]>,
    ) -> Result<(), PhysicalRootError> {
        if self.pending.is_some() || self.compaction.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let root = self.admission.builder();
        let successor = received_successor(
            &attached,
            token,
            userns,
            words,
            &root.resources,
            root.backing.clone(),
        )?;
        self.admission
            .domain()
            .check_clone(&successor, self.scope(), &|| true)
            .map_err(PhysicalRootError::mount)?;
        if successor.attached
            != self
                .incoming
                .as_ref()
                .ok_or(PhysicalRootError::Mode)?
                .detached
            || !same_backing(successor.attached, root.identity)
        {
            return Err(PhysicalRootError::Backing);
        }
        match (root.mode, quota) {
            (RepositoryWorkspaceMode::Archive, Some(words)) => {
                self.compaction = Some(PendingReadOnlyCompaction::receive(
                    root,
                    &successor,
                    &attached,
                    words,
                    self.nonce.ok_or(PhysicalRootError::Reservation)?,
                    self.scope(),
                )?);
            }
            (RepositoryWorkspaceMode::Process, None) => {
                if backing_identity(&attached, root.mode)? != successor.attached
                    || backing_identity(&root.file, root.mode)? != root.identity
                {
                    return Err(PhysicalRootError::Backing);
                }
            }
            _ => return Err(PhysicalRootError::Mode),
        }
        self.pending = Some(
            ReadOnlyMountClone::from_received_clone(successor).map_err(PhysicalRootError::mount)?,
        );
        Ok(())
    }
    fn finish_reaped(mut self, capture: ReapedCapture) -> Result<Self::Output, PhysicalRootError> {
        capture.binding().verify_finalize_attempt(
            self.admission.builder().identity,
            self.mode(),
            &self.nonce.ok_or(PhysicalRootError::Reservation)?,
        )?;
        if capture.return_code() != 0
            || !capture.stdout().is_empty()
            || !capture.stderr().is_empty()
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let pending = self.pending.take().ok_or(PhysicalRootError::Mode)?;
        // Final sealing sees no retained writable mount token.
        drop(self.incoming.take());
        self.admission
            .finish_reaped(capture, pending, self.compaction.take())
    }
}

#[cfg(test)]
#[path = "repository_process_launch/tests.rs"]
mod tests;
