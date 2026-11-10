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
pub(super) fn observed_equal(
    io: &RootIo<'_>,
    path: &Path,
    framed: &libc::stat,
    old: &RepositoryWorkspaceBody,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<bool, PhysicalRootError> {
    let mut offset = 0usize;
    let mut same = true;
    let running = || active() && scope.check().is_ok();
    let captured = io.read_file_contents(path, &running, |chunk| {
        let end = offset
            .checked_add(chunk.len())
            .ok_or_else(|| io::Error::other("workspace compare overflow"))?;
        if end > old.len() {
            return Err(io::Error::other("workspace comparison grew"));
        }
        // Complete the framed contents stream even after mismatch, preserving
        // every comparison, exact EOF and opened/named-frame validation.
        same &= compare_chunk(old, offset, chunk);
        offset = end;
        Ok(())
    });
    check(scope, active)?;
    let captured = captured?;
    if !seal::unchanged(framed, &captured) || offset != old.len() {
        return Err(PhysicalRootError::Snapshot);
    }
    Ok(same)
}

#[inline]
fn compare_chunk(old: &RepositoryWorkspaceBody, offset: usize, chunk: &[u8]) -> bool {
    #[cfg(test)]
    crate::runtime::repository_workspace_comparison_work_for_test::record_call(chunk.len());
    old.range_eq(offset, chunk)
}
