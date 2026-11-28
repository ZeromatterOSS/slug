//! Linux Host evidence for a directory used only to reach a descendant.

use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::path::Path;

use nix::fcntl::AT_FDCWD;
use nix::fcntl::OFlag;
use nix::fcntl::OpenHow;
use nix::fcntl::ResolveFlag;
use nix::fcntl::openat2;
use slug_workspace_v2::DirectoryTraversalObservation;
use slug_workspace_v2::DirectoryTraversalWitness;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOperationResult;

// Linux UAPI linux/stat.h. The ID stays unique even after a mount is destroyed.
const STATX_MNT_ID_UNIQUE: u32 = 0x0000_4000;
const WITNESS_MASK: u32 = STATX_MNT_ID_UNIQUE
    | libc::STATX_TYPE
    | libc::STATX_MODE
    | libc::STATX_UID
    | libc::STATX_GID
    | libc::STATX_INO;

#[derive(PartialEq, Eq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    mount_id: u64,
}

impl DirectoryIdentity {
    fn with_search_access(self, searchable: bool) -> DirectoryTraversalWitness {
        DirectoryTraversalWitness::new(
            self.device,
            self.inode,
            self.mode,
            self.uid,
            self.gid,
            self.mount_id,
            searchable,
        )
    }
}

pub(super) fn observe(
    path: &NormalizedAbsolutePath,
) -> PathOperationResult<DirectoryTraversalObservation> {
    if path.as_path().as_os_str().len() < libc::PATH_MAX as usize
        && let Some(witness) = witness(path.as_path())
    {
        return PathOperationResult::Present(DirectoryTraversalObservation::Directory(witness));
    }
    // Preserve the existing typed lstat observation when the no-follow walk,
    // mount identity, or effective-access witness cannot be established.
    match super::unix_lstat(path) {
        PathOperationResult::Present(stat) => {
            PathOperationResult::Present(DirectoryTraversalObservation::ExactLstat(stat))
        }
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Error(error) => PathOperationResult::Error(error),
    }
}

fn witness(path: &Path) -> Option<DirectoryTraversalWitness> {
    let directory = open_directory(path)?;
    let before = identity(&directory)?;
    let searchable = search_access(&directory)?;
    if before != identity(&directory)? {
        return None;
    }
    Some(before.with_search_access(searchable))
}

fn open_directory(path: &Path) -> Option<OwnedFd> {
    debug_assert!(path.is_absolute());
    let flags = OFlag::O_PATH | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_DIRECTORY;
    // The normalized absolute path starts at the process root, independent
    // of the working directory. NO_SYMLINKS also excludes magic links. The
    // Host route may cross real or bind mount points.
    openat2(
        AT_FDCWD,
        path,
        OpenHow::new()
            .flags(flags)
            .resolve(ResolveFlag::RESOLVE_NO_SYMLINKS),
    )
    .ok()
}

fn identity(directory: &OwnedFd) -> Option<DirectoryIdentity> {
    let mut value = std::mem::MaybeUninit::<libc::statx>::zeroed();
    // SAFETY: the owned descriptor is live, the empty name is NUL-terminated
    // and AT_EMPTY_PATH selects the descriptor. The output is writable.
    let status = unsafe {
        libc::statx(
            directory.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            WITNESS_MASK,
            value.as_mut_ptr(),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: statx succeeded and initialized its output.
    let value = unsafe { value.assume_init() };
    let mode = u32::from(value.stx_mode);
    if value.stx_mask & WITNESS_MASK != WITNESS_MASK
        || value.stx_mnt_id == 0
        || mode & libc::S_IFMT as u32 != libc::S_IFDIR as u32
    {
        return None;
    }
    Some(DirectoryIdentity {
        device: u64::from(value.stx_dev_major) << 32 | u64::from(value.stx_dev_minor),
        inode: value.stx_ino,
        mode,
        uid: value.stx_uid,
        gid: value.stx_gid,
        mount_id: value.stx_mnt_id,
    })
}

fn search_access(directory: &OwnedFd) -> Option<bool> {
    // SAFETY: the owned descriptor is live, the empty name is NUL-terminated
    // and AT_EMPTY_PATH selects the descriptor. No user memory is written.
    let status = unsafe {
        libc::syscall(
            libc::SYS_faccessat2,
            directory.as_raw_fd(),
            c"".as_ptr(),
            libc::X_OK,
            libc::AT_EMPTY_PATH | libc::AT_EACCESS,
        )
    };
    match status {
        0 => Some(true),
        -1 if io::Error::last_os_error().raw_os_error() == Some(libc::EACCES) => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    use std::time::UNIX_EPOCH;

    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;

    use super::*;

    #[test]
    #[ignore = "requires Linux unique statx mount IDs and effective descriptor search"]
    fn host_traversal_stays_equal_while_explicit_lstat_changes() {
        let fs_root = NormalizedAbsolutePath::new("/").unwrap();
        assert!(matches!(
            super::observe(&fs_root),
            PathOperationResult::Present(DirectoryTraversalObservation::Directory(_))
        ));
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().canonicalize().unwrap();
        let path = NormalizedAbsolutePath::new(parent.clone()).unwrap();
        fs::File::open(&parent)
            .unwrap()
            .set_modified(UNIX_EPOCH)
            .unwrap();
        let traversal = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path.clone(),
            PathObservationOperation::DirectoryTraversal,
        );
        let raw = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path.clone(),
            PathObservationOperation::Lstat,
        );
        let listing = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path,
            PathObservationOperation::DirectoryEntries,
        );
        let observe = || {
            super::super::observe_native(&(), [], [traversal.clone(), raw.clone(), listing.clone()])
                .unwrap()
        };
        let before = observe();
        assert!(matches!(
            before.get(&traversal).unwrap().as_ref(),
            PathObservationResult::DirectoryTraversal(PathOperationResult::Present(
                DirectoryTraversalObservation::Directory(_)
            ))
        ));
        let sibling = parent.join("unrelated");
        fs::create_dir(&sibling).unwrap();
        let after = observe();
        fs::remove_dir(&sibling).unwrap();
        assert_eq!(before.get(&traversal), after.get(&traversal));
        assert_ne!(before.get(&raw), after.get(&raw));
        assert_ne!(before.get(&listing), after.get(&listing));
    }

    #[test]
    #[ignore = "requires Linux unique statx mount IDs and effective descriptor search"]
    fn host_traversal_tracks_directory_replacement_and_effective_search() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().canonicalize().unwrap();
        let directory = parent.join("directory");
        fs::create_dir(&directory).unwrap();
        let child = directory.join("child");
        fs::write(&child, b"child").unwrap();
        let path = NormalizedAbsolutePath::new(directory.clone()).unwrap();
        let initial = witness(&directory).unwrap();

        for mode in [0o100, 0o000] {
            fs::set_permissions(&directory, fs::Permissions::from_mode(mode)).unwrap();
            let effective_search = match fs::symlink_metadata(&child) {
                Ok(_) => true,
                Err(error) => {
                    assert_eq!(error.raw_os_error(), Some(libc::EACCES));
                    false
                }
            };
            let fd = open_directory(&directory).unwrap();
            let expected = identity(&fd).unwrap().with_search_access(effective_search);
            assert_ne!(initial, expected);
            assert_eq!(
                observe(&path),
                PathOperationResult::Present(DirectoryTraversalObservation::Directory(expected)),
            );
        }

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let old = witness(&directory).unwrap();
        fs::rename(&directory, parent.join("old")).unwrap();
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(old, witness(&directory).unwrap());
    }

    #[test]
    fn host_traversal_keeps_exact_symlinks_non_directories_and_raw_failures() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().canonicalize().unwrap();
        fs::create_dir_all(parent.join("real/child")).unwrap();
        symlink("real", parent.join("alias")).unwrap();
        fs::write(parent.join("file"), b"file").unwrap();
        for relative in ["alias", "alias/child", "file"] {
            let path = NormalizedAbsolutePath::new(parent.join(relative)).unwrap();
            let PathOperationResult::Present(raw) = super::super::unix_lstat(&path) else {
                panic!("missing fixture metadata");
            };
            assert_eq!(
                observe(&path),
                PathOperationResult::Present(DirectoryTraversalObservation::ExactLstat(raw)),
            );
        }
        let missing = NormalizedAbsolutePath::new(parent.join("missing")).unwrap();
        assert_eq!(observe(&missing), PathOperationResult::Missing);
        let oversized = "x".repeat(libc::PATH_MAX as usize);
        for invalid in [
            "invalid\0path".to_owned(),
            oversized.clone(),
            oversized + "\0",
        ] {
            let path = NormalizedAbsolutePath::new(parent.join(invalid)).unwrap();
            let PathOperationResult::Error(raw_error) = super::super::unix_lstat(&path) else {
                panic!("expected typed raw path error");
            };
            assert_eq!(observe(&path), PathOperationResult::Error(raw_error));
        }
    }
}
