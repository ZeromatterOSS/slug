//! Actual sealed General owner reuse and independently charged pure-body life.
use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;
use crate::runtime::dice::source_service::SourceCounter;

#[test]
#[ignore = "actual compiled CLI claimed General root; finite native supervisor and SLUG_V2_BIN required"]
fn native_owned_file_observation_reuses_canonical_body_and_preserves_history_after_root_drop() {
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
    let (token, generation) =
        generation(&actor, &key, &resources, &readers, &scope, &runtime, BODY);
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
    let first = actor
        .observe_native(token, [bytes.clone(), hash.clone()])
        .unwrap();
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
    let repeated = actor.observe_native(token, [bytes.clone(), hash]).unwrap();
    let after_repeat = scope.snapshot();
    let same_repeat = repeated.observations().values().any(|value| matches!(value.as_ref(),PathObservationResult::FileBytes(PathOperationResult::Present(body)) if Arc::ptr_eq(body,canonical.raw_arc().unwrap())));
    drop(first);
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
        after_repeat.decoded_bytes - after_first.decoded_bytes,
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
