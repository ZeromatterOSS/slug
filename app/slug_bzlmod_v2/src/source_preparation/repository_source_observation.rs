/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select the license that applies to you.
 */

use super::*;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositorySourceObservationInput {
    Root(HostRepositorySourceInput),
    Canonical(HostCanonicalRepositorySourceInput),
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostRepositorySourceObservationInputView<'a> {
    disposition: HostRepositorySourceInputDispositionView<'a>,
}

impl HostRepositorySourceObservationInput {
    #[doc(hidden)]
    pub fn view(&self) -> HostRepositorySourceObservationInputView<'_> {
        HostRepositorySourceObservationInputView {
            disposition: self.disposition(),
        }
    }

    fn disposition(&self) -> HostRepositorySourceInputDispositionView<'_> {
        match self {
            Self::Root(input) => input.view().disposition(),
            Self::Canonical(input) => input.view().disposition(),
        }
    }

    fn canonical_repo(&self) -> &CanonicalRepoName {
        match self {
            Self::Root(input) => input.view().capability().canonical_repo(),
            Self::Canonical(input) => input.view().route().view().canonical_repo(),
        }
    }
}

impl<'a> HostRepositorySourceObservationInputView<'a> {
    pub fn disposition(self) -> HostRepositorySourceInputDispositionView<'a> {
        self.disposition
    }
}

impl Hash for HostRepositorySourceObservationInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Root(input) => {
                input.capability.hash(state);
                std::mem::discriminant(&input.disposition).hash(state);
                if let HostRepositoryMaterializationDisposition::Request(request) =
                    &input.disposition
                {
                    request.id.hash(state);
                }
            }
            Self::Canonical(input) => input.hash(state),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositorySourceObservation {
    Builtin(BuiltinBazelToolsSourceFileValue),
    Request(HostRepositorySourceFileValue),
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum HostRepositorySourceObservationView<'a> {
    Builtin(&'a BuiltinBazelToolsSourceFileValue),
    Request(&'a HostRepositorySourceFileValue),
}

impl HostRepositorySourceObservation {
    #[doc(hidden)]
    pub fn view(&self) -> HostRepositorySourceObservationView<'_> {
        match self {
            Self::Builtin(value) => HostRepositorySourceObservationView::Builtin(value),
            Self::Request(value) => HostRepositorySourceObservationView::Request(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostRepositorySourceObservationErrorKind {
    BuiltinPath,
    Builtin(BuiltinBazelToolsSourceFileError),
    BuiltinCompute(Arc<str>),
    Request(RepositorySourceFileError),
    RequestCompute(Arc<str>),
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceObservationError {
    pub(super) input: HostRepositorySourceObservationInput,
    pub(super) relative_path: HostRepositoryRelativePath,
    pub(super) kind: HostRepositorySourceObservationErrorKind,
}

impl HostRepositorySourceObservationError {
    #[doc(hidden)]
    pub fn input(&self) -> &HostRepositorySourceInput {
        match &self.input {
            HostRepositorySourceObservationInput::Root(input) => input,
            HostRepositorySourceObservationInput::Canonical(_) => {
                panic!("canonical source errors use source_observation_input()")
            }
        }
    }

    #[doc(hidden)]
    pub fn source_observation_input(&self) -> &HostRepositorySourceObservationInput {
        &self.input
    }

    #[doc(hidden)]
    pub fn relative_path(&self) -> &HostRepositoryRelativePath {
        &self.relative_path
    }
}

impl fmt::Display for HostRepositorySourceObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostRepositorySourceObservationError {}

#[doc(hidden)]
pub type HostRepositorySourceObservationResult =
    Result<HostRepositorySourceObservation, HostRepositorySourceObservationError>;
#[doc(hidden)]
pub type HostRepositorySourceObservationOutcome =
    SourcePreparationOutcome<Arc<HostRepositorySourceObservationResult>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositorySourceObservationKey {
    pub(super) input: HostRepositorySourceObservationInput,
    pub(super) relative_path: HostRepositoryRelativePath,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRepositorySourceObservationEpochKey(HostRepositorySourceObservationKey);

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRepositorySourceObservation {
    result: Arc<HostRepositorySourceObservationResult>,
    observations: PathObservationEpoch,
}

impl HostRepositorySourceObservationKey {
    #[doc(hidden)]
    pub fn new(
        input: HostRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self {
            input: HostRepositorySourceObservationInput::Root(input),
            relative_path,
        }
    }

    #[doc(hidden)]
    pub fn new_canonical(
        input: HostCanonicalRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self {
            input: HostRepositorySourceObservationInput::Canonical(input),
            relative_path,
        }
    }
}

impl Hash for HostRepositorySourceObservationKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.input.hash(state);
        self.relative_path.path_arc().hash(state);
    }
}

impl HostRepositorySourceObservationEpochKey {
    #[doc(hidden)]
    pub fn new(
        input: HostRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self(HostRepositorySourceObservationKey::new(
            input,
            relative_path,
        ))
    }

    #[doc(hidden)]
    pub fn new_canonical(
        input: HostCanonicalRepositorySourceInput,
        relative_path: HostRepositoryRelativePath,
    ) -> Self {
        Self(HostRepositorySourceObservationKey::new_canonical(
            input,
            relative_path,
        ))
    }
}

impl ObservedHostRepositorySourceObservation {
    #[doc(hidden)]
    pub fn result(&self) -> &Arc<HostRepositorySourceObservationResult> {
        &self.result
    }

    #[doc(hidden)]
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

impl fmt::Display for HostRepositorySourceObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-repository-source-observation:{}:{}",
            self.input.canonical_repo(),
            self.relative_path.as_path().display()
        )
    }
}

impl fmt::Display for HostRepositorySourceObservationEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type SourceObservationDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<HostRepositorySourceObservationResult>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

fn source_observation_complete(
    key: &HostRepositorySourceObservationKey,
    result: Result<HostRepositorySourceObservation, HostRepositorySourceObservationErrorKind>,
    observations: PathObservationEpoch,
) -> SourceObservationDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(result.map_err(|kind| HostRepositorySourceObservationError {
            input: key.input.clone(),
            relative_path: key.relative_path.clone(),
            kind,
        })),
        observations,
    )))
}

fn source_observation_compute_error_kind(
    key: &HostRepositorySourceObservationKey,
    message: Arc<str>,
) -> HostRepositorySourceObservationErrorKind {
    match key.input.disposition() {
        HostRepositorySourceInputDispositionView::Builtin(_) => {
            HostRepositorySourceObservationErrorKind::BuiltinCompute(message)
        }
        HostRepositorySourceInputDispositionView::Request(_) => {
            HostRepositorySourceObservationErrorKind::RequestCompute(message)
        }
    }
}

pub(super) fn source_observation_compute_error(
    key: &HostRepositorySourceObservationKey,
    message: Arc<str>,
) -> HostRepositorySourceObservationOutcome {
    let SourcePreparationOutcome::Complete(Ok((result, _))) = source_observation_complete(
        key,
        Err(source_observation_compute_error_kind(key, message)),
        PathObservationEpoch::empty(),
    ) else {
        unreachable!("a source-observation compute error is complete")
    };
    SourcePreparationOutcome::Complete(result)
}

pub(super) fn source_observation_epoch_compute_error(
    key: &HostRepositorySourceObservationKey,
    message: Arc<str>,
) -> SourcePreparationOutcome<
    Result<ObservedHostRepositorySourceObservation, ObservedPathFrontierError>,
> {
    let SourcePreparationOutcome::Complete(Ok((result, observations))) =
        source_observation_complete(
            key,
            Err(source_observation_compute_error_kind(key, message)),
            PathObservationEpoch::empty(),
        )
    else {
        unreachable!("an observed source-observation compute error is complete")
    };
    SourcePreparationOutcome::Complete(Ok(ObservedHostRepositorySourceObservation {
        result,
        observations,
    }))
}

async fn resolve_source_path(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositorySourceObservationKey,
    namespace: PathObservationNamespace,
    requested_path: NormalizedAbsolutePath,
    repo_relative_path: Arc<PathBuf>,
    mode: HostRepositoryObservationMode,
) -> ControlFlow<SourceObservationDriverOutcome, (ResolvedPath, PathObservationEpoch)> {
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
                ControlFlow::Break(source_observation_complete(
                    key,
                    Err(HostRepositorySourceObservationErrorKind::Request(
                        project_resolution_error(repo_relative_path, error),
                    )),
                    PathObservationEpoch::empty(),
                ))
            }
            Err(error) => ControlFlow::Break(source_observation_complete(
                key,
                Err(HostRepositorySourceObservationErrorKind::RequestCompute(
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
                Err(error) => ControlFlow::Break(source_observation_complete(
                    key,
                    Err(HostRepositorySourceObservationErrorKind::Request(
                        project_resolution_error(repo_relative_path, error.clone()),
                    )),
                    value.observations().dupe(),
                )),
            },
            Err(error) => ControlFlow::Break(source_observation_complete(
                key,
                Err(HostRepositorySourceObservationErrorKind::RequestCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            )),
        },
    }
}

async fn drive_request_source(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositorySourceObservationKey,
    request: Arc<RepositoryMaterializationRequest>,
    mode: HostRepositoryObservationMode,
) -> SourceObservationDriverOutcome {
    let repo_relative_path = key.relative_path.path_arc().clone();
    let materialization = match ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
        Err(error) => {
            return source_observation_complete(
                key,
                Err(HostRepositorySourceObservationErrorKind::RequestCompute(
                    error.to_string().into(),
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return source_observation_complete(
                key,
                Err(HostRepositorySourceObservationErrorKind::Request(
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
            return source_observation_complete(
                key,
                Err(HostRepositorySourceObservationErrorKind::Request(
                    RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path },
                )),
                PathObservationEpoch::empty(),
            );
        }
    };
    let (resolved, observations) = match resolve_source_path(
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
            source_observation_complete(
                key,
                result
                    .map(HostRepositorySourceObservation::Request)
                    .map_err(HostRepositorySourceObservationErrorKind::Request),
                observations,
            )
        }
    }
}

async fn drive_source_observation(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositorySourceObservationKey,
    mode: HostRepositoryObservationMode,
) -> SourceObservationDriverOutcome {
    match key.input.disposition() {
        HostRepositorySourceInputDispositionView::Builtin(identity) => {
            let Some(path) = key.relative_path.as_path().to_str() else {
                return source_observation_complete(
                    key,
                    Err(HostRepositorySourceObservationErrorKind::BuiltinPath),
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
                Ok(value) => source_observation_complete(
                    key,
                    value
                        .as_ref()
                        .clone()
                        .map(HostRepositorySourceObservation::Builtin)
                        .map_err(HostRepositorySourceObservationErrorKind::Builtin),
                    PathObservationEpoch::empty(),
                ),
                Err(error) => source_observation_complete(
                    key,
                    Err(HostRepositorySourceObservationErrorKind::BuiltinCompute(
                        error.to_string().into(),
                    )),
                    PathObservationEpoch::empty(),
                ),
            }
        }
        HostRepositorySourceInputDispositionView::Request(request) => {
            drive_request_source(ctx, key, request.clone(), mode).await
        }
    }
}

#[async_trait]
impl Key for HostRepositorySourceObservationKey {
    type Value = HostRepositorySourceObservationOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_source_observation(ctx, self, HostRepositoryObservationMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy source observation has no observed outer")
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
impl Key for HostRepositorySourceObservationEpochKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostRepositorySourceObservation, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_source_observation(ctx, &self.0, HostRepositoryObservationMode::Observed)
            .await
            .map(|value| {
                value.map(
                    |(result, observations)| ObservedHostRepositorySourceObservation {
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
