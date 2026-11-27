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
use std::fmt;
use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::vec;

use dupe::Dupe;
use starlark_syntax::ErrorKind;
use starlark_syntax::codemap::FileSpan;
use starlark_syntax::slice_vec_ext::SliceExt;

use crate::errors::Frame;
use crate::eval::CallStack;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::inlined_frame::InlinedFrames;
use crate::hint::unlikely;
use crate::values::FrozenRef;
use crate::values::FrozenValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;

// A value akin to Frame, but can be created cheaply, since it doesn't resolve
// anything in advance.
// The downside is it has a lifetime on 'v and keeps alive the whole CodeMap.
#[derive(Clone, Copy, Dupe)]
struct CheapFrame<'v> {
    function: Value<'v>,
    span: Option<FrozenRef<'static, FrameSpan>>,
}

impl CheapFrame<'_> {
    fn location(&self) -> Option<FileSpan> {
        self.span.map(|span| span.span.to_file_span())
    }

    fn extend_frames(&self, frames: &mut Vec<Frame>) {
        if let Some(span) = self.span {
            span.inlined_frames.extend_frames(frames);
        }
        frames.push(self.to_frame());
    }

    fn to_frame(&self) -> Frame {
        Frame {
            name: self.function.name_for_call_stack(),
            location: self.location(),
        }
    }
}

impl Debug for CheapFrame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut x = f.debug_struct("Frame");
        x.field("function", &self.function);
        x.field("span", &self.span);
        x.finish()
    }
}

#[derive(Debug, thiserror::Error)]
enum CallStackError {
    #[error("Requested {0}-th top frame, but stack size is {1} (internal error)")]
    StackIsTooShallowForNthTopFrame(usize, usize),
    #[error("Starlark call stack overflow")]
    Overflow,
    #[error("Starlark call stack is already allocated")]
    AlreadyAllocated,
}

/// Starlark call stack.
#[derive(Debug)]
pub(crate) struct CheapCallStack<'v> {
    count: usize,
    stack: Box<[CheapFrame<'v>]>,
}

impl<'v> Default for CheapCallStack<'v> {
    fn default() -> Self {
        Self {
            count: 0,
            stack: Box::new(
                [CheapFrame {
                    function: Value::new_none(),
                    span: None,
                }; 0],
            ),
        }
    }
}

unsafe impl<'v> Trace<'v> for CheapCallStack<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        let (used, unused) = self.stack.split_at_mut(self.count);
        for x in used {
            x.function.trace(tracer);
        }
        // Not required, but since we are choosing not to walk those above
        // the current stack depth, it's good practice to blank those values out
        for x in unused {
            x.function = Value::new_none();
            x.span = None;
        }
    }
}

impl<'v> CheapCallStack<'v> {
    // Currently, each frame typically allocates about 1K of native stack size (see `test_frame_size`),
    // but it is a bit more complicated:
    // * each for loop in a frame allocates more native stack
    // * inlined functions do not allocate native stack
    // Practically max call stack depends on native stack size,
    // and depending on environment, it may be configured differently, for example:
    // * macOS default stack size is 512KB
    // * Linux default stack size is 8MB
    // * [tokio default stack size is 2MB][1]
    // [1] https://docs.rs/tokio/0.2.1/tokio/runtime/struct.Builder.html#method.thread_stack_size
    pub(crate) fn alloc_if_needed(
        &mut self,
        max_size: usize,
        budget: Option<&ExecutionBudget>,
    ) -> crate::Result<()> {
        if !self.stack.is_empty() {
            if self.stack.len() != max_size {
                return Err(crate::Error::new_other(CallStackError::AlreadyAllocated));
            }
            if let Some(budget) = budget {
                budget.check()?;
            }
            return Ok(());
        }

        let empty = CheapFrame {
            function: Value::new_none(),
            span: None,
        };
        self.stack = match budget {
            Some(budget) => Self::alloc_budgeted(max_size, empty, budget)?,
            None => vec![empty; max_size].into_boxed_slice(),
        };
        Ok(())
    }

    fn alloc_budgeted(
        max_size: usize,
        empty: CheapFrame<'v>,
        budget: &ExecutionBudget,
    ) -> Result<Box<[CheapFrame<'v>]>, ExecutionLimit> {
        let layout = Layout::array::<CheapFrame<'v>>(max_size)
            .map_err(|_| budget.refuse(ExecutionLimit::AllocationBytes))?;
        budget.charge(max_size, layout.size())?;
        if max_size == 0 {
            return Ok(Box::new([]));
        }

        // SAFETY: The checked layout has nonzero size. The allocation uses the
        // global allocator and matches the boxed slice's layout exactly.
        let memory = NonNull::new(unsafe { alloc(layout) })
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationFailed))?;
        let slice =
            NonNull::slice_from_raw_parts(memory.cast::<MaybeUninit<CheapFrame<'v>>>(), max_size);
        // SAFETY: The uninitialized elements are valid MaybeUninit values.
        let mut frames = unsafe { Box::from_raw(slice.as_ptr()) };
        frames.fill(MaybeUninit::new(empty));
        // SAFETY: Every element was initialized above.
        Ok(unsafe { frames.assume_init() })
    }

    /// Push an element to the stack. It is important the each `push` is paired
    /// with a `pop`.
    pub(crate) fn push(
        &mut self,
        function: Value<'v>,
        span: Option<FrozenRef<'static, FrameSpan>>,
    ) -> crate::Result<()> {
        if unlikely(self.count >= self.stack.len()) {
            return Err(crate::Error::new_kind(ErrorKind::StackOverflow(
                CallStackError::Overflow.into(),
            )));
        }
        self.stack[self.count] = CheapFrame { function, span };
        self.count += 1;
        Ok(())
    }

    /// Remove the top element from the stack. Called after `push`.
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.count >= 1);
        // We could clear the elements, but don't need to bother
        self.count -= 1;
    }

    /// Current size (in frames) of the stack.
    pub(crate) fn count(&self) -> usize {
        self.count
    }

    /// The frame at the top of the stack. May be `None` if
    /// either there the stack is empty, or the top of the stack lacks location
    /// information (e.g. called from Rust).
    pub(crate) fn top_frame(&self) -> Option<Frame> {
        Some(self.stack.last().as_ref()?.to_frame())
    }

    /// The location at the top of the stack. May be `None` if
    /// either there the stack is empty, or the top of the stack lacks location
    /// information (e.g. called from Rust).
    pub(crate) fn top_location(&self) -> Option<FileSpan> {
        if self.count == 0 {
            None
        } else {
            self.stack[self.count - 1].location()
        }
    }

    pub(crate) fn nth_location(&self, n: usize) -> Option<FileSpan> {
        if n >= self.count {
            None
        } else {
            self.stack[self.count - 1 - n].location()
        }
    }

    /// `n`-th element from the top of the stack.
    pub(crate) fn top_nth_function(&self, n: usize) -> anyhow::Result<Value<'v>> {
        self.top_nth_function_opt(n)
            .ok_or_else(|| CallStackError::StackIsTooShallowForNthTopFrame(n, self.count).into())
    }

    pub(crate) fn top_nth_function_opt(&self, n: usize) -> Option<Value<'v>> {
        let index = self.count.checked_sub(1).and_then(|x| x.checked_sub(n))?;
        Some(self.stack[index].function)
    }

    pub(crate) fn native_call_innermost_inlined_function(&self) -> Option<FrozenValue> {
        let span = self
            .count
            .checked_sub(1)
            .and_then(|index| self.stack[index].span)?;
        span.inlined_frames.innermost_function()
    }

    pub(crate) fn native_call_inlined_functions(&self) -> Vec<FrozenValue> {
        self.count
            .checked_sub(1)
            .and_then(|index| self.stack[index].span)
            .map(|span| span.inlined_frames.functions_innermost_first())
            .unwrap_or_default()
    }

    /// Logical source filenames represented by the native call-site span,
    /// ordered from the innermost Starlark body through compiler-inlined
    /// callers. The enclosing non-inlined call stack is intentionally omitted.
    pub(crate) fn native_call_inlined_source_filenames(&self) -> Option<Vec<String>> {
        let span = self
            .count
            .checked_sub(1)
            .and_then(|index| self.stack[index].span)?;
        let mut filenames = vec![span.span.to_file_span().file.filename().to_owned()];
        filenames.extend(span.inlined_frames.source_filenames_innermost_first());
        Some(filenames)
    }

    pub(crate) fn to_diagnostic_frames(&self, inlined_frames: InlinedFrames) -> CallStack {
        // The first entry is just the entire module, so skip it
        let mut frames = Vec::new();
        for frame in &self.stack[1..self.count] {
            frame.extend_frames(&mut frames);
        }
        inlined_frames.extend_frames(&mut frames);
        CallStack { frames }
    }

    /// List the entries on the stack as values
    pub(crate) fn to_function_values(&self) -> Vec<Value<'v>> {
        self.stack[1..self.count].map(|x| x.function)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_limit(error: crate::Error, expected: ExecutionLimit) {
        assert!(matches!(
            error.kind(),
            ErrorKind::Other(error) if error.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ));
    }

    #[test]
    fn budgeted_call_stack_admits_exact_initialization_and_reuses_storage() {
        let frames = 3;
        let bytes = Layout::array::<CheapFrame<'_>>(frames).unwrap().size();
        for (work, bytes, refusal) in [
            (frames, bytes, None),
            (frames, bytes - 1, Some(ExecutionLimit::AllocationBytes)),
            (frames - 1, bytes, Some(ExecutionLimit::Work)),
        ] {
            let budget = ExecutionBudget::new(work, bytes, 8);
            let mut stack = CheapCallStack::default();
            let result = stack.alloc_if_needed(frames, Some(&budget));
            assert_eq!(stack.count(), 0);
            if let Some(refusal) = refusal {
                assert_limit(result.unwrap_err(), refusal);
                assert!(stack.stack.is_empty());
                assert_eq!(budget.check(), Err(refusal));
                continue;
            }

            result.unwrap();
            assert_eq!(stack.stack.len(), frames);
            assert!(
                stack
                    .stack
                    .iter()
                    .all(|frame| frame.function.is_none() && frame.span.is_none())
            );
            let storage = stack.stack.as_ptr();
            stack.alloc_if_needed(frames, Some(&budget)).unwrap();
            assert_eq!(stack.stack.as_ptr(), storage);
            assert_eq!(budget.check(), Ok(()));
            assert!(stack.alloc_if_needed(frames + 1, Some(&budget)).is_err());
            assert_eq!(stack.stack.as_ptr(), storage);
        }
    }

    #[test]
    fn budgeted_call_stack_refuses_layout_overflow_before_allocation() {
        let budget = ExecutionBudget::new(usize::MAX, usize::MAX, 8);
        let mut stack = CheapCallStack::default();
        let result = stack.alloc_if_needed(usize::MAX, Some(&budget));
        assert_limit(result.unwrap_err(), ExecutionLimit::AllocationBytes);
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        assert!(stack.stack.is_empty());
    }
}
