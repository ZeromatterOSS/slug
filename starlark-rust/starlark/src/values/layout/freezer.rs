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
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt::Debug;
use std::mem;

use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::values::AllocFrozenValue;
use crate::values::FreezeError;
use crate::values::FreezeResult;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalues::simple::AValueSimple;
use crate::values::layout::heap::arena::Reservation;
use crate::values::layout::heap::repr::AValueOrForwardUnpack;
use crate::values::layout::value::FrozenValue;
use crate::values::layout::value::Value;
use crate::values::layout::value_captured::ValueCaptured;
use crate::values::types::any::StarlarkAny;

mod traversal;

#[cfg(test)]
mod traversal_tests;

use traversal::FreezeTraversalState;

// Only a new live object increases native recursion depth.
const MAX_FREEZE_DEPTH: usize = 64;

struct RestoreDepth<'a>(&'a Cell<usize>, usize);

impl Drop for RestoreDepth<'_> {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}

/// Used to `freeze` values by [`Freeze::freeze`](crate::values::Freeze::freeze).
pub struct Freezer<'fv> {
    /// Freezing into this heap.
    pub(crate) heap: &'fv FrozenHeap,
    /// Defs frozen by this freezer.
    pub(crate) frozen_defs: RefCell<Vec<FrozenRef<'static, FrozenDef>>>,
    traversal: FreezeTraversalState,
    budget: Option<&'fv ExecutionBudget>,
    depth: Cell<usize>,
}

impl<'fv> Freezer<'fv> {
    pub(crate) fn new(heap: &'fv FrozenHeap) -> Self {
        Self::with_execution_budget(heap, None)
    }

    pub(crate) fn with_execution_budget(
        heap: &'fv FrozenHeap,
        budget: Option<&'fv ExecutionBudget>,
    ) -> Self {
        Freezer {
            heap,
            frozen_defs: RefCell::new(Vec::new()),
            traversal: FreezeTraversalState::default(),
            budget,
            depth: Cell::new(0),
        }
    }

    pub(crate) fn execution_budget(&self) -> Option<&ExecutionBudget> {
        self.budget
    }

    // The bounded module entry point recovers the typed cause from the ledger.
    pub(crate) fn execution_error(_limit: ExecutionLimit) -> FreezeError {
        FreezeError::new("bounded Starlark freeze refused".to_owned())
    }

    pub(crate) fn refuse_execution(&self, limit: ExecutionLimit) -> FreezeError {
        Self::execution_error(self.budget.expect("bounded freezer").refuse(limit))
    }

    pub(crate) fn charge_execution(&self, work: usize, bytes: usize) -> FreezeResult<()> {
        if let Some(budget) = self.budget {
            budget.charge(work, bytes).map_err(Self::execution_error)?;
        }
        Ok(())
    }

    pub(crate) fn collect_exact_vec<T>(
        &self,
        values: impl ExactSizeIterator<Item = FreezeResult<T>>,
    ) -> FreezeResult<Vec<T>> {
        let count = values.len();
        let layout = Layout::array::<T>(count)
            .map_err(|_| self.refuse_execution(ExecutionLimit::AllocationBytes))?;
        self.charge_execution(
            count.max(ExecutionBudget::text_work(layout.size())),
            layout.size(),
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| self.refuse_execution(ExecutionLimit::AllocationFailed))?;
        for value in values {
            result.push(value?);
        }
        Ok(result)
    }

    pub(crate) fn collect_exact_boxed<T>(
        &self,
        values: impl ExactSizeIterator<Item = FreezeResult<T>>,
    ) -> FreezeResult<Box<[T]>> {
        let result = self.collect_exact_vec(values)?;
        if mem::size_of::<T>() != 0 && result.capacity() != result.len() {
            // The pinned allocator normally reports exact capacity. Admit a
            // possible boxed-slice shrink and relocation before it runs.
            let bytes = mem::size_of_val(result.as_slice());
            self.charge_execution(result.len().max(ExecutionBudget::text_work(bytes)), bytes)?;
        }
        Ok(result.into_boxed_slice())
    }

    pub(crate) fn alloc_module_data<T: Debug + Send + Sync + 'static>(
        &self,
        value: T,
    ) -> FreezeResult<FrozenRef<'static, T>> {
        self.admit_arena_object::<AValueSimple<StarlarkAny<T>>>()?;
        Ok(self.heap.alloc_any(value))
    }

    fn admit_arena_object<'v, T: AValue<'v, ExtraElem = ()>>(&self) -> FreezeResult<()> {
        if self.budget.is_some() {
            let bytes = T::checked_alloc_size_for_extra_len(0)
                .ok_or_else(|| self.refuse_execution(ExecutionLimit::AllocationBytes))?
                .bytes() as usize;
            self.charge_execution(1 + ExecutionBudget::text_work(bytes), bytes)?;
        }
        Ok(())
    }

    pub(crate) fn register_frozen_def(
        &self,
        def: FrozenRef<'static, FrozenDef>,
    ) -> FreezeResult<()> {
        let mut defs = self.frozen_defs.borrow_mut();
        if let Some(budget) = self.budget {
            budget
                .push_vec(&mut defs, def)
                .map_err(Self::execution_error)?;
        } else {
            defs.push(def);
        }
        Ok(())
    }

    /// Allocate a new value while freezing. Usually not a great idea.
    pub fn alloc<'v, T: AllocFrozenValue>(&'v self, val: T) -> FrozenValue {
        val.alloc_frozen_value(self.heap)
    }

    pub(crate) fn reserve<'v, 'v2, T: AValue<'v2, ExtraElem = ()>>(
        &'v self,
    ) -> FreezeResult<(FrozenValue, Reservation<'v2, T>)> {
        self.admit_arena_object::<T>()?;
        let (fv, r, extra) = self.heap.reserve_with_extra::<T>(0);
        let extra = unsafe { &mut *extra };
        debug_assert!(extra.is_empty());
        Ok((fv, r))
    }

    /// Test whether a value is frozen or has a freeze forwarding header.
    ///
    /// This never starts freezing or inspects a forwarded payload: the target may
    /// still be under construction. It does not make a failed freezer reusable.
    pub fn is_frozen_or_forwarded(&self, value: Value) -> bool {
        value.unpack_frozen().is_some()
            || matches!(
                value.0.unpack_ptr().unwrap().unpack(),
                AValueOrForwardUnpack::Forward(_)
            )
    }

    /// Charge work for a cooperative custom freeze traversal.
    ///
    /// All nested traversals share 65536 records per freezer, without refunds.
    /// This does not meter unrelated existing freeze operations.
    pub fn charge_traversal_work(&self, amount: usize) -> FreezeResult<()> {
        self.traversal.charge_work(amount)
    }

    /// Prepay additional scratch peak bytes for a cooperative freeze traversal.
    ///
    /// The 64MiB claim is shared across nested traversals, without refunds. Charge
    /// checked growth forecasts before allocating; this is not an allocator fence.
    pub fn charge_traversal_scratch(&self, additional_peak: usize) -> FreezeResult<()> {
        self.traversal.charge_scratch(additional_peak)
    }

    /// Guard a complete cooperative traversal, including forecasts and final fields.
    ///
    /// Every error poisons this freezer. The closure runs without a held state
    /// borrow and may recursively freeze values using this same freezer.
    pub fn with_traversal<T>(&self, f: impl FnOnce() -> FreezeResult<T>) -> FreezeResult<T> {
        self.traversal.check()?;
        match f() {
            Ok(value) => {
                self.traversal.check()?;
                Ok(value)
            }
            Err(error) => {
                self.traversal.poison();
                Err(error)
            }
        }
    }

    /// Freeze a nested value while freezing yourself.
    ///
    /// After an error this freezer is unusable, including for forwarded values.
    pub fn freeze(&self, value: Value) -> FreezeResult<FrozenValue> {
        self.with_traversal(|| {
            self.charge_execution(1, 0)?;
            if let Some(x) = value.unpack_frozen() {
                return Ok(x);
            }
            let value = value.0.unpack_ptr().unwrap();
            match value.unpack() {
                AValueOrForwardUnpack::Forward(x) => {
                    Ok(unsafe { x.forward_ptr().unpack_frozen_value() })
                }
                AValueOrForwardUnpack::Header(v) => {
                    let value = v.unpack();
                    let _depth = if let Some(budget) = self.budget {
                        // Only these concrete owners have audited freeze work and
                        // allocations. Check before virtual Freeze/direct-freeze.
                        let admitted = value
                            .downcast_ref::<Def>()
                            .is_some_and(Def::deferred_for_freeze)
                            || value.downcast_ref::<ValueCaptured>().is_some();
                        if !admitted {
                            return Err(self.refuse_execution(
                                ExecutionLimit::UnsupportedOperation {
                                    operation: "freeze",
                                    value_type: value.vtable().type_name,
                                },
                            ));
                        }
                        let depth = self.depth.get();
                        if depth >= budget.text_depth().min(MAX_FREEZE_DEPTH) {
                            return Err(self.refuse_execution(ExecutionLimit::FreezeDepth));
                        }
                        self.depth.set(depth + 1);
                        Some(RestoreDepth(&self.depth, depth))
                    } else {
                        None
                    };
                    unsafe { value.heap_freeze(self) }
                }
            }
        })
    }

    /// Frozen heap where the values are frozen to.
    ///
    /// Can be used to allocate additional values while freezing.
    pub fn frozen_heap(&self) -> &'fv FrozenHeap {
        self.heap
    }
}
