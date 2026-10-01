//! Descriptor-relative primitives. No operation follows an ambient symlink.

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Component;
use std::path::Path;

use nix::fcntl::AtFlags;
use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use nix::sys::stat::fstatat;

/// One syscall attempt: Interrupted is operational failure, never an
/// unbounded retry that outlives cancellation or the enclosing command.
pub(in crate::runtime::input_image) fn single_call<T>(
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    operation()
}

pub(in crate::runtime::input_image) fn directory(
    parent: &File,
    component: &Path,
) -> io::Result<File> {
    single_call(|| {
        openat(
            parent,
            component,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(Into::into)
    })
}

pub(in crate::runtime::input_image) fn open_anchor(path: &Path) -> io::Result<File> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid image anchor",
        ));
    }
    let mut current = File::from(
        nix::fcntl::open(
            Path::new("/"),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(io::Error::from)?,
    );
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => current = directory(&current, Path::new(name))?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid image anchor",
                ));
            }
        }
    }
    Ok(current)
}

pub(in crate::runtime::input_image) fn parent<'path>(
    root: &File,
    relative: &'path Path,
) -> io::Result<(File, &'path Path)> {
    if relative.as_os_str().len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image path too long",
        ));
    }
    let mut parts = relative.components().peekable();
    let mut current = directory(root, Path::new("."))?;
    let mut depth = 0;
    while let Some(component) = parts.next() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nonrelative image path",
            ));
        };
        depth += 1;
        if depth > 2048 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "image path too deep",
            ));
        }
        if parts.peek().is_none() {
            return Ok((current, Path::new(name)));
        }
        current = directory(&current, Path::new(name))?;
    }
    Ok((current, Path::new(".")))
}

pub(in crate::runtime::input_image) fn lstat(
    root: &File,
    relative: &Path,
) -> io::Result<libc::stat> {
    let (parent, name) = parent(root, relative)?;
    single_call(|| fstatat(&parent, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(Into::into))
}

pub(in crate::runtime::input_image) fn regular(root: &File, relative: &Path) -> io::Result<File> {
    let (parent, name) = parent(root, relative)?;
    let file = single_call(|| {
        openat(
            &parent,
            name,
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)
    })?;
    if nix::sys::stat::fstat(&file)
        .map_err(io::Error::from)?
        .st_mode
        & libc::S_IFMT
        != libc::S_IFREG
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "image source is not regular",
        ));
    }
    Ok(file)
}

pub(in crate::runtime::input_image) fn read_link_into(
    root: &File,
    relative: &Path,
    buffer: &mut [u8],
) -> io::Result<usize> {
    let (parent, name) = parent(root, relative)?;
    let name = CString::new(name.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "image link contains NUL"))?;
    single_call(|| {
        // SAFETY: parent is live; name is terminated and the output slice is writable.
        let count = unsafe {
            libc::readlinkat(
                parent.as_raw_fd(),
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        if count < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(count as usize)
        }
    })
}
