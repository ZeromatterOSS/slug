use super::*;

fn request(lease: u64) -> Arc<RepositorySourceSnapshotRequest> {
    let route = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(route, None).unwrap();
    Arc::new(RepositorySourceSnapshotRequest::from_canonical(&input, lease).unwrap())
}
fn reservation(lease: u64, instance: u64) -> Arc<RepositorySourceSnapshotReservation> {
    Arc::new(
        RepositorySourceSnapshotReservation::new(
            request(lease),
            NormalizedAbsolutePath::new("/__slug_repositories__/v1/builtin").unwrap(),
            PathObservationInstanceId::new(instance),
        )
        .unwrap(),
    )
}
fn root() -> RepositorySourceSnapshotEntry {
    RepositorySourceSnapshotEntry::new("".into(), 0o755, RepositorySourceSnapshotNode::Directory)
        .unwrap()
}
fn file(name: &str, size: u64) -> RepositorySourceSnapshotEntry {
    RepositorySourceSnapshotEntry::new(
        name.into(),
        0o644,
        RepositorySourceSnapshotNode::File(FileContentDigest::new([1; 32], size).unwrap()),
    )
    .unwrap()
}

#[test]
fn genuine_builtin_subject_keeps_catalog_mapping_and_has_no_invented_spec() {
    let a = request(7);
    assert_eq!(a.kind(), RepositorySourceSnapshotKind::Builtin);
    assert_eq!(
        a.builtin_identity(),
        Some(crate::BuiltinBazelToolsSnapshot::CURRENT.route_identity())
    );
    assert!(a.repo_spec().is_none());
    assert!(a.materialization_request().is_none());
    assert!(a.selected_identity().is_none());
    assert_ne!(a, request(8));
    assert!(
        a.retained_storage_bytes().unwrap()
            > std::mem::size_of::<RepositorySourceSnapshotRequest>()
    );
    assert!(
        RepositorySourceSnapshotRequest::from_canonical(
            &crate::host_canonical_repository_source_input(
                Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    crate::HostBuiltinBazelToolsRepositoryMapping::testing()
                )),
                None
            )
            .unwrap(),
            0
        )
        .is_err()
    );
}

#[test]
fn digest_state_enforces_exact_file_and_total_limits_without_body_retention() {
    let r = reservation(7, 1);
    let entries = vec![
        root(),
        file("a", 64 * 1024 * 1024),
        file("b", 64 * 1024 * 1024),
        file("c", 64 * 1024 * 1024),
        file("d", 64 * 1024 * 1024),
    ];
    let forecast = RepositorySourceSnapshotState::retained_storage_bytes_for(&r, &entries).unwrap();
    let state = RepositorySourceSnapshotState::new(r.clone(), entries).unwrap();
    assert_eq!(state.file_bytes(), REPOSITORY_SOURCE_SNAPSHOT_TOTAL_BYTES);
    assert_eq!(forecast, state.retained_storage_bytes().unwrap());
    assert!(forecast < 64 * 1024);
    assert_eq!(
        RepositorySourceSnapshotState::new(
            r.clone(),
            vec![root(), file("a", REPOSITORY_SOURCE_SNAPSHOT_FILE_BYTES + 1)]
        )
        .unwrap_err(),
        RepositoryWorkspaceError::StateLimit
    );
    let mut excess = state.entries().to_vec();
    excess.push(file("e", 1));
    assert_eq!(
        RepositorySourceSnapshotState::new(r, excess).unwrap_err(),
        RepositoryWorkspaceError::StateLimit
    );
}

#[test]
fn complete_tree_rejects_missing_parents_aliases_and_escaping_literal_links() {
    let r = reservation(7, 1);
    for entries in [
        vec![],
        vec![file("a", 1)],
        vec![root(), file("absent/a", 1)],
        vec![root(), file("a", 1), file("a", 1)],
    ] {
        assert!(RepositorySourceSnapshotState::new(r.clone(), entries).is_err());
    }
    for link in ["/absolute", "../outside", "a/../../outside"] {
        let e = RepositorySourceSnapshotEntry::new(
            "link".into(),
            0o777,
            RepositorySourceSnapshotNode::Link(link.into()),
        )
        .unwrap();
        assert_eq!(
            RepositorySourceSnapshotState::new(r.clone(), vec![root(), e]).unwrap_err(),
            RepositoryWorkspaceError::InvalidPath
        );
    }
    let link = |text: &str| {
        RepositorySourceSnapshotEntry::new(
            "link".into(),
            0o777,
            RepositorySourceSnapshotNode::Link(text.into()),
        )
        .unwrap()
    };
    let a =
        RepositorySourceSnapshotState::new(r.clone(), vec![root(), file("data", 1), link("data")])
            .unwrap();
    let b = RepositorySourceSnapshotState::new(r, vec![root(), file("data", 1), link("./data")])
        .unwrap();
    assert_ne!(a, b);
    assert!(
        RepositorySourceSnapshotEntry::new(
            "a/../b".into(),
            0o644,
            RepositorySourceSnapshotNode::Directory
        )
        .is_err()
    );
    assert!(
        RepositorySourceSnapshotEntry::new(
            "a".repeat(255).into(),
            0o755,
            RepositorySourceSnapshotNode::Directory
        )
        .is_ok()
    );
    assert!(
        RepositorySourceSnapshotEntry::new(
            "a".repeat(256).into(),
            0o755,
            RepositorySourceSnapshotNode::Directory
        )
        .is_err()
    );
    assert!(
        RepositorySourceSnapshotEntry::new(
            "a".repeat(257).into(),
            0o644,
            RepositorySourceSnapshotNode::Directory
        )
        .is_err()
    );
}

#[test]
fn snapshot_epoch_is_command_bound_and_rejects_replay_and_revocation() {
    let state = Arc::new(
        RepositorySourceSnapshotState::new(reservation(7, 1), vec![root(), file("data", 1)])
            .unwrap(),
    );
    let epoch =
        RepositorySourceSnapshotEpoch::fresh(NormalizedAbsolutePath::new("/workspace").unwrap(), 7)
            .unwrap();
    assert!(epoch.find(&request(7)).unwrap().is_none());
    let epoch = epoch.with_snapshot(state.clone()).unwrap();
    assert!(Arc::ptr_eq(
        epoch.find(&request(7)).unwrap().unwrap(),
        &state
    ));
    assert_eq!(epoch.clone(), epoch.with_snapshot(state).unwrap());
    let changed = Arc::new(
        RepositorySourceSnapshotState::new(reservation(7, 2), vec![root(), file("data", 1)])
            .unwrap(),
    );
    assert_eq!(
        epoch.with_snapshot(changed).unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    assert_eq!(
        epoch.find(&request(8)).unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    assert_eq!(
        RepositorySourceSnapshotEpoch::revoked()
            .find(&request(7))
            .unwrap_err(),
        RepositoryWorkspaceError::Revoked
    );
}

#[path = "needs_tests.rs"]
mod needs_tests;
