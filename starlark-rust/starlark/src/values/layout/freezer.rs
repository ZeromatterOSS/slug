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

use std::cell::RefCell;

use crate::eval::compiler::def::FrozenDef;
use crate::values::AllocFrozenValue;
use crate::values::FreezeResult;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::layout::avalue::AValue;
use crate::values::layout::heap::arena::Reservation;
use crate::values::layout::heap::repr::AValueOrForwardUnpack;
use crate::values::layout::value::FrozenValue;
use crate::values::layout::value::Value;

mod traversal;

#[cfg(test)]
mod traversal_tests;

use traversal::FreezeTraversalState;

/// Used to `freeze` values by [`Freeze::freeze`](crate::values::Freeze::freeze).
pub struct Freezer<'fv> {
    /// Freezing into this heap.
    pub(crate) heap: &'fv FrozenHeap,
    /// Defs frozen by this freezer.
    pub(crate) frozen_defs: RefCell<Vec<FrozenRef<'static, FrozenDef>>>,
    traversal: FreezeTraversalState,
}

impl<'fv> Freezer<'fv> {
    pub(crate) fn new(heap: &'fv FrozenHeap) -> Self {
        Freezer {
            heap,
            frozen_defs: RefCell::new(Vec::new()),
            traversal: FreezeTraversalState::default(),
        }
    }

    /// Allocate a new value while freezing. Usually not a great idea.
    pub fn alloc<'v, T: AllocFrozenValue>(&'v self, val: T) -> FrozenValue {
        val.alloc_frozen_value(self.heap)
    }

    pub(crate) fn reserve<'v, 'v2, T: AValue<'v2, ExtraElem = ()>>(
        &'v self,
    ) -> (FrozenValue, Reservation<'v2, T>) {
        let (fv, r, extra) = self.heap.reserve_with_extra::<T>(0);
        let extra = unsafe { &mut *extra };
        debug_assert!(extra.is_empty());
        (fv, r)
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
            if let Some(x) = value.unpack_frozen() {
                return Ok(x);
            }
            let value = value.0.unpack_ptr().unwrap();
            match value.unpack() {
                AValueOrForwardUnpack::Forward(x) => {
                    Ok(unsafe { x.forward_ptr().unpack_frozen_value() })
                }
                AValueOrForwardUnpack::Header(v) => unsafe { v.unpack().heap_freeze(self) },
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
