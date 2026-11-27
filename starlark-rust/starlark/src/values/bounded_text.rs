/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::fmt;
use std::fmt::Write;

use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::StarlarkBigInt;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::DictRef;
use crate::values::float::StarlarkFloat;
use crate::values::list::ListRef;
use crate::values::structs::StructRef;
use crate::values::tuple::TupleRef;
use crate::values::types::float::float::write_compact;
use crate::values::types::string::repr::write_string_repr;

// Keep the Rust call stack bounded even if the caller supplies a larger limit.
const MAX_TEXT_DEPTH: usize = 64;

/// The Starlark representation to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueTextMode {
    /// The representation used by `str` and `%s`, with unquoted strings.
    Str,
    /// The representation used by `repr` and `%r`.
    Repr,
    /// The compact representation used by Starlark's Rust `Display` implementation.
    Display,
}

impl ValueTextMode {
    fn list_or_dict_element(self) -> Self {
        match self {
            Self::Str | Self::Repr => Self::Repr,
            Self::Display => Self::Display,
        }
    }
}

/// Scratch text with bounded rendering, growth, and traversal.
pub struct BoundedText<'a> {
    budget: &'a ExecutionBudget,
    max_bytes: usize,
    output: String,
}

impl<'a> BoundedText<'a> {
    /// Creates an empty writer with a byte limit and a shared execution budget.
    pub fn new(budget: &'a ExecutionBudget, max_bytes: usize) -> Self {
        Self {
            budget,
            max_bytes,
            output: String::new(),
        }
    }

    /// Appends borrowed text after admitting work and any needed allocation.
    pub fn push_str(&mut self, text: &str) -> Result<(), ExecutionLimit> {
        self.budget.check()?;
        let len = self.output.len();
        let new_len = len
            .checked_add(text.len())
            .filter(|&new_len| new_len <= self.max_bytes)
            .ok_or_else(|| self.budget.refuse(ExecutionLimit::TextBytes))?;
        self.budget.charge_text_work(text.len())?;

        let capacity = self.output.capacity();
        if new_len > capacity {
            let next_capacity = new_len.max(capacity.saturating_mul(2)).min(self.max_bytes);
            // Growth can relocate all bytes in the previous allocation.
            self.budget.charge_text_work(capacity)?;
            self.budget.charge(0, next_capacity)?;
            self.output
                .try_reserve_exact(next_capacity - len)
                .map_err(|_| self.budget.refuse(ExecutionLimit::AllocationFailed))?;
        }
        self.output.push_str(text);
        Ok(())
    }

    /// Appends a value using the requested representation.
    ///
    /// Depth counts the root as one level. Native values require an explicit
    /// bounded renderer. An error can leave a partial value in scratch.
    pub fn write_value(
        &mut self,
        value: Value<'_>,
        mode: ValueTextMode,
    ) -> Result<(), ExecutionLimit> {
        self.write_value_at(value, mode, 0, &mut [Value::new_none(); MAX_TEXT_DEPTH])
    }

    /// Borrows the scratch text. Publish it only after checking the operation result.
    pub fn as_str(&self) -> &str {
        &self.output
    }

    /// Returns the text if the shared budget has accepted all work.
    pub fn finish(self) -> Result<String, ExecutionLimit> {
        self.budget.check()?;
        Ok(self.output)
    }

    fn write_value_at<'v>(
        &mut self,
        value: Value<'v>,
        mode: ValueTextMode,
        depth: usize,
        ancestors: &mut [Value<'v>; MAX_TEXT_DEPTH],
    ) -> Result<(), ExecutionLimit> {
        self.budget.charge(1, 0)?;
        if depth >= self.budget.text_depth().min(MAX_TEXT_DEPTH) {
            return Err(self.budget.refuse(ExecutionLimit::TextDepth));
        }

        if let Some(text) = value.unpack_str() {
            if mode == ValueTextMode::Str {
                return self.push_str(text);
            }
            self.budget.charge_text_work(text.len())?;
            let result = write_string_repr(text, self);
            return self.check_format(result);
        }
        if value.is_none() {
            return self.push_str("None");
        }
        if let Some(boolean) = value.unpack_bool() {
            return self.push_str(if boolean { "True" } else { "False" });
        }
        if let Some(integer) = value.unpack_i32() {
            let result = write!(self, "{integer}");
            return self.check_format(result);
        }
        if let Some(float) = value.downcast_ref::<StarlarkFloat>() {
            let result = write_compact(self, float.0, 'e');
            return self.check_format(result);
        }
        if value.downcast_ref::<StarlarkBigInt>().is_some() {
            // Decimal conversion can allocate scratch before writing to the sink.
            return Err(self.budget.refuse(ExecutionLimit::UnsupportedText("int")));
        }

        if let Some(list) = ListRef::from_value(value) {
            if self.is_cycle(value, depth, ancestors)? {
                return self.push_str("[...]");
            }
            self.push_str("[")?;
            self.write_items(list.iter(), mode.list_or_dict_element(), depth, ancestors)?;
            return self.push_str("]");
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            if self.is_cycle(value, depth, ancestors)? {
                return self.push_str("(...)");
            }
            self.push_str("(")?;
            self.write_items(tuple.iter(), ValueTextMode::Display, depth, ancestors)?;
            if tuple.content().len() == 1 {
                self.push_str(",")?;
            }
            return self.push_str(")");
        }
        if let Some(dict) = DictRef::from_value(value) {
            if self.is_cycle(value, depth, ancestors)? {
                return self.push_str("{...}");
            }
            self.push_str("{")?;
            let child_mode = mode.list_or_dict_element();
            for (index, (key, value)) in dict.iter().enumerate() {
                self.write_separator(index)?;
                self.write_value_at(key, child_mode, depth + 1, ancestors)?;
                self.push_str(": ")?;
                self.write_value_at(value, child_mode, depth + 1, ancestors)?;
            }
            return self.push_str("}");
        }
        if let Some(structure) = StructRef::from_value(value) {
            if self.is_cycle(value, depth, ancestors)? {
                return self.push_str("struct(...)");
            }
            self.push_str("struct(")?;
            for (index, (name, value)) in structure.iter().enumerate() {
                self.write_separator(index)?;
                self.push_str(name.as_str())?;
                self.push_str("=")?;
                self.write_value_at(value, ValueTextMode::Display, depth + 1, ancestors)?;
            }
            return self.push_str(")");
        }

        match value.get_ref().write_bounded_text(mode, self) {
            Some(result) => self.check_format(result),
            None => Err(self
                .budget
                .refuse(ExecutionLimit::UnsupportedText(value.get_type()))),
        }
    }

    fn write_items<'v>(
        &mut self,
        items: impl Iterator<Item = Value<'v>>,
        mode: ValueTextMode,
        depth: usize,
        ancestors: &mut [Value<'v>; MAX_TEXT_DEPTH],
    ) -> Result<(), ExecutionLimit> {
        for (index, item) in items.enumerate() {
            self.write_separator(index)?;
            self.write_value_at(item, mode, depth + 1, ancestors)?;
        }
        Ok(())
    }

    fn write_separator(&mut self, index: usize) -> Result<(), ExecutionLimit> {
        if index == 0 {
            Ok(())
        } else {
            self.push_str(", ")
        }
    }

    fn is_cycle<'v>(
        &self,
        value: Value<'v>,
        depth: usize,
        ancestors: &mut [Value<'v>; MAX_TEXT_DEPTH],
    ) -> Result<bool, ExecutionLimit> {
        self.budget.charge(depth, 0)?;
        if ancestors[..depth]
            .iter()
            .any(|ancestor| value.ptr_eq(*ancestor))
        {
            return Ok(true);
        }
        ancestors[depth] = value;
        Ok(false)
    }

    fn check_format(&self, result: fmt::Result) -> Result<(), ExecutionLimit> {
        result.map_err(|_| self.budget.refuse(ExecutionLimit::TextFormatting))?;
        self.budget.check()
    }
}

impl fmt::Write for BoundedText<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push_str(text).map_err(|_| fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use allocative::Allocative;

    use super::*;
    use crate as starlark;
    use crate::any::ProvidesStaticType;
    use crate::environment::GlobalsBuilder;
    use crate::environment::LibraryExtension;
    use crate::environment::Module;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::NoSerialize;
    use crate::values::StarlarkValue;
    use crate::values::starlark_value;

    fn evaluate<'v>(module: &'v Module, source: &str) -> Value<'v> {
        let globals = GlobalsBuilder::extended_by(&[LibraryExtension::StructType]).build();
        let ast =
            AstModule::parse("bounded_text.star", source.to_owned(), &Dialect::Standard).unwrap();
        Evaluator::new(module).eval_module(ast, &globals).unwrap()
    }

    fn assert_modes(value: Value<'_>) {
        for (mode, expected) in [
            (ValueTextMode::Str, value.to_str()),
            (ValueTextMode::Repr, value.to_repr()),
            (ValueTextMode::Display, value.to_string()),
        ] {
            let budget = ExecutionBudget::new(10_000, 10_000, 12);
            let mut text = BoundedText::new(&budget, expected.len());
            text.write_value(value, mode).unwrap();
            assert_eq!(text.finish().unwrap(), expected);
        }
    }

    #[test]
    fn exact_text_and_allocation_limits_are_sticky() {
        let budget = ExecutionBudget::new(100, 6, 4);
        let mut text = BoundedText::new(&budget, 4);
        text.push_str("é").unwrap();
        text.push_str("ab").unwrap();
        assert_eq!(text.finish().unwrap(), "éab");

        let budget = ExecutionBudget::new(100, 5, 4);
        let mut text = BoundedText::new(&budget, 4);
        text.push_str("é").unwrap();
        assert_eq!(text.push_str("ab"), Err(ExecutionLimit::AllocationBytes));
        assert_eq!(text.as_str(), "é");
        assert_eq!(text.finish(), Err(ExecutionLimit::AllocationBytes));

        let budget = ExecutionBudget::new(100, 100, 4);
        let mut text = BoundedText::new(&budget, 3);
        text.push_str("é").unwrap();
        assert_eq!(text.push_str("ab"), Err(ExecutionLimit::TextBytes));
        assert_eq!(text.as_str(), "é");
        let mut another = BoundedText::new(&budget, 10);
        assert_eq!(another.push_str("new"), Err(ExecutionLimit::TextBytes));
    }

    #[test]
    fn built_in_grammar_handles_escaped_strings_and_active_path_cycles() {
        let module = Module::new();
        let value = evaluate(
            &module,
            r#"
def sample():
    repeated = [None, True, -7, 4.0, "\u200b\té"]
    array = []
    array.append(array)
    dictionary = {}
    dictionary["self"] = dictionary
    tuple_contents = []
    sequence = (tuple_contents,)
    tuple_contents.append(sequence)
    struct_contents = []
    record = struct(field = struct_contents)
    struct_contents.append(record)
    return {"first": repeated, "again": repeated, "cycles": [array, dictionary, sequence, record]}
sample()
"#,
        );
        assert_modes(value);
        let budget = ExecutionBudget::new(100, 100, 4);
        let mut text = BoundedText::new(&budget, 3);
        text.write_value(
            module.heap().alloc_str("\n\"").to_value(),
            ValueTextMode::Str,
        )
        .unwrap();
        assert_eq!(text.finish().unwrap(), "\n\"");
    }

    #[test]
    fn shared_children_are_expanded_and_depth_is_admitted() {
        let module = Module::new();
        let value = evaluate(
            &module,
            "def sample():\n    child = [\"x\"]\n    return [child, child]\nsample()",
        );
        let expected = r#"[["x"], ["x"]]"#;
        let budget = ExecutionBudget::new(100, 100, 3);
        let mut text = BoundedText::new(&budget, expected.len());
        text.write_value(value, ValueTextMode::Repr).unwrap();
        assert_eq!(text.finish().unwrap(), expected);

        let budget = ExecutionBudget::new(100, 100, 3);
        let mut text = BoundedText::new(&budget, expected.len() - 1);
        assert_eq!(
            text.write_value(value, ValueTextMode::Display),
            Err(ExecutionLimit::TextBytes)
        );

        let budget = ExecutionBudget::new(100, 100, 2);
        let mut text = BoundedText::new(&budget, expected.len());
        assert_eq!(
            text.write_value(value, ValueTextMode::Repr),
            Err(ExecutionLimit::TextDepth)
        );
    }

    #[test]
    fn string_scan_is_admitted_before_writing() {
        let module = Module::new();
        let string = "x".repeat(1024);
        let value = module.heap().alloc_str(&string).to_value();
        let budget = ExecutionBudget::new(4, 10_000, 4);
        let mut text = BoundedText::new(&budget, 2_000);
        assert_eq!(
            text.write_value(value, ValueTextMode::Repr),
            Err(ExecutionLimit::Work)
        );
        assert!(text.as_str().is_empty());
    }

    #[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
    struct Unadmitted;

    impl fmt::Display for Unadmitted {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("an unadmitted type was formatted");
        }
    }

    crate::starlark_simple_value!(Unadmitted);

    #[starlark_value(type = "bounded_text_unadmitted")]
    impl<'v> StarlarkValue<'v> for Unadmitted {}

    #[test]
    fn ordinary_display_and_unbounded_bigint_conversion_are_unavailable() {
        let module = Module::new();
        for (value, type_name) in [
            (module.heap().alloc(Unadmitted), "bounded_text_unadmitted"),
            (evaluate(&module, "4294967296"), "int"),
        ] {
            let budget = ExecutionBudget::new(100, 100, 4);
            let mut text = BoundedText::new(&budget, 30);
            assert_eq!(
                text.write_value(value, ValueTextMode::Repr),
                Err(ExecutionLimit::UnsupportedText(type_name))
            );
            assert_eq!(
                text.finish(),
                Err(ExecutionLimit::UnsupportedText(type_name))
            );
        }
    }
}
