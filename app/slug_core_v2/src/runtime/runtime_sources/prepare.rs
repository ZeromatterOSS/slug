//! Command-local runtime storage and combined admission; no nested preparation.
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

pub(in crate::runtime::dice) struct Storage {
    stages: [Option<input_image::PreparedInputImageStage>; 64],
    images: [Option<input_image::CapturedInputImageLease>; 64],
    failed_owner: Option<RuntimeInputOwner>,
    credit: Option<ImageDescriptorReservation>,
}
impl Storage {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        admitted_len: usize,
    ) -> Result<Self, NativeDemandSessionError> {
        if admitted_len > 64 {
            return Err(refusal("runtime stage count overflow"));
        }
        // Both fixed slot arrays and the final owner are admitted BEFORE any
        // stage allocation. Their ownership moves into the final retained block.
        let credit = runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(
                std::mem::size_of::<Self>() + std::mem::size_of::<RuntimeInputOwner>() + 256,
            )
            .map_err(image_error)?;
        let mut stages = std::array::from_fn(|_| None);
        for slot in &mut stages[..admitted_len] {
            *slot = Some(
                input_image::prepare_input_image_stage(&runtime.image_resources)
                    .map_err(NativeDemandSessionError::InputImage)?,
            );
        }
        Ok(Self {
            stages,
            images: std::array::from_fn(|_| None),
            failed_owner: None,
            credit: Some(credit),
        })
    }
    pub(in crate::runtime::dice) fn capture(
        &mut self,
        command: &NativeDemandCommand<'_>,
        profile: &ObservedRuntimeSourceProfile,
        catalog: &catalogue::Catalogue,
        tools: &selected_tool_source::CertifiedSelectedToolInputsLease,
    ) -> Result<(), NativeDemandSessionError> {
        if tools.certificate() != profile.tool_sources() {
            return Err(refusal("runtime capture has foreign tools"));
        }
        for (index, image) in profile.images().enumerate() {
            match profile
                .image_binding(index)
                .ok_or_else(|| refusal("missing runtime binding"))?
            {
                slug_loading_v2::RuntimeSourceImageBinding::Captured => {}
                slug_loading_v2::RuntimeSourceImageBinding::CoveredTool { .. } => {
                    // Normal interval proof plus actual retained certificate binds the FD.
                    let root = profile
                        .tool_root_member(index)
                        .ok_or_else(|| refusal("missing covered runtime root"))?;
                    let matched = tools
                        .with_image_root(root.0, |_, actual| {
                            actual.entries().get(root.1).is_some_and(|e| {
                                e.path().as_path() == image.source_root().as_path()
                            }) && actual.namespace() == image.namespace()
                                && actual.anchor_root() == image.anchor_root()
                        })
                        .unwrap_or(false);
                    if !matched {
                        return Err(refusal("covered runtime capture root changed"));
                    }
                    continue;
                }
            }
            self.images[index] = Some(
                catalog.capture(
                    command,
                    index,
                    image.dupe(),
                    self.stages[index]
                        .as_mut()
                        .expect("precreated runtime stage"),
                )?,
            );
            self.stages[index].take();
        }
        Ok(())
    }
    #[cfg(test)]
    pub(in crate::runtime::dice) fn visit_stage_paths(
        &self,
        visitor: &mut dyn FnMut(&std::path::Path),
    ) {
        for stage in self.stages.iter().flatten() {
            visitor(stage.test_path());
        }
        for image in self.images.iter().flatten() {
            visitor(image.test_path());
        }
        if let Some(owner) = &self.failed_owner {
            for stage in owner.unused.iter().flatten() {
                visitor(stage.test_path());
            }
            for image in owner.images.iter().flatten() {
                visitor(image.test_path());
            }
        }
    }
    pub(in crate::runtime::dice) fn finish(
        &mut self,
        profile: ObservedRuntimeSourceProfile,
        tools: selected_tool_source::CertifiedSelectedToolInputsLease,
    ) -> Result<CertifiedRuntimeSourcesLease, NativeDemandSessionError> {
        let result = self
            .credit
            .take()
            .ok_or_else(|| refusal("capture storage already consumed"))?
            .freeze_preserving(RuntimeInputOwner {
                profile,
                tools,
                images: std::mem::replace(&mut self.images, std::array::from_fn(|_| None)),
                unused: std::mem::replace(&mut self.stages, std::array::from_fn(|_| None)),
            });
        match result {
            Ok(block) => Ok(CertifiedRuntimeSourcesLease(block)),
            Err((error, owner, credit)) => {
                self.failed_owner = Some(owner);
                self.credit = Some(credit);
                Err(image_error(error))
            }
        }
    }
}

pub(in crate::runtime::dice) fn observe_combined(
    command: &NativeDemandCommand<'_>,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &catalogue::Catalogue,
    demands: Vec<PathObservationDemand>,
    deadline: std::time::Instant,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    observe_combined_with_data(command, tool, runtime, None, demands, deadline)
}

pub(in crate::runtime::dice) fn observe_combined_with_data(
    command: &NativeDemandCommand<'_>,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &catalogue::Catalogue,
    data: Option<&selected_label_data::catalogue::Catalogue>,
    demands: Vec<PathObservationDemand>,
    deadline: std::time::Instant,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    // Admission and every Vec/path/merge replacement peak precede all factories.
    let _credit = selected_tool_source::demand_credit(command, demands.iter())?;
    let mut data_rows = Vec::with_capacity(if data.is_some() { demands.len() } else { 0 });
    let mut runtime_rows = Vec::with_capacity(demands.len());
    let mut other_rows = Vec::with_capacity(demands.len());
    for demand in demands {
        if data
            .map(|d| d.observes(&demand))
            .transpose()?
            .unwrap_or(false)
        {
            data_rows.push(demand)
        } else if runtime.is_runtime(&demand)? {
            runtime_rows.push(demand)
        } else {
            other_rows.push(demand)
        }
    }
    if let Some(data) = data {
        data.preflight(command, &data_rows)?;
    }
    runtime.preflight(command, &runtime_rows)?;
    tool.preflight(command, &other_rows)?;
    let first = tool.observe(command, other_rows, deadline)?;
    let second = runtime.observe(command, runtime_rows)?;
    let third = data
        .map(|data| data.observe(command, data_rows))
        .transpose()?;
    PathObservationEpoch::from_shared(
        first
            .observations()
            .iter()
            .chain(second.observations().iter())
            .chain(third.iter().flat_map(|epoch| epoch.observations().iter()))
            .map(|(demand, value)| (demand.clone(), value.dupe())),
    )
    .map_err(NativeDemandSessionError::PathEpoch)
}

pub(in crate::runtime::dice) fn verify_final(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedRuntimeSourceRequests,
    profile: &ObservedRuntimeSourceProfile,
    reachable: &SelectedWorkspaceDemands,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &catalogue::Catalogue,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_final(command, key, plan, profile, reachable, deadline)?;
    // Complete inherited Label/prefix freshness is checked before any new root
    // factory, then the full merged profile (tools AND runtime) is reobserved.
    selected_scratch_source::verify_current(
        command,
        key.tool_sources_key().checkpoint(),
        profile.tool_sources().prefix(),
        reachable,
        deadline,
    )?;
    let _credit = selected_tool_source::frontier_credit(command, profile.observations())?;
    let fresh = observe_combined(
        command,
        tool,
        runtime,
        profile
            .observations()
            .observations()
            .keys()
            .cloned()
            .collect(),
        deadline,
    )?;
    if &fresh != profile.observations() {
        return Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource,
        ));
    }
    validate_final(command, key, plan, profile, reachable, deadline)
}

pub(in crate::runtime::dice) fn progress_sources(
    guard: &mut NativeDemandAbortGuard<'_>,
    need: &SourcePreparationNeeds,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedRuntimeSourceRequests,
    reachable: &SelectedWorkspaceDemands,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &catalogue::Catalogue,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    progress_sources_with_data(
        guard, need, key, plan, reachable, tool, runtime, None, deadline,
    )
}

pub(in crate::runtime::dice) fn progress_sources_with_data(
    guard: &mut NativeDemandAbortGuard<'_>,
    need: &SourcePreparationNeeds,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedRuntimeSourceRequests,
    reachable: &SelectedWorkspaceDemands,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &catalogue::Catalogue,
    data: Option<&selected_label_data::catalogue::Catalogue>,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    selected_scratch_source::active(guard.command(), deadline)?;
    if !need.repository_executes().is_empty()
        || !need.repository_archives().is_empty()
        || need.root_module_bootstrap_request().is_some()
        || !need.repository_materializations().is_empty()
        || !need.selected_extension_downloads().is_empty()
        || need.repository_environment().is_some()
    {
        return Err(refusal("runtime consumer emitted unexpected source Need"));
    }
    // Split/admit the entire incoming batch without IO; scratch proof must not
    // start merely because its first request is valid.
    let mut _credit = None;
    if let Some(batch) = need.path_observations() {
        _credit = Some(selected_tool_source::demand_credit(
            guard.command(),
            batch.demands().iter(),
        )?);
        let mut data_rows = Vec::with_capacity(if data.is_some() {
            batch.demands().len()
        } else {
            0
        });
        let mut runtime_rows = Vec::with_capacity(batch.demands().len());
        let mut other = Vec::with_capacity(batch.demands().len());
        for demand in batch.demands() {
            if data
                .map(|d| d.observes(demand))
                .transpose()?
                .unwrap_or(false)
            {
                data_rows.push(demand.clone())
            } else if runtime.is_runtime(demand)? {
                runtime_rows.push(demand.clone())
            } else {
                other.push(demand.clone())
            }
        }
        if let Some(data) = data {
            data.preflight(guard.command(), &data_rows)?;
        }
        runtime.preflight(guard.command(), &runtime_rows)?;
        tool.preflight(guard.command(), &other)?;
    }
    if let Some(batch) = need.scratch_elf_metadata_requests() {
        guard
            .command()
            .scratch_metadata
            .as_ref()
            .ok_or_else(|| refusal("missing scratch authorization"))?
            .preflight(plan.tool_requests(), batch)?;
    }
    validate_plan(guard.command(), key, plan, reachable, deadline)?;
    selected_scratch_source::verify_current(
        guard.command(),
        key.tool_sources_key().checkpoint(),
        plan.tool_requests().prefix(),
        reachable,
        deadline,
    )?;
    if let Some(batch) = need.scratch_elf_metadata_requests() {
        let next = guard.command().scratch_metadata.as_ref().unwrap().fulfill(
            plan.tool_requests(),
            batch,
            &|| selected_scratch_source::active(guard.command(), deadline).is_ok(),
        )?;
        guard.command.as_mut().unwrap().scratch_metadata = Some(next);
        return Ok(());
    }
    if let Some(batch) = need.path_observations() {
        let command = guard.command();
        let fresh = observe_combined_with_data(
            command,
            tool,
            runtime,
            data,
            batch.demands().iter().cloned().collect(),
            deadline,
        )?;
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
            .all(|d| command.path_observations.get(d).is_some())
        {
            return Err(refusal("runtime image made no progress"));
        }
        let _merge = selected_tool_source::frontier_credit(command, &command.path_observations)?;
        let merged = PathObservationEpoch::from_shared(
            command
                .path_observations
                .observations()
                .iter()
                .chain(fresh.observations().iter())
                .map(|(d, v)| (d.clone(), v.dupe())),
        )
        .map_err(NativeDemandSessionError::PathEpoch)?;
        guard.command.as_mut().unwrap().path_observations = merged;
        return Ok(());
    }
    Err(refusal("runtime consumer emitted empty Need"))
}
