use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
use slug_build_api_v2::cc_variables::CcVariablesKind;
use slug_build_api_v2::cc_variables::CcVariablesOccurrence;
use slug_identity_v2::CanonicalLabel;

use super::*;

#[test]
fn variables_frozen_publication_round_trip_retains_cache_kinds_and_parent_aliases() {
    let retained = {
        let source = FrozenHeap::new();
        let parent = node(
            CcVariablesKind::Map,
            None,
            vec![("parent", AnalysisValue::string("P"))],
            vec![],
        );
        let items = AnalysisValue::list(vec![
            AnalysisValue::string("old"),
            AnalysisValue::string("new"),
        ]);
        let child = node(
            CcVariablesKind::Map,
            Some(parent.clone()),
            vec![
                ("direct", items.clone()),
                ("obj", AnalysisValue::strukt([("items", items)])),
                (
                    "unused_file",
                    AnalysisValue::artifact(AnalysisArtifact::Source(
                        CanonicalLabel::parse("@@//src:input.cc").unwrap(),
                    )),
                ),
            ],
            vec![
                (
                    "obj.items",
                    CcVariableCacheEntry::Sequence(vec![AnalysisValue::string("old")].into()),
                ),
                ("obj.missing", CcVariableCacheEntry::Missing),
                (
                    "obj.error",
                    CcVariableCacheEntry::Error("cached diagnostic".into()),
                ),
                (
                    "obj.scalar",
                    CcVariableCacheEntry::Value(AnalysisValue::string("stale")),
                ),
            ],
        );
        let graph = AnalysisValue::tuple(vec![
            AnalysisValue::cc_toolchain_variables(parent),
            AnalysisValue::cc_toolchain_variables(child.clone()),
            AnalysisValue::cc_toolchain_variables(child),
        ]);
        let value = AnalysisValueMaterializer::new(&source)
            .value(&graph)
            .unwrap();
        let retained = AnalysisValueLowerer::default()
            .lower(value.to_value(), "source")
            .unwrap();
        assert!(graph.publication_eq(&retained));
        retained
    }; // No source evaluator heap survives.
    let destination = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&destination)
        .value(&retained)
        .unwrap();
    let tuple = TupleRef::from_value(restored.to_value()).unwrap();
    let parent = slug_loading_v2::cc_variables_value(tuple.content()[0]).unwrap();
    let child = slug_loading_v2::cc_variables_value(tuple.content()[1]).unwrap();
    assert_eq!(
        child.occurrence,
        slug_loading_v2::cc_variables_value(tuple.content()[2])
            .unwrap()
            .occurrence
    );
    assert_eq!(
        parent.occurrence,
        slug_loading_v2::cc_variables_value(child.parent.unwrap())
            .unwrap()
            .occurrence
    );
    let sequence = child
        .cache
        .iter()
        .find(|(name, _)| name == "obj.items")
        .unwrap();
    assert!(
        matches!(&sequence.1,CcVariableCacheEntry::Sequence(values) if values.len()==1 && values[0].unpack_str()==Some("old"))
    );
    let twice = AnalysisValueLowerer::default()
        .lower(restored.to_value(), "restored")
        .unwrap();
    assert!(retained.publication_eq(&twice));
}

fn node(
    kind: CcVariablesKind,
    parent: Option<CcToolchainVariables>,
    locals: Vec<(&str, AnalysisValue)>,
    cache: Vec<(&str, CcVariableCacheEntry<AnalysisValue>)>,
) -> CcToolchainVariables {
    CcToolchainVariables::new(CcVariablesModel {
        kind,
        occurrence: CcVariablesOccurrence::new(),
        parent,
        locals: locals.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        cache: cache.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    })
}

#[test]
fn variables_publication_compares_kind_parent_aliases_and_unexpanded_cache_payload() {
    let parent = || {
        node(
            CcVariablesKind::Map,
            None,
            vec![("p", AnalysisValue::string("parent"))],
            vec![],
        )
    };
    let child = |p| {
        node(
            CcVariablesKind::Single,
            Some(p),
            vec![("local", AnalysisValue::string("value"))],
            vec![],
        )
    };
    let p = parent();
    let q = parent();
    assert_eq!(p, q);
    assert_ne!(child(p.clone()), child(q.clone()));
    let graph = |separate: bool| {
        let p = parent();
        let q = if separate { parent() } else { p.clone() };
        AnalysisValue::tuple(vec![
            AnalysisValue::cc_toolchain_variables(child(p)),
            AnalysisValue::cc_toolchain_variables(child(q)),
        ])
    };
    assert!(graph(false).publication_eq(&graph(false)));
    assert!(!graph(false).publication_eq(&graph(true)));
    assert!(!graph(true).publication_eq(&graph(false)));
    let cached = |text: &str| {
        node(
            CcVariablesKind::Map,
            None,
            vec![("unused", AnalysisValue::string("equal"))],
            vec![(
                "obj.field",
                CcVariableCacheEntry::Value(AnalysisValue::string(text)),
            )],
        )
    };
    let a = cached("old");
    let b = cached("new");
    assert_eq!(a, b); // Native equality deliberately ignores lookup-cache history.
    assert!(
        !AnalysisValue::cc_toolchain_variables(a)
            .publication_eq(&AnalysisValue::cc_toolchain_variables(b))
    );
    let map = node(
        CcVariablesKind::Map,
        None,
        vec![("local", AnalysisValue::string("value"))],
        vec![],
    );
    let single = node(
        CcVariablesKind::Single,
        None,
        vec![("local", AnalysisValue::string("value"))],
        vec![],
    );
    assert_ne!(map, single);
    assert!(
        !AnalysisValue::cc_toolchain_variables(map)
            .publication_eq(&AnalysisValue::cc_toolchain_variables(single))
    );
}

#[test]
fn variables_retained_hashability_and_unused_artifact_owner_are_not_path_only() {
    let owner = |label: &str| {
        slug_build_api_v2::AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            vec![1],
        )
    };
    let artifact = |label: &str| {
        AnalysisValue::artifact(AnalysisArtifact::Derived {
            owner: owner(label),
            output: slug_build_api_v2::ActionOutput::new(
                crate::analysis_value::test_artifact_root(),
                "same/output.o",
                slug_build_api_v2::ActionOutputKind::File,
            ),
        })
    };
    let vars = |value| {
        node(
            CcVariablesKind::Map,
            None,
            vec![("raw", AnalysisValue::list(vec![value]))],
            vec![],
        )
    };
    let a = AnalysisValue::cc_toolchain_variables(vars(artifact("@@//pkg:a")));
    let b = AnalysisValue::cc_toolchain_variables(vars(artifact("@@//pkg:b")));
    assert!(a.is_starlark_immutable());
    assert!(a.starlark_hash().is_ok());
    assert!(!a.publication_eq(&b));
    let single = AnalysisValue::cc_toolchain_variables(node(
        CcVariablesKind::Single,
        None,
        vec![("x", AnalysisValue::string("v"))],
        vec![],
    ));
    assert!(!single.is_starlark_immutable());
    assert!(single.starlark_hash().is_err());
    let source = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&source).value(&a).unwrap();
    assert!(restored.to_value().get_hashed().is_ok());
    assert!(
        a.publication_eq(
            &AnalysisValueLowerer::default()
                .lower(restored.to_value(), "vars")
                .unwrap()
        )
    );
}
