use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::FrozenModule;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::list::ListRef;

fn functions() -> FrozenModule {
    Assert::new().pass_module(
        r#"
def lines(s: str, ends: bool) -> list[str]: return s.splitlines(ends)
def extra(s): return s.splitlines(False, False)
def named(s): return s.splitlines(keepends = True)
def wrong_bool(s): return s.splitlines(1)
"#,
    )
}

fn strings(value: crate::values::Value<'_>) -> Vec<&str> {
    ListRef::from_value(value)
        .unwrap()
        .iter()
        .map(|v| v.unpack_str().unwrap())
        .collect()
}

// Independent character cursor oracle: treat CRLF as one terminator and retain
// exact source characters. Non-CR/LF Unicode separators remain ordinary text.
fn expected_lines(raw: &str, keepends: bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut line = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' {
            if keepends {
                line.push(c);
            }
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
                if keepends {
                    line.push('\n');
                }
            }
            result.push(std::mem::take(&mut line));
        } else {
            line.push(c);
        }
    }
    if !line.is_empty() {
        result.push(line);
    }
    result
}

#[test]
fn unsplit_lines_share_whole_input_and_avoid_payload_growth() {
    let functions = functions();
    let function = functions.get("lines").unwrap();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    // Isolate public heap accounting; explicit GC is covered independently.
    eval.disable_gc();
    let mut sharing = Vec::new();
    let mut growths = Vec::new();
    for (kind, raw) in [
        ("ascii", "already_one_line_01234567".repeat(4096)),
        ("unicode", "é世界😀\u{85}\u{2028}\u{2029}\0".repeat(4096)),
    ] {
        for keepends in [false, true] {
            let input = module.heap().alloc_str(&raw).to_value();
            let before = module.heap().allocated_bytes();
            let result = eval
                .eval_function(
                    function.value(),
                    &[input, module.heap().alloc(keepends)],
                    &[],
                )
                .unwrap();
            assert_eq!(strings(result), [raw.as_str()]);
            let shared = ListRef::from_value(result)
                .unwrap()
                .iter()
                .next()
                .unwrap()
                .ptr_eq(input);
            let growth = module.heap().allocated_bytes() - before;
            println!(
                "SPLITLINES kind={kind} keepends={keepends} bytes={} shared={shared} heap_growth={growth}",
                raw.len()
            );
            sharing.push(shared);
            growths.push((growth, raw.len()));
        }
    }
    assert_eq!(sharing, [true; 4]);
    // The result list still allocates; this is not a zero-allocation assertion.
    assert!(growths.iter().all(|(growth, len)| growth < len));
}

#[test]
fn splitlines_bytes_lists_parameters_and_exact_errors_are_preserved() {
    let functions = functions();
    let function = functions.get("lines").unwrap();
    let errors = ["extra", "named", "wrong_bool", "lines"].map(|n| (n, functions.get(n).unwrap()));
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.disable_gc();
    for (raw, plain, kept) in [
        ("", vec![], vec![]),
        ("a", vec!["a"], vec!["a"]),
        ("\r", vec![""], vec!["\r"]),
        ("\n", vec![""], vec!["\n"]),
        ("\r\n", vec![""], vec!["\r\n"]),
        (
            "a\r\nb\rc\n",
            vec!["a", "b", "c"],
            vec!["a\r\n", "b\r", "c\n"],
        ),
        ("\n\ra", vec!["", "", "a"], vec!["\n", "\r", "a"]),
        ("a\r\r\nb", vec!["a", "", "b"], vec!["a\r", "\r\n", "b"]),
        (
            "é\r\n世界\n😀",
            vec!["é", "世界", "😀"],
            vec!["é\r\n", "世界\n", "😀"],
        ),
        (
            "a\u{85}\u{2028}\u{2029}\0b",
            vec!["a\u{85}\u{2028}\u{2029}\0b"],
            vec!["a\u{85}\u{2028}\u{2029}\0b"],
        ),
    ] {
        for (keepends, expected) in [(false, plain), (true, kept)] {
            let input = module.heap().alloc_str(raw).to_value();
            let result = eval
                .eval_function(
                    function.value(),
                    &[input, module.heap().alloc(keepends)],
                    &[],
                )
                .unwrap();
            assert_eq!(strings(result), expected);
        }
    }
    let alphabet = ['a', '\r', '\n', 'é', '\u{85}', '\u{2028}', '\0'];
    let mut comparisons = 0;
    for len in 0..=4u32 {
        for mut index in 0..alphabet.len().pow(len) {
            let mut raw = String::new();
            for _ in 0..len {
                raw.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            let input = module.heap().alloc_str(&raw).to_value();
            for keepends in [false, true] {
                let result = eval
                    .eval_function(
                        function.value(),
                        &[input, module.heap().alloc(keepends)],
                        &[],
                    )
                    .unwrap();
                assert_eq!(strings(result), expected_lines(&raw, keepends));
                comparisons += 1;
            }
        }
    }
    let mut records = Vec::new();
    for (name, function) in &errors {
        let args = if *name == "lines" {
            vec![module.heap().alloc(1), module.heap().alloc(false)]
        } else {
            vec![module.heap().alloc_str("input").to_value()]
        };
        records.push(
            eval.eval_function(function.value(), &args, &[])
                .unwrap_err()
                .to_string(),
        );
    }
    println!("SPLITLINES_COMPARISONS={comparisons}");
    println!(
        "SPLITLINES_ERRORS={}",
        serde_json::to_string(&records).unwrap()
    );
}

#[starlark_module]
fn test_functions(builder: &mut GlobalsBuilder) {
    fn fresh_string<'v>(text: &str, heap: Heap<'v>) -> anyhow::Result<StringValue<'v>> {
        Ok(heap.alloc_str(text))
    }
}

#[test]
fn splitlines_lists_and_functions_survive_gc_freeze_and_source_drop() {
    let source = String::from(
        r#"
def lines(s, ends): return s.splitlines(ends)
def constant_lines(): return ["é世界".splitlines(), "a\r\nb\n".splitlines(True)]
s = fresh_string("é世界" * 4096)
RESULT = [s.splitlines(), s.splitlines(True), fresh_string("a\r\nb\r\n").splitlines(True)]
s = None
garbage_collect()
assert_eq(RESULT, [["é世界" * 4096], ["é世界" * 4096], ["a\r\n", "b\r\n"]])
x = fresh_string("one line").splitlines()
x.append("another")
assert_eq(type(x), "list")
assert_eq(x, ["one line", "another"])
assert_eq(constant_lines(), [["é世界"], ["a\r\n", "b\n"]])
"#,
    );
    let mut assertion = Assert::new();
    assertion.globals_add(test_functions);
    let module = assertion.pass_module(&source);
    let result = module.get("RESULT").unwrap();
    let function = module.get("lines").unwrap();
    let constant = module.get("constant_lines").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let values = ListRef::from_value(result.value())
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    let expected = "é世界".repeat(4096);
    assert_eq!(strings(values[0]), [expected.as_str()]);
    assert_eq!(strings(values[1]), [expected.as_str()]);
    assert_eq!(strings(values[2]), ["a\r\n", "b\r\n"]);
    let heap = Module::new();
    let mut eval = Evaluator::new(&heap);
    let input = heap.heap().alloc_str("é\r\n世界\n").to_value();
    assert_eq!(
        strings(
            eval.eval_function(function.value(), &[input, heap.heap().alloc(true)], &[])
                .unwrap()
        ),
        ["é\r\n", "世界\n"]
    );
    let retained = eval.eval_function(constant.value(), &[], &[]).unwrap();
    let values = ListRef::from_value(retained)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(strings(values[0]), ["é世界"]);
    assert_eq!(strings(values[1]), ["a\r\n", "b\n"]);
}
