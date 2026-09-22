use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::AnalysisArtifact;
use crate::AnalysisDepset;
use crate::DepsetOrder;
use crate::ProviderId;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::cc_variables::CcVariablesKind;
use crate::cc_variables::CcVariablesModel;
use crate::cc_variables::CcVariablesOccurrence;

fn variables(
    parent: Option<CcToolchainVariables>,
    locals: Vec<(&str, AnalysisValue)>,
    cache: Vec<(&str, CcVariableCacheEntry<AnalysisValue>)>,
) -> CcToolchainVariables {
    CcToolchainVariables::new(CcVariablesModel {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent,
        locals: locals.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        cache: cache.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    })
}
fn s(value: &str) -> AnalysisValue {
    AnalysisValue::string(value)
}
fn object(value: &str) -> AnalysisValue {
    AnalysisValue::strukt([("field", s(value))])
}
fn string(access: &mut OwnedCcVariableAccess<'_>, name: &str) -> String {
    let value = access.lookup(name, true).unwrap().unwrap();
    access.string(&value, name).unwrap().to_string()
}
fn sequence(access: &mut OwnedCcVariableAccess<'_>, name: &str) -> Vec<String> {
    let value = access.lookup(name, true).unwrap().unwrap();
    access
        .sequence(&value, name)
        .unwrap()
        .map(|v| access.string(&v.unwrap(), name).unwrap().to_string())
        .collect()
}

#[test]
fn original_seed_and_fresh_overlay_are_distinct_and_immutable() {
    let current = AnalysisValue::list(vec![s("old"), s("new")]);
    let vars = variables(
        None,
        vec![
            ("obj", AnalysisValue::strukt([("items", current.clone())])),
            ("direct", current),
        ],
        vec![
            (
                "obj.items",
                CcVariableCacheEntry::Sequence(vec![s("old")].into()),
            ),
            ("obj.field", CcVariableCacheEntry::Value(s("cached"))),
        ],
    );
    let before = AnalysisValue::cc_toolchain_variables(vars.clone());
    let occurrence = vars.occurrence().pointer();
    let mut original = OwnedCcVariableAccess::original(&vars);
    assert_eq!(sequence(&mut original, "obj.items"), ["old"]);
    assert_eq!(string(&mut original, "obj.field"), "cached");
    for _ in 0..2 {
        let mut fresh = OwnedCcVariableAccess::fresh_overlay(&vars);
        assert_eq!(sequence(&mut fresh, "obj.items"), ["old", "new"]);
        assert!(fresh.lookup("obj.field", false).unwrap().is_none());
        assert_eq!(sequence(&mut fresh, "direct"), ["old", "new"]);
    }
    assert_eq!(sequence(&mut original, "direct"), ["old", "new"]);
    assert_eq!(vars.occurrence().pointer(), occurrence);
    assert_eq!(vars.model().cache.len(), 2);
    assert!(before.publication_eq(&AnalysisValue::cc_toolchain_variables(vars)));
}

#[test]
fn longest_prefix_none_shadowing_and_parent_cache_rules() {
    let parent = variables(
        None,
        vec![("x", s("parent")), ("p", object("live"))],
        vec![("p.field", CcVariableCacheEntry::Value(s("parent cache")))],
    );
    let vars = variables(
        Some(parent),
        vec![
            ("x", AnalysisValue::none()),
            ("a", AnalysisValue::strukt([("b", object("short"))])),
            ("a.b", object("long")),
            ("a.b.literal", s("literal")),
        ],
        vec![("a.b.literal", CcVariableCacheEntry::Value(s("wrong cache")))],
    );
    let mut access = OwnedCcVariableAccess::original(&vars);
    assert!(access.lookup("x", false).unwrap().is_none());
    assert_eq!(string(&mut access, "a.b.field"), "long");
    assert_eq!(string(&mut access, "a.b.literal"), "literal");
    assert_eq!(string(&mut access, "p.field"), "live");
}

#[test]
fn iteration_scopes_own_independent_dotted_caches() {
    let vars = variables(
        None,
        vec![("obj", object("root"))],
        vec![("obj.field", CcVariableCacheEntry::Value(s("seed")))],
    );
    let mut root = OwnedCcVariableAccess::original(&vars);
    for name in ["first", "second"] {
        let mut child = root.with_binding("obj", adapt(&object(name)).unwrap());
        assert_eq!(string(&mut child, "obj.field"), name);
        let mut clone = child.clone();
        assert_eq!(string(&mut clone, "obj.field"), name);
        let mut nested = child.with_binding("obj", adapt(&object("nested")).unwrap());
        assert_eq!(string(&mut nested, "obj.field"), "nested");
        assert_eq!(string(&mut child, "obj.field"), name);
    }
    assert_eq!(string(&mut root, "obj.field"), "seed");
    assert_eq!(vars.model().cache.len(), 1);
}

#[test]
fn sequence_conversion_is_lazy_and_preserves_container_order() {
    let depset =
        AnalysisDepset::new(DepsetOrder::Default, vec![s("b"), s("a"), s("b")], vec![]).unwrap();
    let vars = variables(
        None,
        vec![
            (
                "bad",
                AnalysisValue::list(vec![s("ok"), AnalysisValue::integer(7)]),
            ),
            ("tuple", AnalysisValue::tuple(vec![s("b"), s("a")])),
            (
                "dict",
                AnalysisValue::dictionary([
                    (s("b"), AnalysisValue::integer(9)),
                    (s("a"), AnalysisValue::none()),
                ])
                .unwrap(),
            ),
            ("depset", AnalysisValue::depset(depset)),
        ],
        vec![(
            "cached.items",
            CcVariableCacheEntry::Sequence(vec![s("ok"), AnalysisValue::integer(7)].into()),
        )],
    );
    let mut access = OwnedCcVariableAccess::original(&vars);
    for name in ["bad", "cached.items"] {
        let value = access.lookup(name, true).unwrap().unwrap();
        let mut values = access.sequence(&value, name).unwrap();
        assert_eq!(
            access
                .string(&values.next().unwrap().unwrap(), name)
                .unwrap(),
            "ok"
        );
        assert!(
            values
                .next()
                .unwrap()
                .err()
                .unwrap()
                .to_string()
                .contains("unsupported C++ variable value of type int")
        );
        assert!(values.next().is_none());
    }
    for name in ["tuple", "dict", "depset"] {
        assert_eq!(sequence(&mut access, name), ["b", "a"]);
    }
}

#[test]
fn scalar_and_structure_admission_matches_live_adapter() {
    let file = AnalysisValue::artifact(AnalysisArtifact::Source(
        CanonicalLabel::parse("@@repo+//pkg:header.h").unwrap(),
    ));
    let user = ProviderIdentity::user(ProviderId::unqualified("Info").unwrap());
    let vars = variables(
        None,
        vec![
            ("yes", AnalysisValue::boolean(true)),
            ("no", AnalysisValue::boolean(false)),
            ("empty", s("")),
            ("file", file),
            (
                "user",
                AnalysisValue::provider(ProviderOccurrence::new(user, [("field", s("user"))])),
            ),
            (
                "toolchain",
                AnalysisValue::provider(ProviderOccurrence::new(
                    ProviderIdentity::builtin("ToolchainInfo"),
                    [("field", s("toolchain"))],
                )),
            ),
            (
                "default",
                AnalysisValue::provider(ProviderOccurrence::empty(ProviderIdentity::builtin(
                    "DefaultInfo",
                ))),
            ),
            (
                "bad",
                AnalysisValue::strukt([("field", AnalysisValue::integer(42))]),
            ),
        ],
        vec![],
    );
    let mut access = OwnedCcVariableAccess::original(&vars);
    assert_eq!(string(&mut access, "yes"), "1");
    assert_eq!(string(&mut access, "no"), "0");
    assert_eq!(string(&mut access, "file"), "external/repo+/pkg/header.h");
    assert_eq!(string(&mut access, "user.field"), "user");
    assert_eq!(string(&mut access, "toolchain.field"), "toolchain");
    for (name, expected) in [
        ("yes", true),
        ("no", false),
        ("empty", false),
        ("file", true),
        ("user", true),
    ] {
        let value = access.lookup(name, true).unwrap().unwrap();
        assert_eq!(access.truthy(&value).unwrap(), expected);
    }
    for name in ["default", "bad.field"] {
        assert!(
            access
                .lookup(name, false)
                .err()
                .unwrap()
                .to_string()
                .contains("unsupported C++ variable value")
        );
    }
}

#[test]
fn missing_and_error_seeds_preserve_failure_order_without_publication_mutation() {
    let vars = variables(
        None,
        vec![("obj", object("live")), ("scalar", s("not structure"))],
        vec![
            ("obj.field", CcVariableCacheEntry::Missing),
            (
                "obj.error",
                CcVariableCacheEntry::Error("saved failure".into()),
            ),
        ],
    );
    let mut access = OwnedCcVariableAccess::original(&vars);
    assert!(access.lookup("obj.field", false).unwrap().is_none());
    assert!(
        access
            .lookup("obj.field", true)
            .err()
            .unwrap()
            .to_string()
            .contains("Cannot find variable")
    );
    assert!(access.lookup("obj.error", false).unwrap().is_none());
    assert_eq!(
        access.lookup("obj.error", true).err().unwrap().0,
        "saved failure"
    );
    assert!(access.lookup("scalar.field", false).unwrap().is_none());
    assert!(
        access
            .lookup("scalar.field", true)
            .err()
            .unwrap()
            .to_string()
            .contains("Cannot find variable")
    );
    let mut fresh = OwnedCcVariableAccess::fresh_overlay(&vars);
    assert_eq!(string(&mut fresh, "obj.field"), "live");
    assert!(
        fresh
            .lookup("scalar.field", true)
            .err()
            .unwrap()
            .to_string()
            .contains("expected structure")
    );
    assert!(fresh.lookup("scalar.field", false).unwrap().is_none());
    assert_eq!(vars.model().cache.len(), 2);
}
