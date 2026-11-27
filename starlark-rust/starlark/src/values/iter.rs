/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use std::fmt::Debug;

use crate::eval::ExecutionLimit;
use crate::typing::TyStarlarkValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;
use crate::values::dict::DictRef;
use crate::values::int::inline_int::InlineInt;
use crate::values::list::ListRef;
use crate::values::range::Range;
use crate::values::set::SetRef;
use crate::values::tuple::TupleRef;

impl<'v> Heap<'v> {
    /// Admit iterator startup on types with fixed-work, nonallocating steps.
    /// The caller must charge each step before advancing the iterator.
    pub(crate) fn bounded_iterator(
        self,
        value: Value<'v>,
        operation: &'static str,
    ) -> crate::Result<StarlarkIterator<'v>> {
        if let Some(budget) = self.execution_budget() {
            budget.charge(1, 0)?;
            if !TyStarlarkValue::is_iterable(&value.vtable().starlark_value) {
                return ValueError::unsupported_type(value, "(iter)");
            }
            let admitted = ListRef::from_value(value).is_some()
                || TupleRef::from_value(value).is_some()
                || DictRef::from_value(value).is_some()
                || SetRef::unpack_value_opt(value).is_some()
                // A range step emits an i32. Admit it when that is stored inline.
                || (!InlineInt::smaller_than_i32() && value.downcast_ref::<Range>().is_some());
            if !admitted {
                return Err(budget
                    .refuse(ExecutionLimit::UnsupportedOperation {
                        operation,
                        value_type: value.get_type(),
                    })
                    .into());
            }
        }
        value.iterate(self)
    }
}

/// Iterator of starlark values.
#[derive(Debug)]
pub struct StarlarkIterator<'v> {
    /// Iterator implementation. Typically an iterable itself.
    value: Value<'v>,
    /// Current index.
    index: usize,
    /// Heap to allocate values on.
    heap: Heap<'v>,
}

impl<'v> StarlarkIterator<'v> {
    /// Construct iterator from the given value.
    #[inline]
    pub(crate) fn new(value: Value<'v>, heap: Heap<'v>) -> StarlarkIterator<'v> {
        StarlarkIterator {
            value,
            index: 0,
            heap,
        }
    }

    /// Iterator yielding no values.
    #[inline]
    pub fn empty(heap: Heap<'v>) -> StarlarkIterator<'v> {
        Self::new(Value::new_empty_tuple(), heap)
    }
}

impl<'v> Iterator for StarlarkIterator<'v> {
    type Item = Value<'v>;

    #[inline]
    fn next(&mut self) -> Option<Value<'v>> {
        let r = self.value.get_ref().iter_next(self.index, self.heap);
        if r.is_some() {
            self.index += 1;
        } else {
            self.value.get_ref().iter_stop();
            // We must call `iter_stop` exactly once, regardless of whether
            // iterator is exhausted or not, even if `next` is called after `None`.
            // So we replace `value` with empty tuple, for which we know that `iter_stop` is no-op.
            self.value = Value::new_empty_tuple();
            self.index = 0;
        }
        r
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.value.get_ref().iter_size_hint(self.index)
    }
}

impl<'v> Drop for StarlarkIterator<'v> {
    #[inline]
    fn drop(&mut self) {
        // `iter_stop` is no-op for empty tuple, this saves us from virtual call
        // after iterator is exhausted.
        if !self.value.ptr_eq(Value::new_empty_tuple()) {
            self.value.get_ref().iter_stop();
        }
    }
}
