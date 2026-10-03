//! Precreated exact-leaf storage retained beside the existing combined captures.
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

struct Owner {
    sources: ObservedSelectedDataLabelSources,
    captures: [Option<input_image::CapturedInputFileLease>; 64],
    capture_for_occurrence: [Option<usize>; 64],
    _unused: [Option<input_image::PreparedInputImageStage>; 64],
}
#[derive(Clone, Dupe)]
pub(in crate::runtime::dice) struct Lease(BudgetedImageBlock<Owner>);
impl Lease {
    #[cfg(test)]
    pub(in crate::runtime::dice) fn visit_stage_paths(
        &self,
        visitor: &mut dyn FnMut(&std::path::Path),
    ) {
        let owner = self.0.get();
        for stage in owner._unused.iter().flatten() {
            visitor(stage.test_path());
        }
        for image in owner.captures.iter().flatten() {
            let path = image.test_path();
            visitor(path.parent().unwrap());
        }
    }

    pub(in crate::runtime::dice) fn sources(&self) -> &ObservedSelectedDataLabelSources {
        &self.0.get().sources
    }
    pub(in crate::runtime::dice) fn with_leaf<R>(
        &self,
        index: usize,
        visitor: impl FnOnce(&std::fs::File, &std::path::Path, &ObservedInputImage) -> R,
    ) -> Option<R> {
        let owner = self.0.get();
        let slot = *owner.capture_for_occurrence.get(index)?;
        Some(owner.captures.get(slot?)?.as_ref()?.with_read_leaf(visitor))
    }
    #[cfg(test)]
    pub(in crate::runtime::dice) fn test_path(&self, index: usize) -> Option<std::path::PathBuf> {
        let slot = *self.0.get().capture_for_occurrence.get(index)?;
        Some(self.0.get().captures.get(slot?)?.as_ref()?.test_path())
    }
}
pub(in crate::runtime::dice) struct Storage {
    stages: [Option<input_image::PreparedInputImageStage>; 64],
    captures: [Option<input_image::CapturedInputFileLease>; 64],
    capture_for_occurrence: [Option<usize>; 64],
    failed_owner: Option<Owner>,
    credit: Option<ImageDescriptorReservation>,
}
impl Storage {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        max: u8,
    ) -> Result<Self, NativeDemandSessionError> {
        if max == 0 || max > 64 {
            return Err(refusal("invalid data Label storage bound"));
        }
        let credit = runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<Self>() + std::mem::size_of::<Owner>() + 256)
            .map_err(image_error)?;
        let mut stages = std::array::from_fn(|_| None);
        for stage in &mut stages[..usize::from(max)] {
            *stage = Some(
                input_image::prepare_input_image_stage(&runtime.image_resources)
                    .map_err(NativeDemandSessionError::InputImage)?,
            );
        }
        Ok(Self {
            stages,
            captures: std::array::from_fn(|_| None),
            capture_for_occurrence: [None; 64],
            failed_owner: None,
            credit: Some(credit),
        })
    }
    pub(in crate::runtime::dice) fn capture(
        &mut self,
        command: &NativeDemandCommand<'_>,
        data: &ObservedSelectedDataLabelSources,
        catalogue: &catalogue::Catalogue,
    ) -> Result<(), NativeDemandSessionError> {
        for (index, source) in data.sources().enumerate() {
            let SelectedDataLabelBinding::Leaf { image } = source.binding() else {
                continue;
            };
            let request = data
                .requests()
                .requests()
                .nth(index)
                .ok_or_else(|| refusal("data Label source lacks request"))?;
            let earlier = data.sources().take(index).enumerate().find(|(i, s)| {
                matches!(s.binding(),SelectedDataLabelBinding::Leaf{image:old} if old==image)
                    && data
                        .requests()
                        .requests()
                        .nth(*i)
                        .is_some_and(|r| r.selection() == request.selection())
            });
            if let Some((earlier, _)) = earlier {
                self.capture_for_occurrence[index] = self.capture_for_occurrence[earlier];
                continue;
            }
            self.captures[index] = Some(
                catalogue.capture(
                    command,
                    index,
                    image.dupe(),
                    self.stages[index]
                        .as_mut()
                        .ok_or_else(|| refusal("missing precreated data Label stage"))?,
                )?,
            );
            self.stages[index].take();
            self.capture_for_occurrence[index] = Some(index);
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
        for image in self.captures.iter().flatten() {
            let path = image.test_path();
            visitor(path.parent().unwrap());
        }
        if let Some(owner) = &self.failed_owner {
            for stage in owner._unused.iter().flatten() {
                visitor(stage.test_path());
            }
            for image in owner.captures.iter().flatten() {
                let path = image.test_path();
                visitor(path.parent().unwrap());
            }
        }
    }
    pub(in crate::runtime::dice) fn finish(
        &mut self,
        sources: ObservedSelectedDataLabelSources,
    ) -> Result<Lease, NativeDemandSessionError> {
        let result = self
            .credit
            .take()
            .ok_or_else(|| refusal("capture storage already consumed"))?
            .freeze_preserving(Owner {
                sources,
                captures: std::mem::replace(&mut self.captures, std::array::from_fn(|_| None)),
                capture_for_occurrence: self.capture_for_occurrence,
                _unused: std::mem::replace(&mut self.stages, std::array::from_fn(|_| None)),
            });
        match result {
            Ok(block) => Ok(Lease(block)),
            Err((error, owner, credit)) => {
                self.failed_owner = Some(owner);
                self.credit = Some(credit);
                Err(image_error(error))
            }
        }
    }
}

pub(in crate::runtime::dice) fn verify_final(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedSelectedDataLabelRequests,
    data: &ObservedSelectedDataLabelSources,
    reachable: &SelectedWorkspaceDemands,
    tool: &selected_tool_source::declarations::Catalog,
    runtime: &runtime_sources::catalogue::Catalogue,
    catalogue: &catalogue::Catalogue,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_sources(command, key, plan, data, reachable, deadline)?;
    selected_scratch_source::verify_current(
        command,
        key.tool_sources_key().checkpoint(),
        data.profile().tool_sources().prefix(),
        reachable,
        deadline,
    )?;
    let _credit = selected_tool_source::frontier_credit(command, data.observations())?;
    let fresh = runtime_sources::prepare::observe_combined_with_data(
        command,
        tool,
        runtime,
        Some(catalogue),
        data.observations().observations().keys().cloned().collect(),
        deadline,
    )?;
    if &fresh != data.observations() {
        return Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::StaleSource,
        ));
    }
    validate_sources(command, key, plan, data, reachable, deadline)
}
