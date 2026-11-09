//! Native body-coverage joins. These allocation witnesses are independent of
//! full semantic equality, physical verification and external read-view custody.
use slug_bzlmod_v2::RepositoryWorkspaceStoragePartition;

use super::*;

pub(super) fn partition_bytes(
    storage: RepositoryWorkspaceStoragePartition,
    sources_covered: bool,
) -> Result<usize, PhysicalRootError> {
    if sources_covered {
        Ok(storage.owned_bytes())
    } else {
        storage.total_bytes().map_err(PhysicalRootError::Protocol)
    }
}

impl RepositoryWorkspaceCommand {
    pub(super) fn body_ledger_at(
        &self,
        slot: usize,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<&Arc<NativeBodyLedger>, PhysicalRootError> {
        let root = self.roots.get(slot).ok_or(PhysicalRootError::Reservation)?;
        root.body_ledger
            .check_origin(&self.resources, scope, &root.reservation, active)?;
        Ok(&root.body_ledger)
    }

    pub(super) fn request_body_candidates(
        &self,
        request: &RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyCandidates, PhysicalRootError> {
        let slot = self.root_slot(request.reservation())?;
        let mut bodies = self.body_ledger_at(slot, scope, active)?.candidates()?;
        request.try_visit_body_allocations(&mut |body| bodies.retain(body, active).map(|_| ()))?;
        Ok(bodies)
    }

    pub(super) fn materialization_body_candidates(
        &self,
        request: &slug_bzlmod_v2::RepositoryMaterializationRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyCandidates, PhysicalRootError> {
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
            &request.kind
        else {
            return Err(PhysicalRootError::Reservation);
        };
        let slot = self.root_slot(realization.reservation())?;
        let mut bodies = self.body_ledger_at(slot, scope, active)?.candidates()?;
        realization
            .try_visit_body_allocations(&mut |body| bodies.retain(body, active).map(|_| ()))?;
        Ok(bodies)
    }

    pub(super) fn check_transcript_body_coverage(
        &self,
        transcript: &slug_bzlmod_v2::RepositoryWorkspaceTranscript,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        // A sealed root no longer has mutable progress, but its transcript is
        // still retained by the epoch. The data ledger outlives that payload;
        // this cost-only lookup must not demand or mint mutation authority.
        let slot = self
            .roots
            .iter()
            .position(|root| root.reservation.as_ref() == transcript.reservation().as_ref())
            .ok_or(PhysicalRootError::Reservation)?;
        let ledger = self.body_ledger_at(slot, scope, active)?;
        transcript.try_visit_body_allocations(&mut |body| {
            ledger
                .lookup(body, active)?
                .ok_or(PhysicalRootError::Reservation)?;
            Ok(())
        })
    }

    pub(super) fn check_epoch_body_coverage(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        for transcript in self
            .epoch()
            .transcripts()
            .ok_or(PhysicalRootError::Reservation)?
        {
            self.check_transcript_body_coverage(transcript, scope, active)?;
        }
        Ok(())
    }
}
