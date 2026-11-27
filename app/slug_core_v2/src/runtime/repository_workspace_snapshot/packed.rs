//! Covered snapshots build packed immutable bodies directly from actual FD chunks.
use slug_bzlmod_v2::REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES as BLOCK_BYTES;

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

fn assemble_blocks<E>(
    partial: &mut Vec<u8>,
    mut chunk: &[u8],
    mut append: impl FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), E> {
    while !chunk.is_empty() {
        if partial.is_empty() && chunk.len() >= BLOCK_BYTES {
            let (block, rest) = chunk.split_at(BLOCK_BYTES);
            append(block)?;
            chunk = rest;
            continue;
        }
        let count = chunk.len().min(BLOCK_BYTES - partial.len());
        let (fragment, rest) = chunk.split_at(count);
        partial.extend_from_slice(fragment);
        chunk = rest;
        if partial.len() == BLOCK_BYTES {
            append(partial)?;
            partial.clear();
        }
    }
    Ok(())
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
    // fragment buffer off the fixed Source-worker stack, under the existing
    // 2MiB construction allowance. Initialize only bytes that need assembly.
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(BLOCK_BYTES)
        .map_err(|_| PhysicalRootError::Resource)?;
    if buffer.capacity() > BLOCK_BYTES {
        return Err(PhysicalRootError::Resource);
    }
    let mut offset = 0usize;
    let mut callback_error = None;
    let captured = io.read_file_contents(path, &|| active() && scope.check().is_ok(), |chunk| {
        let result = (|| {
            check(scope, active)?;
            offset = offset
                .checked_add(chunk.len())
                .filter(|end| *end <= length)
                .ok_or(PhysicalRootError::Snapshot)?;
            assemble_blocks(&mut buffer, chunk, |block| {
                append(&mut construction, block, scope, active)
            })
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
        if !buffer.is_empty() {
            append(&mut construction, &buffer, scope, active)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_block_assembly_forwards_aligned_input_and_assembles_fragments() {
        let payload = (0..4 * BLOCK_BYTES + 37)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let mut buffer = Vec::with_capacity(BLOCK_BYTES);
        let mut seen = 0;
        let mut forwarded = 0;
        let mut position = 0;
        for size in [
            BLOCK_BYTES,
            BLOCK_BYTES - 3,
            BLOCK_BYTES,
            3,
            BLOCK_BYTES,
            37,
        ] {
            let chunk = &payload[position..position + size];
            assemble_blocks(&mut buffer, chunk, |block| {
                assert_eq!(block, &payload[seen..seen + BLOCK_BYTES]);
                if std::ptr::eq(block.as_ptr(), payload[seen..].as_ptr()) {
                    forwarded += block.len();
                }
                seen += block.len();
                Ok::<_, ()>(())
            })
            .unwrap();
            position += size;
        }
        assert_eq!(seen, 4 * BLOCK_BYTES);
        assert_eq!(buffer, payload[seen..]);
        assert_eq!(forwarded, 2 * BLOCK_BYTES);
    }
}
