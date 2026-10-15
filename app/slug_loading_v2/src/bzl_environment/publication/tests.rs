use std::cell::Cell;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;

fn body() -> PublicationShape {
    PublicationShape {
        visits: 1,
        ..PublicationShape::default()
    }
}
#[test]
fn positive_metadata_preserves_expanded_full_without_expanded_policy_replay() {
    let child = PublicationShape {
        visits: RECORD_LIMIT,
        bytes: 7,
        depth: 2,
        ..body()
    };
    let mut work = Budget::default();
    let prepared = PreparedPublication::prepare(body(), 2, [child, child].into_iter(), |r, b| {
        work.charge(r, b)
    })
    .unwrap();
    assert_eq!(prepared.storage.visits, 3);
    assert_eq!(prepared.intrinsic.visits, 4);
    assert_eq!(prepared.full.visits, 4 + 2 * RECORD_LIMIT);
    assert_eq!(prepared.full.depth, 3);
    assert!(work.records < RECORD_LIMIT);
    assert!(work.bytes < BYTE_LIMIT);
    assert!(
        PreparedPublication::prepare(body(), usize::MAX, std::iter::empty(), |_, _| Ok(()))
            .is_err()
    );
    for child in [
        PublicationShape {
            visits: usize::MAX,
            ..body()
        },
        PublicationShape {
            depth: 256,
            ..body()
        },
    ] {
        assert!(
            PreparedPublication::prepare(body(), 1, [child].into_iter(), |_, _| Ok(())).is_err()
        );
    }
}
#[test]
fn metadata_work_and_array_preflight_stop_before_child_reads_or_guard_copies() {
    for bytes in [false, true] {
        let reads = Cell::new(0);
        let mut work = Budget::default();
        if bytes {
            work.bytes = BYTE_LIMIT;
        } else {
            work.records = RECORD_LIMIT;
        }
        assert!(
            PreparedPublication::prepare(
                body(),
                1,
                std::iter::once_with(|| {
                    reads.set(reads.get() + 1);
                    body()
                }),
                |r, b| work.charge(r, b)
            )
            .is_err()
        );
        assert_eq!(reads.get(), 0);
    }
    let mut exact = Budget::default();
    PreparedPublication::prepare(body(), 1, [body()].into_iter(), |r, b| exact.charge(r, b))
        .unwrap();
    for plus in [0, 1] {
        let mut work = Budget {
            records: RECORD_LIMIT - exact.records + plus,
            ..Budget::default()
        };
        assert_eq!(
            PreparedPublication::prepare(body(), 1, [body()].into_iter(), |r, b| work.charge(r, b))
                .is_ok(),
            plus == 0
        );
        let mut work = Budget {
            bytes: BYTE_LIMIT - exact.bytes + plus,
            ..Budget::default()
        };
        assert_eq!(
            PreparedPublication::prepare(body(), 1, [body()].into_iter(), |r, b| work.charge(r, b))
                .is_ok(),
            plus == 0
        );
    }
}
