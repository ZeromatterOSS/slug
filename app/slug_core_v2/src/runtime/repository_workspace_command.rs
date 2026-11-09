//! Exclusive native command owner. Immutable epochs project to DICE; neither a
//! projected old epoch nor an individual root can mint process admission. Taking
//! the whole command prevents concurrent roots spending the same history budget.
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::super::mount_lease::InitialRepositoryMount;
use super::super::mount_lease::RepositoryMountLease;
use super::super::mount_lease::domain::NativeMountDomain;
use super::super::mount_lease::quota::VerifiedProcessQuota;
use super::*;

struct RetirementState {
    owners: std::sync::Mutex<usize>,
    changed: std::sync::Condvar,
    _credit: resources::ResourceCredit,
}
#[derive(Clone)]
pub(in crate::runtime) struct RepositoryWorkspaceRetirementObserver {
    state: Arc<RetirementState>,
}
pub(in crate::runtime::repository_physical_root) struct RepositoryWorkspaceRetirementGuard {
    state: Arc<RetirementState>,
}
#[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
#[derive(Clone)]
pub(in crate::runtime::repository_physical_root) struct RepositoryWorkspaceProcessProbe {
    pub(in crate::runtime::repository_physical_root) stage: Arc<std::sync::atomic::AtomicUsize>,
    pub(in crate::runtime::repository_physical_root) hold_before_hello: bool,
}
impl std::fmt::Debug for RepositoryWorkspaceRetirementGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryWorkspaceRetirementGuard")
            .finish_non_exhaustive()
    }
}
impl RepositoryWorkspaceRetirementObserver {
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) fn owner_count_for_test(&self) -> usize {
        *self
            .state
            .owners
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
    pub(in crate::runtime::repository_physical_root) fn worker_guard(
        &self,
    ) -> RepositoryWorkspaceRetirementGuard {
        let mut owners = self
            .state
            .owners
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *owners = owners
            .checked_add(1)
            .expect("bounded native retirement ownership");
        RepositoryWorkspaceRetirementGuard {
            state: self.state.clone(),
        }
    }
    /// Zero means all command fields and worker/result owners have retired.
    /// A quarantine timeout leaves the enclosing native session Busy.
    pub(in crate::runtime) fn wait_until(&self, deadline: std::time::Instant) -> bool {
        let mut owners = self
            .state
            .owners
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while *owners != 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, _) = self
                .state
                .changed
                .wait_timeout(owners, remaining)
                .unwrap_or_else(|poison| poison.into_inner());
            owners = next;
        }
        true
    }
}
impl Drop for RepositoryWorkspaceRetirementGuard {
    fn drop(&mut self) {
        let mut owners = self
            .state
            .owners
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *owners = owners
            .checked_sub(1)
            .expect("native retirement owner underflow");
        if *owners == 0 {
            self.state.changed.notify_all();
        }
    }
}

struct CreditedEpoch {
    epoch: RepositoryWorkspaceEpoch,
    _credit: resources::ResourceCredit,
}
struct NativeRootSlot {
    reservation: Arc<RepositoryWorkspaceReservation>,
    progress: Option<RepositoryWorkspaceProgress>,
    initial: Option<InitialRepositoryMount>,
    lease: Option<RepositoryMountLease>,
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    process_view: Option<selected::RetainedProcessView>,
    // Survives removal of mutable progress until the semantic epoch retires.
    body_ledger: Arc<NativeBodyLedger>,
}

struct NativeSourceSlot {
    reservation: Arc<slug_bzlmod_v2::RepositorySourceSnapshotReservation>,
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    process_view: Option<selected::RetainedProcessView>,
}

/// Created fresh once by the native command lifecycle, never from an imported
/// epoch. This is unfinished command state, not a second materialization registry.
/// No Clone, root FD, generic mutation callback or child result injection exists.
pub(in crate::runtime) struct RepositoryWorkspaceCommand {
    // Drop semantic epoch aliases before root-local allocation leases.
    epoch: CreditedEpoch,
    roots: Vec<NativeRootSlot>,
    source_slots: Vec<NativeSourceSlot>,
    mount_domain: Option<Arc<NativeMountDomain>>,
    // Semantic epoch payloads retire before the source-data lifetime union.
    source_dependencies: Arc<NativeReadViewDependencies>,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    max_roots: usize,
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    process_probe: Option<RepositoryWorkspaceProcessProbe>,
    _control: resources::ResourceCredit,
    // LAST: a drain cannot report completion before authority/credits above.
    _retirement: RepositoryWorkspaceRetirementGuard,
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
        check_bank(&scope, &resources)?;
        if &workspace != scope.workspace() {
            return Err(PhysicalRootError::SourceService(
                SourceServiceError::ForeignScope,
            ));
        }
        let control = resources.reserve(std::mem::size_of::<Self>(), 0, 0)?;
        let retirement_credit = resources.reserve(arc_bytes::<RetirementState>()?, 0, 0)?;
        let retirement = RepositoryWorkspaceRetirementGuard {
            state: Arc::new(RetirementState {
                owners: std::sync::Mutex::new(1),
                changed: std::sync::Condvar::new(),
                _credit: retirement_credit,
            }),
        };
        let bytes = RepositoryWorkspaceEpoch::fresh_storage_bytes(&workspace, lease, limits)
            .map_err(PhysicalRootError::Protocol)?;
        let credit = resources.reserve(add(bytes, std::mem::size_of::<CreditedEpoch>())?, 0, 0)?;
        let epoch = RepositoryWorkspaceEpoch::fresh(workspace, lease, limits)
            .map_err(PhysicalRootError::Protocol)?;
        let dependencies = NativeReadViewDependencies::empty(&resources, limits.max_roots())?;
        Ok(Self {
            roots: Vec::new(),
            source_slots: Vec::new(),
            mount_domain: None,
            epoch: CreditedEpoch {
                epoch,
                _credit: credit,
            },
            source_dependencies: dependencies,
            scope,
            resources,
            max_roots: limits.max_roots(),
            #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
            process_probe: None,
            _control: control,
            _retirement: retirement,
        })
    }
    pub(in crate::runtime) fn retirement_observer(&self) -> RepositoryWorkspaceRetirementObserver {
        RepositoryWorkspaceRetirementObserver {
            state: self._retirement.state.clone(),
        }
    }
    /// Pure complete-frontier preadmission. Future snapshots remain actual
    /// per-operation work; this validates every currently knowable replay fact.
    pub(in crate::runtime) fn validate_frontier_request(
        &self,
        request: &RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
        comparison: &mut ViewComparison<'_>,
    ) -> Result<(), PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slot = self.root_slot(request.reservation())?;
        if self.roots[slot].progress.is_none() {
            return Err(PhysicalRootError::Reservation);
        }
        comparison.prepare_workspace_coverage()?;
        let mut bodies = self.request_body_candidates(request, scope, active)?;
        let storage = request
            .body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let bytes = if self.source_dependencies.covers_request(request) {
            storage.owned_bytes()
        } else {
            storage.total_bytes().map_err(PhysicalRootError::Protocol)?
        };
        struct ExpectedRequest {
            request: RepositoryWorkspaceRequest,
        }
        let mut credit =
            self.resources
                .reserve(add(bytes, std::mem::size_of::<ExpectedRequest>())?, 0, 0)?;
        let expected = ExpectedRequest {
            request: self
                .epoch()
                .request_at(
                    request.reservation(),
                    request.index(),
                    request.operation().clone(),
                )
                .map_err(PhysicalRootError::Protocol)?,
        };
        expected
            .request
            .try_visit_body_allocations(&mut |body| bodies.retain(body, active).map(|_| ()))?;
        let expected_storage = expected
            .request
            .body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let expected_bytes = body_admission::partition_bytes(
            expected_storage,
            self.source_dependencies.covers_request(&expected.request),
        )?;
        credit
            .bytes
            .try_grow(expected_bytes)
            .map_err(|_| PhysicalRootError::Resource)?;
        // The caller's memo can outlive this borrowed request validation.
        comparison.hold_workspace_coverage(bodies, self.source_dependencies.clone(), credit);
        if !request.try_eq_with_states(&expected.request, &mut |left, right| {
            comparison.state_ref_equal(left, right)
        })? {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        let transcript = self
            .epoch()
            .transcripts()
            .unwrap()
            .iter()
            .find(|transcript| transcript.reservation().as_ref() == request.reservation().as_ref())
            .ok_or(PhysicalRootError::Reservation)?;
        if let Some(record) = transcript.records().get(request.index()) {
            if !request.try_eq_with_states(record.request(), &mut |left, right| {
                comparison.state_ref_equal(left, right)
            })? {
                return Err(PhysicalRootError::Protocol(
                    slug_bzlmod_v2::RepositoryWorkspaceError::ReplayConflict,
                ));
            }
        }
        Ok(())
    }
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime) fn with_process_probe_for_test(
        mut self,
        stage: Arc<std::sync::atomic::AtomicUsize>,
        hold_before_hello: bool,
    ) -> Self {
        self.process_probe = Some(RepositoryWorkspaceProcessProbe {
            stage,
            hold_before_hello,
        });
        self
    }
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn process_probe(
        &self,
    ) -> Option<RepositoryWorkspaceProcessProbe> {
        self.process_probe.clone()
    }
    pub(in crate::runtime) fn epoch(&self) -> &RepositoryWorkspaceEpoch {
        &self.epoch.epoch
    }
    /// Component tests select only data already owned by an actual command.
    /// This mints no Source root, result, controller or current actor authority.
    #[cfg(test)]
    pub(in crate::runtime::repository_physical_root) fn source_link_set_for_test(
        &self,
        views: &[slug_bzlmod_v2::RepositoryWorkspaceReadView],
        scope: &Arc<SourceServiceScope>,
    ) -> Result<Arc<SourceLinkSet>, PhysicalRootError> {
        SourceLinkSet::from_views(
            &self.source_dependencies,
            views,
            &self.resources,
            scope,
            &|| true,
        )
    }
    pub(in crate::runtime::repository_physical_root) fn mount_domain(
        &self,
    ) -> Option<&Arc<NativeMountDomain>> {
        self.mount_domain.as_ref()
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
    /// Refuse foreign initialization before creating a helper or backing root.
    pub(in crate::runtime::repository_physical_root) fn check_initialization(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        claim: &crate::runtime::repository_io::reservations::SourceRootClaim,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.check_scope(scope, active)?;
        if !Arc::ptr_eq(&self.resources, resources)
            || !claim.belongs_to(resources)
            || !claim.matches_reservation(reservation)
        {
            return Err(PhysicalRootError::Reservation);
        }
        // Source snapshots occupy the same native root capacity but have no
        // General transcript. Refuse before bootstrap, independently of the
        // metadata-derived history budget checked below.
        if self
            .roots
            .len()
            .checked_add(self.source_slots.len())
            .ok_or(PhysicalRootError::Resource)?
            >= self.max_roots
        {
            return Err(PhysicalRootError::Resource);
        }
        // Initial root mode changes no storage forecast. Actual root metadata
        // and the same admission are verified again when ownership returns.
        self.check_epoch_body_coverage(scope, active)?;
        self.epoch()
            .reserve_body_covered_storage_partition(reservation, 0o755)
            .map_err(PhysicalRootError::Protocol)?
            .owned_bytes();
        Ok(())
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
        if self
            .roots
            .len()
            .checked_add(self.source_slots.len())
            .ok_or(PhysicalRootError::Resource)?
            >= self.max_roots
        {
            return Err(PhysicalRootError::Resource);
        }
        if !Arc::ptr_eq(&self.resources, &builder.resources) {
            return Err(PhysicalRootError::Resource);
        }
        let _scratch = self.resources.reserve(IO_SCRATCH, 0, 0)?;
        initial
            .check_binding(&builder, scope, active)
            .map_err(PhysicalRootError::mount)?;
        let domain = match &self.mount_domain {
            Some(domain) => {
                domain
                    .check_root(&builder.reservation, &self.resources, scope, active)
                    .map_err(PhysicalRootError::mount)?;
                domain
                    .check_initial(&builder, &initial, scope, active)
                    .map_err(PhysicalRootError::mount)?;
                domain.clone()
            }
            None => NativeMountDomain::from_initial(&builder, &initial, scope, active)
                .map_err(PhysicalRootError::mount)?,
        };
        let reservation = builder.reservation.clone();
        let root_mode = (RootIo::new(&builder.file, io_limits(builder.mode, &reservation))?
            .root_stat()?
            .st_mode
            & 0o7777) as u16;
        self.check_epoch_body_coverage(scope, active)?;
        let forecast = self
            .epoch()
            .reserve_body_covered_storage_partition(&reservation, root_mode)
            .map_err(PhysicalRootError::Protocol)?
            .owned_bytes();
        let credit =
            self.resources
                .reserve(add(forecast, std::mem::size_of::<CreditedEpoch>())?, 0, 0)?;
        let epoch = self
            .epoch()
            .reserve(reservation.clone(), root_mode)
            .map_err(PhysicalRootError::Protocol)?;
        let progress = RepositoryWorkspaceProgress::new(builder, scope, active)?;
        let body_ledger = progress.body_ledger().clone();
        self.grow_roots()?;
        self.roots.push(NativeRootSlot {
            reservation,
            progress: Some(progress),
            initial: Some(initial),
            lease: None,
            #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
            process_view: None,
            body_ledger,
        });
        self.epoch = CreditedEpoch {
            epoch,
            _credit: credit,
        };
        self.mount_domain = Some(domain);
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
        if !self.source_dependencies.covers_epoch(self.epoch())
            || !self
                .source_dependencies
                .covers_transcript(progress.transcript())
        {
            return Err(PhysicalRootError::Reservation);
        }
        self.check_epoch_body_coverage(&self.scope, &|| true)?;
        self.check_transcript_body_coverage(progress.transcript(), &self.scope, &|| true)?;
        let forecast = self
            .epoch()
            .publish_body_covered_storage_partition(progress.transcript())
            .map_err(PhysicalRootError::Protocol)?
            .owned_bytes();
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
        self,
        request: RepositoryWorkspaceRequest,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        let sources = SourceLinkSet::empty(&self.resources, scope, active)?;
        self.apply_with_sources(request, sources, runtime, distdirs, scope, active)
    }
    pub(in crate::runtime) fn apply_with_sources(
        mut self,
        request: RepositoryWorkspaceRequest,
        sources: Arc<SourceLinkSet>,
        runtime: &tokio::runtime::Runtime,
        distdirs: &crate::runtime::ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let mut bodies = self.request_body_candidates(&request, scope, active)?;
        let partition = request
            .body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        let bytes = if self.source_dependencies.covers_request(&request) {
            partition.owned_bytes()
        } else {
            partition
                .total_bytes()
                .map_err(PhysicalRootError::Protocol)?
        };
        // Guards precede native canonicalization and the comparison memo's
        // foreign Arc clones. Raw semantic owners retire before these guards.
        let _incoming_credit = self.resources.reserve(
            add(bytes, std::mem::size_of::<RepositoryWorkspaceRequest>())?,
            0,
            0,
        )?;
        let incoming = sources.canonicalize_request(request, scope, active)?;
        incoming.try_visit_body_allocations(&mut |body| bodies.retain(body, active).map(|_| ()))?;
        let _expected_credit = self.resources.reserve(
            add(
                partition.owned_bytes(),
                std::mem::size_of::<RepositoryWorkspaceRequest>(),
            )?,
            0,
            0,
        )?;
        let expected = self
            .epoch()
            .request_at(
                incoming.reservation(),
                incoming.index(),
                incoming.operation().clone(),
            )
            .map_err(PhysicalRootError::Protocol)?;
        let mut comparison = ViewComparison::new(&self.resources, scope, active)?;
        if !incoming.try_eq_with_states(&expected, &mut |a, b| comparison.state_ref_equal(a, b))? {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        // The memo's foreign aliases must retire before temporary candidates
        // can be consumed by snapshot completion and canonical survivor commit.
        drop(comparison);
        drop(incoming);
        let request = expected;
        if !sources.dependencies().covers_state(request.predecessor()) {
            return Err(PhysicalRootError::Reservation);
        }
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
        self.source_dependencies = self
            .source_dependencies
            .merge(sources.dependencies().owners(), &self.resources)?;
        let mut progress = self.roots[slot].progress.take().unwrap();
        progress.admit_link_sources(sources)?;
        let progress = if matches!(
            request.operation(),
            RepositoryWorkspaceOperation::Archive(_)
        ) {
            progress
                .apply_archive_with_candidates(request, bodies, runtime, distdirs, scope, active)?
        } else {
            progress.apply_with_candidates(request, bodies, scope, active)?
        };
        if let Some(lease) = self.roots[slot].lease.take() {
            self.roots[slot].lease = Some(
                lease
                    .advance_trusted(&progress, scope, active)
                    .map_err(PhysicalRootError::mount)?,
            );
        }
        self.roots[slot].progress = Some(progress);
        self.publish(slot)?;
        Ok(self)
    }
    /// No externally supplied epoch can mint this capability. Moving the whole
    /// command prevents another root from spending its Process-domain history
    /// budget while setup or a child is in flight.
    pub(in crate::runtime::repository_physical_root) fn admit_first_execute(
        self,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeProcessAdmission, PhysicalRootError> {
        let mut any = false;
        request.visit_read_views(&mut |_| any = true);
        if any {
            return Err(PhysicalRootError::Reservation);
        }
        self.admit_native_execute(request, RepositoryWorkspaceMode::Archive, scope, active)
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn admit_execute(
        self,
        request: RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<super::super::mount_lease::launch::ExecuteDecision, PhysicalRootError> {
        self.check_scope(scope, active)?;
        if let RepositoryWorkspaceOperation::Execute(execute) = request.operation() {
            if !execute.read_views().views().is_empty() {
                return Err(PhysicalRootError::Reservation);
            }
        }
        let slot = self.root_slot(request.reservation())?;
        self.compare_transcript(slot)?;
        let progress = self.roots[slot].progress.as_ref().unwrap();
        if progress
            .transcript()
            .replay(&request)
            .map_err(PhysicalRootError::Protocol)?
            .is_some()
        {
            return Ok(super::super::mount_lease::launch::ExecuteDecision::Replay(
                self,
            ));
        }
        let mode = progress.builder.mode;
        let admission = self.admit_native_execute(request, mode, scope, active)?;
        Ok(super::super::mount_lease::launch::ExecuteDecision::Launch(
            super::super::mount_lease::launch::ExecuteLaunch::new(admission, active)?,
        ))
    }
    fn admit_native_execute(
        mut self,
        request: RepositoryWorkspaceRequest,
        mode: RepositoryWorkspaceMode,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeProcessAdmission, PhysicalRootError> {
        self.check_scope(scope, active)?;
        let bodies = self.request_body_candidates(&request, scope, active)?;
        let covered = CoveredWorkspaceRequest::from_canonical_with_bodies(
            request,
            self.source_dependencies.clone(),
            &self.resources,
            bodies,
            scope,
            active,
        )?;
        self.admit_native_covered(covered, mode, scope, active)
    }
    fn admit_native_covered(
        mut self,
        covered: Arc<CoveredWorkspaceRequest>,
        mode: RepositoryWorkspaceMode,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeProcessAdmission, PhysicalRootError> {
        let request = covered.request();
        self.check_scope(scope, active)?;
        let slot = self.root_slot(request.reservation())?;
        self.compare_transcript(slot)?;
        if let Some(bodies) = covered.body_candidates() {
            bodies.ledger().check_request_origin(
                &self.resources,
                scope,
                request.reservation(),
                active,
            )?;
            request.try_visit_body_allocations(&mut |body| {
                bodies
                    .lookup(body, active)?
                    .ok_or(PhysicalRootError::Reservation)?;
                Ok::<(), PhysicalRootError>(())
            })?;
        }
        let root = self.roots[slot].progress.as_ref().unwrap();
        if root.builder.mode != mode
            || root.transcript().current().mode() != mode
            || match mode {
                RepositoryWorkspaceMode::Archive => {
                    self.roots[slot].initial.is_none() || self.roots[slot].lease.is_some()
                }
                RepositoryWorkspaceMode::Process => {
                    self.roots[slot].lease.is_none() || self.roots[slot].initial.is_some()
                }
            }
            || request.index() != root.transcript().records().len()
            || !matches!(
                request.operation(),
                RepositoryWorkspaceOperation::Execute(_)
            )
        {
            return Err(PhysicalRootError::Mode);
        }
        if !self.source_dependencies.covers_request(&request) {
            return Err(PhysicalRootError::Reservation);
        }
        let credit = self.resources.reserve(
            add(
                arc_bytes::<RepositoryWorkspaceRequest>()?,
                add(
                    std::mem::size_of::<NativeProcessAdmission>(),
                    covered.owned_bytes(),
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
        if expected != *request {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        drop(expected);
        let _scratch = self.resources.reserve(IO_SCRATCH, 3, 0)?;
        if backing_identity(&root.builder.file, mode)? != root.builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        let io = RootIo::new(
            &root.builder.file,
            io_limits(RepositoryWorkspaceMode::Process, &root.builder.reservation),
        )?;
        seal::verify(&io, root.transcript().current(), scope, active)?;
        if backing_identity(&root.builder.file, mode)? != root.builder.identity {
            return Err(PhysicalRootError::RootIdentity);
        }
        check(scope, active)?;
        let selected = self.roots[slot].progress.take().unwrap();
        let initial = self.roots[slot].initial.take();
        let lease = self.roots[slot].lease.take();
        Ok(NativeProcessAdmission {
            selected,
            initial,
            lease,
            request: covered,
            slot,
            command: self,
            quota_confirmed: mode == RepositoryWorkspaceMode::Process,
            _credit: credit,
        })
    }
    /// A metadata-only success or matching final tree cannot adopt a root.
    /// Match the entire exact selected history before retiring mutable authority.
    pub(in crate::runtime) fn seal_for_materialization(
        mut self,
        request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(Self, super::super::SealedWorkspaceMaterialization), PhysicalRootError> {
        self.check_scope(scope, active)?;
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(expected) =
            &request.kind
        else {
            return Err(PhysicalRootError::Reservation);
        };
        let reservation = expected.reservation().clone();
        if &request.id.workspace != reservation.workspace()
            || &request.id.canonical_repo != reservation.repository()
        {
            return Err(PhysicalRootError::Reservation);
        }
        let slot = self.root_slot(&reservation)?;
        self.compare_transcript(slot)?;
        let progress = self.roots[slot]
            .progress
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        let resources = progress.builder.resources.clone();
        let bodies = self.materialization_body_candidates(&request, scope, active)?;
        let storage = request
            .general_workspace_body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?;
        // Pointer coverage proves only storage lifetime, not Spec/history equality.
        let incoming_bytes = if self
            .source_dependencies
            .covers_transcript_state_allocations(expected.transcript())
        {
            storage.owned_bytes()
        } else {
            storage.total_bytes().map_err(PhysicalRootError::Protocol)?
        };
        struct CreditedRequest {
            request: Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>,
            bodies: NativeBodyCandidates,
            credit: ResourceCredit,
        }
        let incoming = CreditedRequest {
            request,
            bodies,
            credit: resources.reserve(incoming_bytes, 0, 0)?,
        };
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(expected) =
            &incoming.request.kind
        else {
            unreachable!()
        };
        if !ViewComparison::new(&resources, scope, active)?
            .transcript_equal(expected.transcript(), progress.transcript())?
        {
            return Err(PhysicalRootError::Protocol(
                slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor,
            ));
        }
        if !self
            .source_dependencies
            .covers_transcript(progress.transcript())
        {
            return Err(PhysicalRootError::Reservation);
        }
        self.check_transcript_body_coverage(progress.transcript(), scope, active)?;
        let canonical_bytes = add(
            progress
                .transcript()
                .realization_body_covered_storage_partition()
                .map_err(PhysicalRootError::Protocol)?
                .owned_bytes(),
            add(
                incoming
                    .request
                    .repo_spec
                    .workspace_retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
                add(
                    arc_bytes::<slug_bzlmod_v2::RepositoryMaterializationRequest>()?,
                    arc_bytes::<NativeWorkspaceHistoryGuard>()?,
                )?,
            )?,
        )?;
        let _construction = resources.reserve(canonical_bytes, 0, 0)?;
        let canonical = progress
            .transcript()
            .realize(progress.transcript().records().len())
            .map_err(PhysicalRootError::Protocol)?;
        let request = Arc::new(slug_bzlmod_v2::RepositoryMaterializationRequest {
            id: incoming.request.id.clone(),
            repo_spec: incoming.request.repo_spec.clone(),
            kind: slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(canonical),
        });
        let owned = request
            .general_workspace_body_covered_storage_partition()
            .map_err(PhysicalRootError::Protocol)?
            .owned_bytes();
        let credit = resources.reserve(
            add(
                owned,
                add(
                    arc_bytes::<NativeWorkspaceHistoryGuard>()?,
                    std::mem::size_of::<super::super::SealedWorkspaceMaterialization>(),
                )?,
            )?,
            0,
            0,
        )?;
        let bodies = self.materialization_body_candidates(&request, scope, active)?;
        let retained = CreditedRequest {
            request,
            bodies,
            credit,
        };
        drop(incoming);
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(expected) =
            &retained.request.kind
        else {
            unreachable!()
        };
        drop(self.roots[slot].initial.take());
        drop(self.roots[slot].lease.take());
        let progress = self.roots[slot]
            .progress
            .take()
            .ok_or(PhysicalRootError::Reservation)?;
        let dependencies = self
            .source_dependencies
            .for_transcript(expected.transcript(), &self.resources)?;
        let root = progress.seal_realization(expected, scope, active)?;
        let sealed = super::super::SealedWorkspaceMaterialization::from_command(
            retained.request,
            root,
            NativeWorkspaceHistoryGuard::new_with_bodies(
                dependencies,
                retained.bodies.ledger().clone(),
                retained.credit,
            ),
        );
        Ok((self, sealed))
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
        drop(self.roots[slot].lease.take());
        let root = self.roots[slot]
            .progress
            .take()
            .unwrap()
            .seal(consumed, scope, active)?;
        Ok((self, root))
    }
}

#[path = "repository_workspace_body_admission.rs"]
mod body_admission;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_workspace_finalize.rs"]
pub(in crate::runtime::repository_physical_root) mod finalize;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_workspace_process_views.rs"]
pub(in crate::runtime::repository_physical_root) mod selected;

pub(in crate::runtime::repository_physical_root) struct NativeProcessAdmission {
    selected: RepositoryWorkspaceProgress,
    initial: Option<InitialRepositoryMount>,
    lease: Option<RepositoryMountLease>,
    request: Arc<CoveredWorkspaceRequest>,
    slot: usize,
    command: RepositoryWorkspaceCommand,
    quota_confirmed: bool,
    _credit: resources::ResourceCredit,
}
impl NativeProcessAdmission {
    #[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn process_probe(
        &self,
    ) -> Option<RepositoryWorkspaceProcessProbe> {
        self.command.process_probe()
    }
    pub(in crate::runtime::repository_physical_root) fn retirement_observer(
        &self,
    ) -> RepositoryWorkspaceRetirementObserver {
        self.command.retirement_observer()
    }
    pub(in crate::runtime::repository_physical_root) fn begin_rolling(
        &mut self,
        active: &dyn Fn() -> bool,
    ) -> Result<super::super::mount_lease::RepositoryMountAttempt, PhysicalRootError> {
        if !self.quota_confirmed || self.initial.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        self.lease
            .take()
            .ok_or(PhysicalRootError::Mode)?
            .begin_covered(
                &self.selected.builder,
                self.request.clone(),
                &self.command.scope,
                active,
            )
            .map_err(PhysicalRootError::mount)
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn finish_reaped(
        mut self,
        pending: super::super::mount_lease::PendingMountLease,
        capture: super::super::process_owner::ReapedCapture,
    ) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
        if !self.quota_confirmed || self.initial.is_some() || self.lease.is_some() {
            return Err(PhysicalRootError::Mode);
        }
        let scope = self.command.scope.clone();
        check(&scope, &|| true)?;
        capture
            .binding()
            .verify(&self.request, self.selected.builder.identity)?;
        let (result, reap_binding, output_credit) = capture.into_result();
        let progress = self.selected.complete_snapshot_covered(
            self.request.clone(),
            RepositoryWorkspaceResult::Execute(result),
            &scope,
            &|| true,
        )?;
        let lease = pending
            .accept(
                super::super::mount_lease::QuiescedMountCompletion::from_reaped_record(
                    &progress,
                    &reap_binding,
                )?,
                &scope,
                &|| true,
            )
            .map_err(PhysicalRootError::mount)?;
        self.command.roots[self.slot].progress = Some(progress);
        self.command.roots[self.slot].lease = Some(lease);
        self.command.publish(self.slot)?;
        drop(output_credit);
        Ok(self.command)
    }
    pub(in crate::runtime::repository_physical_root) fn builder(
        &self,
    ) -> &RepositoryPhysicalRootBuilder {
        &self.selected.builder
    }
    pub(in crate::runtime::repository_physical_root) fn request(
        &self,
    ) -> &RepositoryWorkspaceRequest {
        self.request.request()
    }
    pub(in crate::runtime::repository_physical_root) fn request_owner(
        &self,
    ) -> Arc<RepositoryWorkspaceRequest> {
        self.request.request_owner()
    }
    pub(in crate::runtime::repository_physical_root) fn covered_request_owner(
        &self,
    ) -> Arc<CoveredWorkspaceRequest> {
        self.request.clone()
    }
    pub(in crate::runtime::repository_physical_root) fn request_cover(
        &self,
    ) -> Arc<CoveredWorkspaceRequest> {
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
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(in crate::runtime::repository_physical_root) fn finish_reaped(
        self,
        pending: super::super::mount_lease::PendingMountLease,
        capture: super::super::process_owner::ReapedCapture,
    ) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
        self.admission.finish_reaped(pending, capture)
    }
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

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[path = "repository_workspace_source.rs"]
mod source;
