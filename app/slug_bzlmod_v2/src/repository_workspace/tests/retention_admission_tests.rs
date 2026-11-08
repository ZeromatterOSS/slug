use super::*;

fn policy() -> RepositoryWorkspaceRetentionLimits {
    RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap()
}

fn reserve_id(id: u64) -> Arc<RepositoryWorkspaceReservation> {
    Arc::new(
        RepositoryWorkspaceReservation::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            Arc::new(HostSelectedExtensionOwner::testing("owner")),
            CanonicalRepoName::new(format!("owner+repo{id}")).unwrap(),
            usize::try_from(id).unwrap(),
            workspace_spec(),
            NormalizedAbsolutePath::new(format!("/logical/repo{id}")).unwrap(),
            PathObservationInstanceId::new(id),
            7,
            policy(),
        )
        .unwrap(),
    )
}

fn empty_epoch() -> RepositoryWorkspaceEpoch {
    RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        7,
        policy(),
    )
    .unwrap()
}

fn execution(reservation: &RepositoryWorkspaceReservation) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::Execute(
        RepositoryWorkspaceExecute::new(
            Arc::from([RepositoryWorkspaceArgument::String(
                "arbitrary-helper".into(),
            )]),
            RepositoryEnvironmentSnapshot::empty(),
            SmallMap::new(),
            600,
            true,
            HostRepositoryLabelPathValue::new(
                reservation.display_root().clone(),
                PathObservationNamespace::Materialization(reservation.instance()),
            )
            .unwrap(),
            RepositoryWorkspaceReadViewManifest::empty(reservation),
        )
        .unwrap(),
    )
}

fn execute_result() -> RepositoryWorkspaceResult {
    RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
        0,
        Arc::from([]),
        Arc::from([]),
    ))
}

#[test]
fn history_beyond_128_versions_preserves_noops_and_exact_replay() {
    let mut epoch = empty_epoch();
    let first = reserve_id(1);
    for id in 1..=43 {
        let reservation = reserve_id(id);
        epoch = epoch.reserve(reservation.clone(), 0o755).unwrap();
        for index in 0..if id == 1 { 1 } else { 2 } {
            let request = epoch
                .request_at(
                    &reservation,
                    index,
                    RepositoryWorkspaceOperation::Exists("".into()),
                )
                .unwrap();
            epoch = epoch
                .complete(
                    request,
                    RepositoryWorkspaceResult::Exists(true),
                    RepositoryWorkspaceMode::Archive,
                    vec![root()],
                )
                .unwrap();
        }
    }
    let transcripts = epoch.transcripts().unwrap();
    assert_eq!(transcripts.len(), 43);
    assert_eq!(
        transcripts.iter().map(|t| t.records().len()).sum::<usize>(),
        85
    );
    assert_eq!(
        transcripts
            .iter()
            .map(|t| t.retention().state_versions())
            .sum::<usize>(),
        128
    );
    assert_eq!(
        transcripts
            .iter()
            .map(|t| t.retention().executes())
            .sum::<usize>(),
        0
    );
    let next = reserve_id(44);
    let forecast = epoch.reserve_storage_partition(&next, 0o755).unwrap();
    assert!(forecast.owned_bytes() <= policy().metadata_bytes());
    epoch = epoch.reserve(next, 0o755).unwrap();
    assert_eq!(
        epoch
            .transcripts()
            .unwrap()
            .iter()
            .map(|t| t.retention().state_versions())
            .sum::<usize>(),
        129
    );
    // Identical no-op trees are distinct retained versions; initial and successor
    // owners both remain available for exact historical replay.
    let initial = epoch.transcripts().unwrap()[0].clone();
    assert_eq!(initial.initial(), initial.current());
    let request = epoch
        .request_at(&first, 0, RepositoryWorkspaceOperation::Exists("".into()))
        .unwrap();
    assert!(initial.replay(&request).unwrap().is_some());
    let changed = epoch
        .request_at(
            &first,
            0,
            RepositoryWorkspaceOperation::Exists("absent".into()),
        )
        .unwrap();
    assert_eq!(
        initial.replay(&changed),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    assert_eq!(
        initial.realize(0),
        Err(RepositoryWorkspaceError::UnusedRecords)
    );
    assert!(initial.realize(1).is_ok());
}

#[test]
fn root_capacity_is_independent_and_refuses_forecast_before_new_state() {
    let mut epoch = empty_epoch();
    for id in 1..=128 {
        epoch = epoch.reserve(reserve_id(id), 0o755).unwrap();
    }
    let unchanged = epoch.clone();
    let next = reserve_id(129);
    assert_eq!(
        epoch.reserve_storage_partition(&next, 0o755),
        Err(RepositoryWorkspaceError::RetentionLimit)
    );
    assert_eq!(
        epoch.reserve(next, 0o755),
        Err(RepositoryWorkspaceError::RetentionLimit)
    );
    assert_eq!(epoch, unchanged);
    assert_eq!(epoch.transcripts().unwrap().len(), policy().max_roots());
}

#[test]
fn command_execute_128_is_independent_of_history_and_authenticates_old_replay() {
    let a = reserve_id(1);
    let b = reserve_id(2);
    let mut epoch = empty_epoch()
        .reserve(a.clone(), 0o755)
        .unwrap()
        .reserve(b.clone(), 0o755)
        .unwrap();
    for index in 0..64 {
        for reservation in [&a, &b] {
            let request = epoch
                .request_at(reservation, index, execution(reservation))
                .unwrap();
            epoch = epoch
                .complete(
                    request,
                    execute_result(),
                    RepositoryWorkspaceMode::Process,
                    vec![root()],
                )
                .unwrap();
        }
    }
    let transcripts = epoch.transcripts().unwrap();
    assert_eq!(
        transcripts
            .iter()
            .map(|t| t.retention().state_versions())
            .sum::<usize>(),
        130
    );
    assert_eq!(
        transcripts
            .iter()
            .map(|t| t.retention().executes())
            .sum::<usize>(),
        128
    );
    let unchanged = epoch.clone();
    assert_eq!(
        epoch.request_at(&a, 64, execution(&a)),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    // A standalone transcript is not command authority. Both direct completion
    // and publication forecasts must reject this otherwise valid 65th record.
    let t = &transcripts[0];
    let extra = t.request(execution(&a)).unwrap();
    assert_eq!(
        epoch.complete(
            extra.clone(),
            execute_result(),
            RepositoryWorkspaceMode::Process,
            vec![root()]
        ),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    let candidate = t
        .complete(
            extra,
            execute_result(),
            RepositoryWorkspaceMode::Process,
            vec![root()],
        )
        .unwrap();
    assert_eq!(candidate.retention().executes(), 65);
    assert_eq!(
        epoch.publish_storage_partition(&candidate),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    assert_eq!(
        epoch.publish_completed(candidate),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    assert_eq!(epoch, unchanged);
    for reservation in [&a, &b] {
        let old = epoch
            .request_at(reservation, 0, execution(reservation))
            .unwrap();
        let t = epoch
            .transcripts()
            .unwrap()
            .iter()
            .find(|t| t.reservation() == reservation)
            .unwrap();
        assert!(t.replay(&old).unwrap().is_some());
    }
    // Non-Execute successors still fit; they retain history without spending a
    // sample or refunding an old one.
    let request = epoch
        .request_at(&a, 64, RepositoryWorkspaceOperation::Exists("".into()))
        .unwrap();
    epoch = epoch
        .complete(
            request,
            RepositoryWorkspaceResult::Exists(true),
            RepositoryWorkspaceMode::Process,
            vec![root()],
        )
        .unwrap();
    assert_eq!(epoch.transcripts().unwrap()[0].retention().executes(), 64);
    assert_eq!(
        epoch.transcripts().unwrap()[0].retention().state_versions(),
        66
    );
    assert_eq!(
        epoch.request_at(&a, 65, execution(&a)),
        Err(RepositoryWorkspaceError::StateLimit)
    );
}
