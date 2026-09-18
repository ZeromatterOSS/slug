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

//! Shallow immutable lists whose elements may still belong to an evaluator heap.

use std::fmt;

use allocative::Allocative;

use super::value::ListGen;
use super::value::ListLike;
use super::value::display_list;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::coerce;
use crate::typing::Ty;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::error::ValueError;
use crate::values::type_repr::StarlarkTypeRepr;

#[derive(Debug, Trace, ProvidesStaticType, Allocative)]
pub(crate) struct ImmutableListData<V>(pub(crate) Box<[V]>);

impl Freeze for ListGen<ImmutableListData<Value<'_>>> {
    type Frozen = ListGen<ImmutableListData<FrozenValue>>;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(ListGen(ImmutableListData(self.0.0.freeze(freezer)?)))
    }
}

impl<'v, V: ValueLike<'v>> fmt::Display for ImmutableListData<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_list(coerce(&*self.0), f)
    }
}

impl<'v, V: ValueLike<'v>> ListLike<'v> for ImmutableListData<V> {
    fn content(&self) -> &[Value<'v>] {
        coerce(&*self.0)
    }
    fn set_at(&self, _i: usize, _v: Value<'v>) -> crate::Result<()> {
        Err(crate::Error::new_other(
            ValueError::CannotMutateImmutableValue,
        ))
    }
    unsafe fn new_iter(&self, me: Value<'v>) -> Value<'v> {
        me
    }
    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        let remaining = self.0.len() - index;
        (remaining, Some(remaining))
    }
    unsafe fn iter_next(&self, index: usize) -> Option<Value<'v>> {
        self.0.get(index).map(|v| v.to_value())
    }
    unsafe fn iter_stop(&self) {}
}

/// Allocate a shallow immutable, directly unhashable list.
/// Nested values keep their identity and mutability until module freeze.
pub struct AllocImmutableList<L>(pub L);

impl<L> StarlarkTypeRepr for AllocImmutableList<L>
where
    L: IntoIterator,
    L::Item: StarlarkTypeRepr,
{
    type Canonical = <Vec<L::Item> as StarlarkTypeRepr>::Canonical;
    fn starlark_type_repr() -> Ty {
        Vec::<L::Item>::starlark_type_repr()
    }
}

impl<'v, L> AllocValue<'v> for AllocImmutableList<L>
where
    L: IntoIterator,
    L::Item: AllocValue<'v>,
{
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        let values = self.0.into_iter().map(|v| heap.alloc(v)).collect();
        heap.alloc_complex(ListGen(ImmutableListData::<Value<'v>>(values)))
    }
}
