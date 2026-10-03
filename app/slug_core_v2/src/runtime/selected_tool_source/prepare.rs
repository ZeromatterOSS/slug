//! One command lifecycle for Tools and configured Runtime preparation.
use super::super::guest_namespace;
use super::super::runtime_sources;
use super::super::selected_label_data;
use super::*;
#[path = "prepare/capture.rs"]
mod capture;
#[path = "prepare/setup.rs"]
pub(in crate::runtime::dice) mod setup;

pub(in crate::runtime::dice) enum PreparationMode {
    Tools,
    Namespace {
        key: slug_loading_v2::SelectedGuestNamespaceFactsKey,
        runtime_key: slug_loading_v2::RuntimeSourceProfileKey,
        runtime_policy: slug_bzlmod_v2::RuntimeSourceDeclarationPolicy,
        guest_policy: slug_bzlmod_v2::GuestNamespacePolicy,
    },
    Runtime {
        key: slug_loading_v2::RuntimeSourceProfileKey,
        policy: slug_bzlmod_v2::RuntimeSourceDeclarationPolicy,
    },
}
pub(in crate::runtime::dice) enum PreparedLease {
    Tools(CertifiedSelectedToolInputsLease),
    Namespace(guest_namespace::PreparedGuestNamespaceViewLease),
    Runtime(runtime_sources::CertifiedRuntimeSourcesLease),
}
impl WorkspaceRuntime {
    pub(in crate::runtime::dice) fn prepare_selected_sources_inner(
        &self,
        key: SelectedExtensionToolSourcesKey,
        mode: PreparationMode,
        #[cfg(test)] hook: Option<&dyn Fn(ToolPhase<'_>) -> Result<(), NativeDemandSessionError>>,
    ) -> Result<PreparedLease, NativeDemandSessionError> {
        if key.checkpoint().workspace().as_path() != self.workspace {
            return Err(refusal("foreign tool workspace"));
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(NativeDemandSessionError::InputImage(
                input_image::InputImageCaptureError::UnsupportedPlatform,
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let (runtime_policy, guest_policy) = match &mode {
            PreparationMode::Tools => (None, None),
            PreparationMode::Runtime { policy, .. } => (Some(policy), None),
            PreparationMode::Namespace {
                runtime_policy,
                guest_policy,
                ..
            } => (Some(runtime_policy), Some(*guest_policy)),
        };
        let stages = setup::Stages::new(
            self,
            *key.checkpoint().policy(),
            runtime_policy,
            guest_policy,
        )?;
        let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
        if let PreparationMode::Namespace { guest_policy, .. } = &mode {
            inputs.guest_namespace = Some(*guest_policy);
        }
        if let PreparationMode::Runtime { policy, .. }
        | PreparationMode::Namespace {
            runtime_policy: policy,
            ..
        } = &mode
        {
            inputs.runtime_sources = Some(policy.dupe());
        }
        let guard = NativeDemandAbortGuard::new(
            self.begin_native_demand_command_with_inputs(inputs)?
                .into_command(),
        );
        self.run_selected_sources(
            key,
            mode,
            stages,
            guard,
            None,
            deadline,
            #[cfg(test)]
            hook,
        )
    }

    pub(in crate::runtime::dice) fn run_selected_sources(
        &self,
        key: SelectedExtensionToolSourcesKey,
        mode: PreparationMode,
        mut stages: setup::Stages,
        mut guard: NativeDemandAbortGuard<'_>,
        mut owner: Option<&mut super::super::source_preparation::OwnerAuthorization<'_>>,
        deadline: Instant,
        #[cfg(test)] hook: Option<&dyn Fn(ToolPhase<'_>) -> Result<(), NativeDemandSessionError>>,
    ) -> Result<PreparedLease, NativeDemandSessionError> {
        let result = (|| {
            for _ in 0..stages.attempts {
                selected_scratch_source::active(guard.command(), deadline)?;
                if let Some(owner) = owner.as_deref_mut() {
                    guard.begin_attempt()?;
                    let fresh = self.runtime.block_on(async {
                        tokio::time::timeout_at(
                            tokio::time::Instant::from_std(deadline),
                            super::super::source_preparation::attempt::owner(
                                self, &mut guard, &owner.key, deadline,
                            ),
                        )
                        .await
                        .map_err(|_| refusal("source owner preparation cancelled"))?
                    });
                    self.wait_source_idle()?;
                    match fresh? {
                        attempt::Attempt::Need(need) => {
                            #[cfg(test)]
                            if let Some(hook) = hook {
                                hook(ToolPhase::BeforeProgress(&need))?;
                            }
                            progress_baseline(&mut guard, &need, deadline)?;
                            continue;
                        }
                        attempt::Attempt::Complete(fresh, selected) => {
                            if fresh.owner() != owner.observed.owner()
                                || fresh.requested() != owner.observed.requested()
                            {
                                return Err(refusal("selected source ownership changed"));
                            }
                            let (_, _, terminal) = selected.sidecars.into_parts();
                            terminal
                                .reset_to_idle()
                                .map_err(NativeDemandSessionError::Effect)?;
                            #[cfg(test)]
                            if let Some(hook) = hook {
                                hook(ToolPhase::OwnerRefreshed(&owner.observed, &fresh))?;
                            }
                            owner.observed = fresh;
                        }
                    }
                }
                guard.begin_attempt()?;
                let mut runtime_plan = None;
                let mut data_plan = None;
                let plan_attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
                        match &mode {
                            PreparationMode::Tools => {
                                attempt::plan(self, &mut guard, &key, deadline, owner.as_deref())
                                    .await
                            }
                            PreparationMode::Runtime { key, .. }
                            | PreparationMode::Namespace {
                                runtime_key: key, ..
                            } => {
                                match runtime_sources::attempt::plan(
                                    self,
                                    &mut guard,
                                    key,
                                    deadline,
                                    owner.as_deref(),
                                )
                                .await?
                                {
                                    attempt::Attempt::Need(need) => {
                                        Ok(attempt::Attempt::Need(need))
                                    }
                                    attempt::Attempt::Complete(plan, selected) => {
                                        let tools = plan.tool_requests().dupe();
                                        runtime_plan = Some(plan);
                                        Ok(attempt::Attempt::Complete(tools, selected))
                                    }
                                }
                            }
                        }
                    })
                    .await
                    .map_err(|_| refusal("tool preparation cancelled"))?
                });
                self.wait_source_idle()?;
                let (plan, mut plan_demands) = match plan_attempt? {
                    attempt::Attempt::Need(need) => {
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(ToolPhase::BeforeProgress(&need))?;
                        }
                        progress_baseline(&mut guard, &need, deadline)?;
                        continue;
                    }
                    attempt::Attempt::Complete(plan, selected) => {
                        validate_plan(
                            guard.command(),
                            &key,
                            &plan,
                            selected.sidecars().demands(),
                            deadline,
                        )?;
                        if let (
                            PreparationMode::Runtime { key, .. }
                            | PreparationMode::Namespace {
                                runtime_key: key, ..
                            },
                            Some(runtime_plan),
                        ) = (&mode, &runtime_plan)
                        {
                            runtime_sources::validate_plan(
                                guard.command(),
                                key,
                                runtime_plan,
                                selected.sidecars().demands(),
                                deadline,
                            )?;
                        }
                        let (_, demands, terminal) = selected.sidecars.into_parts();
                        terminal
                            .reset_to_idle()
                            .map_err(NativeDemandSessionError::Effect)?;
                        (plan, demands)
                    }
                };
                let state = ScratchMetadataState::for_plan(
                    &plan,
                    &self.scratch_resources,
                    guard.command().scratch_metadata.as_ref(),
                )?;
                guard
                    .command
                    .as_mut()
                    .expect("live tool command")
                    .scratch_metadata = Some(state);
                if stages.data_storage.is_some() {
                    let runtime_key = match &mode {
                        PreparationMode::Namespace { runtime_key, .. } => runtime_key,
                        _ => unreachable!(),
                    };
                    guard.begin_attempt()?;
                    let next = self.runtime.block_on(async {
                        tokio::time::timeout_at(
                            tokio::time::Instant::from_std(deadline),
                            selected_label_data::plan(
                                self,
                                &mut guard,
                                runtime_key,
                                deadline,
                                owner.as_deref(),
                            ),
                        )
                        .await
                        .map_err(|_| refusal("data preparation cancelled"))?
                    });
                    self.wait_source_idle()?;
                    match next? {
                        attempt::Attempt::Need(need) => {
                            #[cfg(test)]
                            if let Some(hook) = hook {
                                hook(ToolPhase::BeforeProgress(&need))?;
                            }
                            progress_baseline(&mut guard, &need, deadline)?;
                            continue;
                        }
                        attempt::Attempt::Complete(data, selected) => {
                            if data.runtime_requests() != runtime_plan.as_ref().unwrap() {
                                return Err(refusal("data runtime plan changed"));
                            }
                            selected_label_data::validate_requests(
                                guard.command(),
                                runtime_key,
                                &data,
                                selected.sidecars().demands(),
                                deadline,
                            )?;
                            let (_, demands, terminal) = selected.sidecars.into_parts();
                            terminal
                                .reset_to_idle()
                                .map_err(NativeDemandSessionError::Effect)?;
                            plan_demands = demands;
                            data_plan = Some(data);
                        }
                    }
                }
                let data_catalog = if let Some(data) = &data_plan {
                    let runtime_key = match &mode {
                        PreparationMode::Namespace { runtime_key, .. } => runtime_key,
                        _ => unreachable!(),
                    };
                    Some(selected_label_data::catalogue::Catalogue::new(
                        self,
                        runtime_key,
                        data,
                        &plan_demands,
                        deadline,
                    )?)
                } else {
                    None
                };
                let mut catalog = declarations::Catalog::new(self, &plan, deadline)?;
                let runtime_catalog = if let (
                    PreparationMode::Runtime { key, .. }
                    | PreparationMode::Namespace {
                        runtime_key: key, ..
                    },
                    Some(plan),
                ) = (&mode, &runtime_plan)
                {
                    Some(runtime_sources::catalogue::Catalogue::new(
                        self,
                        key,
                        plan,
                        &plan_demands,
                        deadline,
                    )?)
                } else {
                    None
                };
                let mut runtime_profile = None;
                let mut namespace_facts = None;
                guard.begin_attempt()?;
                let outcome = self.runtime.block_on(async {
                    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
                        match &mode {
                            PreparationMode::Tools => {
                                attempt::sources(
                                    self,
                                    &mut guard,
                                    &key,
                                    &plan,
                                    &mut catalog,
                                    deadline,
                                    owner.as_deref(),
                                )
                                .await
                            }
                            PreparationMode::Namespace { key, .. } => {
                                match guest_namespace::attempt::sources(
                                    self,
                                    &mut guard,
                                    key,
                                    runtime_plan.as_ref().expect("normal runtime plan"),
                                    &mut catalog,
                                    deadline,
                                    owner.as_deref(),
                                )
                                .await?
                                {
                                    attempt::Attempt::Need(need) => {
                                        Ok(attempt::Attempt::Need(need))
                                    }
                                    attempt::Attempt::Complete(facts, selected) => {
                                        let tools = facts.profile().tool_sources().dupe();
                                        runtime_profile = Some(facts.profile().dupe());
                                        namespace_facts = Some(facts);
                                        Ok(attempt::Attempt::Complete(tools, selected))
                                    }
                                }
                            }
                            PreparationMode::Runtime { key, .. } => {
                                match runtime_sources::attempt::sources(
                                    self,
                                    &mut guard,
                                    key,
                                    runtime_plan.as_ref().expect("normal runtime plan"),
                                    &mut catalog,
                                    deadline,
                                    owner.as_deref(),
                                )
                                .await?
                                {
                                    attempt::Attempt::Need(need) => {
                                        Ok(attempt::Attempt::Need(need))
                                    }
                                    attempt::Attempt::Complete(profile, selected) => {
                                        let tools = profile.tool_sources().dupe();
                                        runtime_profile = Some(profile);
                                        Ok(attempt::Attempt::Complete(tools, selected))
                                    }
                                }
                            }
                        }
                    })
                    .await
                    .map_err(|_| refusal("tool preparation cancelled"))?
                });
                self.wait_source_idle()?;
                validate_plan(guard.command(), &key, &plan, &plan_demands, deadline)?;
                match outcome? {
                    attempt::Attempt::Need(need) => {
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(ToolPhase::BeforeProgress(&need))?;
                        }
                        if let PreparationMode::Runtime { key, .. }
                        | PreparationMode::Namespace {
                            runtime_key: key, ..
                        } = &mode
                        {
                            runtime_sources::prepare::progress_sources_with_data(
                                &mut guard,
                                &need,
                                key,
                                runtime_plan.as_ref().expect("normal runtime plan"),
                                &plan_demands,
                                &catalog,
                                runtime_catalog.as_ref().expect("runtime catalogue"),
                                data_catalog.as_ref(),
                                deadline,
                            )?;
                        } else {
                            progress_sources(
                                &mut guard,
                                &need,
                                &key,
                                &plan,
                                &plan_demands,
                                &catalog,
                                deadline,
                            )?;
                        }
                    }
                    attempt::Attempt::Complete(certificate, selected) => {
                        return capture::Completion {
                            mode: &mode,
                            key: &key,
                            plan: &plan,
                            runtime_plan: runtime_plan.as_ref(),
                            runtime_profile,
                            namespace_facts,
                            data_plan: data_plan.as_ref(),
                            catalog: &catalog,
                            runtime_catalog: runtime_catalog.as_ref(),
                            data_catalog: data_catalog.as_ref(),
                            certificate,
                            selected,
                            owner: owner.as_deref(),
                            deadline,
                            #[cfg(test)]
                            hook,
                        }
                        .complete(self, guard.command(), &mut stages);
                    }
                }
            }
            Err(refusal("tool preparation made no progress"))
        })();
        let restored = finalize_with_hook(
            self,
            &mut guard,
            result,
            deadline,
            #[cfg(test)]
            hook,
            #[cfg(test)]
            Some(&stages),
        );
        // This includes unsuccessful cells, unused stages and completed partial
        // leases. Release only after discard/restoration and its DICE drain.
        drop(stages);
        restored
    }
}

fn data_catalog_key(mode: &PreparationMode) -> &slug_loading_v2::RuntimeSourceProfileKey {
    match mode {
        PreparationMode::Namespace { runtime_key, .. } => runtime_key,
        _ => unreachable!(),
    }
}

pub(in crate::runtime::dice) fn finalize<T>(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    result: Result<T, NativeDemandSessionError>,
    deadline: Instant,
) -> Result<T, NativeDemandSessionError> {
    finalize_with_hook(
        runtime,
        guard,
        result,
        deadline,
        #[cfg(test)]
        None,
        #[cfg(test)]
        None,
    )
}

fn finalize_with_hook<T>(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    result: Result<T, NativeDemandSessionError>,
    deadline: Instant,
    #[cfg(test)] hook: Option<&dyn Fn(ToolPhase<'_>) -> Result<(), NativeDemandSessionError>>,
    #[cfg(test)] stages: Option<&setup::Stages>,
) -> Result<T, NativeDemandSessionError> {
    // Underlying source stages/certificates remain alive until restoration.
    let suppression = guard.suppress_attempt();
    #[cfg(test)]
    let before = match (hook, stages) {
        (Some(hook), Some(stages)) => hook(ToolPhase::BeforeRestore(stages)),
        _ => Ok(()),
    };
    let restoration = guard
        .command
        .as_mut()
        .expect("tool command")
        .discard_in_place();
    match restoration {
        Ok(()) => {
            guard.phase = NativeDemandAbortPhase::Closed;
            guard.command = None;
            let drained = runtime.wait_source_idle();
            #[cfg(test)]
            let after = match (hook, stages) {
                (Some(hook), Some(stages)) => hook(ToolPhase::AfterRestore(stages)),
                _ => Ok(()),
            };
            // Scoped cleanup is attempted even after acquisition failure. The
            // primary refusal remains authoritative if cleanup also fails.
            if runtime.source_scope().is_some() && result.is_err() {
                return result;
            }
            drained?;
            suppression?;
            #[cfg(test)]
            {
                before?;
                after?;
            }
            if result.is_ok() || runtime.source_scope().is_none() {
                runtime.check_source_scope()?;
                if Instant::now() >= deadline {
                    return Err(refusal("tool preparation cancelled"));
                }
            }
            result
        }
        Err(error) => {
            guard.phase = NativeDemandAbortPhase::FailClosed;
            let drained = runtime.wait_source_idle();
            if runtime.source_scope().is_some() && result.is_err() {
                return result;
            }
            drained?;
            Err(error)
        }
    }
}
