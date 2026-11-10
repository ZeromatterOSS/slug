//! Pure logical range-work and lifetime controls, not native IO authority.
use super::construction_workspace_tests::build;
use super::construction_workspace_tests::closed_semantics;
use super::construction_workspace_tests::fresh_selected;
use super::construction_workspace_tests::noise;
use super::range_comparison_work_for_test as work;
use super::*;

fn mixed() -> Vec<u8> {
    let mut bytes = vec![b'a'; CHUNK];
    bytes.extend(noise(CHUNK));
    bytes.extend(vec![b'z'; 37]);
    bytes
}
fn check_closed(body: &RepositoryWorkspaceBody, bytes: &[u8]) {
    let admissions = bytes
        .chunks(CHUNK)
        .map(|raw| {
            let (_, selected) = fresh_selected(raw);
            arc_bytes(selected.len()).unwrap()
        })
        .collect::<Vec<_>>();
    closed_semantics(body, bytes, &admissions);
}
#[test]
fn multi_block_equality_preserves_closed_bytes_and_decode_work_before_one_init_gate() {
    let bytes = mixed();
    let left = build(&bytes);
    let right = build(&bytes);
    let witness = work::begin();
    assert_eq!(left, right);
    let count = witness.snapshot();
    drop(witness);
    assert_eq!(
        (count.range_calls, count.range_bytes),
        (3, bytes.len() as u64)
    );
    assert_eq!(
        (count.decode_calls, count.decode_bytes),
        (2, (CHUNK + 37) as u64)
    );
    check_closed(&left, &bytes);
    check_closed(&right, &bytes);
    let weak_left = left.downgrade();
    let weak_right = right.downgrade();
    drop(left);
    drop(right);
    assert!(weak_left.upgrade().is_none() && weak_right.upgrade().is_none());
    assert_eq!(
        (count.initialization_calls, count.initialization_bytes),
        (1, CHUNK as u64)
    );
}

#[test]
fn prepared_ranges_redecode_repeats_and_preserve_bounds_before_one_init_gate() {
    let bytes = mixed();
    let body = build(&bytes);
    let mut changed = bytes[..7].to_vec();
    changed[0] ^= 1;
    let queries = [
        (0, bytes[..CHUNK].to_vec(), true),
        (CHUNK - 3, bytes[CHUNK - 3..CHUNK + 6].to_vec(), true),
        (0, bytes[..7].to_vec(), true),
        (0, changed, false),
        (
            2 * CHUNK + 2,
            bytes[2 * CHUNK + 2..2 * CHUNK + 13].to_vec(),
            true,
        ),
        (bytes.len(), Vec::new(), true),
        (bytes.len() + 1, Vec::new(), false),
        (usize::MAX, vec![0], false),
        (CHUNK, bytes[CHUNK..2 * CHUNK].to_vec(), true),
    ];
    let witness = work::begin();
    body.with_range_comparison(|compare| {
        for (offset, request, expected) in &queries {
            assert_eq!(compare(*offset, request), *expected);
        }
    });
    let count = witness.snapshot();
    drop(witness);
    assert_eq!(
        (count.range_calls, count.range_bytes),
        (7, (2 * CHUNK + 34) as u64)
    );
    // Repeated same-block calls ALWAYS decode it again, including the mismatch.
    assert_eq!(
        (count.decode_calls, count.decode_bytes),
        (5, (4 * CHUNK + 37) as u64)
    );
    check_closed(&body, &bytes);
    let weak = body.downgrade();
    drop(body);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        (count.initialization_calls, count.initialization_bytes),
        (1, CHUNK as u64)
    );
}

#[test]
fn raw_empty_invalid_and_preparation_policy_keep_their_distinct_work() {
    let raw = RepositoryWorkspaceBody::from_arc(Arc::from(&b"raw"[..]));
    let empty = build(&[]);
    let body = build(b"packed");
    let same = body.clone();
    let shorter = build(b"short");
    let witness = work::begin();
    raw.with_range_comparison(|compare| {
        assert!(compare(0, b"raw"));
        assert!(!compare(usize::MAX, b"x"));
    });
    empty.with_range_comparison(|_| ());
    assert!(!body.range_eq(usize::MAX, b"x"));
    assert!(!body.range_eq(body.len() + 1, &[]));
    assert_eq!(same, body);
    assert_ne!(shorter, body);
    assert_eq!(witness.snapshot(), work::Snapshot::default());
    drop(witness);

    // Opt-in nonempty Packed preparation may eagerly initialize even when
    // no valid range reaches the kernel. Baseline forwarding does zero work.
    for invalid_only in [false, true] {
        let witness = work::begin();
        body.with_range_comparison(|compare| {
            if invalid_only {
                assert!(!compare(usize::MAX, b"x"));
            }
        });
        let count = witness.snapshot();
        drop(witness);
        assert!(count.initialization_calls <= 1);
        assert_eq!(
            count.initialization_bytes,
            count.initialization_calls * CHUNK as u64
        );
        assert_eq!(
            (
                count.range_calls,
                count.range_bytes,
                count.decode_calls,
                count.decode_bytes
            ),
            (0, 0, 0, 0)
        );
    }
    // The OLD ordinary valid empty Packed range still initializes its block.
    let witness = work::begin();
    assert!(empty.range_eq(0, &[]));
    let count = witness.snapshot();
    drop(witness);
    assert_eq!(
        (count.initialization_calls, count.initialization_bytes),
        (1, CHUNK as u64)
    );
    assert_eq!(
        (
            count.range_calls,
            count.range_bytes,
            count.decode_calls,
            count.decode_bytes
        ),
        (1, 0, 0, 0)
    );
    let witness = work::begin();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        body.with_range_comparison(|compare| {
            assert!(compare(0, b"packed"));
            panic!("range callback unwind");
        });
    }));
    assert!(caught.is_err());
    drop(witness);
    let fresh = work::begin();
    assert_eq!(fresh.snapshot(), work::Snapshot::default());
}
