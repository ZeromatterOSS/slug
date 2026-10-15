use super::*;
#[test]
fn equal_foreign_tables_stay_distinct_and_raw_guard_lives_until_last_declaration() {
    let raw = rows();
    let foreign: Rows = raw.iter().cloned().collect::<Vec<_>>().into();
    let mut work = Budget::default();
    let table = certify(raw.clone(), &mut work);
    let other = certify(foreign, &mut Budget::default());
    assert_eq!(table, other);
    assert!(!Arc::ptr_eq(table.raw(), other.raw()));
    let weak = Arc::downgrade(&raw);
    let descriptor = table.declarations()[0].clone();
    let capture = table.capture(&mut work).unwrap();
    drop(table);
    drop(raw);
    assert!(weak.upgrade().is_some());
    drop(capture);
    assert!(
        weak.upgrade().is_some(),
        "the pure guard owns its exact table"
    );
    drop(descriptor);
    assert!(weak.upgrade().is_none());
    let mut changed = other.raw().to_vec();
    changed[0].1.source_digest[31] ^= 1;
    assert_ne!(other, certify(changed.into(), &mut Budget::default()));
}
