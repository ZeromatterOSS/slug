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

use std::any::TypeId;
use std::any::type_name;
use std::marker::PhantomData;
use std::mem;

use starlark_map::Hashed;
use starlark_map::small_map::BoundedMapError;
use starlark_map::small_map::SmallMap;

use super::simple::AValueSimple;
use crate::coerce::coerce;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::private::Private;
use crate::values::ComplexValue;
use crate::values::FreezeError;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::HeapSendable;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::dict::Dict;
use crate::values::dict::value::MutableDict;
use crate::values::function::BoundMethodGen;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::avalue::heap_copy_deferred;
use crate::values::layout::avalue::try_freeze_directly;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::heap::repr::ForwardPtr;

#[derive(Debug, thiserror::Error)]
enum AValueError {
    #[error("Value of type `{0}` cannot be frozen")]
    CannotBeFrozen(&'static str),
}

struct AValueComplex<T>(PhantomData<T>);

impl<'v, T> AValue<'v> for AValueComplex<T>
where
    T: ComplexValue<'v>,
    T::Frozen: StarlarkValue<'static>,
{
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

            let (fv, r) = freezer.reserve::<AValueSimple<T::Frozen>>()?;
            let x = AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
                me,
                ForwardPtr::new_frozen(fv),
            );
            let res = x.freeze(freezer)?;
            r.fill(res);
            if TypeId::of::<T::Frozen>() == TypeId::of::<FrozenDef>() {
                let frozen_def = fv.downcast_frozen_ref().unwrap();
                freezer.register_frozen_def(frozen_def)?;
            }
            Ok(fv)
        }
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        unsafe { heap_copy_deferred::<Self>(me, tracer) }
    }
}

pub(crate) struct AValueComplexNoFreeze<T>(PhantomData<T>);

impl<'v, T> AValue<'v> for AValueComplexNoFreeze<T>
where
    T: StarlarkValue<'v> + Trace<'v>,
{
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self::StarlarkValue>()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        Err(FreezeError::new(
            AValueError::CannotBeFrozen(type_name::<T>()).to_string(),
        ))
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        unsafe { heap_copy_deferred::<Self>(me, tracer) }
    }
}

impl<'v> Heap<'v> {
    pub(crate) fn alloc_budgeted_bound_method(
        self,
        method: BoundMethodGen<Value<'v>>,
        budget: &ExecutionBudget,
    ) -> Result<Value<'v>, ExecutionLimit> {
        let bytes = AValueComplex::<BoundMethodGen<Value<'v>>>::alloc_size_for_extra_len(0).bytes();
        budget.charge(1, bytes as usize)?;
        Ok(self.alloc_complex(method))
    }

    /// Admit the map and mutable dictionary from admitted unique, prehashed rows.
    pub(crate) fn alloc_budgeted_dict_from_unique_rows(
        self,
        count: usize,
        rows: impl IntoIterator<Item = (Hashed<Value<'v>>, Value<'v>)>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Value<'v>> {
        let rows = SmallMap::try_from_unique_hashed_rows(count, rows, |work, bytes| {
            budget.charge(work, bytes)
        });
        self.alloc_budgeted_dict_from_map(rows, budget)
    }

    /// Admit a mutable Starlark dictionary from exactly `count` trusted rows.
    /// Equal string keys keep the first key and slot and receive the last value.
    /// The caller admits production of each key and value. This operation admits
    /// the map, row advances, hashing, equality, and the final arena object.
    pub fn alloc_budgeted_str_dict_from_rows(
        self,
        count: usize,
        rows: impl IntoIterator<Item = crate::Result<(StringValue<'v>, Value<'v>)>>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Value<'v>> {
        let rows = rows.into_iter().map(|row| -> crate::Result<_> {
            let (key, value) = row?;
            budget.charge(1 + ExecutionBudget::text_work(key.len()), 0)?;
            Ok((key.get_hashed(), value))
        });
        let rows = SmallMap::try_from_hashed_rows(
            count,
            rows,
            |left, right| {
                if left.len() != right.len() {
                    return Ok(false);
                }
                budget.charge_text_work(left.len())?;
                Ok(left.as_str() == right.as_str())
            },
            |work, bytes| Ok(budget.charge(work, bytes)?),
        );
        self.alloc_budgeted_dict_from_map(rows.map(coerce), budget)
    }

    fn alloc_budgeted_dict_from_map(
        self,
        rows: Result<SmallMap<Value<'v>, Value<'v>>, BoundedMapError<impl Into<crate::Error>>>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Value<'v>> {
        let rows = match rows {
            Ok(rows) => rows,
            Err(BoundedMapError::Operation(error)) => return Err(error.into()),
            Err(BoundedMapError::SizeOverflow) => {
                return Err(budget.refuse(ExecutionLimit::AllocationBytes).into());
            }
            Err(BoundedMapError::AllocationFailed) => {
                return Err(budget.refuse(ExecutionLimit::AllocationFailed).into());
            }
            Err(BoundedMapError::RowCountMismatch) => {
                return Err(crate::Error::new_kind(crate::ErrorKind::Internal(
                    anyhow::anyhow!("validated dictionary row count changed"),
                )));
            }
        };
        let bytes = AValueComplex::<MutableDict<'v>>::alloc_size_for_extra_len(0).bytes();
        budget.charge(1, bytes as usize)?;
        Ok(self.alloc(Dict::new(rows)))
    }

    /// Allocate a [`ComplexValue`] on the [`Heap`].
    pub fn alloc_complex<T>(self, x: T) -> Value<'v>
    where
        T: ComplexValue<'v>,
        T::Frozen: StarlarkValue<'static>,
        T: HeapSendable<'v>,
    {
        assert!(!T::is_special(Private));
        self.alloc_raw(AValueImpl::<AValueComplex<T>>::new(x))
            .to_value()
    }

    /// Allocate a value which can be traced (garbage collected), but cannot be frozen.
    pub fn alloc_complex_no_freeze<T>(self, x: T) -> Value<'v>
    where
        T: StarlarkValue<'v> + Trace<'v>,
        T: HeapSendable<'v>,
    {
        assert!(!T::is_special(Private));
        // When specializations are stable, we can have single `alloc_complex` function,
        // which enables or not enables freezing depending on whether `T` implements `Freeze`.
        self.alloc_raw(AValueImpl::<AValueComplexNoFreeze<T>>::new(x))
            .to_value()
    }
}

#[cfg(test)]
#[path = "budgeted_str_dict_tests.rs"]
mod budgeted_str_dict_tests;
