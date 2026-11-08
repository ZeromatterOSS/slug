//! Source images are demanded before repository-context path text escapes.
use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositorySourceSnapshotEpochKey;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;

use super::*;

pub(super) async fn path(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: &RepositoryLabelPathAddress,
    route: Option<&HostRepositorySourceRoute>,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<(Option<HostRepositoryLabelPathValue>, PathObservationEpoch), EffectDriver> {
    let fail = |message: String, observations| {
        complete_effect_error(
            host_input_error(certificate, key.ordinal, message),
            observations,
        )
    };
    let Some(workspace_epoch) = general::fence(ctx, key).await? else {
        return Ok((None, observations));
    };
    // Bazel RepositoryUtils.getRootedPathFromLabel requires a package lookup,
    // including deletion/ignore policy, but does not require the target to exist.
    // Retain the ordinary package producer and its original Host certificate.
    // Its legacy BuiltinCatalog refusal follows successful package validation;
    // only the distinct catalog snapshot producer supplies that missing path.
    let package_key = match route {
        Some(route) => {
            HostRepositoryLabelPathObservationKey::new_external(route.clone(), address.clone())
        }
        None => {
            HostRepositoryLabelPathObservationKey::new_root(key.workspace.dupe(), address.clone())
        }
    }
    .map_err(|error| {
        complete_label_path_error(
            certificate,
            key.ordinal,
            address.clone(),
            error,
            observations.dupe(),
        )
    })?;
    let package = match ctx
        .compute(&package_key)
        .await
        .map_err(|e| fail(e.to_string(), observations.dupe()))?
    {
        SourcePreparationOutcome::Need(needs) => return Err(SourcePreparationOutcome::Need(needs)),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(fail(error.to_string(), observations));
        }
        SourcePreparationOutcome::Complete(Ok(package)) => package,
    };
    match package.result().as_ref() {
        Ok(_) => {}
        Err(HostRepositoryLabelPathError::BuiltinCatalog) if matches!(route,Some(HostRepositorySourceRoute::Canonical(input)) if matches!(input.view().disposition(),HostRepositorySourceInputDispositionView::Builtin(_))) =>
            {}
        Err(error) => {
            return Err(complete_label_path_error(
                certificate,
                key.ordinal,
                address.clone(),
                error.clone(),
                observations,
            ));
        }
    }
    observations = merge_definition_observations(
        mode,
        certificate,
        key.ordinal,
        observations,
        package.observations(),
    )?;
    let lease = workspace_epoch.lease().expect("validated command fence");
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
    let epoch = ctx
        .compute(&RepositorySourceSnapshotEpochKey::new(key.workspace.dupe()))
        .await
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    let Some(snapshot) = epoch
        .find(&request)
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
    else {
        return Err(SourcePreparationOutcome::Need(
            SourcePreparationNeeds::source_snapshot(Arc::new(request)),
        ));
    };
    let relative = std::path::Path::new(address.package().as_str()).join(address.target().as_str());
    let absolute = NormalizedAbsolutePath::new(
        snapshot
            .reservation()
            .display_root()
            .as_path()
            .join(relative),
    )
    .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    let path = HostRepositoryLabelPathValue::new(
        absolute,
        PathObservationNamespace::Materialization(snapshot.reservation().instance()),
    )
    .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    Ok((Some(path), observations))
}

pub(super) async fn read(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    repository: &slug_identity_v2::CanonicalRepoName,
    path: HostRepositoryLabelPathValue,
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
    let epoch = ctx
        .compute(&RepositorySourceSnapshotEpochKey::new(key.workspace.dupe()))
        .await
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
    let root = if let Some(source) = epoch.states().and_then(|states| {
        states.iter().find(|s| {
            s.reservation().repository() == repository
                && path.namespace()
                    == PathObservationNamespace::Materialization(s.reservation().instance())
        })
    }) {
        source.reservation().display_root().clone()
    } else {
        let selected = ctx
            .compute(&slug_bzlmod_v2::RepositoryMaterializationResultEpochKey {
                workspace: key.workspace.dupe(),
            })
            .await
            .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
        let entry = selected.entry_for_repository(repository).ok_or_else(|| {
            fail(
                "missing selected contextual source".to_owned(),
                observations.dupe(),
            )
        })?;
        match &entry.result {
            slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                    generation_root,
                    observation_instance,
                    ..
                },
            ) if path.namespace()
                == PathObservationNamespace::Materialization(*observation_instance) =>
            {
                NormalizedAbsolutePath::new(generation_root)
                    .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?
            }
            _ => {
                return Err(fail(
                    "contextual source association differs".to_owned(),
                    observations,
                ));
            }
        }
    };
    let anchor = slug_workspace_v2::MaterializationPathAnchor::new(path.namespace(), root)
        .map_err(|e| fail(format!("{e:?}"), observations.dupe()))?;
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
        path.namespace(),
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
