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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::ValueTextMode;
use crate::values::string::StarlarkStr;

fn alloc_rendered_text<'v>(
    heap: Heap<'v>,
    budget: &ExecutionBudget,
    value: Value<'v>,
    mode: ValueTextMode,
) -> Result<StringValue<'v>, ExecutionLimit> {
    let mut text = BoundedText::new(budget, usize::MAX);
    text.write_value(value, mode)?;
    heap.alloc_budgeted_str(text.as_str())
}

#[starlark_module]
pub(crate) fn register_str(globals: &mut GlobalsBuilder) {
    /// [chr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#bool
    /// ): returns a string encoding a codepoint.
    ///
    /// `chr(i)` returns a string that encodes the single Unicode code
    /// point whose value is specified by the integer `i`. `chr` fails
    /// unless `0 ≤ i ≤ 0x10FFFF`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// chr(65) == 'A'
    /// chr(1049) == 'Й'
    /// chr(0x1F63F) == '😿'
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn chr(#[starlark(require = pos)] i: i32) -> anyhow::Result<char> {
        let cp = u32::try_from(i)
            .map_err(|_| anyhow::anyhow!("chr() parameter value negative integer {i}"))?;
        match char::from_u32(cp) {
            Some(x) => Ok(x),
            None => Err(anyhow::anyhow!(
                "chr() parameter value is 0x{:x} which is not a valid UTF-8 codepoint",
                cp
            )),
        }
    }

    /// [ord](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#ord
    /// ): returns the codepoint of a character
    ///
    /// `ord(s)` returns the integer value of the sole Unicode code point
    /// encoded by the string `s`.
    ///
    /// If `s` does not encode exactly one Unicode code point, `ord` fails.
    /// Each invalid code within the string is treated as if it encodes the
    /// Unicode replacement character, U+FFFD.
    ///
    /// Example:
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// ord("A")                                == 65
    /// ord("Й")                                == 1049
    /// ord("😿")                               == 0x1F63F
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn ord<'v>(#[starlark(require = pos)] a: StringValue<'v>) -> anyhow::Result<i32> {
        let mut chars = a.as_str().chars();
        if let Some(c) = chars.next() {
            if chars.next().is_none() {
                return Ok(u32::from(c) as i32);
            }
        }
        Err(anyhow::anyhow!(
            "ord(): {} is not a single character string",
            a.to_value().to_repr()
        ))
    }

    /// [repr](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#repr
    /// ): formats its argument as a string.
    ///
    /// All strings in the result are double-quoted.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// repr(1)                 == '1'
    /// repr("x")               == "\"x\""
    /// repr([1, "x"])          == "[1, \"x\"]"
    /// repr("test \"'")        == "\"test \\\"'\""
    /// repr("x\"y😿 \\'")      == "\"x\\\"y\\U0001f63f \\\\'\""
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn repr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        let heap = eval.heap();
        if let Some(budget) = heap.execution_budget() {
            return alloc_rendered_text(heap, budget, a, ValueTextMode::Repr).map_err(Into::into);
        }
        let mut s = eval.string_pool.alloc();
        a.collect_repr(&mut s);
        let r = heap.alloc_str(&s);
        eval.string_pool.release(s);
        Ok(r)
    }

    /// [str](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#str
    /// ): formats its argument as a string.
    ///
    /// If x is a string, the result is x (without quotation).
    /// All other strings, such as elements of a list of strings, are
    /// double-quoted.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// str(1)                          == '1'
    /// str("x")                        == 'x'
    /// str([1, "x"])                   == "[1, \"x\"]"
    /// # "#);
    /// ```
    #[starlark(as_type = StarlarkStr, speculative_exec_safe)]
    fn str<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        let heap = eval.heap();
        let budget = heap.execution_budget();
        if let Some(a) = StringValue::new(a) {
            // Special case that can avoid reallocating, but is equivalent.
            if let Some(budget) = budget {
                budget.check()?;
            }
            Ok(a)
        } else if let Some(budget) = budget {
            alloc_rendered_text(heap, budget, a, ValueTextMode::Str).map_err(Into::into)
        } else {
            let mut s = eval.string_pool.alloc();
            a.collect_str(&mut s);
            let r = heap.alloc_str(&s);
            eval.string_pool.release(s);
            Ok(r)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::ErrorKind;
    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::list::AllocList;

    fn call_builtin<'v>(
        module: &'v Module,
        globals: &'v Globals,
        name: &str,
        input: Value<'v>,
    ) -> crate::Result<Value<'v>> {
        module.frozen_heap().add_reference(globals.heap());
        Evaluator::new_with_reused_call_storage_for_test(module).eval_function(
            globals.get(name).unwrap(),
            &[input],
            &[],
        )
    }

    fn assert_allocation_limit(error: crate::Error) {
        match error.kind() {
            ErrorKind::Native(error) | ErrorKind::Other(error) => {
                assert_eq!(
                    error.downcast_ref::<ExecutionLimit>(),
                    Some(&ExecutionLimit::AllocationBytes),
                );
            }
            _ => panic!("expected native allocation refusal, got {error}"),
        }
    }

    #[test]
    fn budgeted_string_builtins_preserve_rendering() {
        let globals = Globals::standard();
        let source =
            r#"(str("hé\n"), repr("hé\n"), str([None, ("hé\n",)]), repr([None, ("hé\n",)]))"#;
        let evaluate = |module: &Module| {
            let ast = AstModule::parse("text.star", source.to_owned(), &Dialect::Standard).unwrap();
            Evaluator::new_with_reused_call_storage_for_test(module)
                .eval_module(ast, &globals)
                .unwrap()
                .to_repr()
        };
        let budget = Arc::new(ExecutionBudget::new(1_024, 4_096, 16));
        let budgeted = Module::new_with_execution_budget(budget.clone());
        assert_eq!(evaluate(&budgeted), evaluate(&Module::new()));
        assert_eq!(budget.check(), Ok(()));
    }

    #[test]
    fn budgeted_string_builtins_refuse_scratch_growth_and_stay_sticky() {
        let globals = Globals::standard();
        for name in ["str", "repr"] {
            let budget = Arc::new(ExecutionBudget::new(1_024, 0, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            let text = module.heap().alloc("hé\n");
            assert!(
                call_builtin(&module, &globals, "str", text)
                    .unwrap()
                    .ptr_eq(text)
            );

            let list = module.heap().alloc(AllocList([text]));
            let before = module.heap().allocated_bytes();
            assert_allocation_limit(call_builtin(&module, &globals, name, list).unwrap_err());
            assert_eq!(module.heap().allocated_bytes(), before);
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));

            let next = Module::new_with_execution_budget(budget);
            let text = next.heap().alloc("new row");
            assert_allocation_limit(call_builtin(&next, &globals, "str", text).unwrap_err());
        }
    }

    #[test]
    fn budgeted_string_builtins_charge_final_heap_copy() {
        let globals = Globals::standard();
        for (name, mode) in [("str", ValueTextMode::Str), ("repr", ValueTextMode::Repr)] {
            let allowance = "None".len();
            let scratch_budget = ExecutionBudget::new(1_024, allowance, 16);
            let mut scratch = BoundedText::new(&scratch_budget, usize::MAX);
            scratch.write_value(Value::new_none(), mode).unwrap();
            assert_eq!(scratch.as_str(), "None");

            let budget = Arc::new(ExecutionBudget::new(1_024, allowance, 16));
            let module = Module::new_with_execution_budget(budget);
            let before = module.heap().allocated_bytes();
            assert_allocation_limit(
                call_builtin(&module, &globals, name, Value::new_none()).unwrap_err(),
            );
            assert_eq!(module.heap().allocated_bytes(), before);
        }
    }
}
