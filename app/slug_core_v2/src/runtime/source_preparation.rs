//! Private full-caller source preparation. No wire or execution authority.
#![allow(dead_code)]
use slug_bzlmod_v2::SelectedScratchPolicy;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_identity_v2::ApparentRepoName;
use slug_loading_v2::ObservedSelectedPreparationOwner;
use slug_loading_v2::SelectedPreparationOwnerKey;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

#[path = "source_preparation/attempt.rs"]
pub(super) mod attempt;
#[path = "source_preparation/request.rs"]
mod request;
#[cfg(all(test, target_os = "linux"))]
#[path = "source_preparation/tests.rs"]
mod tests;

pub(super) struct SelectedSourcePreparationRequest<'a> {
    pub apparent_repository: &'a ApparentRepoName,
    pub inputs: &'a NativeDemandRequestInputBundle,
    pub scratch_policy: SelectedScratchPolicy,
    pub tool_policy: SelectedToolSourcesPolicy,
}

struct PreparedSourceOwner {
    view: guest_namespace::PreparedGuestNamespaceViewLease,
    selected: ObservedSelectedPreparationOwner,
    inputs: NativeDemandRequestInputBundle,
}
#[derive(Clone, Dupe)]
pub(super) struct PreparedSelectedSourceView(BudgetedImageBlock<PreparedSourceOwner>);
impl PreparedSelectedSourceView {
    pub(super) fn view(&self) -> &guest_namespace::PreparedGuestNamespaceViewLease {
        &self.0.get().view
    }
    pub(super) fn selected(&self) -> &ObservedSelectedPreparationOwner {
        &self.0.get().selected
    }
    pub(super) fn inputs(&self) -> &NativeDemandRequestInputBundle {
        &self.0.get().inputs
    }
}

pub(super) struct OwnerAuthorization<'request> {
    pub key: SelectedPreparationOwnerKey,
    pub observed: ObservedSelectedPreparationOwner,
    #[cfg(test)]
    pub expected_inputs: &'request NativeDemandRequestInputBundle,
    #[cfg(not(test))]
    pub _request: std::marker::PhantomData<&'request NativeDemandRequestInputBundle>,
}

fn refusal(text: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(text))
}

impl WorkspaceRuntime {
    pub(super) fn prepare_selected_source_view(
        &self,
        request: SelectedSourcePreparationRequest<'_>,
    ) -> Result<PreparedSelectedSourceView, NativeDemandSessionError> {
        self.prepare_selected_source_view_inner(
            request,
            #[cfg(test)]
            None,
        )
    }

    fn prepare_selected_source_view_inner(
        &self,
        request: SelectedSourcePreparationRequest<'_>,
        #[cfg(test)] hook: Option<
            &dyn Fn(selected_tool_source::ToolPhase<'_>) -> Result<(), NativeDemandSessionError>,
        >,
    ) -> Result<PreparedSelectedSourceView, NativeDemandSessionError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let runtime_policy = request
            .inputs
            .runtime_sources
            .as_ref()
            .ok_or_else(|| refusal("missing runtime source declaration"))?;
        let guest = request
            .inputs
            .guest_namespace
            .ok_or_else(|| refusal("missing guest namespace declaration"))?;
        if runtime_policy.workspace().as_path() != self.workspace
            || !runtime_policy.same_budget(self.image_resources.descriptor_budget())
        {
            return Err(refusal("foreign runtime source declaration"));
        }
        let credit = request::copy_credit(self, request.inputs)?;
        let owner_key = SelectedPreparationOwnerKey::new(
            self.repository_materializer.workspace().dupe(),
            request.apparent_repository,
            self.image_resources.descriptor_budget(),
        )
        .map_err(|e| NativeDemandSessionError::Computation(anyhow::Error::new(e)))?;
        let inputs = request.inputs.clone();
        let stages = selected_tool_source::prepare::setup::Stages::new(
            self,
            request.scratch_policy,
            Some(runtime_policy),
            Some(guest),
        )?;
        let mut guard = NativeDemandAbortGuard::new(
            self.begin_native_demand_command_with_inputs(inputs.clone())?
                .into_command(),
        );
        let resolved = (|| {
            for _ in 0..stages.attempts {
                selected_scratch_source::active(guard.command(), deadline)?;
                guard.begin_attempt()?;
                let outcome = self.runtime.block_on(async {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        attempt::owner(self, &mut guard, &owner_key, deadline),
                    )
                    .await
                    .map_err(|_| refusal("source owner preparation cancelled"))?
                });
                self.runtime.block_on(self.dice.wait_for_idle());
                match outcome? {
                    selected_tool_source::attempt::Attempt::Need(need) => {
                        selected_tool_source::progress_baseline(&mut guard, &need, deadline)?;
                    }
                    selected_tool_source::attempt::Attempt::Complete(observed, selected) => {
                        let (_, _, terminal) = selected.sidecars.into_parts();
                        terminal
                            .reset_to_idle()
                            .map_err(NativeDemandSessionError::Effect)?;
                        return Ok(OwnerAuthorization {
                            key: owner_key,
                            observed,
                            #[cfg(test)]
                            expected_inputs: &inputs,
                            #[cfg(not(test))]
                            _request: std::marker::PhantomData,
                        });
                    }
                }
            }
            Err(refusal("source owner preparation made no progress"))
        })();
        let mut owner = match resolved {
            Ok(owner) => owner,
            Err(e) => {
                return selected_tool_source::prepare::finalize(self, &mut guard, Err(e), deadline);
            }
        };
        let checkpoint = slug_loading_v2::HostSelectedExtensionScratchPrefixKey::new(
            self.repository_materializer.workspace().dupe(),
            owner.observed.owner().dupe(),
            request.scratch_policy,
        );
        let tools =
            slug_loading_v2::SelectedExtensionToolSourcesKey::new(checkpoint, request.tool_policy);
        let profile = slug_loading_v2::RuntimeSourceProfileKey::new(tools.requests_key().clone());
        let namespace = slug_loading_v2::SelectedGuestNamespaceFactsKey::new(profile.clone());
        let mode = selected_tool_source::PreparationMode::Namespace {
            key: namespace,
            runtime_key: profile,
            runtime_policy: runtime_policy.dupe(),
            guest_policy: guest,
        };
        let lease = self.run_selected_sources(
            tools,
            mode,
            stages,
            guard,
            Some(&mut owner),
            deadline,
            #[cfg(test)]
            hook,
        )?;
        let selected_tool_source::PreparedLease::Namespace(view) = lease else {
            return Err(refusal("source preparation produced wrong lease"));
        };
        credit
            .freeze(PreparedSourceOwner {
                view,
                selected: owner.observed,
                inputs,
            })
            .map(PreparedSelectedSourceView)
            .map_err(|e| NativeDemandSessionError::InputImage(e.into()))
    }
}
