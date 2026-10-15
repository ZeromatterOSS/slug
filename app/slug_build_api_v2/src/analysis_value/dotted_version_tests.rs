use std::convert::Infallible;

use super::*;
use crate::PublishedValueArenaBuilder;
use crate::PublishedValueError;
use crate::published_value::BYTE_LIMIT;

fn value(input: &str) -> AnalysisValue {
    let model = crate::DottedVersion::prepare(input, || Ok::<_, Infallible>(()))
        .unwrap()
        .into_owned();
    AnalysisValue::dotted_version(model)
}

fn shape(value: &AnalysisValue) -> crate::PublicationShape {
    let mut b = PublishedValueArenaBuilder::new();
    b.audit_native(value).unwrap();
    b.local_shape()
}

#[test]
fn dotted_version_language_keys_and_full_publication_spelling_are_distinct() {
    let a = value("1");
    let b = value("01.00.internal.☃");
    let restored = value("1");
    assert_eq!(a.value_type(), AnalysisValueType::DottedVersion);
    assert_eq!(a.value_type().as_str(), "DottedVersion");
    assert!(matches!(a.kind(), AnalysisValueKind::DottedVersion(_)));
    assert!(a.is_starlark_hashable() && a.is_starlark_immutable() && a.is_valid_depset_leaf());
    assert_eq!(a, b);
    assert_eq!(a.starlark_hash().unwrap(), b.starlark_hash().unwrap());
    assert_ne!(a, AnalysisValue::string("1"));
    assert!(!a.publication_eq(&b));
    assert!(a.publication_eq(&restored));
    assert!(matches!(
        AnalysisValue::dictionary([
            (a.clone(), AnalysisValue::none()),
            (b.clone(), AnalysisValue::none())
        ]),
        Err(AnalysisValueError::DuplicateDictionaryKey)
    ));
    let first = AnalysisValue::dictionary([(a, AnalysisValue::integer(1))]).unwrap();
    let changed = AnalysisValue::dictionary([(b, AnalysisValue::integer(1))]).unwrap();
    let again = AnalysisValue::dictionary([(restored, AnalysisValue::integer(1))]).unwrap();
    assert_eq!(first, changed);
    assert!(!first.publication_eq(&changed));
    assert!(first.publication_eq(&again));
}

#[test]
fn dotted_version_audit_includes_full_ignored_suffix_and_last_component() {
    // Both ignored suffixes exceed CompactString's minimum allocation. Their
    // exact full byte delta must remain present despite language equality.
    let prefix = "1.internal.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
    let a = value(prefix);
    let b = value(&(prefix.to_owned() + "☃"));
    let base = shape(&a);
    let full = shape(&b);
    assert_eq!(full.bytes - base.bytes, "☃".len());
    assert_eq!(full.visits, base.visits);
    assert_eq!(full.edges, base.edges);
    assert_eq!(a, b);
    assert!(!a.publication_eq(&b));
    let last = value("1.2a3.4b5");
    let last_shape = shape(&last);
    assert_eq!(last_shape.visits - shape(&value("1.2a3")).visits, 1);
    assert_eq!(last_shape.edges - shape(&value("1.2a3")).edges, 1);
    assert!(!last.publication_eq(&value("1.2a3.4b6")));
    let AnalysisValueKind::DottedVersion(model) = last.kind() else {
        unreachable!()
    };
    assert_eq!(
        last_shape.bytes,
        std::mem::size_of::<AnalysisValue>()
            + published_projection_forecast()
            + model.storage().retained_bytes()
    );
    assert_eq!(last_shape.visits, 2 + model.components().len());
    assert_eq!(last_shape.edges, 1 + model.components().len());
    assert_eq!(last_shape.depth, 2);
}

#[test]
fn dotted_version_refusal_precedes_borrowed_atom_clone_and_poisons_reuse() {
    let small = value("1.2a3");
    let full = value("1.2a3.4b5.internal.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz");
    let needed = shape(&full).bytes + 2 * std::mem::size_of::<usize>();
    let before = Arc::strong_count(&full.0);
    let mut exact = PublishedValueArenaBuilder::new();
    exact.charge_bytes(BYTE_LIMIT - needed).unwrap();
    let retained = exact.native_atom_borrowed(&full).unwrap();
    assert_eq!(exact.charged_bytes(), BYTE_LIMIT);
    assert_eq!(Arc::strong_count(&full.0), before + 1);
    drop(retained);
    assert_eq!(Arc::strong_count(&full.0), before);
    let mut limited = PublishedValueArenaBuilder::new();
    limited.charge_bytes(BYTE_LIMIT - needed + 1).unwrap();
    assert!(matches!(
        limited.native_atom_borrowed(&full),
        Err(PublishedValueError::ByteBudget(_))
    ));
    assert_eq!(Arc::strong_count(&full.0), before);
    assert!(limited.charge_bytes(0).is_err());
    let mut control = PublishedValueArenaBuilder::new();
    control.charge_bytes(BYTE_LIMIT - needed + 1).unwrap();
    assert!(control.native_atom_borrowed(&small).is_ok());
    let mut no_work = PublishedValueArenaBuilder::new();
    for _ in 0..crate::published_value::NODE_LIMIT - 1 {
        no_work.charge_visit().unwrap();
    }
    assert!(no_work.native_atom_borrowed(&full).is_err());
    assert_eq!(Arc::strong_count(&full.0), before);
    assert!(no_work.charge_bytes(0).is_err());
}

#[test]
fn dotted_version_args_native_budget_keeps_full_model_payload() {
    let a = value("1.internal.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz");
    let b = value("1.internal.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz☃");
    let mut base = crate::args_mapper::NativeArgsBudget::default();
    base.audit(&a).unwrap();
    let mut full = crate::args_mapper::NativeArgsBudget::default();
    full.audit(&b).unwrap();
    assert_eq!(full.bytes() - base.bytes(), "☃".len());
    let AnalysisValueKind::DottedVersion(model) = b.kind() else {
        unreachable!()
    };
    assert_eq!(
        full.bytes(),
        std::mem::size_of::<AnalysisValue>()
            + published_projection_forecast()
            + model.storage().retained_bytes()
    );
}

#[test]
fn dotted_version_public_projection_rejects_changed_root_and_native_child_spelling() {
    use crate::published_value::PublishedEdge;
    use crate::published_value::PublishedNode;
    use crate::published_value::PublishedOriginalBinding;
    use crate::published_value::PublishedRootSlot;
    use crate::published_value::PublishedValueArena;
    #[derive(Debug)]
    struct Originals;
    impl PublishedOriginalBinding for Originals {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn root_count(&self) -> usize {
            2
        }
        fn imported_arenas(&self) -> &[PublishedValueArena] {
            &[]
        }
    }
    let mut builder = PublishedValueArenaBuilder::new();
    let native_id = builder.reserve().unwrap();
    let tuple_id = builder.reserve().unwrap();
    let atom = builder.native_atom(value("1")).unwrap();
    builder
        .set_node(native_id, PublishedNode::Native(atom))
        .unwrap();
    let children = builder
        .append_edges(&[PublishedEdge::Local(native_id)])
        .unwrap();
    builder
        .set_node(tuple_id, PublishedNode::Tuple(children))
        .unwrap();
    for (id, slot) in [(native_id, 0), (tuple_id, 1)] {
        builder
            .set_original_slot(id, PublishedRootSlot::new(slot).unwrap())
            .unwrap();
    }
    let arena = builder.seal(Arc::new(Originals)).unwrap();
    let native_origin = arena.value(native_id).unwrap();
    let tuple_origin = arena.value(tuple_id).unwrap();
    for (spelling, accepted) in [("1", true), ("1.0", false), ("1", true)] {
        let raw = value(spelling);
        assert_eq!(raw, value("1"), "all cases are language-equal");
        let root = AnalysisValue::with_original_origin(raw.clone(), native_origin.clone());
        assert_eq!(root.is_ok(), accepted, "native root spelling {spelling}");
        // Keep the child raw: this exercises the actual Native child fallback,
        // rather than the already-bound same-original shortcut.
        assert!(raw.original_origin().is_none());
        let child = AnalysisValue::with_original_origin(
            AnalysisValue::tuple(vec![raw]),
            tuple_origin.clone(),
        );
        assert_eq!(
            child.is_ok(),
            accepted,
            "native tuple child spelling {spelling}"
        );
    }
}
