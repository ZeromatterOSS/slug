//! Native lifetime admission. Byte credits reuse the existing descriptor bank;
//! descriptor counts and virtual tmpfs allowance are separate finite counters.
use std::sync::Mutex;

use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) struct RepositoryRootResourceLimits {
    retained_bytes: usize,
    native_fds: usize,
    backing_bytes: u64,
}
impl RepositoryRootResourceLimits {
    pub fn new(
        retained_bytes: usize,
        native_fds: usize,
        backing_bytes: u64,
    ) -> Result<Self, PhysicalRootError> {
        if retained_bytes == 0
            || retained_bytes > isize::MAX as usize
            || native_fds == 0
            || native_fds > 4096
            || backing_bytes == 0
        {
            return Err(PhysicalRootError::Resource);
        }
        Ok(Self {
            retained_bytes,
            native_fds,
            backing_bytes,
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(in crate::runtime) struct RepositoryRootResourceSnapshot {
    pub retained_bytes: usize,
    pub peak_retained_bytes: usize,
    pub native_fds: usize,
    pub peak_native_fds: usize,
    pub backing_bytes: u64,
    pub peak_backing_bytes: u64,
}

#[derive(Debug, Default)]
struct Usage {
    fds: usize,
    peak_fds: usize,
    backing: u64,
    peak_backing: u64,
}

#[derive(Debug)]
pub(in crate::runtime) struct RepositoryRootResources {
    limits: RepositoryRootResourceLimits,
    bytes: ImageDescriptorBudget,
    usage: Mutex<Usage>,
    // The native bank's own allocation is retained before this credit retires.
    _control: ImageDescriptorReservation,
}
impl RepositoryRootResources {
    pub fn new(limits: RepositoryRootResourceLimits) -> Result<Arc<Self>, PhysicalRootError> {
        let bytes = ImageDescriptorBudget::new(limits.retained_bytes);
        // Existing BudgetInner is three usize-sized fields plus its Arc header.
        // 64 bytes conservatively covers that utility's owner and alignment.
        let control = bytes
            .try_reserve(add(arc_bytes::<Self>()?, 64)?)
            .map_err(|_| PhysicalRootError::Resource)?;
        Ok(Arc::new(Self {
            limits,
            bytes,
            usage: Mutex::new(Usage::default()),
            _control: control,
        }))
    }
    pub fn snapshot(&self) -> RepositoryRootResourceSnapshot {
        let usage = self
            .usage
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        RepositoryRootResourceSnapshot {
            retained_bytes: self.bytes.used_bytes(),
            peak_retained_bytes: self.bytes.peak_reserved_bytes(),
            native_fds: usage.fds,
            peak_native_fds: usage.peak_fds,
            backing_bytes: usage.backing,
            peak_backing_bytes: usage.peak_backing,
        }
    }
    pub(in crate::runtime) fn reserve(
        self: &Arc<Self>,
        bytes: usize,
        fds: usize,
        backing: u64,
    ) -> Result<ResourceCredit, PhysicalRootError> {
        let bytes = self
            .bytes
            .try_reserve(bytes)
            .map_err(|_| PhysicalRootError::Resource)?;
        {
            let mut usage = self.usage.lock().map_err(|_| PhysicalRootError::Resource)?;
            let next_fds = usage
                .fds
                .checked_add(fds)
                .filter(|v| *v <= self.limits.native_fds)
                .ok_or(PhysicalRootError::Resource)?;
            let next_backing = usage
                .backing
                .checked_add(backing)
                .filter(|v| *v <= self.limits.backing_bytes)
                .ok_or(PhysicalRootError::Resource)?;
            usage.fds = next_fds;
            usage.backing = next_backing;
            usage.peak_fds = usage.peak_fds.max(next_fds);
            usage.peak_backing = usage.peak_backing.max(next_backing);
        }
        Ok(ResourceCredit {
            bytes,
            slots: SlotCredit {
                resources: self.clone(),
                fds,
                backing,
            },
        })
    }
}

#[derive(Debug)]
struct SlotCredit {
    resources: Arc<RepositoryRootResources>,
    fds: usize,
    backing: u64,
}
impl Drop for SlotCredit {
    fn drop(&mut self) {
        let mut usage = self
            .resources
            .usage
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        usage.fds -= self.fds;
        usage.backing -= self.backing;
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct ResourceCredit {
    pub(super) bytes: ImageDescriptorReservation,
    slots: SlotCredit,
}

/// Shared lifetime credit for ONE selected backing. Contains no descriptor,
/// reader, mutation or namespace authority and never enters DICE.
#[derive(Debug)]
pub(super) struct RepositoryRootBacking {
    resources: Arc<RepositoryRootResources>,
    state: Mutex<BackingState>,
}
#[derive(Debug)]
struct BackingState {
    mode: RepositoryWorkspaceMode,
    frozen_archive_quota: Option<FrozenArchiveQuota>,
    binding: Option<(RootIdentity, Arc<RepositoryWorkspaceReservation>)>,
    // Stable spelling/domain claims follow every builder/root/token pin. They
    // contain no IO authority and keep collision control alive for old readers.
    claim: Option<Arc<crate::runtime::repository_io::reservations::SourceRootClaim>>,
    source_binding: Option<(
        RootIdentity,
        Arc<slug_bzlmod_v2::RepositorySourceSnapshotReservation>,
    )>,
    source_snapshot_claim:
        Option<Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>>,
    // Binding payload precedes the sole quota/control credit on destruction.
    credit: ResourceCredit,
}
impl RepositoryRootBacking {
    pub(super) fn new(
        resources: &Arc<RepositoryRootResources>,
        mode: RepositoryWorkspaceMode,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let credit = resources.reserve(arc_bytes::<Self>()?, 0, quota(mode).0)?;
        Ok(Arc::new(Self {
            resources: resources.clone(),
            state: Mutex::new(BackingState {
                mode,
                frozen_archive_quota: None,
                binding: None,
                claim: None,
                source_binding: None,
                source_snapshot_claim: None,
                credit,
            }),
        }))
    }
    pub(super) fn bind_claim(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        claim: Arc<crate::runtime::repository_io::reservations::SourceRootClaim>,
    ) -> Result<(), PhysicalRootError> {
        if !claim.matches_reservation(reservation) || !claim.belongs_to(&self.resources) {
            return Err(PhysicalRootError::Reservation);
        }
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if state.source_binding.is_some()
            || state.source_snapshot_claim.is_some()
            || state
                .binding
                .as_ref()
                .is_some_and(|(_, prior)| prior.as_ref() != reservation)
        {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some(prior) = &state.claim {
            if !Arc::ptr_eq(prior, &claim) {
                return Err(PhysicalRootError::Reservation);
            }
        } else {
            state.claim = Some(claim);
        }
        Ok(())
    }
    pub(super) fn source_claim(
        &self,
    ) -> Option<Arc<crate::runtime::repository_io::reservations::SourceRootClaim>> {
        self.state.lock().ok()?.claim.clone()
    }
    pub(super) fn belongs_to(&self, resources: &Arc<RepositoryRootResources>) -> bool {
        Arc::ptr_eq(&self.resources, resources)
    }
    pub(super) fn matches(
        &self,
        resources: &Arc<RepositoryRootResources>,
        mode: RepositoryWorkspaceMode,
    ) -> Result<(), PhysicalRootError> {
        let state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if !Arc::ptr_eq(&self.resources, resources)
            || state.mode != mode
            || state.frozen_archive_quota.is_some()
            || state.credit.slots.backing != quota(mode).0
        {
            return Err(PhysicalRootError::Backing);
        }
        Ok(())
    }
    pub(super) fn matches_data(
        &self,
        resources: &Arc<RepositoryRootResources>,
        mode: RepositoryWorkspaceMode,
        frozen: Option<FrozenArchiveQuota>,
    ) -> Result<(), PhysicalRootError> {
        let state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if !Arc::ptr_eq(&self.resources, resources)
            || state.mode != mode
            || state.frozen_archive_quota != frozen
            || frozen.is_some() && mode != RepositoryWorkspaceMode::Archive
            || state.credit.slots.backing
                != frozen.map_or_else(|| quota(mode).0, |class| class.bytes())
        {
            return Err(PhysicalRootError::Backing);
        }
        Ok(())
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    pub(super) fn freeze_archive(
        &self,
        proof: &super::mount_lease::readonly_quota::VerifiedReadOnlyCompaction<'_>,
    ) -> Result<(), PhysicalRootError> {
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        let (identity, reservation) = state.binding.as_ref().ok_or(PhysicalRootError::Backing)?;
        if state.mode != RepositoryWorkspaceMode::Archive
            || state.frozen_archive_quota.is_some()
            || state.source_binding.is_some()
            || state.credit.slots.backing != ARCHIVE_BYTES
            || !proof.matches_binding(self, *identity, reservation)
        {
            return Err(PhysicalRootError::Backing);
        }
        let class = proof.class();
        let delta = ARCHIVE_BYTES
            .checked_sub(class.bytes())
            .ok_or(PhysicalRootError::Resource)?;
        let mut usage = self
            .resources
            .usage
            .lock()
            .map_err(|_| PhysicalRootError::Resource)?;
        if usage.backing < state.credit.slots.backing {
            return Err(PhysicalRootError::Resource);
        }
        usage.backing = usage
            .backing
            .checked_sub(delta)
            .ok_or(PhysicalRootError::Resource)?;
        state.credit.slots.backing = class.bytes();
        state.frozen_archive_quota = Some(class);
        Ok(())
    }
    pub(super) fn bind(
        &self,
        identity: RootIdentity,
        reservation: &Arc<RepositoryWorkspaceReservation>,
    ) -> Result<(), PhysicalRootError> {
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if state.source_binding.is_some() || state.source_snapshot_claim.is_some() {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some((selected, owner)) = &state.binding {
            if *selected != identity || owner != reservation {
                return Err(PhysicalRootError::Backing);
            }
            return Ok(());
        }
        let retained = reservation
            .retained_storage_bytes()
            .map_err(|_| PhysicalRootError::Resource)?;
        state
            .credit
            .bytes
            .try_grow(retained)
            .map_err(|_| PhysicalRootError::Resource)?;
        state.binding = Some((identity, reservation.clone()));
        Ok(())
    }
}

impl RepositoryRootBacking {
    pub(super) fn bind_source_claim(
        &self,
        reservation: &slug_bzlmod_v2::RepositorySourceSnapshotReservation,
        claim: Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>,
    ) -> Result<(), PhysicalRootError> {
        if !claim.matches_reservation(reservation) || !claim.belongs_to(&self.resources) {
            return Err(PhysicalRootError::Reservation);
        }
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if state.claim.is_some()
            || state.binding.is_some()
            || state
                .source_binding
                .as_ref()
                .is_some_and(|(_, r)| r.as_ref() != reservation)
        {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some(old) = &state.source_snapshot_claim {
            if !Arc::ptr_eq(old, &claim) {
                return Err(PhysicalRootError::Reservation);
            }
        } else {
            state.source_snapshot_claim = Some(claim)
        }
        Ok(())
    }
    pub(super) fn bind_source(
        &self,
        identity: RootIdentity,
        reservation: &Arc<slug_bzlmod_v2::RepositorySourceSnapshotReservation>,
    ) -> Result<(), PhysicalRootError> {
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        if state.claim.is_some() || state.binding.is_some() {
            return Err(PhysicalRootError::Reservation);
        }
        if let Some((old, r)) = &state.source_binding {
            if *old != identity || r != reservation {
                return Err(PhysicalRootError::Backing);
            }
            return Ok(());
        }
        state
            .credit
            .bytes
            .try_grow(
                reservation
                    .retained_storage_bytes()
                    .map_err(PhysicalRootError::Protocol)?,
            )
            .map_err(|_| PhysicalRootError::Resource)?;
        state.source_binding = Some((identity, reservation.clone()));
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn source_snapshot_claim(
        &self,
    ) -> Option<Arc<crate::runtime::repository_io::source_snapshots::SourceSnapshotClaim>> {
        self.state.lock().ok()?.source_snapshot_claim.clone()
    }
    /// One-way atomic accounting for a physically verified SAME backing. No
    /// bytes/FD credit is refunded, no IO or DICE compute occurs under locks.
    pub(super) fn shrink_to_process(
        &self,
        proof: &super::mount_lease::quota::VerifiedProcessQuota,
    ) -> Result<(), PhysicalRootError> {
        let mut state = self.state.lock().map_err(|_| PhysicalRootError::Resource)?;
        let (identity, reservation) = state.binding.as_ref().ok_or(PhysicalRootError::Backing)?;
        if state.mode != RepositoryWorkspaceMode::Archive
            || state.frozen_archive_quota.is_some()
            || state.credit.slots.backing != ARCHIVE_BYTES
            || !proof.matches_binding(self, *identity, reservation)
        {
            return Err(PhysicalRootError::Backing);
        }
        let delta = ARCHIVE_BYTES
            .checked_sub(PROCESS_BYTES)
            .ok_or(PhysicalRootError::Resource)?;
        let mut usage = self
            .resources
            .usage
            .lock()
            .map_err(|_| PhysicalRootError::Resource)?;
        if usage.backing < state.credit.slots.backing {
            return Err(PhysicalRootError::Resource);
        }
        let remaining = usage
            .backing
            .checked_sub(delta)
            .ok_or(PhysicalRootError::Resource)?;
        usage.backing = remaining;
        state.credit.slots.backing = PROCESS_BYTES;
        state.mode = RepositoryWorkspaceMode::Process;
        Ok(())
    }
}
