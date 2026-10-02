//! Normal Label provenance is retained through declared image/ELF preparation.

use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::ObservedSelectedLabelSource;
use slug_bzlmod_v2::RepositoryMaterialization;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::InputImageEntryKind;

use super::*;
use crate::runtime::demands::SelectedWorkspaceDemands;

#[allow(dead_code)]
#[derive(Clone, dupe::Dupe, allocative::Allocative)]
pub(in crate::runtime) struct CapturedSelectedLabelSource {
    pub(in crate::runtime) selected: ObservedSelectedLabelSource,
    // The existing native lease accounts its allocation/disk service itself.
    #[allocative(skip)]
    pub(in crate::runtime) image: CapturedInputImageLease,
}

#[allow(dead_code)]
#[derive(Clone, dupe::Dupe, allocative::Allocative)]
pub(in crate::runtime) struct SelectedLabelElfMetadata {
    pub(in crate::runtime) selected: ObservedSelectedLabelSource,
    pub(in crate::runtime) metadata: ObservedElfMetadata,
}

#[allow(dead_code)]
impl WorkspaceRuntime {
    pub(in crate::runtime) fn capture_selected_label_source(
        &self,
        key: SelectedLabelSourceKey,
    ) -> Result<CapturedSelectedLabelSource, NativeDemandSessionError> {
        let policy = key.recipe().policy().image_policy();
        match self.prepare_declared_input(
            DeclaredImageSource::SelectedLabel(key),
            policy,
            prepare::PreparationKind::Capture,
        )? {
            prepare::PreparedInput::Captured(image, Some(selected)) => {
                Ok(CapturedSelectedLabelSource { selected, image })
            }
            _ => unreachable!("selected Label capture mode"),
        }
    }

    pub(in crate::runtime) fn observe_selected_label_elf_metadata(
        &self,
        key: SelectedLabelSourceKey,
        policy: ElfMetadataPolicy,
    ) -> Result<SelectedLabelElfMetadata, NativeDemandSessionError> {
        let image_policy = key.recipe().policy().image_policy();
        match self.prepare_declared_input(
            DeclaredImageSource::SelectedLabel(key),
            image_policy,
            prepare::PreparationKind::SelectedElf(policy),
        )? {
            prepare::PreparedInput::Elf(metadata, Some(selected)) => {
                Ok(SelectedLabelElfMetadata { selected, metadata })
            }
            _ => unreachable!("selected Label ELF mode"),
        }
    }
}

pub(super) async fn select(
    runtime: &WorkspaceRuntime,
    transaction: &mut dice::DiceTransaction,
    key: &SelectedLabelSourceKey,
    policy: InputImagePolicy,
) -> Result<prepare::SourceSelection, NativeDemandSessionError> {
    check_key(runtime, key, policy)?;
    let selected = match slug_loading_v2::compute_selected_label_source(transaction, key)
        .await
        .map_err(|_| image_error("selected Label computation failed"))?
    {
        SourcePreparationOutcome::Need(need) => {
            return Ok(prepare::SourceSelection::Need(need));
        }
        SourcePreparationOutcome::Complete(result) => {
            result.map_err(|_| image_error("selected Label producer refused source"))?
        }
    };
    if selected.recipe() != key.recipe() {
        return Err(image_error("selected Label origin changed"));
    }
    check_selection(runtime, &selected)?;
    let repository = selected
        .repository()
        .map(|_| DeclaredImageRepository::Selected(selected.clone()));
    Ok(prepare::SourceSelection::Ready(
        selected.image_key().clone(),
        repository,
        Some(selected),
    ))
}

pub(super) fn check_key(
    runtime: &WorkspaceRuntime,
    key: &SelectedLabelSourceKey,
    policy: InputImagePolicy,
) -> Result<(), NativeDemandSessionError> {
    if !key
        .recipe()
        .same_budget(runtime.image_resources.descriptor_budget())
        || key.recipe().workspace().as_path() != runtime.workspace
        || key.recipe().policy().image_policy() != policy
    {
        return Err(image_error("foreign selected Label recipe"));
    }
    if let HostRepositoryLabelPathSource::External(route) = key.recipe().source() {
        if !route.is_immutable_materialization() {
            return Err(image_error(
                "external Local or builtin selected source is unsupported",
            ));
        }
    }
    Ok(())
}

fn check_selection(
    runtime: &WorkspaceRuntime,
    selected: &ObservedSelectedLabelSource,
) -> Result<(), NativeDemandSessionError> {
    let key = selected.image_key();
    let recipe = selected.recipe();
    let budget = runtime.image_resources.descriptor_budget();
    if !selected.same_budget(budget)
        || !recipe.same_budget(budget)
        || recipe.workspace().as_path() != runtime.workspace
        || *key.policy() != recipe.policy().image_policy()
        || !selected
            .selected_file()
            .as_path()
            .starts_with(key.source_root().as_path())
        || !key
            .source_root()
            .as_path()
            .starts_with(key.anchor_root().as_path())
    {
        return Err(image_error("foreign selected Label selectors"));
    }
    match (recipe.source(), selected.repository()) {
        (HostRepositoryLabelPathSource::Root(workspace), None)
            if workspace.as_path() == runtime.workspace
                && key.anchor_root() == workspace
                && key.namespace() == PathObservationNamespace::Host
                && recipe.label().package().repo().is_root() =>
        {
            Ok(())
        }
        (HostRepositoryLabelPathSource::External(route), Some((request, result))) => {
            let RepositoryMaterialization::Immutable {
                canonical_repo,
                repo_spec,
                generation_root,
                observation_instance,
                ..
            } = result
            else {
                return Err(image_error("external Local selected source is unsupported"));
            };
            if request.id.workspace.as_path() != runtime.workspace
                || route.workspace().as_path() != runtime.workspace
                || canonical_repo != &request.id.canonical_repo
                || canonical_repo != route.canonical_repo()
                || canonical_repo != recipe.label().package().repo()
                || repo_spec != &request.repo_spec
                || key.namespace()
                    != PathObservationNamespace::Materialization(*observation_instance)
                || key.anchor_root().as_path() != generation_root
            {
                return Err(image_error("foreign selected immutable source"));
            }
            Ok(())
        }
        _ => Err(image_error("invalid selected Label source origin")),
    }
}

pub(super) fn check_declaration(
    runtime: &WorkspaceRuntime,
    request: &DeclaredImageRequest,
    selected: &ObservedSelectedLabelSource,
) -> Result<(), NativeDemandSessionError> {
    check_selection(runtime, selected)?;
    if request.key != *selected.image_key()
        || request
            .repository
            .as_ref()
            .map(DeclaredImageRepository::selection)
            != selected.repository()
    {
        return Err(image_error("selected Label declaration changed"));
    }
    Ok(())
}

pub(super) fn check_member(
    runtime: &WorkspaceRuntime,
    selected: &ObservedSelectedLabelSource,
    certificate: &ObservedInputImage,
) -> Result<(), NativeDemandSessionError> {
    check_selection(runtime, selected)?;
    let key = selected.image_key();
    if !certificate.same_budget(runtime.image_resources.descriptor_budget())
        || certificate.namespace() != key.namespace()
        || certificate.anchor_root() != key.anchor_root()
        || certificate.source_root() != key.source_root()
        || certificate.policy() != key.policy()
        || !certificate.entries().iter().any(|entry| {
            entry.path().as_path() == selected.selected_file().as_path()
                && matches!(entry.kind(), InputImageEntryKind::RegularFile(_))
        })
    {
        return Err(image_error(
            "selected Label target is not an exact regular image member",
        ));
    }
    Ok(())
}

pub(super) fn check_reachable(
    command: &NativeDemandCommand<'_>,
    selected: &ObservedSelectedLabelSource,
    reachable: &SelectedWorkspaceDemands,
) -> Result<(), NativeDemandSessionError> {
    for (demand, expected) in selected.observations().observations() {
        let reachable_path = reachable.unscoped_paths().contains(demand)
            || selected.repository().is_some_and(|(request, _)| {
                reachable
                    .repository_validations()
                    .iter()
                    .any(|scope| scope.request() == request && scope.paths().contains(demand))
            });
        if !reachable_path || command.path_observations.get(demand) != Some(expected) {
            return Err(image_error(
                "Label frontier is not current reachable evidence",
            ));
        }
    }
    if selected
        .repository()
        .is_some_and(|(request, _)| !reachable.repository_requests().contains(request))
    {
        return Err(image_error("selected Label repository is not reachable"));
    }
    Ok(())
}

/// Re-observe complete normal Label and image/ELF frontiers after compute drains.
/// The Label metadata owner is unchanged; image/ELF still need exact FD authority.
pub(super) fn verify_current(
    command: &NativeDemandCommand<'_>,
    selected: &ObservedSelectedLabelSource,
    certificate: &ObservedInputImage,
    metadata: Option<&ObservedElfMetadata>,
) -> Result<(), NativeDemandSessionError> {
    let request = command
        .image_request
        .as_ref()
        .ok_or_else(|| image_error("missing selected Label declaration"))?;
    check_declaration(command.runtime, request, selected)?;
    check_member(command.runtime, selected, certificate)?;
    if metadata.is_some_and(|value| {
        !value.same_budget(command.runtime.image_resources.descriptor_budget())
            || value.image() != certificate
            || value.file() != selected.selected_file()
            || request
                .elf
                .as_ref()
                .is_none_or(|elf| elf.policy != value.policy())
    }) {
        return Err(image_error("foreign selected Label ELF frontier"));
    }
    // Validate the materializer's current exact request/result/root before any
    // final Label read, not only before the scoped part of the frontier.
    let _source = request.source(command)?;
    let image_frontier = metadata.map_or(certificate.observations(), |value| value.observations());
    for (demand, expected) in selected.observations().observations() {
        if image_frontier
            .get(demand)
            .is_some_and(|other| other != expected)
        {
            return Err(image_error("conflicting selected source frontier"));
        }
    }
    for frontier in [selected.observations(), image_frontier] {
        let count = frontier.observations().len();
        // Cover the demand vector and conservative fixed row/vector replacement
        // peaks in the existing normal observer/epoch owner before allocating.
        let bytes = count
            .checked_mul(
                std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>()
                    .saturating_mul(16),
            )
            .and_then(|bytes| bytes.checked_add(512))
            .ok_or_else(|| image_error("selected frontier descriptor overflow"))?;
        let _credit = command
            .runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(bytes)
            .map_err(|_| image_error("selected frontier descriptor admission failed"))?;
        let demands = frontier
            .observations()
            .iter()
            .map(|(demand, _)| demand.clone())
            .collect();
        let fresh = observe_command_paths(command, demands)?;
        if &fresh != frontier {
            return Err(NativeDemandSessionError::InputImage(
                input_image::InputImageCaptureError::StaleSource,
            ));
        }
    }
    // Revalidate retained source/session after the complete frontier pass.
    request.source(command)?;
    Ok(())
}
