//! Exact selected-source association snapshots for the general process runner.
//! The actor mutex is never held during semantic comparison or native work.
use slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;

use super::*;
use crate::runtime::repository_physical_root::NativeReadViewData;
use crate::runtime::repository_physical_root::PhysicalRootError;
use crate::runtime::repository_physical_root::RepositoryPhysicalRoot;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::RepositoryWorkspaceCommand;

pub(in crate::runtime) struct SelectedProcessSources {
    entries: Vec<RepositoryMaterializationEpochEntry>,
    rows: Vec<RetainedRepositoryRoot>,
    token: RepositorySessionToken,
    lease: u64,
    scope: Arc<SourceServiceScope>,
    resources: Arc<RepositoryRootResources>,
    _credit: crate::runtime::repository_physical_root::ResourceCredit,
}
impl SelectedProcessSources {
    pub(in crate::runtime) fn validate_manifest(
        &self,
        manifest: &RepositoryWorkspaceReadViewManifest,
        executor: &RepositoryWorkspaceReservation,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        if self.lease != executor.lease()
            || !Arc::ptr_eq(resources, &self.resources)
            || !Arc::ptr_eq(scope, &self.scope)
            || manifest.views().len() != self.rows.len()
        {
            return Err(PhysicalRootError::Reservation);
        }
        manifest
            .validate_for(executor)
            .map_err(PhysicalRootError::Protocol)?;
        let mut comparison = crate::runtime::repository_physical_root::ViewComparison::new(
            resources, scope, active,
        )?;
        for view in manifest.views() {
            let row = self
                .rows
                .iter()
                .find(|row| row.observation_instance == view.state().reservation().instance())
                .ok_or(PhysicalRootError::Reservation)?;
            let RetainedRootOwner::Physical(root) = &row.root else {
                return Err(PhysicalRootError::UnsupportedOperation);
            };
            if row.request.repo_spec != *view.repo_spec()
                || !comparison.state_equal(root.state(), view.state())?
                || root.reservation() != view.state().reservation()
            {
                return Err(PhysicalRootError::Reservation);
            }
        }
        Ok(())
    }
    pub(in crate::runtime) fn matches_data(&self, data: &Arc<NativeReadViewData>) -> bool {
        self.rows.iter().any(|row| match &row.root {
            RetainedRootOwner::Physical(root) => {
                Arc::ptr_eq(root, data.root())
                    && row.request.repo_spec == *data.projection().repo_spec()
                    && Arc::ptr_eq(root.state(), data.projection().state())
            }
            _ => false,
        })
    }
    pub(in crate::runtime) fn recheck(
        &self,
        actor: &RepositoryMaterializer,
    ) -> Result<(), PhysicalRootError> {
        self.scope.check()?;
        actor
            .check_source_scope(Some(&self.scope))
            .map_err(|_| PhysicalRootError::Reservation)?;
        let state = actor
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, self.token)
            .map_err(|_| PhysicalRootError::Reservation)?;
        let general = active
            .general
            .as_ref()
            .ok_or(PhysicalRootError::Reservation)?;
        if general.lease() != self.lease
            || !Arc::ptr_eq(general.resources(), &self.resources)
            || !Arc::ptr_eq(general.scope(), &self.scope)
        {
            return Err(PhysicalRootError::Reservation);
        }
        for entry in &self.entries {
            if !active
                .entries
                .iter()
                .any(|now| Arc::ptr_eq(&entry.request, &now.request) && now.result == entry.result)
            {
                return Err(PhysicalRootError::Reservation);
            }
        }
        for row in &self.rows {
            if !physical_roots::association_current(&state, active, row) {
                return Err(PhysicalRootError::Reservation);
            }
        }
        Ok(())
    }
}
impl RepositoryMaterializer {
    pub(in crate::runtime) fn selected_process_sources(
        &self,
        token: RepositorySessionToken,
        selected: &[Arc<RepositoryMaterializationRequest>],
        results: &RepositoryMaterializationResultEpoch,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<SelectedProcessSources, PhysicalRootError> {
        if !active() {
            return Err(PhysicalRootError::Inactive);
        }
        scope.check()?;
        self.check_source_scope(Some(scope))
            .map_err(|_| PhysicalRootError::Reservation)?;
        if selected.len() > 128 {
            return Err(PhysicalRootError::Resource);
        }
        let snapshot = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active(&state, token)
                .map_err(|_| PhysicalRootError::Reservation)?;
            let general = active
                .general
                .as_ref()
                .ok_or(PhysicalRootError::Reservation)?;
            if !Arc::ptr_eq(general.scope(), scope) {
                return Err(PhysicalRootError::Reservation);
            }
            let dynamic = cloned_epoch_dynamic_storage(
                active
                    .entries
                    .iter()
                    .filter(|entry| selected.iter().any(|r| r.id == entry.request.id)),
            )
            .map_err(|_| PhysicalRootError::Resource)?;
            let bytes = selected
                .len()
                .checked_mul(
                    4 * std::mem::size_of::<RepositoryMaterializationEpochEntry>()
                        + 4 * std::mem::size_of::<RetainedRepositoryRoot>()
                        + 4096,
                )
                .and_then(|n| n.checked_add(dynamic))
                .and_then(|n| n.checked_add(std::mem::size_of::<SelectedProcessSources>() + 4096))
                .ok_or(PhysicalRootError::Resource)?;
            let credit = general.resources().reserve(bytes, 0, 0)?;
            let mut rows = Vec::with_capacity(selected.len());
            let mut entries = Vec::with_capacity(selected.len());
            for request in selected {
                if entries
                    .iter()
                    .any(|old: &RepositoryMaterializationEpochEntry| old.request.id == request.id)
                {
                    return Err(PhysicalRootError::Reservation);
                }
                let entry = active
                    .entries
                    .iter()
                    .find(|entry| entry.request.id == request.id)
                    .ok_or(PhysicalRootError::Reservation)?;
                let RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        observation_instance,
                        source_identity,
                        generation_root,
                    },
                ) = &entry.result
                else {
                    return Err(PhysicalRootError::UnsupportedOperation);
                };
                let mut found = state
                    .accepted_roots
                    .iter()
                    .chain(&active.provisional_roots)
                    .filter(|row| row.observation_instance == *observation_instance);
                let row = found.next().ok_or(PhysicalRootError::Reservation)?;
                let RetainedRootOwner::Physical(root) = &row.root else {
                    return Err(PhysicalRootError::UnsupportedOperation);
                };
                if found.next().is_some()
                    || !Arc::ptr_eq(&entry.request, &row.request)
                    || row.guard.is_none()
                    || &row.source_identity != source_identity
                    || row.root.path() != generation_root
                    || root.reservation().lease() != general.lease()
                {
                    return Err(PhysicalRootError::Reservation);
                }
                entries.push(entry.clone());
                rows.push(row.clone());
            }
            SelectedProcessSources {
                entries,
                rows,
                token,
                lease: general.lease(),
                scope: scope.clone(),
                resources: general.resources().clone(),
                _credit: credit,
            }
        };
        // Supplied requests are borrowed semantic values. Only the current,
        // fully guarded actor allocations leave the mutex in the snapshot.
        let mut comparison = crate::runtime::repository_physical_root::ViewComparison::new(
            &snapshot.resources,
            scope,
            active,
        )?;
        for (entry, requested) in snapshot.entries.iter().zip(selected) {
            if !comparison.request_equal(&entry.request, requested)? {
                return Err(PhysicalRootError::Reservation);
            }
        }
        let expected = complete_epoch(&self.workspace, &snapshot.entries)
            .map_err(|_| PhysicalRootError::Reservation)?;
        if !comparison.epoch_equal(&expected, results)? {
            return Err(PhysicalRootError::Reservation);
        }
        snapshot.recheck(self)?;
        Ok(snapshot)
    }
    pub(in crate::runtime) async fn execute_workspace(
        &self,
        token: RepositorySessionToken,
        command: RepositoryWorkspaceCommand,
        request: RepositoryWorkspaceRequest,
        selected: &[Arc<RepositoryMaterializationRequest>],
        results: &RepositoryMaterializationResultEpoch,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
    ) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
        self.execute_workspace_impl(
            token,
            command,
            request,
            selected,
            results,
            scope,
            active,
            |_| {},
        )
        .await
    }
    #[cfg(test)]
    pub(in crate::runtime) async fn execute_workspace_with_recheck_hook_for_test(
        &self,
        token: RepositorySessionToken,
        command: RepositoryWorkspaceCommand,
        request: RepositoryWorkspaceRequest,
        selected: &[Arc<RepositoryMaterializationRequest>],
        results: &RepositoryMaterializationResultEpoch,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
        after_reap: impl FnOnce(&RepositoryWorkspaceCommand),
    ) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
        self.execute_workspace_impl(
            token, command, request, selected, results, scope, active, after_reap,
        )
        .await
    }
    async fn execute_workspace_impl(
        &self,
        token: RepositorySessionToken,
        command: RepositoryWorkspaceCommand,
        request: RepositoryWorkspaceRequest,
        selected: &[Arc<RepositoryMaterializationRequest>],
        results: &RepositoryMaterializationResultEpoch,
        scope: &Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
        after_reap: impl FnOnce(&RepositoryWorkspaceCommand),
    ) -> Result<RepositoryWorkspaceCommand, PhysicalRootError> {
        if !active() {
            return Err(PhysicalRootError::Inactive);
        }
        let snapshot = self.selected_process_sources(token, selected, results, scope, active)?;
        let command = command
            .execute_selected_sources(request, &snapshot, self, scope, &|| {
                active() && self.session_is_active(token)
            })
            .await?;
        after_reap(&command);
        if !active() {
            return Err(PhysicalRootError::Inactive);
        }
        scope.check()?;
        // The worker's updated epoch/controllers remain private until this full
        // same-token association check. Refusal consumes them after actual reap.
        snapshot.recheck(self)?;
        Ok(command)
    }
}

#[cfg(test)]
#[path = "process_view_tests.rs"]
mod tests;
