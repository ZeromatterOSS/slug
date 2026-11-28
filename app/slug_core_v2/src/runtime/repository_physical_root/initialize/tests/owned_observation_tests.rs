//! Actual sealed General owner reuse and independently charged pure-body life.
use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::repository_root_io::read_work_for_test;
use crate::runtime::repository_workspace_comparison_work_for_test as comparison_work;

#[test]
#[ignore = "actual compiled CLI claimed General root; finite native supervisor and SLUG_V2_BIN required"]
fn native_owned_file_observation_reuses_canonical_body_and_preserves_history_after_root_drop() {
    verify_native_owned_observation(false);
}

#[test]
#[ignore = "actual compiled CLI archive finalization and rooted certification"]
fn native_only_frozen_archive_certification_reuses_the_actual_digest() {
    verify_native_owned_observation(true);
}

#[test]
#[ignore = "actual compiled CLI Archive finalization and selected native source reader"]
fn native_selected_direct_archive_reader_reuses_packed_frame_and_probes_real_eof() {
    const BLOCK: usize = slug_bzlmod_v2::REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES;
    let mut expected = vec![b'a'; 2 * BLOCK + 37];
    expected[BLOCK..2 * BLOCK].fill(b'b');
    expected[2 * BLOCK..].fill(b'c');
    let wanted = digest(&expected);
    let wrong = FileContentDigest::new([0; 32], wanted.size_bytes()).unwrap();

    for finalized in [true, false] {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(temporary.path()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(26);
        let scope = SourceServiceScope::new_for_test(
            workspace.clone(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap();
        let resources = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 2 * ARCHIVE_BYTES).unwrap(),
        )
        .unwrap();
        let before = resources.snapshot();
        let readers = new_reader_resources().unwrap();
        let reader_baseline = readers.snapshot();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let key = Arc::new(
            RepositoryWorkspaceStableKey::new(
                workspace.clone(),
                crate::runtime::dice::physical_root_owner_for_test(),
                CanonicalRepoName::new("archive_direct_reader").unwrap(),
                0,
            )
            .unwrap(),
        );
        let actor = RepositoryMaterializer::new_scoped(workspace, scope.clone()).unwrap();
        let (token, generation) = generation_with_adoption_spec(
            &actor,
            &key,
            &resources,
            &readers,
            &scope,
            &runtime,
            FixtureBody::Packed(&expected),
            false,
            finalized,
        )
        .unwrap();
        let lease = generation
            .roots
            .lease_for(&generation.source)
            .unwrap()
            .unwrap();
        let root = lease.physical().unwrap();
        let (index, body, _) = root.canonical_file("input").unwrap();
        assert!(body.raw_arc().is_none());
        assert_eq!(generation.source.digest(), wanted);
        assert_eq!(
            root.archive_digest_receipt(index)
                .map(|receipt| receipt.digest()),
            finalized.then_some(wanted)
        );
        let open_selected = || {
            NativeSourceReader::open_selected_for_test(
                &generation.source,
                &generation.roots,
                Some(scope.clone()),
                readers.clone(),
            )
            .unwrap()
        };
        assert_reader_work(
            open_selected(),
            &expected,
            if finalized { 0 } else { wanted.size_bytes() },
        );
        assert_reader_work(
            root.open_member("input", &scope, &|| true).unwrap(),
            &expected,
            wanted.size_bytes(),
        );
        assert_reader_work(
            root.open_selected_direct_member("input", wrong, &scope, &|| true)
                .unwrap(),
            &expected,
            wanted.size_bytes(),
        );
        let foreign = source(
            &generation,
            generation.source.namespace(),
            generation.source.anchor().cloned(),
            wrong,
        );
        assert!(
            NativeSourceReader::open_selected_for_test(
                &foreign,
                &generation.roots,
                Some(scope.clone()),
                readers.clone(),
            )
            .is_err()
        );

        let mut selected = open_selected();
        actor
            .accept(token, std::slice::from_ref(&generation.request), vec![])
            .unwrap();
        scope.cancel();
        {
            let reads = read_work_for_test::begin();
            let comparisons = comparison_work::begin();
            let error = selected.read(&mut [0; 1]).unwrap_err();
            assert!(error.to_string().contains("Cancelled"));
            assert_eq!(reads.reader_payload_bytes(), 0);
            assert_eq!(reads.reader_eof_probes(), 0);
            assert_eq!(comparisons.snapshot().comparison_calls, 0);
        }
        drop(selected);
        drop(lease);
        drop(generation);
        drop(actor);
        let after = resources.snapshot();
        assert_eq!(after.retained_bytes, before.retained_bytes);
        assert_eq!(after.backing_bytes, 0);
        assert_eq!(after.native_fds, 0);
        assert_eq!(
            readers.snapshot().retained_bytes,
            reader_baseline.retained_bytes
        );
        assert_eq!(readers.snapshot().native_fds, 0);
    }
}

fn assert_reader_work(mut reader: impl Read, expected: &[u8], compared_bytes: u64) {
    let reads = read_work_for_test::begin();
    let comparisons = comparison_work::begin();
    let mut bytes = vec![0; expected.len()];
    reader.read_exact(&mut bytes).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(reads.reader_payload_bytes(), expected.len() as u64);
    assert_eq!(reads.reader_eof_probes(), 1);
    assert_eq!(comparisons.snapshot().comparison_bytes, compared_bytes);
    assert_eq!(
        comparisons.snapshot().comparison_calls == 0,
        compared_bytes == 0
    );
}

fn verify_native_owned_observation(finalize_archive: bool) {
    const BODY: &[u8] = b"owned observation\0\xff";
    let temporary = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(temporary.path()).unwrap();
    let work = Instant::now() + Duration::from_secs(26);
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let owner = crate::runtime::dice::physical_root_owner_for_test();
    let repository = CanonicalRepoName::new(format!(
        "{}+owned_observation",
        owner.unique_name().as_str()
    ))
    .unwrap();
    let key = Arc::new(
        RepositoryWorkspaceStableKey::new(workspace.clone(), owner, repository, 0).unwrap(),
    );
    let actor = RepositoryMaterializer::new_scoped(workspace, scope.clone()).unwrap();
    let (token, generation) = generation_with_adoption_spec(
        &actor,
        &key,
        &resources,
        &readers,
        &scope,
        &runtime,
        FixtureBody::Raw(BODY),
        false,
        finalize_archive,
    )
    .unwrap();
    let lease = generation
        .roots
        .lease_for(&generation.source)
        .unwrap()
        .unwrap();
    let root = lease.physical().unwrap().clone();
    drop(lease);
    let weak_root = Arc::downgrade(&root);
    let weak_state = Arc::downgrade(root.state());
    let RepositoryWorkspaceNode::File(canonical) = root.state().entry("input").unwrap().node()
    else {
        panic!("actual File snapshot must own its canonical body")
    };
    let weak_canonical = canonical.downgrade();
    let namespace = generation.source.namespace();
    let path = generation.source.real_path().clone();
    let bytes =
        PathObservationDemand::new(namespace, path.clone(), PathObservationOperation::FileBytes);
    let hash = PathObservationDemand::new(namespace, path, PathObservationOperation::FileDigest);
    let admission = resources.snapshot();
    let counters = scope.snapshot();
    let outside = PathObservationDemand::new(
        namespace,
        NormalizedAbsolutePath::new(root.anchor().root().as_path().parent().unwrap()).unwrap(),
        PathObservationOperation::FileBytes,
    );
    assert!(
        actor
            .observe_native(token, [bytes.clone(), outside])
            .is_err()
    );
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
    assert_eq!(
        resources.snapshot().peak_native_fds,
        admission.peak_native_fds
    );
    let foreign = SourceServiceScope::new_for_test(
        scope.workspace().clone(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        root.observe(&[bytes.clone()], &foreign, &|| true),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::ForeignScope
        ))
    ));
    assert_eq!(foreign.snapshot().source_attempts, 0);
    assert_eq!(
        resources.snapshot().peak_native_fds,
        admission.peak_native_fds
    );
    let before = scope.snapshot();
    let work = read_work_for_test::begin();
    let first = actor
        .observe_native(token, [bytes.clone(), hash.clone()])
        .unwrap();
    assert_eq!(work.snapshot().digest_sha, BODY.len() as u64);
    drop(work);
    let after_first = scope.snapshot();
    let PathObservationResult::FileBytes(PathOperationResult::Present(body)) =
        first.get(&bytes).unwrap().as_ref()
    else {
        panic!("FileBytes must contain the observed body")
    };
    let same_first = Arc::ptr_eq(body, canonical.raw_arc().unwrap());
    let kept = body.clone();
    assert_eq!(kept.as_ref(), BODY);
    assert!(
        matches!(first.get(&hash).unwrap().as_ref(),PathObservationResult::FileDigest(PathOperationResult::Present(value)) if *value==digest(BODY))
    );
    {
        let remaining = 512 * 1024 * 1024 - resources.snapshot().retained_bytes;
        let _guard = resources.reserve(remaining, 0, 0).unwrap();
        let direct = root.verify_direct_body_digest("input", digest(BODY));
        if finalize_archive {
            direct.unwrap();
            assert!(matches!(
                root.verify_direct_body_digest("input", digest(&[0; BODY.len()])),
                Err(PhysicalRootError::Snapshot)
            ));
        } else {
            assert!(matches!(direct, Err(PhysicalRootError::Resource)));
        }
    }
    let ordinary = actor
        .observe_native(token, [bytes.clone(), hash.clone()])
        .unwrap();
    let after_ordinary = scope.snapshot();
    assert_eq!(ordinary, first);

    let work = read_work_for_test::begin();
    let repeated = actor
        .observe_native_certificate(
            token,
            vec![bytes.clone(), hash.clone()],
            &first,
            &mut |_| {},
            &mut |_, _, _| {},
        )
        .unwrap();
    let digest_bytes = if finalize_archive {
        0
    } else {
        BODY.len() as u64
    };
    assert_eq!(work.snapshot().digest_payload, digest_bytes);
    assert_eq!(work.snapshot().digest_sha, digest_bytes);
    assert_eq!(work.snapshot().contents_payload, BODY.len() as u64);
    let after_certified = scope.snapshot();
    assert_eq!(
        after_certified.decoded_bytes - after_ordinary.decoded_bytes,
        BODY.len() as u64 + digest_bytes
    );
    assert_eq!(repeated, first);
    drop(work);

    let wrong = PathObservationEpoch::new([(
        hash.clone(),
        PathObservationResult::FileDigest(PathOperationResult::Present(digest(b"wrong"))),
    )])
    .unwrap();
    let work = read_work_for_test::begin();
    let fallback = actor
        .observe_native_certificate(
            token,
            vec![hash.clone()],
            &wrong,
            &mut |_| {},
            &mut |_, _, _| {},
        )
        .unwrap();
    assert_eq!(fallback.get(&hash), first.get(&hash));
    assert_eq!(work.snapshot().digest_sha, BODY.len() as u64);
    drop(work);
    let after_repeat = scope.snapshot();
    assert_eq!(
        after_repeat.decoded_bytes - after_certified.decoded_bytes,
        BODY.len() as u64
    );
    let same_repeat = repeated.observations().values().any(|value| matches!(value.as_ref(),PathObservationResult::FileBytes(PathOperationResult::Present(body)) if Arc::ptr_eq(body,canonical.raw_arc().unwrap())));
    drop(first);
    drop(ordinary);
    drop(repeated);
    let weak_body = Arc::downgrade(&kept);
    actor
        .accept(token, std::slice::from_ref(&generation.request), vec![])
        .unwrap();
    // A known owner cannot bypass a sticky failed acquisition scope. This is a
    // real limit refusal, not a fabricated native observation or capture fact.
    assert!(
        scope
            .charge(
                SourceCounter::SourceAttempts,
                scope.limits().source_attempts + 1
            )
            .is_err()
    );
    let before_poisoned_read = resources.snapshot();
    assert!(matches!(
        root.observe(&[bytes], &scope, &|| true),
        Err(PhysicalRootError::SourceService(SourceServiceError::Limit(
            SourceCounter::SourceAttempts
        )))
    ));
    assert_eq!(
        resources.snapshot().peak_native_fds,
        before_poisoned_read.peak_native_fds
    );
    assert_eq!(scope.snapshot().decoded_bytes, after_repeat.decoded_bytes);
    // generation() has already retired command/bootstrap custody. Retire every
    // registry, selected lease, full realization and native data owner next.
    drop(generation);
    drop(actor);
    drop(key);
    drop(root);
    assert!(weak_root.upgrade().is_none());
    assert!(weak_state.upgrade().is_none());
    let after_roots = resources.snapshot();
    assert_eq!(
        (
            after_roots.native_fds,
            after_roots.backing_bytes,
            after_roots.retained_bytes
        ),
        (0, 0, baseline)
    );
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    // The pure body aliases have independent cumulative decoded-history
    // accounting. Native bank refund does not claim they have been destroyed.
    assert!(weak_body.upgrade().is_some());
    assert_eq!(kept.as_ref(), BODY);
    let canonical_survived = weak_canonical.upgrade().is_some();
    assert_eq!(scope.snapshot().decoded_bytes, after_repeat.decoded_bytes);
    drop(kept);
    assert!(weak_body.upgrade().is_none());
    assert!(weak_canonical.upgrade().is_none());
    assert_eq!(
        scope.snapshot().decoded_bytes,
        after_repeat.decoded_bytes,
        "last body drop never refunds cumulative history"
    );
    assert_eq!(
        after_first.decoded_bytes - before.decoded_bytes,
        2 * BODY.len() as u64
    );
    assert_eq!(
        after_ordinary.decoded_bytes - after_first.decoded_bytes,
        2 * BODY.len() as u64
    );
    assert_eq!(
        after_first.source_attempts - before.source_attempts,
        0,
        "direct owned verification is not a new capture"
    );
    assert_eq!(
        after_repeat.source_attempts - after_first.source_attempts,
        0,
        "repeated verification must not spend captures"
    );
    assert!(
        same_first && same_repeat && canonical_survived,
        "each FileBytes result must reuse the exact canonical body allocation"
    );
}
