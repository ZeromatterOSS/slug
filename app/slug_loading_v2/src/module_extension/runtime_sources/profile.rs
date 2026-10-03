use slug_workspace_v2::PathOutcome;

use super::*;

pub(super) async fn compute(
    key: &RuntimeSourceProfileKey,
    ctx: &mut DiceComputations<'_>,
    cancel: &CancellationContext,
) -> Result<ProfileValue, RuntimeSourcesError> {
    let deadline = deadline(ctx);
    let active = || !cancel.is_cancelled() && Instant::now() < deadline;
    require_active(&active)?;
    let requests =
        match crate::dependency_cycle::compute_runtime_source_requests(ctx, key.requests_key())
            .await?
        {
            SourcePreparationOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(value) => value?,
        };
    let r = resources(ctx)?;
    let tx = declaration_transaction(ctx)?;
    let tools =
        match crate::dependency_cycle::compute_selected_tool_sources(ctx, &key.tool_sources_key())
            .await?
        {
            SourcePreparationOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(value) => value?,
        };
    if tools.requests() != requests.tool_requests() {
        return Err(RuntimeSourcesError::Operational);
    }
    let scratch = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchElfMetadataTransaction>()
        .map_err(|_| RuntimeSourcesError::Operational)?
        .dupe();
    let mut credit = r
        .descriptor_budget()
        .try_reserve(256)
        .map_err(|_| RuntimeSourcesError::Operational)?;
    let mut images = reserve_rows::<ObservedInputImage>(&mut credit, requests.requests().len())?;
    let mut bindings =
        reserve_rows::<RuntimeSourceImageBinding>(&mut credit, requests.requests().len())?;
    let mut observations = merge(&mut credit, requests.observations(), tools.observations())?;
    let limits = requests.policy().limits();
    let mut totals = RuntimeSourceTotals {
        entries: 0,
        file_bytes: 0,
        descriptor_bytes: 0,
    };
    for row in requests.requests() {
        require_active(&active)?;
        let image = match ctx
            .compute(row.image_key())
            .await
            .map_err(|_| RuntimeSourcesError::Operational)?
        {
            PathOutcome::Need(need) => {
                return Ok(SourcePreparationOutcome::Need(
                    SourcePreparationNeeds::path(need),
                ));
            }
            PathOutcome::Complete(value) => value?,
        };
        if !image.same_budget(r.descriptor_budget())
            || image.namespace() != row.image_key().namespace()
            || image.anchor_root() != row.image_key().anchor_root()
            || image.source_root() != row.image_key().source_root()
            || image.policy() != row.image_key().policy()
        {
            return Err(RuntimeSourcesError::Operational);
        }
        // AliasDeny images count each original path once, exactly as ImageBuilder's
        // semantic descriptor counter. Capacity/headers/frontiers use separate credits.
        let descriptors = image.entries().iter().try_fold(0usize, |n, e| {
            n.checked_add(e.path().as_path().as_os_str().len())
                .ok_or(RuntimeSourcesError::AggregateLimit)
        })?;
        totals.entries = totals
            .entries
            .checked_add(image.entries().len())
            .ok_or(RuntimeSourcesError::AggregateLimit)?;
        totals.file_bytes = totals
            .file_bytes
            .checked_add(image.total_file_bytes())
            .ok_or(RuntimeSourcesError::AggregateLimit)?;
        totals.descriptor_bytes = totals
            .descriptor_bytes
            .checked_add(descriptors)
            .ok_or(RuntimeSourcesError::AggregateLimit)?;
        if totals.entries > limits.max_entries
            || totals.file_bytes > limits.max_total_file_bytes
            || totals.descriptor_bytes > limits.max_image_descriptor_bytes
        {
            return Err(RuntimeSourcesError::AggregateLimit);
        }
        observations = merge(&mut credit, &observations, image.observations())?;
        bindings.push(covered_tool::certify(row, &image, &tools, &active)?);
        images.push(image);
    }
    require_active(&active)?;
    let value = ObservedRuntimeSourceProfile(
        credit
            .freeze(ProfileInner {
                requests,
                tools,
                images,
                bindings,
                observations,
                totals,
            })
            .map_err(|_| RuntimeSourcesError::Operational)?,
    );
    if !value.matches_key(key)
        || !value.same_resources(&r)
        || !value.authorized_by(&tx, &scratch, &r)
    {
        return Err(RuntimeSourcesError::Operational);
    }
    Ok(SourcePreparationOutcome::Complete(Ok(value)))
}
