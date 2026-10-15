//! One guarded canonical source-input driver for child loads and defining maps.
use super::*;

enum InputContext<'a> {
    Child {
        source: CanonicalLabel,
        load: &'a str,
    },
    DefiningRepository {
        source: CanonicalLabel,
    },
}

impl InputContext<'_> {
    fn failure(
        self,
        error: ExternalBzlRouteError,
        observations: PathObservationEpoch,
    ) -> ExternalBzlDriverOutcome {
        let error = match self {
            Self::Child { source, load } => ExternalBzlModuleError::Route {
                source,
                load: Arc::from(load),
                error,
            },
            Self::DefiningRepository { source } => {
                ExternalBzlModuleError::RepositoryMapping { source, error }
            }
        };
        external_bzl_complete(Err(error), observations)
    }
}

pub(super) async fn compute_canonical_external_child_input(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    child_repo: CanonicalRepoName,
    source: CanonicalLabel,
    load: &str,
    mode: ExternalBzlModuleMode,
    observations: PathObservationEpoch,
) -> ControlFlow<
    ExternalBzlDriverOutcome,
    (
        slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        PathObservationEpoch,
    ),
> {
    canonical_input(
        ctx,
        workspace,
        child_repo,
        InputContext::Child { source, load },
        mode,
        observations,
    )
    .await
}

async fn canonical_input(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    child_repo: CanonicalRepoName,
    request: InputContext<'_>,
    mode: ExternalBzlModuleMode,
    observations: PathObservationEpoch,
) -> ControlFlow<
    ExternalBzlDriverOutcome,
    (
        slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
        PathObservationEpoch,
    ),
> {
    match mode {
        ExternalBzlModuleMode::Legacy => match compute_external_guarded(
            ctx,
            &HostCanonicalRepositoryLoadRouteKey::new(workspace, child_repo),
        )
        .await
        {
            Err(DependencyComputeError::Cycle(cycle)) => ControlFlow::Break(external_bzl_complete(
                Err(ExternalBzlModuleError::DependencyCycle(cycle)),
                observations,
            )),
            Err(error) => ControlFlow::Break(request.failure(
                ExternalBzlRouteError::Compute(Arc::from(format!("{error:?}"))),
                observations,
            )),
            Ok(SourcePreparationOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::Need(need))
            }
            Ok(SourcePreparationOutcome::Complete(result)) => match result.as_ref() {
                Ok(route) => ControlFlow::Continue((route.input().clone(), observations)),
                Err(error) => ControlFlow::Break(request.failure(
                    ExternalBzlRouteError::Load(Box::new(error.clone())),
                    observations,
                )),
            },
        },
        ExternalBzlModuleMode::Observed => match compute_external_guarded(
            ctx,
            &HostCanonicalRepositoryLoadRouteObservationKey::new(workspace, child_repo),
        )
        .await
        {
            Err(DependencyComputeError::Cycle(cycle)) => ControlFlow::Break(external_bzl_complete(
                Err(ExternalBzlModuleError::DependencyCycle(cycle)),
                observations,
            )),
            Err(error) => ControlFlow::Break(request.failure(
                ExternalBzlRouteError::Compute(Arc::from(format!("{error:?}"))),
                observations,
            )),
            Ok(SourcePreparationOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::Need(need))
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                ControlFlow::Break(request.failure(
                    ExternalBzlRouteError::Observation(Arc::from(format!("{error:?}"))),
                    observations,
                ))
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                let observations =
                    match union_host_observations(&observations, observed.observations()) {
                        Ok(observations) => observations,
                        Err(error) => {
                            return ControlFlow::Break(SourcePreparationOutcome::Complete(Err(
                                error,
                            )));
                        }
                    };
                match observed.result().as_ref() {
                    Ok(route) => ControlFlow::Continue((route.input().clone(), observations)),
                    Err(error) => ControlFlow::Break(request.failure(
                        ExternalBzlRouteError::Load(Box::new(error.clone())),
                        observations,
                    )),
                }
            }
        },
    }
}

pub(super) async fn compute_external_bzl_effective_route(
    ctx: &mut DiceComputations<'_>,
    key: &ExternalBzlModuleEvalKey,
    source: CanonicalLabel,
    mode: ExternalBzlModuleMode,
    observations: PathObservationEpoch,
) -> ControlFlow<ExternalBzlDriverOutcome, (HostRepositorySourceRoute, PathObservationEpoch)> {
    let HostRepositorySourceRoute::Root(route) = &key.route else {
        return ControlFlow::Continue((key.route.clone(), observations));
    };
    if !route.is_builtin_bazel_tools() {
        return ControlFlow::Continue((key.route.clone(), observations));
    }
    // Every ordinary defining builtin module uses the complete selected map;
    // its import syntax and source name never select this ownership boundary.
    match canonical_input(
        ctx,
        route.workspace().dupe(),
        route.canonical_repo().clone(),
        InputContext::DefiningRepository { source },
        mode,
        observations,
    )
    .await
    {
        ControlFlow::Continue((input, observations)) => {
            ControlFlow::Continue((HostRepositorySourceRoute::canonical(input), observations))
        }
        ControlFlow::Break(outcome) => ControlFlow::Break(outcome),
    }
}

pub(super) fn defining_repository_mapping(
    route: &HostRepositorySourceRoute,
) -> Result<Arc<[(ApparentRepoName, CanonicalRepoName)]>, String> {
    if let HostRepositorySourceRoute::Canonical(input) = route {
        if let Some(mapping) = input.view().route().builtin_repository_mapping() {
            // The actual input owns the immutable producer across both borrowed
            // preflight/copy traversals. No allocating entries() call precedes it.
            return crate::bzl_environment::BuiltinRepositoryMapping::from_tools(mapping)
                .map(crate::bzl_environment::BuiltinRepositoryMapping::into_entries);
        }
    }
    // Other existing mapping families retain their current copy contract.
    Ok(external_repository_mapping(route))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defining_mapping_refusal_keeps_typed_source_and_admission_cause() {
        let source =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:cache.bzl").unwrap();
        let outcome = InputContext::DefiningRepository {
            source: source.clone(),
        }
        .failure(
            ExternalBzlRouteError::Admission(
                "semantic .bzl evaluation input resource limit".into(),
            ),
            PathObservationEpoch::empty(),
        );
        let SourcePreparationOutcome::Complete(Ok((result, observations))) = outcome else {
            panic!("typed defining-map refusal is a complete terminal")
        };
        assert!(observations.observations().is_empty());
        assert!(
            matches!(result.as_ref(), Err(ExternalBzlModuleError::RepositoryMapping { source: actual, error: ExternalBzlRouteError::Admission(message) })
            if actual == &source && message.as_ref() == "semantic .bzl evaluation input resource limit")
        );
    }
}
