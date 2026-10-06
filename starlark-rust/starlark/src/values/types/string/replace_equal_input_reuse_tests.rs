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
    let mut a = Assert::new();
    // Declare deliberately invalid calls for testing native runtime diagnostics.
    a.disable_static_typechecking();
    a.pass_module(
        r#"
def replace_case(s, old, new, count): return s.replace(old, new, count)
def default_case(s, old, new): return s.replace(old, new)
def negative(s): return s.replace("a", "a", -1)
def empty_negative(s): return s.replace("", "", -2)
def wrong_old(s): return s.replace(1, 1)
def wrong_new(s): return s.replace("a", 1)
def wrong_count(s): return s.replace("a", "a", "bad")
def none_count(s): return s.replace("a", "a", None)
def large_count(s): return s.replace("a", "a", 2147483648)
def extra(s): return s.replace("a", "a", 1, 2)
def named(s): return s.replace(old = "a", new = "a")
"#,
    )
}

#[starlark_module]
fn test_functions(builder: &mut GlobalsBuilder) {
    fn fresh_string<'v>(text: &str, heap: Heap<'v>) -> anyhow::Result<StringValue<'v>> {
        Ok(heap.alloc_str(text))
    }
}

#[test]
fn equal_replacements_share_large_inputs_without_added_heap_accounting() {
    let functions = functions();
    let limited = functions.get("replace_case").unwrap();
    let unlimited = functions.get("default_case").unwrap();
    let mut sharing = Vec::new();
    let mut growths = Vec::new();
    for (raw, pattern) in [("ab".repeat(49152), "ab"), ("é🙂\0".repeat(16384), "é🙂\0")] {
        for count in [Some(1), None] {
            let module = Module::new();
            let mut eval = Evaluator::new(&module);
            // Isolate each arena observation from other fixtures and GC.
            eval.disable_gc();
            let input = module.heap().alloc_str(&raw).to_value();
            // Equal bytes have independent owners; no pointer comparison selects the fast path.
            let old = module.heap().alloc_str(pattern).to_value();
            let new = module.heap().alloc_str(pattern).to_value();
            assert!(!old.ptr_eq(new));
            let before = module.heap().allocated_bytes();
            let result = match count {
                Some(n) => eval.eval_function(
                    limited.value(),
                    &[input, old, new, module.heap().alloc(n)],
                    &[],
                ),
                None => eval.eval_function(unlimited.value(), &[input, old, new], &[]),
            }
            .unwrap();
            let growth = module.heap().allocated_bytes() - before;
            assert_eq!(result.unpack_str(), Some(raw.as_str()));
            let shared = result.ptr_eq(input);
            println!(
                "EQUAL_REPLACE bytes={} count={count:?} shared={shared} heap_growth={growth}",
                raw.len()
            );
            sharing.push(shared);
            growths.push(growth);
        }
    }
    // Capture all observations before the baseline's intended ownership failure.
    assert_eq!(sharing, [true; 4]);
    assert_eq!(growths, [0; 4]);
}

#[test]
fn equal_and_changed_replacements_preserve_bytes_counts_types_and_raw_errors() {
    let functions = functions();
    let function = functions.get("replace_case").unwrap();
    let default = functions.get("default_case").unwrap();
    let error_functions = [
        "negative",
        "empty_negative",
        "wrong_old",
        "wrong_new",
        "wrong_count",
        "none_count",
        "large_count",
        "extra",
        "named",
    ]
    .map(|name| functions.get(name).unwrap());
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.disable_gc();
    let alphabet = ['a', 'é', '🙂', '\0'];
    let mut comparisons = 0;
    for length in 0..=4u32 {
        for mut index in 0..alphabet.len().pow(length) {
            let mut raw = String::new();
            for _ in 0..length {
                raw.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            let input = module.heap().alloc_str(&raw).to_value();
            for old in ["", "a", "aa", "é", "🙂", "\0"] {
                for new in [old, "#"] {
                    let old_value = module.heap().alloc_str(old).to_value();
                    let new_value = module.heap().alloc_str(new).to_value();
                    for count in [Some(0), Some(1), Some(2), None] {
                        let expected =
                            raw.replacen(old, new, count.map_or(usize::MAX, |n| n as usize));
                        let actual = match count {
                            Some(n) => eval.eval_function(
                                function.value(),
                                &[input, old_value, new_value, module.heap().alloc(n)],
                                &[],
                            ),
                            None => eval.eval_function(
                                default.value(),
                                &[input, old_value, new_value],
                                &[],
                            ),
                        }
                        .unwrap();
                        assert_eq!(actual.unpack_str(), Some(expected.as_str()));
                        comparisons += 1;
                    }
                }
            }
        }
    }
    for (raw, old, new, count) in [
        ("aaaaa", "aa", "aa", Some(9)),
        ("aaaaa", "aa", "X", Some(9)),
        ("aé🙂aé🙂", "aé🙂", "aé🙂", Some(1)),
        ("aé🙂aé🙂", "aé🙂", "", None),
        ("e\u{301}é", "é", "é", None),
        ("é🙂\0", "", "", Some(i32::MAX)),
    ] {
        let input = module.heap().alloc_str(raw).to_value();
        let expected = raw.replacen(old, new, count.map_or(usize::MAX, |n| n as usize));
        let args = [
            input,
            module.heap().alloc_str(old).to_value(),
            module.heap().alloc_str(new).to_value(),
        ];
        let actual = match count {
            Some(n) => eval.eval_function(
                function.value(),
                &[args[0], args[1], args[2], module.heap().alloc(n)],
                &[],
            ),
            None => eval.eval_function(default.value(), &args, &[]),
        }
        .unwrap();
        assert_eq!(actual.unpack_str(), Some(expected.as_str()));
        comparisons += 1;
    }
    let mut errors = Vec::new();
    for function in &error_functions {
        let input = module.heap().alloc_str("aé🙂").to_value();
        errors.push(
            eval.eval_function(function.value(), &[input], &[])
                .unwrap_err()
                .to_string(),
        );
    }
    errors.push(
        eval.eval_function(
            default.value(),
            &[
                module.heap().alloc(1),
                module.heap().alloc_str("a").to_value(),
                module.heap().alloc_str("a").to_value(),
            ],
            &[],
        )
        .unwrap_err()
        .to_string(),
    );
    assert!(errors[0].contains("Replace final argument was negative '-1'"));
    assert!(errors[1].contains("Replace final argument was negative '-2'"));
    println!("EQUAL_REPLACE_COMPARISONS={comparisons}");
    println!(
        "EQUAL_REPLACE_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}

#[test]
fn reused_and_changed_results_survive_gc_freeze_and_source_module_drop() {
    let source = String::from(
        r#"
def equal_case(s): return s.replace("é", "é")
def different_case(s): return s.replace("é", "changed", 1)
def constants(): return ["aaé🙂".replace("a", "a"), "aaé🙂".replace("", "", 2), "aaaaa".replace("aa", "X", 9)]
s = fresh_string("é🙂\0" * 4096)
t = fresh_string("ab" * 4096)
RESULT = [equal_case(s), s.replace("", "", 9), t.replace("a", "a", 1), different_case(s)]
s = None
t = None
garbage_collect()
assert_eq(RESULT, ["é🙂\0" * 4096, "é🙂\0" * 4096, "ab" * 4096, "changed" + "🙂\0" + "é🙂\0" * 4095])
assert_eq(constants(), ["aaé🙂", "aaé🙂", "XXa"])
"#,
    );
    let mut a = Assert::new();
    a.globals_add(test_functions);
    let module = a.pass_module(&source);
    let held = module.get("RESULT").unwrap();
    let equal = module.get("equal_case").unwrap();
    let different = module.get("different_case").unwrap();
    let constants = module.get("constants").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let result = ListRef::from_value(held.value()).unwrap();
    let actual = result
        .iter()
        .map(|v| v.unpack_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual[0], "é🙂\0".repeat(4096));
    assert_eq!(actual[1], actual[0]);
    assert_eq!(actual[2], "ab".repeat(4096));
    assert_eq!(actual[3], format!("changed🙂\0{}", "é🙂\0".repeat(4095)));
    println!(
        "EQUAL_REPLACE_RETAINED={:?}",
        actual.iter().map(|s| s.len()).collect::<Vec<_>>()
    );
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    let raw = String::from("prefix-é-🙂-suffix");
    let input = module.heap().alloc_str(&raw).to_value();
    drop(raw);
    let result = eval.eval_function(equal.value(), &[input], &[]).unwrap();
    assert_eq!(result.unpack_str(), Some("prefix-é-🙂-suffix"));
    let result = eval
        .eval_function(different.value(), &[input], &[])
        .unwrap();
    assert_eq!(result.unpack_str(), Some("prefix-changed-🙂-suffix"));
    let result = eval.eval_function(constants.value(), &[], &[]).unwrap();
    assert_eq!(
        ListRef::from_value(result)
            .unwrap()
            .iter()
            .map(|v| v.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["aaé🙂", "aaé🙂", "XXa"]
    );
}
