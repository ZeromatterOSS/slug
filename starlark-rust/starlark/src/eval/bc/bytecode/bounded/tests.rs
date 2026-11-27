use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::bc::frame::alloca_frame;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::dict::DictMut;
use crate::values::list::ListRef;
use crate::values::list::value::ListData;
use crate::values::tuple::TupleRef;

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("unadmitted_iterable")]
struct UnadmittedIterable;

starlark::starlark_simple_value!(UnadmittedIterable);

#[starlark_value(type = "unadmitted_iterable")]
impl<'v> StarlarkValue<'v> for UnadmittedIterable {
    unsafe fn iterate(&self, _me: Value<'v>, _heap: Heap<'v>) -> crate::Result<Value<'v>> {
        panic!("unadmitted iterable reached native startup");
    }
}

fn freeze(source: &str) -> FrozenModule {
    let module = Module::new();
    let ast = AstModule::parse("loops.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(&module)
        .eval_module(ast, &Globals::standard())
        .unwrap();
    module.freeze().unwrap()
}

fn invoke<'v>(
    program: &FrozenModule,
    caller: &'v Module,
    args: &[Value<'v>],
) -> crate::Result<Value<'v>> {
    let run = program
        .get("run")
        .unwrap()
        .owned_value(caller.frozen_heap());
    Evaluator::new_with_reused_call_storage_for_test(caller).eval_function(run, args, &[])
}

#[test]
fn bounded_frozen_bytecode_refusal_releases_only_its_own_nested_iterators() {
    let program = freeze("def run(xs):\n    for x in xs:\n        for y in xs:\n            pass");
    let budget = Arc::new(ExecutionBudget::new(64, 4_096, 16));
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let list = heap.alloc_list(&[Value::new_none(); 128]);
    let host_iterator = list.iterate(heap).unwrap();

    assert!(invoke(&program, &caller, &[list]).is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::Work));
    assert!(ListData::from_value_mut(list).is_err());
    drop(host_iterator);
    assert!(ListData::from_value_mut(list).is_ok());
}

#[test]
fn bounded_bytecode_ordinary_error_releases_outer_and_inner_frames() {
    let program = freeze(concat!(
        "def inner(mapping):\n",
        "    for key in mapping:\n",
        "        x = 1 // key\n",
        "def run(xs, mapping):\n",
        "    for x in xs:\n",
        "        inner(mapping)\n",
    ));
    let budget = Arc::new(ExecutionBudget::new(4_096, 4_096, 16));
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let list = heap.alloc_list(&[Value::new_none()]);
    let dict = heap.alloc(AllocDict([(0, 0)]));

    assert!(invoke(&program, &caller, &[list, dict]).is_err());
    assert_eq!(budget.check(), Ok(()));
    assert!(ListData::from_value_mut(list).is_ok());
    assert!(DictMut::from_value(dict).is_ok());
}

#[test]
fn bounded_bytecode_tracks_comprehension_after_return_stops_outer_loops() {
    let program = freeze(concat!(
        "def run(xs, ys):\n",
        "    for x in xs:\n",
        "        for y in xs:\n",
        "            return [z for z in ys]\n",
        "    return []\n",
    ));
    let budget = Arc::new(ExecutionBudget::new(4_096, 4_096, 16));
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let first = heap.alloc_list(&[Value::new_none()]);
    let second = heap.alloc_list(&[Value::new_bool(true), Value::new_bool(false)]);

    let result = invoke(&program, &caller, &[first, second]).unwrap();
    let result = ListRef::from_value(result).unwrap();
    assert!(
        result
            .iter()
            .map(|x| x.unpack_bool().unwrap())
            .eq([true, false])
    );
    assert_eq!(budget.check(), Ok(()));
    assert!(ListData::from_value_mut(first).is_ok());
    assert!(ListData::from_value_mut(second).is_ok());
}

#[test]
fn strict_frozen_comprehension_preserves_order_and_slot_identity() {
    let program = freeze("def run(xs): return ([x for x in xs], (xs,))");
    let budget = Arc::new(ExecutionBudget::new(4096, 4096, 16).with_bounded_operations());
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let keys = ["first", "second", "snow雪", "fourth", "fifth"].map(|key| heap.alloc(key));
    let input = heap.alloc(AllocDict(
        keys.into_iter().map(|key| (key, Value::new_none())),
    ));
    let result = TupleRef::from_value(invoke(&program, &caller, &[input]).unwrap()).unwrap();
    let collected = ListRef::from_value(result.content()[0]).unwrap();
    assert_eq!(collected.len(), keys.len());
    assert!(
        collected
            .iter()
            .zip(keys)
            .all(|(actual, key)| actual.ptr_eq(key))
    );
    let second = TupleRef::from_value(result.content()[1]).unwrap();
    assert_eq!(second.len(), 1);
    assert!(second.content()[0].ptr_eq(input));
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn strict_frozen_comprehension_refuses_first_growth_and_releases_iteration() {
    let program = freeze("def run(xs): return [x for x in xs]");
    let list_header = Heap::temp(|heap| {
        heap.alloc_list(&[]);
        let available = heap.available_bytes();
        heap.alloc_list(&[]);
        available - heap.available_bytes()
    });
    assert_ne!(list_header, 0);
    // One active-loop slot and one empty result list. No array growth fits.
    let bytes = std::mem::size_of::<BcSlotIn>() + list_header;
    for nonempty in [false, true] {
        let budget = Arc::new(ExecutionBudget::new(4096, bytes, 16).with_bounded_operations());
        let caller = Module::new_with_execution_budget(budget.clone());
        let body = [(17, 23)];
        let input = caller
            .heap()
            .alloc(AllocDict(body.into_iter().take(usize::from(nonempty))));
        let result = invoke(&program, &caller, &[input]);
        if nonempty {
            assert!(result.is_err());
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        } else {
            let result = result.unwrap();
            assert_eq!(ListRef::from_value(result).unwrap().len(), 0);
            assert_eq!(budget.check(), Ok(()));
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        }
        assert!(DictMut::from_value(input).is_ok());
    }
}

#[test]
fn strict_frozen_tuple_opcode_refuses_unadmitted_object_for_every_arity() {
    for (expression, arity) in [("(x,)", 1), ("(x,x)", 2), ("(x,x,x,x,x,x,x,x,x,x)", 10)] {
        let program = freeze(&format!("def run(x): return {expression}"));
        let run = program.get("run").unwrap();
        let bytecode = run
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(bytecode.contains("TupleNPop"), "{bytecode}");
        let ordinary = Module::new();
        let arg = Value::new_none();
        let result = invoke(&program, &ordinary, &[arg]).unwrap();
        assert_eq!(TupleRef::from_value(result).unwrap().len(), arity);

        let budget = Arc::new(ExecutionBudget::new(4096, 0, 16).with_bounded_operations());
        let caller = Module::new_with_execution_budget(budget.clone());
        let before = caller.heap().allocated_bytes();
        assert!(invoke(&program, &caller, &[arg]).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        assert_eq!(caller.heap().allocated_bytes(), before);
    }
}

#[test]
fn bounded_bytecode_checks_the_iterable_after_instruction_hooks() {
    struct ReplaceIterable {
        replacement: FrozenValue,
        calls: usize,
    }

    impl EvaluationCallbacks for ReplaceIterable {
        fn before_instr(
            &mut self,
            eval: &mut Evaluator,
            ip: BcPtrAddr,
            opcode: BcOpcode,
        ) -> crate::Result<()> {
            if opcode == BcOpcode::Iter {
                self.calls += 1;
                let (slot, _, _, _, _) = ip.get_instr::<InstrIter>().arg;
                eval.current_frame
                    .set_bc_slot(slot.get().to_out(), self.replacement.to_value());
            }
            Ok(())
        }
    }

    let program = freeze("def run(xs):\n    for x in xs:\n        pass");
    let run = program.get("run").unwrap();
    let bc = run.value().downcast_ref::<FrozenDef>().unwrap().bc();
    let budget = Arc::new(ExecutionBudget::new(1_024, 1_024, 16));
    let caller = Module::new_with_execution_budget(budget.clone());
    let list = caller.heap().alloc_list(&[Value::new_none()]);
    let mut callback = ReplaceIterable {
        replacement: caller.frozen_heap().alloc(UnadmittedIterable),
        calls: 0,
    };
    let mut eval = Evaluator::new(&caller);
    let result = alloca_frame(
        &mut eval,
        bc.local_count,
        bc.max_stack_size,
        bc.max_loop_depth,
        |eval| {
            eval.current_frame.set_bc_slot(BcSlot(0).to_out(), list);
            bc.run(eval, &mut callback)
        },
    )
    .unwrap();
    assert!(result.is_err());
    assert_eq!(callback.calls, 1);
    assert_eq!(
        budget.check(),
        Err(ExecutionLimit::UnsupportedOperation {
            operation: "bytecode iteration",
            value_type: "unadmitted_iterable",
        })
    );
    assert!(ListData::from_value_mut(list).is_ok());
}
