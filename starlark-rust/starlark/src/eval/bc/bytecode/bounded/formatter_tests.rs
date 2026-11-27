use std::cell::Cell;
use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::bc::frame::alloca_frame;
use crate::eval::bc::stack_ptr::BcSlot;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::dict::DictMut;
use crate::values::function::FUNCTION_TYPE;
use crate::values::list::value::ListData;
use crate::values::tuple::TupleRef;

thread_local! {
    static SLICE_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("slice_canary")]
struct SliceCanary;

starlark::starlark_simple_value!(SliceCanary);

#[starlark_value(type = "slice_canary")]
impl<'v> StarlarkValue<'v> for SliceCanary {
    fn slice(
        &self,
        _start: Option<Value<'v>>,
        _stop: Option<Value<'v>>,
        _stride: Option<Value<'v>>,
        _heap: Heap<'v>,
    ) -> crate::Result<Value<'v>> {
        SLICE_CALLS.with(|calls| calls.set(calls.get() + 1));
        Ok(Value::new_none())
    }
}

fn parse(source: &str, dialect: &Dialect) -> AstModule {
    AstModule::parse("closed_formatter.star", source.to_owned(), dialect).unwrap()
}

fn freeze(source: &str, dialect: &Dialect) -> FrozenModule {
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(parse(source, dialect), &Globals::standard())
        .unwrap();
    module.freeze().unwrap()
}

fn run<'v>(
    frozen: &FrozenModule,
    function: &str,
    module: &'v Module,
    args: &[Value<'v>],
) -> crate::Result<Value<'v>> {
    let function = frozen
        .get(function)
        .unwrap()
        .owned_value(module.frozen_heap());
    Evaluator::new_with_reused_call_storage_for_test(module).eval_function(function, args, &[])
}

fn assert_refusal(error: crate::Error, budget: &ExecutionBudget, expected: ExecutionLimit) {
    assert!(
        matches!(
            error.kind(),
            crate::ErrorKind::Other(cause)
                if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ),
        "{error}"
    );
    assert_eq!(budget.check(), Err(expected));
}

#[test]
fn closed_formatter_denies_native_slice_and_releases_only_its_own_loops() {
    let program = freeze(
        r#"
def run(xs, dictionary, canary):
    for x in xs:
        for key in dictionary:
            ignored = canary[:]
    return True
"#,
        &Dialect::Standard,
    );
    let new_budget = || ExecutionBudget::new(4096, 4096, 16);
    for (budget, closed) in [
        (None, false),
        (Some(new_budget()), false),
        (Some(new_budget().with_bounded_operations()), false),
        (Some(new_budget().with_closed_formatter_bytecode()), true),
    ] {
        let budget = budget.map(Arc::new);
        let caller = match &budget {
            Some(budget) => Module::new_with_execution_budget(budget.clone()),
            None => Module::new(),
        };
        let heap = caller.heap();
        let list = heap.alloc_list(&[Value::new_none()]);
        let dictionary = heap.alloc(AllocDict([(17, 23)]));
        let canary = heap.alloc(SliceCanary);
        let host_iterator = list.iterate(heap).unwrap();
        let calls_before = SLICE_CALLS.with(Cell::get);
        let result = run(&program, "run", &caller, &[list, dictionary, canary]);
        if closed {
            assert_refusal(
                result.unwrap_err(),
                budget.as_deref().unwrap(),
                ExecutionLimit::UnsupportedBytecode { opcode: "Slice" },
            );
        } else {
            assert_eq!(result.unwrap().unpack_bool(), Some(true));
        }
        assert_eq!(
            SLICE_CALLS.with(Cell::get) - calls_before,
            usize::from(!closed)
        );
        assert!(DictMut::from_value(dictionary).is_ok());
        assert!(ListData::from_value_mut(list).is_err());
        drop(host_iterator);
        assert!(ListData::from_value_mut(list).is_ok());
    }
}

#[test]
fn closed_formatter_checks_sticky_failure_after_debug_hooks() {
    struct RefuseAtSlice<'a>(&'a ExecutionBudget, bool);
    impl EvaluationCallbacks for RefuseAtSlice<'_> {
        fn before_instr(
            &mut self,
            _eval: &mut Evaluator,
            _ip: BcPtrAddr,
            opcode: BcOpcode,
        ) -> crate::Result<()> {
            if opcode == BcOpcode::Slice {
                self.1 = true;
                self.0.refuse(ExecutionLimit::Work);
            }
            Ok(())
        }
    }

    let program = freeze("def run(canary): return canary[:]", &Dialect::Standard);
    let function = program.get("run").unwrap();
    let bc = function.value().downcast_ref::<FrozenDef>().unwrap().bc();
    let budget = Arc::new(ExecutionBudget::new(4096, 4096, 16).with_closed_formatter_bytecode());
    let caller = Module::new_with_execution_budget(budget.clone());
    let canary = caller.heap().alloc(SliceCanary);
    let calls_before = SLICE_CALLS.with(Cell::get);
    let mut callback = RefuseAtSlice(&budget, false);
    let mut evaluator = Evaluator::new_with_reused_call_storage_for_test(&caller);
    let result = alloca_frame(
        &mut evaluator,
        bc.local_count,
        bc.max_stack_size,
        bc.max_loop_depth,
        |eval| {
            eval.current_frame.set_bc_slot(BcSlot(0).to_out(), canary);
            bc.run(eval, &mut callback)
        },
    )
    .unwrap();
    assert!(callback.1);
    assert_eq!(SLICE_CALLS.with(Cell::get), calls_before);
    assert_refusal(
        result.unwrap_err().into_error(),
        &budget,
        ExecutionLimit::Work,
    );
}

#[test]
fn closed_formatter_runs_a_deferred_function_after_setup_and_freeze() {
    let budget = Arc::new(ExecutionBudget::new(4096, 4096, 16).with_closed_formatter_bytecode());
    let source = Module::new_with_execution_budget(budget.clone());
    Evaluator::new_with_reused_call_storage_for_test(&source)
        .eval_module(
            parse(
                r#"
def run(items):
    filtered = [item for item in items if item]
    return (filtered[0], filtered[0] != False)
"#,
                &Dialect::Standard,
            ),
            &Globals::standard(),
        )
        .unwrap();
    let program = source.freeze_bounded().unwrap();
    let row = Module::new_with_execution_budget(budget.clone());
    let items = row
        .heap()
        .alloc_list(&[Value::new_bool(false), Value::new_bool(true)]);
    let result = run(&program, "run", &row, &[items]).unwrap();
    let tuple = TupleRef::from_value(result).unwrap();
    assert_eq!(tuple.len(), 2);
    assert!(
        tuple
            .content()
            .iter()
            .all(|value| value.unpack_bool() == Some(true))
    );
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn closed_formatter_refuses_frozen_annotations_and_captures_before_dispatch() {
    let program = freeze(
        r#"
def annotated(argument: int):
    return argument
def annotated_return(argument) -> int:
    return argument
def parameter_capture(argument):
    def inner():
        return argument
    return inner
inherited = parameter_capture(19)
"#,
        &Dialect::Extended,
    );

    for name in [
        "annotated",
        "annotated_return",
        "parameter_capture",
        "inherited",
    ] {
        let budget =
            Arc::new(ExecutionBudget::new(4096, 4096, 16).with_closed_formatter_bytecode());
        let caller = Module::new_with_execution_budget(budget.clone());
        let argument = Value::testing_new_int(17);
        let args = if name == "inherited" {
            &[][..]
        } else {
            std::slice::from_ref(&argument)
        };
        assert_refusal(
            run(&program, name, &caller, args).unwrap_err(),
            &budget,
            ExecutionLimit::UnsupportedOperation {
                operation: "function invocation",
                value_type: FUNCTION_TYPE,
            },
        );
        assert!(
            !caller
                .heap()
                .allocated_summary()
                .summary()
                .contains_key("value_captured")
        );
    }
}
