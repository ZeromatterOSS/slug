use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::ImageDescriptorBudget;

use super::SelectedScratchError;

#[derive(Allocative)]
struct PayloadService {
    limit: usize,
    used: AtomicUsize,
}

#[derive(Clone, Dupe, Allocative)]
pub struct SelectedScratchPayloadBudget(Arc<PayloadService>);
impl std::fmt::Debug for SelectedScratchPayloadBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectedScratchPayloadBudget")
            .field("limit", &self.limit_bytes())
            .field("used", &self.used_bytes())
            .finish()
    }
}
impl SelectedScratchPayloadBudget {
    pub fn new(limit_bytes: usize) -> Self {
        Self(Arc::new(PayloadService {
            limit: limit_bytes,
            used: AtomicUsize::new(0),
        }))
    }
    pub fn used_bytes(&self) -> usize {
        self.0.used.load(Ordering::Acquire)
    }
    pub fn limit_bytes(&self) -> usize {
        self.0.limit
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(super) fn reserve(&self, bytes: usize) -> Result<PayloadReservation, SelectedScratchError> {
        let mut used = self.used_bytes();
        loop {
            let next = used
                .checked_add(bytes)
                .ok_or(SelectedScratchError::PayloadResourceLimit)?;
            if next > self.limit_bytes() {
                return Err(SelectedScratchError::PayloadResourceLimit);
            }
            match self
                .0
                .used
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    return Ok(PayloadReservation {
                        budget: self.dupe(),
                        bytes,
                    });
                }
                Err(actual) => used = actual,
            }
        }
    }
}

#[derive(Debug, Allocative)]
pub(super) struct PayloadReservation {
    pub(super) budget: SelectedScratchPayloadBudget,
    bytes: usize,
}
impl Drop for PayloadReservation {
    fn drop(&mut self) {
        self.budget.0.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Clone, Debug, Dupe, Allocative)]
pub struct SelectedScratchResources {
    payload: SelectedScratchPayloadBudget,
    descriptor: ImageDescriptorBudget,
}
impl SelectedScratchResources {
    pub fn new(payload: SelectedScratchPayloadBudget, descriptor: ImageDescriptorBudget) -> Self {
        Self {
            payload,
            descriptor,
        }
    }
    pub fn payload_budget(&self) -> &SelectedScratchPayloadBudget {
        &self.payload
    }
    pub fn descriptor_budget(&self) -> &ImageDescriptorBudget {
        &self.descriptor
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        self.payload.same_instance(&other.payload)
            && self.descriptor.same_instance(&other.descriptor)
    }
}
