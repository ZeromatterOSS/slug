//! Exact-mode private staging. Source I/O is always descriptor relative.

use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use nix::sys::stat::fchmod;
use nix::sys::stat::mkdirat;
use nix::unistd::symlinkat;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::InputImageEntry;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImage;

use super::active_check;
use super::fd_stat;
use super::fs;
use super::unchanged;
use crate::runtime::input_image::InputImageCaptureError;
use crate::runtime::input_image::ScopedImageSource;

fn relative<'a>(
    entry: &'a InputImageEntry,
    certificate: &ObservedInputImage,
) -> Result<&'a Path, InputImageCaptureError> {
    entry
        .path()
        .as_path()
        .strip_prefix(certificate.source_root().as_path())
        .map_err(|_| InputImageCaptureError::Unauthorized)
}

pub(in crate::runtime::input_image) fn copy_image(
    source: &ScopedImageSource,
    destination: &File,
    certificate: &ObservedInputImage,
    active: &dyn Fn() -> bool,
) -> Result<(), InputImageCaptureError> {
    let mut total = 0u64;
    // Canonical absolute paths put every directory before its descendants.
    // Symlinks are emitted last and are never traversed during native capture.
    for entry in certificate.entries() {
        active_check(active)?;
        let target = relative(entry, certificate)?;
        match entry.kind() {
            InputImageEntryKind::Directory => {
                if target.as_os_str().is_empty() {
                    continue;
                }
                let (parent, name) = fs::parent(destination, target)?;
                mkdirat(&parent, name, Mode::from_bits_truncate(0o700))
                    .map_err(std::io::Error::from)?;
            }
            InputImageEntryKind::RegularFile(expected) => {
                let source_path = entry
                    .path()
                    .as_path()
                    .strip_prefix(source.anchor.as_path())
                    .map_err(|_| InputImageCaptureError::Unauthorized)?;
                let before = fs::lstat(&source.root, source_path)?;
                let mut input = fs::regular(&source.root, source_path)?;
                let opened = fd_stat(&input)?;
                if !unchanged(&before, &opened)
                    || before.st_size < 0
                    || before.st_size as u64 != expected.size_bytes()
                    || before.st_mode & 0o7777 != entry.mode()
                {
                    return Err(InputImageCaptureError::StaleSource);
                }
                let (parent, name) = fs::parent(destination, target)?;
                let mut output = File::from(
                    openat(
                        &parent,
                        name,
                        OFlag::O_WRONLY
                            | OFlag::O_CREAT
                            | OFlag::O_EXCL
                            | OFlag::O_NOFOLLOW
                            | OFlag::O_CLOEXEC,
                        Mode::from_bits_truncate(0o600),
                    )
                    .map_err(std::io::Error::from)?,
                );
                let mut hash = Sha256::new();
                let mut copied = 0u64;
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    active_check(active)?;
                    let remain = expected.size_bytes().saturating_sub(copied);
                    let length = remain.min(buffer.len() as u64).max(1) as usize;
                    let count = fs::single_call(|| input.read(&mut buffer[..length]))?;
                    if count == 0 {
                        break;
                    }
                    copied = copied
                        .checked_add(count as u64)
                        .ok_or(InputImageCaptureError::LimitExceeded)?;
                    if copied > expected.size_bytes() {
                        return Err(InputImageCaptureError::StaleSource);
                    }
                    hash.update(&buffer[..count]);
                    let mut written = 0;
                    while written < count {
                        active_check(active)?;
                        let bytes = output.write(&buffer[written..count])?;
                        if bytes == 0 {
                            return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into());
                        }
                        written += bytes;
                    }
                }
                let digest: [u8; 32] = hash.finalize().into();
                let after = fd_stat(&input)?;
                let named = fs::lstat(&source.root, source_path)?;
                if copied != expected.size_bytes()
                    || &digest != expected.sha256()
                    || !unchanged(&before, &after)
                    || !unchanged(&after, &named)
                {
                    return Err(InputImageCaptureError::StaleSource);
                }
                nix::unistd::fsync(&output).map_err(std::io::Error::from)?;
                // Apply exact permission bits after writing (writing may clear
                // set-ID bits); this is private staging, not process admission.
                fchmod(&output, Mode::from_bits_truncate(entry.mode()))
                    .map_err(std::io::Error::from)?;
                if fd_stat(&output)?.st_mode & 0o7777 != entry.mode() {
                    return Err(InputImageCaptureError::StaleSource);
                }
                total = total
                    .checked_add(copied)
                    .ok_or(InputImageCaptureError::LimitExceeded)?;
            }
            InputImageEntryKind::Symlink => {}
        }
    }
    if total != certificate.total_file_bytes() {
        return Err(InputImageCaptureError::StaleSource);
    }
    for entry in certificate
        .entries()
        .iter()
        .filter(|entry| entry.kind() == InputImageEntryKind::Symlink)
    {
        active_check(active)?;
        let resolved = entry
            .resolved_alias_path()
            .ok_or(InputImageCaptureError::Unauthorized)?;
        if !resolved
            .as_path()
            .starts_with(certificate.source_root().as_path())
            || certificate
                .entries()
                .binary_search_by(|candidate| candidate.path().as_path().cmp(resolved.as_path()))
                .is_err()
        {
            return Err(InputImageCaptureError::Unauthorized);
        }
        // Preserve literal links. An absolute source link is not rewritten to
        // the stage spelling; future namespace consumers must mount the image
        // at its certified source spelling. No host-following API is exposed.
        let literal = entry
            .link_target()
            .ok_or(InputImageCaptureError::Unauthorized)?;
        let (parent, name) = fs::parent(destination, relative(entry, certificate)?)?;
        symlinkat(literal, &parent, name).map_err(std::io::Error::from)?;
    }
    Ok(())
}

pub(in crate::runtime::input_image) fn finish_directory_modes(
    root: &File,
    certificate: &ObservedInputImage,
    active: &dyn Fn() -> bool,
) -> Result<(), InputImageCaptureError> {
    for entry in certificate
        .entries()
        .iter()
        .rev()
        .filter(|entry| entry.kind() == InputImageEntryKind::Directory)
    {
        active_check(active)?;
        let relative = relative(entry, certificate)?;
        let directory = if relative.as_os_str().is_empty() {
            fs::directory(root, Path::new("."))?
        } else {
            let (parent, name) = fs::parent(root, relative)?;
            fs::directory(&parent, name)?
        };
        fchmod(&directory, Mode::from_bits_truncate(entry.mode())).map_err(std::io::Error::from)?;
        if fd_stat(&directory)?.st_mode & 0o7777 != entry.mode() {
            return Err(InputImageCaptureError::StaleSource);
        }
    }
    Ok(())
}

pub(in crate::runtime::input_image) fn restore_directories(
    root: &File,
    certificate: &ObservedInputImage,
) {
    let _ = fchmod(root, Mode::from_bits_truncate(0o700));
    for entry in certificate
        .entries()
        .iter()
        .filter(|entry| entry.kind() == InputImageEntryKind::Directory)
    {
        let Ok(relative) = relative(entry, certificate) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let Ok((parent, name)) = fs::parent(root, relative) else {
            continue;
        };
        let Ok(name) = CString::new(name.as_os_str().as_bytes()) else {
            continue;
        };
        // The stage has exactly one native writer and its FD/path never escapes.
        // Restore a known owned directory before opening mode-000 children;
        // no symlink entry is included in this certificate directory pass.
        let _ = unsafe { libc::fchmodat(parent.as_raw_fd(), name.as_ptr(), 0o700, 0) };
    }
}
