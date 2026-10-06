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
def lower_case(s: str) -> str: return s.lower()
def upper_case(s: str) -> str: return s.upper()
def bad_lower(s): return s.lower(0)
def bad_upper(s): return s.upper(unexpected = 0)
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
fn unchanged_ascii_case_reuses_input_without_heap_growth() {
    let functions = functions();
    let callees = ["lower_case", "upper_case"].map(|name| functions.get(name).unwrap());
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    // Isolate allocation accounting from automatic collection. Lifecycle and
    // explicit GC remain enabled in the separate retained-owner control.
    eval.disable_gc();
    let mut sharing = Vec::new();
    let mut growths = Vec::new();
    for (function, (name, raw)) in callees.iter().zip([
        ("lower_case", "already_lower_0123456789".repeat(4096)),
        ("upper_case", "ALREADY_UPPER_0123456789".repeat(4096)),
    ]) {
        let input = module.heap().alloc_str(&raw).to_value();
        let before = module.heap().allocated_bytes();
        let result = eval.eval_function(function.value(), &[input], &[]).unwrap();
        let growth = module.heap().allocated_bytes() - before;
        assert_eq!(result.unpack_str(), Some(raw.as_str()));
        let shared = result.ptr_eq(input);
        println!(
            "ASCII_CASE method={name} bytes={} shared={shared} heap_growth={growth}",
            raw.len()
        );
        sharing.push(shared);
        growths.push(growth);
    }
    assert_eq!(sharing, [true, true]);
    assert_eq!(growths, [0, 0]);
}

#[test]
fn ascii_pairs_unicode_fallback_types_and_exact_errors_are_preserved() {
    let functions = functions();
    let lower = functions.get("lower_case").unwrap();
    let upper = functions.get("upper_case").unwrap();
    let error_functions = ["bad_lower", "bad_upper", "lower_case", "upper_case"]
        .map(|name| (name, functions.get(name).unwrap()));
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.disable_gc();
    let mut comparisons = 0;
    for a in 0u8..=127 {
        for b in 0u8..=127 {
            let pair = [a, b];
            let raw = std::str::from_utf8(&pair).unwrap();
            let input = module.heap().alloc_str(raw).to_value();
            for (function, expected) in [(&lower, raw.to_lowercase()), (&upper, raw.to_uppercase())]
            {
                let result = eval.eval_function(function.value(), &[input], &[]).unwrap();
                assert_eq!(result.unpack_str(), Some(expected.as_str()));
                comparisons += 1;
            }
        }
    }
    for (raw, expected_lower, expected_upper) in [
        ("", "", ""),
        ("éÉ", "éé", "ÉÉ"),
        ("ǅ", "ǆ", "Ǆ"),
        ("İ", "i\u{307}", "İ"),
        ("ß", "ß", "SS"),
        ("Σσς", "σσς", "ΣΣΣ"),
        ("K", "k", "K"),
        ("ﬀ", "ﬀ", "FF"),
        ("世界😀", "世界😀", "世界😀"),
        ("e\u{301}", "e\u{301}", "E\u{301}"),
        ("aÉ\0Z", "aé\0z", "AÉ\0Z"),
    ] {
        let input = module.heap().alloc_str(raw).to_value();
        for (function, expected) in [(&lower, expected_lower), (&upper, expected_upper)] {
            let result = eval.eval_function(function.value(), &[input], &[]).unwrap();
            assert_eq!(result.unpack_str(), Some(expected));
        }
    }
    let mut errors = Vec::new();
    for (name, function) in &error_functions {
        let input = if name.starts_with("bad_") {
            module.heap().alloc_str("input").to_value()
        } else {
            module.heap().alloc(1)
        };
        errors.push(
            eval.eval_function(function.value(), &[input], &[])
                .unwrap_err()
                .to_string(),
        );
    }
    println!("ASCII_CASE_COMPARISONS={comparisons}");
    println!(
        "ASCII_CASE_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}

#[test]
fn case_results_and_functions_survive_gc_freeze_and_module_drop() {
    let source = String::from(
        r#"
def lower_case(s): return s.lower()
def upper_case(s): return s.upper()
def constant_case(): return ["lower".lower(), "UPPER".upper(), "É".lower(), "ß".upper()]
s = fresh_string("lower" * 4096)
t = fresh_string("UPPER" * 4096)
RESULT = [lower_case(s), upper_case(t), lower_case(fresh_string("ǅ")), upper_case(fresh_string("ß"))]
s = None
t = None
garbage_collect()
assert_eq(RESULT, ["lower" * 4096, "UPPER" * 4096, "ǆ", "SS"])
assert_eq(constant_case(), ["lower", "UPPER", "é", "SS"])
"#,
    );
    let mut assertion = Assert::new();
    assertion.globals_add(test_functions);
    let module = assertion.pass_module(&source);
    let result = module.get("RESULT").unwrap();
    let lower = module.get("lower_case").unwrap();
    let upper = module.get("upper_case").unwrap();
    let constant = module.get("constant_case").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let expected = [
        "lower".repeat(4096),
        "UPPER".repeat(4096),
        "ǆ".to_owned(),
        "SS".to_owned(),
    ];
    let values = ListRef::from_value(result.value()).unwrap();
    assert_eq!(
        values
            .iter()
            .map(|value| value.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    let heap = Module::new();
    let mut eval = Evaluator::new(&heap);
    for (function, raw, expected) in [(&lower, "aÉ\0Z", "aé\0z"), (&upper, "aÉ\0Z", "AÉ\0Z")] {
        let input = heap.heap().alloc_str(raw).to_value();
        assert_eq!(
            eval.eval_function(function.value(), &[input], &[])
                .unwrap()
                .unpack_str(),
            Some(expected)
        );
    }
    let retained = eval.eval_function(constant.value(), &[], &[]).unwrap();
    assert_eq!(
        ListRef::from_value(retained)
            .unwrap()
            .iter()
            .map(|value| value.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["lower", "UPPER", "é", "SS"]
    );
}
