//! Actual function tokens preserve aliases through ordinary dictionaries and GC.
use std::mem::size_of;

use crate::assert::Assert;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;

#[test]
fn actual_factory_functions_keep_distinct_tokens_and_existing_dictionary_keys() {
    Assert::new().pass(
        r#"
def factory():
    return lambda: None
a = factory()
b = factory()
alias = a
assert_eq(a == b, False)
assert_eq(b == a, False)
assert_eq(a, alias)
assert_eq(a == 1, False)
d = {a: 'first', b: 'second'}
tuples = {(a,): 'tuple'}
garbage_collect()
assert_eq(d[alias], 'first')
assert_eq(d[b], 'second')
assert_eq(tuples[(alias,)], 'tuple')
d[alias] = 'replacement'
assert_eq(len(d), 2)
assert_eq(d[a], 'replacement')
"#,
    );
}

#[test]
fn actual_frozen_function_aliases_and_hash_keys_survive_foreign_module_install() {
    let mut a = Assert::new();
    a.module(
        "producer",
        "def f():\n    return 17\nalias = f\nd = {f: 'key'}\n",
    );
    a.pass(
        r#"
load('producer', 'f', 'alias', 'd')
assert_eq(f, alias)
assert_eq(d[alias], 'key')
garbage_collect()
assert_eq(d[f], 'key')
assert_eq(f(), 17)
assert_eq(alias(), 17)
"#,
    );
}

#[test]
fn actual_function_symbol_layout_transfers_without_a_second_heap_allocation() {
    // One shared owner; the token is inline, rather than a second per-function Arc.
    assert_eq!(size_of::<Def<'static>>(), size_of::<FrozenDef>());
}
