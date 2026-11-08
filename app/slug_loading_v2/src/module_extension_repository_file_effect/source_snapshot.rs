//! Source images are demanded before repository-context path text escapes.
use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;
#[path = "source_snapshot/request.rs"]
mod request;

use super::*;

pub(super) async fn path(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    address: &RepositoryLabelPathAddress,
    route: Option<&HostRepositorySourceRoute>,
    context: Option<&Arc<slug_bzlmod_v2::RepositoryWorkspaceContext>>,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
) -> Result<
    (
        Option<(
            HostRepositoryLabelPathValue,
            RepositoryWorkspaceSourceBinding,
        )>,
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
    let Some(context) = context else {
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
    let (request, observations) =
        request::request(ctx, key, certificate, route, context, mode, observations).await?;
    let Some(request) = request else {
        return Ok((None, observations));
    };
    let request = Arc::new(request);
    let observed = source_binding::source(ctx, context.clone(), request.clone())
        .await
        .map_err(|e| fail(e, observations.dupe()))?;
    let Some(snapshot) = observed.state() else {
        return Err(SourcePreparationOutcome::Need(
            SourcePreparationNeeds::source_snapshot(request),
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
    let binding = RepositoryWorkspaceSourceBinding::Source(snapshot.clone());
    binding
        .validate_path(context.workspace(), context.lease(), &path)
        .map_err(|e| {
            complete_effect_error(
                terminal_repository_rule_invocation_error(certificate, key.ordinal, e),
                observations.dupe(),
            )
        })?;
    Ok((Some((path, binding)), observations))
}

#[path = "source_snapshot/read.rs"]
mod read;
pub(super) use read::read;
