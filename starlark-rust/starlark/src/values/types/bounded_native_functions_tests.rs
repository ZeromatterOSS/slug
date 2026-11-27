use std::fmt;
use std::sync::Arc;

use allocative::Allocative;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::LibraryExtension;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::starlark_value;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ReprProbe;

starlark::starlark_simple_value!(ReprProbe);

impl fmt::Display for ReprProbe {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("generated native argument unpack rendered its operand");
    }
}

#[starlark_value(type = "native_repr_probe")]
impl<'v> StarlarkValue<'v> for ReprProbe {
    fn collect_repr(&self, _: &mut String) {
        panic!("generated native argument unpack rendered its operand");
    }
}

#[starlark_module]
fn register_probes(builder: &mut GlobalsBuilder) {
    fn legacy(value: &str) -> anyhow::Result<bool> {
        Ok(!value.is_empty())
    }

    #[starlark(bounded_function)]
    fn identity<'v>(#[starlark(require = pos)] value: Value<'v>) -> anyhow::Result<Value<'v>> {
        Ok(value)
    }
}

fn eval<'v>(module: &'v Module, globals: &Globals, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("native.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, globals)
}

fn strict_budget() -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(10_000, 1 << 20, 16).with_bounded_operations())
}

fn assert_native_refusal(error: crate::Error, budget: &ExecutionBudget) {
    let limit = ExecutionLimit::UnsupportedOperation {
        operation: "function invocation",
        value_type: "function",
    };
    assert!(
        matches!(error.kind(), crate::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&limit)),
        "{error}"
    );
    assert_eq!(budget.check(), Err(limit));
}

fn assert_opcode(function: Value<'_>, opcode: &str) {
    let bytecode = function.downcast_ref::<FrozenDef>().unwrap().dump_debug();
    assert!(
        bytecode.lines().any(|line| {
            line.split_once(": ")
                .and_then(|(_, instruction)| instruction.split_whitespace().next())
                == Some(opcode)
        }),
        "expected {opcode} in {bytecode}"
    );
}

#[test]
fn bounded_native_wrappers_gate_dynamic_and_frozen_calls_before_unpack() {
    let mut builder = GlobalsBuilder::standard();
    register_probes(&mut builder);
    let globals = builder.build();
    let producer = Module::new();
    assert_eq!(
        eval(&producer, &globals, "legacy('ordinary')")
            .unwrap()
            .unpack_bool(),
        Some(true),
    );
    eval(
        &producer,
        &globals,
        "saved = legacy\nsaved_identity = identity\n\
         def call_pos(value): return legacy(value)\n\
         def call_named(value): return legacy(value=value)",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();

    for source in ["legacy(probe)", "saved(probe)", "str(probe)"] {
        let budget = strict_budget();
        let module = Module::new_with_execution_budget(budget.clone());
        module.import_public_symbols(&producer);
        if source == "str(probe)" {
            module.set(
                "str",
                producer
                    .get("saved")
                    .unwrap()
                    .owned_value(module.frozen_heap()),
            );
        }
        module.set("probe", module.heap().alloc(ReprProbe));
        assert_native_refusal(eval(&module, &globals, source).unwrap_err(), &budget);
    }

    for (name, opcode) in [
        ("call_pos", "CallFrozenNativePos"),
        ("call_named", "CallFrozenNative"),
    ] {
        let function = producer.get(name).unwrap();
        assert_opcode(function.value(), opcode);
        let budget = strict_budget();
        let caller = Module::new_with_execution_budget(budget.clone());
        let function = function.owned_value(caller.frozen_heap());
        let probe = caller.heap().alloc(ReprProbe);
        let error = Evaluator::new(&caller)
            .eval_function(function, &[probe], &[])
            .unwrap_err();
        assert_native_refusal(error, &budget);
    }

    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let probe = module.heap().alloc(ReprProbe);
    module.set("probe", probe);
    module.import_public_symbols(&producer);
    for source in ["identity(probe)", "saved_identity(probe)"] {
        assert!(eval(&module, &globals, source).unwrap().ptr_eq(probe));
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_evaluator_refuses_frozen_type_and_isinstance_intrinsics() {
    let globals = Globals::extended_by(&[LibraryExtension::Typing]);
    let producer = Module::new();
    eval(
        &producer,
        &globals,
        "def raw_type(value): return type(value)\n\
         def type_equals(value): return type(value) == 'int'\n\
         def instance(value): return isinstance(value, int)",
    )
    .unwrap();
    let producer = producer.freeze().unwrap();
    for (name, opcode) in [
        ("raw_type", "Type"),
        ("type_equals", "TypeIs"),
        ("instance", "IsInstance"),
    ] {
        let function = producer.get(name).unwrap();
        assert_opcode(function.value(), opcode);
        let budget = strict_budget();
        let caller = Module::new_with_execution_budget(budget.clone());
        let function = function.owned_value(caller.frozen_heap());
        let error = Evaluator::new(&caller)
            .eval_function(function, &[Value::testing_new_int(1)], &[])
            .unwrap_err();
        assert_native_refusal(error, &budget);
    }
}

#[test]
fn admitted_str_and_sorted_validate_reverse_without_legacy_repr() {
    let globals = Globals::standard();
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    assert_eq!(
        eval(&module, &globals, "str(sorted(['b', 'a']))")
            .unwrap()
            .unpack_str(),
        Some("[\"a\", \"b\"]"),
    );
    assert_eq!(
        eval(&module, &globals, "str(sorted(['a', 'b'], reverse=True))")
            .unwrap()
            .unpack_str(),
        Some("[\"b\", \"a\"]"),
    );
    module.set("probe", module.heap().alloc(ReprProbe));
    let error = eval(&module, &globals, "sorted(probe, reverse=probe)").unwrap_err();
    let crate::ErrorKind::Value(cause) = error.kind() else {
        panic!("expected a reverse argument type error, got {error}");
    };
    assert_eq!(
        cause.to_string(),
        "Type of parameter `reverse` doesn't match, expected `bool`, actual `native_repr_probe`"
    );
    assert_eq!(budget.check(), Ok(()));
}
