/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::eval::Arguments;
use crate::eval::ParametersSpec;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;

/// Collect `N` arguments.
///
/// This function is called by generated code.
#[inline]
pub fn parse_signature<'v, const N: usize>(
    parser: &ParametersSpec<FrozenValue>,
    args: &Arguments<'v, '_>,
    heap: Heap<'v>,
) -> crate::Result<[Option<Value<'v>>; N]> {
    parser.collect_into(args, heap)
}

/// Parse positional-only arguments, required and optional.
#[inline(always)]
pub fn parse_positional<'v, const R: usize, const O: usize>(
    args: &Arguments<'v, '_>,
    heap: Heap<'v>,
) -> crate::Result<([Value<'v>; R], [Option<Value<'v>>; O])> {
    if let Some(budget) = heap.execution_budget() {
        args.bounded_no_named_args(budget)?;
    } else {
        args.no_named_args()?;
    }
    args.optional(heap)
}

#[inline(always)]
pub fn parse_positional_kwargs_alloc<'v, 'a, const R: usize, const O: usize>(
    args: &'a Arguments<'v, 'a>,
    heap: Heap<'v>,
) -> crate::Result<([Value<'v>; R], [Option<Value<'v>>; O], Value<'v>)> {
    let (required, optional) = args.optional(heap)?;
    let kwargs = if let Some(budget) = heap.execution_budget() {
        args.bounded_format_names(budget)?
            .alloc_dict(heap, budget)?
    } else {
        heap.alloc(args.names_map()?)
    };
    Ok((required, optional, kwargs))
}

/// Utility for checking a `this` parameter matches what you expect.
#[inline]
pub fn check_this<'v, T: UnpackValue<'v>>(this: Value<'v>) -> crate::Result<T> {
    T::unpack_named_param(this, "this")
}

/// Utility for checking a required parameter matches what you expect.
#[inline]
pub fn check_required<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
) -> crate::Result<T> {
    let x = x.ok_or_else(|| ValueError::MissingRequired(name.to_owned()))?;
    T::unpack_named_param(x, name)
}

/// Utility for checking an optional parameter matches what you expect.
#[inline]
pub fn check_optional<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
) -> crate::Result<Option<T>> {
    match x {
        None => Ok(None),
        Some(x) => Ok(Some(T::unpack_named_param(x, name)?)),
    }
}

#[inline]
pub fn check_defaulted<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
    default: impl FnOnce() -> T,
) -> crate::Result<T> {
    Ok(check_optional(name, x)?.unwrap_or_else(default))
}

/// We already know the parameter is set, so we just unpack it.
#[inline]
pub fn check_unpack<'v, T: UnpackValue<'v>>(name: &str, x: Value<'v>) -> crate::Result<T> {
    T::unpack_named_param(x, name)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::Evaluator;
    use crate::eval::ExecutionBudget;
    use crate::eval::ExecutionLimit;
    use crate::eval::compiler::def::FrozenDef;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::Heap;
    use crate::values::Value;
    use crate::values::ValueLike;
    use crate::values::dict::AllocDict;
    use crate::values::dict::Dict;
    use crate::values::dict::DictMut;
    use crate::values::dict::DictRef;

    #[starlark_module]
    fn globals(builder: &mut GlobalsBuilder) {
        fn choose<'v>(
            #[starlark(require = pos)] first: Value<'v>,
            #[starlark(require = pos)] second: Option<Value<'v>>,
        ) -> anyhow::Result<Value<'v>> {
            Ok(second.unwrap_or(first))
        }

        fn collect_named<'v>(
            #[starlark(require = pos)] _first: Value<'v>,
            #[starlark(kwargs)] kwargs: Value<'v>,
        ) -> anyhow::Result<Value<'v>> {
            Ok(kwargs)
        }
    }

    fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
        let globals = GlobalsBuilder::new().with(globals).build();
        let ast = AstModule::parse("call.star", source.to_owned(), &Dialect::Standard).unwrap();
        Evaluator::new_with_reused_call_storage_for_test(module).eval_module(ast, &globals)
    }

    #[test]
    fn generated_fixed_arity_star_needs_no_scratch() {
        let budget = Arc::new(ExecutionBudget::new(1000, 0, 16));
        let caller = Module::new_with_execution_budget(budget.clone());
        let value = caller.heap().alloc("same value");
        let tail = caller.heap().alloc_tuple(&[value]);
        caller.set("tail", tail);
        assert!(eval(&caller, "choose(0, *tail)").unwrap().ptr_eq(value));

        let producer = Module::new();
        eval(&producer, "def call(tail): return choose(0, *tail)").unwrap();
        let producer = producer.freeze().unwrap();
        let program = producer.get("call").unwrap();
        let bytecode = program
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(bytecode.contains("CallFrozenNative"), "{bytecode}");
        let program = program.owned_value(caller.frozen_heap());
        let result = Evaluator::new_with_reused_call_storage_for_test(&caller)
            .eval_function(program, &[tail], &[])
            .unwrap();
        assert!(result.ptr_eq(value));
        assert!(budget.check().is_ok());
    }

    #[test]
    fn generated_fixed_arity_star_counts_past_arity_with_work_budget() {
        let budget = Arc::new(ExecutionBudget::new(64, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        module.set("tail", module.heap().alloc_list(&[Value::new_none(); 4096]));
        assert!(eval(&module, "choose(*tail)").is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::Work));
    }

    #[test]
    fn generated_fixed_arity_preserves_argument_error_order() {
        let budget = Arc::new(ExecutionBudget::new(1000, 4096, 16));
        let budgeted = Module::new_with_execution_budget(budget.clone());
        let ordinary = Module::new();
        for module in [&budgeted, &ordinary] {
            let heap = module.heap();
            let value = Value::new_none();
            module.set("scalar", value);
            module.set("tail", heap.alloc_tuple(&[value; 4]));
            module.set(
                "kwargs",
                heap.alloc(AllocDict([("tail", value), ("extra", value)])),
            );
            module.set("bad_keys", heap.alloc(AllocDict([(0, value)])));
        }
        for (source, expected) in [
            ("choose(*scalar)", "(iter)"),
            (
                "choose(*tail)",
                "Wrong number of positional arguments, expected between 1 and 2, got 4",
            ),
            (
                "choose(extra=0, *scalar, **kwargs)",
                "Found `extra` `tail` `extra` extra named parameter(s) for call to function",
            ),
            (
                "choose(extra=0, *scalar, **bad_keys)",
                "The argument provided for *args is not an identifier",
            ),
            (
                "collect_named(*(), **scalar)",
                "Wrong number of positional arguments, expected 1, got 0",
            ),
        ] {
            let ordinary = eval(&ordinary, source).unwrap_err();
            let limited = eval(&budgeted, source).unwrap_err();
            let message = limited.without_diagnostic().to_string();
            assert_eq!(
                message,
                ordinary.without_diagnostic().to_string(),
                "{source}"
            );
            assert!(message.contains(expected), "{source}: {message}");
            assert!(budget.check().is_ok());
        }
    }

    #[test]
    fn generated_positional_only_admits_keyword_diagnostic_before_copy() {
        let budget = Arc::new(ExecutionBudget::new(1000, 64, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let key = module.heap().alloc("x".repeat(16 * 1024));
        let kwargs = module.heap().alloc(AllocDict([(key, Value::new_none())]));
        module.set("kwargs", kwargs);
        assert!(eval(&module, "choose(0, **kwargs)").is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
    }

    #[test]
    fn generated_kwargs_capture_preserves_order_and_uses_fresh_mutable_dict() {
        let budget = Arc::new(ExecutionBudget::new(1000, 4096, 16));
        let caller = Module::new_with_execution_budget(budget.clone());
        let tail = caller.heap().alloc(AllocDict([("z", 2), ("a", 3)]));
        caller.set("tail", tail);
        let direct = eval(&caller, "collect_named(0, head=1, **tail)").unwrap();

        let producer = Module::new();
        eval(
            &producer,
            "def call(tail): return collect_named(0, head=1, **tail)",
        )
        .unwrap();
        let producer = producer.freeze().unwrap();
        let program = producer.get("call").unwrap();
        let bytecode = program
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(bytecode.contains("CallFrozenNative"), "{bytecode}");
        let program = program.owned_value(caller.frozen_heap());
        let frozen_call = Evaluator::new_with_reused_call_storage_for_test(&caller)
            .eval_function(program, &[tail], &[])
            .unwrap();

        for result in [direct, frozen_call] {
            assert!(!result.ptr_eq(tail));
            assert!(DictMut::from_value(result).is_ok());
            let captured = DictRef::from_value(result).unwrap();
            let rows: Vec<_> = captured
                .iter()
                .map(|(key, value)| (key.unpack_str().unwrap(), value.unpack_i32().unwrap()))
                .collect();
            assert_eq!(rows, [("head", 1), ("z", 2), ("a", 3)]);
            let original = DictRef::from_value(tail).unwrap();
            for (new, old) in captured.keys().skip(1).zip(original.keys()) {
                assert!(new.ptr_eq(old));
            }
        }
        assert!(budget.check().is_ok());
    }

    #[test]
    fn generated_kwargs_capture_preserves_collision_diagnostic_order_and_budget() {
        for module in [
            Module::new(),
            Module::new_with_execution_budget(Arc::new(ExecutionBudget::new(1000, 4096, 16))),
        ] {
            let heap = module.heap();
            let repeated = (heap.alloc("repeated"), 1);
            let malformed = (heap.alloc(0), 2);
            module.set(
                "duplicate_first",
                heap.alloc(AllocDict([repeated, malformed])),
            );
            module.set(
                "malformed_first",
                heap.alloc(AllocDict([malformed, repeated])),
            );
            for (name, message) in [
                (
                    "duplicate_first",
                    "Argument `repeated` occurs more than once",
                ),
                (
                    "malformed_first",
                    "The argument provided for *args is not an identifier",
                ),
            ] {
                let result = eval(&module, &format!("collect_named(0, repeated=0, **{name})"));
                assert_eq!(
                    result.unwrap_err().without_diagnostic().to_string(),
                    message
                );
            }
        }

        let budget = Arc::new(ExecutionBudget::new(1000, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let repeated = module.heap().alloc(AllocDict([("repeated", 1)]));
        module.set("tail", repeated);
        assert!(eval(&module, "collect_named(0, repeated=0, **tail)").is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
    }

    #[test]
    fn generated_kwargs_capture_admits_exact_empty_dict_arena_object() {
        let bytes = Heap::temp(|heap| {
            heap.alloc(Dict::default())
                .get_ref()
                .total_memory_for_profile()
        });
        for allowance in [bytes - 1, bytes] {
            let budget = Arc::new(ExecutionBudget::new(1000, allowance, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            let result = eval(&module, "collect_named(0)");
            let summary = module.heap().allocated_summary().summary();
            if allowance == bytes {
                assert!(DictRef::from_value(result.unwrap()).unwrap().is_empty());
                assert_eq!(summary.get(Dict::TYPE), Some(&(1, bytes)));
                assert!(budget.check().is_ok());
            } else {
                assert!(result.is_err());
                assert!(!summary.contains_key(Dict::TYPE));
                assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
            }
        }
    }

    #[cfg(all(target_endian = "little", target_pointer_width = "64"))]
    #[test]
    fn generated_kwargs_capture_budgets_indexed_collisions() {
        use std::collections::BTreeMap;

        use crate::collections::StarlarkHashValue;

        // Identical low bytes in a single FxHash word give many genuine collisions.
        // Forty distinct keys require the indexed SmallMap path on all toolchains.
        let mut groups = BTreeMap::<u32, Vec<String>>::new();
        let keys = (0..4096)
            .find_map(|i| {
                let key = format!("key-{i:04}");
                let hash = StarlarkHashValue::new(key.as_str()).get();
                let group = groups.entry(hash).or_default();
                group.push(key);
                (group.len() == 40).then(|| std::mem::take(group))
            })
            .unwrap();

        for (work, bytes, expected) in [
            (10000, 65536, Ok(())),
            // Enough for validation, control initialization, and copying rows.
            (220, 65536, Err(ExecutionLimit::Work)),
            (10000, 8, Err(ExecutionLimit::AllocationBytes)),
        ] {
            let budget = Arc::new(ExecutionBudget::new(work, bytes, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            let rows = keys
                .iter()
                .enumerate()
                .map(|(index, key)| (key.as_str(), index as i32));
            module.set("tail", module.heap().alloc(AllocDict(rows)));
            let result = eval(&module, "collect_named(0, **tail)");
            assert_eq!(budget.check(), expected);
            if expected.is_ok() {
                let captured = DictRef::from_value(result.unwrap()).unwrap();
                assert_eq!(captured.len(), keys.len());
                for (index, ((key, value), expected)) in captured.iter().zip(&keys).enumerate() {
                    assert_eq!(key.unpack_str(), Some(expected.as_str()));
                    assert_eq!(value.unpack_i32(), Some(index as i32));
                    assert_eq!(
                        captured.get_str(expected).unwrap().unpack_i32(),
                        Some(index as i32)
                    );
                }
            } else {
                assert!(result.is_err());
                let summary = module.heap().allocated_summary().summary();
                assert_eq!(summary.get(Dict::TYPE).unwrap().0, 1);
            }
        }
    }
}
