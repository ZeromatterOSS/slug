//! Callback allocation kernel only; no native root/source authority is minted.
use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;

fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(RepositoryRootResourceLimits::new(1024 * 1024, 8, 1).unwrap())
        .unwrap()
}
fn live(state: &CallbackState) -> usize {
    state
        .counts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .live_payload
}
fn calls(state: &CallbackState) -> usize {
    state
        .counts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .malloc_calls
}

#[test]
fn allocator_admits_exact_layout_before_malloc_and_refuses_bank_plus_one() {
    for extra in [0, 1] {
        let bank = bank();
        let baseline = bank.snapshot().retained_bytes;
        let layout = allocation_layout(37).unwrap();
        let hold = bank
            .reserve(1024 * 1024 - baseline - layout.size() + extra, 0, 0)
            .unwrap();
        let state = CallbackState::new(Some(bank.clone()), 37);
        let pointer = state.allocate(1, 37);
        if extra == 0 {
            assert!(!pointer.is_null());
            assert_eq!(pointer as usize % align_of::<libc::max_align_t>(), 0);
            assert_eq!(pointer as usize % align_of::<Header>(), 0);
            assert_eq!(live(&state), 37);
            assert_eq!(calls(&state), 1);
            assert_eq!(bank.snapshot().retained_bytes, 1024 * 1024);
            unsafe {
                pointer.cast::<u8>().write_bytes(0x7f, 37);
                state.release(pointer);
            }
            assert_eq!(live(&state), 0);
        } else {
            assert!(pointer.is_null());
            assert_eq!(calls(&state), 0, "bank refusal precedes malloc");
            assert_eq!(live(&state), 0);
            assert_eq!(state.failure(), Some(Failure::Allocation));
        }
        drop(hold);
        drop(state);
        assert_eq!(bank.snapshot().retained_bytes, baseline);
        assert_eq!(bank.snapshot().native_fds, 0);
        assert_eq!(bank.snapshot().backing_bytes, 0);
    }
}

#[test]
fn allocator_checks_zero_overflow_layout_and_sticky_payload_ceiling() {
    for (items, size) in [
        (0, 1),
        (1, 0),
        (usize::MAX, 2),
        (1, usize::MAX),
        (1, isize::MAX as usize),
    ] {
        let state = CallbackState::new(None, usize::MAX);
        assert!(state.allocate(items, size).is_null());
        assert_eq!(state.failure(), Some(Failure::Allocation));
        assert_eq!(calls(&state), 0);
        unsafe {
            state.release(std::ptr::null_mut());
        }
    }
    let state = CallbackState::new(None, 64);
    let a = state.allocate(2, 16);
    let b = state.allocate(1, 32);
    assert!(!a.is_null() && !b.is_null());
    assert_eq!(live(&state), 64);
    assert!(state.allocate(1, 1).is_null());
    assert_eq!(state.failure(), Some(Failure::DecoderLimit));
    assert_eq!(calls(&state), 2);
    unsafe {
        state.release(a);
        state.release(b);
    }
    assert_eq!(live(&state), 0);
    assert!(
        state.allocate(1, 1).is_null(),
        "refusal stays sticky after frees"
    );
}

#[test]
fn allocator_oom_rolls_back_prepaid_credit_and_records_fixed_failure() {
    unsafe fn fail(_: Layout) -> *mut u8 {
        std::ptr::null_mut()
    }
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let mut state = CallbackState::new(Some(bank.clone()), 1024);
    state.malloc = fail;
    assert!(state.allocate(1, 31).is_null());
    assert_eq!(calls(&state), 1);
    assert_eq!(live(&state), 0);
    assert_eq!(state.failure(), Some(Failure::Allocation));
    assert!(
        bank.snapshot().peak_retained_bytes >= baseline + allocation_layout(31).unwrap().size()
    );
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn extern_callback_contains_preallocation_panic_and_rolls_back_credit() {
    unsafe fn fail(_: Layout) -> *mut u8 {
        panic!("injected allocator failure before malloc")
    }
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let mut state = CallbackState::new(Some(bank.clone()), 1024);
    state.malloc = fail;
    let opaque = (&state as *const CallbackState).cast_mut().cast();
    assert!(allocate(opaque, 1, 31).is_null());
    assert_eq!(state.failure(), Some(Failure::Allocation));
    assert_eq!(live(&state), 0);
    assert_eq!(calls(&state), 1);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    release(opaque, std::ptr::null_mut());
}

#[test]
fn callback_bookkeeping_serializes_simultaneous_allocations_without_ffi_unwind() {
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let state = CallbackState::new(Some(bank.clone()), 64);
    std::thread::scope(|threads| {
        for _ in 0..4 {
            let state = &state;
            threads.spawn(move || {
                for _ in 0..64 {
                    let opaque = (state as *const CallbackState).cast_mut().cast();
                    let pointer = allocate(opaque, 1, 16);
                    assert!(!pointer.is_null());
                    release(opaque, pointer);
                }
            });
        }
    });
    assert_eq!(calls(&state), 256);
    assert_eq!(live(&state), 0);
    assert_eq!(state.failure(), None);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    release(std::ptr::null_mut(), std::ptr::null_mut());
}

#[test]
fn partial_lib_init_failure_frees_prior_successful_allocation_before_context_retirement() {
    std::thread_local! {
        static MALLOC_COUNTDOWN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    unsafe fn fail_second(layout: Layout) -> *mut u8 {
        MALLOC_COUNTDOWN.with(|countdown| {
            let remaining = countdown.get();
            countdown.set(remaining.saturating_sub(1));
            if remaining == 1 {
                std::ptr::null_mut()
            } else {
                // SAFETY: the real allocator validated this positive Layout.
                unsafe { alloc(layout) }
            }
        })
    }
    struct EndStream(lzma_sys::lzma_stream);
    impl Drop for EndStream {
        fn drop(&mut self) {
            // SAFETY: allocator context outlives this guard even on assertions;
            // end accepts an initialized or failed/partially initialized stream.
            unsafe { lzma_sys::lzma_end(&mut self.0) };
        }
    }
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let owner = AllocatorOwner::new(Some(bank.clone()), 136 << 20, size_of::<EndStream>()).unwrap();
    // SAFETY: context is uniquely owned and no C call/borrow is active. The
    // per-thread test seam changes only malloc, before initialization starts.
    unsafe {
        (*owner.context.as_ptr()).state.malloc = fail_second;
    }
    let before_init = bank.snapshot().retained_bytes;
    MALLOC_COUNTDOWN.with(|countdown| countdown.set(2));
    // SAFETY: documented LZMA_STREAM_INIT; no Context borrow spans the C call.
    let mut stream = EndStream(unsafe { std::mem::zeroed() });
    stream.0.allocator = owner.allocator();
    let status = unsafe {
        lzma_sys::lzma_stream_decoder(&mut stream.0, 136 << 20, lzma_sys::LZMA_CONCATENATED)
    };
    assert_eq!(status, lzma_sys::LZMA_MEM_ERROR);
    assert_eq!(owner.failure(), Some(Failure::Allocation));
    let state = unsafe { &(*owner.context.as_ptr()).state };
    assert_eq!(
        calls(state),
        2,
        "one real allocation succeeded before the injected second refusal"
    );
    assert!(bank.snapshot().peak_retained_bytes > before_init);
    drop(stream);
    assert_eq!(live(state), 0);
    assert_eq!(bank.snapshot().retained_bytes, before_init);
    drop(owner);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
}
