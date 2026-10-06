use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::FrozenModule;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::list::ListRef;

fn functions() -> FrozenModule {
    Assert::new().pass_module(
        r#"
def split_parts(s: str, sep, limit) -> list[str]: return s.split(sep, limit)
def rsplit_parts(s: str, sep, limit) -> list[str]: return s.rsplit(sep, limit)
def default_split(s): return s.split()
def default_rsplit(s): return s.rsplit()
def extra(s): return s.split(None, None, None)
def named(s): return s.rsplit(sep = "|")
def wrong_sep(s): return s.split(1)
def wrong_limit(s): return s.rsplit(None, "bad")
"#,
    )
}

fn strings(value: Value<'_>) -> Vec<&str> {
    ListRef::from_value(value)
        .unwrap()
        .iter()
        .map(|v| v.unpack_str().unwrap())
        .collect()
}

fn arguments<'v>(
    heap: Heap<'v>,
    input: Value<'v>,
    sep: Option<&str>,
    limit: Option<i32>,
) -> [Value<'v>; 3] {
    [
        input,
        sep.map_or_else(Value::new_none, |s| heap.alloc_str(s).to_value()),
        limit.map_or_else(Value::new_none, |n| heap.alloc(n)),
    ]
}

// The explicit-separator iterators define the inherited Rust behavior. For
// bounded whitespace splitting, independently frame the remaining original
// source around word ranges rather than copying the production cursor loop.
fn expected_parts(raw: &str, sep: Option<&str>, limit: Option<i32>, reverse: bool) -> Vec<String> {
    let limit = limit.filter(|n| *n >= 0).map(|n| n as usize);
    if let Some(sep) = sep {
        let mut parts: Vec<&str> = match (reverse, limit) {
            (false, None) => raw.split(sep).collect(),
            (false, Some(n)) => raw.splitn(n + 1, sep).collect(),
            (true, None) => raw.rsplit(sep).collect(),
            (true, Some(n)) => raw.rsplitn(n + 1, sep).collect(),
        };
        if reverse {
            parts.reverse();
        }
        return parts.into_iter().map(str::to_owned).collect();
    }
    let words = raw.split_whitespace().collect::<Vec<_>>();
    let Some(limit) = limit.filter(|n| words.len() > *n) else {
        return words.into_iter().map(str::to_owned).collect();
    };
    if reverse {
        let word = words[words.len() - limit - 1];
        let end = word.as_ptr() as usize - raw.as_ptr() as usize + word.len();
        std::iter::once(&raw[..end])
            .chain(words[words.len() - limit..].iter().copied())
            .map(str::to_owned)
            .collect()
    } else {
        let word = words[limit];
        let start = word.as_ptr() as usize - raw.as_ptr() as usize;
        words[..limit]
            .iter()
            .copied()
            .chain(std::iter::once(&raw[start..]))
            .map(str::to_owned)
            .collect()
    }
}

#[test]
fn complete_split_results_share_input_across_directions_and_routes() {
    let functions = functions();
    let callees = ["split_parts", "rsplit_parts"].map(|n| functions.get(n).unwrap());
    let defaults = ["default_split", "default_rsplit"].map(|n| functions.get(n).unwrap());
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    // Isolate public heap accounting; retained mutable owners use explicit GC.
    eval.disable_gc();
    let raws = ["payload_01234567".repeat(8192), "é世界😀_012".repeat(8192)];
    let mut sharing = Vec::new();
    for ((name, callee), raw) in ["split_parts", "rsplit_parts"]
        .into_iter()
        .zip(&callees)
        .zip(&raws)
    {
        let input = module.heap().alloc_str(raw).to_value();
        let args = arguments(module.heap(), input, Some("Ω"), None);
        let before = module.heap().allocated_bytes();
        let result = eval.eval_function(callee.value(), &args, &[]).unwrap();
        let growth = module.heap().allocated_bytes() - before;
        assert_eq!(strings(result), [raw.as_str()]);
        let shared = ListRef::from_value(result)
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .ptr_eq(input);
        println!(
            "SPLIT_INPUT method={name} bytes={} shared={shared} heap_growth={growth}",
            raw.len()
        );
        sharing.push(shared);
    }
    for raw in &raws {
        let input = module.heap().alloc_str(raw).to_value();
        for callee in &callees {
            for (sep, limit) in [
                (None, None),
                (None, Some(0)),
                (None, Some(1)),
                (Some("|"), None),
                (Some("::"), None),
                (Some("Ω"), None),
                (Some("|"), Some(0)),
                (Some("::"), Some(1)),
                (Some("Ω"), Some(-1)),
            ] {
                let result = eval
                    .eval_function(
                        callee.value(),
                        &arguments(module.heap(), input, sep, limit),
                        &[],
                    )
                    .unwrap();
                assert_eq!(strings(result), [raw.as_str()]);
                sharing.push(
                    ListRef::from_value(result)
                        .unwrap()
                        .iter()
                        .next()
                        .unwrap()
                        .ptr_eq(input),
                );
            }
        }
        for callee in &defaults {
            let result = eval.eval_function(callee.value(), &[input], &[]).unwrap();
            assert_eq!(strings(result), [raw.as_str()]);
            sharing.push(
                ListRef::from_value(result)
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .ptr_eq(input),
            );
        }
    }
    println!("SPLIT_INPUT_SHARING_PROBES={}", sharing.len());
    assert_eq!(sharing.len(), 42);
    assert!(sharing.iter().all(|shared| *shared));
}

#[test]
fn split_bytes_trimming_limits_lists_and_exact_errors_are_preserved() {
    let functions = functions();
    let callees = ["split_parts", "rsplit_parts"].map(|n| functions.get(n).unwrap());
    let errors = [
        "extra",
        "named",
        "wrong_sep",
        "wrong_limit",
        "split_parts",
        "rsplit_parts",
    ]
    .map(|n| (n, functions.get(n).unwrap()));
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.disable_gc();
    for (raw, sep, limit, left, right) in [
        ("", None, None, vec![], vec![]),
        ("", Some("|"), None, vec![""], vec![""]),
        ("  a  b  ", None, Some(0), vec!["a  b  "], vec!["  a  b"]),
        (
            "  a  b  c  ",
            None,
            Some(1),
            vec!["a", "b  c  "],
            vec!["  a  b", "c"],
        ),
        (
            "a||b|",
            Some("|"),
            None,
            vec!["a", "", "b", ""],
            vec!["a", "", "b", ""],
        ),
        (
            "a|b|c",
            Some("|"),
            Some(1),
            vec!["a", "b|c"],
            vec!["a|b", "c"],
        ),
        ("a|b", Some("|"), Some(0), vec!["a|b"], vec!["a|b"]),
        ("aaaa", Some("aa"), None, vec!["", "", ""], vec!["", "", ""]),
        (
            "é\u{2003}世界\t😀",
            None,
            None,
            vec!["é", "世界", "😀"],
            vec!["é", "世界", "😀"],
        ),
        (
            "aébé",
            Some("é"),
            None,
            vec!["a", "b", ""],
            vec!["a", "b", ""],
        ),
    ] {
        let input = module.heap().alloc_str(raw).to_value();
        for (callee, expected) in callees.iter().zip([left, right]) {
            let result = eval
                .eval_function(
                    callee.value(),
                    &arguments(module.heap(), input, sep, limit),
                    &[],
                )
                .unwrap();
            assert_eq!(strings(result), expected);
        }
    }
    let alphabet = ['a', '|', ' ', 'é', '\r', '\u{2003}'];
    let mut comparisons = 0;
    for len in 0..=4u32 {
        for mut index in 0..alphabet.len().pow(len) {
            let mut raw = String::new();
            for _ in 0..len {
                raw.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            let input = module.heap().alloc_str(&raw).to_value();
            for sep in [None, Some("|"), Some("||"), Some("é"), Some("")] {
                for limit in [None, Some(-1), Some(0), Some(1), Some(2)] {
                    for (reverse, callee) in callees.iter().enumerate() {
                        let result = eval
                            .eval_function(
                                callee.value(),
                                &arguments(module.heap(), input, sep, limit),
                                &[],
                            )
                            .unwrap();
                        assert_eq!(
                            strings(result),
                            expected_parts(&raw, sep, limit, reverse == 1)
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    let mut records = Vec::new();
    for (name, callee) in &errors {
        let args = if name.ends_with("_parts") {
            arguments(module.heap(), module.heap().alloc(1), None, None).to_vec()
        } else {
            vec![module.heap().alloc_str("input").to_value()]
        };
        records.push(
            eval.eval_function(callee.value(), &args, &[])
                .unwrap_err()
                .to_string(),
        );
    }
    println!("SPLIT_INPUT_COMPARISONS={comparisons}");
    println!(
        "SPLIT_INPUT_ERRORS={}",
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
fn split_results_and_functions_survive_gc_freeze_and_source_drop() {
    let source = String::from(
        r#"
def split_parts(s, sep, limit): return s.split(sep, limit)
def rsplit_parts(s, sep, limit): return s.rsplit(sep, limit)
def constant_parts(): return ["é世界".split("Ω"), "a|b|c".rsplit("|", 1)]
s = fresh_string("é世界" * 4096)
t = fresh_string("one two " * 4096)
RESULT = [s.split(), s.rsplit("Ω"), t.split(None, 0), fresh_string("a|b|c").rsplit("|", 1)]
s = None
t = None
garbage_collect()
assert_eq(RESULT, [["é世界" * 4096], ["é世界" * 4096], ["one two " * 4096], ["a|b", "c"]])
x = fresh_string("one").split()
y = fresh_string("two").rsplit()
x.append("more")
y[0] = "changed"
assert_eq(type(x), "list")
assert_eq(type(y), "list")
assert_eq([x, y], [["one", "more"], ["changed"]])
assert_eq(constant_parts(), [["é世界"], ["a|b", "c"]])
"#,
    );
    let mut assertion = Assert::new();
    assertion.globals_add(test_functions);
    let module = assertion.pass_module(&source);
    let result = module.get("RESULT").unwrap();
    let split = module.get("split_parts").unwrap();
    let rsplit = module.get("rsplit_parts").unwrap();
    let constant = module.get("constant_parts").unwrap();
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
    let expected = "one two ".repeat(4096);
    assert_eq!(strings(values[2]), [expected.as_str()]);
    assert_eq!(strings(values[3]), ["a|b", "c"]);
    let heap = Module::new();
    let mut eval = Evaluator::new(&heap);
    for (callee, expected) in [(&split, vec!["a", "b|c"]), (&rsplit, vec!["a|b", "c"])] {
        let input = heap.heap().alloc_str("a|b|c").to_value();
        let result = eval
            .eval_function(
                callee.value(),
                &arguments(heap.heap(), input, Some("|"), Some(1)),
                &[],
            )
            .unwrap();
        assert_eq!(strings(result), expected);
    }
    let result = eval.eval_function(constant.value(), &[], &[]).unwrap();
    let values = ListRef::from_value(result)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(strings(values[0]), ["é世界"]);
    assert_eq!(strings(values[1]), ["a|b", "c"]);
}
