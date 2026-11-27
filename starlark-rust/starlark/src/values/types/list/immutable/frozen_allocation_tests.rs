use super::AllocImmutableList;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::ValueLike;
use crate::values::list::AllocList;
use crate::values::list::FrozenListRef;
use crate::values::list::ListRef;

#[test]
fn budgeted_immutable_list_preserves_occurrences_and_exact_array_admission() {
    let ordinary = FrozenHeap::new();
    let elem = [FrozenValue::new_bool(true), FrozenValue::new_none()];
    let object_bytes = (0..8)
        .find_map(|_| {
            let (reserved, available) = (ordinary.allocated_bytes(), ordinary.available_bytes());
            ordinary.alloc(AllocImmutableList(elem));
            (ordinary.allocated_bytes() == reserved).then(|| available - ordinary.available_bytes())
        })
        .expect("a list allocation in an open chunk");
    let bytes = object_bytes + std::mem::size_of_val(&elem);

    let heap = FrozenHeap::new();
    let short = ExecutionBudget::new(128, bytes - 1, 8);
    assert_eq!(
        heap.alloc_budgeted_immutable_list(&short, &elem),
        Err(ExecutionLimit::AllocationBytes)
    );
    assert_eq!(heap.allocated_bytes(), 0);

    let exact = ExecutionBudget::new(128, bytes, 8);
    let list = heap.alloc_budgeted_immutable_list(&exact, &elem).unwrap();
    assert_eq!(FrozenListRef::from_frozen_value(list).unwrap().len(), 2);
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));

    let empty_allocation = ExecutionBudget::new(128, 2 * object_bytes, 8);
    let empty = heap
        .alloc_budgeted_immutable_list(&empty_allocation, &[])
        .unwrap();
    let other = heap
        .alloc_budgeted_immutable_list(&empty_allocation, &[])
        .unwrap();
    assert!(!empty.to_value().ptr_eq(other.to_value()));
    assert!(empty.to_value().equals(other.to_value()).unwrap());
    assert_eq!(
        empty_allocation.charge(0, 1),
        Err(ExecutionLimit::AllocationBytes)
    );
}

#[test]
fn frozen_immutable_list_allocation_preserves_occurrences_and_list_behavior() {
    let heap = FrozenHeap::new();
    let empty = heap.alloc(AllocImmutableList(Vec::<String>::new()));
    let other_empty = heap.alloc(AllocImmutableList(Vec::<String>::new()));
    let values = heap.alloc(AllocImmutableList(["one", "two", "two"]));
    let other_values = heap.alloc(AllocImmutableList(["one", "two", "two"]));
    assert_ne!(
        empty.to_value().identity(),
        other_empty.to_value().identity()
    );
    assert_ne!(
        values.to_value().identity(),
        other_values.to_value().identity()
    );
    assert!(values.to_value().equals(other_values.to_value()).unwrap());
    assert_eq!(empty.to_value().get_type(), "list");
    assert!(!empty.to_value().to_bool());
    assert!(values.to_value().to_bool());
    assert!(ListRef::from_value(empty.to_value()).unwrap().is_empty());
    assert_eq!(FrozenListRef::from_frozen_value(values).unwrap().len(), 3);
    // The ordinary allocator retains its canonical empty behavior.
    let ordinary = heap.alloc(AllocList(Vec::<String>::new()));
    let ordinary_again = heap.alloc(AllocList(Vec::<String>::new()));
    assert_eq!(
        ordinary.to_value().identity(),
        ordinary_again.to_value().identity()
    );

    let module = Module::new();
    module.set("values", values.to_value());
    module.set(
        "ordinary",
        heap.alloc(AllocList(["one", "two", "two"])).to_value(),
    );
    Evaluator::new(&module).eval_module(AstModule::parse("list-methods.star",
        "iteration = [x for x in values]\nindex = values.index('two')\nlength = len(values)\nordinary_index = ordinary.index('two')\nordinary_length = len(ordinary)\n".into(), &Dialect::Standard).unwrap(), &Globals::standard()).unwrap();
    let iteration = module.get("iteration").unwrap();
    assert_eq!(
        ListRef::from_value(iteration)
            .unwrap()
            .iter()
            .map(|v| v.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["one", "two", "two"]
    );
    assert_eq!(module.get("index").unwrap().unpack_i32(), Some(1));
    assert_eq!(module.get("length").unwrap().unpack_i32(), Some(3));
    assert_eq!(module.get("ordinary_index").unwrap().unpack_i32(), Some(1));
    assert_eq!(module.get("ordinary_length").unwrap().unpack_i32(), Some(3));
    for source in [
        "values.append('new')",
        "values.extend(['new'])",
        "values[0] = 'new'",
        "ordinary.append('new')",
        "ordinary[0] = 'new'",
    ] {
        let error = Evaluator::new(&module)
            .eval_module(
                AstModule::parse("list-mutation.star", source.into(), &Dialect::Standard).unwrap(),
                &Globals::standard(),
            )
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("immutable"),
            "{error}"
        );
    }
}

#[test]
fn frozen_immutable_list_foreign_elements_keep_explicit_heap_owners() {
    let producer = FrozenHeap::new();
    let text = producer.alloc("foreign owned element");
    let producer_owner = producer.into_ref();
    let destination = FrozenHeap::new();
    destination.add_reference(&producer_owner);
    let values = destination.alloc(AllocImmutableList([text]));
    drop(producer_owner);
    let destination_owner = destination.into_ref();
    let module = Module::new();
    module.frozen_heap().add_reference(&destination_owner);
    module.set("foreign", values.to_value());
    drop(destination_owner);
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "owned-list.star",
                "result = foreign[0]\n".into(),
                &Dialect::Standard,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    assert_eq!(
        module.get("result").unwrap().unpack_str(),
        Some("foreign owned element")
    );
    let frozen = module.freeze().unwrap();
    let owned = frozen.get("foreign").unwrap();
    assert_eq!(
        FrozenListRef::from_value(owned.value()).unwrap()[0]
            .to_value()
            .unpack_str(),
        Some("foreign owned element")
    );
}
