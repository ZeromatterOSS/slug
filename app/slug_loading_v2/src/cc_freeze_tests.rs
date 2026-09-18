use starlark::values::dict::DictRef;
use starlark::values::list::FrozenListRef;
use starlark::values::list::ListRef;

use super::*;

fn owner() -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules_cc+//cc/private:freeze_test.bzl").unwrap(),
        workspace_path: PathBuf::from("/rules_cc/cc/private/freeze_test.bzl"),
        repository_mapping: Arc::from([]),
    }
}

#[test]
fn shallow_immutable_collections_preserve_aliases_operations_and_freeze() {
    let source = r#"
freeze = cc_common.internal_DO_NOT_USE().freeze
nested = [1]
original = [nested, 2]
xs = freeze(original)
ys = freeze((3, 4))
d = freeze({"b": nested, "a": 2})
original.append(9)
nested.append(7)
asserts = [
    type(xs) == "list", type(d) == "dict", xs == [[1, 7], 2],
    xs[0] == d["b"], xs[-1] == 2, xs[1:] == [2], xs.index(2) == 1,
    [v for v in ys] == [3, 4], xs + ys == [[1, 7], 2, 3, 4],
    2 * ys == [3, 4, 3, 4], ys * 0 == [],
    d.keys() == ["b", "a"], d.items() == [("b", [1, 7]), ("a", 2)],
    d.values() == [[1, 7], 2], d.get("z", 5) == 5,
    dict(d) == d, (d | {"a": 3}) == {"b": [1, 7], "a": 3},
    ({"z": 1} | d).keys() == ["z", "b", "a"],
]
CHECKS = False not in asserts
copy = list(ys)
copy.append(5)
dcopy = dict(d)
dcopy["c"] = 8
n = None
n_copy = freeze(n)
P = provider(fields = ["field"])
p = P(field = ys)
p_copy = freeze(p)
cycle = []
cycle_copy = freeze([cycle])
cycle.append(cycle_copy)
"#;
    // Shallow immutable values are evaluator-owned until module freeze.
    let identity = owner();
    let context = BzlEvaluationContext::from_manifest(&BzlLoadManifest {
        root: identity.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([crate::BzlModuleSourceProvenance::new(identity, [0; 32])]),
        fingerprint: [0; 32],
    });
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(
            "/rules_cc/cc/private/freeze_test.bzl",
            source.to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    assert_eq!(module.get("CHECKS").unwrap().unpack_bool(), Some(true));
    assert!(FrozenListRef::from_value(module.get("xs").unwrap()).is_none());
    assert!(ListRef::is_immutable(module.get("xs").unwrap()));
    assert!(DictRef::is_immutable(module.get("d").unwrap()));
    assert_eq!(
        module.get("p").unwrap().identity(),
        module.get("p_copy").unwrap().identity()
    );
    assert_eq!(
        module.get("n").unwrap().identity(),
        module.get("n_copy").unwrap().identity()
    );
    // Module freeze is covered here; the separate Assert harness below forces GC.
    eval.eval_module(
        AstModule::parse(
            "after_reeval.bzl",
            "AFTER_REEVAL = xs == [[1, 7], 2] and d['b'] == xs[0] and ys == [3, 4]\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    let module = module.freeze().unwrap();
    assert_eq!(
        module.get("AFTER_REEVAL").unwrap().unpack_bool(),
        Some(true)
    );
    let xs = module.get("xs").unwrap();
    let d = module.get("d").unwrap();
    assert!(FrozenListRef::from_value(xs.value()).is_some());
    assert!(xs.value().get_hashed().is_err());
    assert!(d.value().get_hashed().is_err());
    assert_eq!(
        module.get("cycle_copy").unwrap().value().to_repr(),
        "[[[...]]]"
    );
}

#[test]
fn shallow_immutable_collections_reject_all_mutators_and_direct_hashes() {
    for statement in [
        "xs.append(3)",
        "xs.clear()",
        "xs.extend([3])",
        "xs.insert(0, 3)",
        "xs.pop()",
        "xs.remove(1)",
        "xs[0] = 3",
        "xs += [3]",
        "d.clear()",
        "d.pop('a')",
        "d.popitem()",
        "d.setdefault('z', 3)",
        "d.update({'z': 3})",
        "d['a'] = 3",
        "d |= {'z': 3}",
        "bad = {xs: 1}",
        "bad = {d: 1}",
        "bad = depset([xs])",
        "bad = depset([d])",
    ] {
        let source = format!(
            "freeze = cc_common.internal_DO_NOT_USE().freeze\nxs = freeze([1, 2])\nd = freeze({{'a': 1}})\ndef run(xs, d):\n    {statement}\nrun(xs, d)\n"
        );
        let error = eval_bzl_with_identity(&source, owner())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Immutable")
                || error.contains("hashable")
                || error.contains("immutable"),
            "{statement}: {error}"
        );
    }
}

#[test]
fn shallow_freeze_rejects_unadmitted_iterables_and_java_nonvalue_scalars() {
    for expr in ["range(3)", "set([1, 2])"] {
        let source = format!("x = cc_common.internal_DO_NOT_USE().freeze({expr})\n");
        let error = eval_bzl_with_identity(&source, owner())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("conversion is not supported"),
            "{expr}: {error}"
        );
    }
    for expr in ["True", "'abc'"] {
        let source = format!("x = cc_common.internal_DO_NOT_USE().freeze({expr})\n");
        let error = eval_bzl_with_identity(&source, owner())
            .unwrap_err()
            .to_string();
        assert!(error.contains("not string or bool"), "{expr}: {error}");
    }
}

#[test]
fn shallow_immutable_collections_preserve_provider_hash_and_reject_mutable_children() {
    let source = r#"
freeze = cc_common.internal_DO_NOT_USE().freeze
P = provider(fields = ["xs", "d"])
a = P(xs = freeze([1, 2]), d = freeze({"a": 1, "b": 2}))
b = P(xs = freeze([1, 2]), d = freeze({"b": 2, "a": 1}))
DS = depset([a, b])
OK = a == b and len(DS.to_list()) == 1
"#;
    let module = eval_bzl_with_identity(source, owner()).unwrap();
    assert_eq!(module.get("OK").unwrap().unpack_bool(), Some(true));
    assert_symmetric_equal_and_hash(
        module.get("a").unwrap().value(),
        module.get("b").unwrap().value(),
    );
    for expression in ["freeze([[1]])", "freeze({'a': [1]})"] {
        let source = format!(
            "freeze = cc_common.internal_DO_NOT_USE().freeze\nP = provider(fields = ['value'])\nx = P(value = {expression})\nbad = depset([x])\n"
        );
        assert!(
            eval_bzl_with_identity(&source, owner()).is_err(),
            "{expression}"
        );
    }
}

#[starlark::starlark_module]
fn immutable_test_globals(builder: &mut starlark::environment::GlobalsBuilder) {
    fn frozen_copy<'v>(
        value: starlark::values::Value<'v>,
        heap: starlark::values::Heap<'v>,
    ) -> anyhow::Result<starlark::values::Value<'v>> {
        if let Some(xs) = ListRef::from_value(value) {
            Ok(heap.alloc(starlark::values::list::AllocImmutableList(xs.iter())))
        } else if let Some(d) = DictRef::from_value(value) {
            Ok(heap.alloc(starlark::values::dict::AllocImmutableDict(d.iter())))
        } else {
            anyhow::bail!("test expects list or dict")
        }
    }
}

#[test]
fn shallow_immutable_collections_survive_forced_gc() {
    let mut assert = starlark::assert::Assert::new();
    assert.globals_add(immutable_test_globals);
    // Assert runs this with automatic, disabled, and forced-at-every-statement GC.
    assert.is_true(
        r#"
nested = [1]
xs = frozen_copy([nested, 2])
d = frozen_copy({"nested": nested, "xs": xs})
nested.append(3)
xs == [[1, 3], 2] and d["xs"] == xs and d["nested"] == nested
"#,
    );
}

#[test]
fn empty_header_info_native_identity_survives_module_freeze() {
    let source = r#"
internal = cc_common.internal_DO_NOT_USE()
a = internal.create_header_info()
alias = a
b = internal.create_header_info()
P = provider(fields = ["header"])
x = P(header = a)
y = P(header = b)
OK = a == alias and a != b and len(depset([a, alias, b]).to_list()) == 2 and len(depset([x, y, x]).to_list()) == 2
"#;
    let module = eval_bzl_with_identity(source, owner()).unwrap();
    assert_eq!(module.get("OK").unwrap().unpack_bool(), Some(true));
    let a = module.get("a").unwrap();
    let alias = module.get("alias").unwrap();
    let b = module.get("b").unwrap();
    assert_symmetric_equal_and_hash(a.value(), alias.value());
    assert!(!a.value().equals(b.value()).unwrap());
    assert!(a.value().get_hashed().is_ok());
    assert!(
        eval_bzl_with_identity(
            "x = cc_common.internal_DO_NOT_USE().create_header_info(modular_public_headers = [1])",
            owner()
        )
        .is_err()
    );
}
