//! Original-evaluation fences and immutable reservation binding.
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicyKey;
use slug_bzlmod_v2::RepositoryWorkspaceObservationKey;
use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;
use slug_bzlmod_v2::source_preparation::RepositoryWorkspaceStableKey;

use super::*;

pub(super) async fn fence(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
) -> Result<Option<RepositoryWorkspaceEpoch>, EffectDriver> {
    let policy = ctx
        .compute(&RepositoryWorkspaceExecutionPolicyKey::new(
            key.workspace.dupe(),
        ))
        .await
        .map_err(|error| {
            complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                PathObservationEpoch::empty(),
            )
        })?;
    if policy == RepositoryWorkspaceExecutionPolicy::Disabled {
        return Ok(None);
    }
    // Check injection first: the observation key assumes Core's active command.
    ctx.compute(&RepositoryWorkspaceEpochKey::new(key.workspace.dupe()))
        .await
        .map_err(|error| {
            complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                PathObservationEpoch::empty(),
            )
        })?;
    let epoch = ctx
        .compute(&RepositoryWorkspaceObservationKey::new(
            key.workspace.dupe(),
        ))
        .await
        .map_err(|error| {
            complete_effect_error(
                HostSelectedRepositoryFileEffectError::Compute(error.to_string().into()),
                PathObservationEpoch::empty(),
            )
        })?;
    if epoch.workspace() != Some(&key.workspace) || epoch.lease().is_none() {
        return Err(complete_effect_error(
            HostSelectedRepositoryFileEffectError::Compute(
                "repository workspace epoch is revoked or foreign".into(),
            ),
            PathObservationEpoch::empty(),
        ));
    }
    Ok(Some(epoch))
}
pub(super) fn reservation(
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    repository: &HostInstantiatedModuleExtensionRepository,
    epoch: &RepositoryWorkspaceEpoch,
    observations: &PathObservationEpoch,
) -> Result<RepositoryWorkspaceReservationDemand, EffectDriver> {
    let (name, spec) = repository.spec_parts();
    let demand = RepositoryWorkspaceStableKey::new(
        key.workspace.dupe(),
        key.owner.clone(),
        name.clone(),
        key.ordinal,
    )
    .and_then(|key| {
        RepositoryWorkspaceReservationDemand::new(Arc::new(key), Arc::new(spec.clone()))
    })
    .map_err(|error| {
        complete_effect_error(
            host_input_error(certificate, key.ordinal, error.to_string()),
            observations.dupe(),
        )
    })?;
    match demand.find_in(epoch) {
        Ok(Some(_)) => Ok(demand),
        Ok(None) => Err(SourcePreparationOutcome::Need(
            SourcePreparationNeeds::repository_workspace_reservation(Arc::new(demand)),
        )),
        Err(error) => Err(complete_effect_error(
            host_input_error(certificate, key.ordinal, error.to_string()),
            observations.dupe(),
        )),
    }
}
pub(super) async fn selected_epoch(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    observations: &PathObservationEpoch,
) -> Result<RepositoryMaterializationResultEpoch, EffectDriver> {
    ctx.compute(&RepositoryMaterializationResultEpochKey {
        workspace: key.workspace.dupe(),
    })
    .await
    .map_err(|error| {
        complete_effect_error(
            host_input_error(certificate, key.ordinal, error.to_string()),
            observations.dupe(),
        )
    })
}

pub(super) async fn read_host(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    path: NormalizedAbsolutePath,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<((NormalizedAbsolutePath, Arc<[u8]>), PathObservationEpoch), EffectDriver> {
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationKey;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    let fail = |message: String, observations| {
        complete_effect_error(
            host_input_error(certificate, key.ordinal, message),
            observations,
        )
    };
    let resolved = match ctx
        .compute(&ResolvedPathObservationKey::new(
            PathObservationNamespace::Host,
            path.clone(),
        ))
        .await
        .map_err(|error| fail(error.to_string(), observations.dupe()))?
    {
        PathOutcome::Need(need) => {
            return Err(SourcePreparationOutcome::Need(
                SourcePreparationNeeds::path(need),
            ));
        }
        PathOutcome::Complete(Err(error)) => return Err(fail(error.to_string(), observations)),
        PathOutcome::Complete(Ok(observed)) => observed,
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
    let resolved = resolved.result().as_ref().map_err(|error| {
        fail(
            format!("Host read resolution: {error:?}"),
            observations.dupe(),
        )
    })?;
    if !matches!(resolved.state(),ResolvedPathState::Present(metadata) if metadata.kind() == PathNodeKind::RegularFile)
    {
        return Err(fail(
            "repository_ctx.read requires a regular file".to_owned(),
            observations,
        ));
    }
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        resolved.real_path().clone(),
        PathObservationOperation::FileBytes,
    );
    let observed = match ctx
        .compute(&PathObservationKey::new(demand.clone()))
        .await
        .map_err(|error| fail(error.to_string(), observations.dupe()))?
    {
        PathOutcome::Need(need) => {
            return Err(SourcePreparationOutcome::Need(
                SourcePreparationNeeds::path(need),
            ));
        }
        PathOutcome::Complete(result) => result,
    };
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) = observed.as_ref()
    else {
        return Err(fail(
            "repository_ctx.read has missing, unstable, or unsupported file bytes".to_owned(),
            observations,
        ));
    };
    if matches!(mode, EffectMode::Observed) {
        let incoming = PathObservationEpoch::from_shared([(demand, observed.clone())])
            .map_err(|error| fail(error.to_string(), observations.dupe()))?;
        observations =
            merge_definition_observations(mode, certificate, key.ordinal, observations, &incoming)?;
    }
    Ok(((path, bytes.clone()), observations))
}
