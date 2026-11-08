//! Exclusive native command owner. Immutable epochs project to DICE; neither a
//! projected old epoch nor an individual root can mint process admission. Taking
//! the whole command prevents concurrent roots spending the same history budget.
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::super::mount_lease::InitialRepositoryMount;
use super::super::mount_lease::quota::VerifiedProcessQuota;
use super::*;

struct CreditedEpoch {
    epoch: RepositoryWorkspaceEpoch,
    _credit: resources::ResourceCredit,
}
struct NativeRootSlot {
    reservation: Arc<RepositoryWorkspaceReservation>,
    progress: Option<RepositoryWorkspaceProgress>,
    initial: Option<InitialRepositoryMount>,
}

/// Created fresh once by the native command lifecycle, never from an imported
/// epoch. This is unfinished command state, not a second materialization registry.
/// No Clone, root FD, generic mutation callback or child result injection exists.
pub(in crate::runtime) struct RepositoryWorkspaceCommand {
    roots: Vec<NativeRootSlot>,
    epoch: CreditedEpoch,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    max_roots: usize,
    _control: resources::ResourceCredit,
}

impl RepositoryWorkspaceCommand {
    pub(in crate::runtime) fn new(
        workspace: NormalizedAbsolutePath,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(&scope, active)?;
        if &workspace != scope.workspace() {
            return Err(PhysicalRootError::SourceService(
                SourceServiceError::ForeignScope,
            ));
        }
        let control = resources.reserve(std::mem::size_of::<Self>(), 0, 0)?;
        let bytes = RepositoryWorkspaceEpoch::fresh_storage_bytes(&workspace, lease, limits)
            .map_err(PhysicalRootError::Protocol)?;
        let credit = resources.reserve(add(bytes, std::mem::size_of::<CreditedEpoch>())?, 0, 0)?;
        let epoch = RepositoryWorkspaceEpoch::fresh(workspace, lease, limits)
            .map_err(PhysicalRootError::Protocol)?;
        Ok(Self {
            roots: Vec::new(),
            epoch: CreditedEpoch {
                epoch,
                _credit: credit,
            },
            scope,
            resources,
            max_roots: limits.max_state_versions(),
            _control: control,
        })
    }
    pub(in crate::runtime) fn epoch(&self) -> &RepositoryWorkspaceEpoch {
        &self.epoch.epoch
    }
    fn check_scope(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        crate::runtime::repository_io::source_service::check_identity(
            Some(&self.scope),
            Some(scope),
        )?;
        check(scope, active)
    }
    fn root_slot(
        &self,
        reservation: &RepositoryWorkspaceReservation,
    ) -> Result<usize, PhysicalRootError> {
        self.roots
            .iter()
            .position(|slot| slot.reservation.as_ref() == reservation && slot.progress.is_some())
            .ok_or(PhysicalRootError::Reservation)
    }
    fn compare_transcript(&self, slot: usize) -> Result<(), PhysicalRootError> {
        let native = self.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let semantic = self
            .epoch()
            .transcripts()
            .ok_or(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::Revoked,
            ))?
            .iter()
            .find(|t| t.reservation() == &self.roots[slot].reservation)
            .ok_or(PhysicalRootError::Reservation)?;
        if semantic != native.transcript() {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        Ok(())
    }
    fn grow_roots(&mut self) -> Result<(), PhysicalRootError> {
        if self.roots.len() == self.roots.capacity() {
            let capacity = self
                .roots
                .capacity()
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?
                .max(4)
                .min(self.max_roots);
            if capacity <= self.roots.len() {
                return Err(PhysicalRootError::Resource);
            }
            self._control
                .bytes
                .try_grow(add(
                    capacity
                        .checked_mul(std::mem::size_of::<NativeRootSlot>())
                        .ok_or(PhysicalRootError::Resource)?,
                    64,
                )?)
                .map_err(|_| PhysicalRootError::Resource)?;
            self.roots
                .try_reserve_exact(capacity - self.roots.len())
                .map_err(|_| PhysicalRootError::Resource)?;
            if self.roots.capacity() > capacity {
                return Err(PhysicalRootError::Resource);
            }
        }
        Ok(())
    }
    /// The initialized seed owns the same prepaid backing and has never executed
    /// a payload. It remains command-owned through deterministic mutations.
    pub(in crate::runtime::repository_physical_root) fn reserve_root(
        mut self,
        builder: RepositoryPhysicalRootBuilder,
        initial: InitialRepositoryMount,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope, active)?;
        if !Arc::ptr_eq(&self.resources, &builder.resources) {
            return Err(PhysicalRootError::Resource);
        }
        let _scratch = self.resources.reserve(IO_SCRATCH, 0, 0)?;
        initial
            .check_binding(&builder, scope, active)
            .map_err(PhysicalRootError::mount)?;
        let reservation = builder.reservation.clone();
        let root_mode = (RootIo::new(&builder.file, io_limits(builder.mode, &reservation))?
            .root_stat()?
            .st_mode
            & 0o7777) as u16;
        let forecast = self
            .epoch()
            .reserve_storage_bytes(&reservation, root_mode)
            .map_err(PhysicalRootError::Protocol)?;
        let credit =
            self.resources
                .reserve(add(forecast, std::mem::size_of::<CreditedEpoch>())?, 0, 0)?;
        let epoch = self
            .epoch()
            .reserve(reservation.clone(), root_mode)
            .map_err(PhysicalRootError::Protocol)?;
        let progress = RepositoryWorkspaceProgress::new(builder, scope, active)?;
        self.grow_roots()?;
        self.roots.push(NativeRootSlot {
            reservation,
            progress: Some(progress),
            initial: Some(initial),
        });
        self.epoch = CreditedEpoch {
            epoch,
            _credit: credit,
        };
        Ok(self)
    }
    pub(in crate::runtime) fn request_at(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        index: usize,
        operation: RepositoryWorkspaceOperation,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryWorkspaceRequest, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(reservation)?;
        self.compare_transcript(slot)?;
        self.epoch()
            .request_at(reservation, index, operation)
            .map_err(PhysicalRootError::Protocol)
    }
    pub(in crate::runtime) fn request(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        operation: RepositoryWorkspaceOperation,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<RepositoryWorkspaceRequest, PhysicalRootError> {
        let slot = self.root_slot(reservation)?;
        self.request_at(
            reservation,
            self.roots[slot]
                .progress
                .as_ref()
                .unwrap()
                .transcript()
                .records()
                .len(),
            operation,
            scope,
            active,
        )
    }
    fn publish(&mut self, slot: usize) -> Result<(), PhysicalRootError> {
        let progress = self.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let forecast = self
            .epoch()
            .publish_storage_bytes(progress.transcript())
            .map_err(PhysicalRootError::Protocol)?;
        let credit =
            self.resources
                .reserve(add(forecast, std::mem::size_of::<CreditedEpoch>())?, 0, 0)?;
        let epoch = self
            .epoch()
            .publish_completed(progress.transcript().clone())
            .map_err(PhysicalRootError::Protocol)?;
        self.epoch = CreditedEpoch {
            epoch,
            _credit: credit,
        };
        Ok(())
    }
    /// All native errors consume the entire command. Exact replay performs no
    /// IO, acquisition, transcript adoption or counter reset.
    pub(in crate::runtime) fn apply(
        mut self,
        request: RepositoryWorkspaceRequest,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(request.reservation())?;
        self.compare_transcript(slot)?;
        if self.roots[slot]
            .progress
            .as_ref()
            .unwrap()
            .transcript()
            .replay(&request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(self);
        }
        let progress = self.roots[slot].progress.take().unwrap();
        let progress = if matches!(
            request.operation(),
            RepositoryWorkspaceOperation::Archive(_)
        ) {
            progress.apply_archive(request, runtime, distdirs, scope, active)?
        } else {
            progress.apply(request, scope, active)?
        };
        self.roots[slot].progress = Some(progress);
        self.publish(slot)?;
        Ok(self)
    }
    /// No externally supplied epoch can mint this capability. Moving the whole
    /// command prevents another root from spending its Process-domain history
    /// budget while setup or a child is in flight.
    pub(in crate::runtime::repository_physical_root) fn admit_first_execute(
        mut self,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeProcessAdmission, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(request.reservation())?;
        self.compare_transcript(slot)?;
        let root = self.roots[slot].progress.as_ref().unwrap();
        if root.builder.mode != RepositoryWorkspaceMode::Archive
            || root.transcript().current().mode() != RepositoryWorkspaceMode::Archive
            || self.roots[slot].initial.is_none()
            || request.index() != root.transcript().records().len()
            || !matches!(
                request.operation(),
                RepositoryWorkspaceOperation::Execute(_)
            )
        {
            return Err(PhysicalRootError::Mode);
        }
        let credit = self.resources.reserve(
            add(
                arc_bytes::<RepositoryWorkspaceRequest>()?,
                add(
                    std::mem::size_of::<NativeProcessAdmission>(),
                    request
                        .retained_storage_bytes()
                        .map_err(PhysicalRootError::Protocol)?,
                )?,
            )?,
            0,
            0,
        )?;
        let expected = self
            .epoch()
            .request_at(
                request.reservation(),
                request.index(),
                request.operation().clone(),
            )
            .map_err(PhysicalRootError::Protocol)?;
        if expected != request {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        drop(expected);
        let _scratch = self.resources.reserve(IO_SCRATCH, 3, 0)?;
        if backing_identity(&root.builder.file, RepositoryWorkspaceMode::Archive)?
            != root.builder.identity
        {
            return Err(PhysicalRootError::RootIdentity);
        }
        let io = RootIo::new(
            &root.builder.file,
            io_limits(RepositoryWorkspaceMode::Process, &root.builder.reservation),
        )?;
        seal::verify(&io, root.transcript().current(), scope, active)?;
        if backing_identity(&root.builder.file, RepositoryWorkspaceMode::Archive)?
            != root.builder.identity
        {
            return Err(PhysicalRootError::RootIdentity);
        }
        check(scope, active)?;
        let selected = self.roots[slot].progress.take().unwrap();
        let initial = self.roots[slot].initial.take().unwrap();
        Ok(NativeProcessAdmission {
            selected,
            initial: Some(initial),
            request: Arc::new(request),
            slot,
            command: self,
            quota_confirmed: false,
            _credit: credit,
        })
    }
    pub(in crate::runtime) fn seal_root(
        mut self,
        reservation: &RepositoryWorkspaceReservation,
        consumed: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(Self, Arc<RepositoryPhysicalRoot>), PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(reservation)?;
        self.compare_transcript(slot)?;
        drop(self.roots[slot].initial.take());
        let root = self.roots[slot]
            .progress
            .take()
            .unwrap()
            .seal(consumed, scope, active)?;
        Ok((self, root))
    }
}

pub(in crate::runtime::repository_physical_root) struct NativeProcessAdmission {
    selected: RepositoryWorkspaceProgress,
    initial: Option<InitialRepositoryMount>,
    request: Arc<RepositoryWorkspaceRequest>,
    slot: usize,
    command: RepositoryWorkspaceCommand,
    quota_confirmed: bool,
    _credit: resources::ResourceCredit,
}
impl NativeProcessAdmission {
    pub(in crate::runtime::repository_physical_root) fn builder(
        &self,
    ) -> &RepositoryPhysicalRootBuilder {
        &self.selected.builder
    }
    pub(in crate::runtime::repository_physical_root) fn request(
        &self,
    ) -> &RepositoryWorkspaceRequest {
        &self.request
    }
    pub(in crate::runtime::repository_physical_root) fn request_owner(
        &self,
    ) -> Arc<RepositoryWorkspaceRequest> {
        self.request.clone()
    }
    pub(in crate::runtime::repository_physical_root) fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.command.scope
    }
    pub(in crate::runtime::repository_physical_root) fn take_initial_seed(
        &mut self,
    ) -> Result<InitialRepositoryMount, PhysicalRootError> {
        self.initial.take().ok_or(PhysicalRootError::Mode)
    }
    pub(in crate::runtime::repository_physical_root) fn commit_process_in_place(
        &mut self,
        proof: &VerifiedProcessQuota,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.command.check_scope(&self.command.scope, active)?;
        if self.quota_confirmed {
            return Err(PhysicalRootError::Mode);
        }
        proof.commit(
            &mut self.selected.builder,
            &self.request,
            &self.command.scope,
            active,
        )?;
        self.quota_confirmed = true;
        Ok(())
    }
    /// Called by the owning lifecycle only after its setup/child boundary is
    /// settled. This is quota readiness, never an execution/quiescence receipt.
    pub(in crate::runtime::repository_physical_root) fn into_pending_first_execute(
        self,
    ) -> Result<PendingFirstExecute, PhysicalRootError> {
        if !self.quota_confirmed {
            return Err(PhysicalRootError::Mode);
        }
        Ok(PendingFirstExecute { admission: self })
    }
}
pub(in crate::runtime::repository_physical_root) struct PendingFirstExecute {
    admission: NativeProcessAdmission,
}
impl PendingFirstExecute {
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
}

#[cfg(test)]
#[path = "repository_workspace_command/tests.rs"]
mod tests;
