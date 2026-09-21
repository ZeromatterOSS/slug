//! Generic host proxy identity: no application-specific recipes or code capture.
use std::fmt;
use std::hash::Hasher;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use starlark_derive::NoSerialize;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert::Assert;
use crate::collections::StarlarkHashValue;
use crate::collections::StarlarkHasher;
use crate::environment::GlobalsBuilder;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::starlark_module;
use crate::values::Demand;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::function::StarlarkFunctionIdentity;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct NativeProxy {
    token: Arc<()>,
    name_hash: StarlarkHashValue,
}
crate::starlark_simple_value!(NativeProxy);

impl fmt::Display for NativeProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<native function proxy>")
    }
}
impl StarlarkFunctionIdentity for NativeProxy {
    fn issued_identity(&self) -> Option<&Arc<()>> {
        Some(&self.token)
    }
    fn issue_identity(&self) -> Arc<()> {
        self.token.clone()
    }
    fn name_hash(&self) -> StarlarkHashValue {
        self.name_hash
    }
}
#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for NativeProxy {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkFunctionIdentity>(self);
    }
    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        Ok(other
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .and_then(|other| other.issued_identity())
            .is_some_and(|other| Arc::ptr_eq(&self.token, other)))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        hasher.write_u32(self.name_hash.get());
        Ok(())
    }
}

#[starlark_module]
fn helpers(builder: &mut GlobalsBuilder) {
    fn proxy<'v>(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        let identity = value
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .ok_or_else(|| anyhow::anyhow!("function identity capability missing"))?;
        Ok(heap.alloc(NativeProxy {
            token: identity.issue_identity(),
            name_hash: identity.name_hash(),
        }))
    }
    fn issued(value: Value<'_>) -> anyhow::Result<bool> {
        Ok(value
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .ok_or_else(|| anyhow::anyhow!("function identity capability missing"))?
            .issued_identity()
            .is_some())
    }
    fn same_hash(left: Value<'_>, right: Value<'_>) -> starlark::Result<bool> {
        Ok(left.get_hashed()?.hash() == right.get_hashed()?.hash())
    }
}
fn harness() -> Assert<'static> {
    let mut assert = Assert::new();
    assert.globals_add(helpers);
    assert
}

#[test]
fn ordinary_functions_and_capability_queries_do_not_issue_identity() {
    harness().pass(
        r#"
def factory():
    def same_name():
        return 1
    return same_name
a = factory()
b = factory()
assert_eq(a == b, False)
assert_eq(b == a, False)
assert_eq(a, a)
assert_eq(issued(a), False)
assert_eq(issued(b), False)
assert_eq(same_hash(a, b), True)
d = {a: "first", b: "second"}
assert_eq(d[a], "first")
assert_eq(issued(a), False)
assert_eq(issued(b), False)
p = proxy(a)
assert_eq(a == b, False)
assert_eq(b == a, False)
assert_eq(issued(b), False)
assert_eq(a == 1, False)
assert_eq(1 == a, False)
"#,
    );
}

#[test]
fn live_proxy_equality_hash_and_existing_dict_keys_survive_gc() {
    // Assert runs Auto, Never and Always collection strategies.
    harness().pass(
        r#"
def factory():
    return lambda: None
f = factory()
other = factory()
d = {f: "before issuance"}
tuple_keys = {(f,): "tuple before issuance"}
p = proxy(f)
alias = proxy(f)
assert_eq(f == p, True)
assert_eq(p == f, True)
assert_eq(p == alias, True)
assert_eq(alias == p, True)
assert_eq(same_hash(f, p), True)
assert_eq(same_hash((f,), (p,)), True)
assert_eq(d[p], "before issuance")
assert_eq(tuple_keys[(p,)], "tuple before issuance")
d[p] = "replacement"
assert_eq(len(d), 1)
assert_eq(d[f], "replacement")
inverse = {p: "proxy first"}
assert_eq(inverse[f], "proxy first")
garbage_collect()
assert_eq(f == p, True)
assert_eq(p == f, True)
assert_eq(d[p], "replacement")
q = proxy(other)
assert_eq(f == other, False)
assert_eq(f == q, False)
assert_eq(q == f, False)
assert_eq(p == q, False)
"#,
    );
}

#[test]
fn identity_issued_before_and_after_freeze_preserves_aliases_and_hashes() {
    let mut a = harness();
    a.module(
        "before",
        r#"
def f():
    pass
d = {f: "old key"}
p = proxy(f)
"#,
    );
    let after = a.module(
        "after",
        r#"
def f():
    pass
d = {f: "unissued key"}
assert_eq(issued(f), False)
"#,
    );
    assert!(
        after
            .get("f")
            .unwrap()
            .value()
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap()
            .issued_identity()
            .is_none()
    );
    a.pass(
        r#"
load("before", old = "f", p = "p", old_dict = "d")
load("after", fresh = "f", fresh_dict = "d")
assert_eq(old == p, True)
assert_eq(p == old, True)
assert_eq(old_dict[p], "old key")
q = proxy(fresh)
assert_eq(q == fresh, True)
assert_eq(fresh == q, True)
assert_eq(fresh_dict[q], "unissued key")
assert_eq(same_hash(fresh, q), True)
assert_eq(q == p, False)
assert_eq(old == fresh, False)
garbage_collect()
assert_eq(proxy(old), p)
assert_eq(proxy(fresh), q)
"#,
    );
}

#[test]
fn identity_slot_layout_is_accounted_for() {
    let slot = size_of::<OnceLock<Arc<()>>>();
    let live = size_of::<Def<'static>>();
    let frozen = size_of::<FrozenDef>();
    eprintln!(
        "function identity layout: slot={slot}, live Def={live}, frozen Def={frozen}, pointer={}",
        size_of::<usize>()
    );
    assert!(slot >= size_of::<Arc<()>>());
    assert_eq!(live, frozen);
    // No allocation is made until the explicit issuance operation.
    let cell = OnceLock::<Arc<()>>::new();
    assert!(cell.get().is_none());
    let token = cell.get_or_init(|| Arc::new(()));
    assert_eq!(Arc::strong_count(token), 1);
}
