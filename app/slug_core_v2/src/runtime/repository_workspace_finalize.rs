//! Consuming trusted finalization. The command and its entire immutable history
//! remain owned until setup is reaped and the existing full seal succeeds.
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;

use super::super::super::mount_lease::AttachedMountClone;
use super::super::super::mount_lease::launch::FinalizeLaunch;
use super::super::super::mount_lease::readonly::FinalizedRepositoryView;
use super::super::super::mount_lease::readonly::ReadOnlyMountClone;
use super::super::super::process_owner;
use super::super::super::process_owner::ReapedCapture;
use super::*;
use crate::runtime::repository_process_wire::RunMode;

pub(in crate::runtime::repository_physical_root) struct NativeViewFinalization {
    request: Arc<RepositoryMaterializationRequest>,
    command: RepositoryWorkspaceCommand,
    slot: usize,
    // Covers the independent incoming request until full seal has its own
    // retained request guard. It retires after both semantic owners above.
    _credit: resources::ResourceCredit,
}
impl RepositoryWorkspaceCommand {
    fn admit_view_finalization(
        self,
        request: Arc<RepositoryMaterializationRequest>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeViewFinalization, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let RepositoryMaterializationKind::GeneralWorkspace(expected) = &request.kind else {
            return Err(PhysicalRootError::Reservation);
        };
        let reservation = expected.reservation();
        if &request.id.workspace != reservation.workspace()
            || &request.id.canonical_repo != reservation.repository()
        {
            return Err(PhysicalRootError::Reservation);
        }
        let slot = self.root_slot(reservation)?;
        self.compare_transcript(slot)?;
        let progress = self.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let storage = request
            .general_workspace_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let bytes = if self
            .source_dependencies
            .covers_transcript_state_allocations(expected.transcript())
        {
            storage.owned_bytes()
        } else {
            storage.total_bytes().map_err(PhysicalRootError::Protocol)?
        };
        let credit = self.resources.reserve(
            add(bytes, std::mem::size_of::<NativeViewFinalization>())?,
            0,
            0,
        )?;
        let admission = NativeViewFinalization {
            request,
            command: self,
            slot,
            _credit: credit,
        };
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(expected) =
            &admission.request.kind
        else {
            unreachable!()
        };
        let progress = admission.command.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        if !ViewComparison::new(&admission.command.resources, scope, active)?
            .transcript_equal(expected.transcript(), progress.transcript())?
        {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        admission
            .domain()
            .check_root(
                expected.reservation(),
                &admission.command.resources,
                scope,
                active,
            )
            .map_err(PhysicalRootError::mount)?;
        Ok(admission)
    }

    /// Inactive physical capability: no Execute record/index is fabricated.
    /// A usable view is returned only alongside the existing actual full seal.
    pub(in crate::runtime::repository_physical_root) async fn finalize_read_view(
        self,
        request: Arc<RepositoryMaterializationRequest>,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
    ) -> Result<
        (
            Self,
            super::super::super::SealedWorkspaceMaterialization,
            FinalizedRepositoryView,
        ),
        PhysicalRootError,
    > {
        let admission = self.admit_view_finalization(request, scope, active)?;
        let launch = FinalizeLaunch::new(admission, active)?;
        process_owner::run(launch, super::super::super::initialize::platform()?, active).await
    }
}
impl NativeViewFinalization {
    pub(in crate::runtime::repository_physical_root) fn builder(
        &self,
    ) -> &RepositoryPhysicalRootBuilder {
        &self.command.roots[self.slot]
            .progress
            .as_ref()
            .expect("owned finalization root")
            .builder
    }
    pub(in crate::runtime::repository_physical_root) fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.command.scope
    }
    pub(in crate::runtime::repository_physical_root) fn domain(&self) -> &Arc<NativeMountDomain> {
        self.command
            .mount_domain
            .as_ref()
            .expect("admitted finalization domain")
    }
    pub(in crate::runtime::repository_physical_root) fn take_clone(
        &mut self,
        active: &dyn Fn() -> bool,
    ) -> Result<AttachedMountClone, PhysicalRootError> {
        let slot = &mut self.command.roots[self.slot];
        let progress = slot
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let domain = self
            .command
            .mount_domain
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let result = match (slot.initial.take(), slot.lease.take()) {
            (Some(initial), None) => {
                initial.into_clone_for_finalize(progress, domain, &self.command.scope, active)
            }
            (None, Some(lease)) => {
                lease.into_clone_for_finalize(progress, domain, &self.command.scope, active)
            }
            _ => return Err(PhysicalRootError::Mode),
        };
        result.map_err(PhysicalRootError::mount)
    }
    pub(in crate::runtime::repository_physical_root) fn finish_reaped(
        self,
        capture: ReapedCapture,
        pending: ReadOnlyMountClone,
    ) -> Result<
        (
            RepositoryWorkspaceCommand,
            super::super::super::SealedWorkspaceMaterialization,
            FinalizedRepositoryView,
        ),
        PhysicalRootError,
    > {
        let mode = match self.builder().mode {
            RepositoryWorkspaceMode::Archive => RunMode::FinalizeArchive,
            RepositoryWorkspaceMode::Process => RunMode::FinalizeProcess,
        };
        capture
            .binding()
            .verify_finalize(self.builder().identity, mode)?;
        if capture.return_code() != 0
            || !capture.stdout().is_empty()
            || !capture.stderr().is_empty()
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let scope = self.scope().clone();
        let domain = self.domain().clone();
        let (command, sealed) =
            self.command
                .seal_for_materialization(self.request, &scope, &|| true)?;
        let view = FinalizedRepositoryView::bind(&sealed, pending, domain, &scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(capture);
        Ok((command, sealed, view))
    }
}
