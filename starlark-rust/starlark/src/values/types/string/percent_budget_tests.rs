use std::sync::Arc;

use super::*;
use crate::ErrorKind;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast =
        AstModule::parse("percent-budget.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new_with_reused_call_storage_for_test(module).eval_module(ast, &Globals::standard())
}

fn assert_limit(error: crate::Error, expected: ExecutionLimit) {
    match error.kind() {
        ErrorKind::Native(error) | ErrorKind::Other(error) => {
            assert_eq!(error.downcast_ref::<ExecutionLimit>(), Some(&expected));
        }
        _ => panic!("expected a typed resource refusal, got {error}"),
    }
}

#[test]
fn budgeted_dynamic_percent_and_assignment_preserve_output() {
    let source = r#"
def apply(pattern, arg):
    return pattern % arg
alias = apply
text = "é%%=%s"
text %= (["雪\n"],)
(text, alias("%r:%d:%o:%x:%X:%e:%E:%f:%F:%g:%G", ("é", -7.2, -7, -8, -9, 1, 2, 3, 4, 5, 6)))
"#;
    let ordinary = Module::new();
    let budget = Arc::new(ExecutionBudget::new(8192, 8192, 8));
    let bounded = Module::new_with_execution_budget(budget.clone());
    assert_eq!(
        eval(&bounded, source).unwrap().to_repr(),
        eval(&ordinary, source).unwrap().to_repr(),
    );
    assert_eq!(budget.check(), Ok(()));

    for source in [
        "pattern = '%s'\npattern % ('text',)",
        "pattern = '%s'\npattern %= ('text',)",
    ] {
        let budget = Arc::new(ExecutionBudget::new(1024, 0, 8));
        let bounded = Module::new_with_execution_budget(budget.clone());
        assert_limit(
            eval(&bounded, source).unwrap_err(),
            ExecutionLimit::AllocationBytes,
        );
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
    }
}

#[test]
fn ordinary_errors_and_bounded_float_type_error_match() {
    let budget = Arc::new(ExecutionBudget::new(8192, 8192, 8));
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let unicode = "世界".repeat(45);
    for (pattern, arg) in [
        ("%s", heap.alloc_tuple(&[])),
        ("%s", heap.alloc((1, 2))),
        ("%q", heap.alloc_tuple(&[])),
        ("%s%", heap.alloc((1,))),
        ("%d", Value::new_bool(true)),
        ("%f", heap.alloc("wrong")),
        ("%f%q", heap.alloc("wrong")),
        ("%E", heap.alloc(unicode)),
    ] {
        assert_eq!(
            percent_bounded(pattern, arg, heap).unwrap_err().to_string(),
            percent(pattern, arg).unwrap_err().to_string(),
        );
        assert_eq!(budget.check(), Ok(()));
    }
}

#[test]
fn parser_diagnostics_and_heap_copy_refuse_before_amplifying() {
    let large = "x".repeat(1024);
    for (credits, bytes, pattern, argument, expected) in [
        (4, 1024, large.as_str(), "", ExecutionLimit::Work),
        (3, 1024, "%f", large.as_str(), ExecutionLimit::Work),
        (
            1024,
            0,
            "%f",
            large.as_str(),
            ExecutionLimit::AllocationBytes,
        ),
        (1024, 4, "%s", "four", ExecutionLimit::AllocationBytes),
    ] {
        let budget = Arc::new(ExecutionBudget::new(credits, bytes, 8));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let arg = heap.alloc(argument);
        let before = heap.allocated_bytes();
        assert_limit(percent_bounded(pattern, arg, heap).unwrap_err(), expected);
        assert_eq!(heap.allocated_bytes(), before);
        assert_eq!(budget.check(), Err(expected));
    }

    for pattern in ["%d", "%o", "%x", "%X", "%e", "%f", "%g"] {
        let budget = Arc::new(ExecutionBudget::new(1024, 1024, 8));
        let module = Module::new_with_execution_budget(budget.clone());
        let arg = eval(&module, "4294967296").unwrap();
        let expected = ExecutionLimit::UnsupportedText("int");
        assert_limit(
            percent_bounded(pattern, arg, module.heap()).unwrap_err(),
            expected,
        );
        assert_eq!(budget.check(), Err(expected));
    }
}

#[test]
fn numeric_conversion_admits_work_before_checking_the_operand() {
    let budget = Arc::new(ExecutionBudget::new(2, 1024, 8));
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let arg = heap.alloc(4_294_967_296.0);
    assert!(percent("%d", arg).is_err());
    assert_limit(
        percent_bounded("%d", arg, heap).unwrap_err(),
        ExecutionLimit::Work,
    );
    assert_eq!(budget.check(), Err(ExecutionLimit::Work));
}

#[test]
fn frozen_ordinary_percent_intrinsic_uses_the_callers_budget() {
    let producer = Module::new();
    eval(
        &producer,
        "def plain(arg): return '%s' % arg\ndef affixed(arg): return '<%s>' % arg",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    let budget = Arc::new(ExecutionBudget::new(1024, 0, 8));
    let consumer = Module::new_with_execution_budget(budget.clone());
    let heap = consumer.heap();
    let arg = heap.alloc("雪");
    let plain = producer
        .get("plain")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    let affixed = producer
        .get("affixed")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    let mut eval = Evaluator::new_with_reused_call_storage_for_test(&consumer);
    assert!(
        eval.eval_function(plain, &[heap.alloc((arg,))], &[])
            .unwrap()
            .ptr_eq(arg)
    );
    let before = heap.allocated_bytes();
    assert_limit(
        eval.eval_function(affixed, &[arg], &[]).unwrap_err(),
        ExecutionLimit::AllocationBytes,
    );
    assert_eq!(heap.allocated_bytes(), before);
    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
}
