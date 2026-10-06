use std::fmt;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert::Assert;
use crate::environment::FrozenModule;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::starlark_simple_value;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::Value;

fn functions() -> FrozenModule {
    Assert::new().pass_module(
        r#"
def joined(sep, xs): return sep.join(xs)
def elements(sep, s): return sep.join(s.elems())
def extra(sep, xs): return sep.join(xs, xs)
def named(sep, xs): return sep.join(to_join = xs)
"#,
    )
}

#[test]
fn join_cardinality_bytes_parameters_and_raw_errors_are_preserved() {
    let functions = functions();
    let joined = functions.get("joined").unwrap();
    let elements = functions.get("elements").unwrap();
    let error_functions = ["extra", "named"].map(|name| functions.get(name).unwrap());
    let module = Module::new();
    let heap = module.heap();
    let mut eval = Evaluator::new(&module);
    let alphabet = ["", "x", "é\0", "世界"];
    let mut comparisons = 0;
    for len in 0..=4u32 {
        for mut index in 0..alphabet.len().pow(len) {
            let parts = (0..len)
                .map(|_| {
                    let s = alphabet[index % alphabet.len()];
                    index /= alphabet.len();
                    s
                })
                .collect::<Vec<_>>();
            let values = parts
                .iter()
                .map(|s| heap.alloc_str(s).to_value())
                .collect::<Vec<_>>();
            for sep in ["", "|", "é\0"] {
                let expected = parts.join(sep);
                for tuple in [false, true] {
                    let input = if tuple {
                        heap.alloc_tuple(&values)
                    } else {
                        heap.alloc(values.clone())
                    };
                    let result = eval
                        .eval_function(
                            joined.value(),
                            &[heap.alloc_str(sep).to_value(), input],
                            &[],
                        )
                        .unwrap();
                    assert_eq!(result.unpack_str(), Some(expected.as_str()));
                    if len == 1 {
                        assert!(result.ptr_eq(values[0]));
                    }
                    if expected.len() <= 1 {
                        assert!(result.ptr_eq(heap.alloc_str(&expected).to_value()));
                    }
                    comparisons += 1;
                }
            }
        }
    }
    for raw in ["", "x", "ab", "aé", "世界", "é\0x", "abcd"] {
        let expected = raw
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join("|");
        let result = eval
            .eval_function(
                elements.value(),
                &[
                    heap.alloc_str("|").to_value(),
                    heap.alloc_str(raw).to_value(),
                ],
                &[],
            )
            .unwrap();
        assert_eq!(result.unpack_str(), Some(expected.as_str()));
        comparisons += 1;
    }
    let mut errors = Vec::new();
    for len in 1..=4 {
        for invalid in 0..len {
            let values = (0..len)
                .map(|i| {
                    if i == invalid {
                        heap.alloc(17)
                    } else {
                        heap.alloc_str("valid").to_value()
                    }
                })
                .collect::<Vec<_>>();
            errors.push(
                eval.eval_function(
                    joined.value(),
                    &[heap.alloc_str("|").to_value(), heap.alloc(values)],
                    &[],
                )
                .unwrap_err()
                .to_string(),
            );
        }
    }
    errors.push(
        eval.eval_function(
            joined.value(),
            &[heap.alloc_str("|").to_value(), heap.alloc(7)],
            &[],
        )
        .unwrap_err()
        .to_string(),
    );
    for function in &error_functions {
        errors.push(
            eval.eval_function(
                function.value(),
                &[heap.alloc_str("|").to_value(), heap.alloc_tuple(&[])],
                &[],
            )
            .unwrap_err()
            .to_string(),
        );
    }
    assert_eq!(comparisons, 2053);
    assert_eq!(errors.len(), 13);
    println!("JOIN_COMPARISONS={comparisons}");
    println!("JOIN_ERRORS={}", serde_json::to_string(&errors).unwrap());
}

type Trace = Arc<Mutex<Vec<String>>>;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct TraceIterator {
    // None denotes an intentionally invalid string element.
    items: Vec<Option<String>>,
    virtual_len: Option<usize>,
    unknown_hint: bool,
    #[allocative(skip)]
    trace: Trace,
}

impl fmt::Display for TraceIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("join_trace_iterator")
    }
}

starlark_simple_value!(TraceIterator);

#[starlark_value(type = "join_trace_iterator")]
impl<'v> StarlarkValue<'v> for TraceIterator {
    unsafe fn iterate(&self, me: Value<'v>, _heap: Heap<'v>) -> crate::Result<Value<'v>> {
        self.trace.lock().unwrap().push("begin".into());
        Ok(me)
    }
    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        self.trace.lock().unwrap().push(format!("hint:{index}"));
        if self.unknown_hint {
            (0, None)
        } else {
            let remaining = self.virtual_len.unwrap_or(self.items.len()) - index;
            (remaining, Some(remaining))
        }
    }
    unsafe fn iter_next(&self, index: usize, heap: Heap<'v>) -> Option<Value<'v>> {
        self.trace.lock().unwrap().push(format!("next:{index}"));
        if let Some(len) = self.virtual_len {
            return (index < len).then(|| heap.alloc_str("x").to_value());
        }
        self.items.get(index).map(|s| match s {
            Some(s) => heap.alloc_str(s).to_value(),
            None => heap.alloc(17),
        })
    }
    unsafe fn iter_stop(&self) {
        self.trace.lock().unwrap().push("stop".into());
    }
}

#[test]
fn join_iterator_validation_hint_cleanup_and_capacity_panic_order_are_preserved() {
    let functions = functions();
    let joined = functions.get("joined").unwrap();
    let mut observed = Vec::new();
    let mut errors = Vec::new();
    for unknown_hint in [false, true] {
        for len in 0..=4 {
            for invalid in std::iter::once(None).chain((0..len).map(Some)) {
                let module = Module::new();
                let heap = module.heap();
                let trace = Trace::default();
                let parts = (0..len).map(|i| format!("é-item-{i}")).collect::<Vec<_>>();
                let input = heap.alloc_simple(TraceIterator {
                    items: parts
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (Some(i) != invalid).then(|| s.clone()))
                        .collect(),
                    virtual_len: None,
                    unknown_hint,
                    trace: trace.clone(),
                });
                let mut eval = Evaluator::new(&module);
                let result = eval.eval_function(
                    joined.value(),
                    &[heap.alloc_str("|").to_value(), input],
                    &[],
                );
                if invalid.is_some() {
                    errors.push(result.unwrap_err().to_string());
                } else {
                    assert_eq!(result.unwrap().unpack_str(), Some(parts.join("|").as_str()));
                }
                let mut expected = vec!["begin".to_owned(), "next:0".into()];
                if len > 0 {
                    expected.push("next:1".into());
                }
                if len >= 2 && invalid.is_none_or(|i| i >= 2) {
                    expected.push("hint:2".into());
                    let last_next = invalid.unwrap_or(len);
                    expected.extend((2..=last_next).map(|i| format!("next:{i}")));
                }
                expected.push("stop".into());
                assert_eq!(
                    *trace.lock().unwrap(),
                    expected,
                    "len={len} invalid={invalid:?}"
                );
                observed.push(expected);
            }
        }
    }
    let module = Module::new();
    let trace = Trace::default();
    // A lazy sequence with a truthful enormous size hint, without backing storage.
    let input = module.heap().alloc_simple(TraceIterator {
        items: Vec::new(),
        virtual_len: Some(isize::MAX as usize + 2),
        unknown_hint: false,
        trace: trace.clone(),
    });
    let mut eval = Evaluator::new(&module);
    let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
        eval.eval_function(joined.value(), &[Value::new_empty_string(), input], &[])
    }))
    .unwrap_err();
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap();
    assert_eq!(message, "capacity overflow");
    let overflow_trace = trace.lock().unwrap().clone();
    assert_eq!(
        overflow_trace,
        ["begin", "next:0", "next:1", "hint:2", "stop"]
    );
    observed.push(overflow_trace);
    assert_eq!(observed.len(), 31);
    println!("JOIN_TRACES={}", serde_json::to_string(&observed).unwrap());
    println!(
        "JOIN_ITERATOR_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}

#[starlark_module]
fn test_functions(builder: &mut GlobalsBuilder) {
    fn fresh_string<'v>(s: &str, heap: Heap<'v>) -> anyhow::Result<StringValue<'v>> {
        Ok(heap.alloc_str(s))
    }
}

#[test]
fn joined_results_and_functions_survive_gc_freeze_and_source_drop() {
    let source = String::from(
        r#"
def joined(sep, xs): return sep.join(xs)
def constants(): return ["|".join([]), "|".join(["x"]), "".join(["", ""]), "".join(["", "x"]), "|".join(["a", "b"]), "|".join(["a", "b", "c"])]
s = [fresh_string("é" * 8192), fresh_string("世界" * 4096)]
separator = fresh_string("sep" * 512)
RESULT = joined(separator, s)
s.append("after")
s = None
separator = None
garbage_collect()
assert_eq(RESULT, "é" * 8192 + "sep" * 512 + "世界" * 4096)
assert_eq(constants(), ["", "x", "", "x", "a|b", "a|b|c"])
"#,
    );
    let mut assertion = Assert::new();
    assertion.globals_add(test_functions);
    let frozen = assertion.pass_module(&source);
    let output = frozen.get("RESULT").unwrap();
    let joined = frozen.get("joined").unwrap();
    let constants = frozen.get("constants").unwrap();
    let clone = frozen.clone();
    drop(source);
    drop(frozen);
    drop(clone);
    let expected = format!(
        "{}{}{}",
        "é".repeat(8192),
        "sep".repeat(512),
        "世界".repeat(4096)
    );
    assert_eq!(output.value().unpack_str(), Some(expected.as_str()));
    let module = Module::new();
    let heap = module.heap();
    let mut eval = Evaluator::new(&module);
    assert_eq!(
        eval.eval_function(constants.value(), &[], &[])
            .unwrap()
            .to_repr(),
        "[\"\", \"x\", \"\", \"x\", \"a|b\", \"a|b|c\"]"
    );
    let input = heap.alloc(("é", "世界"));
    assert_eq!(
        eval.eval_function(
            joined.value(),
            &[heap.alloc_str("|").to_value(), input],
            &[]
        )
        .unwrap()
        .unpack_str(),
        Some("é|世界")
    );
    println!("JOIN_RETAINED_BYTES={}", expected.len());
}
