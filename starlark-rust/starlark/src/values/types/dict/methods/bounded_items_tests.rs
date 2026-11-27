use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::items_bounded;
use crate as starlark;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::dict::AllocDict;
use crate::values::dict::DictRef;
use crate::values::list::ListRef;
use crate::values::tuple::TupleRef;

fn budget(bytes: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(50_000, bytes, 16).with_bounded_operations())
}

fn freeze(source: &str) -> FrozenModule {
    let module = Module::new();
    let ast = AstModule::parse("items.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(&module)
        .eval_module(ast, &Globals::standard())
        .unwrap();
    module.freeze().unwrap()
}

fn invoke<'v>(
    program: &FrozenModule,
    caller: &'v Module,
    name: &str,
    arg: Value<'v>,
) -> crate::Result<Value<'v>> {
    let function = program.get(name).unwrap().owned_value(caller.frozen_heap());
    Evaluator::new(caller).eval_function(function, &[arg], &[])
}

#[test]
fn strict_items_and_unpack_run_ordinary_frozen_bytecode_in_order() {
    let program = freeze(
        r#"
def items(mapping):
    return mapping.items()

def select(mapping):
    for key, value in mapping.items():
        if key.endswith("match"):
            return value
"#,
    );
    let budget = budget(1 << 20);
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let rows = [
        (heap.alloc("leading"), heap.alloc("ignored")),
        (heap.alloc("z-match"), heap.alloc("chosen")),
        (heap.alloc("a-match"), heap.alloc("later")),
    ];
    let mapping = heap.alloc(AllocDict(rows));
    let result = invoke(&program, &caller, "items", mapping).unwrap();
    let pairs = ListRef::from_value(result).unwrap();
    assert_eq!(pairs.len(), rows.len());
    for (pair, (key, value)) in pairs.iter().zip(rows) {
        let pair = TupleRef::from_value(pair).unwrap();
        assert_eq!(pair.len(), 2);
        assert!(pair.content()[0].ptr_eq(key));
        assert!(pair.content()[1].ptr_eq(value));
    }
    assert!(
        invoke(&program, &caller, "select", mapping)
            .unwrap()
            .ptr_eq(rows[1].1)
    );
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn strict_items_pay_the_observed_list_and_tuple_bytes() {
    let credits = budget(1 << 20);
    let sample = Module::new_with_execution_budget(credits.clone());
    let heap = sample.heap();
    let dict = DictRef::from_value(heap.alloc(AllocDict([("b", 1), ("a", 2)]))).unwrap();
    items_bounded(&dict, heap, &credits).unwrap();
    let (reserved, available) = (heap.allocated_bytes(), heap.available_bytes());
    items_bounded(&dict, heap, &credits).unwrap();
    assert_eq!(heap.allocated_bytes(), reserved);
    let bytes = available - heap.available_bytes();
    assert!(bytes > 1);

    for available in [bytes - 1, bytes] {
        let credits = budget(available);
        let caller = Module::new_with_execution_budget(credits.clone());
        let heap = caller.heap();
        let dict = DictRef::from_value(heap.alloc(AllocDict([("b", 1), ("a", 2)]))).unwrap();
        let result = items_bounded(&dict, heap, &credits);
        if available == bytes {
            assert_eq!(ListRef::from_value(result.unwrap()).unwrap().len(), 2);
            assert_eq!(credits.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_eq!(result.unwrap_err(), ExecutionLimit::AllocationBytes);
            assert_eq!(credits.check(), Err(ExecutionLimit::AllocationBytes));
        }
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("unpack_canary")]
struct UnpackCanary;

starlark::starlark_simple_value!(UnpackCanary);

#[starlark_value(type = "unpack_canary")]
impl<'v> StarlarkValue<'v> for UnpackCanary {
    fn length(&self) -> crate::Result<i32> {
        panic!("unpack invoked an unadmitted length callback");
    }

    unsafe fn iterate(&self, _me: Value<'v>, _heap: Heap<'v>) -> crate::Result<Value<'v>> {
        panic!("unpack invoked an unadmitted iteration callback");
    }
}

#[test]
fn strict_unpack_borrows_lists_and_tuples_and_refuses_virtual_hooks() {
    let program = freeze("def run(pair):\n    first, second = pair\n    return second");
    let credits = budget(1 << 20);
    let caller = Module::new_with_execution_budget(credits.clone());
    let heap = caller.heap();
    let first = heap.alloc("first");
    let second = heap.alloc("retained alias");
    for pair in [
        heap.alloc_list(&[first, second]),
        heap.alloc_tuple(&[first, second]),
    ] {
        assert!(
            invoke(&program, &caller, "run", pair)
                .unwrap()
                .ptr_eq(second)
        );
    }
    let short = invoke(&program, &caller, "run", heap.alloc_tuple(&[first])).unwrap_err();
    assert!(
        short
            .to_string()
            .contains("Unpacked 1 values but expected 2")
    );
    assert_eq!(credits.check(), Ok(()));

    let error = invoke(&program, &caller, "run", heap.alloc(UnpackCanary)).unwrap_err();
    let expected = ExecutionLimit::UnsupportedOperation {
        operation: "sequence unpacking",
        value_type: "unpack_canary",
    };
    assert!(matches!(error.kind(), crate::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)));
    assert_eq!(credits.check(), Err(expected));

    let legacy =
        Module::new_with_execution_budget(Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16)));
    let ast = AstModule::parse("range.star", "range(2)".to_owned(), &Dialect::Standard).unwrap();
    let range = Evaluator::new(&legacy)
        .eval_module(ast, &Globals::standard())
        .unwrap();
    assert_eq!(
        invoke(&program, &legacy, "run", range)
            .unwrap()
            .unpack_i32(),
        Some(1)
    );
}
