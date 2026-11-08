use std::os::fd::AsRawFd;

use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::repository_physical_root::mount_lease::quota::initialized_archive_for_test;
use crate::runtime::repository_physical_root::native_tests::reservation;
use crate::runtime::repository_physical_root::tests::resources;
use crate::runtime::repository_physical_root::tests::scope;

fn execute(selected: &RepositoryWorkspaceReservation) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::Execute(
        RepositoryWorkspaceExecute::new(
            Arc::from([RepositoryWorkspaceArgument::String(
                "unrelated-payload".into(),
            )]),
            RepositoryEnvironmentSnapshot::empty(),
            SmallMap::new(),
            600,
            true,
            HostRepositoryLabelPathValue::new(
                selected.display_root().clone(),
                PathObservationNamespace::Materialization(selected.instance()),
            )
            .unwrap(),
        )
        .unwrap(),
    )
}
fn release(bank: &RepositoryRootResources, baseline: usize, fd: i32) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    let now = bank.snapshot();
    assert_eq!(
        (now.retained_bytes, now.native_fds, now.backing_bytes),
        (baseline, 0, 0)
    );
}
fn start(
    selected: Arc<RepositoryWorkspaceReservation>,
    bank: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
) -> (RepositoryWorkspaceCommand, i32) {
    let (builder, initial) = initialized_archive_for_test(selected.clone(), &bank, &scope).unwrap();
    let fd = builder.file.as_raw_fd();
    let command = RepositoryWorkspaceCommand::new(
        selected.workspace().clone(),
        selected.lease(),
        selected.limits(),
        bank,
        scope.clone(),
        &|| true,
    )
    .unwrap()
    .reserve_root(builder, initial, &scope, &|| true)
    .unwrap();
    (command, fd)
}
fn write_one(
    command: RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceCommand {
    let request = command
        .request(
            selected,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(b"one".as_slice()),
                mode: 0o644,
            },
            scope,
            &|| true,
        )
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    command
        .apply(
            request,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap()
}

#[test]
fn command_creation_is_fresh_and_scope_refusal_precedes_native_allocation() {
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let canceled = scope();
    canceled.cancel();
    assert!(matches!(
        RepositoryWorkspaceCommand::new(
            NormalizedAbsolutePath::new("/command").unwrap(),
            7,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap(),
            bank.clone(),
            canceled,
            &|| true
        ),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    let scope = scope();
    assert!(matches!(
        RepositoryWorkspaceCommand::new(
            NormalizedAbsolutePath::new("/other-command").unwrap(),
            7,
            RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap(),
            bank.clone(),
            scope.clone(),
            &|| true,
        ),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::ForeignScope
        ))
    ));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    let command = RepositoryWorkspaceCommand::new(
        scope.workspace().clone(),
        7,
        RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, ARCHIVE_BYTES).unwrap(),
        bank.clone(),
        scope,
        &|| true,
    )
    .unwrap();
    assert_eq!(command.epoch().lease(), Some(7));
    assert!(command.epoch().transcripts().unwrap().is_empty());
    drop(command);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
#[ignore = "requires actual archive root/initial seed under trusted Rust bwrap fixture"]
fn native_command_seal_retires_seed_and_exact_replay_keeps_epoch_unchanged() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1801);
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let producer = bank.reserve(1024 * 1024, 0, 0).unwrap();
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let command = write_one(command, &selected, &scope);
    let epoch = command.epoch().clone();
    let replay = command
        .request_at(
            &selected,
            0,
            RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: Arc::from(b"one".as_slice()),
                mode: 0o644,
            },
            &scope,
            &|| true,
        )
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let command = command
        .apply(
            replay,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    assert_eq!(command.epoch(), &epoch);
    assert_eq!(bank.snapshot().native_fds, 3);
    let (command, sealed) = command.seal_root(&selected, 1, &scope, &|| true).unwrap();
    assert_eq!(bank.snapshot().native_fds, 1);
    assert!(
        matches!(sealed.state().entry("out").unwrap().node(),RepositoryWorkspaceNode::File(body)if body.as_ref()==b"one")
    );
    assert!(
        command
            .request(&selected, execute(&selected), &scope, &|| true)
            .is_err()
    );
    drop(sealed);
    drop(command);
    drop(epoch);
    drop(producer);
    release(&bank, baseline, fd);
}

#[test]
#[ignore = "requires actual archive root/initial seed under trusted Rust bwrap fixture"]
fn native_command_refuses_rewritten_epoch_and_actual_tree_drift_before_quota_setup() {
    for drift in [false, true] {
        let fixture = tempfile::Builder::new()
            .prefix("slug-wp425-rolling-")
            .tempdir()
            .unwrap();
        let display = fixture.path().join("root");
        std::fs::create_dir(&display).unwrap();
        let selected = reservation(&display, 1802 + u64::from(drift));
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let scope = scope();
        let producer = bank.reserve(1024 * 1024, 0, 0).unwrap();
        let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
        let mut command = write_one(command, &selected, &scope);
        let request = command
            .request(&selected, execute(&selected), &scope, &|| true)
            .unwrap();
        if drift {
            let _scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
            let root = &command.roots[0].progress.as_ref().unwrap().builder;
            RootIo::new(&root.file, io_limits(root.mode, &selected))
                .unwrap()
                .write_file(
                    Path::new("out"),
                    b"bad",
                    0o644,
                    WriteMode::ReplaceRegular,
                    &|| true,
                )
                .unwrap();
        } else {
            // Same final tree, different complete history. Only tests can replace
            // this private slot; ordinary callers cannot import an old epoch.
            let initial = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
            let mut alternate = RepositoryWorkspaceEpoch::fresh(
                selected.workspace().clone(),
                selected.lease(),
                selected.limits(),
            )
            .unwrap()
            .reserve(selected.clone(), 0o755)
            .unwrap();
            let mut transcript = initial;
            for bytes in [b"two".as_slice(), b"one".as_slice()] {
                let req = transcript
                    .request(RepositoryWorkspaceOperation::File {
                        path: "out".into(),
                        body: Arc::from(bytes),
                        mode: 0o644,
                    })
                    .unwrap();
                transcript = transcript
                    .complete(
                        req,
                        RepositoryWorkspaceResult::Unit,
                        RepositoryWorkspaceMode::Archive,
                        vec![
                            slug_bzlmod_v2::RepositoryWorkspaceEntry::new(
                                "".into(),
                                0o755,
                                RepositoryWorkspaceNode::Directory,
                            )
                            .unwrap(),
                            slug_bzlmod_v2::RepositoryWorkspaceEntry::new(
                                "out".into(),
                                0o644,
                                RepositoryWorkspaceNode::File(Arc::from(bytes)),
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap();
                alternate = alternate.publish_completed(transcript.clone()).unwrap();
            }
            assert_eq!(
                alternate.transcripts().unwrap()[0].current(),
                command.roots[0]
                    .progress
                    .as_ref()
                    .unwrap()
                    .transcript()
                    .current()
            );
            let credit = bank
                .reserve(alternate.retained_storage_bytes().unwrap(), 0, 0)
                .unwrap();
            command.epoch = CreditedEpoch {
                epoch: alternate,
                _credit: credit,
            };
        }
        let result = command.admit_first_execute(request, &scope, &|| true);
        if drift {
            assert!(matches!(result, Err(PhysicalRootError::Snapshot)));
        } else {
            assert!(matches!(
                result,
                Err(PhysicalRootError::Protocol(
                    slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor
                ))
            ));
        }
        drop(producer);
        release(&bank, baseline, fd);
    }
}

#[test]
#[ignore = "requires actual Archive history above Process bound; finite 64MiB+1 body fixture"]
fn native_command_first_execute_checks_deleted_history_without_lowering_archive_admission() {
    use std::time::Duration;
    use std::time::Instant;

    use crate::runtime::dice::source_service::SourceServiceLimits;
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1804);
    // Positive overlap allowance for input Vec→Arc, actual snapshot and both
    // old/new immutable epoch guards. This is not a whole-command RSS baseline.
    let bank = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(1024 * 1024 * 1024, 32, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let deadline = Instant::now() + Duration::from_secs(26);
    let scope = SourceServiceScope::new_for_test(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let size =
        usize::try_from(slug_bzlmod_v2::REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES + 1).unwrap();
    let producer = bank.reserve(2 * size + 1024 * 1024, 0, 0).unwrap();
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let body: Arc<[u8]> = Arc::from(vec![b'x'; size]);
    let request = command
        .request(
            &selected,
            RepositoryWorkspaceOperation::File {
                path: "large".into(),
                body,
                mode: 0o644,
            },
            &scope,
            &|| true,
        )
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let command = command
        .apply(
            request,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    let request = command
        .request(
            &selected,
            RepositoryWorkspaceOperation::Delete("large".into()),
            &scope,
            &|| true,
        )
        .unwrap();
    let command = command
        .apply(
            request,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    let transcript = &command.epoch().transcripts().unwrap()[0];
    assert_eq!(transcript.current().entries().len(), 1);
    assert_eq!(transcript.retention().body_bytes(), size as u64);
    assert!(matches!(
        command.request(&selected, execute(&selected), &scope, &|| true),
        Err(PhysicalRootError::Protocol(
            slug_bzlmod_v2::RepositoryWorkspaceError::RetentionLimit
        ))
    ));
    // The valid Archive-only result still seals after Process admission refuses.
    let (command, sealed) = command.seal_root(&selected, 2, &scope, &|| true).unwrap();
    assert_eq!(sealed.state().mode(), RepositoryWorkspaceMode::Archive);
    assert_eq!(sealed.state().entries().len(), 1);
    drop(sealed);
    drop(command);
    drop(producer);
    release(&bank, baseline, fd);
}
