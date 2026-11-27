use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::starlark_value;
use crate::values::tuple::TupleRef;

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("methods.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn strict_budget(bytes: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(50_000, bytes, 16).with_bounded_operations())
}

fn freeze(source: &str) -> FrozenModule {
    let module = Module::new();
    eval(&module, source).unwrap();
    module.freeze().unwrap()
}

fn assert_limit(error: crate::Error, budget: &ExecutionBudget, expected: ExecutionLimit) {
    assert!(
        matches!(
            error.kind(),
            crate::ErrorKind::Other(cause) if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ),
        "{error}"
    );
    assert_eq!(budget.check(), Err(expected));
}

#[test]
fn strict_string_methods_work_through_direct_bound_getattr_and_frozen_routes() {
    let budget = strict_budget(1 << 20);
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("text", module.heap().alloc("fixture.雪"));
    module.set("pattern", module.heap().alloc("<{}>"));
    module.import_public_symbols(&freeze(
        "frozen_suffix = 'fixture.雪'.endswith\nfrozen_formatter = '<{}>'.format",
    ));
    let result = eval(
        &module,
        r#"
bound_suffix = text.endswith
bound_formatter = pattern.format
(
    text.endswith("雪"),
    bound_suffix("雪"),
    getattr(text, "endswith")("雪"),
    frozen_suffix("雪"),
    pattern.format("雪"),
    bound_formatter("雪"),
    getattr(pattern, "format")("雪"),
    frozen_formatter("雪"),
    getattr(text, "missing_method", "default"),
)
"#,
    )
    .unwrap();
    let tuple = TupleRef::from_value(result).unwrap();
    assert_eq!(tuple.len(), 9);
    for value in &tuple.content()[..4] {
        assert_eq!(value.unpack_bool(), Some(true));
    }
    for value in &tuple.content()[4..8] {
        assert_eq!(value.unpack_str(), Some("<雪>"));
    }
    assert_eq!(tuple.content()[8].unpack_str(), Some("default"));
    assert_eq!(budget.check(), Ok(()));
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct MethodTestReceiver;

impl fmt::Display for MethodTestReceiver {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("an unadmitted method unpacked its argument");
    }
}

starlark::starlark_simple_value!(MethodTestReceiver);

static TEST_METHODS: MethodsStatic = MethodsStatic::new();

#[starlark_value(type = "method_test_receiver")]
impl<'v> StarlarkValue<'v> for MethodTestReceiver {
    fn get_methods() -> Option<&'static Methods> {
        TEST_METHODS.methods(test_methods)
    }

    fn get_methods_bounded() -> Option<&'static Methods> {
        TEST_METHODS.get()
    }
}

#[starlark_module]
fn test_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn property(this: Value) -> anyhow::Result<i32> {
        let _ = this;
        panic!("an unadmitted native property ran");
    }
}

#[test]
fn strict_methods_refuse_lookup_and_frozen_invocation_before_unpack() {
    let frozen = freeze("frozen_find = 'abc'.find");
    for (source, operation) in [
        ("text.find(poison)", "attribute access"),
        ("text.find", "attribute access"),
        ("getattr(text, 'find', 'default')", "attribute access"),
        ("'//pkg:target'.name", "attribute access"),
        ("frozen_find(poison)", "method invocation"),
    ] {
        let budget = strict_budget(1 << 20);
        let module = Module::new_with_execution_budget(budget.clone());
        module.set("text", module.heap().alloc("abc"));
        module.set("poison", module.heap().alloc(MethodTestReceiver));
        module.import_public_symbols(&frozen);
        assert_limit(
            eval(&module, source).unwrap_err(),
            &budget,
            ExecutionLimit::UnsupportedOperation {
                operation,
                value_type: "string",
            },
        );
    }
}

#[test]
fn a_trusted_registry_does_not_admit_native_attributes() {
    MethodTestReceiver::get_methods().unwrap();
    let budget = strict_budget(1 << 20);
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("receiver", module.heap().alloc(MethodTestReceiver));
    assert_limit(
        eval(&module, "getattr(receiver, 'property', 'default')").unwrap_err(),
        &budget,
        ExecutionLimit::UnsupportedOperation {
            operation: "attribute access",
            value_type: "method_test_receiver",
        },
    );
}

#[test]
fn strict_string_binding_admits_the_observed_ordinary_wrapper_bytes() {
    let ordinary = Module::new();
    let heap = ordinary.heap();
    let text = heap.alloc("x");
    // Warm the ordinary arena before measuring one fixed object in its open chunk.
    text.get_attr("endswith", heap).unwrap().unwrap();
    let (reserved, available) = (heap.allocated_bytes(), heap.available_bytes());
    text.get_attr("endswith", heap).unwrap().unwrap();
    assert_eq!(heap.allocated_bytes(), reserved);
    let bytes = available - heap.available_bytes();
    assert!(bytes > 0);

    let short = strict_budget(bytes - 1);
    let module = Module::new_with_execution_budget(short.clone());
    let heap = module.heap();
    let text = heap.alloc("x");
    let before = (heap.allocated_bytes(), heap.available_bytes());
    assert_limit(
        text.get_attr("endswith", heap).unwrap_err(),
        &short,
        ExecutionLimit::AllocationBytes,
    );
    assert_eq!((heap.allocated_bytes(), heap.available_bytes()), before);

    let exact = strict_budget(bytes);
    let module = Module::new_with_execution_budget(exact.clone());
    let heap = module.heap();
    assert!(
        heap.alloc("x")
            .get_attr("endswith", heap)
            .unwrap()
            .is_some()
    );
    assert_eq!(exact.check(), Ok(()));
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
}
