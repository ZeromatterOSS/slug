/*
 * Copyright 2019 The Starlark in Rust Authors.
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

use std::fmt;
use std::sync::Arc;

use dupe::Dupe;
use itertools::Itertools;
use once_cell::sync::Lazy;
use starlark_derive::starlark_module;
use starlark_syntax::codemap::Span;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueOfUnchecked;
use crate::values::function::StarlarkFunction;
use crate::values::none::NoneOr;
use crate::values::none::NoneType;
use crate::values::tuple::UnpackTuple;
use crate::values::typing::iter::StarlarkIter;

#[starlark_module]
pub fn filter(builder: &mut GlobalsBuilder) {
    /// Apply a predicate to each element of the iterable, returning those that match.
    /// As a special case if the function is `None` then removes all the `None` values.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// filter(bool, [0, 1, False, True]) == [1, True]
    /// filter(lambda x: x > 2, [1, 2, 3, 4]) == [3, 4]
    /// filter(None, [True, None, False]) == [True, False]
    /// # "#);
    /// ```
    fn filter<'v>(
        #[starlark(require = pos)] func: NoneOr<ValueOfUnchecked<'v, StarlarkFunction>>,
        #[starlark(require = pos)] seq: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Vec<Value<'v>>> {
        let mut res = Vec::new();

        for v in seq.get().iterate(eval.heap())? {
            match func {
                NoneOr::None => {
                    if !v.is_none() {
                        res.push(v);
                    }
                }
                NoneOr::Other(func) => {
                    if func.get().invoke_pos(&[v], eval)?.to_bool() {
                        res.push(v);
                    }
                }
            }
        }
        Ok(res)
    }
}

#[starlark_module]
pub fn map(builder: &mut GlobalsBuilder) {
    /// Apply a function to each element of the iterable, returning the results.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// map(abs, [7, -5, -6]) == [7, 5, 6]
    /// map(lambda x: x * 2, [1, 2, 3, 4]) == [2, 4, 6, 8]
    /// # "#);
    /// ```
    fn map<'v>(
        #[starlark(require = pos)] func: ValueOfUnchecked<'v, StarlarkFunction>,
        #[starlark(require = pos)] seq: ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Vec<Value<'v>>> {
        let it = seq.get().iterate(eval.heap())?;
        let mut res = Vec::with_capacity(it.size_hint().0);
        for v in it {
            res.push(func.get().invoke_pos(&[v], eval)?);
        }
        Ok(res)
    }
}

#[starlark_module]
pub fn debug(builder: &mut GlobalsBuilder) {
    /// Print the value with full debug formatting. The result may not be stable over time.
    /// Intended for debugging purposes and guaranteed to produce verbose output not suitable for user display.
    fn debug(#[starlark(require = pos)] val: Value) -> anyhow::Result<String> {
        Ok(format!("{val:?}"))
    }
}

struct PrintWrapper<'a, 'b>(&'a Vec<Value<'b>>);
impl fmt::Display for PrintWrapper<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            if i != 0 {
                f.write_str(" ")?;
            }
            fmt::Display::fmt(v, f)?;
        }
        Ok(())
    }
}

/// Bazel-shaped source location for one `print` or `pprint` call.
#[derive(Clone, Debug, Dupe, Eq, PartialEq)]
pub struct PrintLocation {
    file: Arc<str>,
    line: u32,
    column: u32,
}

impl PrintLocation {
    fn from_evaluator(eval: &Evaluator<'_, '_, '_>) -> Self {
        static BUILTIN: Lazy<Arc<str>> = Lazy::new(|| Arc::from("<builtin>"));

        let Some(location) = eval.call_stack_top_location() else {
            return Self {
                file: BUILTIN.dupe(),
                line: 0,
                column: 0,
            };
        };
        if location.file.is_native() {
            return Self {
                file: BUILTIN.dupe(),
                line: 0,
                column: 0,
            };
        };

        let point = location.span.begin();
        let line = location.file.find_line(point);
        let line_span = location.file.line_span(line);
        let prefix = location
            .file
            .source_span(Span::new(line_span.begin(), point));
        Self {
            file: location.file.filename_shared(),
            line: (line + 1) as u32,
            column: (prefix.encode_utf16().count() + 1) as u32,
        }
    }

    /// Consume this location into its apparent filename, line, and column.
    pub fn into_parts(self) -> (Arc<str>, u32, u32) {
        (self.file, self.line, self.column)
    }
}

impl fmt::Display for PrintLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.file)?;
        if self.line != 0 {
            write!(f, ":{}", self.line)?;
            if self.column != 0 {
                write!(f, ":{}", self.column)?;
            }
        }
        Ok(())
    }
}

/// Invoked from `print` or `pprint` to print a value.
pub trait PrintHandler {
    /// If this function returns error, evaluation fails with this error.
    fn println(&self, location: PrintLocation, text: &str) -> crate::Result<()>;
}

pub(crate) struct StderrPrintHandler;

impl PrintHandler for StderrPrintHandler {
    fn println(&self, location: PrintLocation, text: &str) -> crate::Result<()> {
        eprintln!("{location}: {text}");
        Ok(())
    }
}

#[starlark_module]
pub fn print(builder: &mut GlobalsBuilder) {
    /// Print some values to the output.
    fn print(
        #[starlark(args)] args: UnpackTuple<Value>,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        // In practice most users should want to put the print somewhere else, but this does for now
        // Unfortunately, we can't use PrintWrapper because strings to_str() and Display are different.
        let location = PrintLocation::from_evaluator(eval);
        let text = args.items.iter().map(|x| x.to_str()).join(" ");
        eval.print_handler.println(location, &text)?;
        Ok(NoneType)
    }
}

#[starlark_module]
pub fn pprint(builder: &mut GlobalsBuilder) {
    fn pprint(
        #[starlark(args)] args: UnpackTuple<Value>,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        // In practice most users may want to put the print somewhere else, but this does for now
        let location = PrintLocation::from_evaluator(eval);
        eval.print_handler
            .println(location, &format!("{:#}", PrintWrapper(&args.items)))?;
        Ok(NoneType)
    }
}

fn pretty_repr<'v>(
    a: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<StringValue<'v>> {
    use std::fmt::Write;

    let mut s = eval.string_pool.alloc();
    write!(s, "{a:#}").unwrap();
    let r = eval.heap().alloc_str(&s);
    eval.string_pool.release(s);
    Ok(r)
}

#[starlark_module]
pub fn pstr(builder: &mut GlobalsBuilder) {
    /// Like `str`, but produces more verbose pretty-printed output
    fn pstr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        if let Some(a) = StringValue::new(a) {
            return Ok(a);
        }

        pretty_repr(a, eval)
    }
}

#[starlark_module]
pub fn prepr(builder: &mut GlobalsBuilder) {
    /// Like `repr`, but produces more verbose pretty-printed output
    fn prepr<'v>(
        #[starlark(require = pos)] a: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StringValue<'v>> {
        pretty_repr(a, eval)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use dupe::Dupe;
    use starlark_derive::starlark_module;
    use starlark_syntax::error::StarlarkResultExt;

    use crate as starlark;
    use crate::assert;
    use crate::assert::Assert;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::Arguments;
    use crate::eval::Evaluator;
    use crate::stdlib::PrintHandler;
    use crate::stdlib::PrintLocation;
    use crate::values::Value;

    #[starlark_module]
    fn native_print_location_globals(globals: &mut GlobalsBuilder) {
        fn native_print_location(eval: &mut Evaluator) -> anyhow::Result<String> {
            Ok(PrintLocation::from_evaluator(eval).to_string())
        }

        fn invoke_with_native_location<'v>(
            function: Value<'v>,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> anyhow::Result<Value<'v>> {
            use crate::eval::runtime::rust_loc::rust_loc;

            function
                .invoke_with_loc(Some(rust_loc!()), &Arguments::default(), eval)
                .into_anyhow_result()
        }
    }

    #[test]
    fn test_filter() {
        let mut a = Assert::new();
        // TODO(nga): fix and enable.
        a.disable_static_typechecking();
        a.pass(
            r#"
def contains_hello(s):
    if "hello" in s:
        return True
    return False

def positive(i):
    return i > 0

assert_eq([], filter(positive, []))
assert_eq([1, 2, 3], filter(positive, [1, 2, 3]))
assert_eq([], filter(positive, [-1, -2, -3]))
assert_eq([1, 2, 3], filter(positive, [-1, 1, 2, -2, -3, 3]))
assert_eq(["hello world!"], filter(contains_hello, ["hello world!", "goodbye"]))
"#,
        );
    }

    #[test]
    fn test_map() {
        let mut a = Assert::new();
        // TODO: fix and enable.
        a.disable_static_typechecking();
        a.pass(
            r#"
def double(x):
    return x + x

assert_eq([], map(int, []))
assert_eq([1,2,3], map(int, ["1","2","3"]))
assert_eq(["0","1","2"], map(str, range(3)))
assert_eq(["11",8], map(double, ["1",4]))
"#,
        );
    }

    #[test]
    fn test_debug() {
        assert::pass(
            r#"assert_eq(
                debug([1,2]),
                "Value(ListGen(ListData { content: Cell { value: ValueTyped(Value(Array { len: 2, capacity: 2, iter_count: 0, content: [Value(1), Value(2)] })) } }))"
                )"#,
        );
    }

    #[test]
    fn test_print_preserves_bazel_call_locations() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_copy = seen.dupe();
        struct PrintHandlerImpl {
            seen: Rc<RefCell<Vec<(PrintLocation, String)>>>,
        }
        impl PrintHandler for PrintHandlerImpl {
            fn println(&self, location: PrintLocation, text: &str) -> crate::Result<()> {
                self.seen.borrow_mut().push((location, text.to_owned()));
                Ok(())
            }
        }
        let print_handler = PrintHandlerImpl { seen: seen.dupe() };
        let mut a = Assert::new();
        a.set_print_handler(&print_handler);
        a.pass(
            r#"print("top")
print ("spaced")
def nested():
        print("nested")
nested()
x = "😀"; print("utf16")
pprint("pretty")
print("first\nsecond")
"#,
        );

        let seen = seen_copy.borrow();
        let expected = [
            ("assert.bzl:1:6", "top"),
            ("assert.bzl:2:7", "spaced"),
            ("assert.bzl:4:14", "nested"),
            ("assert.bzl:6:16", "utf16"),
            ("assert.bzl:7:7", "\"pretty\""),
            ("assert.bzl:8:6", "first\nsecond"),
        ];
        assert!(!seen.is_empty());
        assert_eq!(seen.len() % expected.len(), 0);
        for run in seen.chunks_exact(expected.len()) {
            let actual = run
                .iter()
                .map(|(location, text)| (location.to_string(), text.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|(location, text)| ((*location).to_owned(), *text))
                    .collect::<Vec<_>>()
            );
            assert!(
                run.windows(2)
                    .all(|pair| Arc::ptr_eq(&pair[0].0.file, &pair[1].0.file))
            );
        }
    }

    #[test]
    fn print_location_without_a_call_frame_is_builtin() {
        let module = Module::new();
        let eval = Evaluator::new(&module);
        assert_eq!(
            PrintLocation::from_evaluator(&eval).to_string(),
            "<builtin>"
        );
    }

    #[test]
    fn print_location_with_a_native_call_frame_is_builtin() {
        let mut a = Assert::new();
        a.globals_add(native_print_location_globals);
        a.pass(
            r#"assert_eq(
                invoke_with_native_location(native_print_location),
                "<builtin>",
            )"#,
        );
    }

    #[test]
    fn test_pstr() {
        assert::pass(
            r#"
assert_eq(pstr([]), "[]")
assert_eq(pstr([1,2,[]]), """[
  1,
  2,
  []
]""")
assert_eq(pstr("abcd"), "abcd")
"#,
        );
    }

    #[test]
    fn test_prepr() {
        assert::pass(
            r#"
assert_eq(prepr([]), "[]")
assert_eq(prepr([1,2,[]]), """[
  1,
  2,
  []
]""")
assert_eq(prepr("abcd"), "\"abcd\"")
"#,
        );
    }
}
