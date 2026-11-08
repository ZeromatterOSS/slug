//! Native selection/mount-domain spelling claims. The only registry is the
//! existing materializer actor; weak rows keep their control allocation paid.
use std::sync::Weak;

use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;

use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::ResourceCredit;
use crate::runtime::repository_physical_root::SealedWorkspaceMaterialization;

fn refusal() -> RepositorySessionError {
    RepositorySessionError::NativeWorkspace("native workspace reservation refused".into())
}
fn plus(a: usize, b: usize) -> Result<usize, RepositorySessionError> {
    a.checked_add(b).ok_or_else(refusal)
}
fn arc_size<T>() -> usize {
    std::mem::size_of::<T>() + 2 * std::mem::size_of::<usize>() + 64
}
#[derive(Debug)]
pub(in crate::runtime) struct SourceRootClaim {
    key: Arc<RepositoryWorkspaceStableKey>,
    display: NormalizedAbsolutePath,
    instance: PathObservationInstanceId,
    domain: Arc<NativeSelectionDomain>,
    _credit: ResourceCredit,
}
impl SourceRootClaim {
    pub(in crate::runtime) fn key(&self) -> &Arc<RepositoryWorkspaceStableKey> {
        &self.key
    }
    pub(in crate::runtime) fn display(&self) -> &NormalizedAbsolutePath {
        &self.display
    }
    pub(in crate::runtime) fn instance(&self) -> PathObservationInstanceId {
        self.instance
    }
    pub(in crate::runtime) fn belongs_to(&self, resources: &Arc<RepositoryRootResources>) -> bool {
        Arc::ptr_eq(&self.domain.resources, resources)
    }
    pub(in crate::runtime) fn same_domain(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.domain, &other.domain)
    }
    pub(in crate::runtime) fn matches_reservation(
        &self,
        reservation: &RepositoryWorkspaceReservation,
    ) -> bool {
        self.key.workspace() == reservation.workspace()
            && self.key.owner() == reservation.owner()
            && self.key.repository() == reservation.repository()
            && self.key.ordinal() == reservation.ordinal()
            && self.display == *reservation.display_root()
            && self.instance == reservation.instance()
    }
}
#[derive(Debug)]
struct ClaimRow {
    candidate: [u8; 32],
    claim: Weak<SourceRootClaim>,
    _credit: ResourceCredit,
}
#[derive(Debug, Default)]
struct Claims {
    rows: Vec<ClaimRow>,
    capacity_credit: Option<ResourceCredit>,
}
#[derive(Debug)]
pub(in crate::runtime) struct NativeSelectionDomain {
    resources: Arc<RepositoryRootResources>,
    pub(super) max: usize,
    claims: Mutex<Claims>,
    pub(super) source_claims: Mutex<super::source_snapshots::SourceClaims>,
    _credit: ResourceCredit,
}
impl NativeSelectionDomain {
    pub(super) fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.resources
    }
    pub(super) fn reserve_observation(
        &self,
        bytes: usize,
    ) -> Result<ResourceCredit, RepositorySessionError> {
        self.resources.reserve(bytes, 0, 0).map_err(|_| refusal())
    }
    fn new(
        resources: Arc<RepositoryRootResources>,
        max: usize,
    ) -> Result<Arc<Self>, RepositorySessionError> {
        let credit = resources
            .reserve(arc_size::<Self>(), 0, 0)
            .map_err(|_| refusal())?;
        Ok(Arc::new(Self {
            resources,
            max,
            claims: Mutex::new(Claims::default()),
            source_claims: Mutex::new(super::source_snapshots::SourceClaims::default()),
            _credit: credit,
        }))
    }
    fn reserve(
        self: &Arc<Self>,
        key: Arc<RepositoryWorkspaceStableKey>,
        instance: PathObservationInstanceId,
    ) -> Result<Arc<SourceRootClaim>, RepositorySessionError> {
        self.reserve_candidate(key.clone(), instance, key.candidate())
    }
    fn reserve_candidate(
        self: &Arc<Self>,
        key: Arc<RepositoryWorkspaceStableKey>,
        instance: PathObservationInstanceId,
        candidate: [u8; 32],
    ) -> Result<Arc<SourceRootClaim>, RepositorySessionError> {
        let mut claims = self.claims.lock().map_err(|_| refusal())?;
        claims.rows.retain(|row| row.claim.strong_count() != 0);
        for row in &claims.rows {
            if row.candidate == candidate {
                if let Some(live) = row.claim.upgrade() {
                    if live.key != key {
                        return Err(refusal());
                    }
                    // Equal spelling may coexist as separate data generations.
                    // Actual mounted sets must separately refuse overlays.
                }
            }
        }
        if claims.rows.len() >= self.max {
            return Err(refusal());
        }
        let retained = key.retained_storage_bytes().map_err(|_| refusal())?;
        let credit = self
            .resources
            .reserve(plus(retained, arc_size::<SourceRootClaim>() + 4096)?, 0, 0)
            .map_err(|_| refusal())?;
        let row_credit = self
            .resources
            .reserve(
                arc_size::<SourceRootClaim>() + std::mem::size_of::<ClaimRow>(),
                0,
                0,
            )
            .map_err(|_| refusal())?;
        let Claims {
            rows,
            capacity_credit,
        } = &mut *claims;
        reserve_capacity(rows, capacity_credit, &self.resources, self.max)?;
        let mut name = String::with_capacity(128);
        name.push_str("/__slug_repositories__/v1/");
        for byte in candidate {
            use std::fmt::Write;
            write!(&mut name, "{byte:02x}").map_err(|_| refusal())?;
        }
        let display = NormalizedAbsolutePath::new(name).map_err(|_| refusal())?;
        let claim = Arc::new(SourceRootClaim {
            key,
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
pub(super) fn reserve_capacity<T>(
    rows: &mut Vec<T>,
    credit: &mut Option<ResourceCredit>,
    resources: &Arc<RepositoryRootResources>,
    max: usize,
) -> Result<(), RepositorySessionError> {
    if rows.len() < rows.capacity() {
        return Ok(());
    }
    let capacity = rows
        .capacity()
        .checked_mul(2)
        .ok_or_else(refusal)?
        .max(4)
        .min(max);
    if capacity <= rows.len() {
        return Err(refusal());
    }
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| n.checked_add(64))
        .ok_or_else(refusal)?;
    let next = resources.reserve(bytes, 0, 0).map_err(|_| refusal())?;
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(capacity)
        .map_err(|_| refusal())?;
    if replacement.capacity() > capacity {
        return Err(refusal());
    }
    replacement.append(rows);
    let previous = std::mem::replace(rows, replacement);
    drop(previous);
    *credit = Some(next);
    Ok(())
}
struct ReservationSlot {
    reservation: Arc<RepositoryWorkspaceReservation>,
    claim: Weak<SourceRootClaim>,
    adopted: bool,
    _credit: ResourceCredit,
}
pub(super) struct GeneralWorkspaceSession {
    lease: u64,
    limits: RepositoryWorkspaceRetentionLimits,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    slots: Vec<ReservationSlot>,
    capacity_credit: Option<ResourceCredit>,
    entry_credit: Option<ResourceCredit>,
    root_credit: Option<ResourceCredit>,
    _credit: ResourceCredit,
}
impl GeneralWorkspaceSession {
    pub(super) fn lease(&self) -> u64 {
        self.lease
    }
    pub(super) fn resources(&self) -> &Arc<RepositoryRootResources> {
        &self.resources
    }
    pub(super) fn scope(&self) -> &Arc<SourceServiceScope> {
        &self.scope
    }
}
pub(in crate::runtime) struct ReservedRepositoryWorkspace {
    token: RepositorySessionToken,
    reservation: Arc<RepositoryWorkspaceReservation>,
    claim: Arc<SourceRootClaim>,
    _credit: ResourceCredit,
}
impl ReservedRepositoryWorkspace {
    pub(in crate::runtime) fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub(in crate::runtime) fn repo_spec(&self) -> &Arc<RepoSpec> {
        self.reservation.expected_repo_spec()
    }
    pub(in crate::runtime) fn claim(&self) -> &Arc<SourceRootClaim> {
        &self.claim
    }
}
impl RepositoryMaterializer {
    pub(in crate::runtime) fn begin_general_workspace(
        &self,
        token: RepositorySessionToken,
        resources: Arc<RepositoryRootResources>,
        limits: RepositoryWorkspaceRetentionLimits,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<u64, RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        if self.source_scope.is_none()
            && !scope
                .command_resources()
                .is_some_and(|bank| Arc::ptr_eq(bank, &resources))
        {
            return Err(refusal());
        }
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        if let Some(general) = &active.general {
            if !Arc::ptr_eq(&general.resources, &resources)
                || !Arc::ptr_eq(&general.scope, scope)
                || general.limits != limits
            {
                return Err(refusal());
            }
            return Ok(general.lease);
        }
        if let Some(domain) = &state.claim_domain {
            if !Arc::ptr_eq(&domain.resources, &resources)
                || domain.max != limits.max_state_versions()
            {
                return Err(refusal());
            }
        }
        let credit = resources
            .reserve(std::mem::size_of::<GeneralWorkspaceSession>(), 0, 0)
            .map_err(|_| refusal())?;
        let domain = match &state.claim_domain {
            Some(domain) => domain.clone(),
            None => NativeSelectionDomain::new(resources.clone(), limits.max_state_versions())?,
        };
        let lease = active.workspace_lease;
        state.claim_domain = Some(domain);
        matching_validated_active_mut(&mut state, token)?.general = Some(GeneralWorkspaceSession {
            lease,
            limits,
            resources,
            scope: scope.clone(),
            slots: Vec::new(),
            capacity_credit: None,
            entry_credit: None,
            root_credit: None,
            _credit: credit,
        });
        Ok(lease)
    }
    pub(in crate::runtime) fn reserve_workspace(
        &self,
        token: RepositorySessionToken,
        key: Arc<RepositoryWorkspaceStableKey>,
        repo_spec: Arc<RepoSpec>,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<ReservedRepositoryWorkspace, RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        if key.workspace() != &self.workspace {
            return Err(RepositorySessionError::WrongWorkspace);
        }
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let domain = state.claim_domain.clone().ok_or_else(refusal)?;
        let general = matching_validated_active_mut(&mut state, token)?
            .general
            .as_mut()
            .ok_or_else(refusal)?;
        if !Arc::ptr_eq(&general.scope, scope)
            || general.slots.iter().any(|slot| {
                slot.reservation.owner() == key.owner()
                    && slot.reservation.repository() == key.repository()
                    && slot.reservation.ordinal() == key.ordinal()
            })
        {
            return Err(refusal());
        }
        let instance = crate::runtime::native_identity::next_repository_instance()
            .map_err(|_| RepositorySessionError::InstanceExhausted)?;
        let claim = domain.reserve(key.clone(), instance)?;
        let retained = plus(
            plus(
                key.retained_storage_bytes().map_err(|_| refusal())?,
                repo_spec
                    .workspace_retained_storage_bytes()
                    .map_err(|_| refusal())?,
            )?,
            4096 + arc_size::<RepositoryWorkspaceReservation>(),
        )?;
        let credit = general
            .resources
            .reserve(
                plus(retained, std::mem::size_of::<ReservedRepositoryWorkspace>())?,
                0,
                0,
            )
            .map_err(|_| refusal())?;
        let slot_credit = general
            .resources
            .reserve(
                plus(retained, std::mem::size_of::<ReservationSlot>())?,
                0,
                0,
            )
            .map_err(|_| refusal())?;
        reserve_capacity(
            &mut general.slots,
            &mut general.capacity_credit,
            &general.resources,
            general.limits.max_state_versions(),
        )?;
        let reservation = Arc::new(
            RepositoryWorkspaceReservation::new(
                self.workspace.clone(),
                key.owner().clone(),
                key.repository().clone(),
                key.ordinal(),
                repo_spec,
                claim.display.clone(),
                instance,
                general.lease,
                general.limits,
            )
            .map_err(|_| refusal())?,
        );
        general.slots.push(ReservationSlot {
            reservation: reservation.clone(),
            claim: Arc::downgrade(&claim),
            adopted: false,
            _credit: slot_credit,
        });
        Ok(ReservedRepositoryWorkspace {
            token,
            reservation,
            claim,
            _credit: credit,
        })
    }
    pub(in crate::runtime) fn adopt_workspace(
        &self,
        token: RepositorySessionToken,
        ticket: &ReservedRepositoryWorkspace,
        generation: RepositoryMaterializationGeneration,
        sealed: SealedWorkspaceMaterialization,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        self.check_command_scope(token, scope)?;
        if token != ticket.token {
            return Err(refusal());
        }
        let request = sealed.request();
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
            return Err(RepositorySessionError::KindMismatch);
        };
        if &request.repo_spec != ticket.repo_spec().as_ref()
            || request.id.workspace != self.workspace
            || request.id.canonical_repo != *ticket.reservation.repository()
            || realization.reservation() != &ticket.reservation
            || sealed.root().reservation() != &ticket.reservation
            || sealed.root().state() != realization.final_state()
        {
            return Err(refusal());
        }
        let source_identity =
            slug_bzlmod_v2::source_preparation::general_workspace_source_identity(realization);
        let generation_root = ticket.reservation.display_root().as_path().to_path_buf();
        let result =
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
                source_identity,
                generation_root,
                observation_instance: ticket.reservation.instance(),
            });
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active_mut(&mut state, token)?;
        if active
            .entries
            .iter()
            .any(|entry| entry.request.id == request.id)
        {
            return Err(RepositorySessionError::ConflictingRequest(
                request.id.canonical_repo.clone(),
            ));
        }
        let general = active.general.as_mut().ok_or_else(refusal)?;
        if !Arc::ptr_eq(&general.scope, scope) || general.lease != ticket.reservation.lease() {
            return Err(refusal());
        }
        let slot_index = general
            .slots
            .iter()
            .position(|slot| slot.reservation == ticket.reservation)
            .ok_or_else(refusal)?;
        if general.slots[slot_index].adopted
            || !general.slots[slot_index]
                .claim
                .upgrade()
                .is_some_and(|claim| Arc::ptr_eq(&claim, &ticket.claim))
            || !sealed
                .root()
                .source_claim()
                .is_some_and(|claim| Arc::ptr_eq(&claim, &ticket.claim))
        {
            return Err(refusal());
        }
        reserve_capacity(
            &mut active.provisional_roots,
            &mut general.root_credit,
            &general.resources,
            general.limits.max_state_versions(),
        )?;
        reserve_capacity(
            &mut active.entries,
            &mut general.entry_credit,
            &general.resources,
            general.limits.max_state_versions(),
        )?;
        let count = active.entries.len().checked_add(1).ok_or_else(refusal)?;
        let dynamic = cloned_epoch_dynamic_storage(&active.entries)?
            .checked_add(cloned_materialization_dynamic_storage(
                &request.id.canonical_repo,
                &result,
            )?)
            .ok_or_else(refusal)?;
        let peak = count
            .checked_mul(16 * std::mem::size_of::<RepositoryMaterializationEpochEntry>() + 4096)
            .and_then(|n| n.checked_add(dynamic))
            .ok_or_else(refusal)?;
        let _projection = general
            .resources
            .reserve(peak, 0, 0)
            .map_err(|_| refusal())?;
        let mut entries = active.entries.clone();
        entries.push(RepositoryMaterializationEpochEntry {
            request: request.clone(),
            result: result.clone(),
        });
        let epoch = complete_epoch(&self.workspace, &entries)?;
        // The actor lock already proves the exact active token. Recheck the
        // borrowed original scope without recursively locking the actor.
        source_service::check_identity(Some(&general.scope), Some(scope))
            .map_err(RepositorySessionError::SourceService)?;
        // Only now disarm the receipt. Every earlier refusal drops actual root
        // and immutable payload before its complete guard, outside the mutex.
        let (request, root, credit) = sealed.into_parts();
        active.provisional_roots.push(RetainedRepositoryRoot {
            observation_instance: ticket.reservation.instance(),
            request: request.clone(),
            source_identity: match &result {
                RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        source_identity, ..
                    },
                ) => source_identity.clone(),
                _ => unreachable!(),
            },
            generation,
            root: RetainedRootOwner::Physical(root),
            guard: Some(credit),
        });
        active
            .entries
            .push(RepositoryMaterializationEpochEntry { request, result });
        general.slots[slot_index].adopted = true;
        Ok(epoch)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::time::Instant;

    use super::*;
    use crate::runtime::dice::source_service::SourceServiceLimits;
    fn resources() -> Arc<RepositoryRootResources> {
        RepositoryRootResources::new(
            crate::runtime::repository_physical_root::RepositoryRootResourceLimits::new(
                16 * 1024 * 1024,
                128,
                4 * 1024 * 1024 * 1024,
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn scope() -> Arc<SourceServiceScope> {
        let deadline = Instant::now() + Duration::from_secs(5);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap()
    }
    fn key(
        owner: Arc<slug_bzlmod_v2::HostSelectedExtensionOwner>,
        repo: &str,
    ) -> Arc<RepositoryWorkspaceStableKey> {
        Arc::new(
            RepositoryWorkspaceStableKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                owner,
                CanonicalRepoName::new(repo).unwrap(),
                0,
            )
            .unwrap(),
        )
    }
    fn spec(value: &str) -> Arc<RepoSpec> {
        Arc::new(RepoSpec {
            rule_id: slug_bzlmod_v2::RepoRuleId {
                bzl_file: slug_identity_v2::CanonicalLabel::parse("@@owner//:defs.bzl").unwrap(),
                rule_name: "rule".into(),
            },
            attributes: Arc::new(starlark_map::small_map::SmallMap::from_iter([(
                compact_str::CompactString::new("value"),
                OverrideAttributeValue::String(value.into()),
            )])),
        })
    }
    #[test]
    fn live_full_key_collision_refuses_and_dead_weak_control_stays_paid_until_prune() {
        let resources = resources();
        let baseline = resources.snapshot().retained_bytes;
        let domain = NativeSelectionDomain::new(resources.clone(), 8).unwrap();
        let owner = crate::runtime::dice::physical_root_owner_for_test();
        let a = key(owner.clone(), "a");
        let b = key(owner, "b");
        let first = domain
            .reserve_candidate(
                a.clone(),
                crate::runtime::native_identity::next_repository_instance().unwrap(),
                [7; 32],
            )
            .unwrap();
        assert!(
            domain
                .reserve_candidate(
                    b.clone(),
                    crate::runtime::native_identity::next_repository_instance().unwrap(),
                    [7; 32]
                )
                .is_err()
        );
        let second = domain
            .reserve_candidate(
                a,
                crate::runtime::native_identity::next_repository_instance().unwrap(),
                [7; 32],
            )
            .unwrap();
        assert_eq!(first.display(), second.display());
        assert_ne!(first.instance(), second.instance());
        assert!(first.same_domain(&second));
        drop(first);
        drop(second);
        assert_eq!(domain.claims.lock().unwrap().rows.len(), 2);
        assert!(resources.snapshot().retained_bytes > baseline);
        let replacement = domain
            .reserve_candidate(
                b,
                crate::runtime::native_identity::next_repository_instance().unwrap(),
                [7; 32],
            )
            .unwrap();
        assert_eq!(domain.claims.lock().unwrap().rows.len(), 1);
        drop(replacement);
        drop(domain);
        assert_eq!(
            (
                resources.snapshot().retained_bytes,
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
    }
    #[test]
    fn reservation_binds_fresh_command_spec_scope_and_blocks_generic_success_before_callback() {
        let scope = scope();
        let resources = resources();
        let baseline = resources.snapshot().retained_bytes;
        let actor =
            RepositoryMaterializer::new_scoped(scope.workspace().clone(), scope.clone()).unwrap();
        let token = actor.begin().unwrap();
        let early = actor.workspace_lease(token).unwrap();
        actor
            .validate(token, &mut |_, _| panic!("no prior roots"))
            .unwrap();
        let limits = RepositoryWorkspaceRetentionLimits::new(8, 1024 * 1024, 1024 * 1024).unwrap();
        assert_eq!(
            actor
                .begin_general_workspace(token, resources.clone(), limits, &scope)
                .unwrap(),
            early
        );
        let owner = crate::runtime::dice::physical_root_owner_for_test();
        let key = key(owner, "repo");
        let expected = spec("a");
        let first = actor
            .reserve_workspace(token, key.clone(), expected.clone(), &scope)
            .unwrap();
        assert_eq!(first.reservation().lease(), early);
        assert!(Arc::ptr_eq(first.repo_spec(), &expected));
        assert!(first.claim().matches_reservation(first.reservation()));
        assert!(
            actor
                .reserve_workspace(token, key.clone(), spec("b"), &scope)
                .is_err()
        );
        let realization =
            slug_bzlmod_v2::RepositoryWorkspaceTranscript::new(first.reservation().clone(), 0o755)
                .unwrap()
                .realize(0)
                .unwrap();
        let request = Arc::new(RepositoryMaterializationRequest {
            id: slug_bzlmod_v2::RepositoryMaterializationRequestId {
                workspace: scope.workspace().clone(),
                canonical_repo: first.reservation().repository().clone(),
            },
            repo_spec: (*expected).clone(),
            kind: RepositoryMaterializationKind::GeneralWorkspace(realization),
        });
        let mut called = false;
        assert_eq!(
            actor
                .materialize_with_scope(
                    token,
                    request,
                    RepositoryMaterializationGeneration(1),
                    Some(&scope),
                    || {
                        called = true;
                        RepositoryMaterializationAttempt::Local
                    }
                )
                .unwrap_err(),
            RepositorySessionError::KindMismatch
        );
        assert!(!called);
        let other = super::tests::scope();
        assert!(
            actor
                .reserve_workspace(token, key.clone(), spec("a"), &other)
                .is_err()
        );
        actor.discard(token).unwrap();
        let next = actor.begin().unwrap();
        actor
            .validate(next, &mut |_, _| panic!("no accepted roots"))
            .unwrap();
        assert_ne!(actor.workspace_lease(next).unwrap(), early);
        actor
            .begin_general_workspace(next, resources.clone(), limits, &scope)
            .unwrap();
        let second = actor
            .reserve_workspace(next, key, spec("changed"), &scope)
            .unwrap();
        assert_eq!(
            first.reservation().display_root(),
            second.reservation().display_root()
        );
        assert_ne!(
            first.reservation().instance(),
            second.reservation().instance()
        );
        actor.discard(next).unwrap();
        drop(actor);
        assert!(resources.snapshot().retained_bytes > baseline);
        drop(first);
        drop(second);
        assert_eq!(
            (
                resources.snapshot().retained_bytes,
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes
            ),
            (baseline, 0, 0)
        );
    }
}
