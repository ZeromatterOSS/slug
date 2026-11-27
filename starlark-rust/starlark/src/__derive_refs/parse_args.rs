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
    let kwargs = args.names_map()?;
    let kwargs = heap.alloc(kwargs);
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
    use crate::values::Value;
    use crate::values::ValueLike;
    use crate::values::dict::AllocDict;

    #[starlark_module]
    fn globals(builder: &mut GlobalsBuilder) {
        fn choose<'v>(
            #[starlark(require = pos)] first: Value<'v>,
            #[starlark(require = pos)] second: Option<Value<'v>>,
        ) -> anyhow::Result<Value<'v>> {
            Ok(second.unwrap_or(first))
        }

        fn collect_named<'v>(
            #[starlark(require = pos)] first: Value<'v>,
            #[starlark(kwargs)] _kwargs: Value<'v>,
        ) -> anyhow::Result<Value<'v>> {
            Ok(first)
        }
    }

    fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
        let globals = GlobalsBuilder::new().with(globals).build();
        let ast = AstModule::parse("call.star", source.to_owned(), &Dialect::Standard).unwrap();
        Evaluator::new(module).eval_module(ast, &globals)
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
        let result = Evaluator::new(&caller)
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
}
