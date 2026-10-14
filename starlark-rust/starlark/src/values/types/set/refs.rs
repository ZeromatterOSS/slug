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

use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::convert::Infallible;

use dupe::Dupe;
use either::Either;

use super::value::FrozenSetData;
use super::value::SetData;
use crate::coerce::coerce;
use crate::typing::Ty;
use crate::values::FrozenValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::layout::value::ValueLike;
use crate::values::set::value::SetGen;
use crate::values::type_repr::SetType;
use crate::values::type_repr::StarlarkTypeRepr;

/// Define the set type.
pub struct SetRef<'v> {
    pub(crate) aref: Either<Ref<'v, SetData<'v>>, &'v SetData<'v>>,
}

impl<'v> SetRef<'v> {
    /// Number of elements in the borrowed set.
    pub fn len(&self) -> usize {
        self.aref.content.len()
    }

    /// Borrow elements in their existing iteration order without rehashing.
    pub fn iter(&self) -> impl Iterator<Item = Value<'v>> + '_ {
        self.aref.content.iter().copied()
    }
}

impl<'v> Clone for SetRef<'v> {
    fn clone(&self) -> Self {
        match &self.aref {
            Either::Left(x) => SetRef {
                aref: Either::Left(Ref::clone(x)),
            },
            Either::Right(x) => SetRef {
                aref: Either::Right(*x),
            },
        }
    }
}

impl<'v> Dupe for SetRef<'v> {}

/// Mutably borrowed `Set`.
pub struct SetMut<'v> {
    pub(crate) aref: RefMut<'v, SetData<'v>>,
}

impl<'v> SetMut<'v> {
    /// Downcast the value to a mutable set reference.
    #[inline]
    pub fn from_value(x: Value<'v>) -> anyhow::Result<SetMut<'v>> {
        #[derive(thiserror::Error, Debug)]
        #[error("Value is not set, value type: `{0}`")]
        struct NotSetError(&'static str);

        #[cold]
        #[inline(never)]
        fn error<'v>(x: Value<'v>) -> anyhow::Error {
            if x.downcast_ref::<SetGen<FrozenSetData>>().is_some() {
                ValueError::CannotMutateImmutableValue.into()
            } else {
                NotSetError(x.get_type()).into()
            }
        }

        let ptr = x.downcast_ref::<SetGen<RefCell<SetData<'v>>>>();
        match ptr {
            None => Err(error(x)),
            Some(ptr) => match ptr.0.try_borrow_mut() {
                Ok(x) => Ok(SetMut { aref: x }),
                Err(_) => Err(ValueError::MutationDuringIteration.into()),
            },
        }
    }
}

impl<'v> StarlarkTypeRepr for SetRef<'v> {
    type Canonical = <SetType<FrozenValue> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        <Self::Canonical as StarlarkTypeRepr>::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for SetRef<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<SetRef<'v>>, Infallible> {
        let result = if let Some(value) = value.unpack_frozen() {
            value
                .downcast_ref::<SetGen<FrozenSetData>>()
                .map(|x| SetRef {
                    aref: Either::Right(coerce(&x.0)),
                })
        } else {
            value
                .downcast_ref::<SetGen<RefCell<SetData<'v>>>>()
                .map(|ptr| SetRef {
                    aref: Either::Left(ptr.0.borrow()),
                })
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;

    fn check<'v>(values: Value<'v>, first: Value<'v>, second: Value<'v>, empty: Value<'v>) {
        let view = SetRef::unpack_value_opt(values).unwrap();
        assert_eq!(view.len(), 2);
        let mut elements = view.iter();
        assert!(elements.next().unwrap().ptr_eq(second));
        assert!(elements.next().unwrap().ptr_eq(first));
        assert!(elements.next().is_none());
        let empty = SetRef::unpack_value_opt(empty).unwrap();
        assert_eq!(empty.len(), 0);
        assert!(empty.iter().next().is_none());
    }

    #[test]
    fn borrowed_set_view_preserves_element_order_and_aliases_through_freeze() {
        let module = Module::new();
        let globals = GlobalsBuilder::standard()
            .with(super::super::set::register_set)
            .build();
        let mut eval = Evaluator::new(&module);
        eval.eval_module(
            AstModule::parse(
                "set-view.bzl",
                "first = ('one', 1)\nsecond = ('two', 2)\nvalues = set([second, first, second])\nempty = set()\n".into(),
                &Dialect::Standard,
            )
            .unwrap(),
            &globals,
        )
        .unwrap();
        drop(eval);
        let before = module.heap().allocated_bytes();
        check(
            module.get("values").unwrap(),
            module.get("first").unwrap(),
            module.get("second").unwrap(),
            module.get("empty").unwrap(),
        );
        assert_eq!(module.heap().allocated_bytes(), before);
        let values = module.get("values").unwrap();
        let borrowed = SetRef::unpack_value_opt(values).unwrap();
        assert!(SetMut::from_value(values).is_err());
        drop(borrowed);
        assert!(SetMut::from_value(values).is_ok());
        assert!(SetRef::unpack_value_opt(Value::new_none()).is_none());
        let frozen = module.freeze().unwrap();
        let values = frozen.get("values").unwrap();
        let first = frozen.get("first").unwrap();
        let second = frozen.get("second").unwrap();
        let empty = frozen.get("empty").unwrap();
        check(values.value(), first.value(), second.value(), empty.value());
    }
}
