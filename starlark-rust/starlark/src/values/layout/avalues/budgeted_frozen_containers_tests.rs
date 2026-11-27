use std::hash::Hasher;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::collections::StarlarkHasher;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::dict::DictRef;
use crate::values::dict::FrozenDictRef;
use crate::values::structs::AllocStruct;
use crate::values::structs::FrozenStructRef;
use crate::values::structs::StructuralIdentityOp;

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("native-key {}", _0)]
struct NativeKey(u32);

#[starlark_value(type = "native_key")]
impl<'v> StarlarkValue<'v> for NativeKey {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        // Different keys deliberately collide.
        hasher.write_u8(0);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        Ok(other
            .downcast_ref::<Self>()
            .is_some_and(|other| other.0 == self.0))
    }
}

struct NativeModel(bool);

impl<'v> StructuralIdentityNative<'v> for NativeModel {
    fn admit_atomic(
        &self,
        value: Value<'v>,
        _op: StructuralIdentityOp,
        budget: &ExecutionBudget,
    ) -> Result<bool, ExecutionLimit> {
        budget.charge(1, 0)?;
        Ok(self.0 && value.downcast_ref::<NativeKey>().is_some())
    }

    fn is_immutable(
        &self,
        _ctx: &mut StructuralIdentity<'_, 'v, Self>,
        _value: Value<'v>,
    ) -> crate::Result<Option<bool>> {
        Ok(None)
    }

    fn write_hash(
        &self,
        _ctx: &mut StructuralIdentity<'_, 'v, Self>,
        _value: Value<'v>,
        _hasher: &mut StarlarkHasher,
        _structural: bool,
    ) -> crate::Result<bool> {
        Ok(false)
    }

    fn equals(
        &self,
        _ctx: &mut StructuralIdentity<'_, 'v, Self>,
        _left: Value<'v>,
        _right: Value<'v>,
    ) -> crate::Result<Option<bool>> {
        Ok(None)
    }
}

#[test]
fn bounded_frozen_dict_preserves_native_hashes_and_collision_order() {
    let heap = FrozenHeap::new();
    let first = heap.alloc_simple(NativeKey(1));
    let duplicate = heap.alloc_simple(NativeKey(1));
    let colliding = heap.alloc_simple(NativeKey(2));
    assert!(!first.to_value().ptr_eq(duplicate.to_value()));
    assert_eq!(
        first.get_hashed().unwrap().hash(),
        colliding.get_hashed().unwrap().hash()
    );
    let tuple = heap.alloc_tuple(&[first, heap.alloc("tuple key")]);
    let original = heap.alloc("first value");
    let replacement = heap.alloc("last value");
    let rows = [
        (first, original),
        (colliding, original),
        (duplicate, replacement),
        (tuple, original),
    ];
    let bytes = SmallMap::<FrozenValue, FrozenValue>::with_capacity(rows.len())
        .retained_storage_bytes_upper_bound()
        .unwrap()
        + AValueSimple::<DictGen<FrozenDictData>>::alloc_size_for_extra_len(0).bytes() as usize;
    let exact = ExecutionBudget::new(10_000, bytes, 16);
    let value = heap
        .alloc_budgeted_dict_from_rows(&exact, rows.len(), rows.map(Ok), &NativeModel(true))
        .unwrap();
    let dict = FrozenDictRef::from_frozen_value(value).unwrap();
    let ordinary = heap.alloc(AllocDict(rows));
    let ordinary = FrozenDictRef::from_frozen_value(ordinary).unwrap();
    assert_eq!(dict.iter().len(), 3);
    for (actual, expected) in dict.iter().zip(ordinary.iter()) {
        assert!(actual.0.to_value().ptr_eq(expected.0.to_value()));
        assert!(actual.1.to_value().ptr_eq(expected.1.to_value()));
    }
    let dict = DictRef::from_value(value.to_value()).unwrap();
    assert!(
        dict.get(duplicate.to_value())
            .unwrap()
            .unwrap()
            .ptr_eq(replacement.to_value())
    );
    for (key, _) in dict.iter_hashed() {
        assert_eq!(key.hash(), key.key().get_hashed().unwrap().hash());
    }
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));

    let before = (heap.allocated_bytes(), heap.available_bytes());
    let short = ExecutionBudget::new(10_000, bytes - 1, 16);
    assert!(
        heap.alloc_budgeted_dict_from_rows(&short, rows.len(), rows.map(Ok), &NativeModel(true))
            .is_err()
    );
    assert_eq!(short.check(), Err(ExecutionLimit::AllocationBytes));
    assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);
}

#[test]
fn bounded_frozen_dictionary_refuses_unadmitted_native_key_and_keeps_sticky_error() {
    let heap = FrozenHeap::new();
    let key = heap.alloc_simple(NativeKey(0));
    let budget = ExecutionBudget::new(100, 100_000, 16);
    let result = heap.alloc_budgeted_dict_from_rows(
        &budget,
        1,
        [Ok((key, FrozenValue::new_none()))],
        &NativeModel(false),
    );
    let expected = ExecutionLimit::UnsupportedOperation {
        operation: "structural hashing",
        value_type: "native_key",
    };
    assert!(matches!(
        result.unwrap_err().kind(),
        crate::ErrorKind::Other(error) if error.downcast_ref::<ExecutionLimit>() == Some(&expected)
    ));
    assert_eq!(budget.check(), Err(expected));

    let collision = heap.alloc_simple(NativeKey(1));
    let before = (heap.allocated_bytes(), heap.available_bytes());
    // Two rows and their hashes (6), scan and equality candidate (2),
    // and equality entry and its first native operand (2).
    let budget = ExecutionBudget::new(10, 100_000, 16);
    let result = heap.alloc_budgeted_dict_from_rows(
        &budget,
        2,
        [(key, key), (collision, collision)].map(Ok),
        &NativeModel(true),
    );
    assert!(matches!(
        result.unwrap_err().kind(),
        crate::ErrorKind::Other(error)
            if error.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::Work)
    ));
    assert_eq!(budget.check(), Err(ExecutionLimit::Work));
    assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);
}

#[test]
fn bounded_frozen_struct_preserves_field_order_hashes_and_values() {
    let heap = FrozenHeap::new();
    let first = heap.alloc_str("z");
    let second = heap.alloc_str("a雪");
    let rows = [
        (first, heap.alloc("first field")),
        (second, heap.alloc_simple(NativeKey(2))),
    ];
    let bytes = SmallMap::<FrozenStringValue, FrozenValue>::with_capacity(rows.len())
        .retained_storage_bytes_upper_bound()
        .unwrap()
        + AValueSimple::<FrozenStruct>::alloc_size_for_extra_len(0).bytes() as usize;
    let exact = ExecutionBudget::new(10_000, bytes, 16);
    let value = heap
        .alloc_budgeted_struct_from_rows(&exact, rows.len(), rows.map(Ok))
        .unwrap();
    let record = FrozenStructRef::from_value(value).unwrap();
    assert_eq!(record.iter().len(), 2);
    for ((key, field), (expected_key, expected_field)) in record.iter().zip(rows) {
        assert!(
            key.to_frozen_value()
                .to_value()
                .ptr_eq(expected_key.to_frozen_value().to_value())
        );
        assert!(field.to_value().ptr_eq(expected_field.to_value()));
    }
    assert_eq!(
        value.get_hashed().unwrap().hash(),
        heap.alloc(AllocStruct(rows)).get_hashed().unwrap().hash()
    );
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));

    let before = (heap.allocated_bytes(), heap.available_bytes());
    let short = ExecutionBudget::new(10_000, bytes - 1, 16);
    assert!(
        heap.alloc_budgeted_struct_from_rows(&short, rows.len(), rows.map(Ok))
            .is_err()
    );
    assert_eq!(short.check(), Err(ExecutionLimit::AllocationBytes));
    assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);

    let duplicate = [(first, rows[0].1), (first, rows[1].1)];
    let budget = ExecutionBudget::new(10_000, bytes, 16);
    let error = heap
        .alloc_budgeted_struct_from_rows(&budget, duplicate.len(), duplicate.map(Ok))
        .unwrap_err();
    assert!(error.to_string().contains("duplicate struct field"));
    assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);
}
