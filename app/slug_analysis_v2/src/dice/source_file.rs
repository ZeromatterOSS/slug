//! Source existence follows the selected repository owner, never an execution-path string.

use allocative::Allocative;
use dice::DiceComputations;
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositoryPathKey;
use slug_bzlmod_v2::HostRepositoryPathObservationKey;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteKey;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteObservationKey;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;

use super::AnalysisError;
use super::AnalysisSemanticOutcome;
use super::ConfiguredAnalysisMode;

/// The shared source route/path observation, without reading source content.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ObservedSourcePath {
    result: Result<ResolvedPath, AnalysisError>,
    observations: PathObservationEpoch,
}

impl ObservedSourcePath {
    pub fn result(&self) -> &Result<ResolvedPath, AnalysisError> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type SourcePathOutcome =
    LoadingPreparationOutcome<Result<ObservedSourcePath, ObservedPathFrontierError>>;

fn complete(
    result: Result<ResolvedPath, AnalysisError>,
    observations: PathObservationEpoch,
) -> SourcePathOutcome {
    LoadingPreparationOutcome::Complete(Ok(ObservedSourcePath {
        result,
        observations,
    }))
}

/// Resolve a retained source label through the same owners used by analysis.
/// This does not check declaration/visibility or authorize action execution.
#[doc(hidden)]
pub async fn resolve_source_input_observed(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> SourcePathOutcome {
    resolve_source_with_observations(ctx, ConfiguredAnalysisMode::Observed, workspace, label).await
}

pub(super) async fn resolve_source_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> AnalysisSemanticOutcome<ResolvedPath> {
    resolve_source_with_observations(ctx, mode, workspace, label)
        .await
        .map(|result| result.map(|observed| observed.result))
}

async fn resolve_source_with_observations(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> SourcePathOutcome {
    if label.package().repo().is_root() {
        return resolve_root_source_input(ctx, mode, source_path(workspace, label), label).await;
    }
    let mut observations = PathObservationEpoch::empty();
    let repo = label.package().repo().clone();
    let route = match mode {
        ConfiguredAnalysisMode::Legacy => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                workspace.dupe(),
                repo,
            ))
            .await
        {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                return LoadingPreparationOutcome::Need(need);
            }
            Ok(LoadingPreparationOutcome::Complete(result)) => result,
            Err(error) => {
                return complete(
                    Err(AnalysisError::message(format!(
                        "resolving source repository for {label}: {error}"
                    ))),
                    observations,
                );
            }
        },
        ConfiguredAnalysisMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                workspace.dupe(),
                repo,
            ))
            .await
        {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                return LoadingPreparationOutcome::Need(need);
            }
            Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                observations = observed.observations().dupe();
                observed.result().dupe()
            }
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                return complete(
                    Err(AnalysisError::message(format!(
                        "resolving source repository for {label}: {error:?}"
                    ))),
                    observations,
                );
            }
            Err(error) => {
                return complete(
                    Err(AnalysisError::message(format!(
                        "resolving source repository for {label}: {error}"
                    ))),
                    observations,
                );
            }
        },
    };
    let route = match route.as_ref() {
        Ok(route) => HostRepositorySourceRoute::canonical(route.input().clone()),
        Err(error) => {
            return complete(
                Err(AnalysisError::message(format!(
                    "resolving source repository for {label}: {error}"
                ))),
                observations,
            );
        }
    };
    let relative =
        std::path::PathBuf::from(label.package().package().as_str()).join(label.target().as_str());
    let resolved = match mode {
        ConfiguredAnalysisMode::Legacy => match ctx
            .compute(&HostRepositoryPathKey::from_source_route(route, relative))
            .await
        {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                return LoadingPreparationOutcome::Need(need);
            }
            Ok(LoadingPreparationOutcome::Complete(result)) => result,
            Err(error) => {
                return complete(
                    Err(AnalysisError::message(format!(
                        "resolving source file {label} through DICE: {error}"
                    ))),
                    observations,
                );
            }
        },
        ConfiguredAnalysisMode::Observed => match ctx
            .compute(&HostRepositoryPathObservationKey::from_source_route(
                route, relative,
            ))
            .await
        {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                return LoadingPreparationOutcome::Need(need);
            }
            Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                observations = match PathObservationEpoch::from_shared(
                    observations
                        .observations()
                        .iter()
                        .chain(observed.observations().observations().iter())
                        .map(|(demand, result)| (demand.dupe(), result.dupe())),
                ) {
                    Ok(epoch) => epoch,
                    Err(error) => return LoadingPreparationOutcome::Complete(Err(error.into())),
                };
                observed.result().as_ref().clone()
            }
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            Err(error) => {
                return complete(
                    Err(AnalysisError::message(format!(
                        "resolving source file {label} through DICE: {error}"
                    ))),
                    observations,
                );
            }
        },
    };
    complete(
        resolved
            .map(|value| value.resolved().dupe())
            .map_err(|error| {
                AnalysisError::message(format!("resolving source file {label}: {error:?}"))
            }),
        observations,
    )
}
fn source_path(
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> NormalizedAbsolutePath {
    let mut path = workspace.as_path().to_path_buf();
    let package = label.package().package().as_str();
    if !package.is_empty() {
        path.push(package);
    }
    path.push(label.target().as_str());
    NormalizedAbsolutePath::new(path)
        .expect("validated package and target names remain below the absolute workspace path")
}

async fn resolve_root_source_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    path: NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> SourcePathOutcome {
    match mode {
        ConfiguredAnalysisMode::Legacy => {
            match ctx
                .compute(&ResolvedPathKey::new(PathObservationNamespace::Host, path))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Ok(resolved))) => {
                    complete(Ok(resolved), PathObservationEpoch::empty())
                }
                Ok(PathOutcome::Complete(Err(error))) => complete(
                    Err(AnalysisError::new(format!(
                        "resolving source file {label}: {error:?}"
                    ))),
                    PathObservationEpoch::empty(),
                ),
                Err(error) => complete(
                    Err(AnalysisError::new(format!(
                        "resolving source file through DICE: {error}"
                    ))),
                    PathObservationEpoch::empty(),
                ),
            }
        }
        ConfiguredAnalysisMode::Observed => {
            match ctx
                .compute(&ResolvedPathObservationKey::new(
                    PathObservationNamespace::Host,
                    path,
                ))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Err(error))) => {
                    LoadingPreparationOutcome::Complete(Err(error))
                }
                Ok(PathOutcome::Complete(Ok(observed))) => match observed.result() {
                    Ok(resolved) => complete(Ok(resolved.dupe()), observed.observations().dupe()),
                    Err(error) => complete(
                        Err(AnalysisError::new(format!(
                            "resolving source file {label}: {error:?}"
                        ))),
                        observed.observations().dupe(),
                    ),
                },
                Err(error) => complete(
                    Err(AnalysisError::new(format!(
                        "resolving source file through DICE: {error}"
                    ))),
                    PathObservationEpoch::empty(),
                ),
            }
        }
    }
}
