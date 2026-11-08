//! Small no-follow syscall kernels, matching input_image/linux/fs.rs. The
//! input-image owner is unchanged; common extraction can follow integration.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use nix::fcntl::AtFlags;
use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use nix::sys::stat::fchmod;
use nix::sys::stat::fstat;
use nix::sys::stat::fstatat;
use nix::sys::stat::mkdirat;
use nix::unistd::UnlinkatFlags;
use nix::unistd::unlinkat;

use super::WriteMode;

pub(super) fn stat(file: &File) -> io::Result<libc::stat> {
    fstat(file).map_err(Into::into)
}

pub(super) fn lstat(parent: &File, name: &Path) -> io::Result<libc::stat> {
    fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(Into::into)
}

pub(super) fn directory(parent: &File, name: &Path) -> io::Result<File> {
    openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

pub(super) fn open_regular(
    parent: &File,
    name: &Path,
    write: bool,
    mode: Option<WriteMode>,
) -> io::Result<File> {
    let mut flags = OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    flags |= if write {
        OFlag::O_WRONLY
    } else {
        OFlag::O_RDONLY
    };
    if let Some(mode) = mode {
        flags |= OFlag::O_CREAT;
        if matches!(mode, WriteMode::CreateNew) {
            flags |= OFlag::O_EXCL;
        }
    }
    openat(parent, name, flags, Mode::from_bits_truncate(0o600))
        .map(File::from)
        .map_err(Into::into)
}

pub(super) fn chmod(file: &File, mode: u32) -> io::Result<()> {
    fchmod(file, Mode::from_bits_truncate(mode)).map_err(Into::into)
}

pub(super) fn mkdir(parent: &File, name: &Path, mode: u32) -> io::Result<bool> {
    match mkdirat(parent, name, Mode::from_bits_truncate(mode)) {
        Ok(()) => Ok(true),
        Err(nix::errno::Errno::EEXIST) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn unlink(parent: &File, name: &Path, directory: bool) -> io::Result<()> {
    unlinkat(
        parent,
        name,
        if directory {
            UnlinkatFlags::RemoveDir
        } else {
            UnlinkatFlags::NoRemoveDir
        },
    )
    .map_err(Into::into)
}

pub(super) fn symlink(parent: &File, name: &Path, target: &Path) -> io::Result<()> {
    let name = name.as_os_str().as_bytes();
    let target = target.as_os_str().as_bytes();
    if name.len() > 255 || target.len() > 4096 || name.contains(&0) || target.contains(&0) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let mut terminated_name = [0u8; 256];
    let mut terminated_target = [0u8; 4097];
    terminated_name[..name.len()].copy_from_slice(name);
    terminated_target[..target.len()].copy_from_slice(target);
    if unsafe {
        libc::symlinkat(
            terminated_target.as_ptr().cast(),
            parent.as_raw_fd(),
            terminated_name.as_ptr().cast(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub(super) fn rename(
    from_parent: &File,
    from: &Path,
    to_parent: &File,
    to: &Path,
) -> io::Result<()> {
    nix::fcntl::renameat(from_parent, from, to_parent, to).map_err(Into::into)
}

pub(super) fn read_link(parent: &File, name: &Path, buffer: &mut [u8]) -> io::Result<usize> {
    let bytes = name.as_os_str().as_bytes();
    if bytes.len() > 255 || bytes.contains(&0) {
        return Err(super::invalid("link name exceeds its component bound"));
    }
    let mut name = [0u8; 256];
    name[..bytes.len()].copy_from_slice(bytes);
    // SAFETY: parent is live, name is terminated, buffer is writable. All
    // syscalls are single attempts: EINTR is a bounded operational refusal.
    let count = unsafe {
        libc::readlinkat(
            parent.as_raw_fd(),
            name.as_ptr().cast(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if count < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(count as usize)
    }
}

pub(super) fn read_directory(
    directory: &File,
    mut visitor: impl FnMut(&OsStr) -> io::Result<()>,
) -> io::Result<()> {
    // Separate description avoids changing the caller's directory offset.
    let enumeration = self::directory(directory, Path::new("."))?;
    // Fixed scratch avoids libc DIR's filesystem-sized allocation. Linux's
    // getdents64 ABI has u64 inode, i64 cookie, u16 reclen, u8 type, then a
    // terminated name. Decode lengths without unaligned struct references.
    let mut buffer = [0u8; 8192];
    loop {
        // SAFETY: the directory FD is live and the complete fixed buffer is
        // writable. EINTR is an error; there is no unbounded retry loop.
        let count = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                enumeration.as_raw_fd(),
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        if count == 0 {
            return Ok(());
        }
        let mut remaining = &buffer[..count as usize];
        while !remaining.is_empty() {
            if remaining.len() < 20 {
                return Err(super::invalid("truncated rooted directory record"));
            }
            let size = u16::from_ne_bytes([remaining[16], remaining[17]]) as usize;
            if size < 20 || size > remaining.len() {
                return Err(super::invalid("invalid rooted directory record length"));
            }
            let text = &remaining[19..size];
            let end = text
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| super::invalid("unterminated rooted directory name"))?;
            let name = &text[..end];
            if name != b"." && name != b".." {
                visitor(OsStr::from_bytes(name))?;
            }
            remaining = &remaining[size..];
        }
    }
}
