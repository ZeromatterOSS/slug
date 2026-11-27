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

use std::borrow::Cow;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use starlark_syntax::dot_format_parser::FormatConv;
use starlark_syntax::dot_format_parser::FormatParser;
use starlark_syntax::dot_format_parser::FormatToken;

use crate::collections::string_pool::StringPool;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;
use crate::values::ValueTextMode;
use crate::values::dict::Dict;

fn append_format_text<'a>(segment: &mut Cow<'a, str>, text: &'a str) {
    if matches!(segment, Cow::Borrowed("")) {
        *segment = Cow::Borrowed(text);
    } else {
        segment.to_mut().push_str(text);
    }
}

/// Try parse `"aaa{}bbb"` and return `("aaa", "bbb")`, borrowing unescaped text.
pub(crate) fn parse_format_one(s: &str) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let mut parser = FormatParser::new(s);
    let mut before = Cow::Borrowed("");
    loop {
        match parser.next().ok()?? {
            FormatToken::Text(text) => append_format_text(&mut before, text),
            FormatToken::Escape(e) => before.to_mut().push_str(e.as_str()),
            FormatToken::Capture {
                capture: "",
                conv: FormatConv::Str,
                pos: _,
            } => break,
            FormatToken::Capture { .. } => return None,
        }
    }

    let mut after = Cow::Borrowed("");
    loop {
        match parser.next().ok()? {
            Some(FormatToken::Text(text)) => append_format_text(&mut after, text),
            Some(FormatToken::Escape(e)) => after.to_mut().push_str(e.as_str()),
            Some(FormatToken::Capture { .. }) => return None,
            None => break,
        }
    }

    Some((before, after))
}

/// Evaluate `"<before>{}<after>".format(arg)`.
pub(crate) fn format_one<'v>(
    before: &str,
    arg: Value<'v>,
    after: &str,
    heap: Heap<'v>,
) -> StringValue<'v> {
    match StringValue::new(arg) {
        Some(arg) if before.is_empty() && after.is_empty() => arg,
        Some(arg) => heap.alloc_str_concat3(before, &arg, after),
        None => {
            let mut result = String::with_capacity(before.len() + 10);
            result.push_str(before);
            arg.collect_str(&mut result);
            heap.alloc_str_concat(&result, after)
        }
    }
}

/// Evaluate the single-argument intrinsic with the runtime heap's budget.
pub(crate) fn format_one_bounded<'v>(
    before: &str,
    arg: Value<'v>,
    after: &str,
    heap: Heap<'v>,
) -> crate::Result<StringValue<'v>> {
    let Some(budget) = heap.execution_budget() else {
        return Ok(format_one(before, arg, after, heap));
    };
    budget.charge(1, 0)?;
    if let Some(arg) = StringValue::new(arg) {
        if before.is_empty() && after.is_empty() {
            return Ok(arg);
        }
    }
    let mut result = BoundedText::new(budget, usize::MAX);
    result.push_str(before)?;
    result.write_value(arg, ValueTextMode::Str)?;
    result.push_str(after)?;
    Ok(heap.alloc_budgeted_str(result.as_str())?)
}

/// The format string can either have explicit indices,
/// or grab things sequentially, but not both.
/// FormatArgs knows which we are doing and keeps them in mind.
struct FormatArgs<'v, T: Iterator<Item = Value<'v>>> {
    // Initially we have the iterator set and the args empty.
    // If we ever ask by index, we decant the iterator into args.
    iterator: T,
    args: Vec<Value<'v>>,
    by_index: bool,
    by_order: bool,
}

impl<'v, T: Iterator<Item = Value<'v>>> FormatArgs<'v, T> {
    fn new(iterator: T) -> Self {
        Self {
            iterator,
            args: Vec::new(),
            by_index: false,
            by_order: false,
        }
    }

    fn next_ordered(&mut self) -> anyhow::Result<Value<'v>> {
        if self.by_index {
            Err(anyhow::anyhow!(
                "Cannot mix manual field specification and automatic field numbering in format string",
            ))
        } else {
            self.by_order = true;
            match self.iterator.next() {
                None => Err(anyhow::anyhow!("Not enough parameters in format string")),
                Some(x) => Ok(x),
            }
        }
    }

    fn by_index(&mut self, index: usize) -> anyhow::Result<Value<'v>> {
        self.indexed(index, |args, iterator| {
            args.extend(iterator);
            Ok(())
        })
    }

    fn indexed(
        &mut self,
        index: usize,
        fill: impl FnOnce(&mut Vec<Value<'v>>, &mut T) -> anyhow::Result<()>,
    ) -> anyhow::Result<Value<'v>> {
        if self.by_order {
            Err(anyhow::anyhow!(
                "Cannot mix manual field specification and automatic field numbering in format string",
            ))
        } else {
            if !self.by_index {
                fill(&mut self.args, &mut self.iterator)?;
                self.by_index = true;
            }
            match self.args.get(index) {
                None => Err(ValueError::IndexOutOfBound(index as i32).into()),
                Some(v) => Ok(*v),
            }
        }
    }
}

fn collect_indexed_args<'v>(
    args: &mut Vec<Value<'v>>,
    iterator: &mut impl Iterator<Item = Value<'v>>,
    budget: &ExecutionBudget,
) -> Result<(), ExecutionLimit> {
    loop {
        budget.charge(1, 0)?;
        let Some(value) = iterator.next() else {
            return Ok(());
        };
        budget.push_vec(args, value)?;
    }
}

pub(crate) fn format<'v>(
    this: &str,
    args: impl Iterator<Item = Value<'v>>,
    kwargs: Dict<'v>,
    string_pool: &mut StringPool,
    heap: Heap<'v>,
) -> anyhow::Result<StringValue<'v>> {
    let mut parser = FormatParser::new(this);
    let mut result = string_pool.alloc();
    let mut args = FormatArgs::new(args);
    while let Some(token) = parser.next()? {
        match token {
            FormatToken::Text(text) => result.push_str(text),
            FormatToken::Escape(e) => result.push_str(e.as_str()),
            FormatToken::Capture {
                capture,
                conv,
                pos: _,
            } => format_capture(capture, conv, &mut args, &kwargs, &mut result)?,
        }
    }
    let r = heap.alloc_str(&result);
    string_pool.release(result);
    Ok(r)
}

/// Format using admitted positional iteration and named lookup from the caller.
pub(crate) fn format_bounded<'v>(
    this: &str,
    positions: impl Iterator<Item = Value<'v>>,
    mut named_lookup: impl FnMut(&str) -> crate::Result<Option<Value<'v>>>,
    heap: Heap<'v>,
) -> anyhow::Result<StringValue<'v>> {
    let budget = heap
        .execution_budget()
        .expect("bounded format needs a budget");
    budget.charge_text_work(this.len())?;
    let mut parser = FormatParser::new(this);
    let mut result = BoundedText::new(budget, usize::MAX);
    let mut args = FormatArgs::new(positions);
    while let Some(token) = parser
        .next_borrowed()
        .map_err(|error| bounded_format_error(budget, format_args!("{error}")))?
    {
        budget.charge(1, 0)?;
        match token {
            FormatToken::Text(text) => result.push_str(text)?,
            FormatToken::Escape(escape) => result.push_str(escape.as_str())?,
            FormatToken::Capture { capture, conv, .. } => {
                // Classification and parsing can each scan the captured field.
                budget.charge(2 * ExecutionBudget::text_work(capture.len()), 0)?;
                let field = parse_field(capture)
                    .map_err(|error| bounded_format_error(budget, format_args!("{error}")))?;
                let value = match field {
                    FormatField::Ordered => {
                        budget.charge(1, 0)?;
                        args.next_ordered()?
                    }
                    FormatField::Index(index) => {
                        budget.charge(1, 0)?;
                        args.indexed(index, |values, positions| {
                            collect_indexed_args(values, positions, budget)?;
                            Ok(())
                        })?
                    }
                    FormatField::Named => {
                        let value = named_lookup(capture);
                        budget.check()?;
                        value.map_err(crate::Error::into_anyhow)?.ok_or_else(|| {
                            bounded_format_error(
                                budget,
                                format_args!("Key `{capture}` was not found"),
                            )
                        })?
                    }
                };
                let mode = match conv {
                    FormatConv::Str => ValueTextMode::Str,
                    FormatConv::Repr => ValueTextMode::Repr,
                };
                result.write_value(value, mode)?;
            }
        }
    }
    Ok(heap.alloc_budgeted_str(result.as_str())?)
}

fn bounded_format_error(budget: &ExecutionBudget, args: fmt::Arguments<'_>) -> anyhow::Error {
    let mut diagnostic = BoundedText::new(budget, usize::MAX);
    if fmt::write(&mut diagnostic, args).is_err() {
        return budget.refuse(ExecutionLimit::TextFormatting).into();
    }
    match diagnostic.finish() {
        Ok(message) => anyhow::Error::msg(message),
        Err(limit) => limit.into(),
    }
}

enum FormatField {
    Ordered,
    Index(usize),
    Named,
}

#[derive(Debug, thiserror::Error)]
enum FormatFieldError<'a> {
    #[error("Error parsing `{0}` as a format string index: {1}")]
    Index(&'a str, ParseIntError),
    #[error("Invalid character '{0}' inside replacement field")]
    InvalidChar(char),
}

fn parse_field(field: &str) -> Result<FormatField, FormatFieldError<'_>> {
    if field.is_empty() {
        Ok(FormatField::Ordered)
    } else if field.bytes().all(|c| c.is_ascii_digit()) {
        let index =
            usize::from_str(field).map_err(|error| FormatFieldError::Index(field, error))?;
        Ok(FormatField::Index(index))
    } else if let Some(c) = field
        .bytes()
        .find(|c| matches!(c, b'.' | b',' | b'[' | b']'))
    {
        Err(FormatFieldError::InvalidChar(char::from(c)))
    } else {
        Ok(FormatField::Named)
    }
}

fn format_capture<'v, T: Iterator<Item = Value<'v>>>(
    field: &str,
    conv: FormatConv,
    args: &mut FormatArgs<'v, T>,
    kwargs: &Dict<'v>,
    result: &mut String,
) -> anyhow::Result<()> {
    let conv_s = |x: Value, result: &mut String| x.collect_str(result);
    let conv_r = |x: Value, result: &mut String| x.collect_repr(result);
    let conv: &dyn Fn(Value, &mut String) = match conv {
        FormatConv::Str => &conv_s,
        FormatConv::Repr => &conv_r,
    };
    let value = match parse_field(field).map_err(|error| anyhow::anyhow!("{error}"))? {
        FormatField::Ordered => args.next_ordered()?,
        FormatField::Index(index) => args.by_index(index)?,
        FormatField::Named => kwargs
            .get_str(field)
            .ok_or_else(|| ValueError::KeyNotFound(field.to_owned()))?,
    };
    conv(value, result);
    Ok(())
}

#[cfg(test)]
#[path = "format_suffix_memory_tests.rs"]
mod format_suffix_memory_tests;

#[cfg(test)]
#[path = "dot_format_memory_tests.rs"]
mod dot_format_memory_tests;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use starlark_map::small_map::SmallMap;
    use starlark_syntax::dot_format_parser::FormatConv;

    use crate::assert;
    use crate::coerce::coerce;
    use crate::collections::string_pool::StringPool;
    use crate::environment::Module;
    use crate::eval::ExecutionBudget;
    use crate::eval::ExecutionLimit;
    use crate::values::Heap;
    use crate::values::Value;
    use crate::values::dict::Dict;
    use crate::values::string::dot_format::FormatArgs;
    use crate::values::string::dot_format::format;
    use crate::values::string::dot_format::format_bounded;
    use crate::values::string::dot_format::format_capture;
    use crate::values::string::dot_format::format_one;
    use crate::values::string::dot_format::format_one_bounded;
    use crate::values::string::dot_format::parse_format_one;

    fn format_capture_for_test<'v, T: Iterator<Item = Value<'v>>>(
        capture: &str,
        conv: FormatConv,
        args: &mut FormatArgs<'v, T>,
        kwargs: &Dict<'v>,
    ) -> anyhow::Result<String> {
        let mut result = String::new();
        format_capture(capture, conv, args, kwargs, &mut result)?;
        Ok(result)
    }

    #[test]
    fn test_format_capture() {
        Heap::temp(|heap| {
            let original_args = [heap.alloc("1"), heap.alloc("2"), heap.alloc("3")];
            let mut args = FormatArgs::new(original_args.iter().copied());
            let mut kwargs = SmallMap::new();

            kwargs.insert_hashed(heap.alloc_str("a").get_hashed(), heap.alloc("x"));
            kwargs.insert_hashed(heap.alloc_str("b").get_hashed(), heap.alloc("y"));
            kwargs.insert_hashed(heap.alloc_str("c").get_hashed(), heap.alloc("z"));
            let kwargs = Dict::new(coerce(kwargs));
            assert_eq!(
                format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).unwrap(),
                "1"
            );
            assert_eq!(
                format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).unwrap(),
                "2"
            );
            assert_eq!(
                format_capture_for_test("", FormatConv::Repr, &mut args, &kwargs).unwrap(),
                "\"3\""
            );
            assert_eq!(
                format_capture_for_test("a", FormatConv::Repr, &mut args, &kwargs).unwrap(),
                "\"x\""
            );
            assert_eq!(
                format_capture_for_test("a", FormatConv::Str, &mut args, &kwargs).unwrap(),
                "x"
            );
            assert!(format_capture_for_test("1", FormatConv::Str, &mut args, &kwargs).is_err());
            let mut args = FormatArgs::new(original_args.iter().copied());
            assert_eq!(
                format_capture_for_test("1", FormatConv::Str, &mut args, &kwargs).unwrap(),
                "2"
            );
            assert!(format_capture_for_test("", FormatConv::Str, &mut args, &kwargs).is_err());
        });
    }

    #[test]
    fn bounded_format_matches_ordinary_tokens_and_error_order() {
        let budget = Arc::new(ExecutionBudget::new(4_096, 16_384, 16));
        let module = Module::new_with_execution_budget(budget);
        let heap = module.heap();
        let args = [heap.alloc("zero"), heap.alloc("é\n")];
        let named = heap.alloc("named");
        for this in [
            "{{{1!r}:{0!s}:{name}}}",
            "({name}:{}:{!r})",
            "{missing} }",
            "{} {0}",
            "{0} {}",
            "{bad.field} {0!x}",
            "{0!x}",
            "{0!}",
            "{0",
            "}",
            "{99999999999999999999999999999}",
        ] {
            let mut kwargs = SmallMap::new();
            kwargs.insert_hashed(heap.alloc_str("name").get_hashed(), named);
            let kwargs = Dict::new(coerce(kwargs));
            let bounded = format_bounded(
                this,
                args.into_iter(),
                |name| Ok(kwargs.get_str(name)),
                heap,
            );
            let ordinary = format(
                this,
                args.into_iter(),
                kwargs,
                &mut StringPool::default(),
                heap,
            );
            match (bounded, ordinary) {
                (Ok(bounded), Ok(ordinary)) => assert_eq!(bounded.as_str(), ordinary.as_str()),
                (Err(bounded), Err(ordinary)) => {
                    assert_eq!(bounded.to_string(), ordinary.to_string())
                }
                pair => panic!("format paths differ for {this:?}: {pair:?}"),
            }
        }
    }

    #[test]
    fn bounded_format_admits_scan_vector_diagnostics_and_final_copy() {
        let new_module = |work, allocation| {
            Module::new_with_execution_budget(Arc::new(ExecutionBudget::new(work, allocation, 16)))
        };
        let module = new_module(1, 1024);
        let text = "x".repeat(257);
        let error =
            format_bounded(&text, std::iter::empty(), |_| Ok(None), module.heap()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ExecutionLimit>(),
            Some(&ExecutionLimit::Work)
        );

        let module = new_module(1024, 0);
        let visited = Cell::new(0);
        let positions = std::iter::from_fn(|| {
            visited.set(visited.get() + 1);
            Some(Value::new_none())
        });
        let error = format_bounded("{0}", positions, |_| Ok(None), module.heap()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ExecutionLimit>(),
            Some(&ExecutionLimit::AllocationBytes)
        );
        assert_eq!(visited.get(), 1);
        let error =
            format_bounded("", std::iter::empty(), |_| Ok(None), module.heap()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ExecutionLimit>(),
            Some(&ExecutionLimit::AllocationBytes)
        );

        let module = new_module(1024, 64);
        let text = format!("{{{}", "x".repeat(257));
        let error =
            format_bounded(&text, std::iter::empty(), |_| Ok(None), module.heap()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ExecutionLimit>(),
            Some(&ExecutionLimit::AllocationBytes)
        );

        let literal = "native heap copy";
        let module = new_module(1024, literal.len());
        let before = module.heap().allocated_bytes();
        let error =
            format_bounded(literal, std::iter::empty(), |_| Ok(None), module.heap()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ExecutionLimit>(),
            Some(&ExecutionLimit::AllocationBytes)
        );
        assert_eq!(module.heap().allocated_bytes(), before);
    }

    #[test]
    fn bounded_format_one_preserves_identity_and_admits_nonstring_rendering() {
        let budget = Arc::new(ExecutionBudget::new(1_024, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let string = heap.alloc("existing");
        assert!(
            format_one_bounded("", string, "", heap)
                .unwrap()
                .to_value()
                .ptr_eq(string)
        );
        assert!(format_one_bounded("prefix", string, "", heap).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        assert!(format_one_bounded("", string, "", heap).is_err());

        let budget = Arc::new(ExecutionBudget::new(1_024, 4_096, 16));
        let module = Module::new_with_execution_budget(budget);
        let heap = module.heap();
        let value = heap.alloc(vec!["é\n"]);
        assert_eq!(
            format_one_bounded("pré ", value, " fin", heap)
                .unwrap()
                .as_str(),
            format_one("pré ", value, " fin", heap).as_str()
        );
    }

    #[test]
    fn test_format() {
        assert::eq("'a{x}b{y}c{}'.format(1, x=2, y=3)", "'a2b3c1'");
        assert::eq("'a{x}b{{y}}c{}'.format(1, x=2)", "'a2b{y}c1'");
    }

    #[test]
    fn test_parse_format_one() {
        assert_eq!(
            Some(("abc".into(), "def".into())),
            parse_format_one("abc{}def")
        );
        assert_eq!(
            Some(("abc".into(), "def".into())),
            parse_format_one("abc{!s}def")
        );
        assert_eq!(None, parse_format_one("abc{!r}def"));
        assert_eq!(
            Some(("a{b".into(), "c}d{".into())),
            parse_format_one("a{{b{}c}}d{{")
        );
        assert_eq!(None, parse_format_one("a{"));
        assert_eq!(None, parse_format_one("a{}{}"));
        assert_eq!(None, parse_format_one("{x}"));
    }
}
