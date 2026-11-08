//! Inactive original-evaluation join. Native authority stays in the exclusive
//! command; only its immutable epoch and explicit policy enter DICE.
use slug_bzlmod_v2::RepositoryMaterializationResult;

use super::*;

fn refusal(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}

impl WorkspaceRuntime {
    pub(super) fn repository_workspace_policy(
        &self,
    ) -> slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        if self.general_workspaces.is_some() {
            return slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::LinuxReadOnlyHostV1;
        }
        slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::Disabled
    }

    /// Trusted inactive proof configuration, independent of caller/source/tool.
    /// Native production has no original command scope yet and cannot enable it.
    #[cfg(all(
        test,
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    pub(in crate::runtime) fn enable_general_repository_workspaces_for_test(
        &mut self,
        resources: Arc<super::super::repository_physical_root::RepositoryRootResources>,
        limits: slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits,
    ) -> Result<(), NativeDemandSessionError> {
        self.source_scope
            .as_ref()
            .ok_or_else(|| refusal("general workspace proof requires original diagnostic scope"))?
            .check()
            .map_err(NativeDemandSessionError::SourceService)?;
        if self.general_workspaces.is_some()
            || !matches!(
                self.native_demand_sessions
                    .state
                    .lock()
                    .expect("native demand mutex poisoned")
                    .phase,
                NativeDemandLeasePhase::Idle
            )
        {
            return Err(refusal("general workspace policy is already bound"));
        }
        self.general_workspaces = Some(GeneralWorkspaceRuntime {
            resources,
            limits,
            probe: None,
        });
        Ok(())
    }
    #[cfg(all(
        test,
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    fn observe_general_processes_for_test(
        &mut self,
        stage: Arc<std::sync::atomic::AtomicUsize>,
        hold_before_hello: bool,
    ) -> Arc<
        Mutex<
            Option<crate::runtime::repository_physical_root::RepositoryWorkspaceRetirementObserver>,
        >,
    > {
        let retirement = Arc::new(Mutex::new(None));
        self.general_workspaces
            .as_mut()
            .expect("enabled generic test policy")
            .probe = Some(native::GeneralWorkspaceProbe {
            stage,
            hold_before_hello,
            retirement: retirement.clone(),
        });
        retirement
    }
}

impl NativeDemandCommand<'_> {
    pub(super) fn repository_requests_equal(
        &self,
        left: &RepositoryMaterializationRequest,
        right: &RepositoryMaterializationRequest,
    ) -> Result<bool, NativeDemandSessionError> {
        let general = matches!(
            left.kind,
            RepositoryMaterializationKind::GeneralWorkspace(_)
        );
        if general
            != matches!(
                right.kind,
                RepositoryMaterializationKind::GeneralWorkspace(_)
            )
        {
            return Ok(false);
        }
        if general {
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            if let Some(progress) = &self.workspace_progress {
                return progress.request_equal(left, right, &|| {
                    self.runtime
                        .repository_materializer
                        .session_is_active(self.repository_session)
                });
            }
            return Err(refusal(
                "general request comparison requires its original native owner",
            ));
        }
        Ok(left == right)
    }
    pub(super) fn current_workspace_epoch(&self) -> &slug_bzlmod_v2::RepositoryWorkspaceEpoch {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        if let Some(progress) = &self.workspace_progress {
            return progress
                .command
                .as_ref()
                .expect("native workspace command is not in flight during DICE")
                .epoch();
        }
        &self.workspace_epoch
    }

    pub(super) fn progress_general_workspace(
        &mut self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<Option<NativeDemandProgress>, NativeDemandSessionError> {
        let demanded =
            !needs.workspace_reservations().is_empty() || !needs.workspace_operations().is_empty();
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        if let Some(progress) = &mut self.workspace_progress {
            if !needs.repository_executes().is_empty()
                || !needs.repository_filesystem_operations().is_empty()
                || needs.repository_materializations().values().any(|request| {
                    matches!(
                        request.kind,
                        RepositoryMaterializationKind::GeneratedFileEffects(_)
                            | RepositoryMaterializationKind::GeneratedFilesystemEffects(_)
                    )
                })
            {
                return Err(refusal(
                    "general original evaluation emitted a legacy operation or realization",
                ));
            }
            if demanded {
                progress.progress(
                    self.runtime,
                    self.repository_session,
                    needs,
                    &self.inputs.request.repository_environment,
                    &self.inputs.request.archive_distdirs,
                )?;
                return Ok(Some(NativeDemandProgress::Workspaces));
            }
        }
        if demanded {
            return Err(refusal(
                "general repository workspace execution is disabled",
            ));
        }
        Ok(None)
    }

    pub(super) fn finish_general_workspace(
        &mut self,
        request: Arc<RepositoryMaterializationRequest>,
    ) -> Result<RepositoryMaterializationResultEpoch, NativeDemandSessionError> {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        if let Some(progress) = &mut self.workspace_progress {
            return progress.finish(
                self.runtime,
                self.repository_session,
                request,
                self.inputs.generations.repository,
            );
        }
        let _ = request;
        Err(refusal("general repository materialization is disabled"))
    }
    pub(super) fn retire_general_workspace(&mut self) -> Result<(), NativeDemandSessionError> {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        if let Some(progress) = &mut self.workspace_progress {
            progress.retire()?;
        }
        Ok(())
    }
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(super) use native::GeneralWorkspaceRuntime;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(super) use native::NativeWorkspaceProgress;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
mod native {
    use slug_bzlmod_v2::RepositoryWorkspaceOperation;
    use slug_bzlmod_v2::RepositoryWorkspaceReservation;
    use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;
    use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;

    use super::*;
    use crate::runtime::repository_io::reservations::ReservedRepositoryWorkspace;
    use crate::runtime::repository_physical_root::PhysicalRootError;
    use crate::runtime::repository_physical_root::RepositoryRootResources;
    use crate::runtime::repository_physical_root::RepositoryWorkspaceCommand;
    use crate::runtime::repository_physical_root::ResourceCredit;

    pub(in crate::runtime::dice) struct GeneralWorkspaceRuntime {
        pub(super) resources: Arc<RepositoryRootResources>,
        pub(super) limits: RepositoryWorkspaceRetentionLimits,
        #[cfg(test)]
        pub(super) probe: Option<GeneralWorkspaceProbe>,
    }
    #[cfg(test)]
    pub(super) struct GeneralWorkspaceProbe {
        pub(super) stage: Arc<std::sync::atomic::AtomicUsize>,
        pub(super) hold_before_hello: bool,
        pub(super) retirement: Arc<
            Mutex<
                Option<
                    crate::runtime::repository_physical_root::RepositoryWorkspaceRetirementObserver,
                >,
            >,
        >,
    }
    pub(in crate::runtime::dice) struct NativeWorkspaceProgress {
        pub(super) command: Option<RepositoryWorkspaceCommand>,
        retirement: crate::runtime::repository_physical_root::RepositoryWorkspaceRetirementObserver,
        tickets: SmallMap<Arc<RepositoryWorkspaceStableKey>, ReservedRepositoryWorkspace>,
        scope: Arc<source_service::SourceServiceScope>,
        resources: Arc<RepositoryRootResources>,
        max_roots: usize,
        _credit: ResourceCredit,
    }
    struct OperationSelection {
        requests: Vec<Arc<RepositoryMaterializationRequest>>,
        epoch: RepositoryMaterializationResultEpoch,
        _credit: ResourceCredit,
    }
    fn physical(error: PhysicalRootError) -> NativeDemandSessionError {
        NativeDemandSessionError::RepositoryWorkspace(error)
    }
    impl NativeWorkspaceProgress {
        pub(in crate::runtime::dice) fn new(
            runtime: &WorkspaceRuntime,
            token: super::super::super::repository_io::RepositorySessionToken,
        ) -> Result<Option<Self>, NativeDemandSessionError> {
            let Some(config) = &runtime.general_workspaces else {
                return Ok(None);
            };
            #[cfg(test)]
            runtime
                .native_demand_sessions
                .mark_workspace_stage("native_admission");
            let scope = runtime
                .source_scope
                .as_ref()
                .ok_or_else(|| refusal("general workspace requires original diagnostic scope"))?
                .clone();
            scope
                .check()
                .map_err(NativeDemandSessionError::SourceService)?;
            let max_roots = config.limits.max_state_versions();
            let bytes = max_roots
                .checked_mul(
                    4 * (std::mem::size_of::<(
                        Arc<RepositoryWorkspaceStableKey>,
                        ReservedRepositoryWorkspace,
                    )>() + 64),
                )
                .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 4096))
                .ok_or_else(|| refusal("workspace ticket storage overflow"))?;
            let credit = config.resources.reserve(bytes, 0, 0).map_err(physical)?;
            let lease = runtime
                .repository_materializer
                .begin_general_workspace(token, config.resources.clone(), config.limits, &scope)
                .map_err(NativeDemandSessionError::Repository)?;
            let command = RepositoryWorkspaceCommand::new(
                runtime.repository_materializer.workspace().clone(),
                lease,
                config.limits,
                config.resources.clone(),
                scope.clone(),
                &|| runtime.repository_materializer.session_is_active(token),
            )
            .map_err(physical)?;
            let retirement = command.retirement_observer();
            #[cfg(test)]
            let command = if let Some(probe) = &config.probe {
                *probe
                    .retirement
                    .lock()
                    .expect("retirement probe mutex poisoned") = Some(retirement.clone());
                command.with_process_probe_for_test(probe.stage.clone(), probe.hold_before_hello)
            } else {
                command
            };
            Ok(Some(Self {
                command: Some(command),
                retirement,
                tickets: SmallMap::new(),
                scope,
                resources: config.resources.clone(),
                max_roots,
                _credit: credit,
            }))
        }
        pub(super) fn retire(&mut self) -> Result<(), NativeDemandSessionError> {
            drop(self.command.take());
            if !self.retirement.wait_until(self.scope.cleanup_deadline()) {
                return Err(NativeDemandSessionError::SourceService(
                    source_service::SourceServiceError::CleanupDeadline,
                ));
            }
            Ok(())
        }
        pub(super) fn request_equal(
            &self,
            left: &RepositoryMaterializationRequest,
            right: &RepositoryMaterializationRequest,
            active: &dyn Fn() -> bool,
        ) -> Result<bool, NativeDemandSessionError> {
            crate::runtime::repository_physical_root::ViewComparison::new(
                &self.resources,
                &self.scope,
                active,
            )
            .map_err(physical)?
            .request_equal(left, right)
            .map_err(physical)
        }
        fn ticket(
            &self,
            reservation: &RepositoryWorkspaceReservation,
        ) -> Result<&ReservedRepositoryWorkspace, NativeDemandSessionError> {
            self.tickets
                .values()
                .find(|ticket| ticket.reservation().as_ref() == reservation)
                .ok_or_else(|| refusal("workspace operation has no exact reservation ticket"))
        }
        fn selected(
            &self,
            demand: &slug_bzlmod_v2::RepositoryWorkspaceOperationDemand,
        ) -> Result<OperationSelection, NativeDemandSessionError> {
            let entries = demand.selected_entries();
            let dynamic = entries.iter().try_fold(0usize, |total, entry| {
                let result = match &entry.result {
                    RepositoryMaterializationResult::Success(
                        slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                            generation_root,
                            ..
                        },
                    ) => generation_root.capacity(),
                    RepositoryMaterializationResult::Success(_) => 0,
                    RepositoryMaterializationResult::SpecError(message)
                    | RepositoryMaterializationResult::MaterializationError { message, .. }
                    | RepositoryMaterializationResult::TransportError { message, .. } => {
                        message.capacity()
                    }
                };
                total
                    .checked_add(
                        result
                            .checked_add(entry.request.id.canonical_repo.retained_capacity_bytes())
                            .and_then(|n| n.checked_mul(4))
                            .ok_or_else(|| refusal("workspace selection storage overflow"))?,
                    )
                    .ok_or_else(|| refusal("workspace selection storage overflow"))
            })?;
            let bytes = entries
                .len()
                .checked_mul(
                    8 * std::mem::size_of::<slug_bzlmod_v2::RepositoryMaterializationEpochEntry>()
                        + 4 * std::mem::size_of::<Arc<RepositoryMaterializationRequest>>()
                        + 256,
                )
                .and_then(|n| n.checked_add(dynamic))
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| refusal("workspace selection storage overflow"))?;
            let credit = self.resources.reserve(bytes, 0, 0).map_err(physical)?;
            let requests = entries.iter().map(|entry| entry.request.clone()).collect();
            let epoch = RepositoryMaterializationResultEpoch::new(
                demand.request().reservation().workspace().clone(),
                entries.iter().cloned(),
            )
            .map_err(|_| refusal("workspace selected epoch is invalid"))?;
            Ok(OperationSelection {
                requests,
                epoch,
                _credit: credit,
            })
        }
        pub(super) fn progress(
            &mut self,
            runtime: &WorkspaceRuntime,
            token: super::super::super::repository_io::RepositorySessionToken,
            needs: &slug_bzlmod_v2::SourcePreparationNeeds,
            environment: &slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
            distdirs: &crate::runtime::ArchiveDistdirPolicy,
        ) -> Result<(), NativeDemandSessionError> {
            #[cfg(test)]
            runtime
                .native_demand_sessions
                .mark_workspace_stage("frontier_preflight");
            self.scope
                .check()
                .map_err(NativeDemandSessionError::SourceService)?;
            if needs.workspace_operations().len() > self.max_roots
                || self
                    .tickets
                    .len()
                    .checked_add(
                        needs
                            .workspace_reservations()
                            .keys()
                            .filter(|key| !self.tickets.contains_key(*key))
                            .count(),
                    )
                    .is_none_or(|count| count > self.max_roots)
            {
                return Err(refusal("workspace demand count exceeds policy"));
            }
            // Validate the complete frontier before setup or mutation. The pure
            // carrier's source declarations are independent of argv/manifest.
            for demand in needs.workspace_reservations().values() {
                if demand.key().workspace() != runtime.repository_materializer.workspace() {
                    return Err(refusal("foreign workspace reservation demand"));
                }
                if let Some(ticket) = self.tickets.get(demand.key()) {
                    if !demand.matches(ticket.reservation()) {
                        return Err(refusal("workspace reservation demand changed"));
                    }
                }
            }
            let active = || runtime.repository_materializer.session_is_active(token);
            let mut comparison = crate::runtime::repository_physical_root::ViewComparison::new(
                &self.resources,
                &self.scope,
                &active,
            )
            .map_err(physical)?;
            for demand in needs.workspace_operations().values() {
                self.ticket(demand.request().reservation())?;
                self.command
                    .as_ref()
                    .ok_or_else(|| refusal("workspace command was consumed"))?
                    .validate_frontier_request(
                        demand.request(),
                        &self.scope,
                        &active,
                        &mut comparison,
                    )
                    .map_err(physical)?;
                if let RepositoryWorkspaceOperation::Execute(execute) = demand.request().operation()
                {
                    if execute.environment() != environment {
                        return Err(NativeDemandSessionError::ExecuteEnvironmentMismatch);
                    }
                    let selected = self.selected(demand)?;
                    let checked = runtime
                        .repository_materializer
                        .selected_process_sources(
                            token,
                            &selected.requests,
                            &selected.epoch,
                            &self.scope,
                            &|| runtime.repository_materializer.session_is_active(token),
                        )
                        .map_err(physical)?;
                    checked
                        .validate_manifest(
                            execute.read_views(),
                            demand.request().reservation(),
                            &self.resources,
                            &self.scope,
                            &|| runtime.repository_materializer.session_is_active(token),
                        )
                        .map_err(physical)?;
                } else if !demand.selected_entries().is_empty() {
                    let selected = self.selected(demand)?;
                    runtime
                        .repository_materializer
                        .pin_selected_roots(
                            token,
                            &selected.requests,
                            &selected.epoch,
                            &runtime.source_readers,
                        )
                        .map_err(|_| {
                            refusal("workspace source declarations differ from current selection")
                        })?;
                }
            }
            drop(comparison);
            let mut advanced = false;
            for demand in needs.workspace_reservations().values() {
                if self.tickets.contains_key(demand.key()) {
                    continue;
                }
                if self.tickets.len() >= self.max_roots {
                    return Err(refusal("workspace reservation count exceeds policy"));
                }
                #[cfg(test)]
                runtime
                    .native_demand_sessions
                    .mark_workspace_stage("reservation");
                let ticket = runtime
                    .repository_materializer
                    .reserve_workspace(
                        token,
                        demand.key().clone(),
                        demand.repo_spec().clone(),
                        &self.scope,
                    )
                    .map_err(NativeDemandSessionError::Repository)?;
                let command = self
                    .command
                    .take()
                    .ok_or_else(|| refusal("workspace command was consumed"))?;
                #[cfg(test)]
                runtime
                    .native_demand_sessions
                    .mark_workspace_stage("bootstrap");
                self.command = Some(
                    runtime
                        .runtime
                        .block_on(command.initialize_claimed_root(
                            ticket.reservation().clone(),
                            ticket.claim().clone(),
                            self.resources.clone(),
                            self.scope.clone(),
                            &|| runtime.repository_materializer.session_is_active(token),
                        ))
                        .map_err(physical)?,
                );
                self.tickets.insert(demand.key().clone(), ticket);
                advanced = true;
            }
            for demand in needs.workspace_operations().values() {
                let before = self
                    .command
                    .as_ref()
                    .ok_or_else(|| refusal("workspace command was consumed"))?
                    .epoch()
                    .transcripts()
                    .unwrap()
                    .iter()
                    .find(|transcript| transcript.reservation() == demand.request().reservation())
                    .ok_or_else(|| refusal("workspace operation has no transcript"))?
                    .records()
                    .len();
                let command = self
                    .command
                    .take()
                    .ok_or_else(|| refusal("workspace command was consumed"))?;
                self.command = Some(
                    if matches!(
                        demand.request().operation(),
                        RepositoryWorkspaceOperation::Execute(_)
                    ) {
                        #[cfg(test)]
                        runtime
                            .native_demand_sessions
                            .mark_workspace_stage("execute");
                        let selected = self.selected(demand)?;
                        runtime
                            .runtime
                            .block_on(runtime.repository_materializer.execute_workspace(
                                token,
                                command,
                                demand.request().as_ref().clone(),
                                &selected.requests,
                                &selected.epoch,
                                &self.scope,
                                &|| runtime.repository_materializer.session_is_active(token),
                            ))
                            .map_err(physical)?
                    } else {
                        #[cfg(test)]
                        runtime
                            .native_demand_sessions
                            .mark_workspace_stage("trusted_operation");
                        command
                            .apply(
                                demand.request().as_ref().clone(),
                                &runtime.runtime,
                                distdirs,
                                &self.scope,
                                &|| runtime.repository_materializer.session_is_active(token),
                            )
                            .map_err(physical)?
                    },
                );
                advanced |= self
                    .command
                    .as_ref()
                    .unwrap()
                    .epoch()
                    .transcripts()
                    .unwrap()
                    .iter()
                    .find(|transcript| transcript.reservation() == demand.request().reservation())
                    .unwrap()
                    .records()
                    .len()
                    > before;
            }
            if !advanced {
                return Err(refusal("workspace frontier made no progress"));
            }
            Ok(())
        }
        pub(super) fn finish(
            &mut self,
            runtime: &WorkspaceRuntime,
            token: super::super::super::repository_io::RepositorySessionToken,
            request: Arc<RepositoryMaterializationRequest>,
            generation: RepositoryMaterializationGeneration,
        ) -> Result<RepositoryMaterializationResultEpoch, NativeDemandSessionError> {
            let RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind else {
                return Err(refusal("workspace terminal kind mismatch"));
            };
            let ticket = self.ticket(realization.reservation())?;
            if ticket.repo_spec().as_ref() != &request.repo_spec {
                return Err(refusal("workspace terminal spec differs from ticket"));
            }
            let key = ticket.claim().key().clone();
            #[cfg(test)]
            runtime
                .native_demand_sessions
                .mark_workspace_stage("finalization");
            let command = self
                .command
                .take()
                .ok_or_else(|| refusal("workspace command was consumed"))?;
            let (command, sealed) = runtime
                .runtime
                .block_on(command.finalize_workspace(request, &self.scope, &|| {
                    runtime.repository_materializer.session_is_active(token)
                }))
                .map_err(physical)?;
            self.command = Some(command);
            #[cfg(test)]
            runtime
                .native_demand_sessions
                .mark_workspace_stage("adoption");
            runtime
                .repository_materializer
                .adopt_workspace(
                    token,
                    self.tickets.get(&key).expect("retained reservation ticket"),
                    generation,
                    sealed,
                    &self.scope,
                )
                .map_err(NativeDemandSessionError::Repository)
        }
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[path = "repository_workspace_driver/tests.rs"]
mod tests;
