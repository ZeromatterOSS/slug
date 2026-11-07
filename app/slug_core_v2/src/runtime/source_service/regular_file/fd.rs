//! Credited capture from an already-open readable file. No path lookup occurs.
use std::fs::File;
use std::fs::Metadata;
use std::io;
use std::os::unix::fs::FileExt;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;

use super::*;

/// Consumes a producer-credited readable descriptor. The rooted caller retains
/// its selected root/route owner and revalidates that association before exposing
/// these bytes. Neither a display path nor a proc alias grants authority here.
pub(in crate::runtime) fn read_open_regular_file(
    scope: &Arc<SourceServiceScope>,
    input: File,
    expected: &Metadata,
) -> Result<Arc<[u8]>, RegularFileError> {
    scope.charge(SourceCounter::SourceAttempts, 1)?;
    read_open_regular_file_after_attempt(scope, input, expected)
}

/// For an observation owner that charged SourceAttempts before opening its
/// rooted input, including failed opens. Skips only that already-issued charge;
/// all scope, byte, storage and identity checks remain in this shared path.
pub(in crate::runtime) fn read_open_regular_file_after_attempt(
    scope: &Arc<SourceServiceScope>,
    mut input: File,
    expected: &Metadata,
) -> Result<Arc<[u8]>, RegularFileError> {
    scope.check()?;
    if !expected.is_file() {
        return Err(RegularFileError::NonRegular {
            kind: if expected.is_dir() {
                PathNodeKind::Directory
            } else {
                PathNodeKind::SpecialFile
            },
        });
    }
    if expected.len() > HOST_FILE_OBJECT_BYTES as u64 {
        return Err(scope
            .poison(SourceServiceError::Limit(SourceCounter::DecodedBytes))
            .into());
    }
    let mut offset = 0u64;
    read_open_with(
        scope,
        &mut input,
        expected,
        ReadPolicy::Host,
        true,
        |_, _| {},
        |file, bytes| {
            let count = file.read_at(bytes, offset)?;
            offset = offset.checked_add(count as u64).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "regular-file read offset overflow",
                )
            })?;
            Ok(count)
        },
        || Ok(()),
    )
}

pub(super) fn identity(m: &Metadata) -> (u64, u64, u32, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

// Only the existing private Host/Registry seam injects hooks/read behavior.
// Rooted production callers always use the concrete positional File entry above.
#[allow(clippy::too_many_arguments)]
pub(super) fn read_open_with(
    scope: &Arc<SourceServiceScope>,
    input: &mut File,
    before: &Metadata,
    policy: ReadPolicy,
    check_links: bool,
    mut hook: impl FnMut(ReadPoint, &File),
    mut read: impl FnMut(&mut File, &mut [u8]) -> io::Result<usize>,
    validate_name: impl FnOnce() -> Result<(), RegularFileError>,
) -> Result<Arc<[u8]>, RegularFileError> {
    let reopened = checked_io(
        scope,
        policy,
        RegularFileStage::Stat,
        "scoped registry data stat failed",
        || input.metadata(),
    )?;
    if identity(&reopened) != identity(before)
        || (check_links && reopened.nlink() != before.nlink())
    {
        return Err(RegularFileError::Changed { before_read: true });
    }
    let capacity = if policy.is_host() {
        before.len() as usize
    } else {
        policy.maximum(scope)
    };
    let (mut buffer, _read_credit) = ReadCapture::new(scope, policy, capacity)?;
    let mut chunk = [0u8; CHUNK_BYTES];
    loop {
        hook(ReadPoint::BeforeRead, input);
        scope.check()?;
        // Registry keeps its original full chunk, including overflow rejection
        // before any charge. Host reads its pinned length plus one EOF probe.
        let remaining = capacity - buffer.len();
        let requested = if !policy.is_host() {
            CHUNK_BYTES
        } else if remaining == 0 {
            1
        } else {
            remaining.min(CHUNK_BYTES)
        };
        let result = read(input, &mut chunk[..requested]);
        if policy.is_host() || result.is_ok() {
            scope.check()?;
        }
        let count = match result {
            Err(e) if policy.is_host() && e.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            other => other.map_err(|error| RegularFileError::Io {
                stage: RegularFileStage::Read,
                error,
                message: "scoped registry local read failed",
            })?,
        };
        hook(ReadPoint::AfterRead, input);
        if policy.is_host() {
            scope.check()?;
        }
        if count == 0 {
            break;
        }
        if policy.is_host() && count > remaining {
            return Err(RegularFileError::Changed { before_read: false });
        }
        buffer.append(scope, &chunk[..count])?;
    }
    let after = checked_io(
        scope,
        policy,
        RegularFileStage::Stat,
        "scoped registry final stat failed",
        || input.metadata(),
    )?;
    if identity(&after) != identity(before)
        || (check_links && after.nlink() != before.nlink())
        || buffer.len() as u64 != before.len()
    {
        return Err(RegularFileError::Changed { before_read: false });
    }
    hook(ReadPoint::BeforeConversion, input);
    validate_name()?;
    let result = buffer.finish_with(scope, |_| {})?;
    if policy.is_host() {
        scope.check()?;
    }
    Ok(result)
}
