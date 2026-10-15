use starlark_map::small_map::SmallMap;

use super::peak_context::FirstLedger;
use super::peak_context::PeakContext;
use super::peak_context::PeakOwner;
use super::peak_context::PeakPhase;
use super::*;

const CONTEXT: PeakContext = PeakContext::new(
    PeakOwner::SemanticInputs,
    PeakPhase::RowIndexGrowth,
    FirstLedger::Logical,
);

#[test]
fn aggregate_peak_admits_exact_boundary_and_refuses_overflow_without_changing_logical_credit() {
    let logical = Budget {
        bytes: BYTE_LIMIT - 4096,
        records: 17,
        depth: 2,
    };
    let scratch = Budget {
        bytes: 4096,
        records: 11,
        depth: 0,
    };
    admit_peak(CONTEXT, logical, scratch, 0).unwrap();
    let error = admit_peak(CONTEXT, logical, scratch, 1).unwrap_err();
    assert_eq!(
        error,
        format!(
            "semantic preparation joint peak: {} (logical_records=17 logical_bytes={} scratch_records=11 scratch_bytes=4096 extra_bytes=1 owner=semantic-inputs phase=row-index-growth first_ledger=logical-facts)",
            limit(),
            BYTE_LIMIT - 4096,
        )
    );
    assert_eq!(
        admit_peak(CONTEXT, logical, scratch, usize::MAX).unwrap_err(),
        format!(
            "{} (operation=joint-peak kind=extra-overflow owner=semantic-inputs phase=row-index-growth first_ledger=logical-facts logical_records=17 logical_bytes={} scratch_records=11 scratch_bytes=4096 extra_bytes={} base_total={BYTE_LIMIT} extra_total=overflow byte_limit={BYTE_LIMIT})",
            limit(),
            BYTE_LIMIT - 4096,
            usize::MAX,
        )
    );
    assert_eq!(
        (logical.bytes, logical.records, logical.depth),
        (BYTE_LIMIT - 4096, 17, 2)
    );
}

#[test]
fn aggregate_refusal_precedes_index_candidate_and_borrowed_vector_growth() {
    let logical = Budget {
        bytes: BYTE_LIMIT - 2048,
        ..Budget::default()
    };
    let mut scratch = Budget::default();
    let mut index = SmallMap::<u64, Vec<usize>>::new();
    let mut borrowed = Vec::<usize>::new();
    let before = (index.len(), borrowed.len(), borrowed.capacity());
    let admitted = admit_index_growth::<usize>(CONTEXT, logical, &mut scratch, borrowed.len(), 0);
    // Each independent account fits, but two 1KiB nodes plus Vec storage do not.
    assert!(admitted.is_err());
    let error = admitted.as_ref().unwrap_err();
    assert!(
        error.contains("owner=semantic-inputs phase=row-index-growth first_ledger=logical-facts")
    );
    if admitted.is_ok() {
        index.entry(0).or_default().push(borrowed.len());
        borrowed.push(1);
    }
    assert_eq!((index.len(), borrowed.len(), borrowed.capacity()), before);
    assert_eq!((scratch.bytes, scratch.records), (0, 0));
    assert_eq!(logical.bytes, BYTE_LIMIT - 2048);

    let sufficient = Budget {
        bytes: BYTE_LIMIT - 4096,
        ..logical
    };
    admit_index_growth::<usize>(CONTEXT, sufficient, &mut scratch, 0, 0).unwrap();
    index.entry(0).or_default().push(borrowed.len());
    borrowed.push(1);
    assert_eq!((index.len(), borrowed.len()), (1, 1));
}

#[test]
fn peak_context_inventory_is_closed_bounded_and_never_adds_a_joint_record_limit() {
    use super::peak_context::MAX_LABEL_BYTES;
    for name in PeakOwner::ALL
        .iter()
        .map(|v| v.name())
        .chain(PeakPhase::ALL.iter().map(|v| v.name()))
        .chain(FirstLedger::ALL.iter().map(|v| v.name()))
    {
        assert!(!name.is_empty());
        assert!(name.len() <= MAX_LABEL_BYTES);
        assert!(name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
    }
    let first = Budget {
        records: RECORD_LIMIT,
        ..Default::default()
    };
    let second = Budget {
        records: RECORD_LIMIT,
        ..Default::default()
    };
    admit_peak(CONTEXT, first, second, 0).unwrap();
    assert_eq!(
        (first.records, second.records),
        (RECORD_LIMIT, RECORD_LIMIT)
    );
    for ledger in FirstLedger::ALL {
        let context = PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::ReferenceDescriptorCompare,
            *ledger,
        );
        let first = Budget {
            bytes: BYTE_LIMIT,
            ..first
        };
        let second = Budget { bytes: 1, ..second };
        let error = admit_peak(context, first, second, 0).unwrap_err();
        assert!(error.contains("owner=function-owner phase=reference-descriptor-compare"));
        assert!(error.ends_with(&format!("first_ledger={})", ledger.name())));
        assert!(error.contains("logical_records=65536"));
        assert!(error.contains("scratch_records=65536"));
    }
}
