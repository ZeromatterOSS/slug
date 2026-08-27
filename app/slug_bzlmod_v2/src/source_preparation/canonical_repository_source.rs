/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use super::*;
use crate::HostCanonicalRepositoryRoute;
use crate::HostCanonicalRepositoryRouteKind;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositorySourceInput {
    route: Arc<HostCanonicalRepositoryRoute>,
    disposition: HostRepositoryMaterializationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostCanonicalRepositorySourceInputError {
    Root,
    MissingRepositorySpecification,
    MissingGeneratedFileEffectPlan,
    ExtraneousGeneratedFileEffectPlan,
    Projection(RepositoryMaterializationError),
}

#[derive(Debug, Clone, Copy)]
pub struct HostCanonicalRepositorySourceInputView<'a> {
    route: &'a Arc<HostCanonicalRepositoryRoute>,
    disposition: HostRepositorySourceInputDispositionView<'a>,
}

impl HostCanonicalRepositorySourceInput {
    pub fn view(&self) -> HostCanonicalRepositorySourceInputView<'_> {
        HostCanonicalRepositorySourceInputView {
            route: &self.route,
            disposition: match &self.disposition {
                HostRepositoryMaterializationDisposition::Builtin(identity) => {
                    HostRepositorySourceInputDispositionView::Builtin(identity)
                }
                HostRepositoryMaterializationDisposition::Request(request) => {
                    HostRepositorySourceInputDispositionView::Request(request)
                }
            },
        }
    }
}

impl Hash for HostCanonicalRepositorySourceInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        std::mem::discriminant(&self.disposition).hash(state);
        match &self.disposition {
            HostRepositoryMaterializationDisposition::Builtin(identity) => identity.hash(state),
            HostRepositoryMaterializationDisposition::Request(request) => {
                request.id.hash(state);
                std::mem::discriminant(&request.kind).hash(state);
                if let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind {
                    plan.hash(state);
                }
            }
        }
    }
}

impl<'a> HostCanonicalRepositorySourceInputView<'a> {
    pub fn route(self) -> &'a Arc<HostCanonicalRepositoryRoute> {
        self.route
    }

    pub fn disposition(self) -> HostRepositorySourceInputDispositionView<'a> {
        self.disposition
    }
}

pub fn host_canonical_repository_source_input(
    route: Arc<HostCanonicalRepositoryRoute>,
    generated_plan: Option<GeneratedRepositoryFileEffectPlan>,
) -> Result<HostCanonicalRepositorySourceInput, HostCanonicalRepositorySourceInputError> {
    let view = route.view();
    let disposition = match view.kind() {
        HostCanonicalRepositoryRouteKind::Root => {
            return Err(HostCanonicalRepositorySourceInputError::Root);
        }
        HostCanonicalRepositoryRouteKind::Builtin => {
            if generated_plan.is_some() {
                return Err(
                    HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFileEffectPlan,
                );
            }
            HostRepositoryMaterializationDisposition::Builtin(
                view.builtin_identity()
                    .expect("a built-in canonical route retains its snapshot identity"),
            )
        }
        HostCanonicalRepositoryRouteKind::Generated => {
            let plan = generated_plan
                .ok_or(HostCanonicalRepositorySourceInputError::MissingGeneratedFileEffectPlan)?;
            let repo_spec = view
                .repo_spec()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            HostRepositoryMaterializationDisposition::Request(Arc::new(
                RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: view.workspace().dupe(),
                        canonical_repo: view.canonical_repo().clone(),
                    },
                    repo_spec: repo_spec.clone(),
                    kind: RepositoryMaterializationKind::GeneratedFileEffects(plan),
                },
            ))
        }
        HostCanonicalRepositoryRouteKind::SelectedRegistry
        | HostCanonicalRepositoryRouteKind::SelectedNonregistry => {
            if generated_plan.is_some() {
                return Err(
                    HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFileEffectPlan,
                );
            }
            let repo_spec = view
                .repo_spec()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            let local_path_policy = view
                .local_path_policy()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            let kind = request_kind(view.workspace(), repo_spec, local_path_policy)
                .map_err(HostCanonicalRepositorySourceInputError::Projection)?;
            HostRepositoryMaterializationDisposition::Request(Arc::new(
                RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: view.workspace().dupe(),
                        canonical_repo: view.canonical_repo().clone(),
                    },
                    repo_spec: repo_spec.clone(),
                    kind,
                },
            ))
        }
    };
    Ok(HostCanonicalRepositorySourceInput { route, disposition })
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositorySourceFileErrorKind {
    InvalidPath,
    Builtin(BuiltinBazelToolsSourceFileError),
    BuiltinCompute(Arc<str>),
    Request(RepositorySourceFileError),
    RequestCompute(Arc<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositorySourceFileError {
    input: HostCanonicalRepositorySourceInput,
    relative_path: HostRepositoryRelativePath,
    kind: HostCanonicalRepositorySourceFileErrorKind,
}

impl fmt::Display for HostCanonicalRepositorySourceFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostCanonicalRepositorySourceFileError {}

type CanonicalSourceFileResult =
    Arc<Result<HostRepositorySourceObservation, HostCanonicalRepositorySourceFileError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositorySourceFileKey {
    input: HostCanonicalRepositorySourceInput,
    relative_path: HostRepositoryRelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositorySourceFileObservationKey(HostCanonicalRepositorySourceFileKey);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostCanonicalRepositorySourceFile {
    result: CanonicalSourceFileResult,
    observations: PathObservationEpoch,
}

impl HostCanonicalRepositorySourceFileKey {
    pub fn new(
        input: HostCanonicalRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self {
            input,
            relative_path,
        }
    }

    fn shared_key(&self) -> HostRepositorySourceObservationKey {
        HostRepositorySourceObservationKey::new_canonical(
            self.input.clone(),
            self.relative_path.clone(),
        )
    }
}

fn project_shared_source_result(
    key: &HostCanonicalRepositorySourceFileKey,
    result: &Arc<HostRepositorySourceObservationResult>,
) -> CanonicalSourceFileResult {
    Arc::new(match result.as_ref() {
        Ok(value) => Ok(value.clone()),
        Err(error) => Err(HostCanonicalRepositorySourceFileError {
            input: key.input.clone(),
            relative_path: key.relative_path.clone(),
            kind: match &error.kind {
                HostRepositorySourceObservationErrorKind::BuiltinPath => {
                    HostCanonicalRepositorySourceFileErrorKind::InvalidPath
                }
                HostRepositorySourceObservationErrorKind::Builtin(error) => {
                    HostCanonicalRepositorySourceFileErrorKind::Builtin(error.clone())
                }
                HostRepositorySourceObservationErrorKind::BuiltinCompute(error) => {
                    HostCanonicalRepositorySourceFileErrorKind::BuiltinCompute(error.clone())
                }
                HostRepositorySourceObservationErrorKind::Request(error) => {
                    HostCanonicalRepositorySourceFileErrorKind::Request(error.clone())
                }
                HostRepositorySourceObservationErrorKind::RequestCompute(error) => {
                    HostCanonicalRepositorySourceFileErrorKind::RequestCompute(error.clone())
                }
            },
        }),
    })
}

impl Hash for HostCanonicalRepositorySourceFileKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.input.hash(state);
        self.relative_path.path_arc().hash(state);
    }
}

impl HostCanonicalRepositorySourceFileObservationKey {
    pub fn new(
        input: HostCanonicalRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self(HostCanonicalRepositorySourceFileKey::new(
            input,
            relative_path,
        ))
    }
}

impl ObservedHostCanonicalRepositorySourceFile {
    pub fn result(&self) -> &CanonicalSourceFileResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

impl fmt::Display for HostCanonicalRepositorySourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-source-file:{}:{}",
            self.input.route.view().canonical_repo(),
            self.relative_path.as_path().display()
        )
    }
}

impl fmt::Display for HostCanonicalRepositorySourceFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[async_trait]
impl Key for HostCanonicalRepositorySourceFileKey {
    type Value = SourcePreparationOutcome<CanonicalSourceFileResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let key = self.shared_key();
        match ctx.compute(&key).await {
            Ok(value) => value.map(|result| project_shared_source_result(self, &result)),
            Err(error) => source_observation_compute_error(&key, error.to_string().into())
                .map(|result| project_shared_source_result(self, &result)),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostCanonicalRepositorySourceFileObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostCanonicalRepositorySourceFile, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let key = self.0.shared_key();
        let observed = HostRepositorySourceObservationEpochKey::new_canonical(
            self.0.input.clone(),
            self.0.relative_path.clone(),
        );
        match ctx.compute(&observed).await {
            Ok(value) => value.map(|value| {
                value.map(|observed| ObservedHostCanonicalRepositorySourceFile {
                    result: project_shared_source_result(&self.0, observed.result()),
                    observations: observed.observations().dupe(),
                })
            }),
            Err(error) => source_observation_epoch_compute_error(&key, error.to_string().into())
                .map(|value| {
                    value.map(|observed| ObservedHostCanonicalRepositorySourceFile {
                        result: project_shared_source_result(&self.0, observed.result()),
                        observations: observed.observations().dupe(),
                    })
                }),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositoryDirectoryListingKey {
    input: HostCanonicalRepositorySourceInput,
    directory: PackagePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostCanonicalRepositoryDirectoryListingObservationKey(
    HostCanonicalRepositoryDirectoryListingKey,
);

impl HostCanonicalRepositoryDirectoryListingKey {
    pub fn new(input: HostCanonicalRepositorySourceInput, directory: PackagePath) -> Self {
        Self { input, directory }
    }
}

impl Hash for HostCanonicalRepositoryDirectoryListingKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.input.hash(state);
        self.directory.hash(state);
    }
}

impl HostCanonicalRepositoryDirectoryListingObservationKey {
    pub fn new(input: HostCanonicalRepositorySourceInput, directory: PackagePath) -> Self {
        Self(HostCanonicalRepositoryDirectoryListingKey::new(
            input, directory,
        ))
    }
}

impl fmt::Display for HostCanonicalRepositoryDirectoryListingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-directory-listing:{}://{}",
            self.input.route.view().canonical_repo(),
            self.directory
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryDirectoryListingObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[async_trait]
impl Key for HostCanonicalRepositoryDirectoryListingKey {
    type Value = SourcePreparationResult<
        HostRepositoryDirectoryListing,
        HostRepositoryDirectoryListingError,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_repository_directory_listing_from_disposition(
            ctx,
            self.input.disposition.clone(),
            &self.directory,
            HostRepositoryObservationMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy canonical directory listing has no observed outer")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostCanonicalRepositoryDirectoryListingObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRepositoryDirectoryListing, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_repository_directory_listing_from_disposition(
            ctx,
            self.0.input.disposition.clone(),
            &self.0.directory,
            HostRepositoryObservationMode::Observed,
        )
        .await
        .map(|outcome| {
            outcome.map(
                |(result, observations)| ObservedHostRepositoryDirectoryListing {
                    result: Arc::new(result),
                    observations,
                },
            )
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
