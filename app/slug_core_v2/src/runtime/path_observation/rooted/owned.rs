//! Stream verification against an existing owned body. This private kernel
//! grants no exemption: only the sealed carrier in the batch owner selects it.
use super::*;

fn framed(error: io::Error) -> RootedObservationError {
    if is_missing_error(&error)
        || matches!(
            error.raw_os_error(),
            Some(libc::ESTALE | libc::EXDEV | libc::ELOOP)
        )
        || matches!(
            error.kind(),
            io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput
        )
    {
        RootedObservationError::Unstable
    } else {
        RootedObservationError::RootIo(error)
    }
}

pub(super) fn verify(
    root: &RootIo<'_>,
    path: &Path,
    body: &Arc<[u8]>,
    mode: u16,
    operation: PathObservationOperation,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, RootedObservationError> {
    check(scope, active)?;
    if operation == PathObservationOperation::FileBytes
        && body.len() > crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES
    {
        return Err(scope
            .poison(SourceServiceError::Limit(SourceCounter::DecodedBytes))
            .into());
    }
    root.admit_file_size(body.len() as u64)
        .map_err(RootedObservationError::RootIo)?;
    let before = root.lstat(path).map_err(framed)?;
    check(scope, active)?;
    if kind(&before) != PathNodeKind::RegularFile
        || before.st_nlink != 1
        || before.st_size < 0
        || before.st_size as u64 != body.len() as u64
        || (before.st_mode & 0o7777) as u16 != mode
    {
        return Err(RootedObservationError::Unstable);
    }
    let mut offset = 0usize;
    let mut callback_error = None;
    let captured = root.read_file(path, &|| active() && scope.check().is_ok(), |chunk| {
        let verified = (|| {
            check(scope, active)?;
            // Every returned body keeps this independent monotonic history
            // debit even after the native State owner's credit retires.
            scope.charge(SourceCounter::DecodedBytes, chunk.len() as u64)?;
            let end = offset
                .checked_add(chunk.len())
                .filter(|end| *end <= body.len())
                .ok_or(RootedObservationError::Unstable)?;
            if body[offset..end] != *chunk {
                return Err(RootedObservationError::Unstable);
            }
            offset = end;
            Ok(())
        })();
        if let Err(error) = verified {
            callback_error = Some(error);
            return Err(io::Error::other("owned file verification refused"));
        }
        Ok(())
    });
    if let Some(error) = callback_error {
        return Err(error);
    }
    check(scope, active)?;
    let captured = captured.map_err(framed)?;
    if offset != body.len() || !unchanged(&before, &captured.metadata) {
        return Err(RootedObservationError::Unstable);
    }
    let result = match operation {
        PathObservationOperation::FileBytes => {
            PathObservationResult::FileBytes(PathOperationResult::Present(body.clone()))
        }
        PathObservationOperation::FileDigest => {
            PathObservationResult::FileDigest(PathOperationResult::Present(
                FileContentDigest::new(captured.digest, body.len() as u64)
                    .ok_or(RootedObservationError::Bound)?,
            ))
        }
        _ => unreachable!("batch owner selects only content operations"),
    };
    check(scope, active)?;
    Ok(result)
}
