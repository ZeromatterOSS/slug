use std::path::Path;

use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicyKey;
use slug_bzlmod_v2::RuntimeSourceSelectorRef;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::PathObservationNamespace;

use super::*;

pub(super) async fn compute(
    key: &RuntimeSourceRequestsKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<RequestsValue, RuntimeSourcesError> {
    let deadline = deadline(ctx);
    let active = || !cancel.is_cancelled() && Instant::now() < deadline;
    require_active(&active)?;
    // Raw absence is a source prerequisite before lower tool/source dependencies.
    let policy = ctx
        .compute(&RuntimeSourceDeclarationPolicyKey::new(
            key.workspace().dupe(),
        ))
        .await
        .map_err(|_| RuntimeSourcesError::Operational)?
        .ok_or(RuntimeSourcesError::MissingDeclaration)?;
    let r = resources(ctx)?;
    let tx = declaration_transaction(ctx)?;
    if !policy.same_budget(r.descriptor_budget())
        || !tx.same_budget(r.descriptor_budget())
        || !tx.authorizes(&policy)
        || policy.workspace().normalized_ref() != key.workspace()
    {
        return Err(RuntimeSourcesError::Operational);
    }
    let tools = match crate::dependency_cycle::compute_selected_tool_source_requests(
        ctx,
        key.tool_requests_key(),
    )
    .await?
    {
        SourcePreparationOutcome::Need(need) => return Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(value) => value?,
    };
    let platform = tools.prefix().platform();
    if platform.os_name() != "linux" || platform.arch() != "x86_64" {
        return Err(RuntimeSourcesError::UnsupportedPlatform);
    }
    let mut credit = r
        .descriptor_budget()
        .try_reserve(256)
        .map_err(|_| RuntimeSourcesError::Operational)?;
    let mut rows = reserve_rows::<RuntimeSourceRequest>(&mut credit, policy.declarations().len())?;
    let mut observations = tools.prefix().observations().dupe();
    for source in tools.requests().filter_map(|r| r.label_selection()) {
        observations = merge(&mut credit, &observations, source.observations())?;
    }
    for (index, declaration) in policy.declarations().enumerate() {
        require_active(&active)?;
        let guest = declaration.guest_prefix().as_path();
        if tools.requests().any(|tool| {
            let spelling = Path::new(tool.selection().spelling());
            spelling.starts_with(guest) || guest.starts_with(spelling)
        }) {
            return Err(RuntimeSourcesError::InvalidScope);
        }
        let (image, selected) = match declaration.selector() {
            RuntimeSourceSelectorRef::ExplicitHostDirectory { anchor } => {
                let workspace = key.workspace().as_path();
                if anchor.as_path().starts_with(workspace)
                    || workspace.starts_with(anchor.as_path())
                {
                    return Err(RuntimeSourcesError::InvalidScope);
                }
                (
                    ObservedInputImageKey::from_charged(
                        PathObservationNamespace::Host,
                        anchor.dupe(),
                        anchor.dupe(),
                        declaration.image_policy(),
                    )?,
                    None,
                )
            }
            RuntimeSourceSelectorRef::WorkspaceRelativeDirectory { components } => {
                let anchor =
                    ImageChargedPath::pin_existing(key.workspace().dupe(), r.descriptor_budget())
                        .map_err(|_| RuntimeSourcesError::Operational)?;
                let mut directory = anchor.dupe();
                for component in components {
                    // The existing public path join takes an owned validated name.
                    // Reserve its temporary storage before conversion; the joined
                    // path independently keeps its full retained credit.
                    let _name_credit = r
                        .descriptor_budget()
                        .try_reserve(
                            component
                                .as_str()
                                .len()
                                .checked_add(128)
                                .ok_or(RuntimeSourcesError::Operational)?,
                        )
                        .map_err(|_| RuntimeSourcesError::Operational)?;
                    let name = slug_workspace_v2::PathDirectoryName::new(component.as_str())
                        .map_err(|_| RuntimeSourcesError::InvalidScope)?;
                    directory = directory
                        .try_join_component(&name)
                        .map_err(|_| RuntimeSourcesError::Operational)?;
                }
                (
                    ObservedInputImageKey::from_charged(
                        PathObservationNamespace::Host,
                        anchor,
                        directory,
                        declaration.image_policy(),
                    )?,
                    None,
                )
            }
            RuntimeSourceSelectorRef::SelectedToolOriginLabelParent { role, components } => {
                let original = tools
                    .requests()
                    .find(|tool| tool.operand().role == role)
                    .and_then(|tool| tool.label_selection())
                    .ok_or(RuntimeSourcesError::WrongLabelRole)?;
                let mut recipe = original.recipe().origin_parent(r.descriptor_budget())?;
                for component in components {
                    recipe = recipe.child(component.as_str(), r.descriptor_budget())?;
                }
                let selected = match crate::dependency_cycle::compute_selected_label_source(
                    ctx,
                    &SelectedLabelSourceKey::new(recipe),
                )
                .await?
                {
                    SourcePreparationOutcome::Need(need) => {
                        return Ok(SourcePreparationOutcome::Need(need));
                    }
                    SourcePreparationOutcome::Complete(value) => value?,
                };
                if let HostRepositoryLabelPathSource::External(route) = selected.recipe().source() {
                    if !route.is_immutable_materialization() {
                        return Err(RuntimeSourcesError::WrongLabelRole);
                    }
                }
                observations = merge(&mut credit, &observations, selected.observations())?;
                let anchor = ImageChargedPath::pin_existing(
                    selected.image_key().anchor_root().dupe(),
                    r.descriptor_budget(),
                )
                .map_err(|_| RuntimeSourcesError::Operational)?;
                let image = ObservedInputImageKey::from_charged(
                    selected.image_key().namespace(),
                    anchor,
                    selected.selected_file().dupe(),
                    declaration.image_policy(),
                )?;
                (image, Some(selected))
            }
        };
        if image.source_root().as_path() == Path::new("/")
            || rows.iter().any(|previous| {
                let old = previous.image_key().source_root().as_path();
                let new = image.source_root().as_path();
                old.starts_with(new) || new.starts_with(old)
            })
        {
            return Err(RuntimeSourcesError::InvalidScope);
        }
        rows.push(RuntimeSourceRequest {
            policy: policy.dupe(),
            index,
            image,
            selected,
        });
    }
    require_active(&active)?;
    let value = ObservedRuntimeSourceRequests(
        credit
            .freeze(RequestsInner {
                policy,
                tools,
                requests: rows,
                observations,
            })
            .map_err(|_| RuntimeSourcesError::Operational)?,
    );
    if !value.same_resources(&r) || !value.authorized_by(&tx) || !value.matches_key(key) {
        return Err(RuntimeSourcesError::Operational);
    }
    Ok(SourcePreparationOutcome::Complete(Ok(value)))
}
