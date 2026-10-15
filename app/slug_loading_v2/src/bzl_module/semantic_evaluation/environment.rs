//! Tracked environment preparation before any ordinary semantic-key lookup.
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingKey;
use slug_bzlmod_v2::HostBuiltinBazelToolsRepositoryMappingObservationKey;

use super::*;
use crate::bzl_environment::BuiltinRepositoryMapping;
use crate::bzl_environment::BuiltinSourceCatalogKey;
use crate::bzl_environment::BzlEnvironmentMode;
use crate::bzl_environment::PreparedBzlEnvironment;
#[derive(Debug)]
pub(in crate::bzl_module) enum EnvironmentFailure {
    Path(ObservedPathFrontierError),
    Message(String),
}
pub(in crate::bzl_module) async fn load(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    context: BzlModuleContext,
    observed: bool,
    observations: &mut PathObservationEpoch,
) -> SourcePreparationOutcome<Result<PreparedBzlEnvironment, EnvironmentFailure>> {
    // Ordinary MODULE/repository-loaded .bzl also receives actual builtin
    // exports; only its native base differs from BUILD. Explicit bootstrap
    // and private builtin preparation never enter this ordinary route.
    let mode = match context {
        BzlModuleContext::Build => BzlEnvironmentMode::Build,
        BzlModuleContext::Bzlmod => BzlEnvironmentMode::Bzlmod,
    };
    let result = if observed {
        let key = HostBuiltinBazelToolsRepositoryMappingObservationKey::new(workspace.dupe());
        match ctx.compute(&key).await {
            Err(e) => {
                return SourcePreparationOutcome::Complete(Err(EnvironmentFailure::Message(
                    e.to_string(),
                )));
            }
            Ok(SourcePreparationOutcome::Need(n)) => return SourcePreparationOutcome::Need(n),
            Ok(SourcePreparationOutcome::Complete(Err(e))) => {
                return SourcePreparationOutcome::Complete(Err(match e.selected_frontier() {
                    HostSelectedObservationFrontier::Path(p) => EnvironmentFailure::Path(p),
                    HostSelectedObservationFrontier::Infrastructure(m) => {
                        EnvironmentFailure::Message(m.to_string())
                    }
                }));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                *observations = match union_host_observations(observations, value.observations()) {
                    Ok(v) => v,
                    Err(e) => {
                        return SourcePreparationOutcome::Complete(Err(EnvironmentFailure::Path(
                            e,
                        )));
                    }
                };
                value.result().clone()
            }
        }
    } else {
        let key = HostBuiltinBazelToolsRepositoryMappingKey::new(workspace.dupe());
        match ctx.compute(&key).await {
            Err(e) => {
                return SourcePreparationOutcome::Complete(Err(EnvironmentFailure::Message(
                    e.to_string(),
                )));
            }
            Ok(SourcePreparationOutcome::Need(n)) => return SourcePreparationOutcome::Need(n),
            Ok(SourcePreparationOutcome::Complete(v)) => v,
        }
    };
    let result = match result.as_ref() {
        Ok(v) => v,
        Err(e) => {
            return SourcePreparationOutcome::Complete(Err(EnvironmentFailure::Message(
                e.to_string(),
            )));
        }
    };
    // This typed producer proves canonical tools ownership. A main apparent
    // alias must not redirect the private builtin program's defining mapping.
    let mapping = match BuiltinRepositoryMapping::from_tools(result) {
        Ok(v) => v,
        Err(e) => return SourcePreparationOutcome::Complete(Err(EnvironmentFailure::Message(e))),
    };
    SourcePreparationOutcome::Complete(
        crate::bzl_environment::prepare_source_environment(
            ctx,
            &BuiltinSourceCatalogKey::Bundled(
                crate::bzl_environment::BuiltinCatalogSnapshot::Bazel9_2,
            ),
            &mapping,
            mode,
        )
        .await
        .map_err(EnvironmentFailure::Message),
    )
}
