// Producer namespaces classify image facts; path spellings grant no exemption.
use super::*;

#[test]
fn command_source_partition_keeps_host_and_other_materialization_validation() {
    use slug_bzlmod_v2::RepositorySourceSnapshotEntry;
    use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
    use slug_bzlmod_v2::RepositorySourceSnapshotNode;
    use slug_bzlmod_v2::RepositorySourceSnapshotReservation;
    use slug_bzlmod_v2::RepositorySourceSnapshotState;
    use slug_workspace_v2::PathObservationInstanceId;
    let workspace = tempfile::tempdir().unwrap();
    let source = crate::runtime::dice::workspace_source_request_for_test(workspace.path(), 17);
    let display = NormalizedAbsolutePath::new("/__slug_repositories__/s1/partition").unwrap();
    // Pure metadata only; this probe creates no native root or completion.
    let state = Arc::new(
        RepositorySourceSnapshotState::new(
            Arc::new(
                RepositorySourceSnapshotReservation::new(
                    source,
                    display.clone(),
                    PathObservationInstanceId::new(41),
                )
                .unwrap(),
            ),
            vec![
                RepositorySourceSnapshotEntry::new(
                    "".into(),
                    0o755,
                    RepositorySourceSnapshotNode::Directory,
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let epoch = RepositorySourceSnapshotEpoch::fresh(state.reservation().workspace().clone(), 17)
        .unwrap()
        .with_snapshot(state)
        .unwrap();
    let demand = |namespace, relative| {
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(display.as_path().join(relative)).unwrap(),
            PathObservationOperation::Lstat,
        )
    };
    let image = demand(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(41)),
        "body",
    );
    let scoped = demand(image.namespace(), "scoped");
    let host = demand(PathObservationNamespace::Host, "body");
    let ordinary = demand(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(42)),
        "body",
    );
    let repository = request("repo", "rule");
    let selected = SelectedWorkspaceDemands::for_test_with_validation(
        vec![repository.clone()],
        repository.clone(),
        scoped.clone(),
    )
    .with_additional_unscoped_paths([image.clone(), host.clone(), ordinary.clone()]);
    let original = selected.clone();
    assert_eq!(
        selected
            .clone()
            .partition_command_sources(&RepositorySourceSnapshotEpoch::revoked()),
        original
    );
    let selected = selected.partition_command_sources(&epoch);
    let mut expected = vec![image, scoped];
    expected.sort_unstable();
    assert_eq!(selected.command_paths(), expected);
    let mut reusable = vec![host, ordinary];
    reusable.sort_unstable();
    assert_eq!(selected.unscoped_paths(), reusable);
    assert_eq!(selected.repository_validations().len(), 1);
    assert!(selected.repository_validations()[0].paths().is_empty());
    assert!(Arc::ptr_eq(&selected.repository_requests()[0], &repository));
}
