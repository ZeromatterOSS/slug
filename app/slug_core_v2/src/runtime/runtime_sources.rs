//! Configured source trees, prepared with certified tools in one restored command.
//! This private lease grants no execution or complete namespace authority.
#![allow(dead_code)]
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceDeclarationTransaction;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_loading_v2::ObservedRuntimeSourceProfile;
use slug_loading_v2::ObservedRuntimeSourceRequests;
use slug_loading_v2::RuntimeSourceProfileKey;
use slug_loading_v2::RuntimeSourceRequest;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ObservedInputImage;

use super::selected_tool_source::PreparationMode;
use super::selected_tool_source::PreparedLease;
use super::selected_tool_source::{self};
use super::*;
use crate::runtime::input_image;

#[path = "runtime_sources/attempt.rs"]
pub(super) mod attempt;
#[path = "runtime_sources/catalogue.rs"]
pub(super) mod catalogue;
#[path = "runtime_sources/factory.rs"]
mod factory;
#[path = "runtime_sources/prepare.rs"]
pub(super) mod prepare;
#[cfg(all(test, target_os = "linux"))]
#[path = "runtime_sources/tests.rs"]
pub(in crate::runtime::dice) mod tests;

fn refusal(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}
fn image_error(error: slug_workspace_v2::ImageDescriptorBudgetError) -> NativeDemandSessionError {
    NativeDemandSessionError::InputImage(error.into())
}

pub(super) fn install_declaration_transaction(
    data: &mut UserComputationData,
    runtime: &WorkspaceRuntime,
    policy: Option<&RuntimeSourceDeclarationPolicy>,
) -> anyhow::Result<()> {
    // Absence has no new reservation or userdata allocation: unrelated commands
    // keep their existing resource-pressure and error ordering.
    if let Some(policy) = policy {
        if policy.workspace().as_path() != runtime.workspace {
            return Err(anyhow::anyhow!("foreign runtime source policy"));
        }
        data.data.set(RuntimeSourceDeclarationTransaction::new(
            policy.workspace().dupe(),
            Some(policy.dupe()),
            runtime.image_resources.descriptor_budget().dupe(),
        )?);
    }
    Ok(())
}
pub(super) fn declaration_transaction(
    command: &NativeDemandCommand<'_>,
) -> Result<RuntimeSourceDeclarationTransaction, NativeDemandSessionError> {
    let policy = command
        .inputs
        .request
        .runtime_sources
        .as_ref()
        .ok_or_else(|| refusal("missing runtime source declaration"))?;
    RuntimeSourceDeclarationTransaction::new(
        policy.workspace().dupe(),
        Some(policy.dupe()),
        command.runtime.image_resources.descriptor_budget().dupe(),
    )
    .map_err(|_| refusal("foreign runtime declaration transaction"))
}

impl WorkspaceRuntime {
    pub(super) fn prepare_selected_runtime_sources(
        &self,
        key: RuntimeSourceProfileKey,
        policy: Option<RuntimeSourceDeclarationPolicy>,
    ) -> Result<CertifiedRuntimeSourcesLease, NativeDemandSessionError> {
        self.prepare_selected_runtime_sources_inner(
            key,
            policy,
            #[cfg(test)]
            None,
        )
    }

    fn prepare_selected_runtime_sources_inner(
        &self,
        key: RuntimeSourceProfileKey,
        policy: Option<RuntimeSourceDeclarationPolicy>,
        #[cfg(test)] hook: Option<
            &dyn Fn(selected_tool_source::ToolPhase) -> Result<(), NativeDemandSessionError>,
        >,
    ) -> Result<CertifiedRuntimeSourcesLease, NativeDemandSessionError> {
        let policy = policy.ok_or_else(|| refusal("missing runtime source declaration"))?;
        if policy.workspace().as_path() != self.workspace
            || !policy.same_budget(self.image_resources.descriptor_budget())
        {
            return Err(refusal("foreign runtime source declaration"));
        }
        match self.prepare_selected_sources_inner(
            key.tool_sources_key(),
            PreparationMode::Runtime { key, policy },
            #[cfg(test)]
            hook,
        )? {
            PreparedLease::Runtime(lease) => Ok(lease),
            PreparedLease::Tools(_) | PreparedLease::Namespace(_) => {
                unreachable!("runtime preparation mode")
            }
        }
    }
}

struct RuntimeInputOwner {
    profile: ObservedRuntimeSourceProfile,
    tools: selected_tool_source::CertifiedSelectedToolInputsLease,
    images: [Option<input_image::CapturedInputImageLease>; 64],
    unused: [Option<input_image::PreparedInputImageStage>; 64],
}
#[derive(Clone, Dupe)]
pub(super) struct CertifiedRuntimeSourcesLease(BudgetedImageBlock<RuntimeInputOwner>);
impl fmt::Debug for CertifiedRuntimeSourcesLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertifiedRuntimeSourcesLease")
            .finish_non_exhaustive()
    }
}
impl CertifiedRuntimeSourcesLease {
    #[cfg(test)]
    pub(super) fn test_image_path(&self, declaration: usize) -> &std::path::Path {
        self.0.get().images[declaration]
            .as_ref()
            .expect("captured runtime image")
            .test_path()
    }

    pub(super) fn with_image_root<R>(
        &self,
        declaration: usize,
        visitor: impl FnOnce(&std::fs::File, &ObservedInputImage) -> R,
    ) -> Option<R> {
        Some(
            self.0
                .get()
                .images
                .get(declaration)?
                .as_ref()?
                .with_read_root(visitor),
        )
    }
    pub(super) fn tools(&self) -> &selected_tool_source::CertifiedSelectedToolInputsLease {
        &self.0.get().tools
    }

    pub(super) fn profile(&self) -> &ObservedRuntimeSourceProfile {
        &self.0.get().profile
    }
}

pub(super) fn validate_plan(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedRuntimeSourceRequests,
    reachable: &SelectedWorkspaceDemands,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    if !plan.matches_key(key.requests_key())
        || !plan.same_resources(&command.runtime.scratch_resources)
        || command.inputs.request.runtime_sources.as_ref() != Some(plan.policy())
        || !plan.authorized_by(&declaration_transaction(command)?)
    {
        return Err(refusal("foreign current runtime source plan"));
    }
    selected_tool_source::validate_plan(
        command,
        &key.tool_sources_key(),
        plan.tool_requests(),
        reachable,
        deadline,
    )?;
    for selected in plan
        .requests()
        .filter_map(|row| row.selected_label_source())
    {
        super::input_image_bridge::label_source::check_selection(command.runtime, selected)?;
        super::input_image_bridge::label_source::check_reachable(command, selected, reachable)?;
    }
    Ok(())
}
pub(super) fn validate_final(
    command: &NativeDemandCommand<'_>,
    key: &RuntimeSourceProfileKey,
    plan: &ObservedRuntimeSourceRequests,
    profile: &ObservedRuntimeSourceProfile,
    reachable: &SelectedWorkspaceDemands,
    deadline: std::time::Instant,
) -> Result<(), NativeDemandSessionError> {
    validate_plan(command, key, plan, reachable, deadline)?;
    let scratch = command
        .scratch_metadata
        .as_ref()
        .ok_or_else(|| refusal("missing scratch authorization"))?
        .transaction()?;
    if profile.requests() != plan
        || !profile.matches_key(key)
        || !profile.same_resources(&command.runtime.scratch_resources)
        || !profile.authorized_by(
            &declaration_transaction(command)?,
            &scratch,
            &command.runtime.scratch_resources,
        )
        || profile.images().len() != plan.requests().len()
    {
        return Err(refusal("foreign completed runtime profile"));
    }
    selected_tool_source::validate_final(
        command,
        &key.tool_sources_key(),
        plan.tool_requests(),
        profile.tool_sources(),
        reachable,
        deadline,
    )?;
    for (request, image) in plan.requests().zip(profile.images()) {
        if image.namespace() != request.image_key().namespace()
            || image.anchor_root() != request.image_key().anchor_root()
            || image.source_root() != request.image_key().source_root()
            || image.policy() != request.image_key().policy()
            || image
                .observations()
                .observations()
                .iter()
                .any(|(demand, result)| {
                    !reachable.unscoped_paths().contains(demand)
                        && !reachable.repository_validations().iter().any(|scope| {
                            request
                                .selected_label_source()
                                .and_then(|s| s.repository())
                                .is_some_and(|(r, _)| scope.request() == r)
                                && scope.paths().contains(demand)
                        })
                        || command.path_observations.get(demand) != Some(result)
                })
        {
            return Err(refusal("runtime image source closure differs"));
        }
    }
    Ok(())
}
