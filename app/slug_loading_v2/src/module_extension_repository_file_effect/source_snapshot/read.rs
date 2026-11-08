//! Anchored source read over the already projected contextual binding.
use super::*;
pub(in crate::module_extension_repository_file_effect) async fn read(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    path: HostRepositoryLabelPathValue,
    binding: RepositoryWorkspaceSourceBinding,
    context: Arc<slug_bzlmod_v2::RepositoryWorkspaceContext>,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<(Arc<[u8]>, PathObservationEpoch), EffectDriver> {
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationKey;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    let fail = |message: String, observations| {
        repository_source_error(certificate, key.ordinal, message, observations)
    };
    let anchor = binding
        .anchor(context.workspace(), context.lease(), &path)
        .map_err(|e| {
            complete_effect_error(
                terminal_repository_rule_invocation_error(certificate, key.ordinal, e),
                observations.dupe(),
            )
        })?;
    let resolved_key =
        slug_workspace_v2::AnchoredResolvedPathObservationKey::new(anchor, path.path().as_path())
            .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    let resolved = match ctx
        .compute(&resolved_key)
        .await
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
    {
        PathOutcome::Need(needs) => {
            return Err(SourcePreparationOutcome::Need(
                SourcePreparationNeeds::path(needs),
            ));
        }
        PathOutcome::Complete(Err(error)) => return Err(fail(error.to_string(), observations)),
        PathOutcome::Complete(Ok(value)) => value,
    };
    if matches!(mode, EffectMode::Observed) {
        observations = merge_definition_observations(
            mode,
            certificate,
            key.ordinal,
            observations,
            resolved.observations(),
        )?;
    }
    let resolved = resolved
        .result()
        .as_ref()
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    if !matches!(resolved.state(),ResolvedPathState::Present(metadata) if metadata.kind()==PathNodeKind::RegularFile)
    {
        return Err(fail(
            "source read requires a regular file".to_owned(),
            observations,
        ));
    }
    let demand = PathObservationDemand::new(
        resolved.content_origin().namespace(),
        resolved.real_path().clone(),
        PathObservationOperation::FileBytes,
    );
    let value = match ctx
        .compute(&PathObservationKey::new(demand.clone()))
        .await
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
    {
        PathOutcome::Need(needs) => {
            return Err(SourcePreparationOutcome::Need(
                SourcePreparationNeeds::path(needs),
            ));
        }
        PathOutcome::Complete(value) => value,
    };
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) = value.as_ref()
    else {
        return Err(fail(
            "source read was absent, unstable or unsupported".to_owned(),
            observations,
        ));
    };
    if matches!(mode, EffectMode::Observed) {
        let incoming = PathObservationEpoch::from_shared([(demand, value.clone())])
            .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
        observations =
            merge_definition_observations(mode, certificate, key.ordinal, observations, &incoming)?;
    }
    Ok((bytes.clone(), observations))
}
