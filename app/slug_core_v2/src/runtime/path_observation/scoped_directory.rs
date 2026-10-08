//! Linux test-scope enumeration: bounded borrowed native names, ordinary semantic values.
use std::ffi::CStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

use super::*;

pub(in crate::runtime) const ENTRY_LIMIT: usize = 128;
pub(in crate::runtime) const NAME_LIMIT: usize = 1024;
pub(in crate::runtime) const NAMES_LIMIT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime) enum DirectoryLimit {
    Entries,
    NameBytes,
    TotalNameBytes,
}

enum Failure {
    Semantic(PrimaryFailure),
    Operational(PathObservationKernelError),
}

enum Read<'a> {
    Entry { name: &'a [u8], native_type: u8 },
    Null(i32),
}

// The owned ordinary read remains unchanged and is never called by this path.
trait ScopedApi: UnixDirectoryApi {
    fn read_borrowed(&mut self, handle: Self::Handle) -> Result<Read<'_>, Failure>;
    fn kind_once(&mut self, path: &Path) -> Result<PathNodeKind, i32>;
}

impl ScopedApi for LibcUnixDirectoryApi {
    fn read_borrowed(&mut self, handle: Self::Handle) -> Result<Read<'_>, Failure> {
        // SAFETY: the armed owner holds a live DIR. No subsequent read occurs
        // while the returned entry borrows this API.
        let entry = unsafe { nix::libc::readdir(handle.as_ptr()) };
        if entry.is_null() {
            return Ok(Read::Null(nix::errno::Errno::last_raw()));
        }
        let offset = std::mem::offset_of!(nix::libc::dirent, d_name);
        // SAFETY: Linux readdir supplies d_reclen bytes including the trailing
        // NUL. Check that record boundary, and scan at most limit+1 bytes before
        // copying; do not use the unbounded ordinary CStr/to_vec reader.
        let record_bytes = unsafe { (*entry).d_reclen as usize };
        let available = record_bytes
            .checked_sub(offset)
            .filter(|available| *available != 0)
            .ok_or_else(|| Failure::Semantic(invalid_directory_data()))?;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                entry.cast::<u8>().add(offset),
                available.min(NAME_LIMIT + 1),
            )
        };
        Ok(Read::Entry {
            name: native_name(bytes)?,
            // SAFETY: d_type is within the live native record.
            native_type: unsafe { (*entry).d_type },
        })
    }

    fn kind_once(&mut self, path: &Path) -> Result<PathNodeKind, i32> {
        std::fs::symlink_metadata(path)
            .map(|metadata| {
                let kind = metadata.file_type();
                if kind.is_symlink() {
                    PathNodeKind::Symlink
                } else if kind.is_dir() {
                    PathNodeKind::Directory
                } else if kind.is_file() {
                    PathNodeKind::RegularFile
                } else {
                    PathNodeKind::SpecialFile
                }
            })
            .map_err(|error| error.raw_os_error().unwrap_or(nix::libc::EIO))
    }
}

fn native_name(bytes: &[u8]) -> Result<&[u8], Failure> {
    match bytes
        .iter()
        .take(NAME_LIMIT + 1)
        .position(|byte| *byte == 0)
    {
        Some(length) => Ok(&bytes[..length]),
        None if bytes.len() > NAME_LIMIT => Err(limit(DirectoryLimit::NameBytes)),
        None => Err(Failure::Semantic(invalid_directory_data())),
    }
}

impl<A: ScopedApi> UnixDirectoryOwner<'_, A> {
    fn read_borrowed(&mut self) -> Result<Read<'_>, Failure> {
        self.api
            .read_borrowed(self.handle.expect("directory owner must be armed"))
    }
}

fn limit(reason: DirectoryLimit) -> Failure {
    Failure::Operational(PathObservationKernelError::DirectoryLimit(reason))
}

fn check(scope: &Arc<SourceServiceScope>) -> Result<(), Failure> {
    scope
        .check()
        .map_err(|error| Failure::Operational(PathObservationKernelError::SourceService(error)))
}

fn kind_with<A: ScopedApi>(
    api: &mut A,
    scope: &Arc<SourceServiceScope>,
    path: &Path,
) -> Result<Result<PathNodeKind, i32>, Failure> {
    loop {
        check(scope)?;
        let result = api.kind_once(path);
        check(scope)?;
        match result {
            Err(nix::libc::EINTR) => continue,
            result => return Ok(result),
        }
    }
}

fn collect<A: ScopedApi>(
    path: &NormalizedAbsolutePath,
    scope: &Arc<SourceServiceScope>,
    api: &mut A,
) -> Result<PathDirectoryEntries, Failure> {
    let c_path = std::ffi::CString::new(path.as_path().as_os_str().as_bytes()).map_err(|_| {
        Failure::Semantic(PrimaryFailure::Final(path_io_error(&std::io::Error::from(
            std::io::ErrorKind::InvalidInput,
        ))))
    })?;
    let handle = loop {
        check(scope)?;
        let opened = api.open_once(&c_path);
        // Arm before the post-call check, so an opened handle closes on refusal.
        match opened {
            Ok(handle) => break handle,
            Err(nix::libc::EINTR) => continue,
            Err(raw) => {
                check(scope)?;
                return Err(Failure::Semantic(classify_directory_error(raw)));
            }
        }
    };
    let mut owner = UnixDirectoryOwner::new(handle, api);
    check(scope)?;
    let mut entries = Vec::new();
    let mut name_bytes = 0;
    loop {
        check(scope)?;
        owner.clear_errno();
        let read = owner.read_borrowed()?;
        check(scope)?;
        match read {
            Read::Entry { name, native_type } => {
                if name == b"." || name == b".." {
                    continue;
                }
                if entries.len() == ENTRY_LIMIT {
                    return Err(limit(DirectoryLimit::Entries));
                }
                if name.len() > NAME_LIMIT {
                    return Err(limit(DirectoryLimit::NameBytes));
                }
                if name.len() > NAMES_LIMIT - name_bytes {
                    return Err(limit(DirectoryLimit::TotalNameBytes));
                }
                name_bytes += name.len();
                // Only validated bounded bytes become independently owned names.
                let name = PathDirectoryName::new(std::ffi::OsString::from_vec(name.to_vec()))
                    .map_err(|_| Failure::Semantic(invalid_directory_data()))?;
                entries.push((name, native_type));
            }
            Read::Null(0) => {
                let closed = owner.close();
                check(scope)?;
                match closed {
                    Ok(()) | Err(nix::libc::EINTR) => break,
                    Err(raw) => return Err(Failure::Semantic(classify_directory_error(raw))),
                }
            }
            Read::Null(raw) if raw == nix::libc::EINTR || raw == nix::libc::EIO => continue,
            Read::Null(raw) => return Err(Failure::Semantic(classify_directory_error(raw))),
        }
    }
    let mut classified = Vec::with_capacity(entries.len());
    for (name, native_type) in entries {
        check(scope)?;
        let kind = match unix_native_directory_kind(native_type) {
            Some(kind) => kind,
            None => {
                // Necessary transient child path, not retained beside the name.
                let child = path.as_path().join(name.as_os_str());
                match kind_with(api, scope, &child)? {
                    Ok(PathNodeKind::RegularFile) => PathDirectoryEntryKind::File,
                    Ok(PathNodeKind::Directory) => PathDirectoryEntryKind::Directory,
                    Ok(PathNodeKind::Symlink) => PathDirectoryEntryKind::Symlink,
                    Ok(PathNodeKind::SpecialFile)
                    | Err(nix::libc::ENOENT | nix::libc::ENOTDIR | nix::libc::ELOOP) => {
                        PathDirectoryEntryKind::Unknown
                    }
                    Err(raw) => {
                        return Err(Failure::Semantic(PrimaryFailure::Final(raw_path_io_error(
                            raw,
                        ))));
                    }
                }
            }
        };
        classified.push(PathDirectoryEntry::new(name, kind));
    }
    check(scope)?;
    Ok(PathDirectoryEntries::new(classified))
}

fn observe_with<A: ScopedApi>(
    demand: &PathObservationDemand,
    scope: &Arc<SourceServiceScope>,
    api: &mut A,
) -> Result<PathObservationResult, PathObservationKernelError> {
    let result = match collect(demand.path(), scope, api) {
        Ok(entries) => PathOperationResult::Present(entries),
        Err(Failure::Operational(error)) => return Err(error),
        Err(Failure::Semantic(PrimaryFailure::Final(error))) => PathOperationResult::Error(error),
        Err(Failure::Semantic(PrimaryFailure::Refine(original))) => {
            // Preserve ordinary refinement without its unbounded EINTR retry.
            match kind_with(api, scope, demand.path().as_path()) {
                Err(Failure::Operational(error)) => return Err(error),
                Err(Failure::Semantic(_)) => {
                    unreachable!("kind lookup has only operational refusal")
                }
                Ok(Err(nix::libc::ENOENT | nix::libc::ENOTDIR)) => PathOperationResult::Missing,
                Ok(Ok(actual)) if actual != PathNodeKind::Directory => {
                    PathOperationResult::Error(PathObservationError::WrongKind {
                        expected: PathNodeKind::Directory,
                        actual,
                    })
                }
                Ok(_) => PathOperationResult::Error(original),
            }
        }
    };
    scope
        .check()
        .map_err(PathObservationKernelError::SourceService)?;
    Ok(PathObservationResult::DirectoryEntries(result))
}

pub(super) fn observe(
    demand: &PathObservationDemand,
    scope: &Arc<SourceServiceScope>,
) -> Result<PathObservationResult, PathObservationKernelError> {
    observe_with(demand, scope, &mut LibcUnixDirectoryApi)
}

#[cfg(test)]
mod tests;
