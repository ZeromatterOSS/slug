//! Exclusive controller checkout. All selected facts validate before any token
//! leaves the command; ordinary semantic records use the same actual runner.
use slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest;

use super::super::super::mount_lease::batch::NativeViewBatch;
use super::super::super::mount_lease::batch::RestoredProcessViews;
use super::super::super::mount_lease::launch::ExecuteLaunch;
use super::super::super::mount_lease::launch::SelectedExecuteDecision;
use super::super::super::mount_lease::launch::SelectedExecuteLaunch;
use super::super::super::mount_lease::readonly::FinalizedRepositoryView;
use super::*;
use crate::runtime::repository_io::process_views::SelectedProcessSources;

pub(super) struct RetainedProcessView {
    view: FinalizedRepositoryView,
    data: Arc<NativeReadViewData>,
}
impl RepositoryWorkspaceCommand {
    pub(in crate::runtime::repository_physical_root) fn install_process_view(
        mut self,
        sealed: &super::super::super::SealedWorkspaceMaterialization,
        view: FinalizedRepositoryView,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let reservation = sealed.root().reservation();
        let slot = self
            .roots
            .iter()
            .position(|slot| &slot.reservation == reservation)
            .ok_or(PhysicalRootError::Reservation)?;
        if self.roots[slot].progress.is_some()
            || self.roots[slot].initial.is_some()
            || self.roots[slot].lease.is_some()
            || self.roots[slot].process_view.is_some()
            || view.instance() != reservation.instance()
            || view.root_identity() != sealed.root().identity
            || !Arc::ptr_eq(
                view.domain(),
                self.mount_domain
                    .as_ref()
                    .ok_or(PhysicalRootError::Reservation)?,
            )
        {
            return Err(PhysicalRootError::Reservation);
        }
        let data = NativeReadViewData::from_sealed(sealed)?;
        let next = self
            .source_dependencies
            .merge(std::slice::from_ref(&data), &self.resources)?;
        self.roots[slot].process_view = Some(RetainedProcessView { view, data });
        self.source_dependencies = next;
        Ok(self)
    }
    pub(in crate::runtime) async fn finalize_workspace(
        self,
        request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Self, super::super::super::SealedWorkspaceMaterialization), PhysicalRootError>
    {
        let (command, sealed, view) = self.finalize_read_view(request, scope, active).await?;
        let command = command.install_process_view(&sealed, view, scope, active)?;
        Ok((command, sealed))
    }
    pub(in crate::runtime::repository_physical_root) fn admit_selected_execute(
        mut self,
        request: RepositoryWorkspaceRequest,
        sources: &SelectedProcessSources,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<SelectedExecuteDecision, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(request.reservation())?;
        self.compare_transcript(slot)?;
        if !matches!(
            request.operation(),
            RepositoryWorkspaceOperation::Execute(_)
        ) {
            return Err(PhysicalRootError::Mode);
        }
        // The raw incoming request remains guarded until its foreign projection
        // owners are dropped after canonicalization. No source bytes are copied.
        let partition = request
            .storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let mut canonical_pointers = true;
        request.visit_read_views(&mut |view| {
            canonical_pointers &= self
                .source_dependencies
                .owners()
                .iter()
                .any(|owner| Arc::ptr_eq(owner.projection().state(), view.state()))
        });
        let bytes = if canonical_pointers {
            partition.owned_bytes()
        } else {
            partition
                .total_bytes()
                .map_err(PhysicalRootError::Protocol)?
        };
        struct Incoming {
            request: RepositoryWorkspaceRequest,
            _credit: ResourceCredit,
        }
        let incoming = Incoming {
            request,
            _credit: self.resources.reserve(bytes, 0, 0)?,
        };
        let RepositoryWorkspaceOperation::Execute(execute) = incoming.request.operation() else {
            unreachable!()
        };
        sources.validate_manifest(
            execute.read_views(),
            incoming.request.reservation(),
            &self.resources,
            scope,
            active,
        )?;
        let count = execute.read_views().views().len();
        let _selection = self.resources.reserve(
            add(
                4096,
                count.checked_mul(512).ok_or(PhysicalRootError::Resource)?,
            )?,
            0,
            0,
        )?;
        let mut indices = Vec::with_capacity(count);
        let mut canonical_bytes = incoming
            .request
            .storage_partition()
            .map_err(PhysicalRootError::Protocol)?
            .owned_bytes();
        for projection in execute.read_views().views() {
            let index = self
                .roots
                .iter()
                .position(|slot| {
                    slot.process_view.as_ref().is_some_and(|view| {
                        view.data.projection().state().reservation()
                            == projection.state().reservation()
                    })
                })
                .ok_or(PhysicalRootError::Reservation)?;
            let controller = self.roots[index].process_view.as_ref().unwrap();
            if controller.data.projection().repo_spec() != projection.repo_spec()
                || !sources.matches_data(&controller.data)
                || !Arc::ptr_eq(
                    controller.view.domain(),
                    self.mount_domain
                        .as_ref()
                        .ok_or(PhysicalRootError::Reservation)?,
                )
            {
                return Err(PhysicalRootError::Reservation);
            }
            indices.push(index);
            canonical_bytes = add(
                canonical_bytes,
                controller
                    .data
                    .projection()
                    .storage_partition()
                    .map_err(PhysicalRootError::Protocol)?
                    .owned_bytes(),
            )?;
        }
        let _construction = self.resources.reserve(
            add(
                canonical_bytes,
                add(
                    4096,
                    count.checked_mul(512).ok_or(PhysicalRootError::Resource)?,
                )?,
            )?,
            0,
            0,
        )?;
        let canonical = indices
            .iter()
            .map(|index| {
                self.roots[*index]
                    .process_view
                    .as_ref()
                    .unwrap()
                    .data
                    .projection()
                    .clone()
            })
            .collect();
        let manifest =
            RepositoryWorkspaceReadViewManifest::new(incoming.request.reservation(), canonical)
                .map_err(PhysicalRootError::Protocol)?;
        let request = incoming
            .request
            .clone()
            .with_read_views(manifest)
            .map_err(PhysicalRootError::Protocol)?;
        let covered = CoveredWorkspaceRequest::from_canonical(
            request,
            self.source_dependencies.clone(),
            &self.resources,
        )?;
        let progress = self.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        if progress
            .transcript()
            .replay(covered.request())
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(SelectedExecuteDecision::Replay(self));
        }
        // Full admission and construction credits precede the infallible local
        // checkout. The command is exclusive, so there is no token mutex/race.
        let executor = progress.builder.identity;
        let domain = self
            .mount_domain
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?
            .clone();
        let resources = self.resources.clone();
        let scope = scope.clone();
        let mut pairs = Vec::with_capacity(count);
        for index in indices {
            let retained = self.roots[index]
                .process_view
                .take()
                .expect("validated exclusive controller");
            pairs.push((retained.view, retained.data));
        }
        let batch = NativeViewBatch::new(
            pairs,
            covered.clone(),
            executor,
            domain,
            scope.clone(),
            resources,
        )?;
        let mode = self.roots[slot].progress.as_ref().unwrap().builder.mode;
        let admission = self.admit_native_covered(covered, mode, &scope, active)?;
        let root = ExecuteLaunch::new(admission, active)?;
        Ok(SelectedExecuteDecision::Launch(SelectedExecuteLaunch::new(
            root, batch,
        )?))
    }
    pub(in crate::runtime) async fn execute_selected_sources(
        self,
        request: RepositoryWorkspaceRequest,
        sources: &SelectedProcessSources,
        actor: &crate::runtime::repository_io::RepositoryMaterializer,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, PhysicalRootError> {
        let decision = self.admit_selected_execute(request, sources, scope, active)?;
        sources.recheck(actor)?;
        super::super::super::run_selected_process(decision, active).await
    }
    pub(in crate::runtime::repository_physical_root) fn restore_process_views(
        mut self,
        mut restored: RestoredProcessViews,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope, active)?;
        for (view, data) in restored.pairs() {
            let slot = self
                .roots
                .iter()
                .find(|slot| &slot.reservation == data.root().reservation())
                .ok_or(PhysicalRootError::Reservation)?;
            if slot.process_view.is_some()
                || view.instance() != data.root().reservation().instance()
                || view.root_identity() != data.root().identity
                || !Arc::ptr_eq(
                    view.domain(),
                    self.mount_domain
                        .as_ref()
                        .ok_or(PhysicalRootError::Reservation)?,
                )
            {
                return Err(PhysicalRootError::Reservation);
            }
        }
        for (view, data) in restored.take_pairs() {
            let slot = self
                .roots
                .iter_mut()
                .find(|slot| &slot.reservation == data.root().reservation())
                .expect("validated restore slot");
            slot.process_view = Some(RetainedProcessView { view, data });
        }
        Ok(self)
    }
}
