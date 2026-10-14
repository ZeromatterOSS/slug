use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::package::loading_globals;

fn evaluate(source: &str, imported: Option<&FrozenModule>) -> Result<FrozenModule, String> {
    let module = Module::new();
    if let Some(imported) = imported {
        module.import_public_symbols(imported);
    }
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse("run_environment.bzl", source.to_owned(), &Dialect::Bazel)
                .map_err(|error| error.to_string())?,
            &loading_globals(),
        )
        .map_err(|error| error.to_string())?;
    module.freeze().map_err(|error| format!("{error:?}"))
}

fn assert_checks(module: &FrozenModule) {
    let checks = module.get("checks").unwrap();
    for (index, value) in ListRef::from_value(checks.value())
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(value.unpack_bool(), Some(true), "check {index}");
    }
}

#[test]
fn constructor_binds_defaults_positional_named_and_expanded_arguments() {
    // RunEnvironmentInfoApi.java83–117 and Sequence.cast: RangeList is also a
    // Sequence, so every empty range is admitted without any integer elements.
    let module = evaluate(
        r#"
a = RunEnvironmentInfo()
b = RunEnvironmentInfo({"A": "one"})
c = RunEnvironmentInfo({"A": "one"}, ["PATH", "PATH"])
d = RunEnvironmentInfo(inherited_environment = ("PATH", "PATH"), environment = {"A": "one"})
e = RunEnvironmentInfo(*[{"A": "one"}], **{"inherited_environment": ("PATH", "PATH")})
f = RunEnvironmentInfo(inherited_environment = ["ONLY"])
g = RunEnvironmentInfo({}, range(0))
h = RunEnvironmentInfo(inherited_environment = range(4, 1))
i = RunEnvironmentInfo(inherited_environment = range(3, 3))
j = RunEnvironmentInfo(inherited_environment = range(1, 4, -1))
checks = [
    a.environment == {}, a.inherited_environment == [],
    b.environment == {"A": "one"}, b.inherited_environment == [],
    c == d, d == e, f.environment == {}, f.inherited_environment == ["ONLY"],
    g == a, h == a, i == a, j == a,
    type(c) == "RunEnvironmentInfo", type(d.inherited_environment) == "list",
]
"#,
        None,
    )
    .unwrap();
    assert_checks(&module);
    for (arguments, expected) in [
        ("{}, [], []", "at most two positional"),
        ("{}, environment = {}", "environment specified twice"),
        (
            "{}, [], inherited_environment = []",
            "inherited_environment specified twice",
        ),
        ("unknown = {}", "unexpected argument"),
        ("None", "environment must be a dict"),
        ("[]", "environment must be a dict"),
        ("[('A', 'one')]", "environment must be a dict"),
        ("{1: 'one'}", "environment keys must be strings"),
        ("{'A': None}", "environment values must be strings"),
        ("{}, None", "must be a list, tuple, or empty range"),
        ("{}, 'PATH'", "must be a list, tuple, or empty range"),
        ("{}, ''", "must be a list, tuple, or empty range"),
        ("{}, {}", "must be a list, tuple, or empty range"),
        ("{}, set()", "must be a list, tuple, or empty range"),
        ("{}, [1]", "elements must be strings"),
        ("{}, ('PATH', False)", "elements must be strings"),
        ("{}, range(1)", "elements must be strings"),
        ("{}, range(3, 0, -1)", "elements must be strings"),
    ] {
        let error = evaluate(&format!("p = RunEnvironmentInfo({arguments})"), None).unwrap_err();
        assert!(error.contains(expected), "{arguments}: {error}");
    }
}

#[test]
fn raw_snapshot_and_fresh_getters_preserve_order_duplicates_and_mutation_isolation() {
    let module = evaluate(
        r#"
source = {"z": "first", "é": "second", "😀": "third"}
inherited = ["PATH", "PATH", "z"]
p = RunEnvironmentInfo(source, inherited)
alias = p
source["z"] = "input mutation"
source["new"] = "not retained"
inherited.clear()
first = p.environment
second = p.environment
names = p.inherited_environment
other_names = p.inherited_environment
first.clear()
first["local"] = "local"
names.append("local")
names[0] = "local"
checks = [
    p.environment == {"z": "first", "é": "second", "😀": "third"},
    second == alias.environment,
    p.environment.keys() == ["z", "é", "😀"],
    p.inherited_environment == ["PATH", "PATH", "z"],
    other_names == alias.inherited_environment,
    dir(p) == ["environment", "inherited_environment"],
    hasattr(p, "environment"), not hasattr(p, "unknown"),
    getattr(p, "unknown", "fallback") == "fallback",
    repr(p) == 'struct(environment = {"z": "first", "é": "second", "😀": "third"}, inherited_environment = ["PATH", "PATH", "z"])',
]
"#,
        None,
    ).unwrap();
    assert_checks(&module);
    let p = module.get("p").unwrap();
    assert!(starlark_structural_is_immutable(p.value()));
    let retained = run_environment_info_ref(p.value()).unwrap();
    assert_eq!(
        retained
            .environment()
            .keys()
            .map(CompactString::as_str)
            .collect::<Vec<_>>(),
        ["z", "é", "😀"]
    );
    assert_eq!(
        retained
            .inherited_environment()
            .iter()
            .map(CompactString::as_str)
            .collect::<Vec<_>>(),
        ["PATH", "PATH", "z"]
    );
    for operation in [
        "p.environment = {}",
        "p.inherited_environment = []",
        "p.environment()",
        "p()",
    ] {
        let error = evaluate(&format!("p = RunEnvironmentInfo()\n{operation}"), None);
        assert!(error.is_err(), "{operation} must reject");
    }
}

#[test]
fn native_field_equality_hash_ignore_dict_order_but_preserve_inherited_sequence() {
    // NativeInfo.java98–117 compares/hashes public fields, not construction
    // occurrence. The native publication model separately preserves dict order.
    let module = evaluate(
        r#"
p = RunEnvironmentInfo({"z": "last", "a": "first"}, ["PATH", "z", "PATH"])
q = RunEnvironmentInfo({"a": "first", "z": "last"}, ("PATH", "z", "PATH"))
different_order = RunEnvironmentInfo(p.environment, ["PATH", "PATH", "z"])
different_duplicates = RunEnvironmentInfo(p.environment, ["PATH", "z"])
different_value = RunEnvironmentInfo({"z": "changed", "a": "first"}, p.inherited_environment)
table = {p: "first"}
table[q] = "second"
key = RunEnvironmentInfo
by_constructor = {key: "constructor"}
copy = p.environment
copy["z"] = "local mutation"
checks = [
    p == q, len(table) == 1, table[p] == "second", table[q] == "second",
    p != different_order, p != different_duplicates, p != different_value,
    RunEnvironmentInfo() == RunEnvironmentInfo(),
    p.environment.keys() != q.environment.keys(),
    key == RunEnvironmentInfo, by_constructor[RunEnvironmentInfo] == "constructor",
    repr(key) == "<function RunEnvironmentInfo>", type(key) == "Provider",
    depset([p, q]).to_list() == [p],
]
"#,
        None,
    )
    .unwrap();
    assert_checks(&module);
    let p = module.get("p").unwrap();
    let q = module.get("q").unwrap();
    assert_eq!(
        p.value().get_hashed().unwrap().hash(),
        q.value().get_hashed().unwrap().hash()
    );
    assert_ne!(
        run_environment_info(p.value()).unwrap(),
        run_environment_info(q.value()).unwrap()
    );
    assert_eq!(
        configured_target_provider_identity(module.get("key").unwrap().value()).unwrap(),
        Some(ProviderIdentity::builtin("RunEnvironmentInfo"))
    );
    assert_eq!(
        configured_target_provider_identity(p.value()).unwrap(),
        None
    );
}

#[test]
fn frozen_native_roundtrip_and_nested_consumers_keep_fresh_mutable_fields_after_drop() {
    let producer = evaluate(
        "key = RunEnvironmentInfo\nsource = {'z': 'first', 'a': 'second'}\ninherited = ['PATH', 'PATH']\np = key(source, inherited)\ntable = {p: 'retained'}\nsource.clear()\ninherited.clear()\nbox = struct(nested = [p])",
        None,
    ).unwrap();
    let snapshot = run_environment_info(producer.get("p").unwrap().value()).unwrap();
    let consumer = evaluate(
        r#"
retained_table = table
forwarded = box.nested[0]
copy = forwarded.environment
copy["z"] = "consumer mutation"
names = forwarded.inherited_environment
names.clear()
minted = key(inherited_environment = ("NEW",))
checks = [
    forwarded.environment == {"z": "first", "a": "second"},
    forwarded.inherited_environment == ["PATH", "PATH"],
    minted.environment == {}, minted.inherited_environment == ["NEW"],
    table[forwarded] == "retained",
]
"#,
        Some(&producer),
    )
    .unwrap();
    drop(producer);
    assert_checks(&consumer);
    let last = evaluate(
        "fresh = forwarded.environment\nfresh.clear()\nchecks = [forwarded.environment.keys() == ['z', 'a'], forwarded.inherited_environment == ['PATH', 'PATH'], retained_table[forwarded] == 'retained']",
        Some(&consumer),
    ).unwrap();
    drop(consumer);
    assert_checks(&last);
    let heap = FrozenHeap::new();
    let restored = alloc_frozen_run_environment_info(&heap, snapshot.clone());
    assert_eq!(run_environment_info(restored.to_value()).unwrap(), snapshot);
    let module = Module::new();
    let first = restored
        .to_value()
        .get_attr_error("environment", module.heap())
        .unwrap();
    let second = restored
        .to_value()
        .get_attr_error("environment", module.heap())
        .unwrap();
    assert!(!first.ptr_eq(second));
    let first = restored
        .to_value()
        .get_attr_error("inherited_environment", module.heap())
        .unwrap();
    let second = restored
        .to_value()
        .get_attr_error("inherited_environment", module.heap())
        .unwrap();
    assert!(!first.ptr_eq(second));
}
