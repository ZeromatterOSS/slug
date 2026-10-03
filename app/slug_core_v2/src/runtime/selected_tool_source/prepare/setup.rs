//! Stage/resource admission before the first normal owner or Label observation.
use super::*;

pub(in crate::runtime::dice) struct Stages {
    pub(in crate::runtime::dice) attempts: usize,
    pub(super) data_storage: Option<selected_label_data::prepare::Storage>,
    pub(super) scratch: Option<selected_scratch_source::PreparedStage>,
    pub(super) image_stages: [Option<input_image::PreparedInputImageStage>; 3],
    pub(super) runtime_storage: Option<runtime_sources::prepare::Storage>,
    // Successful partial captures outlive the inner closure and common restore.
    pub(super) images: [Option<input_image::CapturedInputImageLease>; 3],
    pub(super) captured_scratch: Option<selected_scratch_source::SelectedScratchSourceLease>,
    pub(super) failed_tools: Option<CertifiedToolInputOwner>,
    pub(super) tools: Option<CertifiedSelectedToolInputsLease>,
    pub(super) runtime: Option<runtime_sources::CertifiedRuntimeSourcesLease>,
    pub(super) data: Option<selected_label_data::prepare::Lease>,
    pub(super) namespace_capture: Option<guest_namespace::ViewCapture>,
    // This covers the fixed outer slots; release it after their owned handles.
    pub(super) owner_credit: Option<slug_workspace_v2::ImageDescriptorReservation>,
}
impl Stages {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        scratch_policy: slug_bzlmod_v2::SelectedScratchPolicy,
        runtime_policy: Option<&slug_bzlmod_v2::RuntimeSourceDeclarationPolicy>,
        guest_policy: Option<slug_bzlmod_v2::GuestNamespacePolicy>,
    ) -> Result<Self, NativeDemandSessionError> {
        let mut attempts = scratch_policy
            .limits()
            .max_operations
            .checked_mul(4)
            .and_then(|n| n.checked_add(runtime.workspace.components().count()))
            .and_then(|n| n.checked_add(scratch_policy.limits().max_depth))
            .and_then(|n| n.checked_add(6))
            .ok_or_else(|| refusal("tool attempt bound overflow"))?;
        if let Some(policy) = runtime_policy {
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
        let data_max = guest_policy.and_then(|policy| match policy.label_data() {
            slug_bzlmod_v2::GuestLabelDataPolicy::Disabled => None,
            slug_bzlmod_v2::GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses } => {
                Some(max_witnesses)
            }
        });
        if let Some(max) = data_max {
            attempts = attempts
                .checked_add(
                    usize::from(max)
                        .checked_mul(
                            runtime
                                .workspace
                                .components()
                                .count()
                                .checked_add(70)
                                .ok_or_else(|| refusal("data attempts overflow"))?,
                        )
                        .ok_or_else(|| refusal("data attempts overflow"))?,
                )
                .ok_or_else(|| refusal("data attempts overflow"))?;
        }
        let owner_credit = runtime
            .scratch_resources
            .descriptor_budget()
            .try_reserve(
                std::mem::size_of::<CertifiedToolInputOwner>()
                    + std::mem::size_of::<Self>()
                    + std::mem::size_of::<super::capture::Completion<'_, '_>>()
                    + 256,
            )
            .map_err(image_failure)?;
        let data_storage = data_max
            .map(|max| selected_label_data::prepare::Storage::new(runtime, max))
            .transpose()?;
        // All output directories precede the normal Label ancestor observations.
        let scratch = Some(selected_scratch_source::PreparedStage::new(runtime)?);
        let mut image_stages: [Option<input_image::PreparedInputImageStage>; 3] =
            std::array::from_fn(|_| None);
        for stage in &mut image_stages {
            *stage = Some(
                input_image::prepare_input_image_stage(&runtime.image_resources)
                    .map_err(NativeDemandSessionError::InputImage)?,
            );
        }
        let runtime_storage = runtime_policy
            .map(|p| runtime_sources::prepare::Storage::new(runtime, p.declarations().len()))
            .transpose()?;
        let namespace_capture = if guest_policy.is_some() {
            Some(guest_namespace::ViewCapture::new(
                guest_namespace::reserve_owner(runtime)?,
            ))
        } else {
            None
        };
        Ok(Self {
            attempts,
            data_storage,
            scratch,
            image_stages,
            owner_credit: Some(owner_credit),
            images: std::array::from_fn(|_| None),
            captured_scratch: None,
            failed_tools: None,
            tools: None,
            runtime: None,
            data: None,
            runtime_storage,
            namespace_capture,
        })
    }
}

#[cfg(test)]
impl Stages {
    pub(in crate::runtime::dice) fn partial_image_count(&self) -> usize {
        self.image_stages
            .iter()
            .flatten()
            .filter(|stage| stage.test_is_partial())
            .count()
    }
    /// A bounded borrowed inspection of actual owned output storage. It grants
    /// no production read capability and performs no source discovery.
    pub(in crate::runtime::dice) fn visit_stage_paths(
        &self,
        visitor: &mut dyn FnMut(&std::path::Path),
    ) {
        if let Some(scratch) = &self.scratch {
            scratch.visit_stage_paths(visitor);
        }
        for stage in self.image_stages.iter().flatten() {
            visitor(stage.test_path());
        }
        for image in self.images.iter().flatten() {
            visitor(image.test_path());
        }
        if let Some(scratch) = &self.captured_scratch {
            visitor(scratch.test_path());
        }
        if let Some(owner) = &self.failed_tools {
            for stage in owner.unused_stages.iter().flatten() {
                visitor(stage.test_path());
            }
            for image in owner.images.iter().flatten() {
                visitor(image.test_path());
            }
            visitor(owner.scratch.test_path());
        }
        if let Some(tools) = &self.tools {
            let owner = tools.0.get();
            for stage in owner.unused_stages.iter().flatten() {
                visitor(stage.test_path());
            }
            for image in owner.images.iter().flatten() {
                visitor(image.test_path());
            }
            visitor(owner.scratch.test_path());
        }
        if let Some(storage) = &self.runtime_storage {
            storage.visit_stage_paths(visitor);
        }
        if let Some(storage) = &self.data_storage {
            storage.visit_stage_paths(visitor);
        }
        // Runtime/Data promotion transfers the physical owners to these pins;
        // they keep successful storage live if later namespace checks refuse.
        if let Some(lease) = &self.runtime {
            lease.visit_stage_paths(visitor);
        }
        if let Some(lease) = &self.data {
            lease.visit_stage_paths(visitor);
        }
    }
}
