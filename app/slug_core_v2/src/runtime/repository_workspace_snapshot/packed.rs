//! Covered snapshots build packed immutable bodies directly from actual FD chunks.
use super::*;

fn build_error(error: RepositoryWorkspaceBodyBuildError<PhysicalRootError>) -> PhysicalRootError {
    match error {
        RepositoryWorkspaceBodyBuildError::Admission(error) => error,
        RepositoryWorkspaceBodyBuildError::Body(
            slug_bzlmod_v2::RepositoryWorkspaceBodyError::Allocation
            | slug_bzlmod_v2::RepositoryWorkspaceBodyError::Storage,
        ) => PhysicalRootError::Resource,
        RepositoryWorkspaceBodyBuildError::Body(_) => PhysicalRootError::Snapshot,
    }
}
struct Construction {
    // Unpublished block payloads must retire before their allocation credit.
    builder: Option<RepositoryWorkspaceBodyBuilder>,
    pending: Option<PendingNativeBody>,
    _scratch: ResourceCredit,
}
fn append(
    construction: &mut Construction,
    bytes: &[u8],
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    check(scope, active)?;
    let pending = construction.pending.as_mut().unwrap();
    construction
        .builder
        .as_mut()
        .unwrap()
        .append(bytes, &mut |layout| {
            pending.admit_packed_arc(layout, active)
        })
        .map_err(build_error)?;
    check(scope, active)
}
pub(super) fn capture(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    candidates: &mut NativeBodyCandidates,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<RepositoryWorkspaceBody, PhysicalRootError> {
    let length = usize::try_from(framed.st_size).map_err(|_| PhysicalRootError::Snapshot)?;
    let pending = candidates.reserve_packed(length, active)?;
    let scratch = candidates.ledger().resources().reserve(
        RepositoryWorkspaceBodyBuilder::encoding_scratch_bytes(),
        0,
        0,
    )?;
    let builder =
        RepositoryWorkspaceBodyBuilder::new(length).map_err(|_| PhysicalRootError::Resource)?;
    let mut construction = Construction {
        builder: Some(builder),
        pending: Some(pending),
        _scratch: scratch,
    };
    // RootIo retains its own FD scratch across this callback. Keep the
    // assembler off the fixed Source-worker stack, under the existing 2MiB
    // construction allowance; no full logical file is materialized.
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(64 * 1024)
        .map_err(|_| PhysicalRootError::Resource)?;
    if buffer.capacity() > 64 * 1024 {
        return Err(PhysicalRootError::Resource);
    }
    buffer.resize(64 * 1024, 0);
    let mut filled = 0usize;
    let mut offset = 0usize;
    let mut callback_error = None;
    let captured = io.read_file_contents(path, &|| active() && scope.check().is_ok(), |chunk| {
        let result = (|| {
            check(scope, active)?;
            offset = offset
                .checked_add(chunk.len())
                .filter(|end| *end <= length)
                .ok_or(PhysicalRootError::Snapshot)?;
            let mut remaining = chunk;
            while !remaining.is_empty() {
                let count = remaining.len().min(buffer.len() - filled);
                buffer[filled..filled + count].copy_from_slice(&remaining[..count]);
                filled += count;
                remaining = &remaining[count..];
                if filled == buffer.len() {
                    append(&mut construction, &buffer, scope, active)?;
                    filled = 0;
                }
            }
            Ok::<_, PhysicalRootError>(())
        })();
        if let Err(error) = result {
            callback_error = Some(error);
            Err(io::Error::other("packed snapshot refused"))
        } else {
            Ok(())
        }
    });
    let completed = (|| {
        check(scope, active)?;
        if let Some(error) = callback_error {
            return Err(error);
        }
        let captured = captured?;
        if offset != length || !seal::unchanged(framed, &captured) {
            return Err(PhysicalRootError::Snapshot);
        }
        if filled != 0 {
            append(&mut construction, &buffer[..filled], scope, active)?;
        }
        Ok(())
    })();
    if let Err(error) = completed {
        let refusal = construction
            .pending
            .as_mut()
            .unwrap()
            .take_payload_refusal();
        // This buffer is paid by Construction's scratch credit too.
        drop(buffer);
        drop(construction);
        if let Some(refusal) = refusal {
            return body_recovery::capture_existing(
                io, path, framed, candidates, refusal, scope, active,
            );
        }
        return Err(error);
    }
    let body = construction
        .builder
        .take()
        .unwrap()
        .finish()
        .map_err(|_| PhysicalRootError::Snapshot)?;
    let pending = construction.pending.take().unwrap();
    let lease = candidates.finish(pending, body, active)?;
    Ok(lease.body().clone())
}
