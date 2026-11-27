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

use std::ptr::copy_nonoverlapping;

use starlark_map::Hashed;

use crate::collections::StarlarkHashValue;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::StringValueLike as _;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::constant_string;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::heap::repr::ForwardPtr;
use crate::values::string::str_type::StarlarkStr;

pub(crate) const VALUE_STR_A_VALUE_PTR: AValueHeader =
    AValueHeader::new_const::<StarlarkStrAValue>();

#[inline]
pub(crate) fn starlark_str<'v>(
    len: usize,
    hash: StarlarkHashValue,
) -> AValueImpl<'v, impl AValue<'v, ExtraElem = usize> + Send + Sync> {
    AValueImpl::<StarlarkStrAValue>::new(unsafe { StarlarkStr::new(len, hash) })
}

pub(crate) struct StarlarkStrAValue;

impl<'v> AValue<'v> for StarlarkStrAValue {
    type StarlarkValue = StarlarkStr;

    type ExtraElem = usize;

    fn extra_len(value: &StarlarkStr) -> usize {
        StarlarkStr::payload_len_for_len(value.len())
    }

    fn offset_of_extra() -> usize {
        StarlarkStr::offset_of_content()
    }

    const IS_STR: bool = true;

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        unsafe {
            debug_assert!(
                (*me).payload.len() > 1,
                "short strings are allocated statically"
            );

            let s = (*me).payload.as_str();
            let fv = freezer.alloc(s);
            debug_assert!(fv.is_str());
            AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
                me,
                ForwardPtr::new_frozen(fv),
            );
            Ok(fv)
        }
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        unsafe {
            debug_assert!(
                (*me).payload.len() > 1,
                "short strings are allocated statically"
            );

            let s = (*me).payload.as_str();
            let v = tracer.alloc_str(s);
            debug_assert!(v.is_str());
            AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
                me,
                ForwardPtr::new_unfrozen(v),
            );
            v
        }
    }
}

impl FrozenHeap {
    /// Allocate a string on this heap. Be careful about the warnings around
    /// [`FrozenValue`].
    ///
    /// Since the heap is frozen, we always prefer to intern the string in order
    /// to deduplicate it and save some memory.
    pub fn alloc_str(&self, x: &str) -> FrozenStringValue {
        self.alloc_str_intern(x)
    }

    /// Admit one direct frozen string copy before allocation. This path uses
    /// the static short strings and avoids an interner probe or allocation.
    pub fn alloc_budgeted_str(
        &self,
        budget: &ExecutionBudget,
        text: &str,
    ) -> Result<FrozenStringValue, ExecutionLimit> {
        budget.charge(1, 0)?;
        if let Some(value) = constant_string(text) {
            return Ok(value);
        }
        let payload = StarlarkStr::payload_len_for_len(text.len());
        let bytes = Self::checked_chunk_value_size::<StarlarkStrAValue>(payload)
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?
            .bytes() as usize;
        budget.charge(ExecutionBudget::text_work(text.len()), bytes)?;
        Ok(
            self.alloc_str_init(text.len(), StarlarkStr::UNINIT_HASH, |dest| {
                // SAFETY: The fresh allocation has space for exactly this UTF-8
                // slice and cannot overlap it.
                unsafe { copy_nonoverlapping(text.as_ptr(), dest, text.len()) }
            }),
        )
    }

    /// Intern string.
    pub(crate) fn alloc_str_intern(&self, s: &str) -> FrozenStringValue {
        self.alloc_str_hashed(Hashed::new(s))
    }

    /// Allocate prehashed string.
    pub fn alloc_str_hashed(&self, s: Hashed<&str>) -> FrozenStringValue {
        if let Some(s) = constant_string(*s) {
            s
        } else {
            self.string_interner().intern(s, || {
                self.alloc_str_init(s.len(), s.hash(), |dest| unsafe {
                    copy_nonoverlapping(s.as_ptr(), dest, s.len())
                })
            })
        }
    }
}

fn checked_string_allocation(
    len: usize,
    budget: &ExecutionBudget,
) -> Result<usize, ExecutionLimit> {
    let payload = StarlarkStr::payload_len_for_len(len);
    StarlarkStrAValue::checked_alloc_size_for_extra_len(payload)
        .map(|size| size.bytes() as usize)
        .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))
}

impl<'v> Heap<'v> {
    /// Charge the work and aligned object bytes for a native result string
    /// before copying it to this heap.
    pub fn alloc_budgeted_str(self, text: &str) -> Result<StringValue<'v>, ExecutionLimit> {
        let Some(budget) = self.execution_budget() else {
            return Ok(self.alloc_str(text));
        };
        budget.check()?;
        if let Some(value) = constant_string(text) {
            return Ok(value.to_string_value());
        }
        let bytes = checked_string_allocation(text.len(), budget)?;
        budget.charge(1 + ExecutionBudget::text_work(text.len()), bytes)?;
        Ok(self.alloc_str(text))
    }

    /// Allocate the concatenated result directly in the heap, charging for each
    /// pass over the parts and the aligned result before touching either.
    pub fn alloc_budgeted_str_parts(
        self,
        parts: &[&str],
    ) -> Result<StringValue<'v>, ExecutionLimit> {
        let budget = self.execution_budget();
        if let Some(budget) = budget {
            budget.charge(parts.len(), 0)?;
        }
        let overflow = || {
            budget.map_or(ExecutionLimit::AllocationBytes, |budget| {
                budget.refuse(ExecutionLimit::AllocationBytes)
            })
        };
        let mut len = 0usize;
        let mut last_nonempty = "";
        for &part in parts {
            len = len.checked_add(part.len()).ok_or_else(overflow)?;
            if !part.is_empty() {
                last_nonempty = part;
            }
        }
        if len <= 1 {
            return Ok(self.alloc_str(last_nonempty));
        }

        let payload = StarlarkStr::payload_len_for_len(len);
        let bytes = StarlarkStrAValue::checked_alloc_size_for_extra_len(payload)
            .ok_or_else(overflow)?
            .bytes() as usize;
        if let Some(budget) = budget {
            let work = parts
                .len()
                .checked_add(1 + ExecutionBudget::text_work(len))
                .ok_or_else(|| budget.refuse(ExecutionLimit::Work))?;
            budget.charge(work, bytes)?;
        }
        Ok(
            self.alloc_str_init(len, StarlarkStr::UNINIT_HASH, |mut dest| {
                for part in parts {
                    // SAFETY: The checked length is the sum of the UTF-8 parts.
                    // The fresh allocation is disjoint from all input slices.
                    unsafe {
                        copy_nonoverlapping(part.as_ptr(), dest, part.len());
                        dest = dest.add(part.len());
                    }
                }
            }),
        )
    }

    /// Allocate a string on the heap.
    pub fn alloc_str(self, x: &str) -> StringValue<'v> {
        if let Some(x) = constant_string(x) {
            x.to_string_value()
        } else {
            self.alloc_str_init(x.len(), StarlarkStr::UNINIT_HASH, |dest| unsafe {
                copy_nonoverlapping(x.as_ptr(), dest, x.len())
            })
        }
    }

    /// Intern string.
    pub fn alloc_str_intern(self, x: &str) -> StringValue<'v> {
        if let Some(x) = constant_string(x) {
            x.to_string_value()
        } else {
            let x = Hashed::new(x);
            self.string_interner().intern(x, || {
                self.alloc_str_init(x.len(), x.hash(), |dest| unsafe {
                    copy_nonoverlapping(x.as_ptr(), dest, x.len())
                })
            })
        }
    }

    /// Allocate a string on the heap, based on two concatenated strings.
    pub fn alloc_str_concat(self, x: &str, y: &str) -> StringValue<'v> {
        if x.is_empty() {
            self.alloc_str(y)
        } else if y.is_empty() {
            self.alloc_str(x)
        } else {
            self.alloc_str_init(x.len() + y.len(), StarlarkStr::UNINIT_HASH, |dest| unsafe {
                copy_nonoverlapping(x.as_ptr(), dest, x.len());
                copy_nonoverlapping(y.as_ptr(), dest.add(x.len()), y.len())
            })
        }
    }

    /// Allocate a string on the heap, based on three concatenated strings.
    pub fn alloc_str_concat3(self, x: &str, y: &str, z: &str) -> StringValue<'v> {
        if x.is_empty() {
            self.alloc_str_concat(y, z)
        } else if y.is_empty() {
            self.alloc_str_concat(x, z)
        } else if z.is_empty() {
            self.alloc_str_concat(x, y)
        } else {
            self.alloc_str_init(
                x.len() + y.len() + z.len(),
                StarlarkStr::UNINIT_HASH,
                |dest| unsafe {
                    copy_nonoverlapping(x.as_ptr(), dest, x.len());
                    let dest = dest.add(x.len());
                    copy_nonoverlapping(y.as_ptr(), dest, y.len());
                    let dest = dest.add(y.len());
                    copy_nonoverlapping(z.as_ptr(), dest, z.len());
                },
            )
        }
    }

    pub(crate) fn alloc_char(self, x: char) -> StringValue<'v> {
        let mut dst = [0; 4];
        let res = x.encode_utf8(&mut dst);
        self.alloc_str(res)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::environment::Module;

    #[test]
    fn string_parts_precharge_both_passes_and_aligned_bytes() {
        let prefix = "a".repeat(254);
        let parts = ["", prefix.as_str(), "é", "", "雪/"];
        let expected = format!("{prefix}é雪/");
        let payload = StarlarkStr::payload_len_for_len(expected.len());
        let bytes = StarlarkStrAValue::checked_alloc_size_for_extra_len(payload)
            .unwrap()
            .bytes() as usize;
        let work = 2 * parts.len() + 1 + 2;
        for (work, bytes, refused) in [
            (work - 1, bytes, Some(ExecutionLimit::Work)),
            (work, bytes - 1, Some(ExecutionLimit::AllocationBytes)),
            (work, bytes, None),
        ] {
            let budget = Arc::new(ExecutionBudget::new(work, bytes, 1));
            let module = Module::new_with_execution_budget(budget.clone());
            let actual = module.heap().alloc_budgeted_str_parts(&parts);
            if let Some(refused) = refused {
                assert_eq!(actual.err(), Some(refused));
                assert_eq!(budget.check(), Err(refused));
            } else {
                assert_eq!(actual.unwrap().as_str(), expected);
                assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
            }
        }
    }

    #[test]
    fn empty_and_one_byte_parts_use_static_strings() {
        let budget = Arc::new(ExecutionBudget::new(5, 0, 1));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        for (parts, expected) in [(&["", ""][..], ""), (&["", "x", ""][..], "x")] {
            let actual = heap.alloc_budgeted_str_parts(parts).unwrap();
            assert_eq!(actual.as_str(), expected);
            assert_eq!(
                actual.as_str().as_ptr(),
                heap.alloc_str(expected).as_str().as_ptr()
            );
        }
        assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::Work));
    }

    #[test]
    fn budgeted_frozen_string_admits_copy_and_uses_static_short_strings() {
        let text = "雪\0".repeat(65);
        let work = 1 + ExecutionBudget::text_work(text.len());
        let bytes = StarlarkStrAValue::alloc_size_for_extra_len(StarlarkStr::payload_len_for_len(
            text.len(),
        ))
        .bytes() as usize;
        for (work, bytes, expected) in [
            (work, bytes, Ok(())),
            (work, bytes - 1, Err(ExecutionLimit::AllocationBytes)),
            (work - 1, bytes, Err(ExecutionLimit::Work)),
        ] {
            let heap = FrozenHeap::new();
            let budget = ExecutionBudget::new(work, bytes, 8);
            let result = heap.alloc_budgeted_str(&budget, &text);
            assert_eq!(budget.check(), expected);
            match result {
                Ok(value) => {
                    assert_eq!(value.as_str(), text);
                    assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
                }
                Err(error) => {
                    assert_eq!(Err(error), expected);
                    assert_eq!(heap.allocated_bytes(), 0);
                }
            }
        }
        let heap = FrozenHeap::new();
        let budget = ExecutionBudget::new(2, 0, 8);
        for text in ["", "\0"] {
            let value = heap.alloc_budgeted_str(&budget, text).unwrap();
            assert!(std::ptr::eq(
                value.as_str(),
                constant_string(text).unwrap().as_str()
            ));
        }
        assert_eq!(heap.allocated_bytes(), 0);
    }
}
