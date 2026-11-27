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

use std::marker::PhantomData;
use std::mem;

use crate::cast;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::private::Private;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::Heap;
use crate::values::HeapSendable;
use crate::values::StarlarkValue;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::avalue::heap_copy_impl;
use crate::values::layout::avalue::heap_freeze_simple_impl;
use crate::values::layout::avalue::try_freeze_directly;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::heap::send::HeapSyncable;
use crate::values::layout::value_alloc_size::ValueAllocSize;

pub(crate) fn simple<'v, T: StarlarkValue<'v> + Send + Sync + 'static>(
    x: T,
) -> AValueImpl<'v, AValueSimple<T>> {
    assert!(!T::is_special(Private));
    AValueImpl::<AValueSimple<T>>::new(x)
}

pub(crate) struct AValueSimple<T>(PhantomData<T>);

fn admit_simple_with_cloned_text(
    budget: &ExecutionBudget,
    arena: Option<ValueAllocSize>,
    cloned_text_lengths: impl IntoIterator<Item = usize>,
) -> Result<(), ExecutionLimit> {
    let mut bytes = arena
        .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?
        .bytes() as usize;
    let mut work = 1 + ExecutionBudget::text_work(bytes);
    for length in cloned_text_lengths {
        bytes = bytes
            .checked_add(length)
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?;
        work = work
            .checked_add(1 + ExecutionBudget::text_work(length))
            .ok_or_else(|| budget.refuse(ExecutionLimit::Work))?;
    }
    budget.charge(work, bytes)
}

impl<'v, T: StarlarkValue<'v>> AValue<'v> for AValueSimple<T> {
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self>()
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        unsafe {
            if let Some(f) = try_freeze_directly::<Self>(me, freezer) {
                return f;
            }

            heap_freeze_simple_impl::<Self>(me, freezer)
        }
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        unsafe { heap_copy_impl::<Self>(me, tracer, |_v, _tracer| {}) }
    }
}

impl FrozenHeap {
    pub(crate) fn alloc_simple_typed<
        T: StarlarkValue<'static> + HeapSendable<'static> + HeapSyncable<'static> + Send + Sync,
    >(
        &self,
        val: T,
    ) -> FrozenValueTyped<'static, T> {
        // SAFETY: Not.
        let this: &'static FrozenHeap = unsafe { cast::ptr_lifetime(self) };
        this.alloc_raw(simple(val))
    }

    /// Allocate a simple [`StarlarkValue`] on this heap.
    ///
    /// Simple value is any starlark value which:
    /// * bound by `'static` lifetime (in particular, it cannot contain references to other `Value`s)
    /// * is not special builtin (e.g. `None`)
    pub fn alloc_simple<
        T: StarlarkValue<'static> + HeapSendable<'static> + HeapSyncable<'static> + Send + Sync,
    >(
        &self,
        val: T,
    ) -> FrozenValue {
        self.alloc_simple_typed(val).to_frozen_value()
    }

    /// Admit one aligned simple value before calling its trusted constructor.
    /// Admit any allocator work inside `build` with the same budget.
    pub fn alloc_budgeted_simple<
        T: StarlarkValue<'static> + HeapSendable<'static> + HeapSyncable<'static> + Send + Sync,
    >(
        &self,
        budget: &ExecutionBudget,
        build: impl FnOnce() -> T,
    ) -> Result<FrozenValue, ExecutionLimit> {
        self.alloc_budgeted_simple_with_cloned_text(budget, &[], build)
    }

    /// Admit a simple value and the requested bytes of each text copy before
    /// calling its trusted constructor. Other allocations in `build` must be
    /// admitted separately.
    pub fn alloc_budgeted_simple_with_cloned_text<
        T: StarlarkValue<'static> + HeapSendable<'static> + HeapSyncable<'static> + Send + Sync,
    >(
        &self,
        budget: &ExecutionBudget,
        cloned_text_lengths: &[usize],
        build: impl FnOnce() -> T,
    ) -> Result<FrozenValue, ExecutionLimit> {
        if !cloned_text_lengths.is_empty() {
            budget.charge(cloned_text_lengths.len(), 0)?;
        }
        admit_simple_with_cloned_text(
            budget,
            Self::checked_chunk_value_size::<AValueSimple<T>>(0),
            cloned_text_lengths.iter().copied(),
        )?;
        let value = build();
        budget.check()?;
        Ok(self.alloc_simple(value))
    }
}

impl<'v> Heap<'v> {
    /// Allocate a simple value after admitting its arena object and owned text copies.
    /// `cloned_text_lengths` gives the requested byte length of each string cloned
    /// by `build`. The constructor runs only after admission succeeds.
    pub fn alloc_budgeted_simple_with_cloned_text<
        T: StarlarkValue<'v> + HeapSendable<'v> + Send + Sync + 'static,
    >(
        self,
        cloned_text_lengths: impl IntoIterator<Item = usize>,
        build: impl FnOnce() -> T,
    ) -> Result<Value<'v>, ExecutionLimit> {
        let budget = self.execution_budget();
        if let Some(budget) = budget {
            admit_simple_with_cloned_text(
                budget,
                AValueSimple::<T>::checked_alloc_size_for_extra_len(0),
                cloned_text_lengths,
            )?;
        }
        let value = build();
        if let Some(budget) = budget {
            budget.check()?;
        }
        Ok(self.alloc_simple(value))
    }

    /// Allocate a simple [`StarlarkValue`] on this heap.
    ///
    /// Simple value is any starlark value which:
    /// * bound by `'static` lifetime (in particular, it cannot contain references to other `Value`s)
    /// * is not special builtin (e.g. `None`)
    ///
    /// Must be [`Send`] and [`Sync`] because it will be reused in frozen values.
    pub fn alloc_simple<T: StarlarkValue<'v> + HeapSendable<'v> + Send + Sync + 'static>(
        self,
        x: T,
    ) -> Value<'v> {
        self.alloc_raw(simple(x)).to_value()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use allocative::Allocative;
    use starlark_derive::starlark_value;

    use super::*;
    use crate as starlark;
    use crate::environment::Module;
    use crate::values::NoSerialize;
    use crate::values::ProvidesStaticType;

    #[derive(
        Debug,
        Allocative,
        ProvidesStaticType,
        NoSerialize,
        derive_more::Display
    )]
    #[display("{}", _0)]
    struct OwnedText(String);

    #[starlark_value(type = "owned_text")]
    impl<'v> StarlarkValue<'v> for OwnedText {}

    #[test]
    fn budgeted_simple_text_admits_all_allocation_and_work_before_construction() {
        let text = "x".repeat(257);
        let arena_bytes = AValueSimple::<OwnedText>::alloc_size_for_extra_len(0).bytes() as usize;
        let bytes = arena_bytes + text.len();
        let work =
            2 + ExecutionBudget::text_work(arena_bytes) + ExecutionBudget::text_work(text.len());
        for (work, bytes, expected) in [
            (work, bytes, Ok(())),
            (work, bytes - 1, Err(ExecutionLimit::AllocationBytes)),
            (work - 1, bytes, Err(ExecutionLimit::Work)),
        ] {
            let budget = Arc::new(ExecutionBudget::new(work, bytes, 8));
            let module = Module::new_with_execution_budget(budget.clone());
            let built = Cell::new(false);
            let result = module
                .heap()
                .alloc_budgeted_simple_with_cloned_text([text.len()], || {
                    built.set(true);
                    OwnedText(text.clone())
                });
            assert_eq!(budget.check(), expected);
            assert_eq!(built.get(), expected.is_ok());
            assert_eq!(result.map(|_| ()), expected);
        }
    }

    #[test]
    fn budgeted_frozen_simple_text_refuses_before_its_constructor() {
        let text = "雪".repeat(86);
        let arena_bytes = AValueSimple::<OwnedText>::alloc_size_for_extra_len(0).bytes() as usize;
        let bytes = arena_bytes + text.len();
        let work =
            3 + ExecutionBudget::text_work(arena_bytes) + ExecutionBudget::text_work(text.len());
        for (work, bytes, expected) in [
            (work, bytes, Ok(())),
            (work, bytes - 1, Err(ExecutionLimit::AllocationBytes)),
            (work - 1, bytes, Err(ExecutionLimit::Work)),
        ] {
            let budget = ExecutionBudget::new(work, bytes, 8);
            let heap = FrozenHeap::new();
            let built = Cell::new(false);
            let result =
                heap.alloc_budgeted_simple_with_cloned_text(&budget, &[text.len()], || {
                    built.set(true);
                    OwnedText(text.clone())
                });
            assert_eq!(built.get(), expected.is_ok());
            assert_eq!(budget.check(), expected);
            match result {
                Ok(value) => {
                    assert_eq!(value.downcast_frozen_ref::<OwnedText>().unwrap().0, text);
                    assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
                }
                Err(error) => {
                    assert_eq!(Err(error), expected);
                    assert_eq!(heap.allocated_bytes(), 0);
                }
            }
        }
    }

    #[test]
    fn budgeted_frozen_simple_keeps_constructor_refusal_without_publishing() {
        let budget = ExecutionBudget::new(100, 1024, 8);
        let heap = FrozenHeap::new();
        let result = heap.alloc_budgeted_simple(&budget, || {
            budget.refuse(ExecutionLimit::Work);
            OwnedText(String::new())
        });
        assert!(matches!(result, Err(ExecutionLimit::Work)));
        assert_eq!(heap.allocated_bytes(), 0);
    }
}
