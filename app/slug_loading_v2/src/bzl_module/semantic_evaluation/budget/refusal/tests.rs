use super::super::peak_context::FirstLedger;
use super::super::peak_context::PeakOwner;
use super::super::peak_context::PeakPhase;
use super::super::*;

const CONTEXT: PeakContext = PeakContext::new(
    PeakOwner::SemanticInputs,
    PeakPhase::InputValidation,
    FirstLedger::Validation,
);
fn state(b: Budget) -> (usize, usize, usize) {
    (b.records, b.bytes, b.depth)
}

#[test]
fn contextual_charge_preserves_legacy_state_and_exact_failure_precedence() {
    let cases = [
        (0, 0, RECORD_LIMIT, BYTE_LIMIT, None),
        (0, 0, RECORD_LIMIT + 1, 0, Some("records-cap")),
        (0, 0, 0, BYTE_LIMIT + 1, Some("bytes-cap")),
        (usize::MAX, 7, 1, usize::MAX, Some("records-overflow")),
        (2, usize::MAX, 3, 1, Some("bytes-overflow")),
        (RECORD_LIMIT, usize::MAX, 1, 1, Some("bytes-overflow")),
        (0, 0, RECORD_LIMIT + 1, BYTE_LIMIT + 1, Some("records-cap")),
        (RECORD_LIMIT + 1, 0, 0, 0, Some("records-cap")),
        (0, BYTE_LIMIT + 1, 0, 0, Some("bytes-cap")),
    ];
    for (records, bytes, requested_records, requested_bytes, kind) in cases {
        let before = Budget {
            records,
            bytes,
            depth: 23,
        };
        let mut legacy = before;
        let mut contextual = before;
        let old = legacy.charge(requested_records, requested_bytes);
        let new = contextual.charge_with_context(CONTEXT, requested_records, requested_bytes);
        assert_eq!(old.is_ok(), new.is_ok());
        assert_eq!(state(legacy), state(contextual));
        if let Some(kind) = kind {
            assert_eq!(old.unwrap_err(), limit());
            let error = new.unwrap_err();
            assert!(error.starts_with(&format!("{} (operation=charge kind={kind} ", limit())));
            assert!(
                error.contains(
                    "owner=semantic-inputs phase=input-validation first_ledger=validation"
                )
            );
            assert!(error.contains(&format!(
                "after_records={} after_bytes={} after_depth=23",
                legacy.records, legacy.bytes
            )));
            if kind == "records-overflow" {
                assert!(error.contains("record_total=overflow byte_total=not-reached"));
                assert_eq!(state(contextual), state(before));
            } else if kind == "bytes-overflow" {
                assert!(error.contains(&format!(
                    "record_total={} byte_total=overflow",
                    legacy.records
                )));
                assert_eq!(contextual.bytes, before.bytes);
            }
        }
    }
}

#[test]
fn contextual_charge_reports_exact_before_request_and_committed_totals() {
    let mut budget = Budget {
        records: 9,
        bytes: BYTE_LIMIT,
        depth: 4,
    };
    assert_eq!(
        budget.charge_with_context(CONTEXT, 2, 1).unwrap_err(),
        format!(
            "{} (operation=charge kind=bytes-cap owner=semantic-inputs phase=input-validation first_ledger=validation before_records=9 before_bytes={BYTE_LIMIT} before_depth=4 requested_records=2 requested_bytes=1 record_total=11 byte_total={} after_records=11 after_bytes={} after_depth=4 record_limit={RECORD_LIMIT} byte_limit={BYTE_LIMIT} depth_limit={DEPTH_LIMIT})",
            limit(),
            BYTE_LIMIT + 1,
            BYTE_LIMIT + 1,
        )
    );
    assert_eq!(state(budget), (11, BYTE_LIMIT + 1, 4));
}

#[test]
fn contextual_depth_preserves_max_mutation_legacy_text_and_budget_layout() {
    assert_eq!(
        std::mem::size_of::<Budget>(),
        3 * std::mem::size_of::<usize>()
    );
    for (old, requested) in [
        (0, 0),
        (19, 3),
        (0, DEPTH_LIMIT),
        (0, DEPTH_LIMIT + 1),
        (usize::MAX, 0),
    ] {
        let mut legacy = Budget {
            records: 13,
            bytes: 17,
            depth: old,
        };
        let mut contextual = legacy;
        let a = legacy.depth(requested);
        let b = contextual.depth_with_context(CONTEXT, requested);
        assert_eq!(a.is_ok(), b.is_ok());
        assert_eq!(state(legacy), state(contextual));
        assert_eq!(contextual.depth, old.max(requested));
        if a.is_err() {
            assert_eq!(a.unwrap_err(), limit());
            let error = b.unwrap_err();
            assert!(error.contains("operation=depth kind=depth-cap"));
            assert!(error.contains(&format!("requested_depth={requested}")));
            assert!(error.ends_with(&format!(
                "after_records=13 after_bytes=17 after_depth={} depth_limit={DEPTH_LIMIT})",
                old.max(requested)
            )));
        }
    }
}

mod helpers;
