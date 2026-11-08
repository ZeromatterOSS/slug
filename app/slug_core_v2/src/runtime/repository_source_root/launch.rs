//! Closed no-payload native source setup/finalization. No General operation or
//! semantic process result is synthesized for immutable sources.
use slug_bzlmod_v2::RepositorySourceSnapshotReservation as Reservation;
use slug_bzlmod_v2::RepositorySourceSnapshotState as State;

use super::super::process_owner::ProcessLaunch;
use super::super::process_owner::ReapedCapture;
use super::super::process_owner::{self};
use super::super::source::SealedSourceSnapshot;
use super::super::source::SourceRootBuilder;
use super::*;
use crate::runtime::repository_process_wire::RunMode;

#[path = "readonly_quota.rs"]
pub(in crate::runtime::repository_physical_root) mod readonly_quota;
use readonly_quota::PendingSourceReadOnlyCompaction;

pub(in crate::runtime::repository_physical_root) struct SourceMountSeed {
    clone: AttachedMountClone,
}
pub(in crate::runtime::repository_physical_root) struct SourceBootstrap {
    root: Option<File>,
    successor: Option<AttachedMountClone>,
    reservation: Arc<Reservation>,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    backing: Arc<RepositoryRootBacking>,
    domain: Option<Arc<domain::NativeMountDomain>>,
    observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
    _credit: ResourceCredit,
}
impl SourceBootstrap {
    pub(in crate::runtime::repository_physical_root) fn new(
        reservation: Arc<Reservation>,
        claim: Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        domain: Option<Arc<domain::NativeMountDomain>>,
        observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(&scope, active)?;
        check_bank(&scope, &resources)?;
        if reservation.workspace() != scope.workspace()
            || !claim.matches_reservation(&reservation)
            || !claim.belongs_to(&resources)
        {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some(d) = &domain {
            d.check_source(&reservation, &resources, &scope, active)
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
        backing.bind_source_claim(&reservation, claim)?;
        Ok(Self {
            root: None,
            successor: None,
            reservation,
            resources,
            scope,
            backing,
            domain,
            observer,
            _credit: credit,
        })
    }
}
impl ProcessLaunch for SourceBootstrap {
    type Output = SourceRootBuilder;
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
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        Some(self.observer.clone())
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(self.domain.as_ref().map(|d| d.borrow_namespace()))
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
        let successor = super::launch::received_successor(
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
    fn finish_reaped(
        mut self,
        capture: ReapedCapture,
    ) -> Result<SourceRootBuilder, PhysicalRootError> {
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
        let clone = self.successor.take().ok_or(PhysicalRootError::Mode)?;
        let domain = match self.domain.take() {
            Some(domain) => domain,
            None => domain::NativeMountDomain::from_source(
                &self.reservation,
                &clone,
                &self.resources,
                &self.scope,
                &|| true,
            )
            .map_err(PhysicalRootError::mount)?,
        };
        domain
            .check_source(&self.reservation, &self.resources, &self.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        SourceRootBuilder::from_bootstrap(
            self.root.take().ok_or(PhysicalRootError::Mode)?,
            self.reservation,
            self.resources,
            self.backing,
            self.scope,
            domain,
            SourceMountSeed { clone },
            &self.observer,
        )
    }
}
pub(in crate::runtime::repository_physical_root) struct FinalizedSource {
    pub(in crate::runtime::repository_physical_root) receipt: SealedSourceSnapshot,
    pub(in crate::runtime::repository_physical_root) view: readonly::FinalizedRepositoryView,
    _credit: ResourceCredit,
    _retirement: super::super::progress::command::RepositoryWorkspaceRetirementGuard,
}
pub(in crate::runtime::repository_physical_root) struct SourceFinalize {
    builder: SourceRootBuilder,
    state: Arc<State>,
    state_credit: ResourceCredit,
    incoming: Option<SourceMountSeed>,
    pending: Option<readonly::ReadOnlyMountClone>,
    compaction: Option<PendingSourceReadOnlyCompaction>,
    nonce: Option<[u8; 16]>,
    observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
    _credit: ResourceCredit,
}
impl SourceFinalize {
    pub(in crate::runtime::repository_physical_root) fn new(
        mut builder: SourceRootBuilder,
        state: Arc<State>,
        state_credit: ResourceCredit,
        observer: super::super::progress::command::RepositoryWorkspaceRetirementObserver,
    ) -> Result<Self, PhysicalRootError> {
        builder.check(&builder.scope, &|| true)?;
        if state.reservation() != &builder.reservation {
            return Err(PhysicalRootError::Reservation);
        }
        let credit = builder.resources.reserve(
            std::mem::size_of::<Self>() + std::mem::size_of::<FinalizedSource>(),
            0,
            0,
        )?;
        let incoming = builder.seed.take().ok_or(PhysicalRootError::Mode)?;
        Ok(Self {
            builder,
            state,
            state_credit,
            incoming: Some(incoming),
            pending: None,
            compaction: None,
            nonce: None,
            observer,
            _credit: credit,
        })
    }
}
impl ProcessLaunch for SourceFinalize {
    type Output = FinalizedSource;
    fn mode(&self) -> RunMode {
        RunMode::FinalizeSourceArchive
    }
    fn workspace(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder.reservation.workspace()
    }
    fn display_root(&self) -> &slug_workspace_v2::NormalizedAbsolutePath {
        self.builder.reservation.display_root()
    }
    fn request(&self) -> Option<&RepositoryWorkspaceRequest> {
        None
    }
    fn covered_request_owner(&self) -> Option<Arc<CoveredWorkspaceRequest>> {
        None
    }
    fn root_identity(&self) -> Option<RootIdentity> {
        Some(self.builder.identity)
    }
    fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.builder.scope
    }
    fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.builder.resources
    }
    fn retirement_observer(
        &self,
    ) -> Option<super::super::progress::command::RepositoryWorkspaceRetirementObserver> {
        Some(self.observer.clone())
    }
    fn namespace(&self) -> Result<Option<BorrowedFd<'_>>, PhysicalRootError> {
        Ok(Some(self.builder.domain.borrow_namespace()))
    }
    fn bind_attempt(&mut self, nonce: &[u8; 16]) -> Result<(), PhysicalRootError> {
        if self.nonce.replace(*nonce).is_some() {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    fn transfer(
        &mut self,
        socket: BorrowedFd<'_>,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        let incoming = &self.incoming.as_ref().ok_or(PhysicalRootError::Mode)?.clone;
        self.builder
            .domain
            .check_clone(incoming, &self.builder.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        send_incoming(
            socket,
            &self.builder.file,
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
        let successor = super::launch::received_successor(
            &attached,
            token,
            userns,
            words,
            &self.builder.resources,
            self.builder.backing.clone(),
        )?;
        self.builder
            .domain
            .check_clone(&successor, &self.builder.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        if successor.attached
            != self
                .incoming
                .as_ref()
                .ok_or(PhysicalRootError::Mode)?
                .clone
                .detached
            || !same_backing(successor.attached, self.builder.identity)
        {
            return Err(PhysicalRootError::Backing);
        }
        self.compaction = Some(PendingSourceReadOnlyCompaction::receive(
            &self.builder,
            &successor,
            &attached,
            quota.ok_or(PhysicalRootError::Mode)?,
            self.nonce.ok_or(PhysicalRootError::Reservation)?,
            &self.builder.scope,
        )?);
        self.pending = Some(
            readonly::ReadOnlyMountClone::from_received_clone(successor)
                .map_err(PhysicalRootError::mount)?,
        );
        Ok(())
    }
    fn finish_reaped(
        mut self,
        capture: ReapedCapture,
    ) -> Result<FinalizedSource, PhysicalRootError> {
        capture.binding().verify_finalize_attempt(
            self.builder.identity,
            RunMode::FinalizeSourceArchive,
            &self.nonce.ok_or(PhysicalRootError::Reservation)?,
        )?;
        if capture.return_code() != 0
            || !capture.stdout().is_empty()
            || !capture.stderr().is_empty()
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let compaction = self.compaction.take().ok_or(PhysicalRootError::Mode)?;
        compaction.stage_after_reap(&mut self.builder, &capture)?;
        // No writable setup token survives complete digest-tree sealing.
        drop(self.incoming.take());
        let domain = self.builder.domain.clone();
        let scope = self.builder.scope.clone();
        let resources = self.builder.resources.clone();
        let receipt =
            self.builder
                .seal(self.state, PathObservationEpoch::empty(), &scope, &|| true)?;
        let pending = self.pending.take().ok_or(PhysicalRootError::Mode)?;
        // Full debit survives reap, original-FD re-attestation, complete tree
        // verification and the exact matching read-only successor proof.
        compaction.commit(receipt.root(), &pending, &capture)?;
        let view = readonly::FinalizedRepositoryView::bind_source(
            &receipt,
            pending,
            domain,
            &scope,
            &|| true,
        )
        .map_err(PhysicalRootError::mount)?
        .with_retirement_observer(self.observer.clone());
        let credit = resources.reserve(std::mem::size_of::<FinalizedSource>(), 0, 0)?;
        Ok(FinalizedSource {
            receipt,
            view,
            _credit: credit,
            _retirement: self.observer.worker_guard(),
        })
    }
}
