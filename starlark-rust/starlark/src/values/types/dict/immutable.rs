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

//! Shallow immutable dictionaries with evaluator-owned elements.

use std::fmt;

use allocative::Allocative;

use super::value::Dict;
use super::value::DictGen;
use super::value::DictLike;
use super::value::FrozenDictData;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::collections::StarlarkHasher;
use crate::typing::Ty;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::Heap;
use crate::values::Trace;
use crate::values::Value;
use crate::values::error::ControlError;
use crate::values::error::ValueError;
use crate::values::type_repr::StarlarkTypeRepr;

#[derive(Debug, Trace, ProvidesStaticType, Allocative)]
pub(crate) struct ImmutableDictData<'v>(pub(crate) Dict<'v>);

impl fmt::Display for ImmutableDictData<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<'v> Freeze for DictGen<ImmutableDictData<'v>> {
    type Frozen = DictGen<FrozenDictData>;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(DictGen(FrozenDictData {
            content: self.0.0.content.freeze(freezer)?,
        }))
    }
}

impl<'v> DictLike<'v> for ImmutableDictData<'v> {
    type ContentRef<'a>
        = &'a SmallMap<Value<'v>, Value<'v>>
    where
        Self: 'a,
        'v: 'a;
    fn content<'a>(&'a self) -> Self::ContentRef<'a> {
        &self.0.content
    }
    unsafe fn iter_start(&self) {}
    unsafe fn iter_stop(&self) {}
    unsafe fn content_unchecked(&self) -> &SmallMap<Value<'v>, Value<'v>> {
        &self.0.content
    }
    fn set_at(&self, _index: Hashed<Value<'v>>, _value: Value<'v>) -> crate::Result<()> {
        Err(crate::Error::new_other(
            ValueError::CannotMutateImmutableValue,
        ))
    }
    fn write_hash(&self, _hasher: &mut StarlarkHasher) -> crate::Result<()> {
        Err(crate::Error::new_other(ControlError::NotHashableValue(
            Dict::TYPE.to_owned(),
        )))
    }
}

/// Allocate a shallow immutable, directly unhashable dictionary.
/// Keys must already be hashable, just as with `AllocDict`.
pub struct AllocImmutableDict<D>(pub D);

impl<D, K, V> StarlarkTypeRepr for AllocImmutableDict<D>
where
    D: IntoIterator<Item = (K, V)>,
    K: StarlarkTypeRepr,
    V: StarlarkTypeRepr,
{
    type Canonical = <super::AllocDict<D> as StarlarkTypeRepr>::Canonical;
    fn starlark_type_repr() -> Ty {
        super::AllocDict::<D>::starlark_type_repr()
    }
}

impl<'v, D, K, V> AllocValue<'v> for AllocImmutableDict<D>
where
    D: IntoIterator<Item = (K, V)>,
    K: AllocValue<'v>,
    V: AllocValue<'v>,
{
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        let iter = self.0.into_iter();
        let mut content = SmallMap::with_capacity(iter.size_hint().0);
        for (key, value) in iter {
            content.insert_hashed(heap.alloc(key).get_hashed().unwrap(), heap.alloc(value));
        }
        heap.alloc_complex(DictGen(ImmutableDictData(Dict::new(content))))
    }
}
