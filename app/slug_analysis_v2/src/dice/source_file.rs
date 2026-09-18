//! Source existence follows the selected repository owner, never an execution-path string.

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
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPath;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;

use super::AnalysisError;
use super::AnalysisSemanticOutcome;
use super::ConfiguredAnalysisMode;
use super::analysis_semantic_complete;

pub(super) async fn resolve_source_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> AnalysisSemanticOutcome<ResolvedPath> {
    if label.package().repo().is_root() {
        return resolve_root_source_input(ctx, mode, source_path(workspace, label), label).await;
    }
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
                return analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "resolving source repository for {label}: {error}"
                ))));
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
            Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => observed.result().dupe(),
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                return analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "resolving source repository for {label}: {error:?}"
                ))));
            }
            Err(error) => {
                return analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "resolving source repository for {label}: {error}"
                ))));
            }
        },
    };
    let route = match route.as_ref() {
        Ok(route) => HostRepositorySourceRoute::canonical(route.input().clone()),
        Err(error) => {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "resolving source repository for {label}: {error}"
            ))));
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
                return analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "resolving source file {label} through DICE: {error}"
                ))));
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
                observed.result().as_ref().clone()
            }
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            Err(error) => {
                return analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "resolving source file {label} through DICE: {error}"
                ))));
            }
        },
    };
    analysis_semantic_complete(
        resolved
            .map(|value| value.resolved().dupe())
            .map_err(|error| {
                AnalysisError::message(format!("resolving source file {label}: {error:?}"))
            }),
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
) -> AnalysisSemanticOutcome<slug_workspace_v2::ResolvedPath> {
    match mode {
        ConfiguredAnalysisMode::Legacy => {
            match ctx
                .compute(&ResolvedPathKey::new(PathObservationNamespace::Host, path))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Ok(resolved))) => analysis_semantic_complete(Ok(resolved)),
                Ok(PathOutcome::Complete(Err(error))) => analysis_semantic_complete(Err(
                    AnalysisError::new(format!("resolving source file {label}: {error:?}")),
                )),
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "resolving source file through DICE: {error}"
                )))),
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
                    Ok(resolved) => analysis_semantic_complete(Ok(resolved.dupe())),
                    Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                        "resolving source file {label}: {error:?}"
                    )))),
                },
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "resolving source file through DICE: {error}"
                )))),
            }
        }
    }
}
