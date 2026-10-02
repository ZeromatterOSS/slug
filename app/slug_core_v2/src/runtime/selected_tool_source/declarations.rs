//! A finite catalogue minted only from current normal Label selectors.
use slug_bzlmod_v2::ObservedSelectedLabelSource;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ObservedInputImage;

use super::*;
use crate::runtime::dice::input_image_bridge::DeclaredImageRequest;
use crate::runtime::dice::input_image_bridge::{self};

pub(in crate::runtime::dice) struct Catalog {
    entries: [Option<DeclaredImageRequest>; 3],
    // New fixed owner/header is charged for the entire declaration lifetime.
    _credit: slug_workspace_v2::ImageDescriptorReservation,
}
impl Catalog {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        plan: &ObservedSelectedExtensionToolSourceRequests,
        deadline: Instant,
    ) -> Result<Self, NativeDemandSessionError> {
        let credit = runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<Self>() + 256)
            .map_err(image_failure)?;
        let mut entries = std::array::from_fn(|_| None);
        for (index, request) in plan.requests().enumerate() {
            if let Some(selected) = request.label_selection() {
                entries[index] = Some(input_image_bridge::declared_selected_image(
                    runtime,
                    selected.dupe(),
                    deadline,
                )?);
            }
        }
        Ok(Self {
            entries,
            _credit: credit,
        })
    }
    pub(in crate::runtime::dice) fn declare_elf(
        &mut self,
        runtime: &WorkspaceRuntime,
        index: usize,
        selected: &ObservedSelectedLabelSource,
        image: ObservedInputImage,
        policy: ElfMetadataPolicy,
    ) -> Result<(), NativeDemandSessionError> {
        input_image_bridge::declare_selected_elf(
            runtime,
            self.entries[index]
                .as_mut()
                .ok_or_else(|| refusal("missing Label declaration"))?,
            selected,
            image,
            policy,
        )
    }
    pub(in crate::runtime::dice) fn preflight(
        &self,
        command: &NativeDemandCommand<'_>,
        demands: &[PathObservationDemand],
    ) -> Result<(), NativeDemandSessionError> {
        let Some(first) = self.entries.iter().flatten().next() else {
            return input_image_bridge::preflight_declared_image_paths(command, &[], demands);
        };
        let mut entries: [DeclaredImageRequest; 3] = std::array::from_fn(|_| first.clone());
        let mut count = 0;
        for entry in self.entries.iter().flatten() {
            entries[count] = entry.clone();
            count += 1;
        }
        input_image_bridge::preflight_declared_image_paths(command, &entries[..count], demands)
    }
    pub(in crate::runtime::dice) fn observe(
        &self,
        command: &NativeDemandCommand<'_>,
        demands: Vec<PathObservationDemand>,
        deadline: Instant,
    ) -> Result<PathObservationEpoch, NativeDemandSessionError> {
        let first = self.entries.iter().flatten().next();
        let active = || selected_scratch_source::active(command, deadline).is_ok();
        let Some(first) = first else {
            return input_image_bridge::observe_declared_image_paths(
                command,
                &[],
                demands,
                &active,
            );
        };
        let mut entries: [DeclaredImageRequest; 3] = std::array::from_fn(|_| first.clone());
        let mut count = 0;
        for entry in self.entries.iter().flatten() {
            entries[count] = entry.clone();
            count += 1;
        }
        input_image_bridge::observe_declared_image_paths(
            command,
            &entries[..count],
            demands,
            &active,
        )
    }
    pub(in crate::runtime::dice) fn capture(
        &self,
        command: &NativeDemandCommand<'_>,
        index: usize,
        image: ObservedInputImage,
        stage: input_image::PreparedInputImageStage,
        deadline: Instant,
    ) -> Result<input_image::CapturedInputImageLease, NativeDemandSessionError> {
        input_image_bridge::capture_selected_image(
            command,
            self.entries[index]
                .as_ref()
                .ok_or_else(|| refusal("missing Label capture declaration"))?,
            image,
            stage,
            &|| selected_scratch_source::active(command, deadline).is_ok(),
        )
    }
}
