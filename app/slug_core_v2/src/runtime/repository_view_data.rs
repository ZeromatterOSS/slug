//! Data-only coverage for immutable process-view projections. These owners may
//! follow native requests/history, but never contain mount/domain authority.
use slug_bzlmod_v2::RepositoryWorkspaceReadView;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;

use super::*;

#[derive(Debug)]
pub(in crate::runtime) struct NativeReadViewData {
    projection: RepositoryWorkspaceReadView,
    root: ReadViewRoot,
    _credit: ResourceCredit,
}
#[derive(Debug)]
enum ReadViewRoot {
    Workspace(Arc<RepositoryPhysicalRoot>),
    Source(Arc<source::RepositorySourceRoot>),
}
impl NativeReadViewData {
    pub(super) fn from_sealed(
        sealed: &SealedWorkspaceMaterialization,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let spec = &sealed.request().repo_spec;
        let bytes = add(
            arc_bytes::<Self>()?,
            spec.workspace_retained_storage_bytes()
                .map_err(PhysicalRootError::Protocol)?,
        )?;
        let credit = sealed.root().resources.reserve(bytes, 0, 0)?;
        let projection =
            RepositoryWorkspaceReadView::new(spec.clone(), sealed.root().state().clone())
                .map_err(PhysicalRootError::Protocol)?;
        Ok(Arc::new(Self {
            projection,
            root: ReadViewRoot::Workspace(sealed.root().clone()),
            _credit: credit,
        }))
    }
    pub(in crate::runtime) fn projection(&self) -> &RepositoryWorkspaceReadView {
        &self.projection
    }
    pub(in crate::runtime) fn from_source(
        sealed: &source::SealedSourceSnapshot,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let projection = RepositoryWorkspaceReadView::from_source(sealed.state().clone())
            .map_err(PhysicalRootError::Protocol)?;
        let bytes = add(
            arc_bytes::<Self>()?,
            projection
                .storage_partition()
                .map_err(PhysicalRootError::Protocol)?
                .owned_bytes(),
        )?;
        let credit = sealed.root().resources.reserve(bytes, 0, 0)?;
        Ok(Arc::new(Self {
            projection,
            root: ReadViewRoot::Source(sealed.root().clone()),
            _credit: credit,
        }))
    }
    pub(in crate::runtime) fn root(&self) -> Option<&Arc<RepositoryPhysicalRoot>> {
        match &self.root {
            ReadViewRoot::Workspace(root) => Some(root),
            _ => None,
        }
    }
    pub(in crate::runtime) fn source_root(&self) -> Option<&Arc<source::RepositorySourceRoot>> {
        match &self.root {
            ReadViewRoot::Source(root) => Some(root),
            _ => None,
        }
    }
    pub(super) fn identity(&self) -> RootIdentity {
        match &self.root {
            ReadViewRoot::Workspace(root) => root.identity,
            ReadViewRoot::Source(root) => root.identity,
        }
    }
    pub(super) fn same_state_allocation(&self, view: &RepositoryWorkspaceReadView) -> bool {
        match (
            self.projection.workspace_state(),
            view.workspace_state(),
            self.projection.source_state(),
            view.source_state(),
        ) {
            (Some(a), Some(b), None, None) => Arc::ptr_eq(a, b),
            (None, None, Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub(super) fn covers(&self, view: &RepositoryWorkspaceReadView) -> bool {
        self.same_state_allocation(view) && self.projection.repo_spec() == view.repo_spec()
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct NativeReadViewDependencies {
    owners: Vec<Arc<NativeReadViewData>>,
    max_owners: usize,
    _credit: ResourceCredit,
}
impl NativeReadViewDependencies {
    pub(super) fn empty(
        resources: &Arc<RepositoryRootResources>,
        max_owners: usize,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        Self::from_owners(Vec::new(), max_owners, resources)
    }
    pub(super) fn from_owners(
        owners: Vec<Arc<NativeReadViewData>>,
        max_owners: usize,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        if owners.len() > max_owners || owners.capacity() > max_owners {
            return Err(PhysicalRootError::Resource);
        }
        let bytes = add(
            arc_bytes::<Self>()?,
            owners
                .capacity()
                .checked_mul(std::mem::size_of::<Arc<NativeReadViewData>>())
                .ok_or(PhysicalRootError::Resource)?,
        )?;
        let credit = resources.reserve(bytes, 0, 0)?;
        Ok(Arc::new(Self {
            owners,
            max_owners,
            _credit: credit,
        }))
    }
    pub(super) fn merge(
        &self,
        additions: &[Arc<NativeReadViewData>],
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let capacity = self
            .owners
            .len()
            .checked_add(additions.len())
            .ok_or(PhysicalRootError::Resource)?
            .min(self.max_owners);
        let _peak = resources.reserve(
            add(
                256,
                capacity
                    .checked_mul(2 * std::mem::size_of::<Arc<NativeReadViewData>>())
                    .ok_or(PhysicalRootError::Resource)?,
            )?,
            0,
            0,
        )?;
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(capacity)
            .map_err(|_| PhysicalRootError::Resource)?;
        if owners.capacity() > capacity {
            return Err(PhysicalRootError::Resource);
        }
        for owner in self.owners.iter().chain(additions) {
            if owners.iter().any(|old| Arc::ptr_eq(old, owner)) {
                continue;
            }
            if owners.len() == capacity {
                return Err(PhysicalRootError::Resource);
            }
            if owners.iter().any(|old: &Arc<NativeReadViewData>| {
                old.projection.instance() == owner.projection.instance()
            }) {
                return Err(PhysicalRootError::Reservation);
            }
            owners.push(owner.clone());
        }
        Self::from_owners(owners, self.max_owners, resources)
    }
    pub(in crate::runtime) fn owners(&self) -> &[Arc<NativeReadViewData>] {
        &self.owners
    }
    pub(super) fn covers_request(&self, request: &RepositoryWorkspaceRequest) -> bool {
        let mut covered = true;
        request.visit_read_views(&mut |view| {
            covered &= self.owners.iter().any(|owner| owner.covers(view))
        });
        covered
    }
    pub(super) fn covers_state(&self, state: &RepositoryWorkspaceState) -> bool {
        state.source_bindings().iter().all(|view| {
            self.owners
                .iter()
                .any(|owner| owner.source_root().is_some() && owner.covers(view))
        })
    }
    pub(super) fn for_state(
        &self,
        state: &RepositoryWorkspaceState,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        self.retain_references(
            |visit| {
                for view in state.source_bindings() {
                    visit(view);
                }
            },
            resources,
        )
    }
    pub(super) fn covers_transcript(
        &self,
        transcript: &slug_bzlmod_v2::RepositoryWorkspaceTranscript,
    ) -> bool {
        let mut covered = true;
        transcript.visit_read_views(&mut |view| {
            covered &= self.owners.iter().any(|owner| owner.covers(view))
        });
        covered
    }
    /// Storage coverage only, never semantic authentication. Every incoming
    /// Spec/manifest allocation still needs its complete owned-partition credit.
    pub(super) fn covers_transcript_state_allocations(
        &self,
        transcript: &slug_bzlmod_v2::RepositoryWorkspaceTranscript,
    ) -> bool {
        let mut covered = true;
        transcript.visit_read_views(&mut |view| {
            covered &= self
                .owners
                .iter()
                .any(|owner| owner.same_state_allocation(view))
        });
        covered
    }
    pub(super) fn covers_epoch(&self, epoch: &slug_bzlmod_v2::RepositoryWorkspaceEpoch) -> bool {
        let mut covered = true;
        epoch.visit_read_views(&mut |view| {
            covered &= self.owners.iter().any(|owner| owner.covers(view))
        });
        covered
    }
    fn retain_references(
        &self,
        visit: impl FnOnce(&mut dyn FnMut(&RepositoryWorkspaceReadView)),
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let bytes = self
            .owners
            .len()
            .checked_mul(2 * std::mem::size_of::<Arc<NativeReadViewData>>())
            .ok_or(PhysicalRootError::Resource)?;
        let _peak = resources.reserve(add(bytes, 256)?, 0, 0)?;
        let mut owners = Vec::with_capacity(self.owners.len());
        let mut missing = false;
        visit(
            &mut |view| match self.owners.iter().find(|owner| owner.covers(view)) {
                Some(owner) => {
                    if !owners.iter().any(|old| Arc::ptr_eq(old, owner)) {
                        owners.push(owner.clone());
                    }
                }
                None => missing = true,
            },
        );
        if missing {
            return Err(PhysicalRootError::Reservation);
        }
        Self::from_owners(owners, self.max_owners, resources)
    }
    pub(super) fn for_request(
        &self,
        request: &RepositoryWorkspaceRequest,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        self.retain_references(
            |visit| request.visit_read_views(&mut |view| visit(view)),
            resources,
        )
    }
    pub(super) fn for_transcript(
        &self,
        transcript: &slug_bzlmod_v2::RepositoryWorkspaceTranscript,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        self.retain_references(
            |visit| transcript.visit_read_views(&mut |view| visit(view)),
            resources,
        )
    }
}

/// A raw request can use the owned-only storage partition only after its exact
/// canonical source projections have independent data-root lifetime coverage.
#[derive(Debug)]
pub(in crate::runtime::repository_physical_root) struct CoveredWorkspaceRequest {
    request: Arc<RepositoryWorkspaceRequest>,
    dependencies: Arc<NativeReadViewDependencies>,
    owned_bytes: usize,
    body_candidates: Option<NativeBodyCandidates>,
    _credit: ResourceCredit,
}
impl std::ops::Deref for CoveredWorkspaceRequest {
    type Target = RepositoryWorkspaceRequest;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}
impl CoveredWorkspaceRequest {
    pub(super) fn from_canonical(
        request: RepositoryWorkspaceRequest,
        dependencies: Arc<NativeReadViewDependencies>,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        Self::from_canonical_inner(request, dependencies, resources, None, None, &|| true)
    }
    pub(super) fn from_canonical_with_bodies(
        request: RepositoryWorkspaceRequest,
        dependencies: Arc<NativeReadViewDependencies>,
        resources: &Arc<RepositoryRootResources>,
        bodies: NativeBodyCandidates,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        Self::from_canonical_inner(
            request,
            dependencies,
            resources,
            Some(bodies),
            Some(scope),
            active,
        )
    }
    fn from_canonical_inner(
        request: RepositoryWorkspaceRequest,
        dependencies: Arc<NativeReadViewDependencies>,
        resources: &Arc<RepositoryRootResources>,
        bodies: Option<NativeBodyCandidates>,
        scope: Option<&Arc<SourceServiceScope>>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        struct Incoming {
            request: RepositoryWorkspaceRequest,
            dependencies: Arc<NativeReadViewDependencies>,
            bodies: Option<NativeBodyCandidates>,
        }
        let mut incoming = Incoming {
            request,
            dependencies,
            bodies,
        };
        if !incoming.dependencies.covers_request(&incoming.request) {
            return Err(PhysicalRootError::Reservation);
        }
        let dependencies = incoming
            .dependencies
            .for_request(&incoming.request, resources)?;
        let storage = if let Some(bodies) = &mut incoming.bodies {
            bodies.ledger().check_request_origin(
                resources,
                scope.ok_or(PhysicalRootError::Reservation)?,
                incoming.request.reservation(),
                active,
            )?;
            incoming.request.try_visit_body_allocations(&mut |body| {
                bodies.retain(body, active)?;
                Ok::<_, PhysicalRootError>(())
            })?;
            incoming.request.body_covered_storage_partition()
        } else {
            incoming.request.storage_partition()
        };
        let owned_bytes = storage.map_err(PhysicalRootError::Protocol)?.owned_bytes();
        let credit = resources.reserve(
            add(
                add(
                    arc_bytes::<Self>()?,
                    arc_bytes::<RepositoryWorkspaceRequest>()?,
                )?,
                owned_bytes,
            )?,
            0,
            0,
        )?;
        Ok(Arc::new(Self {
            request: Arc::new(incoming.request),
            dependencies,
            owned_bytes,
            body_candidates: incoming.bodies,
            _credit: credit,
        }))
    }
    pub(super) fn empty(
        request: RepositoryWorkspaceRequest,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let mut any = false;
        request.visit_read_views(&mut |_| any = true);
        if any {
            return Err(PhysicalRootError::Reservation);
        }
        Self::from_canonical(
            request,
            NativeReadViewDependencies::empty(resources, 0)?,
            resources,
        )
    }
    pub(super) fn request(&self) -> &RepositoryWorkspaceRequest {
        &self.request
    }
    pub(super) fn request_owner(&self) -> Arc<RepositoryWorkspaceRequest> {
        self.request.clone()
    }
    pub(super) fn owned_bytes(&self) -> usize {
        self.owned_bytes
    }
    pub(super) fn dependencies(&self) -> &Arc<NativeReadViewDependencies> {
        &self.dependencies
    }
    pub(super) fn body_candidates(&self) -> Option<&NativeBodyCandidates> {
        self.body_candidates.as_ref()
    }
}

/// Full native terminal request owners share this guard. Its data dependencies
/// have no edge to request histories or setup controllers.
#[derive(Debug)]
pub(in crate::runtime) struct NativeWorkspaceHistoryGuard {
    _dependencies: Arc<NativeReadViewDependencies>,
    _bodies: Option<Arc<NativeBodyLedger>>,
    _credit: ResourceCredit,
}
impl NativeWorkspaceHistoryGuard {
    pub(super) fn new(
        dependencies: Arc<NativeReadViewDependencies>,
        credit: ResourceCredit,
    ) -> Arc<Self> {
        Arc::new(Self {
            _dependencies: dependencies,
            _bodies: None,
            _credit: credit,
        })
    }
    pub(super) fn new_with_bodies(
        dependencies: Arc<NativeReadViewDependencies>,
        bodies: Arc<NativeBodyLedger>,
        credit: ResourceCredit,
    ) -> Arc<Self> {
        Arc::new(Self {
            _dependencies: dependencies,
            _bodies: Some(bodies),
            _credit: credit,
        })
    }
}
