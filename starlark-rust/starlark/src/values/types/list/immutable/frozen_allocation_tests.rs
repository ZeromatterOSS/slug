use super::AllocImmutableList;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenHeap;
use crate::values::ValueLike;
use crate::values::list::AllocList;
use crate::values::list::FrozenListRef;
use crate::values::list::ListRef;

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
