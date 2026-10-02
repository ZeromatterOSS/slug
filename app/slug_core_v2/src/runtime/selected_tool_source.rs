//! A single normal command certifies typed tool operands and retains source storage.
//! No runtime closure, process execution or synthetic image namespace is granted.
#![allow(dead_code)] // Private preparation capability, pending an admitted runtime consumer.
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::SelectedScratchElfMetadataEpochKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_loading_v2::ObservedSelectedExtensionToolSourceRequests;
use slug_loading_v2::ObservedSelectedExtensionToolSources;
use slug_loading_v2::SelectedExtensionToolSourcesKey;
use slug_workspace_v2::BudgetedImageBlock;

use super::selected_scratch_source;
use super::*;
use crate::runtime::input_image;

#[path = "selected_tool_source/attempt.rs"]
mod attempt;
#[path = "selected_tool_source/declarations.rs"]
mod declarations;
#[path = "selected_tool_source/scratch_metadata.rs"]
mod scratch_metadata;
pub(super) use scratch_metadata::ScratchMetadataState;
#[cfg(all(test, target_os = "linux"))]
#[path = "selected_tool_source/tests.rs"]
mod tests;

fn refusal(reason: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(reason))
}
fn image_failure(error: slug_workspace_v2::ImageDescriptorBudgetError) -> NativeDemandSessionError {
    NativeDemandSessionError::InputImage(error.into())
}
fn shared_failure(error: slug_bzlmod_v2::SelectedToolSourceError) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::Error::new(error))
}
struct CertifiedToolInputOwner {
    certificate: ObservedSelectedExtensionToolSources,
    scratch: selected_scratch_source::SelectedScratchSourceLease,
    images: [Option<input_image::CapturedInputImageLease>; 3],
    image_for_role: [Option<usize>; 3],
    // Even empty unused stages remain live: deleting one would stale /tmp
    // ancestor facts immediately after the final full-frontier check.
    unused_stages: [Option<input_image::PreparedInputImageStage>; 3],
}
#[derive(Clone, Dupe)]
pub(super) struct CertifiedSelectedToolInputsLease(BudgetedImageBlock<CertifiedToolInputOwner>);
impl fmt::Debug for CertifiedSelectedToolInputsLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertifiedSelectedToolInputsLease")
            .finish_non_exhaustive()
    }
}
impl CertifiedSelectedToolInputsLease {
    #[allow(dead_code)]
    pub(super) fn certificate(&self) -> &ObservedSelectedExtensionToolSources {
        &self.0.get().certificate
    }
}

#[allow(dead_code)]
impl WorkspaceRuntime {
    pub(super) fn prepare_selected_tool_inputs(
        &self,
        key: SelectedExtensionToolSourcesKey,
    ) -> Result<CertifiedSelectedToolInputsLease, NativeDemandSessionError> {
        self.prepare_selected_tool_inputs_inner(
            key,
            #[cfg(test)]
            None,
        )
    }
    fn prepare_selected_tool_inputs_inner(
        &self,
        key: SelectedExtensionToolSourcesKey,
        #[cfg(test)] hook: Option<&dyn Fn(ToolPhase) -> Result<(), NativeDemandSessionError>>,
    ) -> Result<CertifiedSelectedToolInputsLease, NativeDemandSessionError> {
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
        let attempts = key
            .checkpoint()
            .policy()
            .limits()
            .max_operations
            .checked_mul(4)
            .and_then(|n| n.checked_add(self.workspace.components().count()))
            .and_then(|n| n.checked_add(key.checkpoint().policy().limits().max_depth))
            .and_then(|n| n.checked_add(6))
            .ok_or_else(|| refusal("tool attempt bound overflow"))?;
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
        let mut guard =
            NativeDemandAbortGuard::new(self.begin_native_demand_command()?.into_command());
        let result = (|| {
            for _ in 0..attempts {
                selected_scratch_source::active(guard.command(), deadline)?;
                guard.begin_attempt()?;
                let plan_attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        attempt::plan(self, &mut guard, &key, deadline),
                    )
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
                guard.begin_attempt()?;
                let outcome = self.runtime.block_on(async {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        attempt::sources(self, &mut guard, &key, &plan, &mut catalog, deadline),
                    )
                    .await
                    .map_err(|_| refusal("tool preparation cancelled"))?
                });
                self.runtime.block_on(self.dice.wait_for_idle());
                validate_plan(guard.command(), &key, &plan, &plan_demands, deadline)?;
                match outcome? {
                    attempt::Attempt::Need(need) => {
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
                        let (_, _, terminal) = selected.sidecars.into_parts();
                        terminal
                            .reset_to_idle()
                            .map_err(NativeDemandSessionError::Effect)?;
                        return owner_credit
                            .freeze(CertifiedToolInputOwner {
                                certificate,
                                scratch,
                                images,
                                image_for_role,
                                unused_stages: image_stages,
                            })
                            .map(CertifiedSelectedToolInputsLease)
                            .map_err(image_failure);
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

fn validate_plan(
    command: &NativeDemandCommand<'_>,
    key: &SelectedExtensionToolSourcesKey,
    plan: &ObservedSelectedExtensionToolSourceRequests,
    reachable: &SelectedWorkspaceDemands,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    if !plan.matches_key(key.requests_key())
        || !plan.same_resources(&command.runtime.scratch_resources)
    {
        return Err(refusal("foreign current tool plan"));
    }
    selected_scratch_source::validate(
        command,
        key.checkpoint(),
        plan.prefix(),
        reachable,
        deadline,
    )
}
fn validate_final(
    command: &NativeDemandCommand<'_>,
    key: &SelectedExtensionToolSourcesKey,
    plan: &ObservedSelectedExtensionToolSourceRequests,
    certificate: &ObservedSelectedExtensionToolSources,
    reachable: &SelectedWorkspaceDemands,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_plan(command, key, plan, reachable, deadline)?;
    let transaction = command
        .scratch_metadata
        .as_ref()
        .ok_or_else(|| refusal("missing tool authorization"))?
        .transaction()?;
    if !certificate.matches_key(key)
        || certificate.requests() != plan
        || !certificate.same_resources(&command.runtime.scratch_resources)
        || !certificate.authorized_by(&transaction, &command.runtime.scratch_resources)
    {
        return Err(refusal("foreign completed tool certificate"));
    }
    let expected_count = certificate
        .sources()
        .filter(|source| source.scratch_result().is_some())
        .count();
    if reachable.scratch_elf_cells().len() != expected_count {
        return Err(refusal("scratch cell closure differs"));
    }
    for result in certificate
        .sources()
        .filter_map(|source| source.scratch_result())
    {
        let cell = SelectedScratchElfMetadataEpochKey::new(result.request());
        if !reachable.scratch_elf_cells().any(|used| used == &cell)
            || !plan
                .requests()
                .any(|request| request.scratch_request() == Some(result.request()))
            || transaction.get(result.request()) != Some(result)
        {
            return Err(refusal(
                "scratch result lacks exact reachable current proof",
            ));
        }
    }
    for (demand, expected) in certificate.observations().observations() {
        let in_closure = reachable.unscoped_paths().contains(demand)
            || reachable.repository_validations().iter().any(|scope| {
                scope.paths().contains(demand)
                    && reachable.repository_requests().contains(scope.request())
            });
        if !in_closure || command.path_observations.get(demand) != Some(expected) {
            return Err(refusal("tool frontier is not current reachable evidence"));
        }
    }
    Ok(())
}
fn verify_final(
    command: &NativeDemandCommand<'_>,
    key: &SelectedExtensionToolSourcesKey,
    plan: &ObservedSelectedExtensionToolSourceRequests,
    certificate: &ObservedSelectedExtensionToolSources,
    reachable: &SelectedWorkspaceDemands,
    catalog: &declarations::Catalog,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_final(command, key, plan, certificate, reachable, deadline)?;
    let credit = frontier_credit(command, certificate.observations())?;
    let fresh = catalog.observe(
        command,
        certificate
            .observations()
            .observations()
            .keys()
            .map(Clone::clone)
            .collect(),
        deadline,
    )?;
    if &fresh != certificate.observations() {
        return Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource,
        ));
    }
    drop(credit);
    // Revalidate every issued Label session/source root and the whole prefix.
    selected_scratch_source::verify_current(
        command,
        key.checkpoint(),
        certificate.prefix(),
        reachable,
        deadline,
    )?;
    validate_final(command, key, plan, certificate, reachable, deadline)
}
fn frontier_credit(
    command: &NativeDemandCommand<'_>,
    epoch: &PathObservationEpoch,
) -> Result<slug_workspace_v2::ImageDescriptorReservation, NativeDemandSessionError> {
    demand_credit(command, epoch.observations().keys())
}
fn demand_credit<'a>(
    command: &NativeDemandCommand<'_>,
    mut demands: impl Iterator<Item = &'a PathObservationDemand>,
) -> Result<slug_workspace_v2::ImageDescriptorReservation, NativeDemandSessionError> {
    let charge = demands.try_fold(512usize, |total, demand| {
        demand
            .path()
            .as_path()
            .as_os_str()
            .len()
            .checked_add(std::mem::size_of::<(
                PathObservationDemand,
                Arc<PathObservationResult>,
            )>())
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| total.checked_add(n))
            .ok_or_else(|| refusal("tool frontier overflow"))
    })?;
    command
        .runtime
        .scratch_resources
        .descriptor_budget()
        .try_reserve(charge)
        .map_err(image_failure)
}

fn progress_baseline(
    guard: &mut NativeDemandAbortGuard<'_>,
    need: &SourcePreparationNeeds,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    if !need.repository_executes().is_empty() {
        return Err(refusal(
            "tool preparation cannot execute repository processes",
        ));
    }
    if need.scratch_elf_metadata_requests().is_some() {
        return Err(refusal("lower plan emitted scratch ELF Need"));
    }
    let progress = guard.progress(need)?;
    if matches!(
        progress,
        NativeDemandProgress::Repositories | NativeDemandProgress::Downloads
    ) {
        selected_scratch_source::refresh_after_progress(
            guard.command.as_mut().unwrap(),
            deadline,
            #[cfg(test)]
            None,
        )?;
    }
    Ok(())
}
fn progress_sources(
    guard: &mut NativeDemandAbortGuard<'_>,
    need: &SourcePreparationNeeds,
    key: &SelectedExtensionToolSourcesKey,
    plan: &ObservedSelectedExtensionToolSourceRequests,
    reachable: &SelectedWorkspaceDemands,
    catalog: &declarations::Catalog,
    deadline: Instant,
) -> Result<(), NativeDemandSessionError> {
    selected_scratch_source::active(guard.command(), deadline)?;
    // Whole mixed Need authorization precedes the first proof or source read.
    if !need.repository_executes().is_empty()
        || need.root_module_bootstrap_request().is_some()
        || !need.repository_materializations().is_empty()
        || !need.selected_extension_downloads().is_empty()
        || need.repository_environment().is_some()
    {
        return Err(refusal("tool consumer emitted unexpected source Need"));
    }
    if let Some(batch) = need.path_observations() {
        catalog.preflight(guard.command(), batch.demands())?;
    }
    if let Some(batch) = need.scratch_elf_metadata_requests() {
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .unwrap()
            .preflight(plan, batch)?;
    }
    // The entire typed batch has passed authorization. Really reobserve the
    // current normal prefix before any borrowed proof read or Label factory IO.
    selected_scratch_source::verify_current(
        guard.command(),
        key.checkpoint(),
        plan.prefix(),
        reachable,
        deadline,
    )?;
    if let Some(batch) = need.scratch_elf_metadata_requests() {
        // Validate the whole issued batch before any borrowed executable read.
        let next =
            guard
                .command()
                .scratch_metadata
                .as_ref()
                .unwrap()
                .fulfill(plan, batch, &|| {
                    selected_scratch_source::active(guard.command(), deadline).is_ok()
                })?;
        guard.command.as_mut().unwrap().scratch_metadata = Some(next);
        return Ok(());
    }
    if let Some(batch) = need.path_observations() {
        let command = guard.command();
        let _credit = demand_credit(command, batch.demands().iter())?;
        // Observe the whole requested batch; includes any already known rows.
        // Reobserved known rows must agree, never patch a completed frontier.
        let fresh =
            catalog.observe(command, batch.demands().iter().cloned().collect(), deadline)?;
        if fresh.observations().iter().any(|(demand, value)| {
            command
                .path_observations
                .get(demand)
                .is_some_and(|old| old != value)
        }) {
            return Err(NativeDemandSessionError::InputImage(
                input_image::InputImageCaptureError::StaleSource,
            ));
        }
        if fresh
            .observations()
            .keys()
            .all(|demand| command.path_observations.get(demand).is_some())
        {
            return Err(refusal("tool image made no progress"));
        }
        // Existing rows also acquire clone/merge replacement storage. Keep
        // this admission alongside the incoming-batch credit through merge.
        let _merge_credit = frontier_credit(command, &command.path_observations)?;
        let merged = PathObservationEpoch::from_shared(
            command
                .path_observations
                .observations()
                .iter()
                .chain(fresh.observations().iter())
                .map(|(demand, result)| (demand.clone(), result.dupe())),
        )
        .map_err(NativeDemandSessionError::PathEpoch)?;
        guard.command.as_mut().unwrap().path_observations = merged;
        return Ok(());
    }
    // These source-changing branches are not expected after a complete plan;
    // refuse rather than refresh image scopes under obsolete declarations.
    Err(refusal("tool consumer emitted unexpected source Need"))
}
#[cfg(test)]
#[derive(Clone, Copy)]
enum ToolPhase {
    BeforeCapture,
    AfterCapture,
}
