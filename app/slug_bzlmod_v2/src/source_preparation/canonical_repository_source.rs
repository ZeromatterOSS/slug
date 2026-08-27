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
type CanonicalSourceFileDriverOutcome = SourcePreparationOutcome<
    Result<(CanonicalSourceFileResult, PathObservationEpoch), ObservedPathFrontierError>,
>;

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

fn canonical_source_file_complete(
    key: &HostCanonicalRepositorySourceFileKey,
    result: Result<HostRepositorySourceObservation, HostCanonicalRepositorySourceFileErrorKind>,
    observations: PathObservationEpoch,
) -> CanonicalSourceFileDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(
            result.map_err(|kind| HostCanonicalRepositorySourceFileError {
                input: key.input.clone(),
                relative_path: key.relative_path.clone(),
                kind,
            }),
        ),
        observations,
    )))
}

async fn resolve_canonical_source_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositorySourceFileKey,
    namespace: PathObservationNamespace,
    requested_path: NormalizedAbsolutePath,
    repo_relative_path: Arc<PathBuf>,
    mode: HostRepositoryObservationMode,
) -> ControlFlow<CanonicalSourceFileDriverOutcome, (ResolvedPath, PathObservationEpoch)> {
    match mode {
        HostRepositoryObservationMode::Legacy => match ctx
            .compute(&ResolvedPathKey::new(namespace, requested_path))
            .await
        {
            Ok(PathOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::path_need(need))
            }
            Ok(PathOutcome::Complete(Ok(value))) => {
                ControlFlow::Continue((value, PathObservationEpoch::empty()))
            }
            Ok(PathOutcome::Complete(Err(error))) => {
                ControlFlow::Break(canonical_source_file_complete(
                    key,
                    Err(HostCanonicalRepositorySourceFileErrorKind::Request(
                        project_resolution_error(repo_relative_path, error),
                    )),
                    PathObservationEpoch::empty(),
                ))
            }
            Err(error) => ControlFlow::Break(canonical_source_file_complete(
                key,
                Err(HostCanonicalRepositorySourceFileErrorKind::RequestCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )),
        },
        HostRepositoryObservationMode::Observed => match ctx
            .compute(&ResolvedPathObservationKey::new(namespace, requested_path))
            .await
        {
            Ok(PathOutcome::Need(need)) => {
                ControlFlow::Break(SourcePreparationOutcome::path_need(need))
            }
            Ok(PathOutcome::Complete(Err(error))) => {
                ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
            }
            Ok(PathOutcome::Complete(Ok(value))) => match value.result() {
                Ok(resolved) => {
                    ControlFlow::Continue((resolved.clone(), value.observations().dupe()))
                }
                Err(error) => ControlFlow::Break(canonical_source_file_complete(
                    key,
                    Err(HostCanonicalRepositorySourceFileErrorKind::Request(
                        project_resolution_error(repo_relative_path, error.clone()),
                    )),
                    value.observations().dupe(),
                )),
            },
            Err(error) => ControlFlow::Break(canonical_source_file_complete(
                key,
                Err(HostCanonicalRepositorySourceFileErrorKind::RequestCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )),
        },
    }
}

async fn drive_request_source_file(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositorySourceFileKey,
    request: Arc<RepositoryMaterializationRequest>,
    mode: HostRepositoryObservationMode,
) -> CanonicalSourceFileDriverOutcome {
    let repo_relative_path = key.relative_path.path_arc().clone();
    let materialization = match ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
        Err(error) => {
            return canonical_source_file_complete(
                key,
                Err(HostCanonicalRepositorySourceFileErrorKind::RequestCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return canonical_source_file_complete(
                key,
                Err(HostCanonicalRepositorySourceFileErrorKind::Request(
                    RepositorySourceFileError::Materialization {
                        repo_relative_path,
                        error: Arc::new(error.clone()),
                    },
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let (namespace, root) = match materialization {
        RepositoryMaterialization::Local { source_root, .. } => {
            (PathObservationNamespace::Host, source_root)
        }
        RepositoryMaterialization::Immutable {
            generation_root,
            observation_instance,
            ..
        } => (
            PathObservationNamespace::Materialization(*observation_instance),
            generation_root,
        ),
    };
    let requested_path = match NormalizedAbsolutePath::new(root.join(key.relative_path.as_path())) {
        Ok(value) => value,
        Err(_) => {
            return canonical_source_file_complete(
                key,
                Err(HostCanonicalRepositorySourceFileErrorKind::Request(
                    RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path },
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let (resolved, observations) = match resolve_canonical_source_path(
        ctx,
        key,
        namespace,
        requested_path,
        repo_relative_path.clone(),
        mode,
    )
    .await
    {
        ControlFlow::Continue(value) => value,
        ControlFlow::Break(outcome) => return outcome,
    };
    match drive_host_repository_source_from_resolved(
        ctx,
        resolved,
        repo_relative_path,
        mode,
        observations,
    )
    .await
    {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            SourcePreparationOutcome::Complete(Err(error))
        }
        SourcePreparationOutcome::Complete(Ok((result, observations))) => {
            canonical_source_file_complete(
                key,
                result
                    .map(HostRepositorySourceObservation::Request)
                    .map_err(HostCanonicalRepositorySourceFileErrorKind::Request),
                observations,
            )
        }
    }
}

async fn drive_canonical_source_file(
    ctx: &mut DiceComputations<'_>,
    key: &HostCanonicalRepositorySourceFileKey,
    mode: HostRepositoryObservationMode,
) -> CanonicalSourceFileDriverOutcome {
    match key.input.view().disposition() {
        HostRepositorySourceInputDispositionView::Builtin(identity) => {
            let Some(path) = key.relative_path.as_path().to_str() else {
                return canonical_source_file_complete(
                    key,
                    Err(HostCanonicalRepositorySourceFileErrorKind::InvalidPath),
                    PathObservationEpoch::empty(),
                );
            };
            match ctx
                .compute(&BuiltinBazelToolsSourceFileKey::new(
                    identity.snapshot(),
                    path,
                ))
                .await
            {
                Ok(value) => canonical_source_file_complete(
                    key,
                    value
                        .as_ref()
                        .clone()
                        .map(HostRepositorySourceObservation::Builtin)
                        .map_err(HostCanonicalRepositorySourceFileErrorKind::Builtin),
                    PathObservationEpoch::empty(),
                ),
                Err(error) => canonical_source_file_complete(
                    key,
                    Err(HostCanonicalRepositorySourceFileErrorKind::BuiltinCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                ),
            }
        }
        HostRepositorySourceInputDispositionView::Request(request) => {
            drive_request_source_file(ctx, key, request.clone(), mode).await
        }
    }
}

#[async_trait]
impl Key for HostCanonicalRepositorySourceFileKey {
    type Value = SourcePreparationOutcome<CanonicalSourceFileResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_canonical_source_file(ctx, self, HostRepositoryObservationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy canonical source file has no observed outer")
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
impl Key for HostCanonicalRepositorySourceFileObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostCanonicalRepositorySourceFile, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_canonical_source_file(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
            .map(|value| {
                value.map(
                    |(result, observations)| ObservedHostCanonicalRepositorySourceFile {
                        result,
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
