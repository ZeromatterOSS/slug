use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::list::ListRef;

#[starlark_module]
fn test_functions(builder: &mut GlobalsBuilder) {
    fn fresh_string<'v>(text: &str, heap: Heap<'v>) -> anyhow::Result<StringValue<'v>> {
        Ok(heap.alloc_str(text))
    }

    fn same_value(left: Value, right: Value) -> anyhow::Result<bool> {
        Ok(left.ptr_eq(right))
    }
}

#[test]
fn limited_replace_reuses_input_when_no_match_is_consumed() {
    let mut a = Assert::new();
    a.globals_add(test_functions);
    a.pass(
        r#"
s = fresh_string("prefix-é-世界-suffix")
assert_eq(same_value(s, s.replace("é", "changed", 0)), True)
assert_eq(same_value(s, s.replace("", "changed", 0)), True)
assert_eq(same_value(s, s.replace("missing", "changed", 3)), True)
assert_eq(same_value(s, s.replace("missing", "changed")), True)
"#,
    );
}

#[test]
fn limited_replace_preserves_bytes_errors_and_frozen_ownership() {
    let mut a = Assert::new();
    a.globals_add(test_functions);
    let module = a.pass_module(
        r#"
s = fresh_string("é世界é")
assert_eq(s.replace("é", "X", 0), "é世界é")
assert_eq(s.replace("é", "X", 1), "X世界é")
assert_eq(s.replace("é", "X", 9), "X世界X")
assert_eq(s.replace("é", "X"), "X世界X")
assert_eq(s.replace("é", "", 1), "世界é")
assert_eq(s.replace("é", "é", 1), s)
assert_eq(s.replace("", "-", 2), "-é-世界é")
assert_eq(s.replace("", "-"), "-é-世-界-é-")
assert_eq(s.replace("", "", 1), s)
assert_eq(fresh_string("aaaaa").replace("aa", "X", 1), "Xaaa")
assert_eq(fresh_string("aaaaa").replace("aa", "X", 9), "XXa")
assert_eq(fresh_string("a").replace("a", "", 1), "")
assert_eq(fresh_string("").replace("", "X", 0), "")
assert_eq(fresh_string("").replace("", "X", 1), "X")
assert_eq(fresh_string("").replace("", "X"), "X")
assert_eq(fresh_string("").replace("a", "X", 9), "")
RESULT = [s.replace("é", "X", 1), s.replace("", "-", 2)]
s = None
garbage_collect()
assert_eq(RESULT, ["X世界é", "-é-世界é"])
"#,
    );
    let held = module.get("RESULT").unwrap();
    let cloned = module.clone();
    drop(module);
    drop(cloned);
    let actual: Vec<_> = ListRef::from_value(held.value())
        .unwrap()
        .iter()
        .map(|value| value.unpack_str().unwrap())
        .collect();
    assert_eq!(actual, ["X世界é", "-é-世界é"]);
    a.fails(
        "fresh_string('banana').replace('a', 'o', -2)",
        &["Replace final argument was negative '-2'"],
    );
}
