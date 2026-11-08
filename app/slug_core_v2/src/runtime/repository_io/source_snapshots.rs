//! Command-scoped immutable source rows in the materializer's sole registry.
//! Claims retain collision control through data lifetime, without mount authority.
use std::sync::Weak;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;
use slug_bzlmod_v2::RepositorySourceSnapshotReservation;

use super::reservations::NativeSelectionDomain;
use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::ResourceCredit;

fn refusal() -> RepositorySessionError {
    RepositorySessionError::NativeWorkspace("selected source snapshot association refused".into())
}

#[derive(Debug)]
pub(in crate::runtime) struct SourceSnapshotClaim {
    request: Arc<RepositorySourceSnapshotRequest>,
    display: NormalizedAbsolutePath,
    instance: PathObservationInstanceId,
    domain: Arc<NativeSelectionDomain>,
    _credit: ResourceCredit,
}
impl SourceSnapshotClaim {
    pub(in crate::runtime) fn display(&self) -> &NormalizedAbsolutePath {
        &self.display
    }
    pub(in crate::runtime) fn instance(&self) -> PathObservationInstanceId {
        self.instance
    }
    pub(in crate::runtime) fn belongs_to(&self, bank: &Arc<RepositoryRootResources>) -> bool {
        Arc::ptr_eq(self.domain.resources(), bank)
    }
    pub(in crate::runtime) fn matches_reservation(
        &self,
        reservation: &RepositorySourceSnapshotReservation,
    ) -> bool {
        self.request == *reservation.request()
            && self.display == *reservation.display_root()
            && self.instance == reservation.instance()
    }
}
#[derive(Debug)]
struct ClaimRow {
    candidate: [u8; 32],
    claim: Weak<SourceSnapshotClaim>,
    _credit: ResourceCredit,
}
#[derive(Debug, Default)]
pub(super) struct SourceClaims {
    rows: Vec<ClaimRow>,
    capacity_credit: Option<ResourceCredit>,
}
impl NativeSelectionDomain {
    fn reserve_source(
        self: &Arc<Self>,
        request: Arc<RepositorySourceSnapshotRequest>,
        instance: PathObservationInstanceId,
    ) -> Result<Arc<SourceSnapshotClaim>, RepositorySessionError> {
        // Spelling identifies a slot; full structural request equality remains
        // the semantic binding. Framing prevents concatenation ambiguity.
        let mut hash = Sha256::new();
        hash.update(b"slug-v2:selected-source-slot:v1\0");
        for value in [
            request.workspace().as_path().as_os_str().as_encoded_bytes(),
            request.repository().as_str().as_bytes(),
        ] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value);
        }
        let candidate: [u8; 32] = hash.finalize().into();
        let mut claims = self.source_claims.lock().map_err(|_| refusal())?;
        claims.rows.retain(|row| row.claim.strong_count() != 0);
        for row in &claims.rows {
            if row.candidate == candidate {
                if let Some(live) = row.claim.upgrade() {
                    if live.request.workspace() != request.workspace()
                        || live.request.repository() != request.repository()
                    {
                        return Err(refusal());
                    }
                }
            }
        }
        if claims.rows.len() == self.max {
            return Err(refusal());
        }
        let retained = request.retained_storage_bytes().map_err(|_| refusal())?;
        let credit = self
            .resources()
            .reserve(
                retained
                    .checked_add(std::mem::size_of::<SourceSnapshotClaim>() + 8192)
                    .ok_or_else(refusal)?,
                0,
                0,
            )
            .map_err(|_| refusal())?;
        let row_credit = self
            .resources()
            .reserve(
                std::mem::size_of::<ClaimRow>() + std::mem::size_of::<SourceSnapshotClaim>() + 128,
                0,
                0,
            )
            .map_err(|_| refusal())?;
        let SourceClaims {
            rows,
            capacity_credit,
        } = &mut *claims;
        reservations::reserve_capacity(rows, capacity_credit, self.resources(), self.max)?;
        // This prefix and General's /v1/ are structurally disjoint. Both use
        // this same selection domain and independently collision-check slots.
        let display = NormalizedAbsolutePath::new(format!(
            "/__slug_repositories__/s1/{}",
            hex::encode(candidate)
        ))
        .map_err(|_| refusal())?;
        let claim = Arc::new(SourceSnapshotClaim {
            request,
            display,
            instance,
            domain: self.clone(),
            _credit: credit,
        });
        rows.push(ClaimRow {
            candidate,
            claim: Arc::downgrade(&claim),
            _credit: row_credit,
        });
        Ok(claim)
    }
}

pub(in crate::runtime) struct ReservedSourceSnapshot {
    token: RepositorySessionToken,
    reservation: Arc<RepositorySourceSnapshotReservation>,
    claim: Arc<SourceSnapshotClaim>,
    _credit: ResourceCredit,
}

#[derive(Clone)]
pub(super) struct SourceSnapshotRow {
    pub(super) root: Arc<crate::runtime::repository_physical_root::source::RepositorySourceRoot>,
    _credit: Arc<ResourceCredit>,
}
impl SourceSnapshotRow {
    fn request(&self) -> &Arc<RepositorySourceSnapshotRequest> {
        self.root.reservation().request()
    }
}
impl ReservedSourceSnapshot {
    pub(in crate::runtime) fn reservation(&self) -> &Arc<RepositorySourceSnapshotReservation> {
        &self.reservation
    }
    pub(in crate::runtime) fn claim(&self) -> &Arc<SourceSnapshotClaim> {
        &self.claim
    }
}

impl RepositoryMaterializer {
    pub(in crate::runtime) fn reserve_source_snapshot(
        &self,
        token: RepositorySessionToken,
        request: Arc<RepositorySourceSnapshotRequest>,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<ReservedSourceSnapshot, RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        if request.workspace() != &self.workspace {
            return Err(RepositorySessionError::WrongWorkspace);
        }
        let state = self.state.lock().map_err(|_| refusal())?;
        let active = matching_validated_active(&state, token)?;
        let general = active.general.as_ref().ok_or_else(refusal)?;
        if request.lease() != general.lease() || !Arc::ptr_eq(scope, general.scope()) {
            return Err(refusal());
        }
        let domain = state.claim_domain.as_ref().ok_or_else(refusal)?;
        let instance = crate::runtime::native_identity::next_repository_instance()
            .map_err(|_| RepositorySessionError::InstanceExhausted)?;
        let claim = domain.reserve_source(request.clone(), instance)?;
        let bytes = request
            .retained_storage_bytes()
            .map_err(|_| refusal())?
            .checked_add(
                std::mem::size_of::<ReservedSourceSnapshot>()
                    + std::mem::size_of::<RepositorySourceSnapshotReservation>()
                    + 8192,
            )
            .ok_or_else(refusal)?;
        let credit = general
            .resources()
            .reserve(bytes, 0, 0)
            .map_err(|_| refusal())?;
        let reservation = Arc::new(
            RepositorySourceSnapshotReservation::new(request, claim.display.clone(), instance)
                .map_err(|_| refusal())?,
        );
        Ok(ReservedSourceSnapshot {
            token,
            reservation,
            claim,
            _credit: credit,
        })
    }

    pub(in crate::runtime) fn adopt_source_snapshot(
        &self,
        token: RepositorySessionToken,
        ticket: &ReservedSourceSnapshot,
        sealed: crate::runtime::repository_physical_root::source::SealedSourceSnapshot,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<Arc<slug_bzlmod_v2::RepositorySourceSnapshotState>, RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        if ticket.token != token
            || sealed.reservation() != ticket.reservation()
            || !ticket.claim.matches_reservation(sealed.reservation())
            || !sealed
                .root()
                .source_claim()
                .is_some_and(|claim| Arc::ptr_eq(&claim, &ticket.claim))
        {
            return Err(refusal());
        }
        sealed
            .root()
            .check_origin(scope, &|| self.session_is_active(token))
            .map_err(|_| refusal())?;
        let retained = sealed
            .reservation()
            .request()
            .retained_storage_bytes()
            .map_err(|_| refusal())?;
        let mut state = self.state.lock().map_err(|_| refusal())?;
        let active = matching_validated_active_mut(&mut state, token)?;
        let general = active.general.as_ref().ok_or_else(refusal)?;
        if !Arc::ptr_eq(scope, general.scope())
            || sealed.reservation().lease() != general.lease()
            || !ticket.claim.belongs_to(general.resources())
            || active
                .source_snapshots
                .iter()
                .any(|row| row.request().repository() == ticket.reservation.repository())
        {
            return Err(refusal());
        }
        scope
            .check()
            .map_err(RepositorySessionError::SourceService)?;
        let credit = general
            .resources()
            .reserve(
                retained
                    .checked_add(std::mem::size_of::<SourceSnapshotRow>() + 128)
                    .ok_or_else(refusal)?,
                0,
                0,
            )
            .map_err(|_| refusal())?;
        reservations::reserve_capacity(
            &mut active.source_snapshots,
            &mut active.source_snapshot_credit,
            general.resources(),
            slug_bzlmod_v2::REPOSITORY_WORKSPACE_MAX_READ_VIEWS,
        )?;
        let snapshot = sealed.state().clone();
        active.source_snapshots.push(SourceSnapshotRow {
            root: sealed.root().clone(),
            _credit: Arc::new(credit),
        });
        Ok(snapshot)
    }

    pub(super) fn source_row_current(
        &self,
        token: RepositorySessionToken,
        row: &SourceSnapshotRow,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<(), RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        row.root
            .check_origin(scope, &|| self.session_is_active(token))
            .map_err(|_| refusal())?;
        let state = self.state.lock().map_err(|_| refusal())?;
        let active = matching_validated_active(&state, token)?;
        if active.workspace_lease != row.root.reservation().lease()
            || !active
                .source_snapshots
                .iter()
                .any(|now| Arc::ptr_eq(&now.root, &row.root))
        {
            return Err(refusal());
        }
        Ok(())
    }

    /// Admission only: every Source, General and legacy batch validates before IO.
    pub(super) fn split_source_demands(
        &self,
        token: RepositorySessionToken,
        demands: Vec<PathObservationDemand>,
    ) -> Result<SourceObservationBatch, RepositorySessionError> {
        let scope = self.command_source_scope(token)?;
        let (rows, credit) = {
            let state = self.state.lock().map_err(|_| refusal())?;
            let active = matching_active(&state, token)?;
            if active.source_snapshots.is_empty() {
                return Ok(SourceObservationBatch {
                    legacy: demands,
                    rows: Vec::new(),
                    scope,
                    _credit: None,
                });
            }
            let general = active.general.as_ref().ok_or_else(refusal)?;
            let paths = demands.iter().try_fold(0usize, |n, d| {
                n.checked_add(d.path().retained_storage_bytes().ok_or_else(refusal)?)
                    .ok_or_else(refusal)
            })?;
            if paths > 4 * 1024 * 1024 {
                return Err(refusal());
            }
            let bytes = demands
                .len()
                .checked_mul(
                    8 * std::mem::size_of::<PathObservationDemand>()
                        + 8 * std::mem::size_of::<Arc<PathObservationResult>>()
                        + 512,
                )
                .and_then(|n| n.checked_add(paths.checked_mul(8)?))
                .and_then(|n| {
                    n.checked_add(
                        active.source_snapshots.len()
                            * (4 * std::mem::size_of::<SourceSnapshotRow>() + 256)
                            + 4096,
                    )
                })
                .ok_or_else(refusal)?;
            let credit = general
                .resources()
                .reserve(bytes, 0, 0)
                .map_err(|_| refusal())?;
            (active.source_snapshots.clone(), credit)
        };
        let current_scope = scope.as_ref().ok_or_else(refusal)?;
        let mut rows = rows
            .into_iter()
            .map(|row| (row, Vec::new()))
            .collect::<Vec<_>>();
        let mut legacy = Vec::new();
        for demand in demands {
            if let Some((_, batch)) = rows
                .iter_mut()
                .find(|(row, _)| demand.namespace() == row.root.anchor().namespace())
            {
                batch.push(demand);
            } else {
                legacy.push(demand);
            }
        }
        rows.retain(|(_, batch)| !batch.is_empty());
        for (row, batch) in &rows {
            self.source_row_current(token, row, current_scope)?;
            crate::runtime::path_observation::rooted::validate_rooted_demands(
                row.root.anchor(),
                batch,
            )
            .map_err(|_| refusal())?;
        }
        Ok(SourceObservationBatch {
            legacy,
            rows,
            scope,
            _credit: Some(credit),
        })
    }
}

pub(super) struct SourceObservationBatch {
    pub(super) legacy: Vec<PathObservationDemand>,
    rows: Vec<(SourceSnapshotRow, Vec<PathObservationDemand>)>,
    scope: Option<Arc<SourceServiceScope>>,
    // Last: all temporary requests, epochs and construction peaks retire first.
    _credit: Option<ResourceCredit>,
}
impl SourceObservationBatch {
    pub(super) fn observe(
        &self,
        actor: &RepositoryMaterializer,
        token: RepositorySessionToken,
        previous: PathObservationEpoch,
    ) -> Result<PathObservationEpoch, RepositorySessionError> {
        if self.rows.is_empty() {
            return Ok(previous);
        }
        let scope = self.scope.as_ref().ok_or_else(refusal)?;
        for (row, _) in &self.rows {
            actor.source_row_current(token, row, scope)?;
        }
        let mut epochs = Vec::with_capacity(self.rows.len() + 1);
        epochs.push(previous);
        for (row, batch) in &self.rows {
            epochs.push(
                row.root
                    .observe(batch, scope, &|| actor.session_is_active(token))
                    .map_err(|_| refusal())?,
            );
        }
        let result = PathObservationEpoch::from_shared(epochs.iter().flat_map(|epoch| {
            epoch
                .observations()
                .iter()
                .map(|(d, r)| (d.clone(), r.clone()))
        }))
        .map_err(|_| refusal())?;
        for (row, _) in &self.rows {
            actor.source_row_current(token, row, scope)?;
        }
        Ok(result)
    }
}
