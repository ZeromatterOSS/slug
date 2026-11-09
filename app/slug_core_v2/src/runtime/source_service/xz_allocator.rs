//! Single-threaded liblzma callbacks. No Source state/watch, active callback,
//! semantic data, or IO authority is accessed across the C boundary.
use std::alloc::Layout;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;

use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::ResourceCredit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    DecoderLimit,
    Allocation,
}

struct Header {
    layout: Layout,
    payload: usize,
    credit: Option<ResourceCredit>,
}
const ALIGN: usize = if align_of::<libc::max_align_t>() > align_of::<Header>() {
    align_of::<libc::max_align_t>()
} else {
    align_of::<Header>()
};
const PREFIX: usize = (size_of::<Header>() + ALIGN - 1) & !(ALIGN - 1);

fn allocation_layout(payload: usize) -> Option<Layout> {
    let header = Layout::from_size_align(size_of::<Header>(), ALIGN).ok()?;
    let payload = Layout::from_size_align(payload, ALIGN).ok()?;
    let (layout, offset) = header.extend(payload).ok()?;
    if offset != PREFIX {
        return None;
    }
    Some(layout.pad_to_align())
}

#[derive(Default)]
struct Counts {
    live_payload: usize,
    failure: Option<Failure>,
    #[cfg(test)]
    malloc_calls: usize,
}

struct CallbackState {
    counts: Mutex<Counts>,
    resources: Option<Arc<RepositoryRootResources>>,
    limit: usize,
    #[cfg(test)]
    malloc: unsafe fn(Layout) -> *mut u8,
}
impl CallbackState {
    fn new(resources: Option<Arc<RepositoryRootResources>>, limit: usize) -> Self {
        Self {
            counts: Mutex::new(Counts::default()),
            resources,
            limit,
            #[cfg(test)]
            malloc: alloc,
        }
    }
    fn failure(&self) -> Option<Failure> {
        self.counts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .failure
    }
    fn deny(&self, failure: Failure) {
        let mut counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        counts.failure.get_or_insert(failure);
    }
    fn allocate(&self, items: usize, size: usize) -> *mut c_void {
        let mut counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        if counts.failure.is_some() {
            return std::ptr::null_mut();
        }
        let Some(payload) = items.checked_mul(size).filter(|n| *n > 0) else {
            counts.failure = Some(Failure::Allocation);
            return std::ptr::null_mut();
        };
        let Some(layout) = allocation_layout(payload) else {
            counts.failure = Some(Failure::Allocation);
            return std::ptr::null_mut();
        };
        let Some(next) = counts.live_payload.checked_add(payload) else {
            counts.failure = Some(Failure::Allocation);
            return std::ptr::null_mut();
        };
        if next > self.limit {
            counts.failure = Some(Failure::DecoderLimit);
            return std::ptr::null_mut();
        }
        let credit = match self
            .resources
            .as_ref()
            .map(|r| r.reserve(layout.size(), 0, 0))
            .transpose()
        {
            Ok(credit) => credit,
            Err(_) => {
                counts.failure = Some(Failure::Allocation);
                return std::ptr::null_mut();
            }
        };
        #[cfg(test)]
        {
            counts.malloc_calls += 1;
        }
        // SAFETY: positive, checked layout; null is handled before writing.
        #[cfg(not(test))]
        let base = unsafe { alloc(layout) };
        #[cfg(test)]
        let base = unsafe { (self.malloc)(layout) };
        if base.is_null() {
            drop(credit);
            counts.failure = Some(Failure::Allocation);
            return std::ptr::null_mut();
        }
        // SAFETY: the base is Header-aligned and PREFIX leaves disjoint,
        // C-max-aligned payload storage of the checked requested size.
        unsafe {
            base.cast::<Header>().write(Header {
                layout,
                payload,
                credit,
            });
        }
        counts.live_payload = next;
        // SAFETY: PREFIX was verified by Layout::extend above.
        unsafe { base.add(PREFIX).cast() }
    }
    unsafe fn release(&self, address: *mut c_void) {
        if address.is_null() {
            return;
        }
        // SAFETY: liblzma may free only an unfreed pointer returned by this
        // allocator. Reading moves the credit out before exact deallocation.
        let base = unsafe { address.cast::<u8>().sub(PREFIX) };
        let header = unsafe { base.cast::<Header>().read() };
        unsafe { dealloc(base, header.layout) };
        drop(header.credit);
        let mut counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        match counts.live_payload.checked_sub(header.payload) {
            Some(live) => counts.live_payload = live,
            None => {
                counts.failure.get_or_insert(Failure::Allocation);
            }
        }
    }
}

extern "C" fn allocate(opaque: *mut c_void, items: usize, size: usize) -> *mut c_void {
    if opaque.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: opaque points to the immutable, stable callback state until end.
    let state = unsafe { &*opaque.cast::<CallbackState>() };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.allocate(items, size))) {
        Ok(address) => address,
        Err(payload) => {
            // Never run an arbitrary panic payload destructor across C.
            std::mem::forget(payload);
            state.deny(Failure::Allocation);
            std::ptr::null_mut()
        }
    }
}
extern "C" fn release(opaque: *mut c_void, address: *mut c_void) {
    if opaque.is_null() || address.is_null() {
        return;
    }
    // SAFETY: the owner keeps state live until lzma_end has freed every block.
    let state = unsafe { &*opaque.cast::<CallbackState>() };
    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        state.release(address)
    })) {
        std::mem::forget(payload);
        state.deny(Failure::Allocation);
    }
}

struct Context {
    allocator: lzma_sys::lzma_allocator,
    state: CallbackState,
}

pub(super) struct AllocatorOwner {
    context: NonNull<Context>,
    // Stable callback payload is deallocated before its control credit retires.
    _credit: Option<ResourceCredit>,
}
impl AllocatorOwner {
    #[cfg(test)]
    pub(super) fn context_bytes() -> usize {
        size_of::<Context>()
    }
    pub(super) fn new(
        resources: Option<Arc<RepositoryRootResources>>,
        limit: usize,
        owner_bytes: usize,
    ) -> Result<Self, Failure> {
        let bytes = owner_bytes
            .checked_add(size_of::<Context>())
            .ok_or(Failure::Allocation)?;
        let credit = resources
            .as_ref()
            .map(|r| r.reserve(bytes, 0, 0))
            .transpose()
            .map_err(|_| Failure::Allocation)?;
        let layout = Layout::new::<Context>();
        // SAFETY: fixed nonzero layout; null refuses without constructing state.
        let pointer =
            NonNull::new(unsafe { alloc(layout) }.cast::<Context>()).ok_or(Failure::Allocation)?;
        unsafe {
            pointer.as_ptr().write(Context {
                allocator: lzma_sys::lzma_allocator {
                    alloc: Some(allocate),
                    free: Some(release),
                    opaque: std::ptr::null_mut(),
                },
                state: CallbackState::new(resources, limit),
            });
            (*pointer.as_ptr()).allocator.opaque =
                std::ptr::addr_of_mut!((*pointer.as_ptr()).state).cast();
        }
        Ok(Self {
            context: pointer,
            _credit: credit,
        })
    }
    pub(super) fn allocator(&self) -> *const lzma_sys::lzma_allocator {
        // No mutable Context borrow survives a C call. Callbacks access only
        // the disjoint state field through its own opaque pointer.
        unsafe { std::ptr::addr_of!((*self.context.as_ptr()).allocator) }
    }
    pub(super) fn failure(&self) -> Option<Failure> {
        unsafe { &(*self.context.as_ptr()).state }.failure()
    }
}
impl Drop for AllocatorOwner {
    fn drop(&mut self) {
        // Caller must have completed lzma_end before dropping this owner.
        unsafe {
            self.context.as_ptr().drop_in_place();
            dealloc(self.context.as_ptr().cast(), Layout::new::<Context>());
        }
    }
}

#[cfg(test)]
#[path = "xz_allocator_tests.rs"]
mod tests;
