//! Stream verification against an existing owned body. This private kernel
//! grants no exemption: only the sealed carrier in the batch owner selects it.
use super::*;

/// Private construction requires a complete verified file payload read. The
/// rooted owner separately proves the backing that can retain the witness.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct ArchiveDigestReceipt {
    digest: FileContentDigest,
    frame: libc::stat,
}

impl ArchiveDigestReceipt {
    pub(in crate::runtime) fn digest(&self) -> FileContentDigest {
        self.digest
    }

    pub(in crate::runtime) fn matches_file(
        &self,
        digest: FileContentDigest,
        frame: &libc::stat,
    ) -> bool {
        self.digest == digest && unchanged(&self.frame, frame)
    }
}

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

pub(super) fn verify_with_frame(
    root: &RootIo<'_>,
    path: &Path,
    body: &slug_bzlmod_v2::RepositoryWorkspaceBody,
    mode: u16,
    operation: PathObservationOperation,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(PathObservationResult, Option<ArchiveDigestReceipt>), RootedObservationError> {
    check(scope, active)?;
    if operation == PathObservationOperation::FileBytes
        && body.len() > crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES
    {
        return Err(scope
            .poison(SourceServiceError::Limit(SourceCounter::DecodedBytes))
            .into());
    }
    let _decode = if body.raw_arc().is_none() {
        Some(reserve(
            scope,
            slug_bzlmod_v2::RepositoryWorkspaceBody::decode_scratch_bytes(),
        )?)
    } else {
        None
    };
    struct Materialized {
        bytes: Option<Arc<[std::mem::MaybeUninit<u8>]>>,
        _credit: Option<SourceBufferReservation>,
    }
    let mut materialized = Materialized {
        bytes: None,
        _credit: None,
    };
    if operation == PathObservationOperation::FileBytes && body.raw_arc().is_none() {
        // Encoded history remains live; separately prepay the actual decoded Arc.
        materialized._credit = Some(reserve(scope, array::<u8>(body.len())?)?);
        materialized.bytes = Some(Arc::<[u8]>::new_uninit_slice(body.len()));
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
    let captured = body.with_range_comparison(|compare| {
        let running = || active() && scope.check().is_ok();
        let capture = |chunk: &[u8]| {
            let verified = (|| {
                check(scope, active)?;
                // Every returned body keeps this independent monotonic history
                // debit even after the native State owner's credit retires.
                scope.charge(SourceCounter::DecodedBytes, chunk.len() as u64)?;
                let end = offset
                    .checked_add(chunk.len())
                    .filter(|end| *end <= body.len())
                    .ok_or(RootedObservationError::Unstable)?;
                if !compare(offset, chunk) {
                    return Err(RootedObservationError::Unstable);
                }
                if let Some(output) = materialized.bytes.as_mut() {
                    let slots = Arc::get_mut(output).expect("unpublished returned Arc is unique");
                    for (slot, byte) in slots[offset..end].iter_mut().zip(chunk) {
                        slot.write(*byte);
                    }
                }
                offset = end;
                Ok(())
            })();
            if let Err(error) = verified {
                callback_error = Some(error);
                return Err(io::Error::other("owned file verification refused"));
            }
            Ok(())
        };
        match operation {
            PathObservationOperation::FileBytes => root
                .read_file_contents(path, &running, capture)
                .map(|metadata| (metadata, None)),
            PathObservationOperation::FileDigest => root
                .read_file(path, &running, capture)
                .map(|capture| (capture.metadata, Some(capture.digest))),
            _ => unreachable!("batch owner selects only content operations"),
        }
    });
    if let Some(error) = callback_error {
        return Err(error);
    }
    check(scope, active)?;
    let (metadata, digest) = captured.map_err(framed)?;
    if offset != body.len() || !unchanged(&before, &metadata) {
        return Err(RootedObservationError::Unstable);
    }
    let result = match operation {
        PathObservationOperation::FileBytes => {
            let bytes = match body.raw_arc() {
                Some(raw) => raw.clone(),
                None => {
                    // All admitted slots were filled from freshly verified FD
                    // bytes; RootIo authenticated exact EOF and original frames.
                    unsafe { materialized.bytes.take().unwrap().assume_init() }
                }
            };
            (
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)),
                None,
            )
        }
        PathObservationOperation::FileDigest => {
            let digest = FileContentDigest::new(
                digest.expect("digest capture yields SHA-256"),
                body.len() as u64,
            )
            .ok_or(RootedObservationError::Bound)?;
            let receipt = ArchiveDigestReceipt {
                digest,
                frame: metadata,
            };
            (
                PathObservationResult::FileDigest(PathOperationResult::Present(digest)),
                Some(receipt),
            )
        }
        _ => unreachable!("batch owner selects only content operations"),
    };
    check(scope, active)?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn verify(
    root: &RootIo<'_>,
    path: &Path,
    body: &slug_bzlmod_v2::RepositoryWorkspaceBody,
    mode: u16,
    operation: PathObservationOperation,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, RootedObservationError> {
    verify_with_frame(root, path, body, mode, operation, scope, active).map(|(result, _)| result)
}

pub(super) fn verify_prior_digest(
    root: &RootIo<'_>,
    path: &Path,
    receipt: ArchiveDigestReceipt,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, RootedObservationError> {
    verify_prior_frame(root, path, &receipt.frame, scope, active)?;
    Ok(PathObservationResult::FileDigest(
        PathOperationResult::Present(receipt.digest),
    ))
}

pub(super) fn verify_prior_frame(
    root: &RootIo<'_>,
    path: &Path,
    frame: &libc::stat,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), RootedObservationError> {
    check(scope, active)?;
    let verified = root.verify_regular_frame(path, frame, &|| active() && scope.check().is_ok());
    check(scope, active)?;
    verified.map_err(framed)
}
