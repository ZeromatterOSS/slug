use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use super::*;
fn two_rows(foreign: bool) -> Rows {
    let first = rows();
    let mut second = first[0].clone();
    second.0 = "/actual/other.bzl".into();
    second.1.identity.label = CanonicalLabel::parse("@@repo+//pkg:other.bzl").unwrap();
    second.1.identity.source =
        slug_build_api_v2::ModuleSourceOrigin::Workspace("/actual/other.bzl".into());
    if foreign {
        second.1.identity.repository_mapping = second.1.identity.repository_mapping.to_vec().into();
    }
    Arc::from([first[0].clone(), second])
}
#[test]
fn repeated_mapping_backing_is_one_leaf_but_each_ordered_row_keeps_its_declaration() {
    let alias = two_rows(false);
    let foreign = two_rows(true);
    assert_eq!(alias, foreign);
    let a = certify(alias.clone(), &mut Budget::default());
    let b = certify(foreign.clone(), &mut Budget::default());
    assert_eq!(a, b);
    assert_eq!(a.mapping_pairs().len(), 1);
    assert_eq!(b.mapping_pairs().len(), 2);
    for (table, raw) in [(&a, &alias), (&b, &foreign)] {
        let (local, full, union) = super::oracle::shapes(raw);
        assert_eq!(
            super::oracle::fields(table.declarations()[0].intrinsic_shape()),
            super::oracle::fields(local)
        );
        assert_eq!(
            super::oracle::fields(table.declarations()[0].retained_shape()),
            super::oracle::fields(full)
        );
        assert_eq!(table.declarations()[0].shared_inputs().len(), 2);
        let mut audit = PublishedValueArenaBuilder::new();
        let before = audit.local_shape();
        audit
            .audit_shared_input_at_depth(&table.declarations()[0], 1)
            .unwrap();
        let actual = audit.local_shape();
        assert_eq!(actual.visits - before.visits, union.visits + 6);
        assert_eq!(actual.depth, 6);
    }
    let mut alias_audit = PublishedValueArenaBuilder::new();
    let mut foreign_audit = PublishedValueArenaBuilder::new();
    alias_audit
        .audit_shared_input_at_depth(&a.declarations()[0], 0)
        .unwrap();
    foreign_audit
        .audit_shared_input_at_depth(&b.declarations()[0], 0)
        .unwrap();
    assert!(foreign_audit.local_shape().visits > alias_audit.local_shape().visits);
    let mut changed = foreign.to_vec();
    changed[1].1.identity.repository_mapping = changed[1]
        .1
        .identity
        .repository_mapping
        .iter()
        .cloned()
        .rev()
        .collect::<Vec<_>>()
        .into();
    assert_ne!(a, certify(changed.into(), &mut Budget::default()));
}
#[test]
fn inherited_bank_is_borrowed_prepaid_and_refuses_before_any_guard_copy() {
    use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
    let raw = rows();
    let source = certify(raw.clone(), &mut Budget::default());
    let before = Arc::strong_count(&raw[0].1.identity.repository_mapping);
    let mut measured = Budget::default();
    let mut bank = mapping::bank::Bank::new(1, 1, &mut measured).unwrap();
    bank.inherit(&source.mapping_pairs()[0], &mut measured)
        .unwrap();
    drop(bank);
    assert_eq!(
        Arc::strong_count(&raw[0].1.identity.repository_mapping),
        before
    );
    for (r, b, success) in [
        (RECORD_LIMIT - measured.records, 0, true),
        (RECORD_LIMIT - measured.records + 1, 0, false),
        (0, BYTE_LIMIT - measured.bytes, true),
        (0, BYTE_LIMIT - measured.bytes + 1, false),
    ] {
        let mut work = Budget {
            records: r,
            bytes: b,
            depth: 0,
        };
        let result = mapping::bank::Bank::new(1, 1, &mut work).and_then(|mut bank| {
            bank.inherit(&source.mapping_pairs()[0], &mut work)?;
            Ok(bank)
        });
        assert_eq!(result.is_ok(), success);
        drop(result);
        assert_eq!(
            Arc::strong_count(&raw[0].1.identity.repository_mapping),
            before
        );
    }
    let mut work = Budget::default();
    let mut bounded = mapping::bank::Bank::new(0, 0, &mut work).unwrap();
    assert!(
        bounded
            .inherit(&source.mapping_pairs()[0], &mut work)
            .is_err()
    );
    assert_eq!(
        Arc::strong_count(&raw[0].1.identity.repository_mapping),
        before
    );
    let root = &source.declarations()[0];
    let mut refused = PublishedValueArenaBuilder::new();
    assert!(
        refused
            .audit_shared_input_at_depth(root, slug_build_api_v2::published_value::DEPTH_LIMIT - 4)
            .is_err()
    );
    assert!(refused.charge_visit().is_err());
}
