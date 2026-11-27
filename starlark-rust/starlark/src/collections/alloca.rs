/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::alloc::Layout;
use std::alloc::alloc;
use std::alloc::dealloc;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::NonNull;
use std::slice;

use crate::collections::maybe_uninit_backport::maybe_uninit_write_slice_cloned;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::hint::likely;
use crate::hint::unlikely;

/// Holds the allocation.
struct Buffer {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Buffer {
    fn alloc(layout: Layout) -> Buffer {
        Self::try_alloc(layout).unwrap()
    }

    fn try_alloc(layout: Layout) -> Option<Buffer> {
        debug_assert_ne!(layout.size(), 0);
        let ptr = NonNull::new(unsafe { alloc(layout) })?;
        Some(Buffer { ptr, layout })
    }

    fn ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    fn end(&self) -> *mut u8 {
        unsafe { self.ptr.as_ptr().add(self.layout.size()) }
    }

    fn size_words(&self) -> usize {
        self.layout.size() / ALIGN
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr.as_ptr().cast(), self.layout) }
    }
}

/// We'd love to use the real `alloca`, but don't want to blow through the stack space,
/// so define our own wrapper.
/// We use a single continuous buffer. When it needs upgrading, we double it and keep the old one around.
pub(crate) struct Alloca {
    // An alternative design would be to bake the <T> into the type, so all allocations are of the same type.
    // Benchmarking that, even if the most optimistic scenario (just reallocating a single element)
    // the performance difference is only 2%, so keep the flexibility of not needing to predeclare the type.
    alloc: Cell<*mut Align>,
    end: Cell<*mut Align>,
    buffers: RefCell<Vec<Buffer>>,
}

const INITIAL_SIZE: usize = 1000000; // ~ 1Mb

type Align = u64;
const ALIGN: usize = mem::size_of::<Align>();

struct RestoreCursor<'a> {
    alloca: &'a Alloca,
    start: *mut Align,
    stop: *mut Align,
    buffer_end: *mut Align,
}

impl Drop for RestoreCursor<'_> {
    fn drop(&mut self) {
        self.alloca
            .restore_cursor(self.start, self.stop, self.buffer_end);
    }
}

impl Alloca {
    pub fn new() -> Self {
        Self::with_capacity(INITIAL_SIZE)
    }

    pub fn with_capacity(size_bytes: usize) -> Self {
        if size_bytes == 0 {
            return Self::new_lazy();
        }
        let size_words = size_bytes.div_ceil(ALIGN);
        let layout = Layout::array::<Align>(size_words).unwrap();
        let buffer = Buffer::alloc(layout);
        Self {
            alloc: Cell::new(buffer.ptr().cast()),
            end: Cell::new(buffer.end().cast()),
            buffers: RefCell::new(vec![buffer]),
        }
    }

    /// Start without allocating a buffer or its metadata.
    pub(crate) fn new_lazy() -> Self {
        Self {
            alloc: Cell::new(ptr::null_mut()),
            end: Cell::new(ptr::null_mut()),
            buffers: RefCell::new(Vec::new()),
        }
    }

    fn assert_state(&self) {
        unsafe {
            debug_assert!(self.end.get().offset_from(self.alloc.get()) >= 0);
            debug_assert!(
                self.end.get().offset_from(self.alloc.get()) as usize
                    <= self.buffers.borrow().last().unwrap().size_words()
            );
        }
    }

    #[cold]
    #[inline(never)]
    fn allocate_more(&self, len: usize, one: Layout) {
        let want =
            Layout::from_size_align(one.size().checked_mul(len).unwrap(), one.align()).unwrap();
        assert!(want.align() <= mem::align_of::<Align>());
        let mut buffers = self.buffers.borrow_mut();
        let size_words = match buffers.last() {
            Some(buffer) => buffer.size_words() * 2 + want.size() / ALIGN,
            None => INITIAL_SIZE
                .div_ceil(ALIGN)
                .max(want.size().div_ceil(ALIGN)),
        };
        let layout = Layout::array::<Align>(size_words).unwrap();
        self.push_buffer(&mut buffers, Buffer::alloc(layout));
    }

    #[cold]
    #[inline(never)]
    fn try_allocate_more(
        &self,
        required_words: usize,
        budget: &ExecutionBudget,
    ) -> Result<(), ExecutionLimit> {
        let mut buffers = self.buffers.borrow_mut();
        let old_words = buffers.last().map_or(0, Buffer::size_words);
        let size_words = old_words
            .checked_mul(2)
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?
            .max(required_words);
        let layout = Layout::array::<Align>(size_words)
            .map_err(|_| budget.refuse(ExecutionLimit::AllocationBytes))?;
        let len = buffers
            .len()
            .checked_add(1)
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?;

        // Pay for the backing buffer and the metadata write before reserving either.
        budget.charge(1, layout.size())?;
        budget.reserve_vec(&mut buffers, len)?;
        let buffer = Buffer::try_alloc(layout)
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationFailed))?;
        self.push_buffer(&mut buffers, buffer);
        Ok(())
    }

    fn push_buffer(&self, buffers: &mut Vec<Buffer>, buffer: Buffer) {
        let pointer = buffer.ptr().cast();
        let end = buffer.end().cast();
        buffers.push(buffer);
        self.alloc.set(pointer);
        self.end.set(end);
    }

    #[inline(always)]
    fn restore_cursor(&self, start: *mut Align, stop: *mut Align, buffer_end: *mut Align) {
        // Nonempty retained buffers have distinct ends. A nested allocation can leave
        // the cursor at an adjacent buffer's start, which might equal the old stop.
        if likely(ptr::eq(self.alloc.get(), stop) && ptr::eq(self.end.get(), buffer_end)) {
            self.alloc.set(start);
        }
    }

    /// Convert remaining capacity in words to remaining capacity in `T`.
    #[inline(always)]
    fn rem_in_words_to_rem_in_t<T>(rem_in_words: usize) -> usize {
        assert!(mem::align_of::<T>() <= ALIGN);
        rem_in_words * ALIGN / mem::size_of::<T>()
    }

    #[inline(always)]
    fn len_in_to_to_len_in_words<T>(len: usize) -> usize {
        if mem::size_of::<T>().is_multiple_of(ALIGN) {
            // Special case to make common case fast:
            // https://rust.godbolt.org/z/adh3nzdzs
            len * (mem::size_of::<T>() / ALIGN)
        } else {
            // Multiplication does not overflow because we know that `[T; len]`
            // is within the size of the current buffer, which is smaller than `isize::MAX`.
            (len * mem::size_of::<T>()).div_ceil(ALIGN)
        }
    }

    /// Note that the `Drop` for the `T` will not be called. That's safe if there is no `Drop`,
    /// or you call it yourself.
    #[inline(always)]
    pub fn alloca_uninit<T, R>(&self, len: usize, k: impl FnOnce(&mut [MaybeUninit<T>]) -> R) -> R {
        if mem::size_of::<T>() == 0 {
            // SAFETY: The non-null, aligned pointer is never dereferenced for bytes.
            return k(unsafe { slice::from_raw_parts_mut(NonNull::dangling().as_ptr(), len) });
        }
        assert!(mem::align_of::<T>() <= mem::align_of::<Align>());

        let mut start = self.alloc.get();
        if unlikely(start.is_null()) {
            self.allocate_more(len, Layout::new::<T>());
            start = self.alloc.get();
        }
        self.assert_state();

        let rem_words = unsafe { self.end.get().offset_from(start) as usize };
        let rem_in_t = Self::rem_in_words_to_rem_in_t::<T>(rem_words);
        if unlikely(len > rem_in_t) {
            self.allocate_more(len, Layout::new::<T>());
            start = self.alloc.get();
        }

        let size_words = Self::len_in_to_to_len_in_words::<T>(len);

        let stop = start.wrapping_add(size_words);
        let buffer_end = self.end.get();
        self.alloc.set(stop);
        let data = start as *mut MaybeUninit<T>;
        let slice = unsafe { slice::from_raw_parts_mut(data, len) };
        let res = k(slice);

        self.restore_cursor(start, stop, buffer_end);
        self.assert_state();

        res
    }

    /// Admit any new storage before lending an uninitialized slice. The caller pays
    /// for work that initializes and uses the slice. Nonempty storage requires `T`
    /// to fit the arena's alignment.
    pub(crate) fn try_alloca_uninit<T, R>(
        &self,
        len: usize,
        budget: &ExecutionBudget,
        k: impl FnOnce(&mut [MaybeUninit<T>]) -> R,
    ) -> Result<R, ExecutionLimit> {
        let layout =
            Layout::array::<T>(len).map_err(|_| budget.refuse(ExecutionLimit::AllocationBytes))?;
        if layout.size() == 0 {
            budget.check()?;
            // SAFETY: The slice requires an aligned pointer but occupies no bytes.
            let slice = unsafe { slice::from_raw_parts_mut(NonNull::dangling().as_ptr(), len) };
            return Ok(k(slice));
        }
        assert!(layout.align() <= mem::align_of::<Align>());

        let words = layout.size().div_ceil(ALIGN);
        let mut start = self.alloc.get();
        let remaining = if start.is_null() {
            0
        } else {
            // SAFETY: Both cursors are in the same current buffer.
            unsafe { self.end.get().offset_from(start) as usize }
        };
        if words > remaining {
            self.try_allocate_more(words, budget)?;
            start = self.alloc.get();
        } else {
            budget.check()?;
        }

        // SAFETY: The checked word count fits in the current aligned buffer.
        let stop = unsafe { start.add(words) };
        let _restore = RestoreCursor {
            alloca: self,
            start,
            stop,
            buffer_end: self.end.get(),
        };
        self.alloc.set(stop);
        // SAFETY: The rounded allocation covers the full slice and nested calls use
        // later words or a new buffer. The scope guard rewinds after the loan ends.
        let slice = unsafe { slice::from_raw_parts_mut(start.cast(), len) };
        Ok(k(slice))
    }

    #[inline(always)]
    pub fn alloca_init<T, R>(
        &self,
        len: usize,
        init: impl FnMut() -> T,
        k: impl FnOnce(&mut [T]) -> R,
    ) -> R {
        self.alloca_uninit(len, |data| k(Self::init_slice(data, init)))
    }

    /// Charge one unit per initialization before running the fixed-work,
    /// allocation-free initializer.
    pub(crate) fn try_alloca_init<T: Copy, R>(
        &self,
        len: usize,
        budget: &ExecutionBudget,
        init: impl FnMut() -> T,
        k: impl FnOnce(&mut [T]) -> R,
    ) -> Result<R, ExecutionLimit> {
        budget.charge(len, 0)?;
        self.try_alloca_uninit(len, budget, |data| k(Self::init_slice(data, init)))
    }

    #[inline(always)]
    fn init_slice<T>(data: &mut [MaybeUninit<T>], mut init: impl FnMut() -> T) -> &mut [T] {
        for x in data.iter_mut() {
            x.write(init());
        }
        // SAFETY: Every element has been initialized above.
        unsafe { &mut *(data as *mut [MaybeUninit<T>] as *mut [T]) }
    }

    #[allow(dead_code)] // Dead, but morally a sensible API to provide, and useful for testing
    #[inline(always)]
    pub fn alloca_fill<T: Copy, R>(&self, len: usize, fill: T, k: impl FnOnce(&mut [T]) -> R) -> R {
        self.alloca_init(len, || fill, k)
    }

    fn alloca_concat_slow<T: Clone, R, F>(&self, x: &[T], y: &[T], k: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        // We clone the input into a slice allocated in `Alloca`,
        // slice does not invoke `Drop`, so we do drop explicitly.
        struct DropSliceGuard<A>(*mut [A]);
        impl<A> Drop for DropSliceGuard<A> {
            fn drop(&mut self) {
                unsafe {
                    ptr::drop_in_place(self.0);
                }
            }
        }

        self.alloca_uninit(x.len() + y.len(), |xy| {
            let (x_uninit, y_uninit) = xy.split_at_mut(x.len());
            let x = maybe_uninit_write_slice_cloned(x_uninit, x);
            let _x_drop_guard = DropSliceGuard(x);
            let y = maybe_uninit_write_slice_cloned(y_uninit, y);
            let _y_drop_guard = DropSliceGuard(y);
            let xy = unsafe { &mut *(xy as *mut [MaybeUninit<T>] as *mut [T]) };
            k(xy)
        })
    }

    /// Concat two slices and invoke the callback with the result.
    /// Use either slice as is if the other is empty,
    /// otherwise clone the elements into a temporary slice.
    #[inline(always)]
    pub(crate) fn alloca_concat<T: Clone, R, F>(&self, x: &[T], y: &[T], k: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        if x.is_empty() {
            k(y)
        } else if y.is_empty() {
            k(x)
        } else {
            self.alloca_concat_slow(x, y, k)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;

    use super::*;

    #[allow(clippy::identity_op)]
    #[test]
    fn test_rem_in_words_to_rem_in_t() {
        assert_eq!(10, Alloca::rem_in_words_to_rem_in_t::<Align>(10));
        assert_eq!(
            1 * ALIGN / mem::size_of::<u32>(),
            Alloca::rem_in_words_to_rem_in_t::<u32>(1)
        );
        assert_eq!(
            2 * ALIGN / mem::size_of::<u32>(),
            Alloca::rem_in_words_to_rem_in_t::<u32>(2)
        );
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN - 1]>(1));
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN]>(1));
        assert_eq!(0, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN + 1]>(1));
        assert_eq!(2, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN - 1]>(2));
        assert_eq!(2, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN]>(2));
        assert_eq!(1, Alloca::rem_in_words_to_rem_in_t::<[u8; ALIGN + 1]>(2));
    }

    #[test]
    fn test_len_in_t_to_len_in_words() {
        assert_eq!(10, Alloca::len_in_to_to_len_in_words::<Align>(10));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN - 1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(ALIGN + 1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN - 1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN));
        assert_eq!(3, Alloca::len_in_to_to_len_in_words::<u8>(2 * ALIGN + 1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN - 1]>(1));
        assert_eq!(1, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN]>(1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN + 1]>(1));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN - 1]>(2));
        assert_eq!(2, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN]>(2));
        assert_eq!(3, Alloca::len_in_to_to_len_in_words::<[u8; ALIGN + 1]>(2));
    }

    #[test]
    fn test_alloca() {
        // Use a small capacity to encourage overflow behaviour
        let a = Alloca::with_capacity(100);
        a.alloca_fill(3, 8usize, |xs| {
            xs[0] = 5;
            xs[2] = xs[0] + xs[1] + 15;
            a.alloca_fill(200, 18usize, |ys| {
                assert_eq!(xs[2], 8 + 5 + 15);
                assert_eq!(ys[0], 18);
                assert_eq!(ys[200 - 1], 18);
            });
            a.alloca_fill(3, 1u64, |_| {});
            assert_eq!(xs[2], 8 + 5 + 15);
        })
    }

    #[test]
    fn trigger_bug() {
        let a = Alloca::with_capacity(100);
        for _ in 0..100 {
            a.alloca_fill(10, 17usize, |_| {
                a.alloca_fill(1000, 19usize, |_| {});
            });
        }

        assert_eq!(2, a.buffers.borrow().len());
    }

    #[test]
    fn test_alloca_bug_not_aligned() {
        let a = Alloca::with_capacity(100);
        a.alloca_fill(1, 17u8, |xs| {
            // Bug was triggered because the end of first allocation
            // was rounded down instead of up.
            a.alloca_fill(1, 19u8, |ys| {
                assert_eq!([17], xs);
                assert_eq!([19], ys);
            });
        })
    }

    #[test]
    fn test_alloca_concat() {
        let a = Alloca::new();
        let x = vec!["ab".to_owned()];
        let y = vec!["cd".to_owned()];
        a.alloca_concat(&x, &[], |xy| {
            // No copy here.
            assert_eq!(xy.as_ptr(), x.as_ptr());
        });
        a.alloca_concat(&[], &x, |xy| {
            // No copy here.
            assert_eq!(xy.as_ptr(), x.as_ptr());
        });
        a.alloca_concat(&x, &y, |xy| {
            assert_eq!(&["ab".to_owned(), "cd".to_owned()], xy);
        });
    }

    #[test]
    fn lazy_alloca_charges_first_buffer_and_metadata() {
        let allocation_bytes = ALIGN + mem::size_of::<Buffer>();
        let arena = Alloca::new_lazy();
        let short = ExecutionBudget::new(1, allocation_bytes - 1, 1);
        assert_eq!(
            arena.try_alloca_uninit::<u8, ()>(1, &short, |_| unreachable!()),
            Err(ExecutionLimit::AllocationBytes),
        );
        assert_eq!(arena.buffers.borrow().capacity(), 0);
        assert!(arena.alloc.get().is_null());

        let exact = ExecutionBudget::new(1, allocation_bytes, 1);
        let first = arena
            .try_alloca_uninit::<u8, _>(1, &exact, |slice| slice.as_ptr())
            .unwrap();
        let reused = arena
            .try_alloca_uninit::<Align, _>(1, &exact, |slice| slice.as_ptr())
            .unwrap();
        assert_eq!(first.cast::<()>(), reused.cast::<()>());
        assert_eq!(arena.buffers.borrow().len(), 1);
        assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
    }

    #[test]
    fn lazy_alloca_precharges_init_on_reuse() {
        let arena = Alloca::with_capacity(ALIGN);
        let short = ExecutionBudget::new(2, 0, 1);
        assert_eq!(
            arena.try_alloca_init(3, &short, || -> u8 { unreachable!() }, |_| unreachable!()),
            Err::<(), _>(ExecutionLimit::Work),
        );

        let exact = ExecutionBudget::new(3, 0, 1);
        let initials = [11u8, 12u8, 13u8];
        let mut next = initials.into_iter();
        arena
            .try_alloca_init(
                3,
                &exact,
                || next.next().unwrap(),
                |slice| {
                    assert_eq!(slice, &initials);
                },
            )
            .unwrap();
        assert_eq!(exact.charge(1, 0), Err(ExecutionLimit::Work));
    }

    #[test]
    fn lazy_alloca_accepts_zero_size_and_rejects_layout_overflow() {
        #[repr(align(32))]
        struct Empty;

        let arena = Alloca::new_lazy();
        let budget = ExecutionBudget::new(0, 0, 1);
        arena
            .try_alloca_uninit::<Align, _>(0, &budget, |slice| assert!(slice.is_empty()))
            .unwrap();
        arena
            .try_alloca_uninit::<Empty, _>(7, &budget, |slice| {
                assert_eq!(slice.len(), 7);
                assert_eq!(slice.as_ptr(), NonNull::dangling().as_ptr());
            })
            .unwrap();
        assert_eq!(
            arena.try_alloca_uninit::<Align, ()>(usize::MAX, &budget, |_| unreachable!()),
            Err(ExecutionLimit::AllocationBytes),
        );
        assert_eq!(
            arena.try_alloca_uninit::<Align, ()>(0, &budget, |_| unreachable!()),
            Err(ExecutionLimit::AllocationBytes),
        );
        assert_eq!(arena.buffers.borrow().capacity(), 0);
        assert!(arena.alloc.get().is_null());
    }

    #[test]
    fn lazy_alloca_nested_growth_retains_outer_on_error_and_unwind() {
        let arena = Alloca::new_lazy();
        let budget = ExecutionBudget::new(6, 3 * ALIGN + 3 * mem::size_of::<Buffer>(), 1);
        let second_start = Cell::new(ptr::null_mut());
        let result = arena
            .try_alloca_init(
                1,
                &budget,
                || 17 as Align,
                |outer| {
                    let unwind = catch_unwind(AssertUnwindSafe(|| {
                        arena
                            .try_alloca_init(
                                2,
                                &budget,
                                || 42 as Align,
                                |inner| {
                                    assert_eq!([42, 42], inner);
                                    second_start.set(inner.as_mut_ptr());
                                    panic!("nested callback");
                                },
                            )
                            .unwrap();
                    }));
                    assert!(unwind.is_err());
                    assert_eq!([17], outer);
                    assert_eq!(arena.alloc.get(), second_start.get());
                    Err::<(), _>("outer callback failed")
                },
            )
            .unwrap();
        assert_eq!(result, Err("outer callback failed"));
        assert_eq!(arena.alloc.get(), second_start.get());
        assert_eq!(arena.buffers.borrow().len(), 2);

        arena
            .try_alloca_uninit::<Align, _>(2, &budget, |slice| {
                assert_eq!(slice.as_mut_ptr().cast::<Align>(), second_start.get());
            })
            .unwrap();
        assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
    }

    #[test]
    fn lazy_alloca_switches_between_ordinary_and_budgeted() {
        let arena = Alloca::new_lazy();
        let budget = ExecutionBudget::new(0, 0, 1);
        arena.alloca_fill(1, 17 as Align, |outer| {
            arena
                .try_alloca_uninit::<Align, _>(1, &budget, |inner| {
                    assert_ne!(outer.as_ptr(), inner.as_ptr().cast::<Align>());
                    inner[0].write(42);
                })
                .unwrap();
            assert_eq!([17], outer);
        });
        arena
            .try_alloca_uninit::<Align, _>(1, &budget, |outer| {
                arena.alloca_fill(1, 42 as Align, |inner| {
                    assert_ne!(outer.as_ptr().cast::<Align>(), inner.as_ptr());
                });
            })
            .unwrap();
        assert_eq!(arena.buffers.borrow().len(), 1);
    }
}
