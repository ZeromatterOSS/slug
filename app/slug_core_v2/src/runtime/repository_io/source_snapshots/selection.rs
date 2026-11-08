//! A temporary, credited selection snapshot. It grants no new acquisition or
//! mount authority and never enters DICE or an accepted-source registry.
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;

use super::*;
use crate::runtime::native_source_access::NativeReaderCredit;
use crate::runtime::native_source_access::NativeReaderResources;
use crate::runtime::repository_physical_root::ViewComparison;

pub(in crate::runtime::repository_io) struct SelectedSourceSnapshots {
    // Shared request refs retire before the captured roots covering their full
    // allocation. Foreign requests carry a separately prepaid full allowance.
    current_requests: Vec<Option<CurrentRequest>>,
    pub(in crate::runtime::repository_io) rows: Vec<SourceSnapshotRow>,
    scope: Option<Arc<SourceServiceScope>>,
    bank: Option<Arc<RepositoryRootResources>>,
    lease: Option<u64>,
    // Row/data guards precede their temporary allocation allowance on drop.
    _credit: NativeReaderCredit,
}
struct CurrentRequest {
    request: Arc<RepositoryMaterializationRequest>,
    _credit: Option<NativeReaderCredit>,
}

fn request_storage(
    request: &RepositoryMaterializationRequest,
) -> Result<usize, RepositorySessionError> {
    let mut bytes = std::mem::size_of::<RepositoryMaterializationRequest>()
        .checked_add(128)
        .and_then(|n| n.checked_add(request.id.workspace.retained_storage_bytes()?))
        .and_then(|n| n.checked_add(request.id.canonical_repo.retained_capacity_bytes()))
        .and_then(|n| n.checked_add(request.repo_spec.workspace_retained_storage_bytes().ok()?))
        .ok_or_else(refusal)?;
    match &request.kind {
        RepositoryMaterializationKind::Local { logical_root } => {
            bytes = bytes
                .checked_add(logical_root.retained_storage_bytes().ok_or_else(refusal)?)
                .ok_or_else(refusal)?;
        }
        RepositoryMaterializationKind::Immutable => {}
        _ => return Err(refusal()),
    }
    Ok(bytes)
}

impl SelectedSourceSnapshots {
    pub(in crate::runtime::repository_io) fn new(
        actor: &RepositoryMaterializer,
        token: RepositorySessionToken,
        epoch: &RepositorySourceSnapshotEpoch,
        readers: &Arc<NativeReaderResources>,
    ) -> Result<Self, RepositorySessionError> {
        let scope = actor.command_source_scope(token)?;
        let count = epoch.states().map_or(0, |states| states.len());
        if count > slug_bzlmod_v2::REPOSITORY_WORKSPACE_MAX_READ_VIEWS {
            return Err(refusal());
        }
        let bytes = count
            .checked_mul(
                4 * std::mem::size_of::<SourceSnapshotRow>()
                    + 4 * std::mem::size_of::<Option<CurrentRequest>>()
                    + 128,
            )
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 256))
            .ok_or_else(refusal)?;
        let credit = readers.reserve(bytes, 0, 0).map_err(|_| refusal())?;
        let (rows, bank, lease, current_requests) = {
            let state = actor.state.lock().map_err(|_| refusal())?;
            let active = matching_validated_active(&state, token)?;
            if !super::super::command_scope::scope_is_current(active, scope.as_ref()) {
                return Err(refusal());
            }
            match &active.general {
                None => {
                    if epoch.workspace().is_some()
                        || epoch.lease().is_some()
                        || epoch.states().is_some()
                        || !active.source_snapshots.is_empty()
                    {
                        return Err(refusal());
                    }
                    (Vec::new(), None, None, Vec::new())
                }
                Some(general) => {
                    if epoch.workspace() != Some(&actor.workspace)
                        || epoch.lease() != Some(active.workspace_lease)
                        || epoch.lease() != Some(general.lease())
                        || count != active.source_snapshots.len()
                        || !scope
                            .as_ref()
                            .is_some_and(|s| Arc::ptr_eq(s, general.scope()))
                    {
                        return Err(refusal());
                    }
                    let rows = active.source_snapshots.clone();
                    let mut current_requests = Vec::with_capacity(rows.len());
                    for row in &rows {
                        let expected = row.request().materialization_request();
                        let mut matches = active.entries.iter().filter(|entry| {
                            &entry.request.id.canonical_repo == row.request().repository()
                        });
                        let entry = matches.next();
                        if matches.next().is_some() {
                            return Err(refusal());
                        }
                        let retained = match (expected, entry) {
                            (_, None) => None,
                            (None, Some(_)) => return Err(refusal()),
                            (Some(expected), Some(entry)) => {
                                if std::mem::discriminant(&expected.kind)
                                    != std::mem::discriminant(&entry.request.kind)
                                {
                                    return Err(refusal());
                                }
                                let credit = if Arc::ptr_eq(expected, &entry.request) {
                                    None
                                } else {
                                    Some(
                                        readers
                                            .reserve(request_storage(&entry.request)?, 0, 0)
                                            .map_err(|_| refusal())?,
                                    )
                                };
                                Some(CurrentRequest {
                                    request: entry.request.clone(),
                                    _credit: credit,
                                })
                            }
                        };
                        current_requests.push(retained);
                    }
                    (
                        rows,
                        Some(general.resources().clone()),
                        Some(general.lease()),
                        current_requests,
                    )
                }
            }
        };
        if rows.capacity() > count.saturating_mul(4)
            || current_requests.capacity() > count.saturating_mul(4)
        {
            return Err(refusal());
        }
        if let Some(bank) = &bank {
            let scope = scope.as_ref().ok_or_else(refusal)?;
            let active = || actor.session_is_active(token);
            let mut comparison =
                ViewComparison::new(bank, scope, &active).map_err(|_| refusal())?;
            let mut requests =
                RepositoryRequestComparison::new(Some(readers), Some(scope), &active)?;
            for (row, current) in rows.iter().zip(&current_requests) {
                if let Some(current) = current {
                    let expected = row
                        .request()
                        .materialization_request()
                        .ok_or_else(refusal)?;
                    if !requests.equal(expected, &current.request)? {
                        return Err(refusal());
                    }
                }
            }
            // Full structural comparison happens after releasing the actor
            // mutex. Each root pays its own complete request/state lifetime.
            for expected in epoch.states().ok_or_else(refusal)? {
                let row = rows
                    .iter()
                    .find(|row| {
                        row.root.reservation().repository() == expected.reservation().repository()
                    })
                    .ok_or_else(refusal)?;
                let root = &row.root;
                root.check_origin(scope, &active).map_err(|_| refusal())?;
                if !root.source_claim().is_some_and(|claim| {
                    claim.belongs_to(bank) && claim.matches_reservation(root.reservation())
                }) || !comparison
                    .source_state_equal(root.state(), expected)
                    .map_err(|_| refusal())?
                {
                    return Err(refusal());
                }
            }
        }
        let selected = Self {
            current_requests,
            rows,
            scope,
            bank,
            lease,
            _credit: credit,
        };
        selected.recheck(actor, token)?;
        Ok(selected)
    }

    pub(in crate::runtime::repository_io) fn recheck(
        &self,
        actor: &RepositoryMaterializer,
        token: RepositorySessionToken,
    ) -> Result<(), RepositorySessionError> {
        if let Some(scope) = &self.scope {
            actor.check_command_scope(token, scope)?;
            for row in &self.rows {
                row.root
                    .check_origin(scope, &|| actor.session_is_active(token))
                    .map_err(|_| refusal())?;
            }
        }
        let state = actor.state.lock().map_err(|_| refusal())?;
        let active = matching_validated_active(&state, token)?;
        self.recheck_locked(active)
    }

    /// Pure current-association checks may join the materialization recheck in
    /// its existing final actor critical section. No IO or graph call occurs.
    pub(in crate::runtime::repository_io) fn recheck_locked(
        &self,
        active: &ActiveRepositorySession,
    ) -> Result<(), RepositorySessionError> {
        if !super::super::command_scope::scope_is_current(active, self.scope.as_ref())
            || active.source_snapshots.len() != self.rows.len()
        {
            return Err(refusal());
        }
        match (self.lease, active.general.as_ref()) {
            (None, None) if self.rows.is_empty() => {}
            (Some(lease), Some(general))
                if active.workspace_lease == lease
                    && general.lease() == lease
                    && self
                        .bank
                        .as_ref()
                        .is_some_and(|bank| Arc::ptr_eq(bank, general.resources()))
                    && self
                        .scope
                        .as_ref()
                        .is_some_and(|scope| Arc::ptr_eq(scope, general.scope())) => {}
            _ => return Err(refusal()),
        }
        for (row, expected_request) in self.rows.iter().zip(&self.current_requests) {
            if !active
                .source_snapshots
                .iter()
                .any(|current| Arc::ptr_eq(&current.root, &row.root))
            {
                return Err(refusal());
            }
            let mut entries = active
                .entries
                .iter()
                .filter(|entry| &entry.request.id.canonical_repo == row.request().repository());
            match (expected_request, entries.next()) {
                (None, None) => {}
                (Some(expected), Some(current))
                    if Arc::ptr_eq(&expected.request, &current.request) => {}
                _ => return Err(refusal()),
            }
            if entries.next().is_some() {
                return Err(refusal());
            }
        }
        Ok(())
    }
}
