//! One command lifecycle for Tools and configured Runtime preparation.
use super::super::runtime_sources;
use super::*;

pub(in crate::runtime::dice) enum PreparationMode {
    Tools,
    Runtime {
        key: slug_loading_v2::RuntimeSourceProfileKey,
        policy: slug_bzlmod_v2::RuntimeSourceDeclarationPolicy,
    },
}
pub(in crate::runtime::dice) enum PreparedLease {
    Tools(CertifiedSelectedToolInputsLease),
    Runtime(runtime_sources::CertifiedRuntimeSourcesLease),
}
impl WorkspaceRuntime {
    pub(in crate::runtime::dice) fn prepare_selected_sources_inner(
        &self,
        key: SelectedExtensionToolSourcesKey,
        mode: PreparationMode,
        #[cfg(test)] hook: Option<&dyn Fn(ToolPhase) -> Result<(), NativeDemandSessionError>>,
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
        let mut attempts = key
            .checkpoint()
            .policy()
            .limits()
            .max_operations
            .checked_mul(4)
            .and_then(|n| n.checked_add(self.workspace.components().count()))
            .and_then(|n| n.checked_add(key.checkpoint().policy().limits().max_depth))
            .and_then(|n| n.checked_add(6))
            .ok_or_else(|| refusal("tool attempt bound overflow"))?;
        if let PreparationMode::Runtime { policy, .. } = &mode {
            attempts = attempts
                .checked_add(
                    policy
                        .limits()
                        .max_entries
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(policy.limits().max_depth + 6))
                        .and_then(|n| n.checked_mul(policy.limits().max_declarations))
                        .ok_or_else(|| refusal("runtime attempt bound overflow"))?,
                )
                .ok_or_else(|| refusal("runtime attempt bound overflow"))?;
        }
        // All output directories precede the normal Label ancestor observations.
        let mut scratch = Some(selected_scratch_source::PreparedStage::new(self)?);
        let mut image_stages: [Option<input_image::PreparedInputImageStage>; 3] =
            std::array::from_fn(|_| None);
        for stage in &mut image_stages {
            *stage = Some(
                input_image::prepare_input_image_stage(&self.image_resources)
                    .map_err(NativeDemandSessionError::InputImage)?,
            );
        }
        let owner_credit = self
            .scratch_resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<CertifiedToolInputOwner>() + 256)
            .map_err(image_failure)?;
        let mut runtime_storage = match &mode {
            PreparationMode::Tools => None,
            PreparationMode::Runtime { .. } => Some(runtime_sources::prepare::Storage::new(self)?),
        };
        let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
        if let PreparationMode::Runtime { policy, .. } = &mode {
            inputs.runtime_sources = Some(policy.dupe());
        }
        let mut guard = NativeDemandAbortGuard::new(
            self.begin_native_demand_command_with_inputs(inputs)?
                .into_command(),
        );
        let result = (|| {
            for _ in 0..attempts {
                selected_scratch_source::active(guard.command(), deadline)?;
                guard.begin_attempt()?;
                let mut runtime_plan = None;
                let plan_attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
                        match &mode {
                            PreparationMode::Tools => {
                                attempt::plan(self, &mut guard, &key, deadline).await
                            }
                            PreparationMode::Runtime { key, .. } => {
                                match runtime_sources::attempt::plan(
                                    self, &mut guard, key, deadline,
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
                self.runtime.block_on(self.dice.wait_for_idle());
                let (plan, plan_demands) = match plan_attempt? {
                    attempt::Attempt::Need(need) => {
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
                        if let (PreparationMode::Runtime { key, .. }, Some(runtime_plan)) =
                            (&mode, &runtime_plan)
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
                let mut catalog = declarations::Catalog::new(self, &plan, deadline)?;
                let runtime_catalog = if let (PreparationMode::Runtime { key, .. }, Some(plan)) =
                    (&mode, &runtime_plan)
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
                                )
                                .await
                            }
                            PreparationMode::Runtime { key, .. } => {
                                match runtime_sources::attempt::sources(
                                    self,
                                    &mut guard,
                                    key,
                                    runtime_plan.as_ref().expect("normal runtime plan"),
                                    &mut catalog,
                                    deadline,
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
                self.runtime.block_on(self.dice.wait_for_idle());
                validate_plan(guard.command(), &key, &plan, &plan_demands, deadline)?;
                match outcome? {
                    attempt::Attempt::Need(need) => {
                        if let PreparationMode::Runtime { key, .. } = &mode {
                            runtime_sources::prepare::progress_sources(
                                &mut guard,
                                &need,
                                key,
                                runtime_plan.as_ref().expect("normal runtime plan"),
                                &plan_demands,
                                &catalog,
                                runtime_catalog.as_ref().expect("runtime catalogue"),
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
                        let reachable = selected.sidecars().demands();
                        validate_final(
                            guard.command(),
                            &key,
                            &plan,
                            &certificate,
                            reachable,
                            deadline,
                        )?;
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(ToolPhase::BeforeCapture)?;
                        }
                        verify_final(
                            guard.command(),
                            &key,
                            &plan,
                            &certificate,
                            reachable,
                            &catalog,
                            deadline,
                        )?;
                        if let (PreparationMode::Runtime { key, .. }, Some(profile)) =
                            (&mode, &runtime_profile)
                        {
                            runtime_sources::prepare::verify_final(
                                guard.command(),
                                key,
                                runtime_plan.as_ref().unwrap(),
                                profile,
                                reachable,
                                &catalog,
                                runtime_catalog.as_ref().unwrap(),
                                deadline,
                            )?;
                            runtime_storage.as_mut().unwrap().capture(
                                guard.command(),
                                profile,
                                runtime_catalog.as_ref().unwrap(),
                            )?;
                        }
                        let mut images: [Option<input_image::CapturedInputImageLease>; 3] =
                            std::array::from_fn(|_| None);
                        let mut image_for_role = [None; 3];
                        for (index, source) in certificate.sources().enumerate() {
                            let Some(metadata) = source.label_metadata() else {
                                continue;
                            };
                            let earlier = certificate.sources().take(index).enumerate().find(
                                |(_, earlier)| {
                                    earlier
                                        .label_metadata()
                                        .is_some_and(|earlier| earlier.image() == metadata.image())
                                },
                            );
                            if let Some((earlier, _)) = earlier {
                                image_for_role[index] = image_for_role[earlier];
                                continue;
                            }
                            images[index] = Some(catalog.capture(
                                guard.command(),
                                index,
                                metadata.image().dupe(),
                                image_stages[index].take().expect("prepared image stage"),
                                deadline,
                            )?);
                            image_for_role[index] = Some(index);
                        }
                        let scratch = scratch.take().expect("prepared scratch stage").capture(
                            guard.command(),
                            key.checkpoint(),
                            certificate.prefix().dupe(),
                            reachable,
                            deadline,
                        )?;
                        #[cfg(test)]
                        if let Some(hook) = hook {
                            hook(ToolPhase::AfterCapture)?;
                        }
                        verify_final(
                            guard.command(),
                            &key,
                            &plan,
                            &certificate,
                            reachable,
                            &catalog,
                            deadline,
                        )?;
                        if let (PreparationMode::Runtime { key, .. }, Some(profile)) =
                            (&mode, &runtime_profile)
                        {
                            runtime_sources::prepare::verify_final(
                                guard.command(),
                                key,
                                runtime_plan.as_ref().unwrap(),
                                profile,
                                reachable,
                                &catalog,
                                runtime_catalog.as_ref().unwrap(),
                                deadline,
                            )?;
                        }
                        let (_, _, terminal) = selected.sidecars.into_parts();
                        terminal
                            .reset_to_idle()
                            .map_err(NativeDemandSessionError::Effect)?;
                        let tools = owner_credit
                            .freeze(CertifiedToolInputOwner {
                                certificate,
                                scratch,
                                images,
                                image_for_role,
                                unused_stages: image_stages,
                            })
                            .map(CertifiedSelectedToolInputsLease)
                            .map_err(image_failure)?;
                        return if let Some(profile) = runtime_profile {
                            runtime_storage
                                .take()
                                .expect("runtime prepared storage")
                                .finish(profile, tools)
                                .map(PreparedLease::Runtime)
                        } else {
                            Ok(PreparedLease::Tools(tools))
                        };
                    }
                }
            }
            Err(refusal("tool preparation made no progress"))
        })();
        // Underlying source stages/certificates remain alive until restoration.
        let suppression = guard.suppress_attempt();
        let restoration = guard
            .command
            .as_mut()
            .expect("tool command")
            .discard_in_place();
        match restoration {
            Ok(()) => {
                guard.phase = NativeDemandAbortPhase::Closed;
                guard.command = None;
                self.runtime.block_on(self.dice.wait_for_idle());
                suppression?;
                if Instant::now() >= deadline {
                    return Err(refusal("tool preparation cancelled"));
                }
                result
            }
            Err(error) => {
                guard.phase = NativeDemandAbortPhase::FailClosed;
                Err(error)
            }
        }
    }
}
