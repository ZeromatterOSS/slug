use std::sync::Arc;

use starlark_derive::starlark_module;

use super::ParametersSpec;
use super::ParametersSpecParam;
use crate as starlark;
use crate::collections::Hashed;
use crate::collections::symbol::symbol::Symbol;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::runtime::arguments::ArgNames;
use crate::eval::runtime::arguments::ArgumentsFull;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::DialectTypes;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::dict::AllocImmutableDict;
use crate::values::dict::Dict;
use crate::values::dict::DictMut;
use crate::values::dict::DictRef;
use crate::values::tuple::TupleRef;

#[starlark_module]
fn keyword_test_globals(globals: &mut GlobalsBuilder) {
    fn native_capture<'v>(
        #[starlark(require = pos)] _pos: Value<'v>,
        first: Value<'v>,
        #[starlark(require = named)] named: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        Ok(heap.alloc((first, named, kwargs)))
    }
}

fn eval<'v>(module: &'v Module, source: &str, globals: &Globals) -> crate::Result<Value<'v>> {
    let dialect = Dialect {
        enable_types: DialectTypes::Disable,
        ..Dialect::AllOptionsInternal
    };
    let ast = AstModule::parse("keywords.star", source.to_owned(), &dialect).unwrap();
    Evaluator::new(module).eval_module(ast, globals)
}

fn spread<'v: 'a, 'a>(kwargs: Value<'v>) -> Arguments<'v, 'a> {
    Arguments(ArgumentsFull {
        kwargs: Some(kwargs),
        ..ArgumentsFull::default()
    })
}

#[test]
fn bounded_signature_keywords_match_dynamic_resolved_and_generated_calls() {
    const SOURCE: &str = r#"
def capture(pos, /, first, *args, named, **kwargs):
    return (first, named, kwargs)
def via_def(tail):
    return capture(0, first=1, before=2, pos=3, **tail)
def via_native(tail):
    return native_capture(0, first=1, before=2, _pos=3, **tail)
"#;
    const TAIL: [(&str, i32); 4] = [("*args", 4), ("named", 5), ("**kwargs", 6), ("last", 7)];
    let globals = GlobalsBuilder::standard()
        .with(keyword_test_globals)
        .build();
    let producer = Module::new();
    producer.set("frozen_tail", producer.heap().alloc(AllocDict(TAIL)));
    eval(&producer, SOURCE, &globals).unwrap();
    let producer = producer.freeze().unwrap();
    let frozen_tail = producer.get("frozen_tail").unwrap();
    let def = producer.get("via_def").unwrap();
    let native = producer.get("via_native").unwrap();
    for (wrapper, expected_opcode) in [(&def, "CallFrozenDef"), (&native, "CallFrozenNative")] {
        let code = wrapper
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(code.contains(expected_opcode), "{code}");
    }

    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16));
    for caller in [
        Module::new(),
        Module::new_with_execution_budget(budget.clone()),
    ] {
        eval(&caller, SOURCE, &globals).unwrap();
        for input in [
            caller.heap().alloc(AllocDict(TAIL)),
            caller.heap().alloc(AllocImmutableDict(TAIL)),
            frozen_tail.owned_value(caller.frozen_heap()),
        ] {
            caller.set("input", input);
            let dynamic_def = eval(&caller, "via_def(input)", &globals).unwrap();
            let dynamic_native = eval(&caller, "via_native(input)", &globals).unwrap();
            let mut evaluator = Evaluator::new(&caller);
            let resolved = evaluator
                .eval_function(def.owned_value(caller.frozen_heap()), &[input], &[])
                .unwrap();
            let frozen_native = evaluator
                .eval_function(native.owned_value(caller.frozen_heap()), &[input], &[])
                .unwrap();
            for (result, pos_only) in [
                (dynamic_def, "pos"),
                (dynamic_native, "_pos"),
                (resolved, "pos"),
                (frozen_native, "_pos"),
            ] {
                let tuple = TupleRef::from_value(result).unwrap();
                let &[first, named, output] = tuple.content() else {
                    panic!("unexpected result: {result}");
                };
                assert_eq!(first.unpack_i32(), Some(1));
                assert_eq!(named.unpack_i32(), Some(5));
                assert!(!output.ptr_eq(input));
                assert!(DictMut::from_value(output).is_ok());
                let output = DictRef::from_value(output).unwrap();
                assert!(
                    output
                        .iter()
                        .map(|(key, value)| {
                            (key.unpack_str().unwrap(), value.unpack_i32().unwrap())
                        })
                        .eq([
                            ("before", 2),
                            (pos_only, 3),
                            ("*args", 4),
                            ("**kwargs", 6),
                            ("last", 7)
                        ])
                );
                let input = DictRef::from_value(input).unwrap();
                assert!(
                    output
                        .keys()
                        .skip(2)
                        .zip(input.keys().filter(|key| key.unpack_str() != Some("named")))
                        .all(|(new, old)| new.ptr_eq(old))
                );
            }
        }
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_signature_keywords_preserve_error_precedence() {
    let globals = Globals::standard();
    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16));
    let ordinary = Module::new();
    let bounded = Module::new_with_execution_budget(budget.clone());
    for module in [&ordinary, &bounded] {
        eval(module, "def target(a, b): return a", &globals).unwrap();
        let heap = module.heap();
        let duplicate = (heap.alloc("extra"), 1);
        let malformed = (heap.alloc(0), 2);
        module.set(
            "duplicate_first",
            heap.alloc(AllocDict([duplicate, malformed])),
        );
        module.set(
            "malformed_first",
            heap.alloc(AllocDict([malformed, duplicate])),
        );
        module.set(
            "known_first",
            heap.alloc(AllocDict([(heap.alloc("a"), 1), malformed])),
        );
        module.set("clean", heap.alloc(AllocDict([("z", 1), ("tail", 2)])));
        module.set("scalar", Value::new_none());
        module.set("positions", heap.alloc_tuple(&[Value::new_none()]));
    }

    for (source, expected) in [
        (
            "target(a=1, *scalar, **malformed_first)",
            "*args is not iterable",
        ),
        (
            "target(a=1, *positions, **scalar)",
            "Argument `a` occurs more than once",
        ),
        (
            "target(1, extra=7, **duplicate_first)",
            "Argument `extra` occurs more than once",
        ),
        (
            "target(1, extra=7, **malformed_first)",
            "*args is not an identifier",
        ),
        (
            "target(1, **known_first)",
            "Argument `a` occurs more than once",
        ),
        (
            "target(1, extra=7, **scalar)",
            "**kwargs is not a dictionary",
        ),
        ("target(1, extra=7, **clean)", "Missing parameter `b`"),
        (
            "target(1, 2, 3, extra=7, **clean)",
            "Found 1 extra positional argument(s)",
        ),
        (
            "target(1, 2, extra=7, **clean)",
            "Found `extra` `z` `tail` extra named parameter(s)",
        ),
    ] {
        let ordinary = eval(&ordinary, source, &globals).unwrap_err();
        let bounded = eval(&bounded, source, &globals).unwrap_err();
        let message = bounded.without_diagnostic().to_string();
        assert!(
            matches!(bounded.kind(), crate::ErrorKind::Function(_)),
            "{bounded}"
        );
        assert_eq!(
            message,
            ordinary.without_diagnostic().to_string(),
            "{source}"
        );
        assert!(message.contains(expected), "{source}: {message}");
        assert_eq!(budget.check(), Ok(()));
    }
}

#[test]
fn bounded_signature_keywords_admit_full_long_matches_and_use_resolved_slots() {
    let name = "long_".repeat(4096);
    let spec = ParametersSpec::<FrozenValue>::new_named_only(
        "long_name",
        [(name.as_str(), ParametersSpecParam::Required)],
    );
    for (work, expected) in [(1000, Ok(())), (16, Err(ExecutionLimit::Work))] {
        for expanded in [false, true] {
            let budget = Arc::new(ExecutionBudget::new(work, 0, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            let heap = module.heap();
            let value = Value::testing_new_int(19);
            let named = [value];
            let name_value = heap.alloc_str(&name);
            let names = [(Symbol::new(&name), name_value)];
            let args = if expanded {
                spread(heap.alloc(AllocDict([(name_value, value)])))
            } else {
                Arguments(ArgumentsFull {
                    named: &named,
                    names: ArgNames::new_unique(&names),
                    ..ArgumentsFull::default()
                })
            };
            let mut slots = [None];
            let result = spec.collect(&args, &mut slots, heap);
            assert_eq!(budget.check(), expected);
            assert_eq!(result.is_ok(), expected.is_ok());
            assert_eq!(
                slots[0].and_then(Value::unpack_i32),
                expected.ok().map(|()| 19)
            );
        }
    }

    let budget = Arc::new(ExecutionBudget::new(16, 0, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let names = [(
        spec.resolve_name(Hashed::new(name.as_str())),
        heap.alloc_str(&name),
    )];
    let args = ArgumentsFull {
        named: &[Value::testing_new_int(19)],
        names: ArgNames::new_unique(&names),
        ..ArgumentsFull::default()
    };
    let mut slots = [None];
    spec.collect_inline(&args, &mut slots, heap).unwrap();
    assert_eq!(slots[0].unwrap().unpack_i32(), Some(19));
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_signature_keywords_admit_diagnostic_text_before_copying() {
    let budget = Arc::new(ExecutionBudget::new(10_000, 4_096, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let key = heap.alloc("extra".repeat(4096));
    let args = spread(heap.alloc(AllocDict([(key, Value::new_none())])));
    let spec = ParametersSpec::<FrozenValue>::new_named_only("empty", []);
    assert!(spec.collect(&args, &mut [], heap).is_err());
    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
}

#[test]
fn bounded_signature_keywords_refuse_before_publishing_capture() {
    let spec = ParametersSpec::<FrozenValue>::new_parts("capture", [], [], false, [], true);
    let keys: Vec<_> = (0..40).map(|index| format!("key_{index}")).collect();
    for (work, bytes, expected) in [
        (10_000, 1 << 20, Ok(())),
        (300, 1 << 20, Err(ExecutionLimit::Work)),
        (10_000, 0, Err(ExecutionLimit::AllocationBytes)),
    ] {
        let budget = Arc::new(ExecutionBudget::new(work, bytes, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let input = heap.alloc(AllocDict(keys.iter().map(|key| (key.as_str(), 7))));
        let mut slots = [None];
        let result = spec.collect(&spread(input), &mut slots, heap);
        assert_eq!(budget.check(), expected);
        assert!(DictMut::from_value(input).is_ok());
        if expected.is_ok() {
            result.unwrap();
            module.set("output", slots[0].unwrap());
            let module = module.freeze().unwrap();
            let output = module.get("output").unwrap();
            let output = DictRef::from_value(output.value().to_value()).unwrap();
            assert!(
                output
                    .keys()
                    .map(|key| key.unpack_str().unwrap())
                    .eq(keys.iter().map(String::as_str))
            );
        } else {
            assert!(result.is_err());
            assert!(slots[0].is_none());
            assert_eq!(
                heap.allocated_summary()
                    .summary()
                    .get(Dict::TYPE)
                    .unwrap()
                    .0,
                1
            );
        }
    }
}

#[test]
fn bounded_signature_keywords_charge_exact_empty_capture_object() {
    let spec = ParametersSpec::<FrozenValue>::new_parts("capture", [], [], false, [], true);
    let object_bytes = Heap::temp(|heap| {
        heap.alloc(Dict::default())
            .get_ref()
            .total_memory_for_profile()
    });
    for allowance in [object_bytes - 1, object_bytes] {
        let budget = Arc::new(ExecutionBudget::new(1000, allowance, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let mut slots = [None];
        let result = spec.collect(&Arguments::default(), &mut slots, module.heap());
        let allocated = module.heap().allocated_summary().summary();
        if allowance == object_bytes {
            result.unwrap();
            assert!(DictRef::from_value(slots[0].unwrap()).unwrap().is_empty());
            assert_eq!(allocated.get(Dict::TYPE), Some(&(1, object_bytes)));
            assert_eq!(budget.check(), Ok(()));
        } else {
            assert!(result.is_err());
            assert!(slots[0].is_none());
            assert!(!allocated.contains_key(Dict::TYPE));
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        }
    }
}
