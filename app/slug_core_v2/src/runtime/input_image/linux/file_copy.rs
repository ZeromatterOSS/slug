//! One authenticated regular source; no directory enumeration or link traversal.
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use nix::fcntl::OFlag;
use nix::fcntl::openat;
use nix::sys::stat::Mode;
use nix::sys::stat::fchmod;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ObservedInputImage;

use super::active_check;
use super::fd_stat;
use super::fs;
use super::unchanged;
use crate::runtime::input_image::InputImageCaptureError;
use crate::runtime::input_image::ScopedImageSource;

pub(in crate::runtime::input_image) fn copy_file(
    source: &ScopedImageSource,
    destination: &File,
    certificate: &ObservedInputImage,
    active: &dyn Fn() -> bool,
) -> Result<(), InputImageCaptureError> {
    active_check(active)?;
    let entry = &certificate.entries()[0];
    let expected = entry
        .file_digest()
        .ok_or(InputImageCaptureError::Unauthorized)?;
    let relative = entry
        .path()
        .as_path()
        .strip_prefix(source.anchor.as_path())
        .map_err(|_| InputImageCaptureError::Unauthorized)?;
    let before = fs::lstat(&source.root, relative)?;
    let mut input = fs::regular(&source.root, relative)?;
    let opened = fd_stat(&input)?;
    if !unchanged(&before, &opened)
        || opened.st_size < 0
        || opened.st_size as u64 != expected.size_bytes()
        || opened.st_mode & 0o7777 != entry.mode()
    {
        return Err(InputImageCaptureError::StaleSource);
    }
    let mut output = File::from(
        openat(
            destination,
            Path::new("leaf"),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(std::io::Error::from)?,
    );
    let mut hash = Sha256::new();
    let mut copied = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        active_check(active)?;
        let length = expected
            .size_bytes()
            .saturating_sub(copied)
            .min(buffer.len() as u64)
            .max(1) as usize;
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
            let n = output.write(&buffer[written..count])?;
            if n == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into());
            }
            written += n;
        }
    }
    let digest: [u8; 32] = hash.finalize().into();
    let after = fd_stat(&input)?;
    let named = fs::lstat(&source.root, relative)?;
    if copied != expected.size_bytes()
        || &digest != expected.sha256()
        || !unchanged(&before, &after)
        || !unchanged(&after, &named)
    {
        return Err(InputImageCaptureError::StaleSource);
    }
    nix::unistd::fsync(&output).map_err(std::io::Error::from)?;
    fchmod(&output, Mode::from_bits_truncate(entry.mode())).map_err(std::io::Error::from)?;
    let output_stat = fd_stat(&output)?;
    if output_stat.st_mode & 0o7777 != entry.mode()
        || output_stat.st_size < 0
        || output_stat.st_size as u64 != expected.size_bytes()
    {
        return Err(InputImageCaptureError::StaleSource);
    }
    active_check(active)
}
