use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::SelectedLabelSourceKey;
use slug_workspace_v2::ImageChargedPath;

use super::*;

pub(super) async fn compute(
    key: &SelectedDataLabelRequestsKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<RequestsValue, SelectedDataLabelError> {
    let end = deadline(ctx);
    let check = || !cancel.is_cancelled() && Instant::now() < end;
    active(&check)?;
    let policy = ctx
        .compute(&RawGuestNamespacePolicyKey::new(key.workspace().dupe()))
        .await
        .map_err(|_| SelectedDataLabelError::Operational)?
        .ok_or(SelectedDataLabelError::MissingDeclaration)?;
    let GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses } = policy.label_data() else {
        return Err(SelectedDataLabelError::Disabled);
    };
    let r = resources(ctx)?;
    let guest = ctx
        .per_transaction_data()
        .data
        .get::<GuestNamespaceTransaction>()
        .map_err(|_| SelectedDataLabelError::Operational)?
        .dupe();
    if guest.workspace() != key.workspace()
        || !guest.authorizes(&policy)
        || !guest.same_budget(r.descriptor_budget())
    {
        return Err(SelectedDataLabelError::Operational);
    }
    let runtime = match crate::dependency_cycle::compute_runtime_source_requests(
        ctx,
        key.runtime_requests_key(),
    )
    .await?
    {
        SourcePreparationOutcome::Need(n) => return Ok(SourcePreparationOutcome::Need(n)),
        SourcePreparationOutcome::Complete(v) => v?,
    };
    let witnesses = runtime.tool_requests().prefix().label_sources();
    if witnesses.len() > usize::from(max_witnesses) {
        return Err(SelectedDataLabelError::WitnessLimit);
    }
    let mut credit = r
        .descriptor_budget()
        .try_reserve(std::mem::size_of::<RequestsInner>() + 256)
        .map_err(|_| SelectedDataLabelError::Operational)?;
    let mut rows = reserve::<SelectedDataLabelRequest>(&mut credit, witnesses.len())?;
    let mut observations = runtime.observations().dupe();
    for (index, witness) in witnesses.iter().enumerate() {
        active(&check)?;
        let selected = match crate::dependency_cycle::compute_selected_label_source(
            ctx,
            &SelectedLabelSourceKey::new(witness.selection().recipe().dupe()),
        )
        .await?
        {
            SourcePreparationOutcome::Need(n) => return Ok(SourcePreparationOutcome::Need(n)),
            SourcePreparationOutcome::Complete(v) => v?,
        };
        if &selected != witness.selection() {
            return Err(SelectedDataLabelError::Operational);
        }
        if let HostRepositoryLabelPathSource::External(route) = selected.recipe().source() {
            if !route.is_immutable_materialization() {
                return Err(SelectedDataLabelError::UnsupportedSource);
            }
        }
        let anchor = ImageChargedPath::pin_existing(
            selected.image_key().anchor_root().dupe(),
            r.descriptor_budget(),
        )
        .map_err(|_| SelectedDataLabelError::Operational)?;
        let image = ObservedInputImageKey::regular_file_from_charged(
            selected.image_key().namespace(),
            anchor,
            selected.selected_file().dupe(),
            *selected.image_key().policy(),
        )?;
        observations = merge(&mut credit, &observations, selected.observations())?;
        rows.push(SelectedDataLabelRequest {
            index,
            event: witness.event_index(),
            selection: selected,
            image,
        });
    }
    active(&check)?;
    let result = ObservedSelectedDataLabelRequests(
        credit
            .freeze(RequestsInner {
                key: key.clone(),
                policy,
                runtime,
                rows,
                observations,
            })
            .map_err(|_| SelectedDataLabelError::Operational)?,
    );
    super::authorize_requests(ctx, &result)?;
    Ok(SourcePreparationOutcome::Complete(Ok(result)))
}
