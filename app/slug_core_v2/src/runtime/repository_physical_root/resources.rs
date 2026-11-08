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
    pub(super) fn reserve(
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
pub(super) struct ResourceCredit {
    pub(super) bytes: ImageDescriptorReservation,
    slots: SlotCredit,
}
