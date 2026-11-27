//! Physical tests below the retained transcript. Fixture population uses the
//! original FD returned by the compiled Archive launcher.
use std::ffi::CStr;
use std::ffi::CString;
use std::io;

use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;

use super::*;
use crate::runtime::repository_physical_root::IO_SCRATCH;
use crate::runtime::repository_physical_root::snapshot;

const ENTRY_LIMIT: usize = 131_072;
const INODE_LIMIT: u64 = 262_144;

fn with_archive_root(
    id: u64,
    exercise: impl FnOnce(
        &RepositoryPhysicalRootBuilder,
        &Arc<SourceServiceScope>,
        &Arc<RepositoryRootResources>,
    ),
) {
    let (_temporary, selected, bank, baseline, scope, runtime) = command_inputs(id);
    let (builder, seed) = bootstrap_archive(&selected, &bank, &scope, &runtime);
    drop(seed);
    let fd = builder.file.as_raw_fd();
    let empty = facts(fd);
    assert_eq!(
        (empty.bytes, empty.inodes, empty.allocated),
        (ARCHIVE_BYTES, INODE_LIMIT, 0)
    );
    assert_eq!(bank.snapshot().backing_bytes, ARCHIVE_BYTES);
    exercise(&builder, &scope, &bank);
    scope.check().unwrap();
    drop((builder, scope));
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    release(&bank, baseline);
}

fn create(root: &File, name: &CStr) -> io::Result<File> {
    let flags =
        OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    openat(root, name, flags, Mode::from_bits_truncate(0o600))
        .map(File::from)
        .map_err(io::Error::from)
}

#[test]
#[ignore = "actual compiled CLI and root-inclusive Archive sampler maximum; finite supervisor required"]
fn native_archive_entry_sampler_accepts_exact_maximum_and_refuses_one_more() {
    with_archive_root(2420, |builder, scope, bank| {
        let _scratch = bank.reserve(IO_SCRATCH, 0, 0).unwrap();
        let fd = builder.file.as_raw_fd();
        let directory = |index| {
            scope.check().unwrap();
            let name = CString::new(format!("d{index:06x}")).unwrap();
            assert_eq!(
                unsafe { libc::mkdirat(fd, name.as_ptr(), 0o755) },
                0,
                "{}",
                io::Error::last_os_error()
            );
        };
        for index in 0..ENTRY_LIMIT - 1 {
            directory(index);
        }
        let live_credits = || {
            let usage = bank.snapshot();
            (usage.retained_bytes, usage.native_fds, usage.backing_bytes)
        };
        let root_only = live_credits();
        let exact = facts(fd);
        assert!(exact.used_inodes < exact.inodes);
        let initial =
            RepositoryWorkspaceTranscript::new(builder.reservation.clone(), 0o755).unwrap();
        let captured = snapshot::capture(builder, initial.current(), scope, &|| true).unwrap();
        assert_eq!(captured.mode(), RepositoryWorkspaceMode::Archive);
        assert_eq!(captured.entries().len(), ENTRY_LIMIT);
        assert!(
            captured
                .entries()
                .iter()
                .all(|entry| matches!(entry.node(), RepositoryWorkspaceNode::Directory))
        );
        assert_eq!(
            captured
                .entries()
                .iter()
                .map(|entry| entry.path().len())
                .sum::<usize>(),
            (ENTRY_LIMIT - 1) * 7
        );
        drop(captured);
        assert_eq!(live_credits(), root_only);
        assert_eq!(facts(fd), exact);

        directory(ENTRY_LIMIT - 1);
        let plus_one = facts(fd);
        assert!(plus_one.used_inodes > exact.used_inodes);
        assert!(plus_one.used_inodes < plus_one.inodes);
        assert!(matches!(
            snapshot::capture(builder, initial.current(), scope, &|| true),
            Err(PhysicalRootError::Snapshot)
        ));
        assert_eq!(facts(fd), plus_one);
        assert_eq!(live_credits(), root_only);
        eprintln!(
            "native Archive sampler accepted {ENTRY_LIMIT} entries with {} charged inodes and refused {} entries",
            exact.used_inodes,
            ENTRY_LIMIT + 1
        );
    });
}

#[test]
#[ignore = "actual compiled CLI and physical 2 GiB Archive data quota; finite supervisor required"]
fn native_archive_tmpfs_refuses_one_more_allocated_page() {
    with_archive_root(2421, |builder, scope, bank| {
        let _scratch = bank.reserve(IO_SCRATCH, 1, 0).unwrap();
        let file = create(&builder.file, c"pages").unwrap();
        const STEP: u64 = 64 * MIB;
        for offset in (0..ARCHIVE_BYTES).step_by(STEP as usize) {
            scope.check().unwrap();
            assert_eq!(
                unsafe { libc::fallocate(file.as_raw_fd(), 0, offset as i64, STEP as i64) },
                0,
                "{}",
                io::Error::last_os_error(),
            );
        }
        let fd = builder.file.as_raw_fd();
        let full = facts(fd);
        assert_eq!(full.allocated, ARCHIVE_BYTES);
        assert_eq!(file.metadata().unwrap().len(), ARCHIVE_BYTES);
        let byte = [1u8];
        for _ in 0..2 {
            assert_eq!(
                unsafe {
                    libc::pwrite(
                        file.as_raw_fd(),
                        byte.as_ptr().cast(),
                        byte.len(),
                        ARCHIVE_BYTES as i64,
                    )
                },
                -1
            );
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ENOSPC)
            );
            assert_eq!(file.metadata().unwrap().len(), ARCHIVE_BYTES);
            assert_eq!(facts(fd), full);
        }
        eprintln!(
            "native Archive tmpfs filled {} bytes and refused another page",
            full.allocated
        );
    });
}

#[test]
#[ignore = "actual compiled CLI and measured Archive inode exhaustion; finite supervisor required"]
fn native_archive_tmpfs_refuses_another_inode_at_capacity() {
    with_archive_root(2422, |builder, scope, bank| {
        let _scratch = bank.reserve(IO_SCRATCH, 1, 0).unwrap();
        let fd = builder.file.as_raw_fd();
        let mut previous = facts(fd);
        let mut max_charge = 0;
        for index in 0..INODE_LIMIT {
            scope.check().unwrap();
            let name = CString::new(format!("i{index:06x}")).unwrap();
            match create(&builder.file, &name) {
                Ok(file) => {
                    drop(file);
                    let current = facts(fd);
                    assert!(
                        current.used_inodes > previous.used_inodes
                            && current.used_inodes <= INODE_LIMIT
                    );
                    assert_eq!(current.allocated, 0);
                    max_charge = max_charge.max(current.used_inodes - previous.used_inodes);
                    previous = current;
                }
                Err(error) => {
                    assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
                    assert_eq!(facts(fd), previous);
                    assert!(previous.inodes - previous.used_inodes < max_charge);
                    for _ in 0..2 {
                        let error = create(&builder.file, c"extra").unwrap_err();
                        assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
                        assert_eq!(facts(fd), previous);
                    }
                    eprintln!(
                        "native Archive tmpfs refused another inode after {index} entries, {} of {} charged",
                        previous.used_inodes, previous.inodes
                    );
                    return;
                }
            }
        }
        panic!("Archive tmpfs did not enforce its measured inode allowance");
    });
}
