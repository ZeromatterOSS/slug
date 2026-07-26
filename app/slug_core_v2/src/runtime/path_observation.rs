#![allow(dead_code)]
// This private kernel is exposed only through the retained materializer
// bridge; command/DICE activation remains deliberately absent.

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

#[derive(Debug, PartialEq, Eq, Allocative)]
struct RetainedMaterializationRoot {
    instance: PathObservationInstanceId,
    root: NormalizedAbsolutePath,
}

/// A compact authority snapshot whose lifetime is tied to the real owner.
///
/// This intentionally has no `Clone`, `Dupe`, map, or interner. The future
/// caller must retain the materializer owner for the entire observation call.
#[derive(Debug, Allocative)]
struct RetainedMaterializationRoots<'owner> {
    entries: Arc<[RetainedMaterializationRoot]>,
    owner: PhantomData<&'owner ()>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PathObservationKernelError {
    ZeroRetainedInstance,
    DuplicateRetainedInstance(PathObservationInstanceId),
    DuplicateDemand(PathObservationDemand),
    ZeroDemandInstance(PathObservationDemand),
    UnknownDemandInstance(PathObservationDemand),
    UnsupportedLstat,
    Epoch(PathObservationEpochError),
}

pub(super) fn observe_native<T>(
    owner: &T,
    roots: impl IntoIterator<Item = (PathObservationInstanceId, NormalizedAbsolutePath)>,
    demands: impl IntoIterator<Item = PathObservationDemand>,
) -> Result<PathObservationEpoch, PathObservationKernelError> {
    let retained = RetainedMaterializationRoots::new(owner, roots)?;
    #[cfg(unix)]
    {
        observe_unix(&retained, demands)
    }
    #[cfg(windows)]
    {
        windows_native::observe_windows(&retained, demands)
    }
}

impl<'owner> RetainedMaterializationRoots<'owner> {
    fn new<T>(
        owner: &'owner T,
        entries: impl IntoIterator<Item = (PathObservationInstanceId, NormalizedAbsolutePath)>,
    ) -> Result<Self, PathObservationKernelError> {
        let _ = owner;
        let mut entries = entries
            .into_iter()
            .map(|(instance, root)| RetainedMaterializationRoot { instance, root })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.instance);
        if entries.iter().any(|entry| entry.instance.value() == 0) {
            return Err(PathObservationKernelError::ZeroRetainedInstance);
        }
        if let Some(instance) = entries
            .windows(2)
            .find(|pair| pair[0].instance == pair[1].instance)
            .map(|pair| pair[0].instance)
        {
            return Err(PathObservationKernelError::DuplicateRetainedInstance(
                instance,
            ));
        }
        Ok(Self {
            entries: Arc::from(entries),
            owner: PhantomData,
        })
    }

    fn authorizes(&self, instance: PathObservationInstanceId) -> bool {
        self.entries
            .binary_search_by_key(&instance, |entry| entry.instance)
            .is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryFailure {
    Refine(PathObservationError),
    Final(PathObservationError),
}

trait ObservationOperations {
    fn supports_lstat(&mut self) -> bool;

    fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat>;

    fn read_link(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<PathBuf>, PrimaryFailure>;

    fn file_bytes(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<[u8]>, PrimaryFailure>;

    fn directory_entries(
        &mut self,
        path: &NormalizedAbsolutePath,
    ) -> Result<PathDirectoryEntries, PrimaryFailure>;
}

fn observe_with(
    retained: &RetainedMaterializationRoots<'_>,
    demands: impl IntoIterator<Item = PathObservationDemand>,
    operations: &mut impl ObservationOperations,
) -> Result<PathObservationEpoch, PathObservationKernelError> {
    let mut demands = demands.into_iter().collect::<Vec<_>>();
    demands.sort_unstable();

    if let Some(duplicate) = demands
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0].clone())
    {
        return Err(PathObservationKernelError::DuplicateDemand(duplicate));
    }

    for demand in &demands {
        if let PathObservationNamespace::Materialization(instance) = demand.namespace() {
            if instance.value() == 0 {
                return Err(PathObservationKernelError::ZeroDemandInstance(
                    demand.clone(),
                ));
            }
            if !retained.authorizes(instance) {
                return Err(PathObservationKernelError::UnknownDemandInstance(
                    demand.clone(),
                ));
            }
        }
    }

    if demands
        .iter()
        .any(|demand| demand.operation() == PathObservationOperation::Lstat)
        && !operations.supports_lstat()
    {
        return Err(PathObservationKernelError::UnsupportedLstat);
    }

    let observations = demands.into_iter().map(|demand| {
        let result = observe_one(&demand, operations);
        (demand, result)
    });
    PathObservationEpoch::new(observations).map_err(PathObservationKernelError::Epoch)
}

fn observe_one(
    demand: &PathObservationDemand,
    operations: &mut impl ObservationOperations,
) -> PathObservationResult {
    match demand.operation() {
        PathObservationOperation::Lstat => {
            PathObservationResult::Lstat(operations.lstat(demand.path()))
        }
        PathObservationOperation::ReadLink => {
            PathObservationResult::ReadLink(match operations.read_link(demand.path()) {
                Ok(target) => PathOperationResult::Present(target),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_read_link(error, operations.lstat(demand.path()))
                }
            })
        }
        PathObservationOperation::FileBytes => {
            PathObservationResult::FileBytes(match operations.file_bytes(demand.path()) {
                Ok(bytes) => PathOperationResult::Present(bytes),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_file_bytes(error, operations.lstat(demand.path()))
                }
            })
        }
        PathObservationOperation::DirectoryEntries => PathObservationResult::DirectoryEntries(
            match operations.directory_entries(demand.path()) {
                Ok(entries) => PathOperationResult::Present(entries),
                Err(PrimaryFailure::Final(error)) => PathOperationResult::Error(error),
                Err(PrimaryFailure::Refine(error)) => {
                    refine_directory_entries(error, operations.lstat(demand.path()))
                }
            },
        ),
    }
}

fn refine_read_link(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<Arc<PathBuf>> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() != PathNodeKind::Symlink => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::Symlink,
                actual: lstat.kind(),
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

fn refine_file_bytes(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<Arc<[u8]>> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() == PathNodeKind::Directory => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

fn refine_directory_entries(
    original: PathObservationError,
    auxiliary: PathOperationResult<PathLstat>,
) -> PathOperationResult<PathDirectoryEntries> {
    match auxiliary {
        PathOperationResult::Missing => PathOperationResult::Missing,
        PathOperationResult::Present(lstat) if lstat.kind() != PathNodeKind::Directory => {
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::Directory,
                actual: lstat.kind(),
            })
        }
        PathOperationResult::Present(_) | PathOperationResult::Error(_) => {
            PathOperationResult::Error(original)
        }
    }
}

#[cfg(unix)]
fn observe_unix(
    retained: &RetainedMaterializationRoots<'_>,
    demands: impl IntoIterator<Item = PathObservationDemand>,
) -> Result<PathObservationEpoch, PathObservationKernelError> {
    observe_with(retained, demands, &mut UnixPathObservationAdapter)
}

#[cfg(unix)]
struct UnixPathObservationAdapter;

#[cfg(unix)]
impl ObservationOperations for UnixPathObservationAdapter {
    fn supports_lstat(&mut self) -> bool {
        true
    }

    fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat> {
        unix_lstat(path)
    }

    fn read_link(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<PathBuf>, PrimaryFailure> {
        match retry_interrupted(|| std::fs::read_link(path.as_path())) {
            Ok(target) => Ok(Arc::new(target)),
            Err(error) => {
                let observed = path_io_error(&error);
                if error.raw_os_error() == Some(nix::libc::EINVAL) {
                    Err(PrimaryFailure::Refine(PathObservationError::NotALink))
                } else if is_missing_error(&error) {
                    Err(PrimaryFailure::Refine(observed))
                } else {
                    Err(PrimaryFailure::Final(observed))
                }
            }
        }
    }

    fn file_bytes(&mut self, path: &NormalizedAbsolutePath) -> Result<Arc<[u8]>, PrimaryFailure> {
        match retry_interrupted(|| std::fs::read(path.as_path())) {
            Ok(bytes) => Ok(Arc::from(bytes)),
            Err(error) => {
                let observed = path_io_error(&error);
                if is_missing_error(&error) || error.kind() == std::io::ErrorKind::IsADirectory {
                    Err(PrimaryFailure::Refine(observed))
                } else {
                    Err(PrimaryFailure::Final(observed))
                }
            }
        }
    }

    fn directory_entries(
        &mut self,
        path: &NormalizedAbsolutePath,
    ) -> Result<PathDirectoryEntries, PrimaryFailure> {
        unix_directory_entries_with(path, &mut LibcUnixDirectoryApi)
    }
}

#[cfg(unix)]
fn retry_interrupted<T>(mut operation: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    loop {
        match operation() {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

#[cfg(unix)]
fn path_io_error(error: &std::io::Error) -> PathObservationError {
    PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::from(error.kind()),
        raw_os_error: error.raw_os_error(),
    }
}

#[cfg(unix)]
fn raw_path_io_error(raw: i32) -> PathObservationError {
    path_io_error(&std::io::Error::from_raw_os_error(raw))
}

#[cfg(unix)]
fn is_missing_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

#[cfg(unix)]
fn unix_lstat(path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat> {
    use std::os::unix::fs::MetadataExt;

    match retry_interrupted(|| std::fs::symlink_metadata(path.as_path())) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            let kind = if file_type.is_symlink() {
                PathNodeKind::Symlink
            } else if file_type.is_dir() {
                PathNodeKind::Directory
            } else if file_type.is_file() {
                PathNodeKind::RegularFile
            } else {
                PathNodeKind::SpecialFile
            };
            PathOperationResult::Present(PathLstat::new(
                kind,
                metadata.size() as i64,
                metadata
                    .mtime()
                    .wrapping_mul(1_000)
                    .wrapping_add(metadata.mtime_nsec() / 1_000_000),
                metadata
                    .ctime()
                    .wrapping_mul(1_000)
                    .wrapping_add(metadata.ctime_nsec() / 1_000_000),
                metadata.ino() as i64,
                (metadata.mode() & 0o777) as i32,
            ))
        }
        Err(error) if is_missing_error(&error) => PathOperationResult::Missing,
        Err(error) => PathOperationResult::Error(path_io_error(&error)),
    }
}

#[cfg(unix)]
enum UnixDirectoryRead {
    Name(Vec<u8>),
    Null(i32),
}

#[cfg(unix)]
trait UnixDirectoryApi {
    type Handle: Copy;

    fn open_once(&mut self, path: &std::ffi::CStr) -> Result<Self::Handle, i32>;
    fn clear_errno(&mut self);
    fn read_once(&mut self, handle: Self::Handle) -> UnixDirectoryRead;
    fn close_once(&mut self, handle: Self::Handle) -> Result<(), i32>;
}

#[cfg(unix)]
struct UnixDirectoryOwner<'api, A: UnixDirectoryApi> {
    handle: Option<A::Handle>,
    api: &'api mut A,
}

#[cfg(unix)]
impl<'api, A: UnixDirectoryApi> UnixDirectoryOwner<'api, A> {
    fn new(handle: A::Handle, api: &'api mut A) -> Self {
        Self {
            handle: Some(handle),
            api,
        }
    }

    fn clear_errno(&mut self) {
        self.api.clear_errno();
    }

    fn read_once(&mut self) -> UnixDirectoryRead {
        self.api
            .read_once(self.handle.expect("directory owner must be armed"))
    }

    fn close(mut self) -> Result<(), i32> {
        let handle = self.handle.take().expect("directory owner must be armed");
        self.api.close_once(handle)
    }
}

#[cfg(unix)]
impl<A: UnixDirectoryApi> Drop for UnixDirectoryOwner<'_, A> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = self.api.close_once(handle);
        }
    }
}

#[cfg(unix)]
struct LibcUnixDirectoryApi;

#[cfg(unix)]
impl UnixDirectoryApi for LibcUnixDirectoryApi {
    type Handle = std::ptr::NonNull<nix::libc::DIR>;

    fn open_once(&mut self, path: &std::ffi::CStr) -> Result<Self::Handle, i32> {
        // SAFETY: `path` is a live, NUL-terminated C string for the duration
        // of the call. The returned handle is immediately placed in its owner.
        let directory = unsafe { nix::libc::opendir(path.as_ptr()) };
        std::ptr::NonNull::new(directory).ok_or_else(nix::errno::Errno::last_raw)
    }

    fn clear_errno(&mut self) {
        nix::errno::Errno::clear();
    }

    fn read_once(&mut self, handle: Self::Handle) -> UnixDirectoryRead {
        // SAFETY: the handle remains owned and live. `d_name` is copied before
        // any subsequent `readdir` call can invalidate the transient entry.
        let entry = unsafe { nix::libc::readdir(handle.as_ptr()) };
        if entry.is_null() {
            UnixDirectoryRead::Null(nix::errno::Errno::last_raw())
        } else {
            // SAFETY: POSIX guarantees a NUL-terminated `d_name` for a
            // successful `readdir` result.
            let name = unsafe {
                std::ffi::CStr::from_ptr((*entry).d_name.as_ptr())
                    .to_bytes()
                    .to_vec()
            };
            UnixDirectoryRead::Name(name)
        }
    }

    fn close_once(&mut self, handle: Self::Handle) -> Result<(), i32> {
        // SAFETY: `UnixDirectoryOwner` takes and disarms this handle before
        // making the sole explicit close call.
        let result = unsafe { nix::libc::closedir(handle.as_ptr()) };
        if result == 0 {
            Ok(())
        } else {
            Err(nix::errno::Errno::last_raw())
        }
    }
}

#[cfg(unix)]
fn unix_directory_entries_with<A: UnixDirectoryApi>(
    path: &NormalizedAbsolutePath,
    api: &mut A,
) -> Result<PathDirectoryEntries, PrimaryFailure> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    let c_path = std::ffi::CString::new(path.as_path().as_os_str().as_bytes()).map_err(|_| {
        PrimaryFailure::Final(PathObservationError::Io {
            kind: slug_workspace_v2::PathIoErrorKind::InvalidInput,
            raw_os_error: None,
        })
    })?;

    let handle = loop {
        match api.open_once(&c_path) {
            Ok(handle) => break handle,
            Err(raw) if raw == nix::libc::EINTR => continue,
            Err(raw) => return Err(classify_directory_error(raw)),
        }
    };

    let mut owner = UnixDirectoryOwner::new(handle, api);
    let mut names = Vec::new();
    loop {
        owner.clear_errno();
        match owner.read_once() {
            UnixDirectoryRead::Name(name) => {
                if name == b"." || name == b".." {
                    continue;
                }
                let name = std::ffi::OsString::from_vec(name);
                let name = match slug_workspace_v2::PathDirectoryName::new(name) {
                    Ok(name) => name,
                    Err(_) => {
                        let _ = owner.close();
                        return Err(invalid_directory_data());
                    }
                };
                names.push(name);
            }
            UnixDirectoryRead::Null(0) => {
                let entries = match PathDirectoryEntries::new(names) {
                    Ok(entries) => entries,
                    Err(_) => {
                        let _ = owner.close();
                        return Err(invalid_directory_data());
                    }
                };
                return match owner.close() {
                    Ok(()) | Err(nix::libc::EINTR) => Ok(entries),
                    Err(raw) => Err(classify_directory_error(raw)),
                };
            }
            UnixDirectoryRead::Null(raw) if raw == nix::libc::EINTR || raw == nix::libc::EIO => {
                continue;
            }
            UnixDirectoryRead::Null(raw) => {
                let failure = classify_directory_error(raw);
                let _ = owner.close();
                return Err(failure);
            }
        }
    }
}

#[cfg(unix)]
fn classify_directory_error(raw: i32) -> PrimaryFailure {
    let error = std::io::Error::from_raw_os_error(raw);
    let observed = raw_path_io_error(raw);
    if is_missing_error(&error) {
        PrimaryFailure::Refine(observed)
    } else {
        PrimaryFailure::Final(observed)
    }
}

#[cfg(unix)]
fn invalid_directory_data() -> PrimaryFailure {
    PrimaryFailure::Final(PathObservationError::Io {
        kind: slug_workspace_v2::PathIoErrorKind::InvalidData,
        raw_os_error: None,
    })
}

#[cfg(any(test, windows))]
mod windows_pure {
    use super::*;

    pub(super) const ERROR_INVALID_FUNCTION: u32 = 1;
    pub(super) const ERROR_FILE_NOT_FOUND: u32 = 2;
    pub(super) const ERROR_PATH_NOT_FOUND: u32 = 3;
    pub(super) const ERROR_ACCESS_DENIED: u32 = 5;
    pub(super) const ERROR_NO_MORE_FILES: u32 = 18;
    pub(super) const ERROR_SHARING_VIOLATION: u32 = 32;
    pub(super) const ERROR_NOT_A_REPARSE_POINT: u32 = 4390;

    pub(super) const FILE_ATTRIBUTE_READONLY: u32 = 1;
    pub(super) const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    pub(super) const FILE_ATTRIBUTE_DEVICE: u32 = 0x40;
    pub(super) const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    pub(super) const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
    pub(super) const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;
    pub(super) const IO_REPARSE_TAG_LX_SYMLINK: u32 = 0xA000_001D;
    pub(super) const IO_REPARSE_TAG_PROJFS: u32 = 0x9000_001C;

    const WINDOWS_EPOCH_100NS: i64 = 116_444_736_000_000_000;
    const WINDOWS_EPOCH_MICROS: i64 = 11_644_473_600_000_000;

    pub(super) fn node_kind(attributes: u32) -> PathNodeKind {
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            PathNodeKind::Symlink
        } else if attributes & FILE_ATTRIBUTE_DEVICE != 0 {
            PathNodeKind::SpecialFile
        } else if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            PathNodeKind::Directory
        } else {
            PathNodeKind::RegularFile
        }
    }

    pub(super) fn permissions(attributes: u32) -> i32 {
        if attributes & FILE_ATTRIBUTE_READONLY != 0 {
            0o555
        } else {
            0o755
        }
    }

    pub(super) fn metadata_time_millis(filetime: u64) -> i64 {
        let filetime = filetime as i64;
        if let Some(nanos) = filetime
            .checked_sub(WINDOWS_EPOCH_100NS)
            .and_then(|adjusted| adjusted.checked_mul(100))
        {
            nanos / 1_000_000
        } else {
            (filetime / 10 - WINDOWS_EPOCH_MICROS) / 1_000
        }
    }

    pub(super) fn native_time_millis(filetime: i64) -> i64 {
        filetime
            .wrapping_sub(WINDOWS_EPOCH_100NS)
            .wrapping_div(10_000)
    }

    fn io_error(
        kind: slug_workspace_v2::PathIoErrorKind,
        raw: Option<i32>,
    ) -> PathObservationError {
        PathObservationError::Io {
            kind,
            raw_os_error: raw,
        }
    }

    pub(super) fn raw_io_error(raw: u32) -> PathObservationError {
        use slug_workspace_v2::PathIoErrorKind;
        let kind = match raw {
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => PathIoErrorKind::NotFound,
            ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION => PathIoErrorKind::PermissionDenied,
            267 => PathIoErrorKind::NotADirectory,
            _ => PathIoErrorKind::Other,
        };
        io_error(kind, Some(raw as i32))
    }

    fn invalid_input() -> PrimaryFailure {
        PrimaryFailure::Final(io_error(
            slug_workspace_v2::PathIoErrorKind::InvalidInput,
            None,
        ))
    }

    fn invalid_data() -> PrimaryFailure {
        PrimaryFailure::Final(io_error(
            slug_workspace_v2::PathIoErrorKind::InvalidData,
            None,
        ))
    }

    fn unsupported() -> PrimaryFailure {
        PrimaryFailure::Final(io_error(
            slug_workspace_v2::PathIoErrorKind::Unsupported,
            None,
        ))
    }

    pub(super) fn long_path(path: &[u16]) -> Result<Vec<u16>, PrimaryFailure> {
        if path.contains(&0) {
            return Err(invalid_input());
        }
        let verbatim = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
        let already_verbatim = path.starts_with(&verbatim);
        let mut path = path
            .iter()
            .map(|unit| {
                if *unit == b'/' as u16 {
                    b'\\' as u16
                } else {
                    *unit
                }
            })
            .collect::<Vec<_>>();
        if !already_verbatim {
            let mut prefixed = Vec::with_capacity(verbatim.len() + path.len() + 1);
            prefixed.extend(verbatim);
            prefixed.extend(path);
            path = prefixed;
        }
        path.push(0);
        Ok(path)
    }

    pub(super) fn find_name(raw: &[u16; 260]) -> Result<Vec<u16>, PrimaryFailure> {
        let end = raw
            .iter()
            .position(|unit| *unit == 0)
            .ok_or_else(invalid_data)?;
        Ok(raw[..end].to_vec())
    }

    fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, PrimaryFailure> {
        let pair = bytes.get(offset..offset + 2).ok_or_else(invalid_data)?;
        Ok(u16::from_le_bytes([pair[0], pair[1]]))
    }

    fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, PrimaryFailure> {
        let word = bytes.get(offset..offset + 4).ok_or_else(invalid_data)?;
        Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    }

    pub(super) fn reparse_target(
        storage: &[u8],
        returned: usize,
    ) -> Result<Vec<u16>, PrimaryFailure> {
        if returned > storage.len() {
            return Err(invalid_data());
        }
        let bytes = storage.get(..returned).ok_or_else(invalid_data)?;
        if bytes.len() < 8 {
            return Err(invalid_data());
        }
        let tag = u32_at(bytes, 0)?;
        let declared = 8usize
            .checked_add(u16_at(bytes, 4)? as usize)
            .ok_or_else(invalid_data)?;
        if declared > bytes.len() {
            return Err(invalid_data());
        }
        if tag == IO_REPARSE_TAG_PROJFS {
            return Err(PrimaryFailure::Refine(PathObservationError::NotALink));
        }
        let (base, fixed) = match tag {
            IO_REPARSE_TAG_SYMLINK | IO_REPARSE_TAG_LX_SYMLINK => (20usize, 12usize),
            IO_REPARSE_TAG_MOUNT_POINT => (16usize, 8usize),
            _ => return Err(unsupported()),
        };
        if declared < base || declared < 8 + fixed {
            return Err(invalid_data());
        }
        let substitute_offset = u16_at(bytes, 8)? as usize;
        let substitute_length = u16_at(bytes, 10)? as usize;
        if substitute_offset % 2 != 0 || substitute_length % 2 != 0 {
            return Err(invalid_data());
        }
        let start = base
            .checked_add(substitute_offset)
            .ok_or_else(invalid_data)?;
        let end = start
            .checked_add(substitute_length)
            .ok_or_else(invalid_data)?;
        let target = bytes
            .get(start..end)
            .filter(|_| end <= declared)
            .ok_or_else(invalid_data)?;
        let mut target = target
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let verbatim = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
        let nt = [b'\\' as u16, b'?' as u16, b'?' as u16, b'\\' as u16];
        if target.starts_with(&verbatim) || target.starts_with(&nt) {
            target.drain(..4);
        }
        for unit in &mut target {
            if *unit == b'\\' as u16 {
                *unit = b'/' as u16;
            }
        }
        Ok(target)
    }

    pub(super) trait WindowsFileApi {
        type Handle: Copy;

        fn open(&mut self, path: &[u16]) -> Result<Self::Handle, u32>;
        fn query_change_time(&mut self, handle: Self::Handle) -> Result<i64, u32>;
        fn query_reparse(&mut self, handle: Self::Handle, buffer: &mut [u8]) -> Result<usize, u32>;
        fn close(&mut self, handle: Self::Handle) -> Result<(), u32>;
    }

    #[repr(align(8))]
    struct ReparseBuffer([u8; 16 * 1024]);

    struct FileOwner<'api, A: WindowsFileApi> {
        handle: Option<A::Handle>,
        api: &'api mut A,
    }

    impl<'api, A: WindowsFileApi> FileOwner<'api, A> {
        fn new(handle: A::Handle, api: &'api mut A) -> Self {
            Self {
                handle: Some(handle),
                api,
            }
        }

        fn query_change_time(&mut self) -> Result<i64, u32> {
            self.api
                .query_change_time(self.handle.expect("file owner must be armed"))
        }

        fn query_reparse(&mut self, buffer: &mut [u8]) -> Result<usize, u32> {
            self.api
                .query_reparse(self.handle.expect("file owner must be armed"), buffer)
        }

        fn close(mut self) -> Result<(), u32> {
            let handle = self.handle.take().expect("file owner must be armed");
            self.api.close(handle)
        }
    }

    impl<A: WindowsFileApi> Drop for FileOwner<'_, A> {
        fn drop(&mut self) {
            if let Some(handle) = self.handle.take() {
                let _ = self.api.close(handle);
            }
        }
    }

    pub(super) fn change_time_with<A: WindowsFileApi>(
        path: &[u16],
        api: &mut A,
    ) -> Result<i64, u32> {
        let handle = api.open(path)?;
        let mut owner = FileOwner::new(handle, api);
        let result = owner.query_change_time();
        match result {
            Err(raw) => {
                drop(owner);
                Err(raw)
            }
            Ok(value) => {
                let _ = owner.close();
                Ok(value)
            }
        }
    }

    pub(super) fn read_link_with<A: WindowsFileApi>(
        path: &[u16],
        api: &mut A,
    ) -> Result<Vec<u16>, PrimaryFailure> {
        let handle = api
            .open(path)
            .map_err(|raw| classify_read_link_error(raw))?;
        let mut owner = FileOwner::new(handle, api);
        let mut buffer = ReparseBuffer([0u8; 16 * 1024]);
        let result = owner.query_reparse(&mut buffer.0);
        let returned = match result {
            Ok(returned) => returned,
            Err(raw) => {
                let failure = classify_read_link_error(raw);
                drop(owner);
                return Err(failure);
            }
        };
        let parsed = reparse_target(&buffer.0, returned);
        let _ = owner.close();
        parsed
    }

    fn classify_read_link_error(raw: u32) -> PrimaryFailure {
        if raw == ERROR_NOT_A_REPARSE_POINT || raw == ERROR_INVALID_FUNCTION {
            PrimaryFailure::Refine(PathObservationError::NotALink)
        } else if matches!(raw, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | 267) {
            PrimaryFailure::Refine(raw_io_error(raw))
        } else {
            PrimaryFailure::Final(raw_io_error(raw))
        }
    }

    pub(super) fn classify_file_bytes_error(
        kind: std::io::ErrorKind,
        raw: Option<u32>,
    ) -> PrimaryFailure {
        let observed = raw.map(raw_io_error).unwrap_or(PathObservationError::Io {
            kind: slug_workspace_v2::PathIoErrorKind::from(kind),
            raw_os_error: None,
        });
        if raw == Some(ERROR_ACCESS_DENIED)
            || matches!(
                kind,
                std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::NotADirectory
                    | std::io::ErrorKind::IsADirectory
            )
        {
            PrimaryFailure::Refine(observed)
        } else {
            PrimaryFailure::Final(observed)
        }
    }

    pub(super) fn retry_interrupted<T>(
        mut operation: impl FnMut() -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        loop {
            match operation() {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(super) enum FindRead {
        Name([u16; 260]),
        End,
    }

    pub(super) trait WindowsFindApi {
        type Handle: Copy;

        fn first(&mut self, path: &[u16]) -> Result<(Self::Handle, [u16; 260]), u32>;
        fn next(&mut self, handle: Self::Handle) -> Result<FindRead, u32>;
        fn close(&mut self, handle: Self::Handle) -> Result<(), u32>;
    }

    struct FindOwner<'api, A: WindowsFindApi> {
        handle: Option<A::Handle>,
        api: &'api mut A,
    }

    impl<'api, A: WindowsFindApi> FindOwner<'api, A> {
        fn new(handle: A::Handle, api: &'api mut A) -> Self {
            Self {
                handle: Some(handle),
                api,
            }
        }

        fn next(&mut self) -> Result<FindRead, u32> {
            self.api
                .next(self.handle.expect("find owner must be armed"))
        }

        fn close(mut self) -> Result<(), u32> {
            let handle = self.handle.take().expect("find owner must be armed");
            self.api.close(handle)
        }
    }

    impl<A: WindowsFindApi> Drop for FindOwner<'_, A> {
        fn drop(&mut self) {
            if let Some(handle) = self.handle.take() {
                let _ = self.api.close(handle);
            }
        }
    }

    fn classify_find_error(raw: u32) -> PrimaryFailure {
        PrimaryFailure::Refine(raw_io_error(raw))
    }

    pub(super) fn directory_names_with<A: WindowsFindApi>(
        path: &[u16],
        api: &mut A,
    ) -> Result<Vec<Vec<u16>>, PrimaryFailure> {
        let (handle, first) = match api.first(path) {
            Ok(value) => value,
            Err(ERROR_FILE_NOT_FOUND) => return Ok(Vec::new()),
            Err(raw) => return Err(classify_find_error(raw)),
        };
        let mut owner = FindOwner::new(handle, api);
        let mut names = Vec::new();
        let mut record = Some(first);
        loop {
            match record.take() {
                Some(raw) => {
                    let name = match find_name(&raw) {
                        Ok(name) => name,
                        Err(failure) => {
                            drop(owner);
                            return Err(failure);
                        }
                    };
                    if name != [b'.' as u16] && name != [b'.' as u16, b'.' as u16] {
                        names.push(name);
                    }
                }
                None => match owner.next() {
                    Ok(FindRead::Name(raw)) => {
                        record = Some(raw);
                        continue;
                    }
                    Ok(FindRead::End) | Err(ERROR_NO_MORE_FILES) => break,
                    Err(raw) => {
                        let failure = classify_find_error(raw);
                        drop(owner);
                        return Err(failure);
                    }
                },
            }
            record = None;
        }
        let _ = owner.close();
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid_data());
        }
        Ok(names)
    }
}

#[cfg(windows)]
#[allow(non_camel_case_types, non_snake_case, clippy::upper_case_acronyms)]
mod windows_native {
    use std::ffi::OsString;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::fs::MetadataExt;
    use std::ptr;

    use slug_workspace_v2::PathDirectoryName;

    use super::windows_pure::*;
    use super::*;

    type BOOL = i32;
    type HANDLE = *mut c_void;

    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
    const FILE_SHARE_ALL: u32 = 7;
    const OPEN_EXISTING: u32 = 3;
    const OPEN_REPARSE_FLAGS: u32 = 0x0220_0000;
    const FILE_BASIC_INFO_CLASS: i32 = 0;
    const FSCTL_GET_REPARSE_POINT: u32 = 0x0009_00A8;
    const FIND_INFO_BASIC: i32 = 1;
    const FIND_SEARCH_NAME_MATCH: i32 = 0;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FILETIME {
        dwLowDateTime: u32,
        dwHighDateTime: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FILE_BASIC_INFO {
        CreationTime: i64,
        LastAccessTime: i64,
        LastWriteTime: i64,
        ChangeTime: i64,
        FileAttributes: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct WIN32_FIND_DATAW {
        dwFileAttributes: u32,
        ftCreationTime: FILETIME,
        ftLastAccessTime: FILETIME,
        ftLastWriteTime: FILETIME,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        dwReserved0: u32,
        dwReserved1: u32,
        cFileName: [u16; 260],
        cAlternateFileName: [u16; 14],
    }

    impl Default for WIN32_FIND_DATAW {
        fn default() -> Self {
            Self {
                dwFileAttributes: 0,
                ftCreationTime: FILETIME::default(),
                ftLastAccessTime: FILETIME::default(),
                ftLastWriteTime: FILETIME::default(),
                nFileSizeHigh: 0,
                nFileSizeLow: 0,
                dwReserved0: 0,
                dwReserved1: 0,
                cFileName: [0; 260],
                cAlternateFileName: [0; 14],
            }
        }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *const c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: HANDLE,
        ) -> HANDLE;
        fn CloseHandle(object: HANDLE) -> BOOL;
        fn GetLastError() -> u32;
        fn GetFileInformationByHandleEx(
            file: HANDLE,
            information_class: i32,
            information: *mut c_void,
            buffer_size: u32,
        ) -> BOOL;
        fn DeviceIoControl(
            device: HANDLE,
            control_code: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            bytes_returned: *mut u32,
            overlapped: *mut c_void,
        ) -> BOOL;
        fn FindFirstFileExW(
            file_name: *const u16,
            info_level: i32,
            find_data: *mut c_void,
            search_operation: i32,
            search_filter: *const c_void,
            additional_flags: u32,
        ) -> HANDLE;
        fn FindNextFileW(find_file: HANDLE, find_data: *mut WIN32_FIND_DATAW) -> BOOL;
        fn FindClose(find_file: HANDLE) -> BOOL;
    }

    struct KernelFileApi;

    impl WindowsFileApi for KernelFileApi {
        type Handle = HANDLE;

        fn open(&mut self, path: &[u16]) -> Result<Self::Handle, u32> {
            // SAFETY: `long_path` supplies a live NUL-terminated buffer. The
            // returned raw handle is immediately installed in `FileOwner`.
            let handle = unsafe {
                CreateFileW(
                    path.as_ptr(),
                    0,
                    FILE_SHARE_ALL,
                    ptr::null(),
                    OPEN_EXISTING,
                    OPEN_REPARSE_FLAGS,
                    ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok(handle)
            }
        }

        fn query_change_time(&mut self, handle: Self::Handle) -> Result<i64, u32> {
            let mut info = FILE_BASIC_INFO::default();
            // SAFETY: the owner keeps `handle` live and `info` is a correctly
            // sized writable copy of FILE_BASIC_INFO.
            let result = unsafe {
                GetFileInformationByHandleEx(
                    handle,
                    FILE_BASIC_INFO_CLASS,
                    (&mut info as *mut FILE_BASIC_INFO).cast(),
                    std::mem::size_of::<FILE_BASIC_INFO>() as u32,
                )
            };
            if result == 0 {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok(info.ChangeTime)
            }
        }

        fn query_reparse(&mut self, handle: Self::Handle, buffer: &mut [u8]) -> Result<usize, u32> {
            let mut returned = 0u32;
            // SAFETY: the owner keeps `handle` live; the output slice is
            // writable for its stated length and no overlapped I/O is used.
            let result = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_GET_REPARSE_POINT,
                    ptr::null(),
                    0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut returned,
                    ptr::null_mut(),
                )
            };
            if result == 0 {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok(returned as usize)
            }
        }

        fn close(&mut self, handle: Self::Handle) -> Result<(), u32> {
            // SAFETY: owners disarm before making their sole explicit close.
            if unsafe { CloseHandle(handle) } == 0 {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok(())
            }
        }
    }

    struct KernelFindApi;

    impl WindowsFindApi for KernelFindApi {
        type Handle = HANDLE;

        fn first(&mut self, path: &[u16]) -> Result<(Self::Handle, [u16; 260]), u32> {
            let mut data = WIN32_FIND_DATAW::default();
            // SAFETY: `path` is NUL-terminated and `data` is a correctly sized
            // writable WIN32_FIND_DATAW copy.
            let handle = unsafe {
                FindFirstFileExW(
                    path.as_ptr(),
                    FIND_INFO_BASIC,
                    (&mut data as *mut WIN32_FIND_DATAW).cast(),
                    FIND_SEARCH_NAME_MATCH,
                    ptr::null(),
                    0,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok((handle, data.cFileName))
            }
        }

        fn next(&mut self, handle: Self::Handle) -> Result<FindRead, u32> {
            let mut data = WIN32_FIND_DATAW::default();
            // SAFETY: the owner keeps the find handle live and `data` is a
            // correctly sized writable WIN32_FIND_DATAW copy.
            if unsafe { FindNextFileW(handle, &mut data) } != 0 {
                Ok(FindRead::Name(data.cFileName))
            } else {
                // SAFETY: this is read immediately after the failing call.
                let raw = unsafe { GetLastError() };
                if raw == ERROR_NO_MORE_FILES {
                    Ok(FindRead::End)
                } else {
                    Err(raw)
                }
            }
        }

        fn close(&mut self, handle: Self::Handle) -> Result<(), u32> {
            // SAFETY: owners disarm before making their sole explicit close.
            if unsafe { FindClose(handle) } == 0 {
                // SAFETY: this is read immediately after the failing call.
                Err(unsafe { GetLastError() })
            } else {
                Ok(())
            }
        }
    }

    pub(super) fn observe_windows(
        retained: &RetainedMaterializationRoots<'_>,
        demands: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Result<PathObservationEpoch, PathObservationKernelError> {
        observe_with(retained, demands, &mut WindowsPathObservationAdapter)
    }

    struct WindowsPathObservationAdapter;

    impl ObservationOperations for WindowsPathObservationAdapter {
        fn supports_lstat(&mut self) -> bool {
            true
        }

        fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat> {
            let wide = path.as_path().as_os_str().encode_wide().collect::<Vec<_>>();
            let path_for_query = match long_path(&wide) {
                Ok(path) => path,
                Err(PrimaryFailure::Final(error) | PrimaryFailure::Refine(error)) => {
                    return PathOperationResult::Error(error);
                }
            };
            let metadata = loop {
                match std::fs::symlink_metadata(path.as_path()) {
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return PathOperationResult::Missing,
                    Ok(metadata) => break metadata,
                }
            };
            let attributes = metadata.file_attributes();
            let size = metadata.file_size() as i64;
            let mtime = metadata_time_millis(metadata.last_write_time());
            let kind = node_kind(attributes);
            let permissions = permissions(attributes);
            match change_time_with(&path_for_query, &mut KernelFileApi) {
                Ok(change_time) => PathOperationResult::Present(PathLstat::new(
                    kind,
                    size,
                    mtime,
                    native_time_millis(change_time),
                    -1,
                    permissions,
                )),
                Err(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) => PathOperationResult::Missing,
                Err(raw) => PathOperationResult::Error(raw_io_error(raw)),
            }
        }

        fn read_link(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<PathBuf>, PrimaryFailure> {
            let wide = path.as_path().as_os_str().encode_wide().collect::<Vec<_>>();
            let path = long_path(&wide)?;
            let target = read_link_with(&path, &mut KernelFileApi)?;
            Ok(Arc::new(PathBuf::from(OsString::from_wide(&target))))
        }

        fn file_bytes(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<[u8]>, PrimaryFailure> {
            let result = retry_interrupted(|| std::fs::read(path.as_path()));
            match result {
                Ok(bytes) => Ok(Arc::from(bytes)),
                Err(error) => {
                    let raw = error.raw_os_error().map(|raw| raw as u32);
                    Err(classify_file_bytes_error(error.kind(), raw))
                }
            }
        }

        fn directory_entries(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<PathDirectoryEntries, PrimaryFailure> {
            let wide = path.as_path().as_os_str().encode_wide().collect::<Vec<_>>();
            let mut query = long_path(&wide)?;
            query.pop();
            if !query.ends_with(&[b'\\' as u16]) {
                query.push(b'\\' as u16);
            }
            query.push(b'*' as u16);
            query.push(0);
            let raw_names = directory_names_with(&query, &mut KernelFindApi)?;
            let names = raw_names
                .into_iter()
                .map(|name| PathDirectoryName::new(OsString::from_wide(&name)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    PrimaryFailure::Final(PathObservationError::Io {
                        kind: slug_workspace_v2::PathIoErrorKind::InvalidData,
                        raw_os_error: None,
                    })
                })?;
            PathDirectoryEntries::new(names).map_err(|_| {
                PrimaryFailure::Final(PathObservationError::Io {
                    kind: slug_workspace_v2::PathIoErrorKind::InvalidData,
                    raw_os_error: None,
                })
            })
        }
    }

    #[cfg(test)]
    mod abi_tests {
        use super::*;

        #[test]
        fn copied_windows_sys_0_61_2_layouts_match_x64() {
            assert_eq!(std::mem::size_of::<HANDLE>(), 8);
            assert_eq!(std::mem::size_of::<BOOL>(), 4);
            assert_eq!(std::mem::size_of::<FILETIME>(), 8);
            assert_eq!(std::mem::align_of::<FILETIME>(), 4);
            assert_eq!(std::mem::size_of::<FILE_BASIC_INFO>(), 40);
            assert_eq!(std::mem::align_of::<FILE_BASIC_INFO>(), 8);
            assert_eq!(std::mem::size_of::<WIN32_FIND_DATAW>(), 592);
            assert_eq!(std::mem::align_of::<WIN32_FIND_DATAW>(), 4);
        }
    }
}

#[cfg(test)]
mod windows_tests {
    use std::collections::VecDeque;

    use slug_workspace_v2::PathIoErrorKind;

    use super::windows_pure::*;
    use super::*;

    fn io(kind: PathIoErrorKind, raw_os_error: Option<i32>) -> PathObservationError {
        PathObservationError::Io { kind, raw_os_error }
    }

    fn name(value: &[u16]) -> [u16; 260] {
        assert!(value.len() < 260);
        let mut raw = [0u16; 260];
        raw[..value.len()].copy_from_slice(value);
        raw
    }

    fn reparse(tag: u32, target: &[u16], substitute_offset: u16) -> Vec<u8> {
        let base = if tag == IO_REPARSE_TAG_MOUNT_POINT {
            16
        } else {
            20
        };
        let fixed = base - 8;
        let target_bytes = target.len() * 2;
        let mut bytes = vec![0u8; base + substitute_offset as usize + target_bytes];
        bytes[0..4].copy_from_slice(&tag.to_le_bytes());
        let data_length = (fixed + substitute_offset as usize + target_bytes) as u16;
        bytes[4..6].copy_from_slice(&data_length.to_le_bytes());
        bytes[8..10].copy_from_slice(&substitute_offset.to_le_bytes());
        bytes[10..12].copy_from_slice(&(target_bytes as u16).to_le_bytes());
        let start = base + substitute_offset as usize;
        for (index, unit) in target.iter().enumerate() {
            bytes[start + index * 2..start + index * 2 + 2].copy_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn windows_attributes_permissions_errors_and_time_formulas_are_exact() {
        for mask in 0..8 {
            let attributes = (if mask & 1 != 0 {
                FILE_ATTRIBUTE_REPARSE_POINT
            } else {
                0
            }) | (if mask & 2 != 0 {
                FILE_ATTRIBUTE_DEVICE
            } else {
                0
            }) | (if mask & 4 != 0 {
                FILE_ATTRIBUTE_DIRECTORY
            } else {
                0
            });
            let expected = if mask & 1 != 0 {
                PathNodeKind::Symlink
            } else if mask & 2 != 0 {
                PathNodeKind::SpecialFile
            } else if mask & 4 != 0 {
                PathNodeKind::Directory
            } else {
                PathNodeKind::RegularFile
            };
            assert_eq!(node_kind(attributes), expected, "attribute mask {mask}");
        }
        assert_eq!(permissions(0), 0o755);
        assert_eq!(permissions(FILE_ATTRIBUTE_READONLY), 0o555);

        assert_eq!(
            raw_io_error(ERROR_FILE_NOT_FOUND),
            io(PathIoErrorKind::NotFound, Some(2))
        );
        assert_eq!(
            raw_io_error(ERROR_PATH_NOT_FOUND),
            io(PathIoErrorKind::NotFound, Some(3))
        );
        assert_eq!(
            raw_io_error(ERROR_ACCESS_DENIED),
            io(PathIoErrorKind::PermissionDenied, Some(5))
        );
        assert_eq!(
            raw_io_error(ERROR_SHARING_VIOLATION),
            io(PathIoErrorKind::PermissionDenied, Some(32))
        );
        assert_eq!(
            raw_io_error(267),
            io(PathIoErrorKind::NotADirectory, Some(267))
        );
        assert_eq!(raw_io_error(999), io(PathIoErrorKind::Other, Some(999)));

        assert_eq!(metadata_time_millis(116_444_736_000_000_000), 0);
        assert_eq!(metadata_time_millis(116_444_736_012_349_999), 1_234);
        assert_eq!(metadata_time_millis(0), -11_644_473_600_000);
        assert_eq!(metadata_time_millis(u64::MAX), -11_644_473_600_000);
        assert_eq!(
            metadata_time_millis(i64::MAX as u64),
            (i64::MAX / 10 - 11_644_473_600_000_000) / 1_000
        );
        assert_eq!(
            metadata_time_millis(i64::MIN as u64),
            (i64::MIN / 10 - 11_644_473_600_000_000) / 1_000
        );
        let maximum_fast_adjusted = i64::MAX / 100;
        let maximum_fast_time = 116_444_736_000_000_000 + maximum_fast_adjusted;
        assert_eq!(
            metadata_time_millis(maximum_fast_time as u64),
            maximum_fast_adjusted * 100 / 1_000_000
        );
        let maximum_fallback_time = maximum_fast_time + 1;
        assert_eq!(
            metadata_time_millis(maximum_fallback_time as u64),
            (maximum_fallback_time / 10 - 11_644_473_600_000_000) / 1_000
        );
        let minimum_fast_adjusted = i64::MIN / 100;
        let minimum_fast_time = 116_444_736_000_000_000 + minimum_fast_adjusted;
        assert_eq!(
            metadata_time_millis(minimum_fast_time as u64),
            minimum_fast_adjusted * 100 / 1_000_000
        );
        let minimum_fallback_time = minimum_fast_time - 1;
        assert_eq!(
            metadata_time_millis(minimum_fallback_time as u64),
            (minimum_fallback_time / 10 - 11_644_473_600_000_000) / 1_000
        );
        assert_eq!(native_time_millis(116_444_736_000_000_000), 0);
        assert_eq!(
            native_time_millis(i64::MIN),
            i64::MIN
                .wrapping_sub(116_444_736_000_000_000)
                .wrapping_div(10_000)
        );
        assert_eq!(
            native_time_millis(i64::MAX),
            i64::MAX
                .wrapping_sub(116_444_736_000_000_000)
                .wrapping_div(10_000)
        );
        assert_eq!(
            classify_file_bytes_error(std::io::ErrorKind::PermissionDenied, Some(5)),
            PrimaryFailure::Refine(raw_io_error(5))
        );
        assert_eq!(
            classify_file_bytes_error(std::io::ErrorKind::NotFound, Some(3)),
            PrimaryFailure::Refine(raw_io_error(3))
        );
        assert_eq!(
            classify_file_bytes_error(std::io::ErrorKind::IsADirectory, None),
            PrimaryFailure::Refine(io(PathIoErrorKind::IsADirectory, None))
        );
        assert_eq!(
            classify_file_bytes_error(std::io::ErrorKind::NotADirectory, Some(267)),
            PrimaryFailure::Refine(raw_io_error(267))
        );
        assert_eq!(
            classify_file_bytes_error(std::io::ErrorKind::InvalidData, None),
            PrimaryFailure::Final(io(PathIoErrorKind::InvalidData, None))
        );
        let mut attempts = 0;
        assert_eq!(
            super::windows_pure::retry_interrupted(|| {
                attempts += 1;
                if attempts < 3 {
                    Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
                } else {
                    Ok(7)
                }
            })
            .unwrap(),
            7
        );
        assert_eq!(attempts, 3);
    }

    #[test]
    fn windows_long_paths_and_raw_find_names_preserve_utf16() {
        assert_eq!(
            long_path(&"C:/a/b".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\C:\\a\\b\0".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            long_path(&"\\\\?\\C:/a".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\C:\\a\0".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            long_path(&"//?/C:/a".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\\\\\?\\C:\\a\0".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            long_path(&"\\\\server/share".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\\\\\server\\share\0"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            long_path(&"\\??\\C:/a".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\\\??\\C:\\a\0".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            long_path(&"C:/mixed\\path".encode_utf16().collect::<Vec<_>>()).unwrap(),
            "\\\\?\\C:\\mixed\\path\0"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        let lone = [b'C' as u16, b':' as u16, b'\\' as u16, 0xD800];
        let converted = long_path(&lone).unwrap();
        assert_eq!(&converted[4..converted.len() - 1], &lone);
        assert_eq!(
            long_path(&[b'C' as u16, 0]),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidInput,
                None
            )))
        );

        assert_eq!(
            find_name(&name(&[0xD800, b'x' as u16])).unwrap(),
            [0xD800, b'x' as u16]
        );
        assert_eq!(
            find_name(&[b'x' as u16; 260]),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
    }

    #[test]
    fn windows_reparse_parser_accepts_all_link_tags_and_normalizes_once() {
        let target = "\\??\\C:\\dir\\file".encode_utf16().collect::<Vec<_>>();
        for tag in [
            IO_REPARSE_TAG_SYMLINK,
            IO_REPARSE_TAG_LX_SYMLINK,
            IO_REPARSE_TAG_MOUNT_POINT,
        ] {
            let mut bytes = reparse(tag, &target, 0);
            bytes[12..16].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            if tag != IO_REPARSE_TAG_MOUNT_POINT {
                bytes[16..20].copy_from_slice(&0xA5A5_A5A5u32.to_le_bytes());
            }
            bytes.extend([0xAA, 0xBB]);
            assert_eq!(
                reparse_target(&bytes, bytes.len()).unwrap(),
                "C:/dir/file".encode_utf16().collect::<Vec<_>>()
            );
        }

        let twice = "\\\\?\\\\??\\x".encode_utf16().collect::<Vec<_>>();
        let bytes = reparse(IO_REPARSE_TAG_SYMLINK, &twice, 2);
        assert_eq!(
            reparse_target(&bytes, bytes.len()).unwrap(),
            "/??/x".encode_utf16().collect::<Vec<_>>()
        );
        let lone = [b'\\' as u16, b'?' as u16, b'?' as u16, b'\\' as u16, 0xD800];
        let bytes = reparse(IO_REPARSE_TAG_SYMLINK, &lone, 0);
        assert_eq!(reparse_target(&bytes, bytes.len()).unwrap(), [0xD800]);

        let projfs = reparse(IO_REPARSE_TAG_PROJFS, &[], 0);
        assert_eq!(
            reparse_target(&projfs, projfs.len()),
            Err(PrimaryFailure::Refine(PathObservationError::NotALink))
        );
        let unknown = reparse(0xDEAD_BEEF, &[], 0);
        assert_eq!(
            reparse_target(&unknown, unknown.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::Unsupported,
                None
            )))
        );
    }

    #[test]
    fn windows_reparse_parser_rejects_lengths_offsets_and_truncation() {
        let valid = reparse(
            IO_REPARSE_TAG_SYMLINK,
            &"x".encode_utf16().collect::<Vec<_>>(),
            0,
        );
        for returned in 0..8 {
            assert_eq!(
                reparse_target(&valid, returned),
                Err(PrimaryFailure::Final(io(
                    PathIoErrorKind::InvalidData,
                    None
                )))
            );
        }
        assert_eq!(
            reparse_target(&valid, valid.len() + 1),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        let mut exact_capacity = vec![0u8; 16 * 1024];
        exact_capacity[..valid.len()].copy_from_slice(&valid);
        assert_eq!(
            reparse_target(&exact_capacity, exact_capacity.len()).unwrap(),
            [b'x' as u16]
        );

        let mut declared_too_large = valid.clone();
        declared_too_large[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            reparse_target(&declared_too_large, declared_too_large.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );

        let mut declared_too_small = valid.clone();
        declared_too_small[4..6].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            reparse_target(&declared_too_small, declared_too_small.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );

        let odd_offset = reparse(IO_REPARSE_TAG_SYMLINK, &[b'x' as u16], 1);
        assert_eq!(
            reparse_target(&odd_offset, odd_offset.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        let mut odd_length = valid.clone();
        odd_length[10..12].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            reparse_target(&odd_length, odd_length.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        let mut out_of_range = valid.clone();
        out_of_range[8..10].copy_from_slice(&100u16.to_le_bytes());
        assert_eq!(
            reparse_target(&out_of_range, out_of_range.len()),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    enum FileCall {
        Open,
        Change(u8),
        Reparse(u8),
        Close(u8),
    }

    struct ScriptedFileApi {
        calls: Vec<FileCall>,
        open: Result<u8, u32>,
        change: Result<i64, u32>,
        reparse: Result<(Vec<u8>, usize), u32>,
        closes: VecDeque<Result<(), u32>>,
    }

    impl ScriptedFileApi {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                open: Ok(7),
                change: Ok(42),
                reparse: Err(999),
                closes: VecDeque::from([Ok(())]),
            }
        }
    }

    impl WindowsFileApi for ScriptedFileApi {
        type Handle = u8;

        fn open(&mut self, _path: &[u16]) -> Result<Self::Handle, u32> {
            self.calls.push(FileCall::Open);
            self.open
        }

        fn query_change_time(&mut self, handle: Self::Handle) -> Result<i64, u32> {
            self.calls.push(FileCall::Change(handle));
            self.change
        }

        fn query_reparse(&mut self, handle: Self::Handle, buffer: &mut [u8]) -> Result<usize, u32> {
            self.calls.push(FileCall::Reparse(handle));
            assert_eq!((buffer.as_ptr() as usize) % 8, 0);
            match &self.reparse {
                Ok((bytes, returned)) => {
                    let copied = bytes.len().min(buffer.len());
                    buffer[..copied].copy_from_slice(&bytes[..copied]);
                    Ok(*returned)
                }
                Err(raw) => Err(*raw),
            }
        }

        fn close(&mut self, handle: Self::Handle) -> Result<(), u32> {
            self.calls.push(FileCall::Close(handle));
            self.closes.pop_front().expect("script must supply close")
        }
    }

    #[test]
    fn windows_file_owner_disarms_and_preserves_query_error_before_close() {
        let mut success = ScriptedFileApi::new();
        assert_eq!(change_time_with(&[0], &mut success), Ok(42));
        assert_eq!(
            success.calls,
            [FileCall::Open, FileCall::Change(7), FileCall::Close(7)]
        );

        let mut open_error = ScriptedFileApi::new();
        open_error.open = Err(5);
        assert_eq!(change_time_with(&[0], &mut open_error), Err(5));
        assert_eq!(open_error.calls, [FileCall::Open]);

        let mut query_error = ScriptedFileApi::new();
        query_error.change = Err(32);
        query_error.closes = VecDeque::from([Err(999)]);
        assert_eq!(change_time_with(&[0], &mut query_error), Err(32));
        assert_eq!(
            query_error.calls,
            [FileCall::Open, FileCall::Change(7), FileCall::Close(7)]
        );

        let mut close_error = ScriptedFileApi::new();
        close_error.closes = VecDeque::from([Err(5)]);
        assert_eq!(change_time_with(&[0], &mut close_error), Ok(42));
        assert_eq!(
            close_error.calls,
            [FileCall::Open, FileCall::Change(7), FileCall::Close(7)]
        );
    }

    #[test]
    fn windows_readlink_owner_covers_open_query_parse_close_and_overflow() {
        let target = "\\\\?\\C:\\x".encode_utf16().collect::<Vec<_>>();
        let bytes = reparse(IO_REPARSE_TAG_SYMLINK, &target, 0);
        let mut success = ScriptedFileApi::new();
        success.reparse = Ok((bytes.clone(), bytes.len()));
        assert_eq!(
            read_link_with(&[0], &mut success).unwrap(),
            "C:/x".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            success.calls,
            [FileCall::Open, FileCall::Reparse(7), FileCall::Close(7)]
        );

        let mut query_error = ScriptedFileApi::new();
        query_error.reparse = Err(ERROR_NOT_A_REPARSE_POINT);
        query_error.closes = VecDeque::from([Err(999)]);
        assert_eq!(
            read_link_with(&[0], &mut query_error),
            Err(PrimaryFailure::Refine(PathObservationError::NotALink))
        );

        let mut invalid_function = ScriptedFileApi::new();
        invalid_function.reparse = Err(ERROR_INVALID_FUNCTION);
        assert_eq!(
            read_link_with(&[0], &mut invalid_function),
            Err(PrimaryFailure::Refine(PathObservationError::NotALink))
        );

        let mut missing = ScriptedFileApi::new();
        missing.open = Err(ERROR_PATH_NOT_FOUND);
        assert_eq!(
            read_link_with(&[0], &mut missing),
            Err(PrimaryFailure::Refine(raw_io_error(ERROR_PATH_NOT_FOUND)))
        );
        assert_eq!(missing.calls, [FileCall::Open]);

        let mut not_directory = ScriptedFileApi::new();
        not_directory.open = Err(267);
        assert_eq!(
            read_link_with(&[0], &mut not_directory),
            Err(PrimaryFailure::Refine(raw_io_error(267)))
        );
        assert_eq!(not_directory.calls, [FileCall::Open]);

        let mut sharing = ScriptedFileApi::new();
        sharing.open = Err(ERROR_SHARING_VIOLATION);
        assert_eq!(
            read_link_with(&[0], &mut sharing),
            Err(PrimaryFailure::Final(raw_io_error(ERROR_SHARING_VIOLATION)))
        );
        assert_eq!(sharing.calls, [FileCall::Open]);

        let mut denied = ScriptedFileApi::new();
        denied.open = Err(ERROR_ACCESS_DENIED);
        assert_eq!(
            read_link_with(&[0], &mut denied),
            Err(PrimaryFailure::Final(raw_io_error(ERROR_ACCESS_DENIED)))
        );
        assert_eq!(denied.calls, [FileCall::Open]);

        let mut overflow = ScriptedFileApi::new();
        overflow.reparse = Ok((Vec::new(), 16 * 1024 + 1));
        assert_eq!(
            read_link_with(&[0], &mut overflow),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        assert_eq!(
            overflow.calls,
            [FileCall::Open, FileCall::Reparse(7), FileCall::Close(7)]
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    enum FindCall {
        First,
        Next(u8),
        Close(u8),
    }

    struct ScriptedFindApi {
        calls: Vec<FindCall>,
        first: Result<(u8, [u16; 260]), u32>,
        next: VecDeque<Result<FindRead, u32>>,
        closes: VecDeque<Result<(), u32>>,
    }

    impl WindowsFindApi for ScriptedFindApi {
        type Handle = u8;

        fn first(&mut self, _path: &[u16]) -> Result<(Self::Handle, [u16; 260]), u32> {
            self.calls.push(FindCall::First);
            self.first
        }

        fn next(&mut self, handle: Self::Handle) -> Result<FindRead, u32> {
            self.calls.push(FindCall::Next(handle));
            self.next.pop_front().expect("script must supply next")
        }

        fn close(&mut self, handle: Self::Handle) -> Result<(), u32> {
            self.calls.push(FindCall::Close(handle));
            self.closes.pop_front().expect("script must supply close")
        }
    }

    fn find_script(first: Result<(u8, [u16; 260]), u32>) -> ScriptedFindApi {
        ScriptedFindApi {
            calls: Vec::new(),
            first,
            next: VecDeque::new(),
            closes: VecDeque::from([Ok(())]),
        }
    }

    #[test]
    fn windows_find_initial_errors_empty_and_refinement_are_exact() {
        let mut empty = find_script(Err(ERROR_FILE_NOT_FOUND));
        assert_eq!(directory_names_with(&[0], &mut empty), Ok(Vec::new()));
        assert_eq!(empty.calls, [FindCall::First]);

        for (raw, expected) in [
            (
                ERROR_PATH_NOT_FOUND,
                PrimaryFailure::Refine(raw_io_error(ERROR_PATH_NOT_FOUND)),
            ),
            (267, PrimaryFailure::Refine(raw_io_error(267))),
            (
                ERROR_SHARING_VIOLATION,
                PrimaryFailure::Refine(raw_io_error(ERROR_SHARING_VIOLATION)),
            ),
            (
                ERROR_ACCESS_DENIED,
                PrimaryFailure::Refine(raw_io_error(ERROR_ACCESS_DENIED)),
            ),
            (999, PrimaryFailure::Refine(raw_io_error(999))),
        ] {
            let mut api = find_script(Err(raw));
            assert_eq!(directory_names_with(&[0], &mut api), Err(expected));
            assert_eq!(api.calls, [FindCall::First]);
        }
    }

    #[test]
    fn windows_find_uses_one_handle_skips_dots_finishes_then_sorts() {
        let mut api = find_script(Ok((9, name(&[b'.' as u16]))));
        api.next = VecDeque::from([
            Ok(FindRead::Name(name(&[b'z' as u16]))),
            Ok(FindRead::Name(name(&[b'.' as u16, b'.' as u16]))),
            Ok(FindRead::Name(name(&[b'a' as u16]))),
            Err(ERROR_NO_MORE_FILES),
        ]);
        assert_eq!(
            directory_names_with(&[0], &mut api).unwrap(),
            [vec![b'a' as u16], vec![b'z' as u16]]
        );
        assert_eq!(
            api.calls,
            [
                FindCall::First,
                FindCall::Next(9),
                FindCall::Next(9),
                FindCall::Next(9),
                FindCall::Next(9),
                FindCall::Close(9)
            ]
        );
    }

    #[test]
    fn windows_find_discards_partial_on_iterator_or_validation_error_and_closes_once() {
        let mut iterator = find_script(Ok((4, name(&[b'a' as u16]))));
        iterator.next = VecDeque::from([Err(ERROR_PATH_NOT_FOUND)]);
        iterator.closes = VecDeque::from([Err(999)]);
        assert_eq!(
            directory_names_with(&[0], &mut iterator),
            Err(PrimaryFailure::Refine(raw_io_error(ERROR_PATH_NOT_FOUND)))
        );
        assert_eq!(
            iterator.calls,
            [FindCall::First, FindCall::Next(4), FindCall::Close(4)]
        );

        let mut access = find_script(Ok((8, name(&[b'a' as u16]))));
        access.next = VecDeque::from([Err(ERROR_ACCESS_DENIED)]);
        access.closes = VecDeque::from([Err(999)]);
        assert_eq!(
            directory_names_with(&[0], &mut access),
            Err(PrimaryFailure::Refine(raw_io_error(ERROR_ACCESS_DENIED)))
        );
        assert_eq!(
            access.calls,
            [FindCall::First, FindCall::Next(8), FindCall::Close(8)]
        );

        let mut invalid = find_script(Ok((5, [b'x' as u16; 260])));
        assert_eq!(
            directory_names_with(&[0], &mut invalid),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        assert_eq!(invalid.calls, [FindCall::First, FindCall::Close(5)]);

        let mut duplicate = find_script(Ok((6, name(&[b'a' as u16]))));
        duplicate.next =
            VecDeque::from([Ok(FindRead::Name(name(&[b'a' as u16]))), Ok(FindRead::End)]);
        assert_eq!(
            directory_names_with(&[0], &mut duplicate),
            Err(PrimaryFailure::Final(io(
                PathIoErrorKind::InvalidData,
                None
            )))
        );
        assert_eq!(
            duplicate.calls,
            [
                FindCall::First,
                FindCall::Next(6),
                FindCall::Next(6),
                FindCall::Close(6)
            ]
        );
    }

    #[test]
    fn windows_find_close_failure_is_cleanup_only_after_complete_enumeration() {
        let mut api = find_script(Ok((3, name(&[b'a' as u16]))));
        api.next = VecDeque::from([Ok(FindRead::End)]);
        api.closes = VecDeque::from([Err(ERROR_ACCESS_DENIED)]);
        assert_eq!(
            directory_names_with(&[0], &mut api),
            Ok(vec![vec![b'a' as u16]])
        );
        assert_eq!(
            api.calls,
            [FindCall::First, FindCall::Next(3), FindCall::Close(3)]
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::Path;

    use slug_workspace_v2::PathDirectoryName;
    use slug_workspace_v2::PathIoErrorKind;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Lstat(NormalizedAbsolutePath),
        ReadLink(NormalizedAbsolutePath),
        FileBytes(NormalizedAbsolutePath),
        DirectoryEntries(NormalizedAbsolutePath),
    }

    struct ScriptedOperations {
        supports_lstat: bool,
        support_queries: usize,
        calls: Vec<Call>,
        lstats: VecDeque<PathOperationResult<PathLstat>>,
        read_links: VecDeque<Result<Arc<PathBuf>, PrimaryFailure>>,
        file_bytes: VecDeque<Result<Arc<[u8]>, PrimaryFailure>>,
        directory_entries: VecDeque<Result<PathDirectoryEntries, PrimaryFailure>>,
    }

    impl ScriptedOperations {
        fn supported() -> Self {
            Self {
                supports_lstat: true,
                support_queries: 0,
                calls: Vec::new(),
                lstats: VecDeque::new(),
                read_links: VecDeque::new(),
                file_bytes: VecDeque::new(),
                directory_entries: VecDeque::new(),
            }
        }

        fn unsupported() -> Self {
            Self {
                supports_lstat: false,
                ..Self::supported()
            }
        }
    }

    impl ObservationOperations for ScriptedOperations {
        fn supports_lstat(&mut self) -> bool {
            self.support_queries += 1;
            self.supports_lstat
        }

        fn lstat(&mut self, path: &NormalizedAbsolutePath) -> PathOperationResult<PathLstat> {
            self.calls.push(Call::Lstat(path.clone()));
            self.lstats
                .pop_front()
                .expect("script must supply an lstat result")
        }

        fn read_link(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<PathBuf>, PrimaryFailure> {
            self.calls.push(Call::ReadLink(path.clone()));
            self.read_links
                .pop_front()
                .expect("script must supply a read-link result")
        }

        fn file_bytes(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<Arc<[u8]>, PrimaryFailure> {
            self.calls.push(Call::FileBytes(path.clone()));
            self.file_bytes
                .pop_front()
                .expect("script must supply a file-bytes result")
        }

        fn directory_entries(
            &mut self,
            path: &NormalizedAbsolutePath,
        ) -> Result<PathDirectoryEntries, PrimaryFailure> {
            self.calls.push(Call::DirectoryEntries(path.clone()));
            self.directory_entries
                .pop_front()
                .expect("script must supply a directory result")
        }
    }

    fn path(root: &Path, suffix: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(root.join(suffix)).unwrap()
    }

    fn demand(
        namespace: PathObservationNamespace,
        path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
    ) -> PathObservationDemand {
        PathObservationDemand::new(namespace, path, operation)
    }

    fn lstat(kind: PathNodeKind) -> PathLstat {
        PathLstat::new(kind, 1, 2, 3, 4, 0o755)
    }

    fn io(kind: PathIoErrorKind, raw_os_error: Option<i32>) -> PathObservationError {
        PathObservationError::Io { kind, raw_os_error }
    }

    fn empty_entries() -> PathDirectoryEntries {
        PathDirectoryEntries::new([]).unwrap()
    }

    fn roots<'a>(owner: &'a (), root: &NormalizedAbsolutePath) -> RetainedMaterializationRoots<'a> {
        RetainedMaterializationRoots::new(
            owner,
            [(PathObservationInstanceId::new(1), root.clone())],
        )
        .unwrap()
    }

    fn assert_allocative<T: Allocative>() {}

    #[test]
    fn retained_roots_are_one_sorted_arc_slice_with_borrowed_lifetime() {
        assert_allocative::<RetainedMaterializationRoot>();
        assert_allocative::<RetainedMaterializationRoots<'_>>();
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let shared = path(temp.path(), "shared");
        let other = path(temp.path(), "other");
        let retained = RetainedMaterializationRoots::new(
            &owner,
            [
                (PathObservationInstanceId::new(3), shared.clone()),
                (PathObservationInstanceId::new(1), shared.clone()),
                (PathObservationInstanceId::new(2), other),
            ],
        )
        .unwrap();
        assert_eq!(Arc::strong_count(&retained.entries), 1);
        assert_eq!(
            retained
                .entries
                .iter()
                .map(|entry| entry.instance.value())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(retained.entries[0].root, retained.entries[2].root);
    }

    #[test]
    fn retained_roots_reject_zero_and_duplicate_instances() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let first = path(temp.path(), "first");
        let second = path(temp.path(), "second");
        assert!(matches!(
            RetainedMaterializationRoots::new(
                &owner,
                [(PathObservationInstanceId::new(0), first.clone())]
            ),
            Err(PathObservationKernelError::ZeroRetainedInstance)
        ));
        assert!(matches!(
            RetainedMaterializationRoots::new(
                &owner,
                [
                    (PathObservationInstanceId::new(4), first),
                    (PathObservationInstanceId::new(4), second)
                ]
            ),
            Err(PathObservationKernelError::DuplicateRetainedInstance(
                PathObservationInstanceId { .. }
            ))
        ));
    }

    #[test]
    fn invalid_batches_preflight_before_support_or_operation_calls() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "root");
        let retained = roots(&owner, &root);
        let host = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "host"),
            PathObservationOperation::Lstat,
        );
        let zero = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(0)),
            path(temp.path(), "zero"),
            PathObservationOperation::ReadLink,
        );
        let unknown = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
            path(temp.path(), "unknown"),
            PathObservationOperation::FileBytes,
        );

        for (demands, expected) in [
            (
                vec![host.clone(), host.clone()],
                PathObservationKernelError::DuplicateDemand(host.clone()),
            ),
            (
                vec![host.clone(), zero.clone()],
                PathObservationKernelError::ZeroDemandInstance(zero),
            ),
            (
                vec![host.clone(), unknown.clone()],
                PathObservationKernelError::UnknownDemandInstance(unknown),
            ),
        ] {
            let mut operations = ScriptedOperations::unsupported();
            assert_eq!(
                observe_with(&retained, demands, &mut operations),
                Err(expected)
            );
            assert_eq!(operations.support_queries, 0);
            assert!(operations.calls.is_empty());
        }

        let mut operations = ScriptedOperations::unsupported();
        assert_eq!(
            observe_with(&retained, [host], &mut operations),
            Err(PathObservationKernelError::UnsupportedLstat)
        );
        assert_eq!(operations.support_queries, 1);
        assert!(operations.calls.is_empty());

        let earlier = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "a-earlier"),
            PathObservationOperation::FileBytes,
        );
        let later_lstat = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "z-later"),
            PathObservationOperation::Lstat,
        );
        let mut operations = ScriptedOperations::unsupported();
        assert_eq!(
            observe_with(&retained, [earlier, later_lstat], &mut operations),
            Err(PathObservationKernelError::UnsupportedLstat)
        );
        assert_eq!(operations.support_queries, 1);
        assert!(operations.calls.is_empty());
    }

    #[test]
    fn empty_and_non_lstat_batches_do_not_query_lstat_support() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let mut operations = ScriptedOperations::unsupported();
        let epoch = observe_with(&retained, [], &mut operations).unwrap();
        assert!(epoch.observations().is_empty());
        assert_eq!(operations.support_queries, 0);

        let read = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "read"),
            PathObservationOperation::ReadLink,
        );
        let unsupported = io(PathIoErrorKind::Unsupported, None);
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Final(unsupported)));
        let epoch = observe_with(&retained, [read.clone()], &mut operations).unwrap();
        assert_eq!(operations.support_queries, 0);
        assert!(matches!(
            epoch.get(&read).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Error(error))
                if *error == unsupported
        ));
    }

    #[test]
    fn shuffled_demands_execute_in_exact_ord_order_and_errors_continue() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "root");
        let retained = roots(&owner, &root);
        let a = path(temp.path(), "a");
        let b = path(temp.path(), "b");
        let c = path(temp.path(), "c");
        let demands = vec![
            demand(
                PathObservationNamespace::Host,
                c.clone(),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                a.clone(),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                b.clone(),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut sorted = demands.clone();
        sorted.sort_unstable();
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Final(io(
                PathIoErrorKind::PermissionDenied,
                Some(13),
            ))));
        operations.directory_entries.push_back(Ok(empty_entries()));
        operations
            .file_bytes
            .push_back(Ok(Arc::from(&b"bytes"[..])));
        let epoch = observe_with(&retained, demands, &mut operations).unwrap();
        assert_eq!(epoch.observations().len(), 3);
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(a),
                Call::DirectoryEntries(b),
                Call::FileBytes(c)
            ]
        );
        assert_eq!(
            epoch.observations().keys().cloned().collect::<Vec<_>>(),
            sorted
        );
    }

    #[test]
    fn exact_path_is_not_rewritten_and_namespaces_remain_distinct() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let root = path(temp.path(), "retained-root");
        let retained = roots(&owner, &root);
        let escaped = path(temp.path(), "outside/escaped");
        let host = demand(
            PathObservationNamespace::Host,
            escaped.clone(),
            PathObservationOperation::FileBytes,
        );
        let materialized = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            escaped.clone(),
            PathObservationOperation::FileBytes,
        );
        let mut operations = ScriptedOperations::supported();
        operations.file_bytes.extend([
            Ok(Arc::from(&b"host"[..])),
            Ok(Arc::from(&b"materialized"[..])),
        ]);
        let epoch = observe_with(
            &retained,
            [materialized.clone(), host.clone()],
            &mut operations,
        )
        .unwrap();
        assert_eq!(
            operations.calls,
            vec![Call::FileBytes(escaped.clone()), Call::FileBytes(escaped)]
        );
        assert!(epoch.get(&host).is_some());
        assert!(epoch.get(&materialized).is_some());
        assert_ne!(host, materialized);
    }

    #[test]
    fn lstat_and_present_primary_results_match_epoch_variants_without_aux() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let base = path(temp.path(), "base");
        let demands = [
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "lstat"),
                PathObservationOperation::Lstat,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "link"),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "bytes"),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                path(temp.path(), "dir"),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut operations = ScriptedOperations::supported();
        operations
            .lstats
            .push_back(PathOperationResult::Present(lstat(
                PathNodeKind::RegularFile,
            )));
        operations
            .read_links
            .push_back(Ok(Arc::new(base.as_path().to_path_buf())));
        operations.file_bytes.push_back(Ok(Arc::from(&b"x"[..])));
        operations.directory_entries.push_back(Ok(empty_entries()));
        let epoch = observe_with(&retained, demands.clone(), &mut operations).unwrap();
        assert_eq!(operations.support_queries, 1);
        assert_eq!(operations.calls.len(), 4);
        assert!(matches!(
            epoch.get(&demands[0]).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Present(_))
        ));
        assert!(matches!(
            epoch.get(&demands[1]).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Present(_))
        ));
        assert!(matches!(
            epoch.get(&demands[2]).unwrap().as_ref(),
            PathObservationResult::FileBytes(PathOperationResult::Present(bytes))
                if bytes.as_ref() == b"x"
        ));
        assert!(matches!(
            epoch.get(&demands[3]).unwrap().as_ref(),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(_))
        ));
    }

    #[test]
    fn lstat_directly_preserves_missing_and_io_error() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let missing = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "a-missing"),
            PathObservationOperation::Lstat,
        );
        let failed = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "b-failed"),
            PathObservationOperation::Lstat,
        );
        let original = io(PathIoErrorKind::PermissionDenied, Some(13));
        let mut operations = ScriptedOperations::supported();
        operations.lstats.extend([
            PathOperationResult::Missing,
            PathOperationResult::Error(original),
        ]);
        let epoch = observe_with(
            &retained,
            [failed.clone(), missing.clone()],
            &mut operations,
        )
        .unwrap();
        assert!(matches!(
            epoch.get(&missing).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Missing)
        ));
        assert!(matches!(
            epoch.get(&failed).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Error(error))
                if *error == original
        ));
    }

    #[test]
    fn final_primary_failures_never_run_auxiliary_lstat() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        for operation in [
            PathObservationOperation::ReadLink,
            PathObservationOperation::FileBytes,
            PathObservationOperation::DirectoryEntries,
        ] {
            let demand = demand(
                PathObservationNamespace::Host,
                path(temp.path(), &format!("{operation:?}")),
                operation,
            );
            let original = io(PathIoErrorKind::PermissionDenied, Some(13));
            let mut operations = ScriptedOperations::supported();
            match operation {
                PathObservationOperation::ReadLink => operations
                    .read_links
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::FileBytes => operations
                    .file_bytes
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::DirectoryEntries => operations
                    .directory_entries
                    .push_back(Err(PrimaryFailure::Final(original))),
                PathObservationOperation::Lstat => unreachable!(),
            }
            let epoch = observe_with(&retained, [demand.clone()], &mut operations).unwrap();
            assert_eq!(operations.calls.len(), 1);
            let result = epoch.get(&demand).unwrap();
            assert!(matches!(
                result.as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Error(error))
                    | PathObservationResult::FileBytes(PathOperationResult::Error(error))
                    | PathObservationResult::DirectoryEntries(PathOperationResult::Error(error))
                    if *error == original
            ));
        }
    }

    #[test]
    fn read_link_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = PathObservationError::NotALink;
        assert_eq!(
            refine_read_link(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::Directory,
            PathNodeKind::SpecialFile,
        ] {
            assert_eq!(
                refine_read_link(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(PathObservationError::WrongKind {
                    expected: PathNodeKind::Symlink,
                    actual: kind
                })
            );
        }
        assert_eq!(
            refine_read_link(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Symlink))
            ),
            PathOperationResult::Error(original)
        );
        assert_eq!(
            refine_read_link(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );

        let original_io = io(PathIoErrorKind::NotFound, Some(2));
        assert_eq!(
            refine_read_link(
                original_io,
                PathOperationResult::Present(lstat(PathNodeKind::Symlink))
            ),
            PathOperationResult::Error(original_io)
        );
        assert_eq!(
            refine_read_link(
                original_io,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original_io)
        );
    }

    #[test]
    fn file_bytes_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = io(PathIoErrorKind::IsADirectory, Some(21));
        assert_eq!(
            refine_file_bytes(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::SpecialFile,
            PathNodeKind::Symlink,
        ] {
            assert_eq!(
                refine_file_bytes(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(original)
            );
        }
        assert_eq!(
            refine_file_bytes(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Directory))
            ),
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory
            })
        );
        assert_eq!(
            refine_file_bytes(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );
    }

    #[test]
    fn directory_refinement_exhausts_every_auxiliary_kind_and_race() {
        let original = io(PathIoErrorKind::NotADirectory, None);
        assert_eq!(
            refine_directory_entries(original, PathOperationResult::Missing),
            PathOperationResult::Missing
        );
        for kind in [
            PathNodeKind::RegularFile,
            PathNodeKind::Symlink,
            PathNodeKind::SpecialFile,
        ] {
            assert_eq!(
                refine_directory_entries(original, PathOperationResult::Present(lstat(kind))),
                PathOperationResult::Error(PathObservationError::WrongKind {
                    expected: PathNodeKind::Directory,
                    actual: kind
                })
            );
        }
        assert_eq!(
            refine_directory_entries(
                original,
                PathOperationResult::Present(lstat(PathNodeKind::Directory))
            ),
            PathOperationResult::Error(original)
        );
        assert_eq!(
            refine_directory_entries(
                original,
                PathOperationResult::Error(io(PathIoErrorKind::PermissionDenied, Some(13)))
            ),
            PathOperationResult::Error(original)
        );
    }

    #[test]
    fn refine_calls_primary_before_auxiliary_and_special_bytes_can_succeed() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let link_path = path(temp.path(), "link");
        let bytes_path = path(temp.path(), "special");
        let link = demand(
            PathObservationNamespace::Host,
            link_path.clone(),
            PathObservationOperation::ReadLink,
        );
        let bytes = demand(
            PathObservationNamespace::Host,
            bytes_path.clone(),
            PathObservationOperation::FileBytes,
        );
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Refine(PathObservationError::NotALink)));
        operations
            .lstats
            .push_back(PathOperationResult::Present(lstat(PathNodeKind::Symlink)));
        operations
            .file_bytes
            .push_back(Ok(Arc::from(&b"special"[..])));
        let epoch =
            observe_with(&retained, [bytes.clone(), link.clone()], &mut operations).unwrap();
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(link_path.clone()),
                Call::Lstat(link_path),
                Call::FileBytes(bytes_path)
            ]
        );
        assert!(matches!(
            epoch.get(&link).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Error(
                PathObservationError::NotALink
            ))
        ));
        assert!(matches!(
            epoch.get(&bytes).unwrap().as_ref(),
            PathObservationResult::FileBytes(PathOperationResult::Present(value))
                if value.as_ref() == b"special"
        ));
    }

    #[test]
    fn every_refinable_operation_calls_primary_before_its_auxiliary() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let link_path = path(temp.path(), "a-link");
        let bytes_path = path(temp.path(), "b-bytes");
        let directory_path = path(temp.path(), "c-directory");
        let demands = [
            demand(
                PathObservationNamespace::Host,
                link_path.clone(),
                PathObservationOperation::ReadLink,
            ),
            demand(
                PathObservationNamespace::Host,
                bytes_path.clone(),
                PathObservationOperation::FileBytes,
            ),
            demand(
                PathObservationNamespace::Host,
                directory_path.clone(),
                PathObservationOperation::DirectoryEntries,
            ),
        ];
        let mut operations = ScriptedOperations::supported();
        operations
            .read_links
            .push_back(Err(PrimaryFailure::Refine(PathObservationError::NotALink)));
        operations
            .file_bytes
            .push_back(Err(PrimaryFailure::Refine(io(
                PathIoErrorKind::NotFound,
                None,
            ))));
        operations
            .directory_entries
            .push_back(Err(PrimaryFailure::Refine(io(
                PathIoErrorKind::NotADirectory,
                None,
            ))));
        operations.lstats.extend([
            PathOperationResult::Missing,
            PathOperationResult::Present(lstat(PathNodeKind::SpecialFile)),
            PathOperationResult::Present(lstat(PathNodeKind::Directory)),
        ]);
        observe_with(&retained, demands, &mut operations).unwrap();
        assert_eq!(
            operations.calls,
            vec![
                Call::ReadLink(link_path.clone()),
                Call::Lstat(link_path),
                Call::FileBytes(bytes_path.clone()),
                Call::Lstat(bytes_path),
                Call::DirectoryEntries(directory_path.clone()),
                Call::Lstat(directory_path),
            ]
        );
    }

    #[test]
    fn directory_entry_value_can_flow_through_matching_epoch_variant() {
        let temp = tempfile::tempdir().unwrap();
        let owner = ();
        let retained = RetainedMaterializationRoots::new(&owner, []).unwrap();
        let demand = demand(
            PathObservationNamespace::Host,
            path(temp.path(), "directory"),
            PathObservationOperation::DirectoryEntries,
        );
        let entries =
            PathDirectoryEntries::new([PathDirectoryName::new("entry").unwrap()]).unwrap();
        let mut operations = ScriptedOperations::supported();
        operations.directory_entries.push_back(Ok(entries));
        let epoch = observe_with(&retained, [demand.clone()], &mut operations).unwrap();
        assert!(matches!(
            epoch.get(&demand).unwrap().as_ref(),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                if entries.names()[0].as_os_str() == "entry"
        ));
    }

    #[cfg(unix)]
    mod unix_tests {
        use std::collections::VecDeque;
        use std::ffi::OsString;
        use std::fs;
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        use super::*;

        #[derive(Debug, PartialEq, Eq)]
        enum DirectoryEvent {
            Open(Vec<u8>),
            Clear,
            Read(usize),
            Close(usize),
        }

        struct ScriptedDirectoryApi {
            opens: VecDeque<Result<usize, i32>>,
            reads: VecDeque<UnixDirectoryRead>,
            closes: VecDeque<Result<(), i32>>,
            events: Vec<DirectoryEvent>,
        }

        impl ScriptedDirectoryApi {
            fn new(
                opens: impl IntoIterator<Item = Result<usize, i32>>,
                reads: impl IntoIterator<Item = UnixDirectoryRead>,
                closes: impl IntoIterator<Item = Result<(), i32>>,
            ) -> Self {
                Self {
                    opens: opens.into_iter().collect(),
                    reads: reads.into_iter().collect(),
                    closes: closes.into_iter().collect(),
                    events: Vec::new(),
                }
            }

            fn close_count(&self) -> usize {
                self.events
                    .iter()
                    .filter(|event| matches!(event, DirectoryEvent::Close(_)))
                    .count()
            }
        }

        impl UnixDirectoryApi for ScriptedDirectoryApi {
            type Handle = usize;

            fn open_once(&mut self, path: &std::ffi::CStr) -> Result<Self::Handle, i32> {
                self.events
                    .push(DirectoryEvent::Open(path.to_bytes().to_vec()));
                self.opens.pop_front().expect("scripted open result")
            }

            fn clear_errno(&mut self) {
                self.events.push(DirectoryEvent::Clear);
            }

            fn read_once(&mut self, handle: Self::Handle) -> UnixDirectoryRead {
                self.events.push(DirectoryEvent::Read(handle));
                self.reads.pop_front().expect("scripted read result")
            }

            fn close_once(&mut self, handle: Self::Handle) -> Result<(), i32> {
                self.events.push(DirectoryEvent::Close(handle));
                self.closes.pop_front().expect("scripted close result")
            }
        }

        fn normalized(temp: &tempfile::TempDir, suffix: &str) -> NormalizedAbsolutePath {
            path(temp.path(), suffix)
        }

        fn error_raw(failure: PrimaryFailure) -> Option<i32> {
            match failure {
                PrimaryFailure::Refine(PathObservationError::Io { raw_os_error, .. })
                | PrimaryFailure::Final(PathObservationError::Io { raw_os_error, .. }) => {
                    raw_os_error
                }
                _ => None,
            }
        }

        #[test]
        fn owner_explicit_close_disarms_on_success_and_failure_and_drop_is_fallback() {
            for close_result in [Ok(()), Err(nix::libc::EIO)] {
                let mut api = ScriptedDirectoryApi::new([], [], [close_result]);
                assert_eq!(UnixDirectoryOwner::new(7, &mut api).close(), close_result);
                assert_eq!(api.close_count(), 1);
            }

            let mut api = ScriptedDirectoryApi::new([], [], [Ok(())]);
            {
                let _owner = UnixDirectoryOwner::new(9, &mut api);
            }
            assert_eq!(api.events, vec![DirectoryEvent::Close(9)]);
            assert_eq!(api.close_count(), 1);
        }

        #[test]
        fn retry_interrupted_retries_only_interrupted_errors() {
            let mut attempts = 0;
            let value = retry_interrupted(|| {
                attempts += 1;
                if attempts < 3 {
                    Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
                } else {
                    Ok(17)
                }
            })
            .unwrap();
            assert_eq!(value, 17);
            assert_eq!(attempts, 3);

            let mut attempts = 0;
            let error = retry_interrupted(|| {
                attempts += 1;
                Err::<(), _>(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            })
            .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            assert_eq!(attempts, 1);
        }

        #[test]
        fn opener_retries_only_eintr_and_preserves_exact_other_errors() {
            let temp = tempfile::tempdir().unwrap();
            let observed = normalized(&temp, "directory");
            let mut api = ScriptedDirectoryApi::new(
                [Err(nix::libc::EINTR), Err(nix::libc::EINTR), Ok(17)],
                [UnixDirectoryRead::Null(0)],
                [Ok(())],
            );
            assert!(unix_directory_entries_with(&observed, &mut api).is_ok());
            assert_eq!(
                api.events,
                vec![
                    DirectoryEvent::Open(
                        observed.as_path().as_os_str().as_encoded_bytes().to_vec()
                    ),
                    DirectoryEvent::Open(
                        observed.as_path().as_os_str().as_encoded_bytes().to_vec()
                    ),
                    DirectoryEvent::Open(
                        observed.as_path().as_os_str().as_encoded_bytes().to_vec()
                    ),
                    DirectoryEvent::Clear,
                    DirectoryEvent::Read(17),
                    DirectoryEvent::Close(17),
                ]
            );

            for raw in [nix::libc::EIO, nix::libc::EACCES] {
                let mut api = ScriptedDirectoryApi::new([Err(raw)], [], []);
                let failure = unix_directory_entries_with(&observed, &mut api).unwrap_err();
                assert!(matches!(failure, PrimaryFailure::Final(_)));
                assert_eq!(error_raw(failure), Some(raw));
                assert_eq!(api.close_count(), 0);
            }

            let mut api = ScriptedDirectoryApi::new([Err(nix::libc::ENOENT)], [], []);
            assert!(matches!(
                unix_directory_entries_with(&observed, &mut api),
                Err(PrimaryFailure::Refine(_))
            ));
        }

        #[test]
        fn one_handle_retains_names_across_eintr_eio_and_clears_every_read() {
            let temp = tempfile::tempdir().unwrap();
            let observed = normalized(&temp, "directory");
            let mut api = ScriptedDirectoryApi::new(
                [Ok(23)],
                [
                    UnixDirectoryRead::Name(b"z".to_vec()),
                    UnixDirectoryRead::Null(nix::libc::EINTR),
                    UnixDirectoryRead::Name(b".".to_vec()),
                    UnixDirectoryRead::Null(nix::libc::EIO),
                    UnixDirectoryRead::Name(vec![0xff]),
                    UnixDirectoryRead::Name(b"..".to_vec()),
                    UnixDirectoryRead::Name(b"a".to_vec()),
                    UnixDirectoryRead::Null(0),
                ],
                [Ok(())],
            );
            let entries = unix_directory_entries_with(&observed, &mut api).unwrap();
            assert_eq!(
                entries
                    .names()
                    .iter()
                    .map(|name| name.as_os_str().as_encoded_bytes().to_vec())
                    .collect::<Vec<_>>(),
                vec![b"a".to_vec(), b"z".to_vec(), vec![0xff]]
            );
            assert_eq!(
                api.events
                    .iter()
                    .filter(|event| matches!(event, DirectoryEvent::Clear))
                    .count(),
                8
            );
            assert!(
                api.events
                    .iter()
                    .filter_map(|event| match event {
                        DirectoryEvent::Read(handle) => Some(*handle),
                        _ => None,
                    })
                    .all(|handle| handle == 23)
            );
            assert_eq!(api.close_count(), 1);
        }

        #[test]
        fn iterator_error_discards_partial_names_and_wins_over_close_error() {
            let temp = tempfile::tempdir().unwrap();
            let observed = normalized(&temp, "directory");
            let mut api = ScriptedDirectoryApi::new(
                [Ok(31)],
                [
                    UnixDirectoryRead::Name(b"partial".to_vec()),
                    UnixDirectoryRead::Null(nix::libc::EACCES),
                ],
                [Err(nix::libc::EIO)],
            );
            let failure = unix_directory_entries_with(&observed, &mut api).unwrap_err();
            assert_eq!(error_raw(failure), Some(nix::libc::EACCES));
            assert_eq!(api.close_count(), 1);
            assert_eq!(api.events.last(), Some(&DirectoryEvent::Close(31)));

            let mut api = ScriptedDirectoryApi::new(
                [Ok(32)],
                [
                    UnixDirectoryRead::Name(b"discarded".to_vec()),
                    UnixDirectoryRead::Null(nix::libc::ENOENT),
                ],
                [Err(nix::libc::EIO)],
            );
            let failure = unix_directory_entries_with(&observed, &mut api).unwrap_err();
            assert!(matches!(failure, PrimaryFailure::Refine(_)));
            assert_eq!(error_raw(failure), Some(nix::libc::ENOENT));
            assert_eq!(api.close_count(), 1);
        }

        #[test]
        fn eof_close_success_eintr_error_and_candidate_refinement_are_exact() {
            let temp = tempfile::tempdir().unwrap();
            let observed = normalized(&temp, "directory");
            for close in [Ok(()), Err(nix::libc::EINTR)] {
                let mut api =
                    ScriptedDirectoryApi::new([Ok(41)], [UnixDirectoryRead::Null(0)], [close]);
                assert!(unix_directory_entries_with(&observed, &mut api).is_ok());
                assert_eq!(api.close_count(), 1);
            }

            let mut api = ScriptedDirectoryApi::new(
                [Ok(42)],
                [UnixDirectoryRead::Null(0)],
                [Err(nix::libc::EIO)],
            );
            let failure = unix_directory_entries_with(&observed, &mut api).unwrap_err();
            assert!(matches!(failure, PrimaryFailure::Final(_)));
            assert_eq!(error_raw(failure), Some(nix::libc::EIO));

            let mut api = ScriptedDirectoryApi::new(
                [Ok(43)],
                [UnixDirectoryRead::Null(0)],
                [Err(nix::libc::ENOTDIR)],
            );
            assert!(matches!(
                unix_directory_entries_with(&observed, &mut api),
                Err(PrimaryFailure::Refine(_))
            ));
            assert_eq!(api.close_count(), 1);
        }

        #[test]
        fn invalid_duplicate_and_interior_nul_are_final_and_cleanup_once() {
            let temp = tempfile::tempdir().unwrap();
            let observed = normalized(&temp, "directory");
            for reads in [
                vec![UnixDirectoryRead::Name(b"a/b".to_vec())],
                vec![
                    UnixDirectoryRead::Name(b"same".to_vec()),
                    UnixDirectoryRead::Name(b"same".to_vec()),
                    UnixDirectoryRead::Null(0),
                ],
            ] {
                let mut api = ScriptedDirectoryApi::new([Ok(51)], reads, [Err(nix::libc::EIO)]);
                assert!(matches!(
                    unix_directory_entries_with(&observed, &mut api),
                    Err(PrimaryFailure::Final(PathObservationError::Io {
                        kind: PathIoErrorKind::InvalidData,
                        ..
                    }))
                ));
                assert_eq!(api.close_count(), 1);
            }

            let nul = NormalizedAbsolutePath::new(
                temp.path().join(OsString::from_vec(b"nul\0name".to_vec())),
            )
            .unwrap();
            let mut api = ScriptedDirectoryApi::new([], [], []);
            assert!(matches!(
                unix_directory_entries_with(&nul, &mut api),
                Err(PrimaryFailure::Final(PathObservationError::Io {
                    kind: PathIoErrorKind::InvalidInput,
                    raw_os_error: None,
                }))
            ));
            assert!(api.events.is_empty());
        }

        #[test]
        fn real_unix_lstat_readlink_and_file_bytes_preserve_native_values() {
            let temp = tempfile::tempdir().unwrap();
            let file = temp.path().join("file");
            let directory = temp.path().join("directory");
            let link = temp.path().join("link");
            let socket = temp.path().join("socket");
            fs::write(&file, b"bytes").unwrap();
            fs::create_dir(&directory).unwrap();
            symlink("file", &link).unwrap();
            let _listener = UnixListener::bind(&socket).unwrap();

            for (native, expected) in [
                (&file, PathNodeKind::RegularFile),
                (&directory, PathNodeKind::Directory),
                (&link, PathNodeKind::Symlink),
                (&socket, PathNodeKind::SpecialFile),
            ] {
                let normalized = NormalizedAbsolutePath::new(native).unwrap();
                let PathOperationResult::Present(observed) = unix_lstat(&normalized) else {
                    panic!("expected present lstat for {native:?}")
                };
                let metadata = fs::symlink_metadata(native).unwrap();
                assert_eq!(observed.kind(), expected);
                assert_eq!(observed.size(), metadata.size() as i64);
                assert_eq!(observed.node_id(), metadata.ino() as i64);
                assert_eq!(observed.permissions(), (metadata.mode() & 0o777) as i32);
                assert_eq!(
                    observed.mtime_millis(),
                    metadata
                        .mtime()
                        .wrapping_mul(1_000)
                        .wrapping_add(metadata.mtime_nsec() / 1_000_000)
                );
                assert_eq!(
                    observed.ctime_millis(),
                    metadata
                        .ctime()
                        .wrapping_mul(1_000)
                        .wrapping_add(metadata.ctime_nsec() / 1_000_000)
                );
            }

            let mut adapter = UnixPathObservationAdapter;
            assert_eq!(
                adapter
                    .read_link(&NormalizedAbsolutePath::new(&link).unwrap())
                    .unwrap()
                    .as_path(),
                Path::new("file")
            );
            assert_eq!(
                adapter
                    .file_bytes(&NormalizedAbsolutePath::new(&file).unwrap())
                    .unwrap()
                    .as_ref(),
                b"bytes"
            );
            assert_eq!(
                adapter
                    .file_bytes(&NormalizedAbsolutePath::new(&link).unwrap())
                    .unwrap()
                    .as_ref(),
                b"bytes"
            );
            assert!(matches!(
                adapter.read_link(&NormalizedAbsolutePath::new(&file).unwrap()),
                Err(PrimaryFailure::Refine(PathObservationError::NotALink))
            ));
            let nul = NormalizedAbsolutePath::new(
                temp.path().join(OsString::from_vec(b"nul\0link".to_vec())),
            )
            .unwrap();
            assert!(matches!(
                adapter.read_link(&nul),
                Err(PrimaryFailure::Final(PathObservationError::Io {
                    kind: PathIoErrorKind::InvalidInput,
                    raw_os_error: None,
                }))
            ));
            assert!(matches!(
                adapter.read_link(
                    &NormalizedAbsolutePath::new(file.join("not-a-directory")).unwrap()
                ),
                Err(PrimaryFailure::Refine(PathObservationError::Io {
                    kind: PathIoErrorKind::NotADirectory,
                    raw_os_error: Some(raw),
                })) if raw == nix::libc::ENOTDIR
            ));
            assert!(matches!(
                adapter.file_bytes(&NormalizedAbsolutePath::new(&directory).unwrap()),
                Err(PrimaryFailure::Refine(PathObservationError::Io {
                    kind: PathIoErrorKind::IsADirectory,
                    ..
                }))
            ));
            assert!(matches!(
                unix_lstat(&NormalizedAbsolutePath::new(file.join("not-a-directory")).unwrap()),
                PathOperationResult::Missing
            ));
        }

        #[test]
        fn real_file_directory_and_symlink_lifecycles_are_fresh_and_sorted() {
            let temp = tempfile::tempdir().unwrap();
            let owner = ();
            let root = path(temp.path(), "root");
            fs::create_dir(root.as_path()).unwrap();
            let retained = roots(&owner, &root);
            let file = path(temp.path(), "root/file");
            let directory = path(temp.path(), "root/directory");
            let link = path(temp.path(), "root/link");
            let bytes_demand = demand(
                PathObservationNamespace::Host,
                file.clone(),
                PathObservationOperation::FileBytes,
            );
            let directory_demand = demand(
                PathObservationNamespace::Host,
                directory.clone(),
                PathObservationOperation::DirectoryEntries,
            );
            let link_demand = demand(
                PathObservationNamespace::Host,
                link.clone(),
                PathObservationOperation::ReadLink,
            );

            assert!(matches!(
                observe_unix(&retained, [bytes_demand.clone()])
                    .unwrap()
                    .get(&bytes_demand)
                    .unwrap()
                    .as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Missing)
            ));
            fs::write(file.as_path(), b"one").unwrap();
            fs::create_dir(directory.as_path()).unwrap();
            fs::write(directory.as_path().join("z"), b"").unwrap();
            fs::write(directory.as_path().join("a"), b"").unwrap();
            symlink("file", link.as_path()).unwrap();
            assert!(matches!(
                observe_unix(&retained, [bytes_demand.clone()]).unwrap().get(&bytes_demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Present(value)) if value.as_ref() == b"one"
            ));
            let directory_epoch = observe_unix(&retained, [directory_demand.clone()]).unwrap();
            assert!(matches!(
                directory_epoch.get(&directory_demand).unwrap().as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                    if entries.names().iter().map(|name| name.as_os_str()).collect::<Vec<_>>()
                        == vec!["a", "z"]
            ));
            assert!(matches!(
                observe_unix(&retained, [link_demand.clone()]).unwrap().get(&link_demand).unwrap().as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Present(target))
                    if target.as_path() == Path::new("file")
            ));

            fs::write(file.as_path(), b"two").unwrap();
            fs::remove_file(directory.as_path().join("a")).unwrap();
            fs::write(directory.as_path().join("b"), b"").unwrap();
            fs::remove_file(link.as_path()).unwrap();
            symlink("directory", link.as_path()).unwrap();
            assert!(matches!(
                observe_unix(&retained, [bytes_demand.clone()]).unwrap().get(&bytes_demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Present(value)) if value.as_ref() == b"two"
            ));
            assert!(matches!(
                observe_unix(&retained, [link_demand.clone()]).unwrap().get(&link_demand).unwrap().as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Present(target))
                    if target.as_path() == Path::new("directory")
            ));
            let mutated_directory = observe_unix(&retained, [directory_demand.clone()]).unwrap();
            assert!(matches!(
                mutated_directory
                    .get(&directory_demand)
                    .unwrap()
                    .as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                    if entries.names().iter().map(|name| name.as_os_str()).collect::<Vec<_>>()
                        == vec!["b", "z"]
            ));

            fs::remove_file(file.as_path()).unwrap();
            fs::remove_dir_all(directory.as_path()).unwrap();
            fs::remove_file(link.as_path()).unwrap();
            let deleted = observe_unix(
                &retained,
                [
                    bytes_demand.clone(),
                    directory_demand.clone(),
                    link_demand.clone(),
                ],
            )
            .unwrap();
            assert!(matches!(
                deleted.get(&bytes_demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Missing)
            ));
            assert!(matches!(
                deleted.get(&directory_demand).unwrap().as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Missing)
            ));
            assert!(matches!(
                deleted.get(&link_demand).unwrap().as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Missing)
            ));
            fs::write(file.as_path(), b"three").unwrap();
            fs::create_dir(directory.as_path()).unwrap();
            fs::write(directory.as_path().join("recreated"), b"").unwrap();
            symlink("file", link.as_path()).unwrap();
            let recreated = observe_unix(
                &retained,
                [
                    bytes_demand.clone(),
                    directory_demand.clone(),
                    link_demand.clone(),
                ],
            )
            .unwrap();
            assert!(matches!(
                recreated.get(&bytes_demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Present(value))
                    if value.as_ref() == b"three"
            ));
            assert!(matches!(
                recreated.get(&directory_demand).unwrap().as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                    if entries.names().iter().map(|name| name.as_os_str()).collect::<Vec<_>>()
                        == vec!["recreated"]
            ));
            assert!(matches!(
                recreated.get(&link_demand).unwrap().as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Present(target))
                    if target.as_path() == Path::new("file")
            ));
        }

        #[test]
        fn real_non_utf8_wrongkind_and_exact_authorized_escaped_paths() {
            let temp = tempfile::tempdir().unwrap();
            let owner = ();
            let root = path(temp.path(), "retained");
            let outside = path(temp.path(), "outside");
            fs::create_dir(root.as_path()).unwrap();
            fs::create_dir(outside.as_path()).unwrap();
            fs::write(root.as_path().join("inside"), b"inside").unwrap();
            fs::write(outside.as_path().join("value"), b"escaped").unwrap();
            symlink(outside.as_path(), root.as_path().join("escape")).unwrap();
            fs::write(
                root.as_path().join(OsString::from_vec(vec![b'n', 0xff])),
                b"",
            )
            .unwrap();
            let retained = roots(&owner, &root);

            let escaped = NormalizedAbsolutePath::new(root.as_path().join("escape/value")).unwrap();
            let direct_outside =
                NormalizedAbsolutePath::new(outside.as_path().join("value")).unwrap();
            let host = demand(
                PathObservationNamespace::Host,
                escaped.clone(),
                PathObservationOperation::FileBytes,
            );
            let materialized = demand(
                PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
                escaped.clone(),
                PathObservationOperation::FileBytes,
            );
            let outside_demand = demand(
                PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
                direct_outside,
                PathObservationOperation::FileBytes,
            );
            let inside_demand = demand(
                PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
                NormalizedAbsolutePath::new(root.as_path().join("inside")).unwrap(),
                PathObservationOperation::FileBytes,
            );
            let epoch = observe_unix(
                &retained,
                [
                    materialized.clone(),
                    host.clone(),
                    outside_demand.clone(),
                    inside_demand.clone(),
                ],
            )
            .unwrap();
            assert_ne!(host, materialized);
            assert_eq!(epoch.observations().len(), 4);
            for escaped_demand in [&host, &materialized, &outside_demand] {
                assert!(matches!(
                    epoch.get(escaped_demand).unwrap().as_ref(),
                    PathObservationResult::FileBytes(PathOperationResult::Present(value))
                        if value.as_ref() == b"escaped"
                ));
            }
            assert!(matches!(
                epoch.get(&inside_demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Present(value))
                    if value.as_ref() == b"inside"
            ));

            let directory = demand(
                PathObservationNamespace::Host,
                root.clone(),
                PathObservationOperation::DirectoryEntries,
            );
            let directory_epoch = observe_unix(&retained, [directory.clone()]).unwrap();
            assert!(matches!(
                directory_epoch.get(&directory).unwrap().as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
                    if entries.names().iter().any(|name| name.as_os_str().as_encoded_bytes() == [b'n', 0xff])
            ));

            let wrong_link = demand(
                PathObservationNamespace::Host,
                root.clone(),
                PathObservationOperation::ReadLink,
            );
            let wrong_directory = demand(
                PathObservationNamespace::Host,
                escaped,
                PathObservationOperation::DirectoryEntries,
            );
            let smoke =
                observe_unix(&retained, [wrong_link.clone(), wrong_directory.clone()]).unwrap();
            assert!(matches!(
                smoke.get(&wrong_link).unwrap().as_ref(),
                PathObservationResult::ReadLink(PathOperationResult::Error(
                    PathObservationError::WrongKind {
                        expected: PathNodeKind::Symlink,
                        actual: PathNodeKind::Directory
                    }
                ))
            ));
            assert!(matches!(
                smoke.get(&wrong_directory).unwrap().as_ref(),
                PathObservationResult::DirectoryEntries(PathOperationResult::Error(
                    PathObservationError::WrongKind {
                        expected: PathNodeKind::Directory,
                        actual: PathNodeKind::RegularFile
                    }
                ))
            ));
        }
    }
}
