//! Prefer an already captured command source, without initiating a capture.
//!
//! The key is per repository so the natural request/mapping projection is shared
//! by source files. A clean `None` is tracked just as a selected snapshot is.
use async_trait::async_trait;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::HostCanonicalRepositoryRouteKind;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_bzlmod_v2::RepositorySourceSnapshotEpochKey;
use slug_bzlmod_v2::RepositorySourceSnapshotRequest;
use slug_bzlmod_v2::RepositorySourceSnapshotState;
use slug_bzlmod_v2::RepositoryWorkspaceEpochKey;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy;
use slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicyKey;
use slug_bzlmod_v2::RepositoryWorkspaceObservationKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::AnchoredResolvedPathObservationKey;
use slug_workspace_v2::MaterializationPathAnchor;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct CapturedSourceSnapshot {
    pub(super) state: Option<Arc<RepositorySourceSnapshotState>>,
    pub(super) observations: PathObservationEpoch,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct CapturedSourceSnapshotKey {
    workspace: NormalizedAbsolutePath,
    repository: CanonicalRepoName,
}
impl CapturedSourceSnapshotKey {
    pub(super) fn new(workspace: NormalizedAbsolutePath, repository: CanonicalRepoName) -> Self {
        Self {
            workspace,
            repository,
        }
    }
    async fn select(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> LoadingPreparationOutcome<Result<CapturedSourceSnapshot, AnalysisError>> {
        // Never infer a missing fence to mean the default/uncaptured branch.
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => {
                        return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(
                            format!("selecting captured source: {error:?}"),
                        )))
                    }
                }
            };
        }
        let empty = || CapturedSourceSnapshot {
            state: None,
            observations: PathObservationEpoch::empty(),
        };
        let policy = checked!(
            ctx.compute(&RepositoryWorkspaceExecutionPolicyKey::new(
                self.workspace.dupe()
            ))
            .await
        );
        if policy == RepositoryWorkspaceExecutionPolicy::Disabled {
            return LoadingPreparationOutcome::Complete(Ok(empty()));
        }
        checked!(
            ctx.compute(&RepositoryWorkspaceEpochKey::new(self.workspace.dupe()))
                .await
        );
        let workspace_epoch = checked!(
            ctx.compute(&RepositoryWorkspaceObservationKey::new(
                self.workspace.dupe()
            ))
            .await
        );
        let source_epoch = checked!(
            ctx.compute(&RepositorySourceSnapshotEpochKey::new(
                self.workspace.dupe()
            ))
            .await
        );
        if workspace_epoch.workspace() != Some(&self.workspace)
            || workspace_epoch.lease().is_none()
            || source_epoch.workspace() != Some(&self.workspace)
            || source_epoch.lease() != workspace_epoch.lease()
        {
            return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(
                "captured source command epoch is revoked or foreign",
            )));
        }
        let lease = workspace_epoch.lease().expect("checked command lease");
        let (request, observations) = if self.repository.is_root() {
            let observed = match checked!(
                ctx.compute(&HostRootRepositoryMappingObservationKey::new(
                    self.workspace.dupe()
                ))
                .await
            ) {
                SourcePreparationOutcome::Need(needs) => {
                    return LoadingPreparationOutcome::Need(needs);
                }
                SourcePreparationOutcome::Complete(result) => checked!(result),
            };
            let mapping = checked!(observed.result().as_ref());
            (
                checked!(RepositorySourceSnapshotRequest::from_workspace(
                    self.workspace.dupe(),
                    mapping,
                    lease
                )),
                observed.observations().dupe(),
            )
        } else {
            let observed = match checked!(
                ctx.compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                    self.workspace.dupe(),
                    self.repository.clone()
                ))
                .await
            ) {
                LoadingPreparationOutcome::Need(needs) => {
                    return LoadingPreparationOutcome::Need(needs);
                }
                LoadingPreparationOutcome::Complete(result) => checked!(result),
            };
            let route = checked!(observed.result().as_ref());
            // Only these natural producers admit Source images. Uncaptured
            // legacy/generated routes retain their existing producer, but an
            // occupied slot must never be hidden by a changed route kind.
            let input = route.input().view();
            let source_capable = match (input.route().view().kind(), input.disposition()) {
                (
                    HostCanonicalRepositoryRouteKind::Builtin,
                    HostRepositorySourceInputDispositionView::Builtin(_),
                ) => true,
                (
                    HostCanonicalRepositoryRouteKind::SelectedRegistry
                    | HostCanonicalRepositoryRouteKind::SelectedNonregistry,
                    HostRepositorySourceInputDispositionView::Request(request),
                ) => matches!(
                    request.kind,
                    RepositoryMaterializationKind::Local { .. }
                        | RepositoryMaterializationKind::Immutable
                ),
                _ => false,
            };
            if !source_capable {
                return LoadingPreparationOutcome::Complete(uncaptured_ordinary(
                    &source_epoch,
                    &self.repository,
                    observed.observations().dupe(),
                ));
            }
            (
                checked!(RepositorySourceSnapshotRequest::from_canonical(
                    route.input(),
                    lease
                )),
                observed.observations().dupe(),
            )
        };
        let state = checked!(source_epoch.find(&request)).cloned();
        LoadingPreparationOutcome::Complete(Ok(CapturedSourceSnapshot {
            state,
            observations,
        }))
    }
}
// Called only after both current command fences have been checked. A source
// selected for this repository cannot become an ordinary/General route during
// the same command without a full-request conflict.
fn uncaptured_ordinary(
    epoch: &RepositorySourceSnapshotEpoch,
    repository: &CanonicalRepoName,
    observations: PathObservationEpoch,
) -> Result<CapturedSourceSnapshot, AnalysisError> {
    if epoch.states().is_none_or(|states| {
        states
            .iter()
            .any(|state| state.reservation().repository() == repository)
    }) {
        return Err(AnalysisError::message(
            "captured source slot conflicts with ordinary repository route",
        ));
    }
    Ok(CapturedSourceSnapshot {
        state: None,
        observations,
    })
}

impl std::fmt::Display for CapturedSourceSnapshotKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured-source:{}:{}", self.workspace, self.repository)
    }
}
#[async_trait]
impl Key for CapturedSourceSnapshotKey {
    type Value = Arc<LoadingPreparationOutcome<Result<CapturedSourceSnapshot, AnalysisError>>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        Arc::new(self.select(ctx).await)
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a.complete_eq(b)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value.as_ref(), LoadingPreparationOutcome::Complete(Ok(_)))
    }
}

pub(super) async fn resolve(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    label: &CanonicalLabel,
    state: &RepositorySourceSnapshotState,
    observations: PathObservationEpoch,
) -> SourcePathOutcome {
    let fail = |error: String| complete(Err(AnalysisError::message(error)), observations.dupe());
    let reservation = state.reservation();
    let anchor = match MaterializationPathAnchor::new(
        PathObservationNamespace::Materialization(reservation.instance()),
        reservation.display_root().dupe(),
    ) {
        Ok(anchor) => anchor,
        Err(error) => return fail(format!("captured source anchor: {error:?}")),
    };
    let relative =
        std::path::Path::new(label.package().package().as_str()).join(label.target().as_str());
    let absolute = reservation.display_root().as_path().join(relative);
    let key = match AnchoredResolvedPathObservationKey::new(anchor, absolute) {
        Ok(key) => key,
        Err(error) => return fail(format!("captured source path: {error:?}")),
    };
    match ctx.compute(&key).await {
        Err(error) => fail(format!("resolving captured source: {error}")),
        Ok(PathOutcome::Need(needs)) => {
            LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(needs))
        }
        Ok(PathOutcome::Complete(Err(error))) => LoadingPreparationOutcome::Complete(Err(error)),
        Ok(PathOutcome::Complete(Ok(value))) => {
            let observations = if matches!(mode, ConfiguredAnalysisMode::Observed) {
                match PathObservationEpoch::from_shared(
                    observations
                        .observations()
                        .iter()
                        .chain(value.observations().observations().iter())
                        .map(|(d, r)| (d.dupe(), r.dupe())),
                ) {
                    Ok(epoch) => epoch,
                    Err(error) => return LoadingPreparationOutcome::Complete(Err(error.into())),
                }
            } else {
                PathObservationEpoch::empty()
            };
            complete(
                value
                    .result()
                    .clone()
                    .map(SourceResolution::Filesystem)
                    .map_err(|error| {
                        AnalysisError::message(format!(
                            "resolving captured source {label}: {error:?}"
                        ))
                    }),
                observations,
            )
        }
    }
}

#[cfg(test)]
mod tests;
