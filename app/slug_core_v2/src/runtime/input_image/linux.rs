//! Linux observers use borrowed dirent/link buffers and bounded streaming.

use std::ffi::CStr;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::io::{self};
use std::os::fd::IntoRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::DirectoryObservationLimits;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::ImageObservationData;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::ImageObservationRequest;
use slug_workspace_v2::ImageObservationResult;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::InputImageCaptureError;
use super::ScopedImageSource;
mod copy;
pub(super) mod file_copy;
pub(super) mod fs;
pub(super) use copy::copy_image;
pub(super) use copy::finish_directory_modes;
pub(super) use copy::restore_directories;
pub(super) use fs::open_anchor;

fn active_check(active: &dyn Fn() -> bool) -> Result<(), InputImageCaptureError> {
    if active() {
        Ok(())
    } else {
        Err(InputImageCaptureError::Cancelled)
    }
}

pub(super) fn stat_value(stat: &libc::stat) -> PathLstat {
    let kind = match stat.st_mode & libc::S_IFMT {
        libc::S_IFREG => PathNodeKind::RegularFile,
        libc::S_IFDIR => PathNodeKind::Directory,
        libc::S_IFLNK => PathNodeKind::Symlink,
        _ => PathNodeKind::SpecialFile,
    };
    PathLstat::new(
        kind,
        stat.st_size,
        stat.st_mtime
            .saturating_mul(1000)
            .saturating_add(stat.st_mtime_nsec / 1_000_000),
        stat.st_ctime
            .saturating_mul(1000)
            .saturating_add(stat.st_ctime_nsec / 1_000_000),
        stat.st_ino as i64,
        (stat.st_mode & 0o7777) as i32,
    )
}

fn unchanged(before: &libc::stat, after: &libc::stat) -> bool {
    before.st_dev == after.st_dev
        && before.st_ino == after.st_ino
        && before.st_mode == after.st_mode
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_mtime_nsec == after.st_mtime_nsec
        && before.st_ctime == after.st_ctime
        && before.st_ctime_nsec == after.st_ctime_nsec
}

pub(super) fn fd_stat(file: &File) -> io::Result<libc::stat> {
    nix::sys::stat::fstat(file).map_err(Into::into)
}

pub(super) fn observed<T>(result: io::Result<T>) -> PathOperationResult<T> {
    match result {
        Ok(value) => PathOperationResult::Present(value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => PathOperationResult::Missing,
        Err(error) => PathOperationResult::Error(PathObservationError::Io {
            kind: error.kind().into(),
            raw_os_error: error.raw_os_error(),
        }),
    }
}

fn inconsistency(before: &libc::stat, after: Option<&libc::stat>) -> PathObservationError {
    PathObservationError::InconsistentState {
        before: Some(stat_value(before)),
        after: after.map(stat_value),
    }
}

pub(super) fn reject_interrupted(
    data: &ImageObservationData,
) -> Result<(), InputImageCaptureError> {
    use slug_workspace_v2::PathIoErrorKind;
    match data {
        ImageObservationData::Lstat(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::Interrupted,
            raw_os_error,
        }))
        | ImageObservationData::Digest(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::Interrupted,
            raw_os_error,
        }))
        | ImageObservationData::Directory(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::Interrupted,
            raw_os_error,
        }))
        | ImageObservationData::ReadLink(PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::Interrupted,
            raw_os_error,
        })) => Err(raw_os_error
            .map(io::Error::from_raw_os_error)
            .unwrap_or_else(|| io::Error::from(io::ErrorKind::Interrupted))
            .into()),
        _ => Ok(()),
    }
}

pub(super) fn observe(
    source: &ScopedImageSource,
    demand: &PathObservationDemand,
    request: ImageObservationRequest,
    budget: &ImageDescriptorBudget,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, InputImageCaptureError> {
    let relative = demand
        .path()
        .as_path()
        .strip_prefix(source.anchor.as_path())
        .map_err(|_| InputImageCaptureError::Unauthorized)?;
    // Reserve the owned request/header before any I/O. The demand's path pin
    // is reused, retaining its original workspace credit.
    let mut credit = budget.try_reserve(
        request
            .scope_anchor_root()
            .as_path()
            .as_os_str()
            .len()
            .checked_add(256)
            .ok_or(InputImageCaptureError::LimitExceeded)?,
    )?;
    active_check(active)?;
    let data = match request.operation() {
        ImageObservationOperation::Lstat => ImageObservationData::Lstat(observed(
            fs::lstat(&source.root, relative).map(|stat| stat_value(&stat)),
        )),
        ImageObservationOperation::BoundedDigest { max_bytes } => {
            digest(&source.root, relative, max_bytes, active)?
        }
        ImageObservationOperation::BoundedReadLink { max_target_bytes } => read_link(
            &source.root,
            relative,
            max_target_bytes,
            &mut credit,
            active,
        )?,
        ImageObservationOperation::BoundedDirectory(limits) => {
            directory(&source.root, relative, limits, &mut credit, active)?
        }
        // ELF requires the separately declared exact member/certificate;
        // generic image authority cannot fulfill this operation.
        ImageObservationOperation::BoundedElfMetadata { .. } => {
            return Err(InputImageCaptureError::Unauthorized);
        }
    };
    reject_interrupted(&data)?;
    active_check(active)?;
    Ok(PathObservationResult::InputImage(
        ImageObservationResult::new(
            demand
                .image_path()
                .ok_or(InputImageCaptureError::Unauthorized)?
                .clone(),
            request,
            data,
            credit,
        )?,
    ))
}

fn digest(
    root: &File,
    relative: &Path,
    max_bytes: u64,
    active: &dyn Fn() -> bool,
) -> Result<ImageObservationData, InputImageCaptureError> {
    let before = match fs::lstat(root, relative) {
        Ok(stat) => stat,
        Err(error) => return Ok(ImageObservationData::Digest(observed(Err(error)))),
    };
    if stat_value(&before).kind() != PathNodeKind::RegularFile {
        return Ok(ImageObservationData::Digest(PathOperationResult::Error(
            PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: stat_value(&before).kind(),
            },
        )));
    }
    if before.st_size < 0 || before.st_size as u64 > max_bytes {
        return Ok(ImageObservationData::LimitExceeded);
    }
    let mut file = match fs::regular(root, relative) {
        Ok(file) => file,
        Err(error) => return Ok(ImageObservationData::Digest(observed(Err(error)))),
    };
    let opened = fd_stat(&file)?;
    if !unchanged(&before, &opened) {
        return Ok(ImageObservationData::Digest(PathOperationResult::Error(
            inconsistency(&before, Some(&opened)),
        )));
    }
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        active_check(active)?;
        // One extra byte distinguishes an exact bound from a growing file;
        // no source-controlled allocation occurs.
        let available = max_bytes.saturating_sub(count).min(buffer.len() as u64) as usize;
        let read_len = if available == 0 { 1 } else { available };
        let read = fs::single_call(|| file.read(&mut buffer[..read_len]))?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or(InputImageCaptureError::LimitExceeded)?;
        if count > max_bytes {
            return Ok(ImageObservationData::LimitExceeded);
        }
        hash.update(&buffer[..read]);
    }
    let after = fd_stat(&file)?;
    let named = fs::lstat(root, relative)?;
    if !unchanged(&before, &after) || !unchanged(&after, &named) || count != after.st_size as u64 {
        return Ok(ImageObservationData::Digest(PathOperationResult::Error(
            inconsistency(&before, Some(&after)),
        )));
    }
    Ok(ImageObservationData::Digest(PathOperationResult::Present(
        FileContentDigest::new(hash.finalize().into(), count)
            .ok_or(InputImageCaptureError::LimitExceeded)?,
    )))
}

fn read_link(
    root: &File,
    relative: &Path,
    limit: usize,
    credit: &mut ImageDescriptorReservation,
    active: &dyn Fn() -> bool,
) -> Result<ImageObservationData, InputImageCaptureError> {
    if limit > 4096 {
        return Ok(ImageObservationData::LimitExceeded);
    }
    let before = match fs::lstat(root, relative) {
        Ok(value) => value,
        Err(error) => return Ok(ImageObservationData::ReadLink(observed(Err(error)))),
    };
    if stat_value(&before).kind() != PathNodeKind::Symlink {
        return Ok(ImageObservationData::ReadLink(PathOperationResult::Error(
            PathObservationError::NotALink,
        )));
    }
    let mut buffer = [0u8; 4097];
    active_check(active)?;
    let length = match fs::read_link_into(root, relative, &mut buffer[..limit + 1]) {
        Ok(value) => value,
        Err(error) => return Ok(ImageObservationData::ReadLink(observed(Err(error)))),
    };
    if length > limit {
        return Ok(ImageObservationData::LimitExceeded);
    }
    let after = fs::lstat(root, relative)?;
    if !unchanged(&before, &after) {
        return Ok(ImageObservationData::ReadLink(PathOperationResult::Error(
            inconsistency(&before, Some(&after)),
        )));
    }
    credit.try_grow(
        length
            .checked_add(96)
            .ok_or(InputImageCaptureError::LimitExceeded)?,
    )?;
    Ok(ImageObservationData::ReadLink(
        PathOperationResult::Present(Arc::new(PathBuf::from(OsString::from_vec(
            buffer[..length].to_vec(),
        )))),
    ))
}

struct DirectoryStream(*mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

fn directory(
    root: &File,
    relative: &Path,
    limits: DirectoryObservationLimits,
    credit: &mut ImageDescriptorReservation,
    active: &dyn Fn() -> bool,
) -> Result<ImageObservationData, InputImageCaptureError> {
    if limits.max_entries > 32768
        || limits.max_component_bytes > 4096
        || limits.max_name_bytes_total > 64 * 1024 * 1024
    {
        return Ok(ImageObservationData::LimitExceeded);
    }
    let before = match fs::lstat(root, relative) {
        Ok(stat) => stat,
        Err(error) => return Ok(ImageObservationData::Directory(observed(Err(error)))),
    };
    if stat_value(&before).kind() != PathNodeKind::Directory {
        return Ok(ImageObservationData::Directory(PathOperationResult::Error(
            PathObservationError::WrongKind {
                expected: PathNodeKind::Directory,
                actual: stat_value(&before).kind(),
            },
        )));
    }
    let file = if relative.as_os_str().is_empty() {
        // dup shares the directory cursor. A fresh open-file description is
        // required because the same authority reobserves this complete tree.
        fs::directory(root, Path::new("."))?
    } else {
        let (parent, name) = fs::parent(root, relative)?;
        match fs::directory(&parent, name) {
            Ok(file) => file,
            Err(error) => return Ok(ImageObservationData::Directory(observed(Err(error)))),
        }
    };
    let opened = fd_stat(&file)?;
    if !unchanged(&before, &opened) {
        return Ok(ImageObservationData::Directory(PathOperationResult::Error(
            inconsistency(&before, Some(&opened)),
        )));
    }
    let fd = fs::directory(&file, Path::new("."))?.into_raw_fd();
    let pointer = unsafe { libc::fdopendir(fd) };
    if pointer.is_null() {
        unsafe {
            libc::close(fd);
        }
        return Err(io::Error::last_os_error().into());
    }
    let stream = DirectoryStream(pointer);
    let mut rows = Vec::new();
    let mut name_bytes = 0usize;
    loop {
        active_check(active)?;
        // readdir owns one fixed native buffer; inspect its borrowed name before
        // allocating an OS string, row capacity, or sorted output storage.
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let errno = unsafe { *libc::__errno_location() };
            if errno != 0 {
                return Err(io::Error::from_raw_os_error(errno).into());
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        name_bytes = name_bytes
            .checked_add(name.len())
            .ok_or(InputImageCaptureError::LimitExceeded)?;
        if rows.len() == limits.max_entries
            || name.len() > limits.max_component_bytes
            || name_bytes > limits.max_name_bytes_total
        {
            return Ok(ImageObservationData::LimitExceeded);
        }
        if rows.len() == rows.capacity() {
            let capacity = rows
                .capacity()
                .max(4)
                .checked_mul(2)
                .ok_or(InputImageCaptureError::LimitExceeded)?
                .min(limits.max_entries);
            credit.try_grow(
                capacity
                    .checked_mul(std::mem::size_of::<PathDirectoryEntry>())
                    .ok_or(InputImageCaptureError::LimitExceeded)?,
            )?;
            rows.try_reserve_exact(capacity - rows.len())
                .map_err(|_| InputImageCaptureError::LimitExceeded)?;
        }
        credit.try_grow(
            name.len()
                .checked_add(16)
                .ok_or(InputImageCaptureError::LimitExceeded)?,
        )?;
        let name = PathDirectoryName::new(OsString::from_vec(name.to_vec()))
            .map_err(|_| InputImageCaptureError::Unauthorized)?;
        let kind = match unsafe { (*entry).d_type } {
            libc::DT_REG => PathDirectoryEntryKind::File,
            libc::DT_DIR => PathDirectoryEntryKind::Directory,
            libc::DT_LNK => PathDirectoryEntryKind::Symlink,
            _ => PathDirectoryEntryKind::Unknown,
        };
        rows.push(PathDirectoryEntry::new(name, kind));
    }
    let after = fd_stat(&file)?;
    let named = fs::lstat(root, relative)?;
    if !unchanged(&before, &after) || !unchanged(&after, &named) {
        return Ok(ImageObservationData::Directory(PathOperationResult::Error(
            inconsistency(&before, Some(&after)),
        )));
    }
    credit.try_grow(
        rows.len()
            .checked_mul(std::mem::size_of::<PathDirectoryEntry>())
            .and_then(|n| n.checked_add(64))
            .ok_or(InputImageCaptureError::LimitExceeded)?,
    )?;
    Ok(ImageObservationData::Directory(
        PathOperationResult::Present(PathDirectoryEntries::new(rows)),
    ))
}
