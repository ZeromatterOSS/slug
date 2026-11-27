use std::num::NonZeroI32;
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::list::AllocList;
use crate::values::range::Range;
use crate::values::starlark_value;
use crate::values::tuple::AllocTuple;

fn budget(work: usize, bytes: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(work, bytes, 16).with_bounded_operations())
}

#[test]
fn strict_native_len_reads_concrete_builtin_lengths() {
    let globals = Globals::standard();
    let len = globals.get("len").unwrap();
    let budget = budget(4096, 16_384);
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let frozen = module.frozen_heap();
    let mut eval = Evaluator::new(&module);
    for (value, expected) in [
        (heap.alloc(AllocList([1, 2])), 2),
        (frozen.alloc(AllocList([1, 2, 3])).to_value(), 3),
        (heap.alloc(AllocTuple([1, 2, 3, 4])), 4),
        (heap.alloc(AllocDict([("a", 1)])), 1),
        (frozen.alloc(AllocDict([("a", 1), ("b", 2)])).to_value(), 2),
        (heap.alloc("é雪🙂"), 3),
        (
            heap.alloc(Range::new(9, 2, NonZeroI32::new(-2).unwrap())),
            4,
        ),
    ] {
        assert_eq!(
            eval.eval_function(len, &[value], &[]).unwrap().unpack_i32(),
            Some(expected),
        );
    }
    assert_eq!(budget.check(), Ok(()));
}

#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("length_canary")]
struct LengthCanary {
    trap: bool,
}

starlark::starlark_simple_value!(LengthCanary);

#[starlark_value(type = "length_canary")]
impl<'v> StarlarkValue<'v> for LengthCanary {
    fn length(&self) -> crate::Result<i32> {
        assert!(!self.trap, "unadmitted native length callback was called");
        Ok(17)
    }
}

#[test]
fn strict_len_refuses_hooks_through_the_builtin_and_ordinary_frozen_opcode() {
    let globals = Globals::standard();
    let builtin = globals.get("len").unwrap().unpack_frozen().unwrap();
    let producer = Module::new();
    Evaluator::new(&producer)
        .eval_module(
            AstModule::parse(
                "length.star",
                "def run(value): return len(value)".to_owned(),
                &Dialect::Standard,
            )
            .unwrap(),
            &globals,
        )
        .unwrap();
    let producer = producer.freeze().unwrap();
    let run = producer.get("run").unwrap();
    let bytecode = run
        .value()
        .downcast_ref::<FrozenDef>()
        .unwrap()
        .dump_debug();
    assert!(
        bytecode.lines().any(|line| line.contains(": Len ")),
        "{bytecode}"
    );

    let ordinary = Module::new();
    let mut eval = Evaluator::new(&ordinary);
    for function in [builtin.to_value(), run.owned_value(ordinary.frozen_heap())] {
        let native = ordinary.heap().alloc(LengthCanary { trap: false });
        assert_eq!(
            eval.eval_function(function, &[native], &[])
                .unwrap()
                .unpack_i32(),
            Some(17),
        );
    }

    let expected = ExecutionLimit::UnsupportedOperation {
        operation: "len()",
        value_type: "length_canary",
    };
    for frozen_opcode in [false, true] {
        let budget = budget(4096, 16_384);
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let function = if frozen_opcode {
            run.owned_value(module.frozen_heap())
        } else {
            builtin.to_value()
        };
        let mut eval = Evaluator::new(&module);
        assert_eq!(
            eval.eval_function(function, &[heap.alloc((1, 2))], &[])
                .unwrap()
                .unpack_i32(),
            Some(2),
        );
        let error = eval
            .eval_function(function, &[heap.alloc(LengthCanary { trap: true })], &[])
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            crate::ErrorKind::Other(cause) if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ));
        assert_eq!(budget.check(), Err(expected));
    }
}

#[test]
fn strict_string_length_prepays_the_utf8_scan_without_allocation() {
    let text = "é".repeat(129);
    // One dispatch unit and two units for 258 UTF-8 bytes.
    for work in [2, 3] {
        let budget = budget(work, 0);
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let value = heap.alloc(text.as_str());
        let before = heap.allocated_bytes();
        let result = value.length_on_heap(heap);
        if work == 3 {
            assert_eq!(result.unwrap(), 129);
            assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::Work));
        } else {
            assert!(result.is_err());
            assert_eq!(budget.check(), Err(ExecutionLimit::Work));
        }
        assert_eq!(heap.allocated_bytes(), before);
    }
}
