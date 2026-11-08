use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::PathObservationNamespace;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::HostRepositoryLabelPathValue;
use crate::RepositoryEnvironmentEntry;
use crate::RepositoryEnvironmentSnapshot;
use crate::RepositoryExecuteResult;
use crate::RepositoryLabelPathAddress;

fn limits() -> RepositoryWorkspaceRetentionLimits {
    RepositoryWorkspaceRetentionLimits::new(1024, 128 * 1024 * 1024, 2 * 1024 * 1024 * 1024)
        .unwrap()
}
fn reservation_with(
    lease: u64,
    instance: u64,
    limits: RepositoryWorkspaceRetentionLimits,
) -> Arc<RepositoryWorkspaceReservation> {
    Arc::new(
        RepositoryWorkspaceReservation::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            Arc::new(HostSelectedExtensionOwner::testing("owner")),
            CanonicalRepoName::new("owner+repo").unwrap(),
            3,
            NormalizedAbsolutePath::new("/logical/repo").unwrap(),
            PathObservationInstanceId::new(instance),
            lease,
            limits,
        )
        .unwrap(),
    )
}
fn transcript() -> RepositoryWorkspaceTranscript {
    RepositoryWorkspaceTranscript::new(reservation_with(7, 11, limits()), 0o755).unwrap()
}
fn root() -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory).unwrap()
}
fn file(path: &str, body: Arc<[u8]>) -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new(path.into(), 0o644, RepositoryWorkspaceNode::File(body)).unwrap()
}
fn write(
    t: &RepositoryWorkspaceTranscript,
    body: &[u8],
) -> Result<RepositoryWorkspaceTranscript, RepositoryWorkspaceError> {
    t.complete(
        t.request(RepositoryWorkspaceOperation::File {
            path: "out".into(),
            body: Arc::from(body),
            mode: 0o644,
        })?,
        RepositoryWorkspaceResult::Unit,
        t.current().mode(),
        vec![root(), file("out", Arc::from(body))],
    )
}
fn host_path(path: &str) -> HostRepositoryLabelPathValue {
    HostRepositoryLabelPathValue::new(
        NormalizedAbsolutePath::new(path).unwrap(),
        PathObservationNamespace::Host,
    )
    .unwrap()
}
fn execute() -> RepositoryWorkspaceOperation {
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
                NormalizedAbsolutePath::new("/logical/repo").unwrap(),
                PathObservationNamespace::Materialization(PathObservationInstanceId::new(11)),
            )
            .unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn reservation_rejects_zero_and_retains_full_generation_policy_identity() {
    assert!(RepositoryWorkspaceRetentionLimits::new(0, 1024, 1).is_err());
    assert!(RepositoryWorkspaceRetentionLimits::new(1, 0, 1).is_err());
    assert!(RepositoryWorkspaceRetentionLimits::new(1, 1024, 0).is_err());
    assert!(RepositoryWorkspaceRetentionLimits::new(usize::MAX, usize::MAX, 1).is_err());
    let a = reservation_with(7, 11, limits());
    let b = reservation_with(8, 12, limits());
    assert_eq!(a.display_root(), b.display_root());
    assert_ne!(a, b);
    let different_policy = RepositoryWorkspaceRetentionLimits::new(
        1023,
        limits().metadata_bytes(),
        limits().archive_body_bytes(),
    )
    .unwrap();
    assert_ne!(a, reservation_with(7, 11, different_policy));
    assert!(
        RepositoryWorkspaceReservation::new(
            a.workspace().clone(),
            a.owner().clone(),
            a.repository().clone(),
            a.ordinal(),
            a.display_root().clone(),
            PathObservationInstanceId::new(0),
            7,
            limits()
        )
        .is_err()
    );
    assert!(
        RepositoryWorkspaceReservation::new(
            a.workspace().clone(),
            a.owner().clone(),
            a.repository().clone(),
            a.ordinal(),
            a.display_root().clone(),
            a.instance(),
            0,
            limits()
        )
        .is_err()
    );
}

#[test]
fn all_position_arguments_environment_removal_empty_and_requested_timeout_are_literal() {
    let label = CanonicalLabel::parse("@@repo//pkg:tool").unwrap();
    let path = host_path("/opt/tool");
    let argv: Arc<[RepositoryWorkspaceArgument]> = Arc::from([
        RepositoryWorkspaceArgument::Path(path.clone()),
        RepositoryWorkspaceArgument::Label {
            address: RepositoryLabelPathAddress::from_label(&label),
            resolved: path.clone(),
        },
        RepositoryWorkspaceArgument::String("literal /logical/repo/out".into()),
        RepositoryWorkspaceArgument::Path(path.clone()),
    ]);
    let environment = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
        RepositoryEnvironmentEntry::new("EMPTY", "old"),
        RepositoryEnvironmentEntry::new("KEEP", "base"),
        RepositoryEnvironmentEntry::new("REMOVE", "base"),
    ]))
    .unwrap();
    let mut overrides = SmallMap::new();
    overrides.insert("REMOVE".into(), RepositoryWorkspaceEnvironmentValue::Remove);
    overrides.insert(
        "EMPTY".into(),
        RepositoryWorkspaceEnvironmentValue::Override("".into()),
    );
    let command = RepositoryWorkspaceExecute::new(
        argv.clone(),
        environment.clone(),
        overrides.clone(),
        600,
        false,
        path.clone(),
    )
    .unwrap();
    assert_eq!(command.effective_value("REMOVE"), None);
    assert_eq!(command.effective_value("EMPTY"), Some(""));
    assert_eq!(command.effective_value("KEEP"), Some("base"));
    assert_eq!(command.timeout_seconds(), 600);
    assert!(!command.quiet());
    assert_eq!(command.argv()[2].text(), "literal /logical/repo/out");
    assert_ne!(
        command,
        RepositoryWorkspaceExecute::new(
            argv.clone(),
            environment.clone(),
            overrides.clone(),
            30,
            false,
            path.clone()
        )
        .unwrap()
    );
    assert!(
        RepositoryWorkspaceExecute::new(
            argv.clone(),
            environment.clone(),
            overrides.clone(),
            0,
            false,
            path.clone()
        )
        .is_err()
    );
    assert!(
        RepositoryWorkspaceExecute::new(Arc::from([]), environment, overrides, 600, false, path)
            .is_err()
    );
}

#[test]
fn nonzero_exit_keeps_writes_deletions_modes_and_later_read() {
    let before = write(&transcript(), b"old").unwrap();
    let request = before.request(execute()).unwrap();
    let body: Arc<[u8]> = Arc::from(&b"new\xff"[..]);
    let next = before
        .complete(
            request,
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                23,
                Arc::from(&b"\0\xff"[..]),
                Arc::from(&b"stderr"[..]),
            )),
            RepositoryWorkspaceMode::Process,
            vec![
                root(),
                RepositoryWorkspaceEntry::new(
                    "produced".into(),
                    0o4755,
                    RepositoryWorkspaceNode::File(body.clone()),
                )
                .unwrap(),
            ],
        )
        .unwrap();
    assert!(next.current().entry("out").is_none());
    assert_eq!(next.current().entry("produced").unwrap().mode(), 0o4755);
    assert!(
        matches!(next.records()[1].result(), RepositoryWorkspaceResult::Execute(result)
        if result.return_code() == 23 && result.stdout() == b"\0\xff")
    );
    let read = next
        .request(RepositoryWorkspaceOperation::Read("produced".into()))
        .unwrap();
    let read = next
        .complete(
            read,
            RepositoryWorkspaceResult::Read(body),
            RepositoryWorkspaceMode::Process,
            next.current().entries().to_vec(),
        )
        .unwrap();
    assert_eq!(read.records().len(), 3);
}

#[test]
fn replay_requires_full_operation_index_predecessor_and_generation() {
    let initial = transcript();
    let left = write(&initial, b"left").unwrap();
    let right = write(&initial, b"right").unwrap();
    let request = left
        .request(RepositoryWorkspaceOperation::Exists("out".into()))
        .unwrap();
    assert_eq!(
        right.replay(&request).unwrap_err(),
        RepositoryWorkspaceError::ForeignPredecessor
    );
    let replay = left
        .request_at(
            0,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(&b"left"[..]),
                mode: 0o644,
            },
        )
        .unwrap();
    assert!(left.replay(&replay).unwrap().is_some());
    let changed = left
        .request_at(
            0,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(&b"wrong"[..]),
                mode: 0o644,
            },
        )
        .unwrap();
    assert_eq!(
        left.replay(&changed).unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    let foreign =
        RepositoryWorkspaceTranscript::new(reservation_with(8, 12, limits()), 0o755).unwrap();
    assert_eq!(
        foreign.replay(&replay).unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    assert_eq!(
        left.realize(0).unwrap_err(),
        RepositoryWorkspaceError::UnusedRecords
    );
    assert_eq!(left.realize(1).unwrap().final_state(), left.current());
}

#[test]
fn equal_argv_new_index_is_a_new_sample_not_replay() {
    let initial = transcript();
    let first = initial.request(execute()).unwrap();
    let next = initial
        .complete(
            first.clone(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            vec![root()],
        )
        .unwrap();
    assert!(next.replay(&first).unwrap().is_some());
    let second = next.request(execute()).unwrap();
    assert_eq!(second.index(), 1);
    assert!(next.replay(&second).unwrap().is_none());
    assert_ne!(first, second);
}

#[test]
fn state_rejects_missing_parent_duplicate_special_modes_and_escaping_link() {
    let t = transcript();
    let check = |entries: &[RepositoryWorkspaceEntry]| {
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            entries,
        )
    };
    assert!(check(&[root(), file("a/b", Arc::from([]))]).is_err());
    assert!(check(&[root(), file("a", Arc::from([])), file("a", Arc::from([]))]).is_err());
    assert!(
        RepositoryWorkspaceEntry::new("bad".into(), 0o10000, RepositoryWorkspaceNode::Directory)
            .is_err()
    );
    assert!(
        check(&[
            root(),
            RepositoryWorkspaceEntry::new(
                "link".into(),
                0o777,
                RepositoryWorkspaceNode::Link("../outside".into())
            )
            .unwrap()
        ])
        .is_err()
    );
    assert!(
        check(&[
            root(),
            RepositoryWorkspaceEntry::new(
                "link".into(),
                0o777,
                RepositoryWorkspaceNode::Link("/logical/repo/leaf".into())
            )
            .unwrap()
        ])
        .is_ok()
    );
    assert!(
        check(&[
            root(),
            RepositoryWorkspaceEntry::new(
                "link".into(),
                0o777,
                RepositoryWorkspaceNode::Link("/logical/repo-other/leaf".into())
            )
            .unwrap()
        ])
        .is_err()
    );
}

#[test]
fn process_entry_and_depth_exact_plus_one_do_not_lower_archive_admission() {
    let t = transcript();
    let mut entries = vec![root()];
    for i in 0..8191 {
        entries.push(file(&format!("f{i:05}"), Arc::from([])));
    }
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &entries
        )
        .is_ok()
    );
    entries.push(file("f99999", Arc::from([])));
    assert_eq!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &entries
        ),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Archive,
            &entries
        )
        .is_ok()
    );
    let mut deep = vec![root()];
    let mut path = String::new();
    for i in 0..64 {
        if i != 0 {
            path.push('/');
        }
        path.push('x');
        deep.push(
            RepositoryWorkspaceEntry::new(
                path.clone().into(),
                0o755,
                RepositoryWorkspaceNode::Directory,
            )
            .unwrap(),
        );
    }
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &deep
        )
        .is_ok()
    );
    path.push_str("/x");
    deep.push(
        RepositoryWorkspaceEntry::new(path.into(), 0o755, RepositoryWorkspaceNode::Directory)
            .unwrap(),
    );
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &deep
        )
        .is_err()
    );
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Archive,
            &deep
        )
        .is_ok()
    );
}

#[test]
fn equal_foreign_bodies_are_compared_then_replaced_without_double_retention() {
    let first = write(&transcript(), b"same").unwrap();
    let previous = match first.current().entry("out").unwrap().node() {
        RepositoryWorkspaceNode::File(body) => body,
        _ => unreachable!(),
    };
    let foreign: Arc<[u8]> = Arc::from(&b"same"[..]);
    assert!(!Arc::ptr_eq(previous, &foreign));
    let next = write(&first, &foreign).unwrap();
    let retained = match next.current().entry("out").unwrap().node() {
        RepositoryWorkspaceNode::File(body) => body,
        _ => unreachable!(),
    };
    assert!(Arc::ptr_eq(previous, retained));
    assert!(Arc::ptr_eq(&first.records()[0], &next.records()[0]));
    assert_eq!(next.retention().body_bytes(), 4);
    assert_eq!(write(&next, b"diff").unwrap().retention().body_bytes(), 8);
}

#[test]
fn execute_cwd_requires_selected_instance_and_repository_subtree_before_need() {
    let t = transcript();
    let command = |path, namespace| {
        RepositoryWorkspaceOperation::Execute(
            RepositoryWorkspaceExecute::new(
                Arc::from([RepositoryWorkspaceArgument::String("helper".into())]),
                RepositoryEnvironmentSnapshot::empty(),
                SmallMap::new(),
                600,
                true,
                HostRepositoryLabelPathValue::new(
                    NormalizedAbsolutePath::new(path).unwrap(),
                    namespace,
                )
                .unwrap(),
            )
            .unwrap(),
        )
    };
    let selected = PathObservationNamespace::Materialization(t.reservation().instance());
    assert!(t.request(command("/logical/repo", selected)).is_ok());
    assert!(t.request(command("/logical/repo/subdir", selected)).is_ok());
    assert_eq!(
        t.request(command(
            "/logical/repo",
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(12))
        ))
        .unwrap_err(),
        RepositoryWorkspaceError::InvalidOperation
    );
    assert_eq!(
        t.request(command("/logical/repo", PathObservationNamespace::Host))
            .unwrap_err(),
        RepositoryWorkspaceError::InvalidOperation
    );
    assert_eq!(
        t.request(command("/logical/repo-other", selected))
            .unwrap_err(),
        RepositoryWorkspaceError::InvalidOperation
    );
    assert_eq!(
        t.request(command("/tmp", PathObservationNamespace::Host))
            .unwrap_err(),
        RepositoryWorkspaceError::InvalidOperation
    );
}

#[test]
fn archive_deeper_than_256_preserves_lane_while_delete_and_process_keep_own_limits() {
    let t = transcript();
    let archive = crate::GeneratedRepositoryArchiveEffect::new(
        Arc::from([CompactString::from(
            "https://example.invalid/archive.tar.gz",
        )]),
        [0; 32],
        "".into(),
        crate::GeneratedRepositoryArchiveFormat::TarGz,
        "".into(),
        "".into(),
    )
    .unwrap();
    let mut entries = vec![root()];
    let mut path = String::new();
    let mut at_256 = String::new();
    for depth in 1..=257 {
        if depth != 1 {
            path.push('/');
        }
        path.push('x');
        entries.push(
            RepositoryWorkspaceEntry::new(
                path.clone().into(),
                0o755,
                RepositoryWorkspaceNode::Directory,
            )
            .unwrap(),
        );
        if depth == 256 {
            at_256.clone_from(&path);
        }
    }
    let archived = t
        .complete(
            t.request(RepositoryWorkspaceOperation::Archive(archive))
                .unwrap(),
            RepositoryWorkspaceResult::Archive([0; 32]),
            RepositoryWorkspaceMode::Archive,
            entries,
        )
        .unwrap();
    assert!(archived.current().entry(&path).is_some());
    assert!(
        archived
            .request(RepositoryWorkspaceOperation::Delete(at_256.into()))
            .is_ok()
    );
    assert!(
        archived
            .request(RepositoryWorkspaceOperation::Delete(path.into()))
            .is_err()
    );
    assert_eq!(
        archived.request(execute()).unwrap_err(),
        RepositoryWorkspaceError::StateLimit
    );
}

#[test]
fn argument_environment_archive_metadata_charge_actual_spare_storage_and_owner_arrays() {
    let mut spare = CompactString::with_capacity(8192);
    spare.push_str("same");
    let cwd = HostRepositoryLabelPathValue::new(
        NormalizedAbsolutePath::new("/logical/repo").unwrap(),
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(11)),
    )
    .unwrap();
    let command = |arg, environment| {
        RepositoryWorkspaceOperation::Execute(
            RepositoryWorkspaceExecute::new(
                Arc::from([RepositoryWorkspaceArgument::String(arg)]),
                environment,
                SmallMap::new(),
                600,
                true,
                cwd.clone(),
            )
            .unwrap(),
        )
    };
    let base = command("same".into(), RepositoryEnvironmentSnapshot::empty());
    let larger = command(spare, RepositoryEnvironmentSnapshot::empty());
    assert_eq!(base, larger);
    assert!(larger.metadata().unwrap() >= base.metadata().unwrap() + 8000);
    let mut name = CompactString::with_capacity(4096);
    name.push_str("ENV");
    let environment = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
        RepositoryEnvironmentEntry::new(name, "value"),
    ]))
    .unwrap();
    let with_env = command("same".into(), environment);
    assert!(
        with_env.metadata().unwrap()
            >= base.metadata().unwrap()
                + 4096
                + std::mem::size_of::<RepositoryEnvironmentEntry>()
                + 2 * std::mem::size_of::<usize>()
    );
    let mut url = CompactString::with_capacity(8192);
    url.push_str("https://example.invalid/archive.tar.gz");
    let mut id = CompactString::with_capacity(4096);
    id.push_str("id");
    let archive = RepositoryWorkspaceOperation::Archive(
        crate::GeneratedRepositoryArchiveEffect::new(
            Arc::from([url]),
            [0; 32],
            "".into(),
            crate::GeneratedRepositoryArchiveFormat::TarGz,
            id,
            "".into(),
        )
        .unwrap(),
    );
    assert!(archive.metadata().unwrap() >= 8192 + 4096);
}

#[test]
fn state_versions_metadata_and_retained_bodies_have_exact_plus_one_policies() {
    let t = transcript();
    let next = write(&t, b"abc").unwrap();
    let budget =
        RepositoryWorkspaceRetentionLimits::new(2, next.retention().metadata_bytes(), 3).unwrap();
    let exact = RepositoryWorkspaceTranscript::new(reservation_with(7, 11, budget), 0o755).unwrap();
    let exact = write(&exact, b"abc").unwrap();
    assert_eq!(exact.retention().state_versions(), 2);
    assert_eq!(
        write(&exact, b"abc").unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
    let too_small =
        RepositoryWorkspaceRetentionLimits::new(2, next.retention().metadata_bytes() - 1, 3)
            .unwrap();
    assert!(
        write(
            &RepositoryWorkspaceTranscript::new(reservation_with(7, 11, too_small), 0o755).unwrap(),
            b"abc"
        )
        .is_err()
    );
    let body_small =
        RepositoryWorkspaceRetentionLimits::new(4, limits().metadata_bytes(), 3).unwrap();
    let body_small =
        RepositoryWorkspaceTranscript::new(reservation_with(7, 11, body_small), 0o755).unwrap();
    let full = write(&body_small, b"abc").unwrap();
    assert!(write(&full, b"d").is_err());
    // Same bytes reuse the original allocation and do not consume another body.
    assert_eq!(write(&full, b"abc").unwrap().retention().body_bytes(), 3);
}

#[test]
fn fresh_empty_revoked_epochs_reject_stale_reservations_and_global_state_overflow() {
    let a = RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        7,
        limits(),
    )
    .unwrap();
    assert!(a.transcripts().unwrap().is_empty());
    let populated = a.reserve(reservation_with(7, 11, limits()), 0o755).unwrap();
    assert!(
        populated
            .reserve(reservation_with(7, 12, limits()), 0o755)
            .is_err()
    );
    let b = RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        8,
        limits(),
    )
    .unwrap();
    assert!(b.transcripts().unwrap().is_empty());
    assert_ne!(a, b);
    assert_eq!(
        b.reserve(reservation_with(7, 11, limits()), 0o755)
            .unwrap_err(),
        RepositoryWorkspaceError::ForeignReservation
    );
    let revoked = RepositoryWorkspaceEpoch::revoked();
    assert!(revoked.transcripts().is_none());
    assert_eq!(
        revoked
            .reserve(reservation_with(7, 11, limits()), 0o755)
            .unwrap_err(),
        RepositoryWorkspaceError::Revoked
    );
    assert!(!<RepositoryWorkspaceObservationKey as dice::Key>::validity(
        &revoked
    ));
    assert!(<RepositoryWorkspaceObservationKey as dice::Key>::validity(
        &a
    ));
    let small = RepositoryWorkspaceRetentionLimits::new(
        1,
        limits().metadata_bytes(),
        limits().archive_body_bytes(),
    )
    .unwrap();
    let one = RepositoryWorkspaceEpoch::fresh(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        7,
        small,
    )
    .unwrap()
    .reserve(reservation_with(7, 11, small), 0o755)
    .unwrap();
    let r = one
        .request_at(
            &reservation_with(7, 11, small),
            0,
            RepositoryWorkspaceOperation::Exists("".into()),
        )
        .unwrap();
    assert_eq!(
        one.complete(
            r,
            RepositoryWorkspaceResult::Exists(true),
            RepositoryWorkspaceMode::Archive,
            vec![root()]
        )
        .unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
}

#[test]
fn incorrect_read_write_delete_results_and_reverse_mode_are_not_certificates() {
    let t = write(&transcript(), b"abc").unwrap();
    let read = t
        .request(RepositoryWorkspaceOperation::Read("out".into()))
        .unwrap();
    assert_eq!(
        t.complete(
            read,
            RepositoryWorkspaceResult::Read(Arc::from(&b"wrong"[..])),
            RepositoryWorkspaceMode::Archive,
            t.current().entries().to_vec()
        )
        .unwrap_err(),
        RepositoryWorkspaceError::InvalidResult
    );
    let delete = t
        .request(RepositoryWorkspaceOperation::Delete("out".into()))
        .unwrap();
    assert!(
        t.complete(
            delete.clone(),
            RepositoryWorkspaceResult::Delete(false),
            RepositoryWorkspaceMode::Archive,
            vec![root()]
        )
        .is_err()
    );
    assert!(
        t.complete(
            delete,
            RepositoryWorkspaceResult::Delete(true),
            RepositoryWorkspaceMode::Archive,
            vec![root()]
        )
        .is_ok()
    );
    let exec = t.request(execute()).unwrap();
    assert_eq!(
        t.complete(
            exec,
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([])
            )),
            RepositoryWorkspaceMode::Archive,
            vec![root()]
        )
        .unwrap_err(),
        RepositoryWorkspaceError::InvalidMode
    );
}

#[test]
fn protocol_debug_redacts_body_argv_environment_and_captured_bytes() {
    let t = write(&transcript(), b"top-secret-body").unwrap();
    assert!(!format!("{t:?}").contains("top-secret"));
    let arg = RepositoryWorkspaceArgument::String("top-secret-arg".into());
    assert!(!format!("{arg:?}").contains("top-secret"));
    let result = RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
        0,
        Arc::from(&b"top-secret-stdout"[..]),
        Arc::from([]),
    ));
    assert!(!format!("{result:?}").contains("top-secret"));
}

#[test]
fn process_names_and_absolute_path_have_exact_plus_one_limits() {
    let t = transcript();
    let empty: Arc<[u8]> = Arc::from([]);
    let mut entries = vec![root()];
    for i in 0..4112 {
        entries.push(file(&format!("{i:05}{}", "x".repeat(250)), empty.clone()));
    }
    entries.push(file(&"y".repeat(16), empty.clone()));
    assert_eq!(
        entries.iter().map(|e| e.path().len()).sum::<usize>(),
        1024 * 1024
    );
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &entries
        )
        .is_ok()
    );
    entries.push(file("z", empty.clone()));
    assert_eq!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &entries
        ),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Archive,
            &entries
        )
        .is_ok()
    );
    let mut deep = vec![root()];
    let mut parent = String::new();
    for i in 0..15 {
        if i != 0 {
            parent.push('/');
        }
        parent.push_str(&"d".repeat(255));
        deep.push(
            RepositoryWorkspaceEntry::new(
                parent.clone().into(),
                0o755,
                RepositoryWorkspaceNode::Directory,
            )
            .unwrap(),
        );
    }
    let prefix = t
        .reservation()
        .display_root()
        .as_path()
        .to_str()
        .unwrap()
        .len()
        + 1;
    let leaf = format!("{parent}/{}", "f".repeat(4096 - prefix - parent.len() - 1));
    assert_eq!(prefix + leaf.len(), 4096);
    deep.push(file(&leaf, empty.clone()));
    assert!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &deep
        )
        .is_ok()
    );
    deep.pop();
    deep.push(file(&(leaf + "f"), empty));
    assert_eq!(
        RepositoryWorkspaceState::validate(
            t.reservation(),
            RepositoryWorkspaceMode::Process,
            &deep
        ),
        Err(RepositoryWorkspaceError::StateLimit)
    );
}

#[test]
fn process_body_exact_limit_and_new_version_plus_one_count_complete_history() {
    let initial = transcript();
    let body: Arc<[u8]> = vec![b'x'; REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES as usize].into();
    let complete = initial
        .complete(
            initial.request(execute()).unwrap(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            vec![root(), file("out", body.clone())],
        )
        .unwrap();
    assert_eq!(
        complete.retention().body_bytes(),
        REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES
    );
    // Replacing the large file by one byte leaves a tiny current tree but its
    // distinct retained body still exceeds cumulative64MiB. No refund on delete.
    assert_eq!(
        write(&complete, b"y").unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
    let too_big = vec![
        root(),
        file("out", body),
        file("plus", Arc::from(&b"y"[..])),
    ];
    assert_eq!(
        RepositoryWorkspaceState::validate(
            initial.reservation(),
            RepositoryWorkspaceMode::Process,
            &too_big
        ),
        Err(RepositoryWorkspaceError::StateLimit)
    );
    assert!(
        RepositoryWorkspaceState::validate(
            initial.reservation(),
            RepositoryWorkspaceMode::Archive,
            &too_big
        )
        .is_ok()
    );
}

#[test]
fn owner_layout_rounds_byte_tails_and_refuses_impossible_array_layouts() {
    let align = std::mem::align_of::<usize>();
    let header = 2 * std::mem::size_of::<usize>();
    for count in 0..=2 * align {
        let raw = header + count;
        let expected = (raw + align - 1) / align * align;
        assert_eq!(array_bytes::<u8>(count).unwrap(), expected);
    }
    assert!(array_bytes::<u8>(isize::MAX as usize).is_err());
    assert!(array_bytes::<RepositoryWorkspaceRecord>(usize::MAX).is_err());
}

#[test]
fn equal_foreign_predecessor_and_reservation_are_released_before_retention() {
    let process = |initial: &RepositoryWorkspaceTranscript| {
        let body: Arc<[u8]> = vec![b'x'; REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES as usize].into();
        let weak_body = Arc::downgrade(&body);
        let complete = initial
            .complete(
                initial.request(execute()).unwrap(),
                RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                    0,
                    Arc::from([]),
                    Arc::from([]),
                )),
                RepositoryWorkspaceMode::Process,
                vec![root(), file("out", body)],
            )
            .unwrap();
        (complete, weak_body)
    };
    let initial = transcript();
    let (current, _) = process(&initial);
    let foreign_initial = transcript();
    let (foreign, foreign_body) = process(&foreign_initial);
    let foreign_reservation = Arc::downgrade(foreign.reservation());
    let request = foreign
        .request(RepositoryWorkspaceOperation::Exists("out".into()))
        .unwrap();
    let foreign_state = Arc::downgrade(request.predecessor());
    assert_eq!(request.predecessor(), current.current());
    assert!(!Arc::ptr_eq(request.predecessor(), current.current()));
    drop(foreign);
    drop(foreign_initial);
    assert!(foreign_body.upgrade().is_some());
    let completed = current
        .complete(
            request,
            RepositoryWorkspaceResult::Exists(true),
            RepositoryWorkspaceMode::Process,
            current.current().entries().to_vec(),
        )
        .unwrap();
    assert!(foreign_body.upgrade().is_none());
    assert!(foreign_state.upgrade().is_none());
    assert!(foreign_reservation.upgrade().is_none());
    let retained = completed.records().last().unwrap().request();
    assert!(Arc::ptr_eq(retained.reservation(), current.reservation()));
    assert!(Arc::ptr_eq(retained.predecessor(), current.current()));
    assert_eq!(
        completed.retention().body_bytes(),
        REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES
    );
}

#[test]
fn template_replay_preserves_ordered_substitutions_even_when_rendered_bytes_equal() {
    let template = |reversed: bool| {
        let mut substitutions = SmallMap::new();
        let entries = if reversed {
            [("second", "last"), ("first", "second")]
        } else {
            [("first", "second"), ("second", "last")]
        };
        for (from, to) in entries {
            substitutions.insert(from.into(), to.into());
        }
        RepositoryWorkspaceOperation::Template {
            path: "out".into(),
            body: Arc::from(&b"unchanged"[..]),
            mode: 0o644,
            source: host_path("/template"),
            source_body: Arc::from(&b"unchanged"[..]),
            substitutions,
        }
    };
    let initial = transcript();
    let selected = initial.request(template(false)).unwrap();
    let completed = initial
        .complete(
            selected.clone(),
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            vec![root(), file("out", Arc::from(&b"unchanged"[..]))],
        )
        .unwrap();
    assert!(completed.replay(&selected).unwrap().is_some());
    let reordered = completed.request_at(0, template(true)).unwrap();
    assert_ne!(selected, reordered);
    assert_eq!(
        completed.replay(&reordered).unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    assert_ne!(
        completed,
        initial
            .complete(
                initial.request(template(true)).unwrap(),
                RepositoryWorkspaceResult::Unit,
                RepositoryWorkspaceMode::Archive,
                vec![root(), file("out", Arc::from(&b"unchanged"[..]))],
            )
            .unwrap()
    );
}

#[test]
fn execute_count_exact_limit_replays_prior_indices_and_refuses_new_plus_one() {
    let mut completed = transcript();
    for _ in 0..128 {
        completed = completed
            .complete(
                completed.request(execute()).unwrap(),
                RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                    0,
                    Arc::from([]),
                    Arc::from([]),
                )),
                RepositoryWorkspaceMode::Process,
                vec![root()],
            )
            .unwrap();
    }
    assert_eq!(completed.records().len(), 128);
    assert_eq!(
        completed.request(execute()).unwrap_err(),
        RepositoryWorkspaceError::StateLimit
    );
    for index in [0, 127] {
        let replay = completed.request_at(index, execute()).unwrap();
        assert!(completed.replay(&replay).unwrap().is_some());
    }
    // Non-execute operations do not consume the execute counter.
    assert!(
        completed
            .request(RepositoryWorkspaceOperation::Exists("".into()))
            .is_ok()
    );
}

#[test]
fn normal_exit_and_each_output_stream_have_independent_exact_plus_one_limits() {
    let initial = transcript();
    let request = initial.request(execute()).unwrap();
    let stream: Arc<[u8]> = vec![0xff; 1024 * 1024].into();
    assert!(
        initial
            .complete(
                request.clone(),
                RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                    255,
                    stream.clone(),
                    stream.clone(),
                )),
                RepositoryWorkspaceMode::Process,
                vec![root()],
            )
            .is_ok()
    );
    let oversized: Arc<[u8]> = vec![0; 1024 * 1024 + 1].into();
    for (code, stdout, stderr) in [
        (-1, stream.clone(), stream.clone()),
        (256, stream.clone(), stream.clone()),
        (0, oversized.clone(), stream.clone()),
        (0, stream, oversized),
    ] {
        assert_eq!(
            initial
                .complete(
                    request.clone(),
                    RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                        code, stdout, stderr,
                    )),
                    RepositoryWorkspaceMode::Process,
                    vec![root()],
                )
                .unwrap_err(),
            RepositoryWorkspaceError::InvalidResult
        );
    }
}

#[tokio::test]
async fn dice_observes_completed_state_fresh_commands_revocation_and_workspace_isolation() {
    use dice::DetectCycles;
    use dice::Dice;
    use dice::Key;

    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let other_workspace = NormalizedAbsolutePath::new("/other-workspace").unwrap();
    let injected = RepositoryWorkspaceEpochKey::new(workspace.clone());
    let observed = RepositoryWorkspaceObservationKey::new(workspace.clone());
    let other_injected = RepositoryWorkspaceEpochKey::new(other_workspace.clone());
    let other_observed = RepositoryWorkspaceObservationKey::new(other_workspace.clone());
    let fresh = RepositoryWorkspaceEpoch::fresh(workspace.clone(), 7, limits()).unwrap();
    let isolated = RepositoryWorkspaceEpoch::fresh(other_workspace, 70, limits()).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to([
            (injected.clone(), fresh.clone()),
            (other_injected, isolated.clone()),
        ])
        .unwrap();
    let mut transaction = updater.commit().await;
    let initial = transaction.compute(&observed).await.unwrap();
    assert_eq!(initial, fresh);
    assert!(initial.transcripts().unwrap().is_empty());
    assert!(RepositoryWorkspaceObservationKey::validity(&initial));
    assert_eq!(
        transaction.compute(&other_observed).await.unwrap(),
        isolated
    );
    drop(transaction);

    let reservation = reservation_with(7, 11, limits());
    let reserved = initial.reserve(reservation.clone(), 0o755).unwrap();
    let request = reserved
        .request_at(
            &reservation,
            0,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(&b"observed"[..]),
                mode: 0o644,
            },
        )
        .unwrap();
    let completed = reserved
        .complete(
            request.clone(),
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            vec![root(), file("out", Arc::from(&b"observed"[..]))],
        )
        .unwrap();
    let next_command = RepositoryWorkspaceEpoch::fresh(workspace.clone(), 8, limits()).unwrap();
    let restored = RepositoryWorkspaceEpoch::fresh(workspace, 9, limits()).unwrap();
    // Each version uses the same actual observation key. A stale cached initial
    // or completed value fails the structural assertion, including restoration
    // after the deliberately invalid revoked observation.
    for epoch in [
        completed.clone(),
        next_command.clone(),
        RepositoryWorkspaceEpoch::revoked(),
        restored.clone(),
    ] {
        let mut updater = dice.updater();
        updater
            .changed_to([(injected.clone(), epoch.clone())])
            .unwrap();
        let mut transaction = updater.commit().await;
        let actual = transaction.compute(&observed).await.unwrap();
        assert_eq!(actual, epoch);
        assert_eq!(
            RepositoryWorkspaceObservationKey::validity(&actual),
            actual.lease().is_some()
        );
        assert_eq!(
            transaction.compute(&other_observed).await.unwrap(),
            isolated
        );
        if actual == completed {
            let transcript = &actual.transcripts().unwrap()[0];
            assert_eq!(transcript.records().len(), 1);
            assert!(transcript.replay(&request).unwrap().is_some());
            assert_eq!(
                transcript.current().entry("out").unwrap().node(),
                &RepositoryWorkspaceNode::File(Arc::from(&b"observed"[..]))
            );
        } else if actual.lease().is_some() {
            assert!(actual.transcripts().unwrap().is_empty());
            assert_eq!(
                actual.reserve(reservation.clone(), 0o755).unwrap_err(),
                RepositoryWorkspaceError::ForeignReservation
            );
            assert_eq!(
                actual
                    .complete(
                        request.clone(),
                        RepositoryWorkspaceResult::Unit,
                        RepositoryWorkspaceMode::Archive,
                        vec![root(), file("out", Arc::from(&b"observed"[..]))],
                    )
                    .unwrap_err(),
                RepositoryWorkspaceError::ForeignReservation
            );
        } else {
            assert!(actual.transcripts().is_none());
            assert_eq!(
                actual
                    .request_at(
                        &reservation,
                        0,
                        RepositoryWorkspaceOperation::Exists("".into())
                    )
                    .unwrap_err(),
                RepositoryWorkspaceError::Revoked
            );
        }
    }
    assert_ne!(initial, completed);
    assert_ne!(next_command, restored);
}

#[test]
fn native_storage_forecast_covers_initial_unique_and_reused_completion() {
    let mut t = transcript();
    assert_eq!(
        RepositoryWorkspaceTranscript::initial_storage_bytes(t.reservation(), 0o755).unwrap(),
        t.retained_storage_bytes().unwrap(),
    );
    // Unique bodies and actual byte-equal foreign Arcs use the same producer
    // forecast; semantic completion still performs its own canonicalization.
    for body in [b"first".as_slice(), b"different", b"first"] {
        let request = t
            .request(RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(body),
                mode: 0o644,
            })
            .unwrap();
        let entries = vec![root(), file("out", Arc::from(body))];
        let forecast = t
            .completion_storage_bytes(
                &request,
                &RepositoryWorkspaceResult::Unit,
                RepositoryWorkspaceMode::Archive,
                &entries,
            )
            .unwrap();
        let next = t
            .complete(
                request,
                RepositoryWorkspaceResult::Unit,
                RepositoryWorkspaceMode::Archive,
                entries,
            )
            .unwrap();
        assert!(forecast >= next.retained_storage_bytes().unwrap());
        t = next;
    }
    assert_eq!(
        t.retention().body_bytes(),
        (b"first".len() + b"different".len()) as u64
    );
}

#[test]
fn native_storage_forecast_refuses_foreign_replay_and_invalid_effects() {
    let t = transcript();
    let request = t
        .request(RepositoryWorkspaceOperation::File {
            path: "out".into(),
            body: Arc::from(b"required".as_slice()),
            mode: 0o644,
        })
        .unwrap();
    let bad_entries = vec![root(), file("out", Arc::from(b"different".as_slice()))];
    assert_eq!(
        t.completion_storage_bytes(
            &request,
            &RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            &bad_entries
        ),
        Err(RepositoryWorkspaceError::InvalidResult)
    );
    let foreign =
        RepositoryWorkspaceTranscript::new(reservation_with(8, 12, limits()), 0o755).unwrap();
    assert_eq!(
        foreign.completion_storage_bytes(
            &request,
            &RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            &bad_entries
        ),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    let entries = vec![root(), file("out", Arc::from(b"required".as_slice()))];
    let completed = t
        .complete(
            request.clone(),
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            entries.clone(),
        )
        .unwrap();
    assert_eq!(
        completed.completion_storage_bytes(
            &request,
            &RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            &entries
        ),
        Err(RepositoryWorkspaceError::WrongIndex)
    );
}

#[test]
fn native_epoch_forecasts_cover_fresh_reservation_and_exact_completed_append() {
    let reservation = reservation_with(7, 11, limits());
    let fresh = RepositoryWorkspaceEpoch::fresh_storage_bytes(reservation.workspace(), 7, limits())
        .unwrap();
    let epoch =
        RepositoryWorkspaceEpoch::fresh(reservation.workspace().clone(), 7, limits()).unwrap();
    assert!(fresh >= epoch.retained_storage_bytes().unwrap());
    let reserved = epoch.reserve_storage_bytes(&reservation, 0o755).unwrap();
    let epoch = epoch.reserve(reservation, 0o755).unwrap();
    assert!(reserved >= epoch.retained_storage_bytes().unwrap());
    let next = write(&epoch.transcripts().unwrap()[0], b"actual").unwrap();
    let forecast = epoch.publish_storage_bytes(&next).unwrap();
    let epoch = epoch.publish_completed(next).unwrap();
    assert!(forecast >= epoch.retained_storage_bytes().unwrap());
    assert_eq!(
        epoch.publish_completed(epoch.transcripts().unwrap()[0].clone()),
        Err(RepositoryWorkspaceError::WrongIndex)
    );
    assert_eq!(
        RepositoryWorkspaceEpoch::revoked().publish_storage_bytes(&epoch.transcripts().unwrap()[0]),
        Err(RepositoryWorkspaceError::Revoked)
    );
}

#[test]
fn native_epoch_publication_rejects_equal_final_state_with_rewritten_or_skipped_history() {
    let initial = transcript();
    let first = write(&initial, b"original prefix").unwrap();
    let epoch =
        RepositoryWorkspaceEpoch::fresh(first.reservation().workspace().clone(), 7, limits())
            .unwrap()
            .reserve(first.reservation().clone(), 0o755)
            .unwrap()
            .publish_completed(first.clone())
            .unwrap();
    let alternate = write(
        &write(&initial, b"foreign prefix").unwrap(),
        b"same last state",
    )
    .unwrap();
    let correct = write(&first, b"same last state").unwrap();
    assert_eq!(alternate.current(), correct.current());
    assert_eq!(
        epoch.publish_storage_bytes(&alternate),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    assert_eq!(
        epoch.publish_completed(alternate),
        Err(RepositoryWorkspaceError::ReplayConflict)
    );
    let skipped = write(&correct, b"third").unwrap();
    assert_eq!(
        epoch.publish_completed(skipped),
        Err(RepositoryWorkspaceError::WrongIndex)
    );
    let foreign =
        RepositoryWorkspaceTranscript::new(reservation_with(8, 12, limits()), 0o755).unwrap();
    assert_eq!(
        epoch.publish_completed(write(&foreign, b"same last state").unwrap()),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    assert!(epoch.publish_completed(correct).is_ok());
}

#[test]
fn native_epoch_publication_cannot_bypass_other_process_root_history_admission() {
    let a = reservation_with(7, 11, limits());
    let b = Arc::new(
        RepositoryWorkspaceReservation::new(
            a.workspace().clone(),
            a.owner().clone(),
            CanonicalRepoName::new("owner+other").unwrap(),
            4,
            NormalizedAbsolutePath::new("/logical/other").unwrap(),
            PathObservationInstanceId::new(12),
            7,
            limits(),
        )
        .unwrap(),
    );
    let epoch = RepositoryWorkspaceEpoch::fresh(a.workspace().clone(), 7, limits())
        .unwrap()
        .reserve(a.clone(), 0o755)
        .unwrap()
        .reserve(b.clone(), 0o755)
        .unwrap();
    let process = |reservation: &RepositoryWorkspaceReservation| {
        RepositoryWorkspaceOperation::Execute(
            RepositoryWorkspaceExecute::new(
                Arc::from([RepositoryWorkspaceArgument::String("generic-tool".into())]),
                RepositoryEnvironmentSnapshot::empty(),
                SmallMap::new(),
                600,
                true,
                HostRepositoryLabelPathValue::new(
                    reservation.display_root().clone(),
                    PathObservationNamespace::Materialization(reservation.instance()),
                )
                .unwrap(),
            )
            .unwrap(),
        )
    };
    let a0 = &epoch.transcripts().unwrap()[0];
    let a1 = a0
        .complete(
            a0.request(process(&a)).unwrap(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            vec![root(), file("out", Arc::from(vec![b'a'; 40 * 1024 * 1024]))],
        )
        .unwrap();
    let epoch = epoch.publish_completed(a1).unwrap();
    let b0 = &epoch.transcripts().unwrap()[1];
    let b1 = write(b0, &vec![b'b'; 40 * 1024 * 1024]).unwrap();
    let epoch = epoch.publish_completed(b1).unwrap();
    assert_eq!(
        epoch.request_at(&b, 1, process(&b)),
        Err(RepositoryWorkspaceError::RetentionLimit)
    );
    let b1 = &epoch.transcripts().unwrap()[1];
    let b2 = b1
        .complete(
            b1.request(process(&b)).unwrap(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            b1.current().entries().to_vec(),
        )
        .unwrap();
    // A standalone transcript is valid; only the current command knows about A.
    assert_eq!(
        epoch.publish_storage_bytes(&b2),
        Err(RepositoryWorkspaceError::RetentionLimit)
    );
    assert_eq!(
        epoch.publish_completed(b2),
        Err(RepositoryWorkspaceError::RetentionLimit)
    );
}
