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
def left(s, n): return s * n
def right(s, n): return n * s
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
fn multiplication_directions_preserve_bytes_hashes_counts_and_raw_errors() {
    let functions = functions();
    let callees = ["left", "right"].map(|name| functions.get(name).unwrap());
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
            let input_hash = input.get_hashed().unwrap().hash();
            for n in [i32::MIN, -2, -1, 0, 1, 2, 3, 5] {
                let expected = raw.repeat(n.max(0) as usize);
                let reference = module.heap().alloc_str(&expected).to_value();
                for callee in &callees {
                    let actual = eval
                        .eval_function(callee.value(), &[input, module.heap().alloc(n)], &[])
                        .unwrap();
                    assert_eq!(actual.unpack_str(), Some(expected.as_str()));
                    assert_eq!(actual.get_type(), "string");
                    assert_eq!(
                        actual.get_hashed().unwrap().hash(),
                        reference.get_hashed().unwrap().hash()
                    );
                    comparisons += 1;
                }
            }
            assert_eq!(input.unpack_str(), Some(raw.as_str()));
            assert_eq!(input.get_hashed().unwrap().hash(), input_hash);
        }
    }
    let mut large_lengths = Vec::new();
    for raw in ["ab".repeat(49152), "é🙂\0".repeat(16384)] {
        let input = module.heap().alloc_str(&raw).to_value();
        let input_pointer = input.unpack_str().unwrap().as_ptr();
        let expected = raw.repeat(2);
        let reference = module.heap().alloc_str(&expected).to_value();
        for callee in &callees {
            let actual = eval
                .eval_function(callee.value(), &[input, module.heap().alloc(2)], &[])
                .unwrap();
            assert_eq!(actual.unpack_str(), Some(expected.as_str()));
            assert!(!actual.ptr_eq(input));
            assert_eq!(
                actual.get_hashed().unwrap().hash(),
                reference.get_hashed().unwrap().hash()
            );
            assert_eq!(input.unpack_str().unwrap().as_ptr(), input_pointer);
            assert_eq!(input.unpack_str(), Some(raw.as_str()));
            large_lengths.push(actual.unpack_str().unwrap().len());
            comparisons += 1;
        }
    }
    let invalid = [
        Value::new_none(),
        module.heap().alloc(true),
        module.heap().alloc(2.0f64),
        module.heap().alloc_str("2").to_value(),
        module.heap().alloc(vec![1]),
        module.heap().alloc(2147483648i64),
        module.heap().alloc(-2147483649i64),
    ];
    let input = module.heap().alloc_str("aé🙂\0").to_value();
    let mut errors = Vec::new();
    for other in invalid {
        for callee in &callees {
            errors.push(
                eval.eval_function(callee.value(), &[input, other], &[])
                    .unwrap_err()
                    .to_string(),
            );
        }
    }
    assert_eq!(errors.len(), 14);
    assert_eq!(large_lengths, [196608, 196608, 229376, 229376]);
    println!("REPEAT_TWO_COMPARISONS={comparisons}");
    println!("REPEAT_TWO_LARGE={large_lengths:?}");
    println!(
        "REPEAT_TWO_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}

#[test]
fn repeated_outputs_and_functions_survive_gc_freeze_and_source_drop() {
    let source = String::from(
        r#"
def left(s): return s * 2
def right(s): return 2 * s
def constants(): return ["é🙂\0" * 2, 2 * "ab", "" * 2, "a" * -1, "a" * 3]
s = fresh_string("é🙂\0" * 4096)
t = fresh_string("ab" * 4096)
RESULT = [left(s), right(s), left(t), right(t)]
s = None
t = None
garbage_collect()
assert_eq(RESULT, ["é🙂\0" * 8192, "é🙂\0" * 8192, "ab" * 8192, "ab" * 8192])
assert_eq(constants(), ["é🙂\0é🙂\0", "abab", "", "", "aaa"])
"#,
    );
    let mut a = Assert::new();
    a.globals_add(test_functions);
    let frozen = a.pass_module(&source);
    let held = frozen.get("RESULT").unwrap();
    let left = frozen.get("left").unwrap();
    let right = frozen.get("right").unwrap();
    let constants = frozen.get("constants").unwrap();
    let clone = frozen.clone();
    drop(source);
    drop(frozen);
    drop(clone);
    let actual = ListRef::from_value(held.value())
        .unwrap()
        .iter()
        .map(|v| v.unpack_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual[0], "é🙂\0".repeat(8192));
    assert_eq!(actual[1], actual[0]);
    assert_eq!(actual[2], "ab".repeat(8192));
    assert_eq!(actual[3], actual[2]);
    println!(
        "REPEAT_TWO_RETAINED={:?}",
        actual.iter().map(|s| s.len()).collect::<Vec<_>>()
    );
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    let raw = String::from("prefix-é-🙂-\0-suffix");
    let input = module.heap().alloc_str(&raw).to_value();
    drop(raw);
    for callee in [&left, &right] {
        let actual = eval.eval_function(callee.value(), &[input], &[]).unwrap();
        assert_eq!(
            actual.unpack_str(),
            Some("prefix-é-🙂-\0-suffixprefix-é-🙂-\0-suffix")
        );
    }
    let result = eval.eval_function(constants.value(), &[], &[]).unwrap();
    let constants = ListRef::from_value(result)
        .unwrap()
        .iter()
        .map(|v| v.unpack_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(constants, ["é🙂\0é🙂\0", "abab", "", "", "aaa"]);
}
