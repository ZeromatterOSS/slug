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
use crate::values::FrozenHeap;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::HeapSendable;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::dict::Dict;
use crate::values::dict::value::DictGen;
use crate::values::dict::value::FrozenDictData;
use crate::values::dict::value::MutableDict;
use crate::values::function::BoundMethodGen;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::avalue::heap_copy_deferred;
use crate::values::layout::avalue::try_freeze_directly;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::heap::repr::ForwardPtr;
use crate::values::structs::StructuralIdentity;
use crate::values::structs::StructuralIdentityNative;
use crate::values::structs::value::FrozenStruct;

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

impl FrozenHeap {
    /// Build a native frozen dictionary from admitted rows. Equal keys keep
    /// the original key and position and receive the last value. The caller
    /// pays for producing each row and attaches any foreign frozen owners.
    pub fn alloc_budgeted_dict_from_rows<'v>(
        &'v self,
        budget: &ExecutionBudget,
        count: usize,
        rows: impl IntoIterator<Item = crate::Result<(FrozenValue, FrozenValue)>>,
        native: &impl StructuralIdentityNative<'v>,
    ) -> crate::Result<FrozenValue> {
        budget.check()?;
        let mut hash = StructuralIdentity::new(budget, native);
        let mut equals = StructuralIdentity::new(budget, native);
        let rows = rows.into_iter().map(|row| -> crate::Result<_> {
            let (key, value) = row?;
            Ok((
                Hashed::new_unchecked(hash.small_hash(key.to_value())?, key),
                value,
            ))
        });
        let rows = SmallMap::try_from_hashed_rows(
            count,
            rows,
            |existing, incoming| {
                // Ordinary SmallMap insertion compares the incoming key first.
                // FrozenValue::eq treats an ordinary comparison error as false.
                let equal = equals.equals(incoming.to_value(), existing.to_value());
                budget.check()?;
                Ok(equal.unwrap_or(false))
            },
            |work, bytes| Ok(budget.charge(work, bytes)?),
        );
        let content = bounded_map_result(rows, budget)?;
        if content.is_empty() {
            return Ok(FrozenValue::new_empty_dict());
        }
        Ok(self.alloc_budgeted_simple(budget, || DictGen(FrozenDictData { content }))?)
    }

    /// Build a native frozen struct in source field order. The caller pays
    /// for producing each row and attaches any foreign frozen owners.
    pub fn alloc_budgeted_struct_from_rows(
        &self,
        budget: &ExecutionBudget,
        count: usize,
        rows: impl IntoIterator<Item = crate::Result<(FrozenStringValue, FrozenValue)>>,
    ) -> crate::Result<FrozenValue> {
        let fields = bounded_string_map(count, rows, budget, || {
            Err(crate::Error::new_other(anyhow::anyhow!(
                "duplicate struct field"
            )))
        })?;
        Ok(self.alloc_budgeted_simple(budget, || FrozenStruct::new(fields))?)
    }
}

fn bounded_string_map<'v, K: StringValueLike<'v>, V>(
    count: usize,
    rows: impl IntoIterator<Item = crate::Result<(K, V)>>,
    budget: &ExecutionBudget,
    mut equal_key: impl FnMut() -> crate::Result<bool>,
) -> crate::Result<SmallMap<K, V>> {
    let rows = rows.into_iter().map(|row| -> crate::Result<_> {
        let (key, value) = row?;
        budget.charge(1 + ExecutionBudget::text_work(key.as_str().len()), 0)?;
        let hash = key.to_string_value().get_hashed().hash();
        Ok((Hashed::new_unchecked(hash, key), value))
    });
    let rows = SmallMap::try_from_hashed_rows(
        count,
        rows,
        |left, right| {
            let left = left.as_str();
            let right = right.as_str();
            if left.len() != right.len() {
                return Ok(false);
            }
            budget.charge_text_work(left.len())?;
            if left == right {
                equal_key()
            } else {
                Ok(false)
            }
        },
        |work, bytes| Ok(budget.charge(work, bytes)?),
    );
    bounded_map_result(rows, budget)
}

fn bounded_map_result<K, V>(
    rows: Result<SmallMap<K, V>, BoundedMapError<impl Into<crate::Error>>>,
    budget: &ExecutionBudget,
) -> crate::Result<SmallMap<K, V>> {
    budget.check()?;
    match rows {
        Ok(rows) => Ok(rows),
        Err(BoundedMapError::Operation(error)) => Err(error.into()),
        Err(BoundedMapError::SizeOverflow) => {
            Err(budget.refuse(ExecutionLimit::AllocationBytes).into())
        }
        Err(BoundedMapError::AllocationFailed) => {
            Err(budget.refuse(ExecutionLimit::AllocationFailed).into())
        }
        Err(BoundedMapError::RowCountMismatch) => {
            Err(crate::Error::new_kind(crate::ErrorKind::Internal(
                anyhow::anyhow!("validated dictionary or struct row count changed"),
            )))
        }
    }
}

impl<'v> Heap<'v> {
    pub(crate) fn complex_allocation_bytes<T>() -> usize
    where
        T: ComplexValue<'v>,
        T::Frozen: StarlarkValue<'static>,
    {
        AValueComplex::<T>::alloc_size_for_extra_len(0).bytes() as usize
    }

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
        self.alloc_budgeted_dict_from_map(bounded_map_result(rows, budget)?, budget)
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
        let rows = bounded_string_map(count, rows, budget, || Ok(true))?;
        self.alloc_budgeted_dict_from_map(coerce(rows), budget)
    }

    fn alloc_budgeted_dict_from_map(
        self,
        rows: SmallMap<Value<'v>, Value<'v>>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Value<'v>> {
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

#[cfg(test)]
#[path = "budgeted_frozen_containers_tests.rs"]
mod budgeted_frozen_containers_tests;
