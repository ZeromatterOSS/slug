//! Isolated scalar controls do not mint native row-coverage carriers.
use super::*;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
fn fields(b: Budget) -> (usize, usize, usize) {
    (b.records, b.bytes, b.depth)
}
fn owner() -> PeakOwner {
    PeakOwner::FunctionOwner
}

#[test]
fn positive_complement_cost_oracle_keeps_whole_payloads_and_zero_sized_full_path() {
    assert_eq!(size_of::<Full>(), 0);
    let table = Budget {
        records: 7,
        bytes: 101,
        depth: 0,
    };
    let payload = Budget {
        records: 13,
        bytes: 211,
        depth: 0,
    };
    let base = Budget {
        records: 31,
        bytes: 43,
        depth: 2,
    };
    let mut work = Budget::default();
    let mut covered = Covered::prepare(&table, &mut work, &base, owner()).unwrap();
    assert_eq!(fields(covered.complement), fields(table));
    assert_eq!(
        fields(work),
        (3, 2 * size_of::<Covered>() + size_of::<Budget>(), 0)
    );
    covered.add_payload(&payload, &mut work, owner()).unwrap();
    let mut oracle = table;
    oracle.charge(payload.records, payload.bytes).unwrap();
    assert_eq!(fields(covered.complement), fields(oracle));
    assert_eq!(work.records, 8);
    assert_eq!(
        work.bytes,
        2 * size_of::<Covered>() + 3 * size_of::<Budget>()
    );
    covered
        .admit_state(
            owner(),
            PeakPhase::ReferenceFinal,
            &base,
            &table,
            &Budget::default(),
            &mut work,
        )
        .unwrap();
    assert_eq!(work.records, 9);
    assert_eq!(
        work.bytes,
        2 * size_of::<Covered>() + 4 * size_of::<Budget>()
    );
    let mut full_work = Budget::default();
    let mut full = Full::prepare(&table, &mut full_work, &base, owner()).unwrap();
    full.add_payload(&payload, &mut full_work, owner()).unwrap();
    full.admit_state(
        owner(),
        PeakPhase::ReferenceFinal,
        &base,
        &table,
        &oracle,
        &mut full_work,
    )
    .unwrap();
    assert_eq!(fields(full_work), fields(Budget::default()));
}

#[test]
fn covered_exact_record_byte_work_and_partial_refusals_preserve_logical_priority() {
    let table = Budget {
        records: 7,
        bytes: 101,
        depth: 0,
    };
    let payload = Budget {
        records: 13,
        bytes: 211,
        depth: 0,
    };
    let mut work = Budget::default();
    let mut covered = Covered::prepare(&table, &mut work, &Budget::default(), owner()).unwrap();
    covered.add_payload(&payload, &mut work, owner()).unwrap();
    let base = Budget {
        records: RECORD_LIMIT - 20,
        ..Budget::default()
    };
    covered
        .admit_physical(
            owner(),
            PeakPhase::ReferenceFinal,
            &base,
            &Budget::default(),
            &mut work,
        )
        .unwrap();
    let err = covered
        .admit_physical(
            owner(),
            PeakPhase::ReferenceFinal,
            &Budget {
                records: base.records + 1,
                ..base
            },
            &Budget::default(),
            &mut work,
        )
        .unwrap_err();
    assert!(err.contains(
        "kind=records-cap owner=function-owner phase=reference-final first_ledger=physical-forecast"
    ));
    let mut byte_work = Budget::default();
    let covered = Covered::prepare(&table, &mut byte_work, &Budget::default(), owner()).unwrap();
    let base = Budget {
        bytes: BYTE_LIMIT - table.bytes - byte_work.bytes - size_of::<Budget>(),
        ..Budget::default()
    };
    let mut exact = byte_work;
    covered
        .admit_physical(
            owner(),
            PeakPhase::ReferenceFinal,
            &base,
            &Budget::default(),
            &mut exact,
        )
        .unwrap();
    let mut over = byte_work;
    let err = covered
        .admit_physical(
            owner(),
            PeakPhase::ReferenceFinal,
            &Budget {
                bytes: base.bytes + 1,
                ..base
            },
            &Budget::default(),
            &mut over,
        )
        .unwrap_err();
    assert!(err.starts_with("semantic preparation joint peak:"));
    let before = fields(over);
    let logical = Budget {
        records: RECORD_LIMIT,
        ..Budget::default()
    };
    let err = covered
        .admit_state(
            owner(),
            PeakPhase::ReferenceFinal,
            &Budget {
                records: 1,
                ..Budget::default()
            },
            &logical,
            &Budget::default(),
            &mut over,
        )
        .unwrap_err();
    assert!(err.contains("first_ledger=logical-facts"));
    assert_eq!(
        fields(over),
        before,
        "logical refusal precedes additional physical-half work"
    );
    let mut exact = Budget {
        records: RECORD_LIMIT - 3,
        ..Budget::default()
    };
    Covered::prepare(&table, &mut exact, &Budget::default(), owner()).unwrap();
    assert_eq!(exact.records, RECORD_LIMIT);
    let mut over = Budget {
        records: RECORD_LIMIT - 2,
        ..Budget::default()
    };
    assert!(Covered::prepare(&table, &mut over, &Budget::default(), owner()).is_err());
    let mut work = Budget::default();
    let mut covered = Covered::prepare(
        &Budget {
            records: RECORD_LIMIT,
            ..Budget::default()
        },
        &mut work,
        &Budget::default(),
        owner(),
    )
    .unwrap();
    assert!(
        covered
            .add_payload(
                &Budget {
                    records: 1,
                    bytes: 17,
                    depth: 0
                },
                &mut work,
                owner()
            )
            .is_err()
    );
    assert_eq!(fields(covered.complement), (RECORD_LIMIT + 1, 17, 0));
}
