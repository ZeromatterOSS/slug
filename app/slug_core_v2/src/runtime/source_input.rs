//! Observed source content for execution staging, never execution authority.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_analysis_v2::AnalysisError;
use slug_analysis_v2::resolve_source_input_observed;
use slug_build_api_v2::AnalysisArtifact;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathFileDigest;
use slug_workspace_v2::PathFileDigestError;
use slug_workspace_v2::PathFileDigestObservationKey;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPath;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum SourceArtifactInputError {
    NotSource,
    Path(AnalysisError),
    Digest(PathFileDigestError),
    Missing,
    Compute(Arc<str>),
}

/// Content/path projection. Timestamp and symlink provenance live in the epoch.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct SourceArtifactInput {
    label: CanonicalLabel,
    namespace: PathObservationNamespace,
    requested_path: NormalizedAbsolutePath,
    real_path: NormalizedAbsolutePath,
    digest: FileContentDigest,
}

impl SourceArtifactInput {
    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }
    pub fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }
    pub fn requested_path(&self) -> &NormalizedAbsolutePath {
        &self.requested_path
    }
    pub fn real_path(&self) -> &NormalizedAbsolutePath {
        &self.real_path
    }
    pub fn digest(&self) -> FileContentDigest {
        self.digest
    }
}

/// Consumers must validate the full frontier before publishing a request result.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ObservedSourceArtifactInput {
    result: Result<SourceArtifactInput, SourceArtifactInputError>,
    observations: PathObservationEpoch,
}

impl ObservedSourceArtifactInput {
    pub fn result(&self) -> &Result<SourceArtifactInput, SourceArtifactInputError> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

pub type SourceArtifactInputOutcome =
    LoadingPreparationOutcome<Result<Arc<ObservedSourceArtifactInput>, ObservedPathFrontierError>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SourceArtifactInputObservationKey {
    workspace: NormalizedAbsolutePath,
    label: CanonicalLabel,
}

impl SourceArtifactInputObservationKey {
    /// The caller supplies a retained source from its validated action closure.
    /// This type check is not a declaration, visibility or execution check.
    pub fn new(
        workspace: NormalizedAbsolutePath,
        artifact: &AnalysisArtifact,
    ) -> Result<Self, SourceArtifactInputError> {
        match artifact {
            AnalysisArtifact::Source(label) => Ok(Self {
                workspace,
                label: label.clone(),
            }),
            AnalysisArtifact::Derived { .. } => Err(SourceArtifactInputError::NotSource),
        }
    }
}

impl fmt::Display for SourceArtifactInputObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observed-source-artifact-input:{}:{}",
            self.workspace, self.label
        )
    }
}

fn complete(
    result: Result<SourceArtifactInput, SourceArtifactInputError>,
    observations: PathObservationEpoch,
) -> SourceArtifactInputOutcome {
    LoadingPreparationOutcome::Complete(Ok(Arc::new(ObservedSourceArtifactInput {
        result,
        observations,
    })))
}

#[async_trait]
impl Key for SourceArtifactInputObservationKey {
    type Value = SourceArtifactInputOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let path = match resolve_source_input_observed(ctx, &self.workspace, &self.label).await {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(path)) => path,
        };
        match path.result() {
            Err(error) => complete(
                Err(SourceArtifactInputError::Path(error.clone())),
                path.observations().dupe(),
            ),
            Ok(resolved) => {
                finish_source_input(ctx, &self.label, resolved, path.observations()).await
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        Self::validity(x) && Self::validity(y) && x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        matches!(value, LoadingPreparationOutcome::Complete(Ok(observed)) if observed.result.is_ok())
    }
}

async fn finish_source_input(
    ctx: &mut DiceComputations<'_>,
    label: &CanonicalLabel,
    resolved: &ResolvedPath,
    prefix: &PathObservationEpoch,
) -> SourceArtifactInputOutcome {
    let observed = match ctx
        .compute(&PathFileDigestObservationKey::new(
            resolved.namespace(),
            resolved.requested_path().dupe(),
        ))
        .await
    {
        Ok(PathOutcome::Need(need)) => {
            return LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need));
        }
        Ok(PathOutcome::Complete(Err(error))) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        Ok(PathOutcome::Complete(Ok(observed))) => observed,
        Err(error) => {
            return complete(
                Err(SourceArtifactInputError::Compute(error.to_string().into())),
                prefix.dupe(),
            );
        }
    };
    let observations = match PathObservationEpoch::from_shared(
        prefix
            .observations()
            .iter()
            .chain(observed.observations().observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    ) {
        Ok(epoch) => epoch,
        Err(error) => return LoadingPreparationOutcome::Complete(Err(error.into())),
    };
    let result = match observed.result() {
        Ok(PathFileDigest::Present(digest)) => Ok(SourceArtifactInput {
            label: label.clone(),
            namespace: resolved.namespace(),
            requested_path: resolved.requested_path().dupe(),
            real_path: resolved.real_path().dupe(),
            digest: *digest,
        }),
        Ok(PathFileDigest::Missing) => Err(SourceArtifactInputError::Missing),
        Err(error) => Err(SourceArtifactInputError::Digest(error.dupe())),
    };
    complete(result, observations)
}

#[cfg(test)]
mod tests;
