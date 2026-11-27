//! Refused fresh payloads may reuse only a fully observed same-origin lease.
//! Candidate discovery supplies coverage, never filesystem or semantic authority.
use super::super::body_coverage::NativeBodyRefusal;
use super::*;

pub(super) fn capture_existing(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    candidates: &mut NativeBodyCandidates,
    refusal: NativeBodyRefusal,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<RepositoryWorkspaceBody, PhysicalRootError> {
    // This is reached only after the exact fresh-payload bank limit refuses.
    // New deadline/cancellation/IO/frame errors outrank that original Resource.
    check(scope, active)?;
    if let Some(mut search) = candidates.recovery(refusal, active)? {
        // next() prepays control and debits the full length against finite work
        // windows before returning a lease. No ledger lock survives this call.
        while let Some(lease) = search.next(active)? {
            if observed_equal(io, path, framed, lease.body(), scope, active)? {
                // Keep cursor control credit live through the exact claim and
                // retirement of both temporary lease handles.
                let known = candidates.retain_claimed(lease.body(), &lease, active)?;
                return Ok(known.body().clone());
            }
        }
    }
    Err(PhysicalRootError::Resource)
}

/// Shared with ordinary same-path reuse. The snapshot scratch owner covers the
/// existing RootIo buffer; this helper adds no allocation or discovery authority.
pub(in crate::runtime::repository_physical_root) fn observed_equal(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    old: &RepositoryWorkspaceBody,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<bool, PhysicalRootError> {
    let mut same = true;
    old.with_range_comparison(|compare| {
        read_framed_contents(io, path, framed, old.len(), scope, active, |offset, chunk| {
            #[cfg(all(test, feature = "native-probe-observer"))]
            if !offset.is_multiple_of(slug_bzlmod_v2::REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES) {
                crate::runtime::probe_observer::snapshot_work::record_unaligned_comparison_chunk();
            }
            // Complete the framed contents stream even after mismatch, preserving
            // every comparison, exact EOF and opened/named-frame validation.
            same &= compare_chunk(compare, offset, chunk);
        })
    })?;
    Ok(same)
}

pub(super) fn verify_framed_contents(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    length: usize,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    read_framed_contents(io, path, framed, length, scope, active, |_, _| {})
}

fn read_framed_contents(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    length: usize,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
    mut consume: impl FnMut(usize, &[u8]),
) -> Result<(), PhysicalRootError> {
    let mut offset = 0usize;
    let running = || active() && scope.check().is_ok();
    let captured = io.read_file_contents(path, &running, |chunk| {
        let end = offset
            .checked_add(chunk.len())
            .ok_or_else(|| io::Error::other("workspace compare overflow"))?;
        if end > length {
            return Err(io::Error::other("workspace comparison grew"));
        }
        consume(offset, chunk);
        offset = end;
        Ok(())
    });
    check(scope, active)?;
    let captured = captured?;
    if !seal::unchanged(framed, &captured) || offset != length {
        return Err(PhysicalRootError::Snapshot);
    }
    Ok(())
}

#[inline]
fn compare_chunk(
    compare: &mut dyn FnMut(usize, &[u8]) -> bool,
    offset: usize,
    chunk: &[u8],
) -> bool {
    #[cfg(test)]
    crate::runtime::repository_workspace_comparison_work_for_test::record_call(chunk.len());
    compare(offset, chunk)
}
