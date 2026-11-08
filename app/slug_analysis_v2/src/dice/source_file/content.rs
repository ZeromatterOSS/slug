//! Immutable bytes from the exact observed source route; no host IO/path fallback.
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::ResolvedPathState;

use super::*;

/// Successful immutable source body. Constructors and variant choice stay private.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct SourceFileContent(Content);
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum Content {
    Catalog(Arc<Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError>>),
    Filesystem(Arc<PathObservationResult>),
}
impl SourceFileContent {
    pub fn bytes(&self) -> &[u8] {
        match &self.0 {
            Content::Catalog(owner) => owner
                .as_ref()
                .as_ref()
                .expect("private successful catalog content")
                .bytes(),
            Content::Filesystem(owner) => match owner.as_ref() {
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => bytes,
                _ => unreachable!("private successful FileBytes content"),
            },
        }
    }
    pub fn catalog_owner(
        &self,
    ) -> Option<&Arc<Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError>>>
    {
        match &self.0 {
            Content::Catalog(owner) => Some(owner),
            Content::Filesystem(_) => None,
        }
    }
}
/// The complete route/resolution/content observation epoch accompanies every result.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ObservedSourceFile {
    result: Result<SourceFileContent, AnalysisError>,
    observations: PathObservationEpoch,
}
impl ObservedSourceFile {
    pub fn result(&self) -> &Result<SourceFileContent, AnalysisError> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}
fn completed(
    result: Result<SourceFileContent, AnalysisError>,
    observations: PathObservationEpoch,
) -> LoadingPreparationOutcome<Result<ObservedSourceFile, ObservedPathFrontierError>> {
    LoadingPreparationOutcome::Complete(Ok(ObservedSourceFile {
        result,
        observations,
    }))
}
fn finish_file_bytes(
    observations: PathObservationEpoch,
    demand: PathObservationDemand,
    response: Arc<PathObservationResult>,
    label: &CanonicalLabel,
) -> Result<ObservedSourceFile, ObservedPathFrontierError> {
    let observations = PathObservationEpoch::from_shared(
        observations
            .observations()
            .iter()
            .map(|(d, r)| (d.dupe(), r.dupe()))
            .chain(std::iter::once((demand, response.dupe()))),
    )?;
    let result = match response.as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(_)) => {
            Ok(SourceFileContent(Content::Filesystem(response)))
        }
        PathObservationResult::FileBytes(PathOperationResult::Missing) => {
            Err(AnalysisError::message(format!(
                "observed source file {label} disappeared before content acquisition"
            )))
        }
        PathObservationResult::FileBytes(PathOperationResult::Error(error)) => Err(
            AnalysisError::message(format!("reading observed source file {label}: {error:?}")),
        ),
        _ => Err(AnalysisError::message(format!(
            "observed source file {label} returned a mismatched FileBytes response"
        ))),
    };
    Ok(ObservedSourceFile {
        result,
        observations,
    })
}
/// Acquire an ordinary or catalog Source through existing observed owners only.
#[doc(hidden)]
pub async fn resolve_source_file_input_observed(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> LoadingPreparationOutcome<Result<ObservedSourceFile, ObservedPathFrontierError>> {
    let observed = match resolve_source_with_observations(
        ctx,
        ConfiguredAnalysisMode::Observed,
        workspace,
        label,
        SourceAcquisition::FilesystemOrCatalog,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(observed)) => observed,
    };
    let source = match observed.result {
        Err(error) => return completed(Err(error), observed.observations),
        Ok(source) => source,
    };
    let resolved = match source {
        SourceResolution::CatalogFile(owner) => {
            return completed(
                Ok(SourceFileContent(Content::Catalog(owner))),
                observed.observations,
            );
        }
        SourceResolution::Filesystem(resolved) => resolved,
    };
    match resolved.state() {
        ResolvedPathState::Present(metadata) if metadata.kind() == PathNodeKind::RegularFile => {}
        ResolvedPathState::Missing => {
            return completed(
                Err(AnalysisError::message(format!(
                    "observed source file {label} is missing"
                ))),
                observed.observations,
            );
        }
        ResolvedPathState::Present(metadata) => {
            return completed(
                Err(AnalysisError::message(format!(
                    "observed source file {label} is not a regular file: {:?}",
                    metadata.kind()
                ))),
                observed.observations,
            );
        }
    }
    let demand = PathObservationDemand::new(
        resolved.content_origin().namespace(),
        resolved.real_path().dupe(),
        PathObservationOperation::FileBytes,
    );
    match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
        Err(error) => completed(
            Err(AnalysisError::message(format!(
                "reading observed source file {label} through DICE: {error}"
            ))),
            observed.observations,
        ),
        Ok(PathOutcome::Need(need)) => {
            LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
        }
        Ok(PathOutcome::Complete(response)) => LoadingPreparationOutcome::Complete(
            finish_file_bytes(observed.observations, demand, response, label),
        ),
    }
}
#[cfg(test)]
mod tests;
