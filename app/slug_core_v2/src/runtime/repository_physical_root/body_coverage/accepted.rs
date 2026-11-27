//! Compare final data from an accepted generation with a fresh rooted file.
//! Only the credited immutable body can survive in the new generation.
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceNode;

use super::*;

const COMPARISON_BYTES: usize = 1024 * 1024 * 1024;

/// Internal snapshot hint. It exposes no reader, path or origin-scope method.
/// The materializer obtains it from its previous accepted selection.
pub(in crate::runtime) struct AcceptedFinalBodySource {
    prior: Arc<RepositoryPhysicalRoot>,
    current: Arc<RepositoryWorkspaceReservation>,
}

impl RepositoryPhysicalRoot {
    pub(in crate::runtime) fn accepted_final_body_source(
        self: Arc<Self>,
        current: &Arc<RepositoryWorkspaceReservation>,
        resources: &Arc<RepositoryRootResources>,
    ) -> Option<AcceptedFinalBodySource> {
        let prior = self.reservation();
        (self._body_coverage.is_some()
            && Arc::ptr_eq(&self.resources, resources)
            && prior.workspace() == current.workspace()
            && prior.repository() == current.repository()
            && prior.ordinal() == current.ordinal()
            && prior.owner() == current.owner()
            && prior.instance() != current.instance()
            && prior.lease() != current.lease())
        .then(|| AcceptedFinalBodySource {
            prior: self,
            current: current.clone(),
        })
    }
}

pub(in crate::runtime::repository_physical_root) struct AcceptedBodyComparison<'a> {
    source: &'a AcceptedFinalBodySource,
    compared: &'a mut usize,
}

impl AcceptedFinalBodySource {
    pub(in crate::runtime::repository_physical_root) fn compare_with<'a>(
        &'a self,
        current: &Arc<RepositoryWorkspaceReservation>,
        compared: &'a mut usize,
    ) -> Result<AcceptedBodyComparison<'a>, PhysicalRootError> {
        if self.current.as_ref() != current.as_ref() {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(AcceptedBodyComparison {
            source: self,
            compared,
        })
    }
}

impl AcceptedBodyComparison<'_> {
    pub(in crate::runtime::repository_physical_root) fn capture(
        &mut self,
        candidates: &mut NativeBodyCandidates,
        io: &RootIo<'_>,
        path: &Path,
        framed: &libc::stat,
        length: usize,
        local: Option<&RepositoryWorkspaceBody>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<RepositoryWorkspaceBody>, PhysicalRootError> {
        let prior = &self.source.prior;
        let Some(entry) = path.to_str().and_then(|path| prior.state().entry(path)) else {
            return Ok(None);
        };
        let RepositoryWorkspaceNode::File(body) = entry.node() else {
            return Ok(None);
        };
        if body.len() != length || local.is_some_and(|local| local.ptr_eq(body)) {
            return Ok(None);
        }
        let Some(next) = self
            .compared
            .checked_add(length)
            .filter(|n| *n <= COMPARISON_BYTES)
        else {
            return Ok(None);
        };
        // Charge the complete candidate length before reading, even on a miss.
        *self.compared = next;
        candidates.ledger.origin.check(active)?;
        let lease = prior
            ._body_coverage
            .as_ref()
            .and_then(|set| set.storage.bodies.get(&key(body)))
            .ok_or(PhysicalRootError::Reservation)?;
        if !body.ptr_eq(lease.body())
            || !candidates.ledger.origin.resources.owns_body_block(&lease.0)
        {
            return Err(PhysicalRootError::Reservation);
        }
        if !snapshot::body_recovery::observed_equal(io, path, framed, body, scope, active)? {
            return Ok(None);
        }
        candidates.ledger.origin.check(active)?;
        candidates.storage.insert(lease.clone())?;
        Ok(Some(lease.body().clone()))
    }
}
