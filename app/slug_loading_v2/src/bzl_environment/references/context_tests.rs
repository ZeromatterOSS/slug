use super::*;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;

#[test]
fn combined_precharge_reports_actual_ledger_before_peak_and_preserves_first_failure() {
    let owner = PeakOwner::SemanticInputs;
    let phase = PeakPhase::ReferenceDescriptorCompare;
    let base = Budget {
        records: RECORD_LIMIT,
        bytes: 19,
        depth: 3,
    };
    let logical = Budget {
        records: 1,
        bytes: 7,
        depth: 0,
    };
    let physical = Budget {
        records: 2,
        bytes: BYTE_LIMIT,
        depth: 0,
    };
    let work = Budget {
        records: RECORD_LIMIT,
        bytes: BYTE_LIMIT,
        depth: 0,
    };
    let mut expected = base;
    let error = expected
        .charge_with_context(
            PeakContext::new(owner, phase, FirstLedger::Logical),
            logical.records,
            logical.bytes,
        )
        .unwrap_err();
    assert_eq!(
        admit_state(owner, phase, base, logical, physical, work).unwrap_err(),
        error
    );
    assert!(error.contains("kind=records-cap owner=semantic-inputs phase=reference-descriptor-compare first_ledger=logical-facts"));
    let base = Budget {
        records: RECORD_LIMIT - 1,
        bytes: 19,
        depth: 3,
    };
    let physical = Budget {
        records: 2,
        bytes: 7,
        depth: 0,
    };
    let mut expected = base;
    let error = expected
        .charge_with_context(
            PeakContext::new(owner, phase, FirstLedger::Physical),
            physical.records,
            physical.bytes,
        )
        .unwrap_err();
    assert_eq!(
        admit_state(owner, phase, base, logical, physical, Budget::default()).unwrap_err(),
        error
    );
    assert!(error.contains("first_ledger=physical-forecast"));
    assert_eq!(
        (base.records, base.bytes, base.depth),
        (RECORD_LIMIT - 1, 19, 3)
    );
}

#[test]
fn reference_table_context_refuses_before_guard_materialization() {
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let before = Arc::strong_count(&env.inputs);
    let reads = std::cell::Cell::new(0);
    let hashes = std::cell::Cell::new(0);
    let inputs = [&env.inputs]
        .into_iter()
        .inspect(|_| reads.set(reads.get() + 1));
    let work = Budget {
        records: RECORD_LIMIT,
        ..Budget::default()
    };
    let mut expected = work;
    let error = expected
        .charge_with_context(
            PeakContext::new(
                PeakOwner::BuiltinPreparation,
                PeakPhase::ReferenceTables,
                FirstLedger::Work,
            ),
            1,
            0,
        )
        .unwrap_err();
    assert_eq!(
        EnvironmentReferencePlan::prepare_indexed(
            inputs,
            |h| {
                hashes.set(hashes.get() + 1);
                h
            },
            work,
            Budget::default(),
            PeakOwner::BuiltinPreparation,
        )
        .unwrap_err(),
        error
    );
    assert_eq!(
        reads.get(),
        1,
        "the borrowed input is read for the paid count step"
    );
    assert_eq!(hashes.get(), 0);
    assert_eq!(Arc::strong_count(&env.inputs), before);
}
