//! Shared descriptor primitives for native output owners.
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use nix::errno::Errno;
use nix::fcntl::AtFlags;
use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use nix::sys::stat::SFlag;
use nix::sys::stat::fstatat;
use nix::sys::stat::mkdirat;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct Identity(u64, u64);
pub(super) fn identity(file: &File) -> io::Result<Identity> {
    let value = file.metadata()?;
    Ok(Identity(value.dev(), value.ino()))
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) identity: Identity,
    pub(super) kind: SFlag,
}
pub(super) fn entry(parent: &File, name: &str) -> io::Result<Option<Entry>> {
    match fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            let kind = SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT;
            if kind != SFlag::S_IFREG && kind != SFlag::S_IFDIR && kind != SFlag::S_IFLNK {
                return Err(io::Error::other("output namespace contains special file"));
            }
            Ok(Some(Entry {
                identity: Identity(stat.st_dev, stat.st_ino),
                kind,
            }))
        }
        Err(Errno::ENOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn directory(parent: &File, name: &str, create: bool) -> io::Result<File> {
    if create {
        match mkdirat(parent, name, Mode::from_bits_truncate(0o755)) {
            Ok(()) | Err(Errno::EEXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}
pub(super) fn fd_path(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
