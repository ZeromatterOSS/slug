use std::sync::Arc;

use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;
use crate::values::ValueLike;

fn eval<'v>(module: &'v Module, text: &str) -> crate::Result<Value<'v>> {
    let source = AstModule::parse("format.star", text.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(source, &Globals::standard())
}

#[test]
fn format_budget_preserves_star_arguments_and_keyword_error_order() {
    for module in [
        Module::new(),
        Module::new_with_execution_budget(Arc::new(ExecutionBudget::new(1000, 4096, 16))),
    ] {
        let source = r#"
mixed = "{1}:{0!r}:{title}:{suffix}".format("α", title="head", *[3], **{"suffix": "tail"})
keys = "{2}:{0}:{1}".format(*{"z": 0, "y": 0, "x": 0})
"#;
        eval(&module, source).unwrap();
        assert_eq!(
            module.get("mixed").unwrap().unpack_str(),
            Some("3:\"α\":head:tail")
        );
        assert_eq!(module.get("keys").unwrap().unpack_str(), Some("x:z:y"));
        let error = eval(
            &module,
            r#"result = "{".format(title="head", **{"title": "tail"})"#,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Argument `title` occurs more than once")
        );
    }
}

#[test]
fn frozen_format_one_uses_the_callers_budget() {
    let producer = Module::new();
    eval(
        &producer,
        r#"def format_one(x): return "pre:{}:post".format(x)"#,
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    let program = producer.get("format_one").unwrap();
    let bytecode = program
        .value()
        .downcast_ref::<FrozenDef>()
        .unwrap()
        .dump_debug();
    assert!(bytecode.contains("FormatOne"), "{bytecode}");

    let budget = Arc::new(ExecutionBudget::new(1000, 4, 16));
    let caller = Module::new_with_execution_budget(budget.clone());
    let owned = program.owned_value(caller.frozen_heap());
    let input = caller.heap().alloc_list(&[caller.heap().alloc("é snow")]);
    let result = Evaluator::new(&caller).eval_function(owned, &[input], &[]);
    assert!(result.is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
}

#[test]
fn format_distinguishes_a_scalar_error_from_unmodeled_iteration() {
    let budget = Arc::new(ExecutionBudget::new(1000, 4096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let scalar = r#"result = "{".format(*None)"#;
    let ordinary = eval(&Module::new(), scalar).unwrap_err();
    assert_eq!(
        eval(&module, scalar)
            .unwrap_err()
            .without_diagnostic()
            .to_string(),
        ordinary.without_diagnostic().to_string()
    );
    assert!(budget.check().is_ok());
    let source = r#"result = "{}".format(*"abc".codepoints())"#;
    assert!(eval(&Module::new(), source).is_ok());
    assert!(eval(&module, source).is_err());
    assert!(matches!(
        budget.check(),
        Err(ExecutionLimit::UnsupportedOperation { .. })
    ));
}
