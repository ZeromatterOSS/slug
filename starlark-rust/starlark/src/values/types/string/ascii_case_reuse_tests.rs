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
def capitalize_case(s: str) -> str: return s.capitalize()
def title_case(s: str) -> str: return s.title()
def bad_capitalize(s): return s.capitalize(0)
def bad_title(s): return s.title(unexpected = 0)
def bad_lower(s): return s.lower(0)
def bad_upper(s): return s.upper(unexpected = 0)
"#,
    )
}

// Character-based references keep Unicode expansions and the original absolute
// first-position / preceding-letter semantics independent of the byte guards.
fn capitalize_reference(raw: &str) -> String {
    raw.chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().collect::<String>()
            } else {
                c.to_lowercase().collect::<String>()
            }
        })
        .collect()
}

fn title_reference(raw: &str) -> String {
    let mut previous_letter = false;
    raw.chars()
        .map(|c| {
            let word_start = !previous_letter;
            previous_letter = c.is_alphabetic();
            if word_start && previous_letter {
                c.to_uppercase().collect::<String>()
            } else {
                c.to_lowercase().collect::<String>()
            }
        })
        .collect()
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
    let callees = ["lower_case", "upper_case", "capitalize_case", "title_case"]
        .map(|name| functions.get(name).unwrap());
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
        ("capitalize_case", format!("A{}", "a".repeat(98303))),
        ("title_case", "Already_Title_0123456789".repeat(4096)),
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
    assert_eq!(sharing, [true; 4]);
    assert_eq!(growths, [0; 4]);
}

#[test]
fn ascii_pairs_unicode_fallback_types_and_exact_errors_are_preserved() {
    let functions = functions();
    let lower = functions.get("lower_case").unwrap();
    let upper = functions.get("upper_case").unwrap();
    let capitalize = functions.get("capitalize_case").unwrap();
    let title = functions.get("title_case").unwrap();
    let error_functions = [
        "bad_lower",
        "bad_upper",
        "bad_capitalize",
        "bad_title",
        "lower_case",
        "upper_case",
        "capitalize_case",
        "title_case",
    ]
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
            for (function, expected) in [
                (&lower, raw.to_lowercase()),
                (&upper, raw.to_uppercase()),
                (&capitalize, capitalize_reference(raw)),
                (&title, title_reference(raw)),
            ] {
                let result = eval.eval_function(function.value(), &[input], &[]).unwrap();
                assert_eq!(result.unpack_str(), Some(expected.as_str()));
                comparisons += 1;
            }
        }
    }
    let alphabet = ['a', 'A', 'b', 'B', ' ', '_', '0', '\0'];
    for len in 0..=4u32 {
        for mut index in 0..alphabet.len().pow(len) {
            let mut raw = String::new();
            for _ in 0..len {
                raw.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            let input = module.heap().alloc_str(&raw).to_value();
            for (function, expected) in [
                (&capitalize, capitalize_reference(&raw)),
                (&title, title_reference(&raw)),
            ] {
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
        (
            "@hELLo_world 12foo",
            "@hello_world 12foo",
            "@HELLO_WORLD 12FOO",
        ),
    ] {
        let input = module.heap().alloc_str(raw).to_value();
        let expected_capitalize = capitalize_reference(raw);
        let expected_title = title_reference(raw);
        for (function, expected) in [
            (&lower, expected_lower),
            (&upper, expected_upper),
            (&capitalize, expected_capitalize.as_str()),
            (&title, expected_title.as_str()),
        ] {
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
def capitalize_case(s): return s.capitalize()
def title_case(s): return s.title()
def constant_case(): return ["lower".lower(), "UPPER".upper(), "É".lower(), "ß".upper(), "Already capitalized".capitalize(), "Already_Title".title()]
s = fresh_string("lower" * 4096)
t = fresh_string("UPPER" * 4096)
u = fresh_string("A" + "a" * 4096)
v = fresh_string("Already_Title_012" * 4096)
RESULT = [lower_case(s), upper_case(t), lower_case(fresh_string("ǅ")), upper_case(fresh_string("ß")), capitalize_case(u), title_case(v), capitalize_case(fresh_string("ǅ")), title_case(fresh_string("ß"))]
s = None
t = None
u = None
v = None
garbage_collect()
assert_eq(RESULT, ["lower" * 4096, "UPPER" * 4096, "ǆ", "SS", "A" + "a" * 4096, "Already_Title_012" * 4096, "Ǆ", "SS"])
assert_eq(constant_case(), ["lower", "UPPER", "é", "SS", "Already capitalized", "Already_Title"])
"#,
    );
    let mut assertion = Assert::new();
    assertion.globals_add(test_functions);
    let module = assertion.pass_module(&source);
    let result = module.get("RESULT").unwrap();
    let lower = module.get("lower_case").unwrap();
    let upper = module.get("upper_case").unwrap();
    let capitalize = module.get("capitalize_case").unwrap();
    let title = module.get("title_case").unwrap();
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
        format!("A{}", "a".repeat(4096)),
        "Already_Title_012".repeat(4096),
        "Ǆ".to_owned(),
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
    for (function, raw, expected) in [
        (&lower, "aÉ\0Z", "aé\0z"),
        (&upper, "aÉ\0Z", "AÉ\0Z"),
        (&capitalize, "aÉ\0Z", "Aé\0z"),
        (&title, "aÉ\0Z", "Aé\0Z"),
    ] {
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
        [
            "lower",
            "UPPER",
            "é",
            "SS",
            "Already capitalized",
            "Already_Title"
        ]
    );
}
