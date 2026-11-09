use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::repository_physical_root::native_fixture;
use crate::runtime::repository_physical_root::native_tests::reservation;
use crate::runtime::repository_physical_root::tests::resources;
use crate::runtime::repository_physical_root::tests::scope;

pub(super) fn empty_builder(
    reservation: Arc<RepositoryWorkspaceReservation>,
    resources: Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryPhysicalRootBuilder {
    let file = native_fixture::root(RepositoryWorkspaceMode::Archive);
    // Trusted fixture preparation, before handing over the exclusive builder.
    let io = RootIo::new(
        &file,
        io_limits(RepositoryWorkspaceMode::Archive, &reservation),
    )
    .unwrap();
    let mut work = DeleteWork::default();
    for path in ["dir", "link"] {
        crate::runtime::repository_filesystem_progress::deletion::delete_at(
            &io,
            path,
            &mut work,
            &|| true,
            Some(scope),
        )
        .unwrap();
    }
    RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
        file,
        reservation,
        RepositoryWorkspaceMode::Archive,
        resources,
        scope,
        &|| true,
    )
    .unwrap()
}

pub(super) fn write(path: &str, body: &[u8]) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::File {
        path: path.into(),
        body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(body)),
        mode: 0o644,
    }
}
pub(super) fn apply(
    progress: RepositoryWorkspaceProgress,
    operation: RepositoryWorkspaceOperation,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceProgress {
    let request = progress.transcript().request(operation).unwrap();
    progress.apply(request, scope, &|| true).unwrap()
}
pub(super) fn assert_released(resources: &RepositoryRootResources, baseline: usize, fd: i32) {
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    let now = resources.snapshot();
    assert_eq!(now.retained_bytes, baseline);
    assert_eq!(now.native_fds, 0);
    assert_eq!(now.backing_bytes, 0);
}

// Pure comparison inputs only: callers prepay codec, graph and body overlap.
// Reconstructing a transcript here confers no physical-root authority.
pub(super) fn packed_file_operation() -> RepositoryWorkspaceOperation {
    let mut builder = slug_bzlmod_v2::RepositoryWorkspaceBodyBuilder::new(1024).unwrap();
    builder
        .append(&[b'p'; 1024], &mut |_| Ok::<_, ()>(()))
        .unwrap();
    RepositoryWorkspaceOperation::File {
        path: "out".into(),
        body: builder.finish().unwrap(),
        mode: 0o644,
    }
}

pub(super) fn foreign_file_realization(
    native: &RepositoryWorkspaceTranscript,
    extra_history: bool,
) -> (
    RepositoryWorkspaceRealization,
    slug_bzlmod_v2::RepositoryWorkspaceBodyWeak,
) {
    assert_eq!(native.records().len(), 1);
    let RepositoryWorkspaceNode::File(original) = native.current().entry("out").unwrap().node()
    else {
        panic!()
    };
    assert!(original.raw_arc().is_none());
    assert!(original.native_storage_bytes() < original.len());
    let mut bytes = vec![0; original.len()];
    assert!(original.copy_into(0, &mut bytes));
    let mut builder = slug_bzlmod_v2::RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
    for chunk in bytes.chunks(64 * 1024) {
        builder.append(chunk, &mut |_| Ok::<_, ()>(())).unwrap();
    }
    let body = builder.finish().unwrap();
    assert!(body.raw_arc().is_none());
    assert!(body.native_storage_bytes() < body.len());
    assert!(!body.ptr_eq(original));
    let weak = body.downgrade();
    let entries = native
        .current()
        .entries()
        .iter()
        .map(|entry| {
            slug_bzlmod_v2::RepositoryWorkspaceEntry::new(
                entry.path().into(),
                entry.mode(),
                if entry.path() == "out" {
                    RepositoryWorkspaceNode::File(body.clone())
                } else {
                    entry.node().clone()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let initial = RepositoryWorkspaceTranscript::new(
        native.reservation().clone(),
        native.current().entries()[0].mode(),
    )
    .unwrap();
    let request = initial
        .request(RepositoryWorkspaceOperation::File {
            path: "out".into(),
            body,
            mode: 0o644,
        })
        .unwrap();
    let mut foreign = initial
        .complete(
            request,
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            entries.clone(),
        )
        .unwrap();
    assert_eq!(&foreign, native);
    assert!(!Arc::ptr_eq(foreign.current(), native.current()));
    if extra_history {
        let request = foreign
            .request(RepositoryWorkspaceOperation::Exists("absent".into()))
            .unwrap();
        foreign = foreign
            .complete(
                request,
                RepositoryWorkspaceResult::Exists(false),
                RepositoryWorkspaceMode::Archive,
                entries,
            )
            .unwrap();
        assert_ne!(&foreign, native);
        assert_eq!(foreign.current().entries(), native.current().entries());
    }
    (foreign.realize(foreign.records().len()).unwrap(), weak)
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_progress_closed_realization_keeps_canonical_owners_and_record_validation() {
    for extra_history in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let selected = reservation(&directory.path().join("root"), 946);
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let scope = scope();
        let producer = bank.reserve(4 * 1024 * 1024, 0, 0).unwrap();
        let builder = empty_builder(selected, bank.clone(), &scope);
        let fd = builder.file.as_raw_fd();
        let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
        let progress = apply(progress, packed_file_operation(), &scope);
        let state = progress.transcript().current().clone();
        let (foreign, weak) = foreign_file_realization(progress.transcript(), extra_history);
        let identity = progress
            .transcript()
            .realize(1)
            .unwrap()
            .source_identity()
            .clone();
        assert_eq!(foreign.source_identity(), &identity);
        let result = progress.seal_realization(&foreign, &scope, &|| true);
        if extra_history {
            assert!(matches!(
                result,
                Err(PhysicalRootError::Protocol(
                    slug_bzlmod_v2::RepositoryWorkspaceError::ForeignPredecessor
                ))
            ));
        } else {
            let root = result.unwrap();
            assert!(Arc::ptr_eq(root.state(), &state));
            drop(foreign);
            assert!(weak.upgrade().is_none());
            let mut actual = Vec::new();
            root.open_member("out", &scope, &|| true)
                .unwrap()
                .read_to_end(&mut actual)
                .unwrap();
            assert_eq!(actual, [b'p'; 1024]);
            drop(root);
            drop(state);
            drop(producer);
            assert_released(&bank, baseline, fd);
            continue;
        }
        drop((foreign, state));
        assert!(weak.upgrade().is_none());
        drop(producer);
        assert_released(&bank, baseline, fd);
    }
    let directory = tempfile::tempdir().unwrap();
    let selected = reservation(&directory.path().join("root"), 947);
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let builder = empty_builder(selected, bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let progress = apply(progress, write("out", b"one"), &scope);
    assert!(matches!(
        progress.seal(0, &scope, &|| true),
        Err(PhysicalRootError::Protocol(
            slug_bzlmod_v2::RepositoryWorkspaceError::UnusedRecords
        ))
    ));
    assert_released(&bank, baseline, fd);
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_ordered_progress_samples_replays_and_seals_without_host_path() {
    let host = tempfile::tempdir().unwrap();
    let reference = host.path().join("reference-dir");
    std::fs::create_dir(&reference).unwrap();
    let expected_parent_mode = (std::fs::metadata(&reference).unwrap().mode() & 0o777) as u16;
    let display = host.path().join("foreign-display");
    std::fs::create_dir(&display).unwrap();
    std::fs::write(display.join("sentinel"), b"foreign host").unwrap();
    let reservation = reservation(&display, 941);
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let builder = empty_builder(reservation.clone(), bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let first = progress
        .transcript()
        .request(write("nested/body", b"first\0\xff"))
        .unwrap();
    let progress = progress.apply(first.clone(), &scope, &|| true).unwrap();
    assert_eq!(
        progress
            .transcript()
            .current()
            .entry("nested")
            .unwrap()
            .mode(),
        expected_parent_mode
    );
    let progress = apply(progress, write("nested/body", b"second"), &scope);
    let records = progress.transcript().records().len();
    let progress = progress.apply(first, &scope, &|| true).unwrap();
    assert_eq!(progress.transcript().records().len(), records);
    assert!(
        matches!(progress.transcript().current().entry("nested/body").unwrap().node(), RepositoryWorkspaceNode::File(body) if body.equals_slice(b"second"))
    );
    let source = HostRepositoryLabelPathValue::new(
        NormalizedAbsolutePath::new("/tracked/template-source").unwrap(),
        PathObservationNamespace::Host,
    )
    .unwrap();
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Template {
            path: "rendered".into(),
            body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(
                b"literal template".as_slice(),
            )),
            mode: 0o755,
            source,
            source_body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(
                b"literal template".as_slice(),
            )),
            substitutions: SmallMap::new(),
        },
        &scope,
    );
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Read("nested/body".into()),
        &scope,
    );
    assert!(
        matches!(progress.transcript().records().last().unwrap().result(), RepositoryWorkspaceResult::Read(body) if body.equals_slice(b"second"))
    );
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Exists("absent".into()),
        &scope,
    );
    assert!(matches!(
        progress.transcript().records().last().unwrap().result(),
        RepositoryWorkspaceResult::Exists(false)
    ));
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Exists("rendered/child".into()),
        &scope,
    );
    assert!(matches!(
        progress.transcript().records().last().unwrap().result(),
        RepositoryWorkspaceResult::Exists(false)
    ));
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Delete("nested".into()),
        &scope,
    );
    let progress = apply(
        progress,
        RepositoryWorkspaceOperation::Delete("nested".into()),
        &scope,
    );
    assert!(matches!(
        progress.transcript().records().last().unwrap().result(),
        RepositoryWorkspaceResult::Delete(false)
    ));
    let count = progress.transcript().records().len();
    let sealed = progress.seal(count, &scope, &|| true).unwrap();
    assert_eq!(sealed.state().entry("rendered").unwrap().mode(), 0o755);
    assert!(sealed.state().entry("nested").is_none());
    assert_eq!(
        std::fs::read(display.join("sentinel")).unwrap(),
        b"foreign host"
    );
    assert!(!display.join("rendered").exists());
    drop(sealed);
    assert_released(&bank, baseline, fd);
}

#[test]
#[ignore = "requires the explicit trusted bwrap tmpfs/SCM_RIGHTS fixture"]
fn native_progress_refusals_retire_the_entire_generation() {
    let directory = tempfile::tempdir().unwrap();
    let reservation = reservation(&directory.path().join("absent"), 942);
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    // A nonempty imported root cannot fabricate an initial transcript.
    let input = native_fixture::root(RepositoryWorkspaceMode::Archive);
    let fd = input.as_raw_fd();
    let builder = RepositoryPhysicalRootBuilder::from_quiescent_tmpfs(
        input,
        reservation.clone(),
        RepositoryWorkspaceMode::Archive,
        bank.clone(),
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(matches!(
        RepositoryWorkspaceProgress::new(builder, &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    assert_released(&bank, baseline, fd);
    // RootIo may have created a parent before an unadmitted mode is rejected;
    // consuming refusal drops that partially mutated backing and every credit.
    let builder = empty_builder(reservation.clone(), bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let bad = progress
        .transcript()
        .request(RepositoryWorkspaceOperation::File {
            path: "partial/body".into(),
            body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(b"bytes".as_slice())),
            mode: 0o4644,
        })
        .unwrap();
    assert!(progress.apply(bad, &scope, &|| true).is_err());
    assert_released(&bank, baseline, fd);
    let builder = empty_builder(reservation.clone(), bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let request = progress
        .transcript()
        .request(write("cancelled", b"bytes"))
        .unwrap();
    assert!(matches!(
        progress.apply(request, &scope, &|| false),
        Err(PhysicalRootError::Inactive)
    ));
    assert_released(&bank, baseline, fd);
    let builder = empty_builder(reservation, bank.clone(), &scope);
    let fd = builder.file.as_raw_fd();
    let progress = RepositoryWorkspaceProgress::new(builder, &scope, &|| true).unwrap();
    let first = progress
        .transcript()
        .request(write("body", b"first"))
        .unwrap();
    let progress = progress.apply(first, &scope, &|| true).unwrap();
    let conflict = progress
        .transcript()
        .request_at(0, write("body", b"conflict"))
        .unwrap();
    assert!(matches!(
        progress.apply(conflict, &scope, &|| true),
        Err(PhysicalRootError::Protocol(
            slug_bzlmod_v2::RepositoryWorkspaceError::ReplayConflict
        ))
    ));
    assert_released(&bank, baseline, fd);
}
