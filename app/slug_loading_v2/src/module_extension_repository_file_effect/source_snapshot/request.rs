//! Natural contextual source requests, including tracked root mapping.
use super::*;
pub(super) async fn request(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    route: Option<&HostRepositorySourceRoute>,
    context: &Arc<slug_bzlmod_v2::RepositoryWorkspaceContext>,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<
    (
        Option<RepositorySourceSnapshotRequest>,
        PathObservationEpoch,
    ),
    EffectDriver,
> {
    let fail = |message: String, observations| {
        complete_effect_error(
            host_input_error(certificate, key.ordinal, message),
            observations,
        )
    };
    let lease = context.lease();
    let request = match route {
        Some(HostRepositorySourceRoute::Canonical(input)) => {
            if matches!(input.view().disposition(), HostRepositorySourceInputDispositionView::Request(r) if matches!(r.kind,RepositoryMaterializationKind::GeneralWorkspace(_)))
            {
                return Ok((None, observations));
            }
            RepositorySourceSnapshotRequest::from_canonical(input, lease)
                .map_err(|e| fail(e.to_string(), observations.dupe()))?
        }
        Some(HostRepositorySourceRoute::Root(_)) => {
            return Err(fail(
                "noncanonical contextual source route".to_owned(),
                observations,
            ));
        }
        None => {
            let mapping = match mode {
                EffectMode::Legacy => match ctx
                    .compute(&slug_bzlmod_v2::HostRootRepositoryMappingKey::new(
                        key.workspace.dupe(),
                    ))
                    .await
                    .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
                {
                    SourcePreparationOutcome::Need(needs) => {
                        return Err(SourcePreparationOutcome::Need(needs));
                    }
                    SourcePreparationOutcome::Complete(value) => value,
                },
                EffectMode::Observed => {
                    let observed = match ctx
                        .compute(
                            &slug_bzlmod_v2::HostRootRepositoryMappingObservationKey::new(
                                key.workspace.dupe(),
                            ),
                        )
                        .await
                        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
                    {
                        SourcePreparationOutcome::Need(needs) => {
                            return Err(SourcePreparationOutcome::Need(needs));
                        }
                        SourcePreparationOutcome::Complete(Err(error)) => {
                            return Err(fail(format!("{error:?}"), observations));
                        }
                        SourcePreparationOutcome::Complete(Ok(value)) => value,
                    };
                    observations = merge_definition_observations(
                        mode,
                        certificate,
                        key.ordinal,
                        observations,
                        observed.observations(),
                    )?;
                    observed.result().clone()
                }
            };
            let mapping = mapping
                .as_ref()
                .as_ref()
                .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
            RepositorySourceSnapshotRequest::from_workspace(key.workspace.dupe(), mapping, lease)
                .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
        }
    };
    Ok((Some(request), observations))
}
