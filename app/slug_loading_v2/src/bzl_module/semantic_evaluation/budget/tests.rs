use starlark_map::small_map::SmallMap;

use super::*;

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
    admit_peak(logical, scratch, 0).unwrap();
    assert!(admit_peak(logical, scratch, 1).is_err());
    assert!(admit_peak(logical, scratch, usize::MAX).is_err());
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
    let admitted = admit_index_growth::<usize>(logical, &mut scratch, borrowed.len(), 0);
    // Each independent account fits, but two 1KiB nodes plus Vec storage do not.
    assert!(admitted.is_err());
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
    admit_index_growth::<usize>(sufficient, &mut scratch, 0, 0).unwrap();
    index.entry(0).or_default().push(borrowed.len());
    borrowed.push(1);
    assert_eq!((index.len(), borrowed.len()), (1, 1));
}
