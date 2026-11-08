//! Ordinary FD placement-kernel coverage, not native root/custody admission.
//! Real Capture owners and a shared finite bank cover the test input lifetime;
//! no Source request, root, claim, seal, or completion is fabricated here.
use std::cell::Cell;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use super::placement::place_capture;
use super::*;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceServiceError;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;
use crate::runtime::repository_root_io::WriteMode;

fn bounds() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 256,
        max_name_bytes: 255,
        max_depth: 32,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 256,
    }
}
fn live(resources: &RepositoryRootResources) -> (usize, usize, u64) {
    let snapshot = resources.snapshot();
    (
        snapshot.retained_bytes,
        snapshot.native_fds,
        snapshot.backing_bytes,
    )
}
struct Fixture {
    descriptor: File,
    directory: tempfile::TempDir,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    limits: RootIoLimits,
    // Actual file/payload owners precede their control/descriptor credit.
    _credit: ResourceCredit,
}
impl Fixture {
    fn root_path(&self) -> std::path::PathBuf {
        self.directory.path().join("root")
    }
    fn io(&self) -> RootIo<'_> {
        RootIo::new(&self.descriptor, self.limits).unwrap()
    }
    fn capture(&self, bytes: &[u8]) -> Capture {
        Capture::acquire(
            Sha256::digest(bytes).into(),
            (bytes.len() as u64).max(1),
            &self.resources,
            &self.scope,
            &|| true,
            || {
                let mut file =
                    tempfile::NamedTempFile::new_in(self.directory.path()).map_err(|e| {
                        crate::runtime::repository_io::ArchiveMaterializationError::materialization(
                            e.to_string(),
                        )
                    })?;
                file.write_all(bytes).map_err(|e| {
                    crate::runtime::repository_io::ArchiveMaterializationError::materialization(
                        e.to_string(),
                    )
                })?;
                file.flush().map_err(|e| {
                    crate::runtime::repository_io::ArchiveMaterializationError::materialization(
                        e.to_string(),
                    )
                })?;
                Ok(file)
            },
        )
        .unwrap()
    }
    fn old(&self, path: &str) -> libc::stat {
        let io = self.io();
        if let Some(parent) = Path::new(path).parent() {
            io.ensure_directory(parent, 0o755).unwrap();
        }
        io.write_file(
            Path::new(path),
            b"old",
            0o555,
            WriteMode::CreateNew,
            &|| true,
        )
        .unwrap();
        io.lstat(Path::new(path)).unwrap()
    }
    fn bytes(&self, path: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.io()
            .read_file(Path::new(path), &|| true, |chunk| {
                bytes.extend_from_slice(chunk);
                Ok(())
            })
            .unwrap();
        bytes
    }
    fn place(
        &self,
        capture: &Capture,
        path: &str,
        mode: u32,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        // Same temporary working/FD allowance as SourceRootInput::populate.
        let _working = self.resources.reserve(2 * IO_SCRATCH, 8, 0)?;
        place_capture(capture, &self.io(), path, mode, &self.scope, active)
    }
    fn names(&self) -> Vec<std::ffi::OsString> {
        let mut names = fs::read_dir(self.root_path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    }
}
fn with_fixture(
    limits: RootIoLimits,
    source_limits: SourceServiceLimits,
    run: impl FnOnce(&Fixture),
) {
    let resources = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 8 * 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    let baseline = live(&resources);
    {
        let deadline = Instant::now() + Duration::from_secs(26);
        let scope = SourceServiceScope::new_command(
            slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            source_limits,
            resources.clone(),
        )
        .unwrap();
        let credit = resources
            .reserve(std::mem::size_of::<Fixture>() + 4096, 1, 0)
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("root")).unwrap();
        let descriptor = File::open(directory.path().join("root")).unwrap();
        let fixture = Fixture {
            descriptor,
            directory,
            scope,
            resources: resources.clone(),
            limits,
            _credit: credit,
        };
        run(&fixture);
        assert_eq!(fixture.scope.snapshot().live_buffer_bytes, 0);
    }
    assert_eq!(
        live(&resources),
        baseline,
        "all ordinary capture/scope/FD/storage credits retire"
    );
}
fn stage(index: usize) -> String {
    format!(".slug-source-placement-{index:04x}")
}

#[test]
fn placement_replaces_readonly_and_creates_absent_files_with_exact_mode_and_no_stage() {
    with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
        let capture = fixture.capture(b"verified\0\xffbytes");
        for (index, mode) in [0o644, 0o755].into_iter().enumerate() {
            let path = format!("existing{index}");
            let before = fixture.old(&path);
            fixture.place(&capture, &path, mode, &|| true).unwrap();
            let after = fixture.io().lstat(Path::new(&path)).unwrap();
            assert_ne!(before.st_ino, after.st_ino);
            assert_eq!(after.st_mode & 0o7777, mode);
            assert_eq!(fixture.bytes(&path), b"verified\0\xffbytes");
        }
        fixture
            .place(&capture, "nested/absent", 0o644, &|| true)
            .unwrap();
        assert_eq!(fixture.bytes("nested/absent"), b"verified\0\xffbytes");
        assert_eq!(
            fixture
                .io()
                .lstat(Path::new("nested/absent"))
                .unwrap()
                .st_mode
                & 0o7777,
            0o644
        );
        assert!(fixture.names().iter().all(|name| {
            !name
                .as_encoded_bytes()
                .starts_with(b".slug-source-placement-")
        }));
        assert_eq!(
            fixture.scope.snapshot().written_bytes,
            3 * capture.frame().len()
        );
        assert_eq!(fixture.scope.snapshot().source_attempts, 0);
        assert_eq!(fixture.scope.snapshot().decoded_bytes, 0);
    });
}

#[test]
fn placement_refuses_directory_symlink_and_hardlink_targets_without_outside_mutation() {
    for kind in ["directory", "symlink", "hardlink"] {
        with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
            let outside = fixture.directory.path().join("outside");
            fs::write(&outside, b"protected").unwrap();
            let target = fixture.root_path().join("target");
            match kind {
                "directory" => fs::create_dir(&target).unwrap(),
                "symlink" => symlink(&outside, &target).unwrap(),
                "hardlink" => fs::hard_link(&outside, &target).unwrap(),
                _ => unreachable!(),
            }
            let before = fs::symlink_metadata(&target).unwrap();
            let capture = fixture.capture(b"replacement");
            assert!(matches!(
                fixture.place(&capture, "target", 0o644, &|| true),
                Err(PhysicalRootError::Snapshot)
            ));
            let after = fs::symlink_metadata(&target).unwrap();
            assert_eq!(
                (before.dev(), before.ino(), before.mode(), before.nlink()),
                (after.dev(), after.ino(), after.mode(), after.nlink())
            );
            assert_eq!(fs::read(outside).unwrap(), b"protected");
            assert_eq!(fixture.names(), [std::ffi::OsString::from("target")]);
            assert_eq!(fixture.scope.snapshot().written_bytes, 0);
        });
    }
}

#[test]
fn placement_preserves_all_collision_kinds_and_can_replace_a_target_named_like_staging() {
    with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
        let root = fixture.root_path();
        fs::write(root.join(stage(0)), b"collision").unwrap();
        fs::create_dir(root.join(stage(1))).unwrap();
        let outside = fixture.directory.path().join("outside");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, root.join(stage(2))).unwrap();
        fs::hard_link(&outside, root.join(stage(3))).unwrap();
        let before = (0..4)
            .map(|i| fs::symlink_metadata(root.join(stage(i))).unwrap())
            .collect::<Vec<_>>();
        fixture.old("MODULE.bazel");
        let capture = fixture.capture(b"new module");
        fixture
            .place(&capture, "MODULE.bazel", 0o644, &|| true)
            .unwrap();
        for (index, before) in before.iter().enumerate() {
            let after = fs::symlink_metadata(root.join(stage(index))).unwrap();
            assert_eq!(
                (before.dev(), before.ino(), before.mode(), before.nlink()),
                (after.dev(), after.ino(), after.mode(), after.nlink())
            );
        }
        assert_eq!(fs::read(root.join(stage(0))).unwrap(), b"collision");
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        assert!(!root.join(stage(4)).exists());
        assert_eq!(fixture.names().len(), 5);
    });
    with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
        let target = stage(0);
        let before = fixture.old(&target);
        let capture = fixture.capture(b"new");
        fixture.place(&capture, &target, 0o644, &|| true).unwrap();
        assert_eq!(fixture.bytes(&target), b"new");
        assert_ne!(
            fixture.io().lstat(Path::new(&target)).unwrap().st_ino,
            before.st_ino
        );
        assert_eq!(fixture.names(), [std::ffi::OsString::from(target)]);
    });
}

#[test]
fn placement_staging_name_exhaustion_is_bounded_and_preserves_the_existing_target() {
    with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
        // Deliberately hostile ordinary-kernel directory, not an admitted Source
        // tree. This isolates the fixed staging-name refusal, not logical caps.
        for index in 0..8192 {
            fs::write(fixture.root_path().join(stage(index)), []).unwrap();
        }
        let before = fixture.old("MODULE.bazel");
        let capture = fixture.capture(b"new");
        assert!(matches!(
            fixture.place(&capture, "MODULE.bazel", 0o644, &|| true),
            Err(PhysicalRootError::Resource)
        ));
        let after = fixture.io().lstat(Path::new("MODULE.bazel")).unwrap();
        assert_eq!(
            (before.st_ino, before.st_mode, before.st_size),
            (after.st_ino, after.st_mode, after.st_size)
        );
        assert_eq!(fixture.bytes("MODULE.bazel"), b"old");
        assert_eq!(fixture.names().len(), 8193);
        assert_eq!(fixture.scope.snapshot().written_bytes, 0);
    });
}

#[test]
fn placement_root_staging_preserves_exact_valid_target_path_and_depth_bounds() {
    for (maximum, parent, leaf) in [
        (256, vec!["d".repeat(7); 31].join("/"), "payloadx"),
        // RootIo-only envelope case; it does not widen Source's256-byte schema.
        (
            4096,
            [vec!["d".repeat(255); 15].join("/"), "d".repeat(250)].join("/"),
            "short",
        ),
    ] {
        let path = format!("{parent}/{leaf}");
        assert_eq!(path.len(), maximum);
        let mut selected = bounds();
        selected.max_path_bytes = maximum;
        with_fixture(selected, SourceServiceLimits::default(), |fixture| {
            let before = fixture.old(&path);
            let capture = fixture.capture(b"deep");
            fixture.place(&capture, &path, 0o644, &|| true).unwrap();
            assert_eq!(fixture.bytes(&path), b"deep");
            assert_ne!(
                fixture.io().lstat(Path::new(&path)).unwrap().st_ino,
                before.st_ino
            );
            assert!(!fixture.root_path().join(stage(0)).exists());
        });
    }
}

#[test]
fn placement_admits_capture_length_before_parent_creation_or_target_mutation() {
    let mut selected = bounds();
    selected.max_file_bytes = 4;
    with_fixture(selected, SourceServiceLimits::default(), |fixture| {
        let before = fixture.old("old");
        let capture = fixture.capture(b"large");
        for target in ["old", "new/child"] {
            assert!(fixture.place(&capture, target, 0o644, &|| true).is_err());
        }
        let after = fixture.io().lstat(Path::new("old")).unwrap();
        assert_eq!(
            (before.st_ino, before.st_size, before.st_mode),
            (after.st_ino, after.st_size, after.st_mode)
        );
        assert_eq!(fixture.bytes("old"), b"old");
        assert_eq!(fixture.names(), [std::ffi::OsString::from("old")]);
        assert_eq!(fixture.scope.snapshot().written_bytes, 0);
    });
}

#[test]
fn placement_refuses_capture_corruption_eof_and_digest_before_replacing_old_inode() {
    for fault in ["bytes", "short", "extra", "digest"] {
        with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
            let before = fixture.old("MODULE.bazel");
            // The deliberate extra-EOF-byte mutation has its own allowance,
            // declared first so it retires after the actual Capture file.
            let _fault_storage = fixture.resources.reserve(0, 0, 1).unwrap();
            let mut capture = fixture.capture(b"verified");
            match fault {
                "bytes" => {
                    capture.file().write_at(b"corrupt!", 0).unwrap();
                }
                "short" => capture.file().set_len(7).unwrap(),
                "extra" => {
                    capture.file().write_at(b"!", 8).unwrap();
                }
                "digest" => capture.digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(matches!(
                fixture.place(&capture, "MODULE.bazel", 0o644, &|| true),
                Err(PhysicalRootError::Snapshot)
            ));
            let after = fixture.io().lstat(Path::new("MODULE.bazel")).unwrap();
            assert_eq!(
                (before.st_ino, before.st_mode, before.st_size),
                (after.st_ino, after.st_mode, after.st_size)
            );
            assert_eq!(fixture.bytes("MODULE.bazel"), b"old");
            // A failed private generation may retain its private stage until
            // disposal. It must not publish it or replace the old target.
            assert!(fixture.names().len() <= 2);
        });
    }
}

#[test]
fn placement_cancellation_and_write_limit_refuse_before_existing_target_commit() {
    with_fixture(bounds(), SourceServiceLimits::default(), |fixture| {
        let before = fixture.old("MODULE.bazel");
        let capture = fixture.capture(&vec![b'x'; 128 * 1024 + 1]);
        let cancelled = Cell::new(false);
        let active = || {
            if fixture.scope.snapshot().written_bytes > 0 {
                cancelled.set(true);
                fixture.scope.cancel();
            }
            true
        };
        assert!(matches!(
            fixture.place(&capture, "MODULE.bazel", 0o644, &active),
            Err(PhysicalRootError::SourceService(
                SourceServiceError::Cancelled
            ))
        ));
        assert!(cancelled.get());
        assert!(
            fixture.scope.snapshot().written_bytes > 0
                && fixture.scope.snapshot().written_bytes < capture.frame().len()
        );
        assert_eq!(
            fixture
                .io()
                .lstat(Path::new("MODULE.bazel"))
                .unwrap()
                .st_ino,
            before.st_ino
        );
        assert_eq!(fixture.bytes("MODULE.bazel"), b"old");
    });
    with_fixture(
        bounds(),
        SourceServiceLimits {
            written_bytes: 3,
            ..SourceServiceLimits::default()
        },
        |fixture| {
            let before = fixture.old("MODULE.bazel");
            let capture = fixture.capture(b"four");
            // The existing counted writer may wrap the typed scope failure in
            // io::Error; the sticky scope cause below remains authoritative.
            assert!(
                fixture
                    .place(&capture, "MODULE.bazel", 0o644, &|| true)
                    .is_err()
            );
            assert_eq!(
                fixture.scope.check(),
                Err(SourceServiceError::Limit(SourceCounter::WrittenBytes))
            );
            assert_eq!(
                fixture
                    .io()
                    .lstat(Path::new("MODULE.bazel"))
                    .unwrap()
                    .st_ino,
                before.st_ino
            );
            assert_eq!(fixture.bytes("MODULE.bazel"), b"old");
        },
    );
}
