//! One pre-admitted selected set. Every phase keeps data and setup ownership
//! armed until the existing process worker establishes actual namespace death.
use super::domain::NativeMountDomain;
use super::readonly::FinalizedRepositoryView;
use super::readonly::PendingReadOnlyView;
use super::readonly::ReadOnlyViewAttempt;
use super::readonly::ReadOnlyViewInFlight;
use super::readonly::ViewUseBinding;
use super::*;
use crate::runtime::repository_process_wire as wire;

struct Slot {
    data: Option<Arc<NativeReadViewData>>,
    ready: Option<FinalizedRepositoryView>,
    attempt: Option<ReadOnlyViewAttempt>,
    inflight: Option<ReadOnlyViewInFlight>,
    pending: Option<PendingReadOnlyView>,
}
pub(in crate::runtime::repository_physical_root) struct NativeViewBatch {
    incoming: Vec<(FinalizedRepositoryView, Arc<NativeReadViewData>)>,
    slots: Vec<Slot>,
    entries: Vec<wire::ViewEntry>,
    covered: Arc<CoveredWorkspaceRequest>,
    executor: RootIdentity,
    domain: Arc<NativeMountDomain>,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    binding: Option<Arc<ViewUseBinding>>,
    _credit: Arc<resources::ResourceCredit>,
}
pub(in crate::runtime::repository_physical_root) struct RestoredProcessViews {
    pairs: Vec<(FinalizedRepositoryView, Arc<NativeReadViewData>)>,
    _credit: Arc<resources::ResourceCredit>,
}
impl RestoredProcessViews {
    pub(in crate::runtime::repository_physical_root) fn pairs(
        &self,
    ) -> &[(FinalizedRepositoryView, Arc<NativeReadViewData>)] {
        &self.pairs
    }
    /// Caller keeps this bundle alive until its pre-admitted command pool owns
    /// every returned element and its retained allocation.
    pub(in crate::runtime::repository_physical_root) fn take_pairs(
        &mut self,
    ) -> Vec<(FinalizedRepositoryView, Arc<NativeReadViewData>)> {
        std::mem::take(&mut self.pairs)
    }
}
impl NativeViewBatch {
    pub(in crate::runtime::repository_physical_root) fn new(
        pairs: Vec<(FinalizedRepositoryView, Arc<NativeReadViewData>)>,
        covered: Arc<CoveredWorkspaceRequest>,
        executor: RootIdentity,
        domain: Arc<NativeMountDomain>,
        scope: Arc<SourceServiceScope>,
        resources: Arc<RepositoryRootResources>,
    ) -> Result<Self, PhysicalRootError> {
        let n = pairs.len();
        if n > wire::MAX_VIEWS || pairs.capacity() > wire::MAX_VIEWS {
            return Err(PhysicalRootError::Resource);
        }
        let pair_bytes = std::mem::size_of::<(FinalizedRepositoryView, Arc<NativeReadViewData>)>();
        let bytes = add(
            add(
                std::mem::size_of::<Self>(),
                add(
                    std::mem::size_of::<RestoredProcessViews>(),
                    arc_bytes::<resources::ResourceCredit>()?,
                )?,
            )?,
            add(
                pairs
                    .capacity()
                    .checked_mul(pair_bytes)
                    .ok_or(PhysicalRootError::Resource)?,
                n.checked_mul(
                    std::mem::size_of::<Slot>()
                        + std::mem::size_of::<wire::ViewEntry>()
                        + pair_bytes
                        + 4098,
                )
                .ok_or(PhysicalRootError::Resource)?,
            )?,
        )?;
        let credit = Arc::new(resources.reserve(
            bytes,
            n.checked_mul(8).ok_or(PhysicalRootError::Resource)?,
            0,
        )?);
        // Arm input owners before any subsequent validation/allocation can fail.
        let mut owner = Self {
            incoming: pairs,
            slots: Vec::new(),
            entries: Vec::new(),
            covered,
            executor,
            domain,
            scope,
            resources,
            binding: None,
            _credit: credit,
        };
        check(&owner.scope, &|| true)?;
        let RepositoryWorkspaceOperation::Execute(execute) = owner.covered.request().operation()
        else {
            return Err(PhysicalRootError::Mode);
        };
        let manifest = execute.read_views();
        manifest
            .validate_for(owner.covered.request().reservation())
            .map_err(PhysicalRootError::Protocol)?;
        if manifest.views().len() != n {
            return Err(PhysicalRootError::Reservation);
        }
        owner.incoming.sort_unstable_by(|a, b| {
            a.1.projection()
                .display_root()
                .cmp(b.1.projection().display_root())
                .then_with(|| {
                    a.1.projection()
                        .repository()
                        .cmp(b.1.projection().repository())
                })
        });
        // The whole metadata set is checked before any native descriptor probe.
        for ((view, data), expected) in owner.incoming.iter().zip(manifest.views()) {
            if !Arc::ptr_eq(view.domain(), &owner.domain)
                || !match (
                    view.data_root(),
                    data.root(),
                    view.source_data_root(),
                    data.source_root(),
                ) {
                    (Some(a), Some(b), None, None) => Arc::ptr_eq(a, b),
                    (None, None, Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    _ => false,
                }
                || !owner
                    .covered
                    .dependencies()
                    .owners()
                    .iter()
                    .any(|known| Arc::ptr_eq(known, data))
                || !data.same_state_allocation(expected)
                || data.projection().repo_spec() != expected.repo_spec()
                || view.display() != expected.display_root()
                || view.instance() != expected.instance()
                || view.display().as_path().as_os_str().len() > 4096
            {
                return Err(PhysicalRootError::Reservation);
            }
        }
        owner
            .domain
            .check_root(
                owner.covered.request().reservation(),
                &owner.resources,
                &owner.scope,
                &|| true,
            )
            .map_err(PhysicalRootError::mount)?;
        owner
            .slots
            .try_reserve_exact(n)
            .map_err(|_| PhysicalRootError::Resource)?;
        owner
            .entries
            .try_reserve_exact(n)
            .map_err(|_| PhysicalRootError::Resource)?;
        if owner.slots.capacity() > n || owner.entries.capacity() > n {
            return Err(PhysicalRootError::Resource);
        }
        for (view, data) in owner.incoming.drain(..) {
            let id = view.attachment_identity();
            owner.entries.push(wire::ViewEntry {
                display: std::ffi::CString::new(
                    view.display().as_path().as_os_str().as_encoded_bytes(),
                )
                .map_err(|_| PhysicalRootError::Reservation)?,
                instance: view.instance().value(),
                device: id.device,
                inode: id.inode,
                mount: id.mount,
            });
            owner.slots.push(Slot {
                data: Some(data),
                ready: Some(view),
                attempt: None,
                inflight: None,
                pending: None,
            });
        }
        for slot in &mut owner.slots {
            slot.attempt = Some(
                slot.ready
                    .take()
                    .expect("armed ready view")
                    .begin(&owner.scope, &|| true)
                    .map_err(PhysicalRootError::mount)?,
            );
        }
        Ok(owner)
    }
    pub(super) fn request_cover(&self) -> &Arc<CoveredWorkspaceRequest> {
        &self.covered
    }
    pub(super) fn executor(&self) -> RootIdentity {
        self.executor
    }
    pub(super) fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.scope
    }
    pub(super) fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.resources
    }
    pub(in crate::runtime::repository_physical_root) fn entries(&self) -> &[wire::ViewEntry] {
        &self.entries
    }
    pub(in crate::runtime::repository_physical_root) fn bind_attempt(
        &mut self,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        check(&self.scope, &|| true)?;
        if self.binding.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        self.binding = Some(
            ViewUseBinding::new(self.covered.clone(), self.executor, *nonce, &self.resources)
                .map_err(PhysicalRootError::mount)?,
        );
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn transfer(
        &mut self,
        slot: usize,
        socket: BorrowedFd<'_>,
    ) -> Result<(), PhysicalRootError> {
        let binding = self
            .binding
            .as_ref()
            .ok_or(PhysicalRootError::Mode)?
            .clone();
        let row = self.slots.get_mut(slot).ok_or(PhysicalRootError::Mode)?;
        if row.inflight.is_some() || row.pending.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let next = row
            .attempt
            .as_mut()
            .ok_or(PhysicalRootError::Mode)?
            .transfer(socket, binding, slot, &self.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(row.attempt.take());
        row.inflight = Some(next);
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn stage(
        &mut self,
        slot: usize,
        attached: File,
        token: File,
        words: [u64; 8],
    ) -> Result<(), PhysicalRootError> {
        let row = self.slots.get_mut(slot).ok_or(PhysicalRootError::Mode)?;
        if row.pending.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let next = row
            .inflight
            .as_mut()
            .ok_or(PhysicalRootError::Mode)?
            .stage_successor(attached, token, words, &self.scope, &|| true)
            .map_err(PhysicalRootError::mount)?;
        drop(row.inflight.take());
        row.pending = Some(next);
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn complete(
        &mut self,
        capture: &super::super::process_owner::ReapedCapture,
    ) -> Result<RestoredProcessViews, PhysicalRootError> {
        self.binding
            .as_ref()
            .ok_or(PhysicalRootError::Mode)?
            .check_capture(capture)
            .map_err(PhysicalRootError::mount)?;
        for row in &self.slots {
            row.pending
                .as_ref()
                .ok_or(PhysicalRootError::Mode)?
                .check_completion(capture, &self.scope, &|| true)
                .map_err(PhysicalRootError::mount)?;
        }
        let mut restored = Vec::new();
        restored
            .try_reserve_exact(self.slots.len())
            .map_err(|_| PhysicalRootError::Resource)?;
        if restored.capacity() > self.slots.len() {
            return Err(PhysicalRootError::Resource);
        }
        for row in &mut self.slots {
            let view = row
                .pending
                .as_mut()
                .ok_or(PhysicalRootError::Mode)?
                .accept_after_reap(capture, &self.scope, &|| true)
                .map_err(PhysicalRootError::mount)?;
            drop(row.pending.take());
            restored.push((view, row.data.take().expect("owned selected data")));
        }
        Ok(RestoredProcessViews {
            pairs: restored,
            _credit: self._credit.clone(),
        })
    }
}
