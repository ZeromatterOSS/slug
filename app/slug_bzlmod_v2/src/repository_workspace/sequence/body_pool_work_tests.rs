//! Actual pure transcript completion, with no native custody/FD claim.
use slug_identity_v2::CanonicalLabel;

use super::*;

const BYTES: usize = 128 * 1024;
fn root() -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory).unwrap()
}
fn file(body: Arc<[u8]>) -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new("out".into(), 0o644, RepositoryWorkspaceNode::File(body)).unwrap()
}
fn initial() -> RepositoryWorkspaceTranscript {
    let reservation = RepositoryWorkspaceReservation::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        Arc::new(HostSelectedExtensionOwner::testing("body-pool")),
        CanonicalRepoName::new("body+repo").unwrap(),
        0,
        Arc::new(RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
                rule_name: "repository".into(),
            },
            attributes: Arc::new(SmallMap::new()),
        }),
        NormalizedAbsolutePath::new("/logical/body").unwrap(),
        PathObservationInstanceId::new(31),
        7,
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    RepositoryWorkspaceTranscript::new(Arc::new(reservation), 0o755).unwrap()
}
fn write(t: &RepositoryWorkspaceTranscript, body: Arc<[u8]>) -> RepositoryWorkspaceTranscript {
    t.complete(
        t.request(RepositoryWorkspaceOperation::File {
            path: "out".into(),
            body: body.clone(),
            mode: 0o644,
        })
        .unwrap(),
        RepositoryWorkspaceResult::Unit,
        RepositoryWorkspaceMode::Archive,
        vec![root(), file(body)],
    )
    .unwrap()
}
fn body(state: &RepositoryWorkspaceState) -> &Arc<[u8]> {
    let RepositoryWorkspaceNode::File(body) = state.entry("out").unwrap().node() else {
        panic!("regular fixture body")
    };
    body
}

#[test]
fn actual_unchanged_exists_and_read_completion_avoid_canonical_body_hashes() {
    let first = write(&initial(), vec![7; BYTES].into());
    for read in [false, true] {
        let operation = if read {
            RepositoryWorkspaceOperation::Read("out".into())
        } else {
            RepositoryWorkspaceOperation::Exists("out".into())
        };
        let result = if read {
            RepositoryWorkspaceResult::Read(body(first.current()).clone())
        } else {
            RepositoryWorkspaceResult::Exists(true)
        };
        let request = first.request(operation).unwrap();
        body_pool_work::reset();
        let completed = first
            .complete(
                request,
                result,
                first.current().mode(),
                first.current().entries().to_vec(),
            )
            .unwrap();
        let work = body_pool_work::snapshot();
        assert_eq!(work.overflow_mask, 0);
        assert_eq!(
            work.calls[0], 1,
            "old pool construction still hashes its one retained allocation"
        );
        assert_eq!(work.payload_bytes[0], BYTES as u64);
        assert_eq!(
            work.payload_bytes[1], 0,
            "canonical identical Arc needs no full byte-key lookup"
        );
        assert_eq!(work.calls[1], 0);
        assert!(Arc::ptr_eq(
            body(completed.current()),
            body(first.current())
        ));
        assert_eq!(completed.retention.body_bytes, BYTES as u64);
        assert_eq!(completed.records.len(), 2);
    }
}

#[test]
fn raw_equal_distinct_initial_and_history_edges_select_actual_first_pool_key() {
    for with_initial in [false, true] {
        let mut old = write(&initial(), vec![7; BYTES].into());
        let original = body(old.current()).clone();
        let foreign: Arc<[u8]> = Arc::from(original.as_ref());
        let first: Arc<[u8]> = Arc::from(original.as_ref());
        assert!(!Arc::ptr_eq(&original, &foreign));
        if with_initial {
            let state = Arc::new(RepositoryWorkspaceState::from_admitted(
                old.reservation.clone(),
                RepositoryWorkspaceMode::Archive,
                vec![root(), file(first.clone())],
            ));
            old.retention.metadata_bytes = old
                .retention
                .metadata_bytes
                .checked_add(
                    state.retained_metadata_bytes().unwrap()
                        - old.initial.retained_metadata_bytes().unwrap(),
                )
                .unwrap();
            old.initial = state;
        }
        // Deliberately private structural fixture: the original byte-key pool
        // accepts equal-distinct old edges and retains its FIRST key owner.
        let mut record = old.records[0].as_ref().clone();
        record.request.predecessor = old.initial.clone();
        record.next = Arc::new(RepositoryWorkspaceState::from_admitted(
            old.reservation.clone(),
            RepositoryWorkspaceMode::Archive,
            vec![root(), file(foreign.clone())],
        ));
        old.records = Arc::from([Arc::new(record)]);
        let expected = if with_initial { &first } else { &foreign };
        body_pool_work::reset();
        let completed = old
            .complete(
                old.request(RepositoryWorkspaceOperation::Exists("out".into()))
                    .unwrap(),
                RepositoryWorkspaceResult::Exists(true),
                RepositoryWorkspaceMode::Archive,
                vec![root(), file(original.clone())],
            )
            .unwrap();
        let work = body_pool_work::snapshot();
        assert_eq!(work.calls[0], if with_initial { 3 } else { 2 });
        assert!(
            work.payload_bytes[1] >= BYTES as u64,
            "noncanonical historical allocation must not be a fast-path witness"
        );
        assert!(Arc::ptr_eq(body(completed.current()), expected));
        assert!(!Arc::ptr_eq(body(completed.current()), &original));
        assert_eq!(completed.retention.body_bytes, BYTES as u64);
    }
}

#[test]
fn incoming_foreign_equal_reuses_key_and_unequal_bytes_keep_full_history() {
    let old = write(&initial(), vec![7; BYTES].into());
    let foreign: Arc<[u8]> = Arc::from(body(old.current()).as_ref());
    let weak = Arc::downgrade(&foreign);
    body_pool_work::reset();
    let equal = write(&old, foreign);
    assert!(body_pool_work::snapshot().payload_bytes[1] >= BYTES as u64);
    assert!(Arc::ptr_eq(body(equal.current()), body(old.current())));
    assert!(weak.upgrade().is_none());
    assert_eq!(equal.retention.body_bytes, BYTES as u64);
    let mut changed = vec![7; BYTES];
    changed[BYTES - 1] = 8;
    body_pool_work::reset();
    let changed = write(&equal, changed.into());
    assert!(body_pool_work::snapshot().payload_bytes[1] >= BYTES as u64);
    assert!(!Arc::ptr_eq(body(changed.current()), body(equal.current())));
    assert_ne!(
        body(changed.current()).as_ref(),
        body(equal.current()).as_ref()
    );
    assert_eq!(changed.retention.body_bytes, (2 * BYTES) as u64);
    assert!(Arc::ptr_eq(&changed.records[0], &old.records[0]));
}

#[test]
fn invalid_result_precedes_all_pool_hash_work_and_counter_overflow_is_explicit() {
    let old = write(&initial(), vec![7; BYTES].into());
    body_pool_work::reset();
    assert_eq!(
        old.complete(
            old.request(RepositoryWorkspaceOperation::Exists("out".into()))
                .unwrap(),
            RepositoryWorkspaceResult::Exists(false),
            old.current().mode(),
            old.current().entries().to_vec()
        ),
        Err(RepositoryWorkspaceError::InvalidResult)
    );
    assert_eq!(body_pool_work::snapshot(), body_pool_work::Work::default());
    assert_eq!(old.records.len(), 1);
    // Overflow checks are exercised without processing arbitrary payload data.
    body_pool_work::record(body_pool_work::Kind::Seed, usize::MAX);
    body_pool_work::record(body_pool_work::Kind::Seed, usize::MAX);
    let work = body_pool_work::snapshot();
    if usize::BITS == 64 {
        assert_ne!(work.overflow_mask & 4, 0);
        assert_eq!(work.payload_bytes[0], u64::MAX);
    }
    assert_eq!(work.calls[0], 2);
}
