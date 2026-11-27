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

//! Local variables and stack, in single allocation.

use std::mem;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

use dupe::Dupe;

use crate::eval::Evaluator;
use crate::eval::ExecutionLimit;
use crate::eval::bc::for_loop::LoopDepth;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::runtime::slots::LocalSlotIdCapturedOrNot;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;

/// Current `def` frame (but not native function frame).
///
/// Frame memory layout:
///
/// ```text
/// [ loop_indices | BcFrame | locals | stack ]
///   BcFramePtr points here ^
/// ```
#[repr(C)]
struct BcFrame<'v> {
    /// Number of local slots.
    local_count: u32,
    /// Number of stack slots.
    max_stack_size: u32,
    /// Max number of nested for loops.
    max_loop_depth: LoopDepth,
    /// `local_count` local slots followed by `max_stack_size` stack slots.
    slots: [Option<Value<'v>>; 0],
}

const _: () = assert!(mem::size_of::<BcFrame>().is_multiple_of(mem::size_of::<usize>()));
const _: () = assert!(mem::align_of::<BcFrame>() == mem::align_of::<usize>());
const _: () = assert!(mem::size_of::<Option<Value>>() == mem::size_of::<usize>());

#[derive(Copy, Clone, Dupe)]
pub(crate) struct BcFramePtr<'v> {
    /// Pointer to the `slots` field of `BcFrame`.
    ///
    /// We could store `BcFrame` pointer here, but since the most common
    /// data accessed is slots, storing `slots` pointer is slightly more efficient:
    /// no need to add a constant when accessing the field.
    slots_ptr: *mut Option<Value<'v>>,
}

impl<'v> PartialEq for BcFramePtr<'v> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.slots_ptr, other.slots_ptr)
    }
}

impl<'v> BcFramePtr<'v> {
    pub(crate) fn null() -> BcFramePtr<'v> {
        BcFramePtr {
            slots_ptr: ptr::null_mut(),
        }
    }

    /// Is this frame allocated or constructed empty?
    pub(crate) fn is_inititalized(self) -> bool {
        !self.slots_ptr.is_null()
    }

    #[inline(always)]
    fn frame<'a>(self) -> &'a BcFrame<'v> {
        debug_assert!(self.is_inititalized());
        unsafe {
            let frame = (self.slots_ptr as *mut u8).sub(BcFrame::offset_of_slots()) as *mut BcFrame;
            &*frame
        }
    }

    #[inline(always)]
    fn frame_mut<'a>(self) -> &'a mut BcFrame<'v> {
        debug_assert!(self.is_inititalized());
        unsafe {
            let frame = (self.slots_ptr as *mut u8).sub(BcFrame::offset_of_slots()) as *mut BcFrame;
            &mut *frame
        }
    }

    pub(crate) fn get_slot_slow(self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        assert!(slot.0 < self.frame().local_count);
        self.get_slot(slot)
    }

    #[inline(always)]
    pub(crate) fn get_slot(self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        self.frame().get_slot(slot)
    }

    pub(crate) fn set_slot_slow(self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        assert!(slot.0 < self.frame().local_count);
        self.set_slot(slot, value);
    }

    #[inline(always)]
    pub(crate) fn set_slot(self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        self.frame_mut().set_slot(slot, value)
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot(self, slot: BcSlotIn) -> Value<'v> {
        self.frame().get_bc_slot(slot)
    }

    #[inline(always)]
    pub(crate) fn set_bc_slot(self, slot: BcSlotOut, value: Value<'v>) {
        self.frame_mut().set_bc_slot(slot, value)
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot_range<'a>(self, slots: BcSlotInRange) -> &'a [Value<'v>] {
        self.frame().get_bc_slot_range(slots)
    }

    #[inline(always)]
    pub(crate) fn get_iter_index(self, loop_depth: LoopDepth) -> usize {
        self.frame().get_iter_index(loop_depth)
    }

    #[inline(always)]
    pub(crate) fn set_iter_index(self, loop_depth: LoopDepth, index: usize) {
        self.frame_mut().set_iter_index(loop_depth, index)
    }

    pub(crate) fn max_stack_size(self) -> u32 {
        self.frame().max_stack_size
    }

    #[inline(always)]
    pub(crate) unsafe fn locals_mut<'a>(self) -> &'a mut [Option<Value<'v>>] {
        self.frame_mut().locals_mut()
    }
}

impl<'v> BcFrame<'v> {
    #[inline(always)]
    fn offset_of_slots() -> usize {
        let offset = memoffset::offset_of!(BcFrame<'v>, slots);
        assert_eq!(mem::size_of::<BcFrame<'v>>(), offset);
        offset
    }

    #[inline(always)]
    fn frame_ptr(&mut self) -> BcFramePtr<'v> {
        unsafe {
            BcFramePtr {
                slots_ptr: (self as *mut _ as *mut u8).add(Self::offset_of_slots()) as *mut _,
            }
        }
    }

    #[inline(always)]
    fn locals_mut(&mut self) -> &mut [Option<Value<'v>>] {
        unsafe { slice::from_raw_parts_mut(self.slots.as_mut_ptr(), self.local_count as usize) }
    }

    #[inline(always)]
    fn locals_uninit(&mut self) -> &mut [MaybeUninit<Option<Value<'v>>>] {
        unsafe {
            slice::from_raw_parts_mut(self.slots.as_mut_ptr() as *mut _, self.local_count as usize)
        }
    }

    #[inline(always)]
    fn stack_uninit(&mut self) -> &mut [MaybeUninit<Value<'v>>] {
        unsafe {
            slice::from_raw_parts_mut(
                self.slots.as_mut_ptr().add(self.local_count as usize) as *mut _,
                self.max_stack_size as usize,
            )
        }
    }

    /// Initialize frame after it was allocated.
    #[inline(always)]
    fn init(&mut self) {
        self.locals_uninit().fill(MaybeUninit::new(None));

        if cfg!(debug_assertions) {
            // Write junk to the stack to trigger memory error if the stack is used incorrectly.
            unsafe {
                // Any bit pattern would do except
                // * a bit pattern which would make valid int tag (0b_xxxx_x010)
                // * zeros, which would be interpreted as uninitialized if copied to a local
                let byte = 0xef;

                let start = self.stack_uninit().as_ptr() as *mut u8;
                ptr::write_bytes(
                    start,
                    byte,
                    (self.max_stack_size as usize) * mem::size_of::<Value>(),
                );
            }
        }
    }

    /// Gets a local variable. Returns None to indicate the variable is not yet assigned.
    #[inline(always)]
    pub(crate) fn get_slot(&self, slot: LocalSlotIdCapturedOrNot) -> Option<Value<'v>> {
        debug_assert!(slot.0 < self.local_count);
        unsafe { self.slots.as_ptr().add(slot.0 as usize).read() }
    }

    /// Get a stack slot.
    #[inline(always)]
    pub(crate) fn get_bc_slot(&self, slot: BcSlotIn) -> Value<'v> {
        debug_assert!(slot.get().0 < self.local_count + self.max_stack_size);
        unsafe {
            let option = self.slots.as_ptr().add(slot.get().0 as usize).read();
            // Slot must be always initialized.
            option.unwrap_unchecked()
        }
    }

    #[inline(always)]
    pub(crate) fn set_slot(&mut self, slot: LocalSlotIdCapturedOrNot, value: Value<'v>) {
        debug_assert!(slot.0 < self.local_count);
        unsafe {
            self.slots
                .as_mut_ptr()
                .add(slot.0 as usize)
                .write(Some(value))
        }
    }

    #[inline(always)]
    pub(crate) fn get_bc_slot_range(&self, slots: BcSlotInRange) -> &[Value<'v>] {
        debug_assert!(slots.end.get().0 <= self.local_count + self.max_stack_size);
        unsafe {
            let start = self.slots.as_ptr().add(slots.start.get().0 as usize);
            let start = start as *const Value;
            slice::from_raw_parts(start, slots.len() as usize)
        }
    }

    #[inline(always)]
    pub(crate) fn set_bc_slot(&mut self, slot: BcSlotOut, value: Value<'v>) {
        debug_assert!(slot.get().0 < self.local_count + self.max_stack_size);
        unsafe {
            self.slots
                .as_mut_ptr()
                .add(slot.get().0 as usize)
                .write(Some(value))
        }
    }

    #[inline(always)]
    pub(crate) fn set_iter_index(&mut self, iter_index: LoopDepth, index: usize) {
        debug_assert!(iter_index < self.max_loop_depth);
        unsafe {
            (self as *mut BcFrame as *mut usize)
                .sub(iter_index.0 as usize)
                .sub(1)
                .write(index);
        }
    }

    #[inline(always)]
    pub(crate) fn get_iter_index(&self, iter_index: LoopDepth) -> usize {
        debug_assert!(iter_index < self.max_loop_depth);
        unsafe {
            (self as *const BcFrame as *const usize)
                .sub(iter_index.0 as usize)
                .sub(1)
                .read()
        }
    }
}

unsafe impl<'v> Trace<'v> for BcFrame<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.locals_mut().trace(tracer);
        // Note this does not trace the stack.
        // GC can be performed only when the stack is empty.
    }
}

unsafe impl<'v> Trace<'v> for BcFramePtr<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.frame_mut().trace(tracer);
    }
}

struct RestoreFrame<'scope, 'v, 'a, 'e> {
    eval: &'scope mut Evaluator<'v, 'a, 'e>,
    old_frame: BcFramePtr<'v>,
}

impl Drop for RestoreFrame<'_, '_, '_, '_> {
    fn drop(&mut self) {
        self.eval.current_frame = self.old_frame;
    }
}

/// Allocate a frame and store it in the evaluator for the duration of the callback.
#[inline(always)]
pub(crate) fn alloca_frame<'v, 'a, 'e: 'a, R>(
    eval: &mut Evaluator<'v, 'a, 'e>,
    local_count: u32,
    max_stack_size: u32,
    max_loop_depth: LoopDepth,
    k: impl FnOnce(&mut Evaluator<'v, 'a, 'e>) -> R,
) -> Result<R, ExecutionLimit> {
    assert_eq!(mem::align_of::<BcFrame>() % mem::size_of::<usize>(), 0);
    assert_eq!(mem::size_of::<Value>(), mem::size_of::<usize>());
    let header_words = mem::size_of::<BcFrame>() / mem::size_of::<usize>();
    let budget = eval.heap().execution_budget();
    let words = header_words
        .checked_add(local_count as usize)
        .and_then(|words| words.checked_add(max_stack_size as usize))
        .and_then(|words| words.checked_add(max_loop_depth.0 as usize));
    let words = match (words, budget) {
        (Some(words), _) => words,
        (None, Some(budget)) => return Err(budget.refuse(ExecutionLimit::AllocationBytes)),
        (None, None) => panic!("bytecode frame size overflow"),
    };
    if let Some(budget) = budget {
        // The checked storage includes every word written here. Loop indices and
        // the release-build stack are initialized later by bytecode instructions.
        let mut init_words = header_words + local_count as usize;
        if cfg!(debug_assertions) {
            init_words += max_stack_size as usize;
        }
        budget.charge(init_words, 0)?;
    }

    eval.try_alloca_uninit::<usize, _, _>(words, budget, |slice, eval| {
        // SAFETY: The frame starts after the iteration indices in a word-aligned
        // slice sized for its header, locals and stack.
        let frame = unsafe {
            let frame = slice.as_mut_ptr().add(max_loop_depth.0 as usize) as *mut BcFrame;
            frame.write(BcFrame {
                local_count,
                max_stack_size,
                max_loop_depth,
                slots: [],
            });
            (*frame).frame_ptr()
        };
        // TODO(nga): no need to fill the slots for parameters.
        frame.frame_mut().init();
        let old_frame = mem::replace(&mut eval.current_frame, frame);
        let restore = RestoreFrame { eval, old_frame };
        k(restore.eval)
    })
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;
    use std::sync::Arc;

    use super::*;
    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::eval::ExecutionBudget;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;

    #[test]
    fn bounded_frame_prepays_header_locals_and_debug_stack() {
        let local_count = 3;
        let stack_count = 2;
        let init_work = mem::size_of::<BcFrame>() / mem::size_of::<usize>()
            + local_count as usize
            + usize::from(cfg!(debug_assertions)) * stack_count as usize;
        let short = Arc::new(ExecutionBudget::new(init_work - 1, 1024, 1));
        let module = Module::new_with_execution_budget(short);
        let mut eval = Evaluator::new(&module);
        assert_eq!(
            alloca_frame(&mut eval, local_count, stack_count, LoopDepth(1), |_| {
                unreachable!()
            }),
            Err::<(), _>(ExecutionLimit::Work),
        );
        assert!(!eval.current_frame.is_inititalized());

        // One additional unit is the new buffer's metadata write.
        let exact = Arc::new(ExecutionBudget::new(init_work + 1, 1024, 1));
        let module = Module::new_with_execution_budget(exact.clone());
        let mut eval = Evaluator::new(&module);
        alloca_frame(&mut eval, local_count, stack_count, LoopDepth(1), |eval| {
            for slot in 0..local_count {
                assert!(
                    eval.current_frame
                        .get_slot_slow(LocalSlotIdCapturedOrNot(slot))
                        .is_none()
                );
            }
        })
        .unwrap();
        assert_eq!(exact.charge(1, 0), Err(ExecutionLimit::Work));
        assert!(!eval.current_frame.is_inititalized());
    }

    #[test]
    fn bounded_frame_restores_the_outer_frame_after_unwind() {
        let budget = Arc::new(ExecutionBudget::new(1024, 1024, 1));
        let module = Module::new_with_execution_budget(budget);
        let mut eval = Evaluator::new(&module);
        let result = alloca_frame(&mut eval, 1, 1, LoopDepth(0), |eval| {
            let outer = eval.current_frame;
            let slot = LocalSlotIdCapturedOrNot(0);
            outer.set_slot_slow(slot, Value::new_bool(true));
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                alloca_frame::<()>(eval, 8, 1, LoopDepth(0), |eval| {
                    assert!(eval.current_frame != outer);
                    panic!("inner frame callback");
                })
                .unwrap();
            }));
            assert_eq!(
                unwind.unwrap_err().downcast_ref::<&str>(),
                Some(&"inner frame callback"),
            );
            assert!(eval.current_frame == outer);
            assert_eq!(outer.get_slot_slow(slot).unwrap().unpack_bool(), Some(true));
            Err::<(), _>("outer frame callback")
        })
        .unwrap();
        assert_eq!(result, Err("outer frame callback"));
        assert!(!eval.current_frame.is_inititalized());
    }

    #[test]
    fn prepared_frame_uses_the_active_modules_budget() {
        let module = Module::new();
        let budget = Arc::new(ExecutionBudget::new(0, 1024, 1));
        let active = Module::new_with_execution_budget(budget.clone());
        let mut eval = Evaluator::new(&module);
        let globals = Globals::standard();
        let ast = AstModule::parse("ordinary.star", "None".to_owned(), &Dialect::Standard).unwrap();
        // Initialize the ordinary call stack before switching prepared modules.
        eval.eval_module(ast, &globals).unwrap();
        let ast =
            AstModule::parse("active.star", "answer = 42".to_owned(), &Dialect::Standard).unwrap();
        let prepared = eval.prepare_module_in(&active, ast, &globals).unwrap();
        let error = eval.eval_prepared_module(&prepared).unwrap_err();
        match error.kind() {
            starlark_syntax::ErrorKind::Other(error) => {
                assert_eq!(
                    error.downcast_ref::<ExecutionLimit>(),
                    Some(&ExecutionLimit::Work)
                );
            }
            error => panic!("unexpected error: {error:?}"),
        }
        assert_eq!(budget.check(), Err(ExecutionLimit::Work));
        assert!(active.get("answer").is_none());
        assert!(ptr::eq(eval.module(), &module));
    }
}
