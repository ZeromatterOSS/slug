//! Actual pure transcript completion, with no native custody/FD claim.
use slug_identity_v2::CanonicalLabel;

use super::*;

const BYTES: usize = 128 * 1024;
fn root() -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory).unwrap()
}
fn file(body: RepositoryWorkspaceBody) -> RepositoryWorkspaceEntry {
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
fn write(
    t: &RepositoryWorkspaceTranscript,
    body: RepositoryWorkspaceBody,
) -> RepositoryWorkspaceTranscript {
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
fn body(state: &RepositoryWorkspaceState) -> &RepositoryWorkspaceBody {
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
            "old pool construction routes its one retained allocation using the cached word"
        );
        assert_eq!(work.payload_bytes[0], 0);
        assert_eq!(
            work.payload_bytes[1], 0,
            "canonical identical Arc needs no full byte-key lookup"
        );
        assert_eq!(work.calls[1], 0);
        assert!(RepositoryWorkspaceBody::ptr_eq(
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
        let foreign: RepositoryWorkspaceBody =
            RepositoryWorkspaceBody::from_arc(Arc::from(original.raw_arc().unwrap().as_ref()));
        let first: RepositoryWorkspaceBody =
            RepositoryWorkspaceBody::from_arc(Arc::from(original.raw_arc().unwrap().as_ref()));
        assert!(!original.ptr_eq(&foreign));
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
            work.calls[1] >= 1,
            "noncanonical historical allocation must not be a fast-path witness"
        );
        assert!(RepositoryWorkspaceBody::ptr_eq(
            body(completed.current()),
            expected
        ));
        assert!(!RepositoryWorkspaceBody::ptr_eq(
            body(completed.current()),
            &original
        ));
        assert_eq!(completed.retention.body_bytes, BYTES as u64);
    }
}

#[test]
fn incoming_foreign_equal_reuses_key_and_unequal_bytes_keep_full_history() {
    let old = write(&initial(), vec![7; BYTES].into());
    let foreign: RepositoryWorkspaceBody = RepositoryWorkspaceBody::from_arc(Arc::from(
        body(old.current()).raw_arc().unwrap().as_ref(),
    ));
    let weak = foreign.downgrade();
    body_pool_work::reset();
    let equal = write(&old, foreign);
    assert!(body_pool_work::snapshot().calls[1] >= 1);
    assert!(RepositoryWorkspaceBody::ptr_eq(
        body(equal.current()),
        body(old.current())
    ));
    assert!(weak.upgrade().is_none());
    assert_eq!(equal.retention.body_bytes, BYTES as u64);
    let mut changed = vec![7; BYTES];
    changed[BYTES - 1] = 8;
    body_pool_work::reset();
    let changed = write(&equal, changed.into());
    assert!(body_pool_work::snapshot().calls[1] >= 1);
    assert!(!RepositoryWorkspaceBody::ptr_eq(
        body(changed.current()),
        body(equal.current())
    ));
    assert_ne!(body(changed.current()), body(equal.current()));
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

fn packed(bytes: &[u8]) -> RepositoryWorkspaceBody {
    let mut builder = RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    for chunk in bytes.chunks(REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES) {
        builder.append(chunk, &mut |_| Ok::<_, ()>(())).unwrap();
    }
    builder.finish().unwrap()
}
#[test]
fn packed_history_pool_routes_without_generic_hash_or_historical_decode() {
    let initial = write(&initial(), packed(&vec![7; BYTES]));
    for read in [false, true] {
        let op = if read {
            RepositoryWorkspaceOperation::Read("out".into())
        } else {
            RepositoryWorkspaceOperation::Exists("out".into())
        };
        let result = if read {
            RepositoryWorkspaceResult::Read(body(initial.current()).clone())
        } else {
            RepositoryWorkspaceResult::Exists(true)
        };
        let request = initial.request(op).unwrap();
        super::super::body::reset_work();
        body_pool_work::reset();
        let next = initial
            .complete(
                request,
                result,
                RepositoryWorkspaceMode::Archive,
                initial.current().entries().to_vec(),
            )
            .unwrap();
        assert_eq!(
            super::super::body::work(),
            (0, 0, 0, 0),
            "canonical pool routing does not call full Hash or decode historical body bytes"
        );
        assert_eq!(body_pool_work::snapshot().calls[0], 1);
        assert!(body(next.current()).ptr_eq(body(initial.current())));
    }
    let foreign = packed(&vec![7; BYTES]);
    let weak = foreign.downgrade();
    super::super::body::reset_work();
    let next = write(&initial, foreign);
    let work = super::super::body::work();
    assert_eq!(work.0, 0);
    assert_eq!(work.1, 0);
    assert!(work.2 > 0);
    assert!(work.3 >= BYTES);
    assert!(body(next.current()).ptr_eq(body(initial.current())));
    assert!(weak.upgrade().is_none());
}
#[test]
fn actual_completion_routing_collision_preserves_unequal_full_history() {
    let mut first = packed(&vec![1; BYTES]);
    let mut second = packed(&vec![2; BYTES]);
    let collision = starlark_map::StarlarkHashValue::new_unchecked(0);
    first.set_test_routing_hash(collision);
    second.set_test_routing_hash(collision);
    let old = write(&initial(), first);
    let changed = write(&old, second);
    assert_eq!(changed.retention.body_bytes, 2 * BYTES as u64);
    assert!(!body(changed.current()).ptr_eq(body(old.current())));
    assert!(body(changed.current()).equals_slice(&vec![2; BYTES]));
    assert!(body(old.current()).equals_slice(&vec![1; BYTES]));
}

#[test]
fn packed_read_limit_is_logical_and_body_coverage_removes_only_logical_payload() {
    for extra in [0, 1] {
        let bytes = vec![0x61; 2 * 1024 * 1024 + extra];
        let old = write(&initial(), packed(&bytes));
        assert_eq!(old.retention.body_bytes, bytes.len() as u64);
        let retained = body(old.current());
        assert!(retained.native_storage_bytes() < retained.len());
        let full = old.storage_partition().unwrap();
        let covered = old.body_covered_storage_partition().unwrap();
        assert_eq!(full.owned_bytes() - covered.owned_bytes(), bytes.len());
        let request = old
            .request(RepositoryWorkspaceOperation::Read("out".into()))
            .unwrap();
        let result = old.complete(
            request,
            RepositoryWorkspaceResult::Read(retained.clone()),
            RepositoryWorkspaceMode::Archive,
            old.current().entries().to_vec(),
        );
        if extra == 0 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err(), RepositoryWorkspaceError::InvalidResult);
        }
    }
}

fn sha_traversals() -> usize {
    crate::source_preparation::workspace_identity::source_identity_derivation_count()
}

#[test]
fn closed_realization_rebinds_equal_packed_history_without_another_sha_traversal() {
    let foreign = write(&initial(), packed(&vec![7; BYTES]));
    let selected = write(&initial(), packed(&vec![7; BYTES]));
    assert_eq!(foreign, selected);
    assert!(!Arc::ptr_eq(foreign.current(), selected.current()));
    assert!(!body(foreign.current()).ptr_eq(body(selected.current())));
    let before = sha_traversals();
    let realization = foreign.realize(1).unwrap();
    assert_eq!(sha_traversals(), before + 1);
    let weak_foreign = body(foreign.current()).downgrade();
    let reused = realization.canonicalized_for(&selected).unwrap();
    assert_eq!(sha_traversals(), before + 1);
    assert_eq!(&realization, &reused);
    assert!(Arc::ptr_eq(
        realization.source_identity(),
        reused.source_identity()
    ));
    assert!(Arc::ptr_eq(reused.reservation(), selected.reservation()));
    assert!(Arc::ptr_eq(reused.final_state(), selected.current()));
    assert!(Arc::ptr_eq(
        &reused.transcript.records[0],
        &selected.records[0]
    ));
    assert!(body(reused.final_state()).ptr_eq(body(selected.current())));
    let same = reused.canonicalized_for(&selected).unwrap();
    assert_eq!(sha_traversals(), before + 1);
    assert!(Arc::ptr_eq(same.final_state(), selected.current()));
    drop(foreign);
    drop(realization);
    assert!(
        weak_foreign.upgrade().is_none(),
        "rebound realization selects only the canonical history owners"
    );
    assert_eq!(
        selected.realize(1).unwrap().source_identity(),
        reused.source_identity()
    );
    assert_eq!(
        sha_traversals(),
        before + 2,
        "ordinary realization still derives its exact SHA"
    );
    assert_eq!(
        selected.realize(0).unwrap_err(),
        RepositoryWorkspaceError::UnusedRecords
    );
    assert_eq!(sha_traversals(), before + 2);
}

#[test]
fn closed_realization_rebind_refuses_changed_history_counts_and_weak_collision() {
    let mut a = packed(&vec![1; BYTES]);
    let mut b = packed(&vec![2; BYTES]);
    let collision = starlark_map::StarlarkHashValue::new_unchecked(0);
    a.set_test_routing_hash(collision);
    b.set_test_routing_hash(collision);
    let left = write(&initial(), a);
    let different = write(&initial(), b);
    let realization = left.realize(1).unwrap();
    let before = sha_traversals();
    assert_eq!(
        realization.canonicalized_for(&different),
        Err(RepositoryWorkspaceError::ForeignPredecessor)
    );
    let mut changed_counts = left.clone();
    changed_counts.retention.metadata_bytes += 1;
    assert_eq!(
        realization.canonicalized_for(&changed_counts),
        Err(RepositoryWorkspaceError::ForeignPredecessor)
    );
    assert_eq!(sha_traversals(), before);
    let final_left = write(&left, packed(&vec![3; BYTES]));
    let final_right = write(&different, packed(&vec![3; BYTES]));
    let realization = final_left.realize(2).unwrap();
    let other = final_right.realize(2).unwrap();
    assert_eq!(realization.source_identity(), other.source_identity());
    assert_eq!(
        final_left.current().entries(),
        final_right.current().entries()
    );
    let before = sha_traversals();
    assert_eq!(
        realization.canonicalized_for(&final_right),
        Err(RepositoryWorkspaceError::ForeignPredecessor),
        "equal final content never substitutes for exact earlier records"
    );
    assert_eq!(sha_traversals(), before);
}

#[test]
fn closed_realization_rebind_keeps_raw_packed_retention_differences_exact() {
    let bytes = vec![7; BYTES];
    let raw = write(
        &initial(),
        RepositoryWorkspaceBody::from_arc(Arc::from(bytes.as_slice())),
    );
    let packed = write(&initial(), packed(&bytes));
    assert_eq!(raw.current().entries(), packed.current().entries());
    assert_eq!(raw.retention.body_bytes, packed.retention.body_bytes);
    assert_ne!(
        raw.retention.metadata_bytes,
        packed.retention.metadata_bytes
    );
    let realization = raw.realize(1).unwrap();
    let other = packed.realize(1).unwrap();
    assert_eq!(realization.source_identity(), other.source_identity());
    let before = sha_traversals();
    assert_eq!(
        realization.canonicalized_for(&packed),
        Err(RepositoryWorkspaceError::ForeignPredecessor)
    );
    assert_eq!(
        other.canonicalized_for(&raw),
        Err(RepositoryWorkspaceError::ForeignPredecessor)
    );
    assert_eq!(sha_traversals(), before);
    assert!(realization.canonicalized_for(&raw).is_ok());
}
