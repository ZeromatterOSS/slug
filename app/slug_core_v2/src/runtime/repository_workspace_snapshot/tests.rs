//! Ordinary directory fixtures test only the borrowed sampler kernel. They do
//! not pass physical-root admission or activate a Host-path production route.
use std::cell::Cell;
use std::fs;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;

use super::super::native_tests::reservation;
use super::super::tests::scope;
use super::*;

#[path = "cross_path_tests.rs"]
mod cross_path_tests;

#[path = "framed_stream_tests.rs"]
mod framed_stream_tests;

#[path = "observed_comparison_tests.rs"]
mod observed_comparison_tests;

fn resources() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(256 * 1024 * 1024, 16, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap()
}
fn root_entry() -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory).unwrap()
}
fn file_entry(path: &str, body: &[u8]) -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new(
        path.into(),
        0o644,
        RepositoryWorkspaceNode::File(Arc::<[u8]>::from(body).into()),
    )
    .unwrap()
}
fn archive() -> RepositoryWorkspaceOperation {
    // Immutable fixture certificate only; no archive execution is claimed.
    RepositoryWorkspaceOperation::Archive(
        GeneratedRepositoryArchiveEffect::new(
            Arc::from(["https://example.invalid/snapshot-fixture.tar.gz".into()]),
            Some([0; 32]),
            "".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "snapshot-fixture".into(),
            "".into(),
        )
        .unwrap(),
    )
}
fn incompressible(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}
fn prior(
    selected: Arc<RepositoryWorkspaceReservation>,
    entries: Vec<RepositoryWorkspaceEntry>,
) -> RepositoryWorkspaceTranscript {
    let initial = RepositoryWorkspaceTranscript::new(selected, 0o755).unwrap();
    initial
        .complete(
            initial.request(archive()).unwrap(),
            RepositoryWorkspaceResult::Archive([0; 32]),
            RepositoryWorkspaceMode::Archive,
            entries,
        )
        .unwrap()
}
fn body<'a>(entries: &'a [RepositoryWorkspaceEntry], path: &str) -> &'a RepositoryWorkspaceBody {
    match entries.iter().find(|e| e.path() == path).unwrap().node() {
        RepositoryWorkspaceNode::File(body) => body,
        _ => panic!(),
    }
}
fn sample<'a>(
    io: &RootIo<'_>,
    selected: &RepositoryWorkspaceReservation,
    mode: RepositoryWorkspaceMode,
    prior: &'a RepositoryWorkspaceState,
    bank: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedWorkspaceSnapshot<'a>, PhysicalRootError> {
    let _scratch = bank.reserve(IO_SCRATCH, 3, 0)?;
    capture_at(
        io,
        selected,
        mode,
        prior,
        bank,
        scope,
        active,
        Limits::for_mode(mode),
    )
}
fn prepare(root: &Path) {
    fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn snapshot_samples_actual_changes_deletions_modes_links_and_reuses_only_observed_equal_body() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    fs::write(fixture.path().join("changed"), b"OLD").unwrap();
    fs::write(fixture.path().join("keep"), b"same\0\xff").unwrap();
    fs::write(fixture.path().join("removed"), b"gone").unwrap();
    let selected = reservation(Path::new("/absent/snapshot-root"), 201);
    let old = prior(
        selected.clone(),
        vec![
            root_entry(),
            file_entry("changed", b"OLD"),
            file_entry("keep", b"same\0\xff"),
            file_entry("removed", b"gone"),
        ],
    );
    let same = body(old.current().entries(), "keep").clone();
    let changed = body(old.current().entries(), "changed").clone();
    fs::write(fixture.path().join("changed"), b"NEW").unwrap();
    fs::remove_file(fixture.path().join("removed")).unwrap();
    fs::set_permissions(
        fixture.path().join("keep"),
        fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    fs::create_dir(fixture.path().join("newdir")).unwrap();
    fs::write(fixture.path().join("newdir/file"), b"actual").unwrap();
    symlink("newdir/../keep", fixture.path().join("link")).unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let captured = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(RepositoryWorkspaceBody::ptr_eq(
        &same,
        body(captured.entries(), "keep")
    ));
    assert!(!RepositoryWorkspaceBody::ptr_eq(
        &changed,
        body(captured.entries(), "changed")
    ));
    assert!(body(captured.entries(), "changed").equals_slice(b"NEW"));
    assert!(body(captured.entries(), "newdir/file").equals_slice(b"actual"));
    assert!(!captured.entries().iter().any(|e| e.path() == "removed"));
    assert_eq!(
        captured
            .entries()
            .iter()
            .find(|e| e.path() == "keep")
            .unwrap()
            .mode(),
        0o751
    );
    assert!(
        matches!(captured.entries().iter().find(|e|e.path()=="link").unwrap().node(),RepositoryWorkspaceNode::Link(target) if target=="newdir/../keep")
    );
    let captured_bytes = bank.snapshot().retained_bytes;
    assert!(captured_bytes > baseline);
    let completed = captured
        .complete(
            &old,
            old.request(archive()).unwrap(),
            RepositoryWorkspaceResult::Archive([0; 32]),
        )
        .unwrap();
    assert!(RepositoryWorkspaceBody::ptr_eq(
        &same,
        body(completed.transcript().current().entries(), "keep")
    ));
    assert!(bank.snapshot().retained_bytes > baseline);
    assert!(bank.snapshot().peak_retained_bytes > bank.snapshot().retained_bytes);
    drop(completed);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(scope.snapshot().decoded_bytes, 0);
}

#[test]
fn snapshot_retained_descriptor_ignores_absent_and_foreign_display_tree() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir(&display).unwrap();
    prepare(&display);
    fs::write(display.join("file"), b"original").unwrap();
    let fd = File::open(&display).unwrap();
    let selected = reservation(&display, 202);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    fs::rename(&display, &backing).unwrap();
    let bank = resources();
    let scope = scope();
    let a = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(body(a.entries(), "file").equals_slice(b"original"));
    assert!(!display.exists());
    drop(a);
    fs::create_dir(&display).unwrap();
    fs::write(display.join("file"), b"foreign").unwrap();
    let b = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(body(b.entries(), "file").equals_slice(b"original"));
    assert_eq!(fs::read(display.join("file")).unwrap(), b"foreign");
}

#[test]
fn snapshot_limits_include_root_all_paths_literal_targets_and_total_bodies() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    fs::write(fixture.path().join("a"), b"a").unwrap();
    fs::write(fixture.path().join("b"), b"b").unwrap();
    let selected = reservation(Path::new("/snapshot"), 203);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let _scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
    let exact = Limits {
        entries: 3,
        names: 2,
        bodies: 2,
        depth: 1,
    };
    let captured = capture_at(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
        exact,
    )
    .unwrap();
    assert_eq!(captured.entries().len(), 3);
    drop(captured);
    let scratch_baseline = bank.snapshot().retained_bytes;
    for limits in [
        Limits {
            entries: 2,
            ..exact
        },
        Limits { names: 1, ..exact },
        Limits { bodies: 1, ..exact },
        Limits { depth: 0, ..exact },
    ] {
        assert!(
            capture_at(
                &io,
                &selected,
                RepositoryWorkspaceMode::Archive,
                old.current(),
                &bank,
                &scope,
                &|| true,
                limits
            )
            .is_err()
        );
        assert_eq!(bank.snapshot().retained_bytes, scratch_baseline);
    }
    symlink("a", fixture.path().join("l")).unwrap();
    let exact_link = Limits {
        entries: 4,
        names: 4,
        ..exact
    };
    assert!(
        capture_at(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true,
            exact_link
        )
        .is_ok()
    );
    assert!(
        capture_at(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true,
            Limits {
                names: 3,
                ..exact_link
            }
        )
        .is_err()
    );
    drop(_scratch);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(fs::read(fixture.path().join("a")).unwrap(), b"a");
}

#[test]
fn snapshot_process_entry_depth_and_absolute_path_limits_do_not_lower_archive_lane() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    let selected = reservation(Path::new("/snapshot"), 204);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let archive_io =
        RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let scope = scope();
    // Real direct entries discriminate inclusion of root in the8192 bound.
    for i in 0..8191 {
        fs::write(fixture.path().join(format!("f{i:04}")), b"").unwrap();
    }
    let exact = sample(
        &archive_io,
        &selected,
        RepositoryWorkspaceMode::Process,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert_eq!(exact.entries().len(), 8192);
    drop(exact);
    fs::write(fixture.path().join("plus"), b"").unwrap();
    assert!(
        sample(
            &archive_io,
            &selected,
            RepositoryWorkspaceMode::Process,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
    assert_eq!(
        sample(
            &archive_io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .unwrap()
        .entries()
        .len(),
        8193
    );
    for entry in fs::read_dir(fixture.path()).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    let depth64 = std::iter::repeat_n("d", 64).collect::<Vec<_>>().join("/");
    fs::create_dir_all(fixture.path().join(&depth64)).unwrap();
    assert!(
        sample(
            &archive_io,
            &selected,
            RepositoryWorkspaceMode::Process,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_ok()
    );
    fs::write(fixture.path().join(&depth64).join("leaf"), b"deep").unwrap();
    assert!(
        sample(
            &archive_io,
            &selected,
            RepositoryWorkspaceMode::Process,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
    assert!(
        sample(
            &archive_io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_ok()
    );
    // The display spelling alone changes the absolute-path budget. No Host path
    // at either spelling is opened by this rooted kernel.
    let long = reservation(Path::new(&format!("/{}", "x".repeat(4094))), 205);
    let long_prior = RepositoryWorkspaceTranscript::new(long.clone(), 0o755).unwrap();
    assert!(
        sample(
            &archive_io,
            &long,
            RepositoryWorkspaceMode::Archive,
            long_prior.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
}

#[test]
fn snapshot_streams_beyond_source_filebytes_cap_and_admits_size_before_allocating() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    let file = fixture.path().join("large");
    fs::write(&file, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let selected = reservation(Path::new("/snapshot"), 206);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let scope = scope();
    let baseline = bank.snapshot().retained_bytes;
    let captured = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert_eq!(body(captured.entries(), "large").len(), 1024 * 1024 + 1);
    drop(captured);
    assert_eq!(scope.snapshot().decoded_bytes, 0);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    // Sparse+1 over process aggregate cap fails from metadata, before a body
    // allocation or read and without inheriting the ordinary source-object cap.
    File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(PROCESS_BYTES + 1)
        .unwrap();
    let before = bank.snapshot().peak_retained_bytes;
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Process,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert!(bank.snapshot().peak_retained_bytes <= before);
}

#[test]
fn snapshot_rejects_special_hardlinked_and_escaping_literal_nodes_without_following() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"outside").unwrap();
    let selected = reservation(Path::new("/snapshot"), 207);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    for target in [
        "../outside",
        "/snapshot-other/secret",
        "/snapshot/../snapshot/secret",
    ] {
        symlink(target, fixture.path().join("link")).unwrap();
        assert!(
            sample(
                &io,
                &selected,
                RepositoryWorkspaceMode::Archive,
                old.current(),
                &bank,
                &scope,
                &|| true
            )
            .is_err()
        );
        fs::remove_file(fixture.path().join("link")).unwrap();
        assert_eq!(bank.snapshot().retained_bytes, baseline);
    }
    symlink("/snapshot/missing", fixture.path().join("link")).unwrap();
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_ok()
    );
    fs::remove_file(fixture.path().join("link")).unwrap();
    fs::write(fixture.path().join("file"), b"body").unwrap();
    fs::hard_link(fixture.path().join("file"), fixture.path().join("alias")).unwrap();
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
    fs::remove_file(fixture.path().join("alias")).unwrap();
    nix::unistd::mkfifo(&fixture.path().join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &scope,
            &|| true
        )
        .is_err()
    );
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(fs::read(outside.path().join("secret")).unwrap(), b"outside");
}

#[test]
fn snapshot_cancellation_and_mutation_during_old_body_comparison_release_all_storage() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    fs::write(fixture.path().join("file"), b"old").unwrap();
    let selected = reservation(Path::new("/snapshot"), 208);
    let old = prior(
        selected.clone(),
        vec![root_entry(), file_entry("file", b"old")],
    );
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let canceled = scope();
    canceled.cancel();
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &canceled,
            &|| true
        )
        .is_err()
    );
    let active_scope = scope();
    assert!(
        sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            old.current(),
            &bank,
            &active_scope,
            &|| false
        )
        .is_err()
    );
    let framed = io.lstat(Path::new("file")).unwrap();
    let calls = Cell::new(0);
    let active = || {
        let call = calls.get();
        calls.set(call + 1);
        if call == 2 {
            fs::write(fixture.path().join("file"), b"new").unwrap();
        }
        true
    };
    let mut credit = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
    assert!(
        capture_body(
            &io,
            Path::new("file"),
            &framed,
            old.current(),
            &mut credit,
            &active_scope,
            &active
        )
        .is_err()
    );
    drop(credit);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(fs::read(fixture.path().join("file")).unwrap(), b"new");
}

#[test]
fn snapshot_completion_rejects_foreign_owner_and_failed_protocol_without_orphaning_credit() {
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    fs::write(fixture.path().join("file"), b"actual").unwrap();
    let selected = reservation(Path::new("/snapshot"), 209);
    let old = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let foreign =
        RepositoryWorkspaceTranscript::new(reservation(Path::new("/snapshot"), 210), 0o755)
            .unwrap();
    let fd = File::open(fixture.path()).unwrap();
    let io = RootIo::new(&fd, io_limits(RepositoryWorkspaceMode::Archive, &selected)).unwrap();
    let bank = resources();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope();
    let captured = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    assert!(matches!(
        captured.complete(
            &foreign,
            foreign.request(archive()).unwrap(),
            RepositoryWorkspaceResult::Archive([0; 32])
        ),
        Err(PhysicalRootError::Reservation)
    ));
    assert_eq!(bank.snapshot().retained_bytes, baseline);
    let captured = sample(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        old.current(),
        &bank,
        &scope,
        &|| true,
    )
    .unwrap();
    let weak = RepositoryWorkspaceBody::downgrade(body(captured.entries(), "file"));
    let request = old
        .request(RepositoryWorkspaceOperation::Exists("file".into()))
        .unwrap();
    assert!(matches!(
        captured.complete(&old, request, RepositoryWorkspaceResult::Exists(true)),
        Err(PhysicalRootError::Protocol(_))
    ));
    assert!(weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn covered_snapshot_completion_shares_one_actual_body_allocation_with_history() {
    // Borrowed ordinary RootIo kernel only, not a native backing/factory proof.
    // Two full 4MiB payload debits cannot fit this bank, but one payload plus
    // all temporary/control forecasts can. Pure fallback tests stay unchanged.
    let fixture = tempfile::tempdir().unwrap();
    prepare(fixture.path());
    let length = 4 * 1024 * 1024;
    fs::write(
        fixture.path().join("large"),
        incompressible(length, 0x123456789abcdef),
    )
    .unwrap();
    let selected = reservation(Path::new("/absent/covered-snapshot"), 229);
    let scope = scope();
    let bank = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(7 * 1024 * 1024, 16, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let ledger = NativeBodyLedger::new(&bank, &scope, &selected, &|| true).unwrap();
    let mut candidates = ledger.candidates().unwrap();
    let initial = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
    let file = File::open(fixture.path()).unwrap();
    let io = RootIo::new(
        &file,
        io_limits(RepositoryWorkspaceMode::Archive, &selected),
    )
    .unwrap();
    let scratch = bank.reserve(IO_SCRATCH, 3, 0).unwrap();
    let captured = capture_at_with_sources(
        &io,
        &selected,
        RepositoryWorkspaceMode::Archive,
        initial.current(),
        &bank,
        &scope,
        &|| true,
        Limits::for_mode(RepositoryWorkspaceMode::Archive),
        None,
        Some(&mut candidates),
    )
    .unwrap();
    let actual = body(captured.entries(), "large").clone();
    assert!(
        actual.native_storage_bytes() >= length,
        "physical-pressure premise must hold after encoding"
    );
    let weak = RepositoryWorkspaceBody::downgrade(&actual);
    let completed = captured
        .with_body_candidates(candidates)
        .complete_active(
            &initial,
            initial.request(archive()).unwrap(),
            RepositoryWorkspaceResult::Archive([0; 32]),
            None,
            &|| true,
        )
        .unwrap();
    assert!(RepositoryWorkspaceBody::ptr_eq(
        body(completed.transcript().current().entries(), "large"),
        &actual
    ));
    assert!(RepositoryWorkspaceBody::ptr_eq(
        ledger.lookup(&actual, &|| true).unwrap().unwrap().body(),
        &actual
    ));
    assert!(
        completed
            .body_ledger
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(held, &ledger))
    );
    drop((actual, completed));
    assert!(
        weak.upgrade().is_some(),
        "history ledger must retain admitted allocation"
    );
    drop((ledger, scratch));
    assert!(weak.upgrade().is_none());
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}

#[test]
fn covered_snapshot_refuses_foreign_bank_scope_or_reservation_before_control_discount() {
    // Bare RootIo sampling remains a full-credit utility seam. Attaching a
    // separately valid candidate owner cannot turn it into foreign coverage.
    for mismatch in 0..3 {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        fs::write(fixture.path().join("file"), b"actual sampled bytes").unwrap();
        let selected = reservation(Path::new("/absent/coverage-origin"), 230);
        let foreign_selected = reservation(Path::new("/absent/foreign-origin"), 231);
        let original_scope = scope();
        let foreign_scope = scope();
        let bank = resources();
        let foreign_bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let foreign_baseline = foreign_bank.snapshot().retained_bytes;
        let (ledger_bank, ledger_scope, ledger_reservation) = match mismatch {
            0 => (&foreign_bank, &original_scope, &selected),
            1 => (&bank, &foreign_scope, &selected),
            2 => (&bank, &original_scope, &foreign_selected),
            _ => unreachable!(),
        };
        let ledger =
            NativeBodyLedger::new(ledger_bank, ledger_scope, ledger_reservation, &|| true).unwrap();
        let candidates = ledger.candidates().unwrap();
        let initial = RepositoryWorkspaceTranscript::new(selected.clone(), 0o755).unwrap();
        let file = File::open(fixture.path()).unwrap();
        let io = RootIo::new(
            &file,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let captured = sample(
            &io,
            &selected,
            RepositoryWorkspaceMode::Archive,
            initial.current(),
            &bank,
            &original_scope,
            &|| true,
        )
        .unwrap();
        let weak = RepositoryWorkspaceBody::downgrade(body(captured.entries(), "file"));
        let before = ledger_bank.snapshot();
        assert!(
            matches!(
                captured.with_body_candidates(candidates).complete_active(
                    &initial,
                    initial.request(archive()).unwrap(),
                    RepositoryWorkspaceResult::Archive([0; 32]),
                    None,
                    &|| true,
                ),
                Err(PhysicalRootError::Reservation)
            ),
            "foreign coverage origin {mismatch} must refuse before the body-covered forecast"
        );
        assert!(
            weak.upgrade().is_none(),
            "refusal must retire sampled raw bytes"
        );
        assert_eq!(
            ledger_bank.snapshot().peak_retained_bytes,
            before.peak_retained_bytes,
            "origin refusal must precede candidate/body/control admission"
        );
        drop(ledger);
        for (bank, expected) in [(&bank, baseline), (&foreign_bank, foreign_baseline)] {
            let final_usage = bank.snapshot();
            assert_eq!(
                (
                    final_usage.retained_bytes,
                    final_usage.native_fds,
                    final_usage.backing_bytes
                ),
                (expected, 0, 0)
            );
        }
    }
}
