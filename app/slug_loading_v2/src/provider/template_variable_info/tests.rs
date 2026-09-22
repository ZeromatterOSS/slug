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
            AstModule::parse("template_variables.bzl", source.to_owned(), &Dialect::Bazel)
                .map_err(|error| error.to_string())?,
            &loading_globals(),
        )
        .map_err(|error| error.to_string())?;
    module.freeze().map_err(|error| format!("{error:?}"))
}

#[test]
fn constructor_requires_exactly_one_string_dict_with_positional_or_named_binding() {
    evaluate("a = platform_common.TemplateVariableInfo({})\nb = platform_common.TemplateVariableInfo(vars = {'A': 'one'})", None).unwrap();
    for (arguments, expected) in [
        ("", "missing required vars"),
        ("None", "must be a dict"),
        ("[]", "must be a dict"),
        ("[('A', 'one')]", "must be a dict"),
        ("'vars'", "must be a dict"),
        ("{1: 'one'}", "keys must be strings"),
        ("{'A': 1}", "values must be strings"),
        ("{}, {}", "one vars dict"),
        ("{}, vars = {}", "specified twice"),
        ("other = {}", "one vars dict"),
    ] {
        let error = evaluate(
            &format!("p = platform_common.TemplateVariableInfo({arguments})"),
            None,
        )
        .unwrap_err();
        assert!(error.contains(expected), "{arguments}: {error}");
    }
}

#[test]
fn constructor_snapshots_strings_and_exposes_ordered_fresh_mutable_dicts() {
    let escaped = TemplateVariableInfoValue(TemplateVariableInfo::new(
        [(
            CompactString::new("é\u{7f}😀"),
            CompactString::new("\"\\\r\n\t\u{7}"),
        )]
        .into_iter()
        .collect(),
    ));
    assert_eq!(
        escaped.to_string(),
        "struct(variables = {\"é\u{7f}😀\": \"\\\"\\\\\\r\\n\\t\\x07\"})"
    );
    for operation in ["p.variables = {}", "p.variables()", "p()"] {
        let source = format!("p = platform_common.TemplateVariableInfo({{}})\n{operation}");
        assert!(evaluate(&source, None).is_err(), "{operation} must reject");
    }
    let result = evaluate(
        r#"
source = {"z": "first", "é": "second"}
p = platform_common.TemplateVariableInfo(source)
source["z"] = "changed"
source["later"] = "not captured"
a = p.variables
b = p.variables
a["z"] = "mutated copy"
a["new"] = "local"
checks = [
    p.variables == {"z": "first", "é": "second"},
    b == {"z": "first", "é": "second"},
    a != b,
    p.variables.keys() == ["z", "é"],
    type(p) == "TemplateVariableInfo",
    dir(p) == ["variables"],
    hasattr(p, "variables"),
    not hasattr(p, "unknown"),
    getattr(p, "unknown", "fallback") == "fallback",
    repr(p) == 'struct(variables = {"z": "first", "é": "second"})',
    repr(platform_common.TemplateVariableInfo({})) == "struct(variables = {})",
]
"#,
        None,
    )
    .unwrap();
    let checks = result.get("checks").unwrap();
    for (index, value) in ListRef::from_value(checks.value())
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            value.unpack_bool(),
            Some(true),
            "check {index}; provider repr: {}",
            result.get("p").unwrap().value().to_repr()
        );
    }
    let p = result.get("p").unwrap();
    assert!(starlark_structural_is_immutable(p.value()));
    let retained = template_variable_info(p.value()).unwrap();
    assert_eq!(retained.variables().get("z").unwrap(), "first");
}

#[test]
fn occurrence_equality_hash_and_native_constructor_key_are_preserved() {
    let result = evaluate(
        r#"
key = platform_common.TemplateVariableInfo
another_key = platform_common.TemplateVariableInfo
p = key({"A": "one"})
alias = p
q = key({"A": "one"})
by_provider = {p: "first", q: "second"}
by_key = {key: "constructor"}
checks = [
    p == alias,
    p != q,
    len(by_provider) == 2,
    by_provider[alias] == "first",
    depset([p, alias, q]).to_list() == [p, q],
    key == another_key,
    by_key[another_key] == "constructor",
    type(key) == "Provider",
    repr(key) == "<function TemplateVariableInfo>",
]
"#,
        None,
    )
    .unwrap();
    let checks = result.get("checks").unwrap();
    assert!(
        ListRef::from_value(checks.value())
            .unwrap()
            .iter()
            .all(|value| value.unpack_bool() == Some(true))
    );
    let key = result.get("key").unwrap();
    let other = result.get("another_key").unwrap();
    assert_eq!(
        key.value().get_hashed().unwrap().hash(),
        other.value().get_hashed().unwrap().hash()
    );
}

#[test]
fn frozen_provider_reads_remain_mutable_and_frozen_constructor_mints_new_occurrence() {
    let producer = evaluate(
        "key = platform_common.TemplateVariableInfo\np = key({'A': 'one'})\nsource = {'B': 'two'}",
        None,
    )
    .unwrap();
    let original = producer.get("p").unwrap();
    let identity = template_variable_info(original.value()).unwrap();
    let consumer = evaluate(
        r#"
copy = p.variables
forwarded = p
copy["A"] = "consumer only"
q = key(vars = source)
fresh = key({"A": "one"})
checks = [p.variables == {"A": "one"}, q.variables == {"B": "two"}, p != fresh]
"#,
        Some(&producer),
    )
    .unwrap();
    let checks = consumer.get("checks").unwrap();
    assert!(
        ListRef::from_value(checks.value())
            .unwrap()
            .iter()
            .all(|value| value.unpack_bool() == Some(true))
    );
    assert_eq!(
        identity,
        template_variable_info(consumer.get("forwarded").unwrap().value()).unwrap()
    );
    let heap = FrozenHeap::new();
    let restored = alloc_frozen_template_variable_info(&heap, identity.clone());
    assert_eq!(
        identity,
        template_variable_info(restored.to_value()).unwrap()
    );
}
