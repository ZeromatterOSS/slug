use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;

use super::*;

// Source provenance, not a digest/path match. Additional Label frontiers remain in Requests.
fn same_origin(a: &ObservedSelectedLabelSource, b: &ObservedSelectedLabelSource) -> bool {
    a.recipe().workspace() == b.recipe().workspace()
        && a.recipe().source() == b.recipe().source()
        && a.repository() == b.repository()
}
fn eligible(
    request: &SelectedDataLabelRequest,
    image: &ObservedInputImage,
    origin: Option<&ObservedSelectedLabelSource>,
    original_mapping: bool,
) -> Option<usize> {
    let selected = request.selection();
    if image.namespace() != selected.image_key().namespace()
        || image.anchor_root() != selected.image_key().anchor_root()
    {
        return None;
    }
    if let Some(origin) = origin {
        if !same_origin(selected, origin) {
            return None;
        }
    } else if image.namespace() != PathObservationNamespace::Host
        || image.anchor_root() != selected.recipe().workspace().normalized_ref()
        || !matches!(
            selected.recipe().source(),
            HostRepositoryLabelPathSource::Root(_)
        )
    {
        return None;
    }
    image.entries().iter().position(|node| {
        node.path().normalized_ref() == selected.selected_file().normalized_ref()
            && match node.kind() {
                InputImageEntryKind::RegularFile(_) => true,
                InputImageEntryKind::Directory => original_mapping,
                _ => false,
            }
    })
}
fn reusable(
    request: &SelectedDataLabelRequest,
    profile: &ObservedRuntimeSourceProfile,
) -> Option<SelectedDataLabelBinding> {
    for (role, source) in profile.tool_sources().sources().enumerate() {
        let (Some(metadata), Some(origin)) = (source.label_metadata(), source.label_selection())
        else {
            continue;
        };
        if let Some(entry) = eligible(request, metadata.image(), Some(origin), true) {
            return Some(SelectedDataLabelBinding::ToolMember { role, entry });
        }
    }
    for (declaration, (row, image)) in profile
        .requests()
        .requests()
        .zip(profile.images())
        .enumerate()
    {
        let original = row.guest_prefix().as_path() == image.source_root().as_path();
        if let Some(entry) = eligible(request, image, row.selected_label_source(), original) {
            return Some(SelectedDataLabelBinding::RuntimeMember { declaration, entry });
        }
    }
    None
}
pub(super) async fn compute(
    key: &SelectedDataLabelSourcesKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<SourcesValue, SelectedDataLabelError> {
    let end = deadline(ctx);
    let check = || !cancel.is_cancelled() && Instant::now() < end;
    active(&check)?;
    let requests = match crate::dependency_cycle::compute_selected_data_label_requests(
        ctx,
        &key.requests_key(),
    )
    .await?
    {
        SourcePreparationOutcome::Need(n) => return Ok(SourcePreparationOutcome::Need(n)),
        SourcePreparationOutcome::Complete(v) => v?,
    };
    let profile =
        match crate::dependency_cycle::compute_runtime_source_profile(ctx, key.profile_key())
            .await?
        {
            SourcePreparationOutcome::Need(n) => return Ok(SourcePreparationOutcome::Need(n)),
            SourcePreparationOutcome::Complete(v) => v?,
        };
    if profile.requests() != requests.runtime_requests() {
        return Err(SelectedDataLabelError::Operational);
    }
    let r = resources(ctx)?;
    let mut credit = r
        .descriptor_budget()
        .try_reserve(std::mem::size_of::<SourcesInner>() + 256)
        .map_err(|_| SelectedDataLabelError::Operational)?;
    let mut rows = reserve::<SelectedDataLabelSource>(&mut credit, requests.requests().len())?;
    let mut observations = merge(&mut credit, requests.observations(), profile.observations())?;
    for request in requests.requests() {
        active(&check)?;
        let binding = if let Some(binding) = reusable(request, &profile) {
            binding
        } else {
            let image = match ctx
                .compute(request.leaf_image_key())
                .await
                .map_err(|_| SelectedDataLabelError::Operational)?
            {
                PathOutcome::Need(n) => {
                    return Ok(SourcePreparationOutcome::Need(
                        SourcePreparationNeeds::path(n),
                    ));
                }
                PathOutcome::Complete(v) => v?,
            };
            if !image.is_regular_file_root()
                || image.namespace() != request.leaf_image_key().namespace()
                || image.anchor_root() != request.leaf_image_key().anchor_root()
                || image.source_root() != request.leaf_image_key().source_root()
                || image.policy() != request.leaf_image_key().policy()
                || !image.same_budget(r.descriptor_budget())
                || image.entries().len() != 1
                || image.entries()[0].path().normalized_ref() != image.source_root()
                || !matches!(
                    image.entries()[0].kind(),
                    InputImageEntryKind::RegularFile(_)
                )
            {
                return Err(SelectedDataLabelError::InvalidReuse);
            }
            observations = merge(&mut credit, &observations, image.observations())?;
            SelectedDataLabelBinding::Leaf { image }
        };
        rows.push(SelectedDataLabelSource {
            index: request.occurrence_index(),
            binding,
        });
    }
    active(&check)?;
    let result = ObservedSelectedDataLabelSources(
        credit
            .freeze(SourcesInner {
                key: key.clone(),
                requests,
                profile,
                rows,
                observations,
            })
            .map_err(|_| SelectedDataLabelError::Operational)?,
    );
    super::authorize_sources(ctx, &result)?;
    Ok(SourcePreparationOutcome::Complete(Ok(result)))
}
