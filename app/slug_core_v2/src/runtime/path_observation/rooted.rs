//! Inactive, borrowed materialization observer. The caller authenticates the
//! exact live root/anchor association and retains sealed or exclusive quiescent
//! backing throughout this call. An anchor is spelling, never native authority.
//! No registry lookup, ambient path opening, FD, or native lease enters DICE.
//!
//! The source bank covers modeled owned buffers and temporary File wrappers.
//! The caller separately owns actual descriptor/kernel resources (at most three
//! temporary descriptors per RootIo call); these credits are not an FD quota or
//! a kernel-memory/RSS claim. Incoming demand owners are already producer-paid.

use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use slug_workspace_v2::MaterializationPathAnchor;

use super::*;
use crate::runtime::dice::source_service::SourceBufferReservation;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::read_open_regular_file_after_attempt;
use crate::runtime::repository_root_io::RootIo;

const MAX_DEMANDS: usize = 131_072;
const MAX_PATH_STORAGE: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 131_072;
const MAX_DIRECTORY_NAMES: usize = 4 * 1024 * 1024;
const PATH_LIMIT: usize = 4096;
// RootIo component/syscall scratch, directory scratch or digest scratch are
// conservatively prepaid together; the shared FileBytes kernel pays its own
// capture and chunk buffer in addition to this fixed native allowance.
const NATIVE_CREDIT: usize = 64 * 1024
    + 8192
    + 3 * PATH_LIMIT
    + 1024
    + 3 * std::mem::size_of::<File>()
    + 3 * std::mem::size_of::<libc::stat>();

#[derive(Debug)]
pub(in crate::runtime) enum RootedObservationError {
    UnauthorizedDemand(usize),
    DuplicateDemand,
    Bound,
    Unstable,
    Inactive,
    RootIo(io::Error),
    SourceService(SourceServiceError),
    Epoch(PathObservationEpochError),
}

impl From<SourceServiceError> for RootedObservationError {
    fn from(error: SourceServiceError) -> Self {
        Self::SourceService(error)
    }
}

fn check(
    scope: &SourceServiceScope,
    active: &dyn Fn() -> bool,
) -> Result<(), RootedObservationError> {
    scope.check()?;
    if !active() {
        return Err(RootedObservationError::Inactive);
    }
    Ok(())
}

fn add(a: usize, b: usize) -> Result<usize, RootedObservationError> {
    a.checked_add(b).ok_or(RootedObservationError::Bound)
}

fn array<T>(count: usize) -> Result<usize, RootedObservationError> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
        .ok_or(RootedObservationError::Bound)
}

fn reserve(
    scope: &Arc<SourceServiceScope>,
    bytes: usize,
) -> Result<SourceBufferReservation, RootedObservationError> {
    Ok(scope.reserve_buffer(bytes)?)
}

fn vector<T>(count: usize) -> Result<Vec<T>, RootedObservationError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| RootedObservationError::Bound)?;
    if result.capacity() > count {
        return Err(RootedObservationError::Bound);
    }
    Ok(result)
}

fn relative<'a>(
    anchor: &MaterializationPathAnchor,
    demand: &'a PathObservationDemand,
    index: usize,
) -> Result<&'a Path, RootedObservationError> {
    if demand.namespace() != anchor.namespace()
        || !matches!(anchor.namespace(), PathObservationNamespace::Materialization(id) if id.value() != 0)
        || demand.path().as_path().as_os_str().len() > PATH_LIMIT
        || !matches!(
            demand.operation(),
            PathObservationOperation::Lstat
                | PathObservationOperation::ReadLink
                | PathObservationOperation::DirectoryEntries
                | PathObservationOperation::FileBytes
                | PathObservationOperation::FileDigest
        )
    {
        return Err(RootedObservationError::UnauthorizedDemand(index));
    }
    let relative = demand
        .path()
        .as_path()
        .strip_prefix(anchor.root().as_path())
        .map_err(|_| RootedObservationError::UnauthorizedDemand(index))?;
    let bytes = relative.as_os_str().as_bytes();
    if bytes.contains(&0)
        || (!bytes.is_empty()
            && (bytes.split(|b| *b == b'/').count() > 2048
                || bytes.split(|b| *b == b'/').any(|name| {
                    name.is_empty() || name == b"." || name == b".." || name.len() > 255
                })))
    {
        return Err(RootedObservationError::UnauthorizedDemand(index));
    }
    Ok(relative)
}

/// Caller must authenticate the full current selected root/anchor association,
/// not merely match display path text. All batch membership/duplicate/buffer
/// checks precede filesystem IO. Immutable results retain no authority/credits.
pub(in crate::runtime) fn observe_rooted(
    root: &RootIo<'_>,
    anchor: &MaterializationPathAnchor,
    demands: &[PathObservationDemand],
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationEpoch, RootedObservationError> {
    check(scope, active)?;
    if demands.len() > MAX_DEMANDS || anchor.root().as_path().as_os_str().len() > PATH_LIMIT {
        return Err(RootedObservationError::Bound);
    }
    let mut paths = anchor
        .root()
        .retained_storage_bytes()
        .ok_or(RootedObservationError::Bound)?;
    for (index, demand) in demands.iter().enumerate() {
        relative(anchor, demand, index)?;
        paths = add(
            paths,
            demand
                .path()
                .retained_storage_bytes()
                .ok_or(RootedObservationError::Bound)?,
        )?;
        if paths > MAX_PATH_STORAGE {
            return Err(RootedObservationError::Bound);
        }
    }
    // SortedMap uses Buck2's OrderedMap/SmallMap entry/hash/index arrays. Eight
    // times entry+hash+control plus fixed slack bounds construction/growth and
    // transfer alongside the input/result/index/credit vectors. The same
    // conservative component bound is used by the admitted workspace protocol.
    let per_map = add(
        std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>(),
        32,
    )?;
    let maps = demands
        .len()
        .checked_mul(per_map)
        .and_then(|n| n.checked_mul(8))
        .ok_or(RootedObservationError::Bound)?;
    let batch = add(
        paths,
        add(
            maps,
            add(
                array::<(PathObservationDemand, PathObservationResult)>(demands.len())?,
                add(
                    array::<PathObservationResult>(demands.len())?,
                    add(
                        array::<usize>(demands.len())?,
                        add(array::<SourceBufferReservation>(demands.len())?, 4096)?,
                    )?,
                )?,
            )?,
        )?,
    )?;
    let _batch_credit = reserve(scope, batch)?;
    let _native_credit = reserve(scope, NATIVE_CREDIT)?;
    let mut order = vector::<usize>(demands.len())?;
    order.extend(0..demands.len());
    order.sort_unstable_by(|a, b| demands[*a].cmp(&demands[*b]));
    if order
        .windows(2)
        .any(|pair| demands[pair[0]] == demands[pair[1]])
    {
        return Err(RootedObservationError::DuplicateDemand);
    }
    // Error unwinding drops later locals first: retained result payloads must
    // close before their modeled live-bank credits can be reused elsewhere.
    let mut credits = vector(demands.len())?;
    let mut observations = vector(demands.len())?;
    for index in order {
        check(scope, active)?;
        let demand = &demands[index];
        let path = relative(anchor, demand, index)?;
        // FileBytes calls the shared after-attempt kernel; no operation double
        // charges and missing/wrong-kind operations still consume their attempt.
        scope.charge(SourceCounter::SourceAttempts, 1)?;
        let result = observe_one(root, path, demand.operation(), scope, active, &mut credits)?;
        check(scope, active)?;
        observations.push((demand.clone(), result));
    }
    check(scope, active)?;
    let epoch = PathObservationEpoch::new(observations).map_err(RootedObservationError::Epoch)?;
    check(scope, active)?;
    Ok(epoch)
}

fn kind(stat: &libc::stat) -> PathNodeKind {
    match stat.st_mode & libc::S_IFMT {
        libc::S_IFREG => PathNodeKind::RegularFile,
        libc::S_IFDIR => PathNodeKind::Directory,
        libc::S_IFLNK => PathNodeKind::Symlink,
        _ => PathNodeKind::SpecialFile,
    }
}

fn lstat_value(stat: &libc::stat) -> PathLstat {
    PathLstat::new(
        kind(stat),
        stat.st_size,
        stat.st_mtime
            .wrapping_mul(1000)
            .wrapping_add(stat.st_mtime_nsec / 1_000_000),
        stat.st_ctime
            .wrapping_mul(1000)
            .wrapping_add(stat.st_ctime_nsec / 1_000_000),
        stat.st_ino as i64,
        (stat.st_mode & 0o777) as i32,
    )
}

fn unchanged(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}

fn io_result<T>(error: io::Error) -> Result<PathOperationResult<T>, RootedObservationError> {
    if matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData
    ) || matches!(
        error.raw_os_error(),
        Some(libc::ESTALE | libc::EXDEV | libc::ELOOP)
    ) {
        return Err(RootedObservationError::RootIo(error));
    }
    Ok(if is_missing_error(&error) {
        PathOperationResult::Missing
    } else {
        PathOperationResult::Error(path_io_error(&error))
    })
}

fn wrong<T>(expected: PathNodeKind, actual: PathNodeKind) -> PathOperationResult<T> {
    PathOperationResult::Error(PathObservationError::WrongKind { expected, actual })
}

fn framed_io_result<T>(error: io::Error) -> Result<PathOperationResult<T>, RootedObservationError> {
    // Missing is an ordinary fact only at the initial lstat. Disappearance or
    // replacement after a correctly typed frame is an unstable native capture.
    if is_missing_error(&error) || error.kind() == io::ErrorKind::IsADirectory {
        return Err(RootedObservationError::Unstable);
    }
    io_result(error)
}

fn observe_one(
    root: &RootIo<'_>,
    path: &Path,
    operation: PathObservationOperation,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    credits: &mut Vec<SourceBufferReservation>,
) -> Result<PathObservationResult, RootedObservationError> {
    let stat = root.lstat(path);
    check(scope, active)?;
    macro_rules! with_stat {
        ($variant:ident, $expected:expr, $body:expr) => {{
            let value = match stat {
                Err(error) => io_result(error)?,
                Ok(before) if kind(&before) != $expected => wrong($expected, kind(&before)),
                Ok(before) => ($body)(before)?,
            };
            PathObservationResult::$variant(value)
        }};
    }
    Ok(match operation {
        PathObservationOperation::Lstat => PathObservationResult::Lstat(match stat {
            Ok(stat) => PathOperationResult::Present(lstat_value(&stat)),
            Err(error) => io_result(error)?,
        }),
        PathObservationOperation::ReadLink => {
            with_stat!(ReadLink, PathNodeKind::Symlink, |before| read_link(
                root, path, &before, scope, active, credits
            ))
        }
        PathObservationOperation::DirectoryEntries => {
            with_stat!(DirectoryEntries, PathNodeKind::Directory, |before| {
                directory(root, path, &before, scope, active, credits)
            })
        }
        PathObservationOperation::FileBytes => {
            with_stat!(FileBytes, PathNodeKind::RegularFile, |before| file_bytes(
                root, path, &before, scope, active
            ))
        }
        PathObservationOperation::FileDigest => {
            with_stat!(FileDigest, PathNodeKind::RegularFile, |before| digest(
                root, path, &before, scope, active
            ))
        }
        _ => unreachable!("the complete batch was validated before IO"),
    })
}

fn read_link(
    root: &RootIo<'_>,
    path: &Path,
    before: &libc::stat,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    credits: &mut Vec<SourceBufferReservation>,
) -> Result<PathOperationResult<Arc<PathBuf>>, RootedObservationError> {
    let mut scratch = [0u8; PATH_LIMIT + 1];
    let bytes = match root.read_link(path, &mut scratch) {
        Ok(bytes) => bytes,
        Err(error) => {
            check(scope, active)?;
            return framed_io_result(error);
        }
    };
    check(scope, active)?;
    credits.push(reserve(
        scope,
        add(
            bytes.len(),
            std::mem::size_of::<PathBuf>() + 2 * std::mem::size_of::<usize>(),
        )?,
    )?);
    let target = Arc::new(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())));
    if !unchanged(
        before,
        &root.lstat(path).map_err(RootedObservationError::RootIo)?,
    ) {
        return Err(RootedObservationError::Unstable);
    }
    check(scope, active)?;
    Ok(PathOperationResult::Present(target))
}

fn directory(
    root: &RootIo<'_>,
    path: &Path,
    before: &libc::stat,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    credits: &mut Vec<SourceBufferReservation>,
) -> Result<PathOperationResult<PathDirectoryEntries>, RootedObservationError> {
    let running = || active() && scope.check().is_ok();
    let mut count = 0usize;
    let mut names = 0usize;
    let mut overflow = false;
    let first = root.read_directory(path, &running, |name, _| {
        count = count.checked_add(1).unwrap_or(usize::MAX);
        names = names
            .checked_add(name.as_bytes().len())
            .unwrap_or(usize::MAX);
        if count > MAX_DIRECTORY_ENTRIES || names > MAX_DIRECTORY_NAMES {
            overflow = true;
            return Err(io::Error::other("rooted directory capture bound"));
        }
        Ok(())
    });
    check(scope, active)?;
    if overflow {
        return Err(RootedObservationError::Bound);
    }
    if let Err(error) = first {
        return framed_io_result(error);
    }
    // Vec, Arc transfer and stable-sort scratch coexist. Names are moved, not
    // cloned, but their worst-case scratch/capacity peak is prepaid as well.
    let bytes = add(
        array::<PathDirectoryEntry>(count)?
            .checked_mul(4)
            .ok_or(RootedObservationError::Bound)?,
        add(
            names.checked_mul(4).ok_or(RootedObservationError::Bound)?,
            1024,
        )?,
    )?;
    credits.push(reserve(scope, bytes)?);
    let mut entries = vector(count)?;
    let mut seen_names = 0usize;
    let mut changed = false;
    let second = root.read_directory(path, &running, |name, stat| {
        let length = name.as_bytes().len();
        if entries.len() >= count || length > names.saturating_sub(seen_names) {
            changed = true;
            return Err(io::Error::other("rooted directory changed between passes"));
        }
        seen_names += length;
        let name = PathDirectoryName::new(std::ffi::OsString::from_vec(name.as_bytes().to_vec()))
            .map_err(|_| io::Error::other("invalid rooted directory name"))?;
        let entry_kind = match kind(stat) {
            PathNodeKind::RegularFile => PathDirectoryEntryKind::File,
            PathNodeKind::Directory => PathDirectoryEntryKind::Directory,
            PathNodeKind::Symlink => PathDirectoryEntryKind::Symlink,
            PathNodeKind::SpecialFile => PathDirectoryEntryKind::Unknown,
        };
        entries.push(PathDirectoryEntry::new(name, entry_kind));
        Ok(())
    });
    check(scope, active)?;
    if changed {
        return Err(RootedObservationError::Unstable);
    }
    if let Err(error) = second {
        return framed_io_result(error);
    }
    if entries.len() != count
        || seen_names != names
        || !unchanged(
            before,
            &root.lstat(path).map_err(RootedObservationError::RootIo)?,
        )
    {
        return Err(RootedObservationError::Unstable);
    }
    Ok(PathOperationResult::Present(PathDirectoryEntries::new(
        entries,
    )))
}

fn metadata_matches(stat: &libc::stat, metadata: &std::fs::Metadata) -> bool {
    stat.st_dev == metadata.dev()
        && stat.st_ino == metadata.ino()
        && stat.st_mode == metadata.mode()
        && stat.st_nlink == metadata.nlink()
        && stat.st_size >= 0
        && stat.st_size as u64 == metadata.size()
        && stat.st_mtime == metadata.mtime()
        && stat.st_mtime_nsec == metadata.mtime_nsec()
        && stat.st_ctime == metadata.ctime()
        && stat.st_ctime_nsec == metadata.ctime_nsec()
}

fn file_bytes(
    root: &RootIo<'_>,
    path: &Path,
    before: &libc::stat,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathOperationResult<Arc<[u8]>>, RootedObservationError> {
    root.admit_file_size(u64::try_from(before.st_size).map_err(|_| RootedObservationError::Bound)?)
        .map_err(RootedObservationError::RootIo)?;
    let input = match root.open_read_regular(path) {
        Ok(file) => file,
        Err(error) => {
            check(scope, active)?;
            return framed_io_result(error);
        }
    };
    check(scope, active)?;
    let expected = input.metadata().map_err(RootedObservationError::RootIo)?;
    if !metadata_matches(before, &expected) {
        return Err(RootedObservationError::Unstable);
    }
    let bytes = match read_open_regular_file_after_attempt(scope, input, &expected) {
        Ok(bytes) => bytes,
        Err(crate::runtime::dice::source_service::RegularFileError::Scope(error)) => {
            return Err(error.into());
        }
        Err(crate::runtime::dice::source_service::RegularFileError::Io { error, .. }) => {
            check(scope, active)?;
            return framed_io_result(error);
        }
        Err(_) => return Err(RootedObservationError::Unstable),
    };
    check(scope, active)?;
    if !unchanged(
        before,
        &root.lstat(path).map_err(RootedObservationError::RootIo)?,
    ) {
        return Err(RootedObservationError::Unstable);
    }
    Ok(PathOperationResult::Present(bytes))
}

fn digest(
    root: &RootIo<'_>,
    path: &Path,
    before: &libc::stat,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathOperationResult<FileContentDigest>, RootedObservationError> {
    let mut charge_error = None;
    let running = || active() && scope.check().is_ok();
    let read = root.read_file(path, &running, |chunk| {
        scope
            .charge(SourceCounter::DecodedBytes, chunk.len() as u64)
            .map_err(|error| {
                charge_error = Some(error);
                io::Error::other("rooted digest cumulative source limit")
            })
    });
    if let Some(error) = charge_error {
        return Err(error.into());
    }
    check(scope, active)?;
    let captured = match read {
        Ok(captured) => captured,
        Err(error) => return framed_io_result(error),
    };
    if !unchanged(before, &captured.metadata) {
        return Err(RootedObservationError::Unstable);
    }
    let digest = FileContentDigest::new(captured.digest, captured.metadata.st_size as u64)
        .ok_or(RootedObservationError::Bound)?;
    Ok(PathOperationResult::Present(digest))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod resolution_tests;
