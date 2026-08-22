/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::HostRepositorySourceInputError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_source_input;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

use super::root_apparent_repository_route::HostRootApparentRepositoryRouteKey;
use super::root_apparent_repository_route::HostRootApparentRepositoryRouteObservationError;
use super::root_apparent_repository_route::HostRootApparentRepositoryRouteObservationKey;
use super::root_apparent_repository_route::HostRootApparentRepositoryRouteResult;
use super::root_apparent_repository_route::HostRootApparentRepositorySourceDisposition;
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourceInputDisposition {
    Main,
    Input(HostRepositorySourceInput),
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourceInput {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    predecessor: Arc<HostRootApparentRepositoryRouteResult>,
    disposition: HostRootApparentRepositorySourceInputDisposition,
}
#[derive(Debug, Clone, Copy)]
pub(super) enum HostRootApparentRepositorySourceInputDispositionView<'a> {
    Main,
    Input(&'a HostRepositorySourceInput),
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HostRootApparentRepositorySourceInputView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    disposition: HostRootApparentRepositorySourceInputDispositionView<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletedRouteDisposition {
    Route,
    InvalidRoute,
    Source,
}
fn completed_route_disposition(
    predecessor_is_success: bool,
    has_source: bool,
) -> CompletedRouteDisposition {
    match (predecessor_is_success, has_source) {
        (false, _) => CompletedRouteDisposition::Route,
        (true, false) => CompletedRouteDisposition::InvalidRoute,
        (true, true) => CompletedRouteDisposition::Source,
    }
}
fn association_is_valid(
    workspace: &NormalizedAbsolutePath,
    route: &super::root_apparent_repository_route::HostRootApparentRepositoryRoute,
    disposition: HostRootApparentRepositorySourceInputDispositionView<'_>,
) -> bool {
    let Some(view) = route.view() else {
        return false;
    };
    match disposition {
        HostRootApparentRepositorySourceInputDispositionView::Main => {
            view.canonical_repo().is_root()
        }
        HostRootApparentRepositorySourceInputDispositionView::Input(input) => {
            let capability = input.view().capability();
            capability.workspace() == workspace && route.source_capability_matches(capability)
        }
    }
}
impl HostRootApparentRepositorySourceInput {
    pub(super) fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub(super) fn view(&self) -> Option<HostRootApparentRepositorySourceInputView<'_>> {
        let route = self.predecessor.as_ref().as_ref().ok()?;
        let route_view = route.view()?;
        let disposition = match &self.disposition {
            HostRootApparentRepositorySourceInputDisposition::Main => {
                HostRootApparentRepositorySourceInputDispositionView::Main
            }
            HostRootApparentRepositorySourceInputDisposition::Input(input) => {
                HostRootApparentRepositorySourceInputDispositionView::Input(input)
            }
        };
        if route.workspace() != &self.workspace
            || route_view.apparent_repo() != &self.apparent_repo
            || !association_is_valid(&self.workspace, route, disposition)
        {
            return None;
        }
        Some(HostRootApparentRepositorySourceInputView {
            apparent_repo: route_view.apparent_repo(),
            canonical_repo: route_view.canonical_repo(),
            disposition,
        })
    }
}
impl<'a> HostRootApparentRepositorySourceInputView<'a> {
    pub(super) fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }
    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }
    pub(super) fn disposition(self) -> HostRootApparentRepositorySourceInputDispositionView<'a> {
        self.disposition
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourceInputErrorKind {
    Route(Arc<HostRootApparentRepositoryRouteResult>),
    InvalidRoute(Arc<HostRootApparentRepositoryRouteResult>),
    Projection {
        predecessor: Arc<HostRootApparentRepositoryRouteResult>,
        error: HostRepositorySourceInputError,
    },
    Compute(Arc<str>),
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourceInputError {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    kind: HostRootApparentRepositorySourceInputErrorKind,
}
impl fmt::Display for HostRootApparentRepositorySourceInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
impl std::error::Error for HostRootApparentRepositorySourceInputError {}
pub(super) type HostRootApparentRepositorySourceInputResult =
    Result<HostRootApparentRepositorySourceInput, HostRootApparentRepositorySourceInputError>;
pub(super) type HostRootApparentRepositorySourceInputOutcome =
    SourcePreparationOutcome<Arc<HostRootApparentRepositorySourceInputResult>>;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositorySourceInputKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}
impl HostRootApparentRepositorySourceInputKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self {
            workspace,
            apparent_repo,
        })
    }
}
impl fmt::Display for HostRootApparentRepositorySourceInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositorySourceInputObservationKey(
    HostRootApparentRepositorySourceInputKey,
);
impl HostRootApparentRepositorySourceInputObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        HostRootApparentRepositorySourceInputKey::new(workspace, apparent_repo).map(Self)
    }
}
impl fmt::Display for HostRootApparentRepositorySourceInputObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostRootApparentRepositorySourceInput {
    result: Arc<HostRootApparentRepositorySourceInputResult>,
    observations: PathObservationEpoch,
}
impl ObservedHostRootApparentRepositorySourceInput {
    pub(super) fn result(
        &self,
    ) -> &Arc<
        Result<HostRootApparentRepositorySourceInput, HostRootApparentRepositorySourceInputError>,
    > {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootApparentRepositorySourceInputObservationError {
    Route(HostRootApparentRepositoryRouteObservationError),
}
impl Dupe for RootApparentRepositorySourceInputObservationError {}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourceInputObservationError(
    RootApparentRepositorySourceInputObservationError,
);
impl Dupe for HostRootApparentRepositorySourceInputObservationError {}
#[derive(Clone, Copy)]
enum RootApparentRepositorySourceInputMode {
    Legacy,
    Observed,
}

type RootApparentRepositorySourceInputDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<HostRootApparentRepositorySourceInputResult>,
            PathObservationEpoch,
        ),
        RootApparentRepositorySourceInputObservationError,
    >,
>;

fn root_apparent_repository_source_input_error(
    key: &HostRootApparentRepositorySourceInputKey,
    kind: HostRootApparentRepositorySourceInputErrorKind,
) -> Arc<HostRootApparentRepositorySourceInputResult> {
    Arc::new(Err(HostRootApparentRepositorySourceInputError {
        workspace: key.workspace.clone(),
        apparent_repo: key.apparent_repo.clone(),
        kind,
    }))
}

fn finish_root_apparent_repository_source_input(
    key: &HostRootApparentRepositorySourceInputKey,
    predecessor: Arc<HostRootApparentRepositoryRouteResult>,
    observations: PathObservationEpoch,
) -> (
    Arc<HostRootApparentRepositorySourceInputResult>,
    PathObservationEpoch,
) {
    let terminal = |kind| root_apparent_repository_source_input_error(key, kind);
    let source = predecessor
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|route| route.source_capability());
    let result = match completed_route_disposition(predecessor.is_ok(), source.is_some()) {
        CompletedRouteDisposition::Route => terminal(
            HostRootApparentRepositorySourceInputErrorKind::Route(predecessor),
        ),
        CompletedRouteDisposition::InvalidRoute => {
            terminal(HostRootApparentRepositorySourceInputErrorKind::InvalidRoute(predecessor))
        }
        CompletedRouteDisposition::Source => {
            let disposition = match source.expect("completed disposition checked source presence") {
                HostRootApparentRepositorySourceDisposition::Main => {
                    HostRootApparentRepositorySourceInputDisposition::Main
                }
                HostRootApparentRepositorySourceDisposition::Capability(capability) => {
                    match host_repository_source_input(capability) {
                        Ok(input) => HostRootApparentRepositorySourceInputDisposition::Input(input),
                        Err(error) => {
                            return (
                                terminal(
                                    HostRootApparentRepositorySourceInputErrorKind::Projection {
                                        predecessor,
                                        error,
                                    },
                                ),
                                observations,
                            );
                        }
                    }
                }
            };
            let certificate = HostRootApparentRepositorySourceInput {
                workspace: key.workspace.clone(),
                apparent_repo: key.apparent_repo.clone(),
                predecessor: predecessor.clone(),
                disposition,
            };
            if certificate.view().is_none() {
                terminal(HostRootApparentRepositorySourceInputErrorKind::InvalidRoute(predecessor))
            } else {
                Arc::new(Ok(certificate))
            }
        }
    };
    (result, observations)
}

async fn compute_root_apparent_repository_source_input(
    key: &HostRootApparentRepositorySourceInputKey,
    mode: RootApparentRepositorySourceInputMode,
    ctx: &mut DiceComputations<'_>,
) -> RootApparentRepositorySourceInputDriverOutcome {
    let (predecessor, observations) = match mode {
        RootApparentRepositorySourceInputMode::Legacy => match ctx
            .compute(
                &HostRootApparentRepositoryRouteKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("source-input key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(predecessor)) => {
                (predecessor, PathObservationEpoch::empty())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    root_apparent_repository_source_input_error(
                        key,
                        HostRootApparentRepositorySourceInputErrorKind::Compute(
                            error.to_string().into(),
                        ),
                    ),
                    PathObservationEpoch::empty(),
                )));
            }
        },
        RootApparentRepositorySourceInputMode::Observed => match ctx
            .compute(
                &HostRootApparentRepositoryRouteObservationKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("source-input key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootApparentRepositorySourceInputObservationError::Route(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().clone())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    root_apparent_repository_source_input_error(
                        key,
                        HostRootApparentRepositorySourceInputErrorKind::Compute(
                            error.to_string().into(),
                        ),
                    ),
                    PathObservationEpoch::empty(),
                )));
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok(finish_root_apparent_repository_source_input(
        key,
        predecessor,
        observations,
    )))
}

#[async_trait]
impl Key for HostRootApparentRepositorySourceInputKey {
    type Value = HostRootApparentRepositorySourceInputOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_input(
            self,
            RootApparentRepositorySourceInputMode::Legacy,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy source input has no observation outer")
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                assert_eq!(observations, PathObservationEpoch::empty());
                SourcePreparationOutcome::Complete(result)
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
impl Key for HostRootApparentRepositorySourceInputObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostRootApparentRepositorySourceInput,
            HostRootApparentRepositorySourceInputObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_input(
            &self.0,
            RootApparentRepositorySourceInputMode::Observed,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostRootApparentRepositorySourceInputObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostRootApparentRepositorySourceInput {
                        result,
                        observations,
                    },
                ))
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
#[cfg(test)]
pub(super) mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_events_v2::EventBatch;
    use slug_loading_v2::RepositoryPackageLoadKey;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::transaction_with_command_override;
    use super::super::generated_repository_definition::tests::validated;
    use super::super::root_apparent_repository_definition::tests::prepare_builtin;
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRoute;
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRouteError;
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRouteObservationError;
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRouteObservationKey;
    use super::super::root_apparent_repository_route::ObservedHostRootApparentRepositoryRoute;
    use super::*;

    type ObservedContext<'a> = (
        &'a Arc<Arc<Dice>>,
        &'a NormalizedAbsolutePath,
        &'a HostRootApparentRepositorySourceInputObservationKey,
        &'a HostRootApparentRepositoryRouteObservationKey,
    );

    #[test]
    fn root_apparent_repository_route_observation_surface_is_sibling_usable() {
        let key = HostRootApparentRepositoryRouteObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "observed-HostRootApparentRepositoryRouteKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\") }"
        );

        fn inspect(
            _: &<HostRootApparentRepositoryRouteObservationKey as Key>::Value,
            observed: &ObservedHostRootApparentRepositoryRoute,
            _: &HostRootApparentRepositoryRouteObservationError,
        ) {
            let _: &Arc<
                Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostRootApparentRepositoryRoute,
                        HostRootApparentRepositoryRouteObservationError,
                    >,
                >,
                &ObservedHostRootApparentRepositoryRoute,
                &HostRootApparentRepositoryRouteObservationError,
            );
    }

    #[derive(Default)]
    struct Tracker {
        order: Mutex<Vec<(&'static str, ActivationKind)>>,
        observed_owner: Mutex<Vec<ActivationKind>>,
        observed_route: Mutex<Vec<ActivationKind>>,
        activations: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
        events: Mutex<usize>,
        forbidden: Mutex<Vec<&'static str>>,
    }
    impl ActivationTracker for Tracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }
        fn tracks_rich_activations(&self) -> bool {
            true
        }
        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let kind = activation.kind();
            let batch = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe);
            self.activations
                .lock()
                .unwrap()
                .push((key.to_string(), kind, batch));
            let owner = key
                .downcast_ref::<HostRootApparentRepositorySourceInputKey>()
                .is_some();
            let route = key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some();
            if key
                .downcast_ref::<HostRootApparentRepositorySourceInputObservationKey>()
                .is_some()
            {
                self.observed_owner.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostRootApparentRepositoryRouteObservationKey>()
                .is_some()
            {
                self.observed_route.lock().unwrap().push(kind);
            } else if owner || route {
                self.order
                    .lock()
                    .unwrap()
                    .push((if owner { "owner" } else { "route" }, kind));
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            } else if key.downcast_ref::<RootRepositoryRouteKey>().is_some() {
                self.forbidden.lock().unwrap().push("root-route");
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryPackageSourceKey>().is_some() {
                self.forbidden.lock().unwrap().push("package-source");
            } else if key.downcast_ref::<RepositoryPackageLoadKey>().is_some() {
                self.forbidden.lock().unwrap().push("package-load");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            } else if key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
            {
                self.forbidden.lock().unwrap().push("source");
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                self.forbidden.lock().unwrap().push("filesystem");
            }
        }
    }
    impl Tracker {
        fn clear(&self) {
            self.order.lock().unwrap().clear();
            self.observed_owner.lock().unwrap().clear();
            self.observed_route.lock().unwrap().clear();
            self.activations.lock().unwrap().clear();
            self.dependencies.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
    }
    fn observed_source_input_value(
        outcome: &<HostRootApparentRepositorySourceInputObservationKey as Key>::Value,
    ) -> &ObservedHostRootApparentRepositorySourceInput {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("observed source input must have a carrier: {value:?}"),
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FinishedKind {
        Ok,
        Route,
        InvalidRoute,
        Projection,
    }
    fn assert_finished(
        key: &HostRootApparentRepositorySourceInputKey,
        route: &ObservedHostRootApparentRepositoryRoute,
        expected: FinishedKind,
    ) -> Arc<HostRootApparentRepositorySourceInputResult> {
        let (result, epoch) = finish_root_apparent_repository_source_input(
            key,
            route.result().clone(),
            route.observations().clone(),
        );
        assert_eq!(&epoch, route.observations());
        if expected == FinishedKind::Ok {
            assert!(result.is_ok());
        } else {
            let error = result.as_ref().as_ref().unwrap_err();
            let (actual, retained) = match &error.kind {
                HostRootApparentRepositorySourceInputErrorKind::Route(value) => {
                    (FinishedKind::Route, value)
                }
                HostRootApparentRepositorySourceInputErrorKind::InvalidRoute(value) => {
                    (FinishedKind::InvalidRoute, value)
                }
                HostRootApparentRepositorySourceInputErrorKind::Projection {
                    predecessor, ..
                } => (FinishedKind::Projection, predecessor),
                value => panic!("unexpected finished source error: {value:?}"),
            };
            assert_eq!(actual, expected);
            assert!(Arc::ptr_eq(retained, route.result()));
        }
        result
    }
    macro_rules! dependency_row {
        ($tracker:expr, $key:expr) => {
            $tracker
                .dependencies
                .lock()
                .unwrap()
                .iter()
                .find(|(name, _)| name == $key)
                .unwrap()
                .1
                .clone()
        };
    }
    macro_rules! event_rows {
        ($tracker:expr) => {
            $tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(owner, _, batch)| batch.dupe().map(|batch| (owner.clone(), batch)))
                .collect::<Vec<(String, EventBatch)>>()
        };
    }
    macro_rules! complete_after_materialization {
        ($tx:expr, $dice:expr, $workspace:expr, $key:expr, $tracker:expr) => {{
            let mut tx = $tx;
            let mut outcome = tx.compute($key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &outcome {
                let request = need
                    .repository_materializations()
                    .values()
                    .next()
                    .unwrap()
                    .clone();
                tx = materialized_transaction($dice, $workspace, request, $tracker.clone()).await;
                $tracker.clear();
                outcome = tx.compute($key).await.unwrap();
            }
            (tx, outcome)
        }};
    }
    async fn materialized_transaction(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
        request: Arc<RepositoryMaterializationRequest>,
        tracker: Arc<Tracker>,
    ) -> dice::DiceTransaction {
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace.clone(),
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }
    async fn changed_observed_state(
        context: ObservedContext<'_>,
        module: &str,
        extension: &str,
    ) -> (
        ObservedHostRootApparentRepositorySourceInput,
        ObservedHostRootApparentRepositoryRoute,
    ) {
        let tx = transaction(context.0.as_ref(), module, extension, true, None).await;
        let (_, parent, child) =
            completed_observed_state(context, tx, Arc::new(Tracker::default())).await;
        (parent, child)
    }
    async fn completed_observed_state(
        context: ObservedContext<'_>,
        tx: dice::DiceTransaction,
        tracker: Arc<Tracker>,
    ) -> (
        dice::DiceTransaction,
        ObservedHostRootApparentRepositorySourceInput,
        ObservedHostRootApparentRepositoryRoute,
    ) {
        let (mut tx, outcome) =
            complete_after_materialization!(tx, context.0.as_ref(), context.1, context.2, tracker);
        let parent = observed_source_input_value(&outcome).dupe();
        let child = tx.compute(context.3).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("observed route child must have a carrier")
        };
        let global = tx.compute(&PathObservationEpochKey).await.unwrap();
        assert_eq!(parent.observations(), child.observations());
        for (demand, result) in parent.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        (tx, parent, child)
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RealSourceInputFamily {
        Generated,
        SelectedWorkspace,
        SelectedCommand,
        MappingFailure,
        Missing,
        Main,
        Builtin,
    }
    async fn real_source_input_transaction(
        dice: &Arc<Dice>,
        family: RealSourceInputFamily,
        tracker: Arc<Tracker>,
    ) -> dice::DiceTransaction {
        if family == RealSourceInputFamily::Builtin {
            prepare_builtin(dice, &NormalizedAbsolutePath::new(WORKSPACE).unwrap()).await;
        } else if family == RealSourceInputFamily::SelectedCommand {
            let module = "module(name='bazel_tools')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
            let _ = transaction_with_command_override(dice, module, EXTENSION_A, "local").await;
        } else {
            let module = match family {
                RealSourceInputFamily::Generated | RealSourceInputFamily::Missing => MODULE,
                RealSourceInputFamily::SelectedWorkspace => {
                    "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n"
                }
                RealSourceInputFamily::MappingFailure => "this is not valid Starlark\n",
                RealSourceInputFamily::Main => {
                    "module(name='bazel_tools', repo_name='root_self')\n"
                }
                _ => unreachable!(),
            };
            return transaction(dice, module, EXTENSION_A, true, Some(tracker)).await;
        }
        dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker),
            ..Default::default()
        })
        .commit()
        .await
    }
    pub(in crate::runtime) async fn complete_local(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
        key: &HostRootApparentRepositorySourceInputKey,
    ) -> HostRootApparentRepositorySourceInputOutcome {
        let route_key = HostRootApparentRepositoryRouteKey::new(
            key.workspace.clone(),
            key.apparent_repo.clone(),
        )
        .unwrap();
        let mut tx = dice.updater().commit().await;
        let mut outcome = tx.compute(&route_key).await.unwrap();
        let mut requests = Vec::<Arc<RepositoryMaterializationRequest>>::new();
        while let SourcePreparationOutcome::Need(need) = &outcome {
            for request in need.repository_materializations().values() {
                if !requests.iter().any(|seen| seen.id == request.id) {
                    requests.push(request.clone());
                }
            }
            let entries =
                requests
                    .iter()
                    .cloned()
                    .map(|request| RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    });
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(
                    RepositoryMaterializationResultEpochKey {
                        workspace: workspace.clone(),
                    },
                    RepositoryMaterializationResultEpoch::new(workspace.clone(), entries).unwrap(),
                )])
                .unwrap();
            tx = updater.commit().await;
            outcome = tx.compute(&route_key).await.unwrap();
        }
        let tracker = Arc::new(Tracker::default());
        tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let owner = tx.compute(key).await.unwrap();
        let order = tracker.order.lock().unwrap();
        assert_eq!(order.last(), Some(&("owner", ActivationKind::Evaluated)));
        assert!(
            order[..order.len() - 1]
                .iter()
                .all(|row| row == &("route", ActivationKind::Reused))
        );
        drop(order);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        })
        .commit()
        .await
        .compute(key)
        .await
        .unwrap();
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [("owner", ActivationKind::Reused)]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        owner
    }
    pub(in crate::runtime) fn value(
        outcome: &HostRootApparentRepositorySourceInputOutcome,
    ) -> &HostRootApparentRepositorySourceInput {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("source input must complete: {outcome:?}")
        };
        value.as_ref().as_ref().unwrap()
    }
    pub(in crate::runtime) fn corrupt_workspace(
        value: &HostRootApparentRepositorySourceInput,
    ) -> HostRootApparentRepositorySourceInput {
        let mut corrupt = value.clone();
        corrupt.workspace = NormalizedAbsolutePath::new("/other").unwrap();
        corrupt
    }
    fn key_shape_is_fail_closed() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        assert!(
            HostRootApparentRepositorySourceInputKey::new(
                workspace.clone(),
                ApparentRepoName::root(),
            )
            .is_none()
        );
        let key = HostRootApparentRepositorySourceInputKey::new(
            workspace,
            ApparentRepoName::new("dep").unwrap(),
        )
        .unwrap();
        assert!(key.to_string().contains("dep"));
        assert_ne!(
            key,
            HostRootApparentRepositorySourceInputKey::new(
                NormalizedAbsolutePath::new("/other").unwrap(),
                ApparentRepoName::new("dep").unwrap(),
            )
            .unwrap()
        );
        for (success, source, expected) in [
            (false, false, CompletedRouteDisposition::Route),
            (false, true, CompletedRouteDisposition::Route),
            (true, false, CompletedRouteDisposition::InvalidRoute),
            (true, true, CompletedRouteDisposition::Source),
        ] {
            assert_eq!(completed_route_disposition(success, source), expected);
        }
        let error = HostRootApparentRepositorySourceInputError {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            apparent_repo: ApparentRepoName::new("dep").unwrap(),
            kind: HostRootApparentRepositorySourceInputErrorKind::Compute("boom".into()),
        };
        assert_eq!(error.apparent_repo.as_str(), "dep");
        assert!(
            matches!(error.kind, HostRootApparentRepositorySourceInputErrorKind::Compute(message) if &*message == "boom")
        );
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_input_identity_finisher_and_terminal_algebra()
    {
        key_shape_is_fail_closed();
        production_edge_is_only_route_then_pure_projection();
        type IKey = HostRootApparentRepositorySourceInputKey;
        type OKey = HostRootApparentRepositorySourceInputObservationKey;
        type RKey = HostRootApparentRepositoryRouteObservationKey;
        let root_workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = |name| ApparentRepoName::new(name).unwrap();
        let observed = |name| OKey::new(root_workspace.clone(), apparent(name)).unwrap();
        let input = |name| IKey::new(root_workspace.clone(), apparent(name)).unwrap();
        let route_key_for = |name| RKey::new(root_workspace.clone(), apparent(name)).unwrap();
        let display_key = OKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            apparent("first"),
        )
        .unwrap();
        let observed_key = observed("first");
        let same = observed("first");
        let other = observed("second");
        let hash = |value: &OKey| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        assert_eq!(
            display_key.to_string(),
            "observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\") }"
        );
        assert!(OKey::new(root_workspace.clone(), ApparentRepoName::root()).is_none());
        assert_eq!(observed_key, same);
        assert_ne!(observed_key, other);
        assert_eq!(hash(&observed_key), hash(&same));
        assert_ne!(hash(&observed_key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let observed = tx.compute(&observed_key).await.unwrap();
        let carrier = observed_source_input_value(&observed);
        assert!(OKey::validity(&observed));
        assert!(OKey::equality(&observed, &observed));
        assert!(!carrier.observations().observations().is_empty());
        let route_key = route_key_for("first");
        assert_eq!(
            dependency_row!(&tracker, &observed_key.to_string()),
            [route_key.to_string()]
        );
        let route = tx.compute(&route_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(route)) = route else {
            panic!("observed route carrier expected")
        };
        let finished = assert_finished(&observed_key.0, &route, FinishedKind::Projection);
        assert_eq!(finished.as_ref(), carrier.result().as_ref());

        let missing_key = input("absent");
        let missing_route_key = route_key_for("absent");
        let missing_route = tx.compute(&missing_route_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(missing_route)) = missing_route else {
            panic!("missing route carrier expected")
        };
        assert_finished(&missing_key, &missing_route, FinishedKind::Route);

        let invalid_key = input("second");
        let main_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut main_tx = transaction(
            &main_dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let main_route_key = route_key_for("root_self");
        let main_route = main_tx.compute(&main_route_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(main_route)) = main_route else {
            panic!("main route carrier expected")
        };
        let main_key = input("root_self");
        assert_finished(&main_key, &main_route, FinishedKind::Ok);
        assert_finished(&invalid_key, &main_route, FinishedKind::InvalidRoute);

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&need_dice, MODULE, EXTENSION_A, true, None).await;
        let mut updater = need_dice.updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::empty(),
            )])
            .unwrap();
        let need = updater.commit().await.compute(&observed_key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!OKey::validity(&need));
        assert!(!OKey::equality(&need, &need));
    }
    async fn preserve_legacy_main_builtin_and_generated_assertions() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut tx = transaction(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let apparent = ApparentRepoName::new("root_self").unwrap();
        let route_key =
            HostRootApparentRepositoryRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let SourcePreparationOutcome::Complete(route_predecessor) =
            tx.compute(&route_key).await.unwrap()
        else {
            panic!("main route must complete")
        };
        let key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent).unwrap();
        let main = tx.compute(&key).await.unwrap();
        let main = value(&main);
        assert!(Arc::ptr_eq(&main.predecessor, &route_predecessor));
        let view = main.view().unwrap();
        assert_eq!(view.apparent_repo().as_str(), "root_self");
        assert!(view.canonical_repo().is_root());
        assert!(matches!(
            view.disposition(),
            HostRootApparentRepositorySourceInputDispositionView::Main
        ));
        let mut corrupt = main.clone();
        corrupt.workspace = NormalizedAbsolutePath::new("/other").unwrap();
        assert!(corrupt.view().is_none());
        corrupt.workspace = workspace.clone();
        corrupt.apparent_repo = ApparentRepoName::new("other").unwrap();
        assert!(corrupt.view().is_none());
        prepare_builtin(&dice, &workspace).await;
        let builtin_key = HostRootApparentRepositorySourceInputKey::new(
            workspace.clone(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        )
        .unwrap();
        let mut tx = dice.updater().commit().await;
        let route_key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(&route_key).await.unwrap()
        else {
            unreachable!()
        };
        let tracker = Arc::new(Tracker::default());
        tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let builtin = tx.compute(&builtin_key).await.unwrap();
        assert!(Arc::ptr_eq(&value(&builtin).predecessor, &predecessor));
        assert_eq!(
            dependency_row!(&tracker, &builtin_key.to_string()),
            [route_key.to_string()]
        );
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [
                ("route", ActivationKind::Reused),
                ("owner", ActivationKind::Evaluated)
            ]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        tx.compute(&builtin_key).await.unwrap();
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [("owner", ActivationKind::Reused)]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let HostRootApparentRepositorySourceInputDispositionView::Input(input) =
            value(&builtin).view().unwrap().disposition()
        else {
            unreachable!()
        };
        let HostRepositorySourceInputDispositionView::Builtin(identity) =
            input.view().disposition()
        else {
            unreachable!()
        };
        assert_eq!(
            identity,
            &BuiltinBazelToolsSnapshot::CURRENT.route_identity()
        );

        let generated_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut generated_tx = transaction(&generated_dice, MODULE, EXTENSION_A, true, None).await;
        validated(&mut generated_tx).await;
        let generated_key = HostRootApparentRepositorySourceInputKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let generated_route_key = HostRootApparentRepositoryRouteKey::new(
            workspace,
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(generated_predecessor) =
            generated_tx.compute(&generated_route_key).await.unwrap()
        else {
            unreachable!()
        };
        let expected_error = {
            let route = generated_predecessor.as_ref().as_ref().unwrap();
            let HostRootApparentRepositorySourceDisposition::Capability(capability) =
                route.source_capability().unwrap()
            else {
                unreachable!()
            };
            host_repository_source_input(capability).unwrap_err()
        };
        let generated_outcome = generated_tx.compute(&generated_key).await.unwrap();
        let SourcePreparationOutcome::Complete(generated) = &generated_outcome else {
            panic!("generated projection must complete")
        };
        let error = generated.as_ref().as_ref().unwrap_err();
        let HostRootApparentRepositorySourceInputErrorKind::Projection { predecessor, error } =
            &error.kind
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(predecessor, &generated_predecessor));
        assert_eq!(error, &expected_error);
        let changed_extension = EXTENSION_A.replacen("value='one'", "value='changed'", 1);
        for (module, extension) in [
            (
                MODULE.replace(
                    "first='first', second='second'",
                    "second='second', first='first'",
                ),
                EXTENSION_A,
            ),
            (
                format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
                EXTENSION_A,
            ),
            (
                format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
                EXTENSION_A,
            ),
            (MODULE.to_owned(), changed_extension.as_str()),
        ] {
            let changed = transaction(&generated_dice, &module, extension, true, None)
                .await
                .compute(&generated_key)
                .await
                .unwrap();
            assert!(!HostRootApparentRepositorySourceInputKey::equality(
                &generated_outcome,
                &changed
            ));
            let restored = transaction(&generated_dice, MODULE, EXTENSION_A, true, None)
                .await
                .compute(&generated_key)
                .await
                .unwrap();
            assert!(HostRootApparentRepositorySourceInputKey::equality(
                &generated_outcome,
                &restored
            ));
        }
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_input_real_families_events_and_parity() {
        type IKey = HostRootApparentRepositorySourceInputKey;
        type OKey = HostRootApparentRepositorySourceInputObservationKey;
        type RKey = HostRootApparentRepositoryRouteObservationKey;
        type EKind = HostRootApparentRepositorySourceInputErrorKind;
        type F = RealSourceInputFamily;
        type D<'a> = HostRootApparentRepositorySourceInputDispositionView<'a>;
        type IE = HostRootApparentRepositorySourceInputError;
        preserve_legacy_main_builtin_and_generated_assertions().await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        for family in [
            F::Generated,
            F::SelectedWorkspace,
            F::SelectedCommand,
            F::MappingFailure,
            F::Missing,
            F::Main,
            F::Builtin,
        ] {
            let apparent = ApparentRepoName::new(match family {
                F::Generated | F::MappingFailure => "first",
                F::SelectedWorkspace | F::SelectedCommand => "local_alias",
                F::Missing => "absent",
                F::Main => "root_self",
                F::Builtin => "bazel_tools",
            })
            .unwrap();
            let observed_key = OKey::new(workspace.clone(), apparent.clone()).unwrap();
            let child_key = RKey::new(workspace.clone(), apparent.clone()).unwrap();
            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let context = (&observed_dice, &workspace, &observed_key, &child_key);
            let observed_tracker = Arc::new(Tracker::default());
            let observed_tx =
                real_source_input_transaction(&observed_dice, family, observed_tracker.clone())
                    .await;
            let (mut observed_tx, carrier, child) =
                completed_observed_state(context, observed_tx, observed_tracker.clone()).await;
            assert_eq!(
                dependency_row!(&observed_tracker, &observed_key.to_string()),
                [child_key.to_string()],
                "{family:?}"
            );
            let activations = observed_tracker.activations.lock().unwrap();
            let parent = activations
                .iter()
                .find(|(name, _, _)| name == &observed_key.to_string())
                .unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated);
            assert!(parent.2.is_none());
            drop(activations);
            let parent_events = event_rows!(&observed_tracker);
            assert_eq!(carrier.observations(), child.observations());
            let global = observed_tx.compute(&PathObservationEpochKey).await.unwrap();
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            match (family, carrier.result().as_ref()) {
                (F::Generated, Err(error)) => {
                    assert!(matches!(error.kind, EKind::Projection { .. }))
                }
                (F::MappingFailure | F::Missing, Err(error)) => {
                    assert!(matches!(error.kind, EKind::Route(_)))
                }
                (F::Main | F::Builtin, Ok(_)) => {}
                (F::SelectedWorkspace | F::SelectedCommand, Ok(value)) => {
                    let D::Input(input) = value.view().unwrap().disposition() else {
                        panic!("selected input expected")
                    };
                    let expected = if family == F::SelectedWorkspace {
                        slug_bzlmod_v2::HostRepositoryLocalPathPolicy::WorkspaceRelative
                    } else {
                        slug_bzlmod_v2::HostRepositoryLocalPathPolicy::CommandAbsolute
                    };
                    assert_eq!(
                        input.view().capability().local_path_policy(),
                        Some(expected)
                    );
                }
                value => panic!("unexpected {family:?} source terminal: {value:?}"),
            }
            match carrier.result().as_ref() {
                Ok(value) => assert!(Arc::ptr_eq(&value.predecessor, child.result())),
                Err(IE {
                    kind: EKind::Route(retained) | EKind::InvalidRoute(retained),
                    ..
                }) => assert!(Arc::ptr_eq(retained, child.result())),
                Err(IE {
                    kind: EKind::Projection { predecessor, .. },
                    ..
                }) => assert!(Arc::ptr_eq(predecessor, child.result())),
                value => panic!("unexpected retained terminal: {value:?}"),
            }
            let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let direct_tracker = Arc::new(Tracker::default());
            let direct_tx =
                real_source_input_transaction(&direct_dice, family, direct_tracker.clone()).await;
            direct_tracker.clear();
            let (_direct_tx, direct) = complete_after_materialization!(
                direct_tx,
                &direct_dice,
                &workspace,
                &child_key,
                direct_tracker
            );
            assert!(matches!(direct, SourcePreparationOutcome::Complete(Ok(_))));
            assert_eq!(parent_events, event_rows!(&direct_tracker), "{family:?}");
            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(Tracker::default());
            let legacy_tx =
                real_source_input_transaction(&legacy_dice, family, legacy_tracker.clone()).await;
            let legacy_key = IKey::new(workspace.clone(), apparent).unwrap();
            let (_legacy_tx, legacy) = complete_after_materialization!(
                legacy_tx,
                &legacy_dice,
                &workspace,
                &legacy_key,
                legacy_tracker
            );
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("{family:?}: legacy source must complete")
            };
            assert_eq!(legacy.as_ref(), carrier.result().as_ref(), "{family:?}");
            observed_tracker.clear();
            let warm = observed_tx.compute(&observed_key).await.unwrap();
            assert!(Arc::ptr_eq(
                observed_source_input_value(&warm).result(),
                carrier.result()
            ));
            let warm = observed_tracker.activations.lock().unwrap();
            assert!(!warm.is_empty());
            assert!(
                warm.iter()
                    .all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none())
            );
            drop(warm);
            assert!(event_rows!(&observed_tracker).is_empty());
        }
    }

    async fn preserve_legacy_selected_need_policy_and_route_terminal_assertions() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let apparent = ApparentRepoName::new("local_alias").unwrap();
        let key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let mut tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let need = tx.compute(&key).await.unwrap();
        assert!(!HostRootApparentRepositorySourceInputKey::validity(&need));
        assert!(!HostRootApparentRepositorySourceInputKey::equality(
            &need, &need
        ));
        let route_key =
            HostRootApparentRepositoryRouteKey::new(workspace.clone(), apparent).unwrap();
        let route_need = tx.compute(&route_key).await.unwrap();
        let (
            SourcePreparationOutcome::Need(owner_need),
            SourcePreparationOutcome::Need(route_need),
        ) = (&need, &route_need)
        else {
            unreachable!()
        };
        assert_eq!(
            owner_need.repository_materializations(),
            route_need.repository_materializations()
        );
        let root = complete_local(&dice, &workspace, &key).await;
        let HostRootApparentRepositorySourceInputDispositionView::Input(root_input) =
            value(&root).view().unwrap().disposition()
        else {
            unreachable!()
        };
        assert_eq!(
            root_input.view().capability().local_path_policy(),
            Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::WorkspaceRelative)
        );
        let HostRepositorySourceInputDispositionView::Request(root_request) =
            root_input.view().disposition()
        else {
            unreachable!()
        };
        let cloned = value(&root).clone();
        let HostRootApparentRepositorySourceInputDispositionView::Input(cloned_input) =
            cloned.view().unwrap().disposition()
        else {
            unreachable!()
        };
        let HostRepositorySourceInputDispositionView::Request(cloned_request) =
            cloned_input.view().disposition()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(root_request, cloned_request));
        let mut corrupt = value(&root).clone();
        corrupt.apparent_repo = ApparentRepoName::new("other").unwrap();
        assert!(corrupt.view().is_none());

        let command_module = "module(name='bazel_tools')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let _tx =
            transaction_with_command_override(&dice, command_module, EXTENSION_A, "local").await;
        let command = complete_local(&dice, &workspace, &key).await;
        let HostRootApparentRepositorySourceInputDispositionView::Input(command_input) =
            value(&command).view().unwrap().disposition()
        else {
            unreachable!()
        };
        assert_eq!(
            command_input.view().capability().local_path_policy(),
            Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::CommandAbsolute)
        );
        assert!(!HostRootApparentRepositorySourceInputKey::equality(
            &root, &command
        ));
        let mut corrupt = value(&root).clone();
        corrupt.disposition = value(&command).disposition.clone();
        assert!(corrupt.view().is_none());

        let _tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let restored = complete_local(&dice, &workspace, &key).await;
        assert!(HostRootApparentRepositorySourceInputKey::equality(
            &root, &restored
        ));

        let missing_apparent = ApparentRepoName::new("absent").unwrap();
        let route_key =
            HostRootApparentRepositoryRouteKey::new(workspace.clone(), missing_apparent.clone())
                .unwrap();
        let mut tx = dice.updater().commit().await;
        let SourcePreparationOutcome::Complete(route_error) = tx.compute(&route_key).await.unwrap()
        else {
            unreachable!()
        };
        let missing_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), missing_apparent)
                .unwrap();
        let SourcePreparationOutcome::Complete(missing) = tx.compute(&missing_key).await.unwrap()
        else {
            unreachable!()
        };
        let error = missing.as_ref().as_ref().unwrap_err();
        assert_eq!(error.workspace, workspace);
        assert_eq!(error.apparent_repo.as_str(), "absent");
        let HostRootApparentRepositorySourceInputErrorKind::Route(predecessor) = &error.kind else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(predecessor, &route_error));
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_input_lifecycle_cancellation_and_nonactivation()
     {
        type OKey = HostRootApparentRepositorySourceInputObservationKey;
        type RKey = HostRootApparentRepositoryRouteObservationKey;
        preserve_legacy_selected_need_policy_and_route_terminal_assertions().await;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = OKey::new(workspace.clone(), apparent.clone()).unwrap();
        let child_key = RKey::new(workspace.clone(), apparent).unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let context = (&dice, &workspace, &key, &child_key);
        let tracker = Arc::new(Tracker::default());
        let a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (a_tx, a, a_child) = completed_observed_state(context, a_tx, tracker.clone()).await;
        let a_result = a.result().clone();
        let a_epoch = a.observations().clone();
        let a_child_result = a_child.result().clone();
        let a_child_epoch = a_child.observations().clone();

        tracker.clear();
        let (_a_tx, warm, warm_child) =
            completed_observed_state(context, a_tx, tracker.clone()).await;
        assert!(Arc::ptr_eq(warm.result(), a.result()));
        assert!(Arc::ptr_eq(warm_child.result(), a_child.result()));
        assert_eq!(
            tracker.observed_owner.lock().unwrap().as_slice(),
            [ActivationKind::Reused]
        );
        assert_eq!(
            tracker.observed_route.lock().unwrap().as_slice(),
            [ActivationKind::Reused]
        );
        assert!(
            tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .all(|(_, _, batch)| batch.is_none())
        );

        let mapping_b = MODULE.replacen(
            "first='first', second='second'",
            "first='second', second='first'",
            1,
        );
        let extension_b = EXTENSION_A.replacen("value='one'", "value='changed'", 1);
        for (module, extension) in [
            (mapping_b.as_str(), EXTENSION_A),
            (MODULE, extension_b.as_str()),
        ] {
            let (changed, changed_child) = changed_observed_state(context, module, extension).await;
            assert_ne!(changed.result(), a.result());
            assert_ne!(changed_child.result(), a_child.result());
            let (restored, restored_child) =
                changed_observed_state(context, MODULE, EXTENSION_A).await;
            assert_eq!(restored.result(), a.result());
            assert_eq!(restored_child.result(), a_child.result());
            assert!(!Arc::ptr_eq(restored.result(), a.result()));
            assert!(!Arc::ptr_eq(restored_child.result(), a_child.result()));
        }

        let neutral_module = format!("{MODULE}\n");
        let (neutral, neutral_child) =
            changed_observed_state(context, &neutral_module, EXTENSION_A).await;
        assert_eq!(neutral.result(), a.result());
        assert_eq!(neutral_child.result(), a_child.result());
        assert_ne!(neutral.observations(), a.observations());
        assert_ne!(neutral_child.observations(), a_child.observations());
        assert!(!Arc::ptr_eq(neutral.result(), a.result()));
        assert!(!Arc::ptr_eq(neutral_child.result(), a_child.result()));
        assert_ne!(neutral, a);
        assert_ne!(neutral_child, a_child);
        assert_eq!(a.result(), &a_result);
        assert_eq!(a.observations(), &a_epoch);
        assert_eq!(a_child.result(), &a_child_result);
        assert_eq!(a_child.observations(), &a_child_epoch);

        let local_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let local_module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_apparent = ApparentRepoName::new("local_alias").unwrap();
        let local_key = OKey::new(workspace.clone(), local_apparent.clone()).unwrap();
        let local_child_key = RKey::new(workspace.clone(), local_apparent).unwrap();
        let local_context = (&local_dice, &workspace, &local_key, &local_child_key);
        let local_tracker = Arc::new(Tracker::default());
        let mut local_tx = transaction(
            &local_dice,
            local_module,
            EXTENSION_A,
            true,
            Some(local_tracker.clone()),
        )
        .await;
        let (local_tx, local_a, local_child_a) =
            completed_observed_state(local_context, local_tx, local_tracker.clone()).await;
        drop(local_tx);
        let command_module = "module(name='bazel_tools')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let _ =
            transaction_with_command_override(&local_dice, command_module, EXTENSION_A, "local")
                .await;
        let mut command_tx = local_dice.updater().commit().await;
        let (command_tx, command, command_child) =
            completed_observed_state(local_context, command_tx, local_tracker.clone()).await;
        drop(command_tx);
        assert_ne!(command.result(), local_a.result());
        assert_ne!(command_child.result(), local_child_a.result());
        let _ = transaction(&local_dice, local_module, EXTENSION_A, true, None).await;
        let mut local_restore_tx = local_dice.updater().commit().await;
        let (local_restore_tx, local_restored, local_child_restored) =
            completed_observed_state(local_context, local_restore_tx, local_tracker).await;
        drop(local_restore_tx);
        assert_eq!(local_restored.result(), local_a.result());
        assert_eq!(local_child_restored.result(), local_child_a.result());
        assert!(!Arc::ptr_eq(local_restored.result(), local_a.result()));
        assert!(!Arc::ptr_eq(
            local_child_restored.result(),
            local_child_a.result()
        ));

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(Tracker::default());
        let mut cancelled = transaction(
            &cancel_dice,
            MODULE,
            EXTENSION_A,
            true,
            Some(cancel_tracker.clone()),
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        let cancelled_trace = format!(
            "{:?}{:?}",
            *cancel_tracker.activations.lock().unwrap(),
            *cancel_tracker.dependencies.lock().unwrap()
        );
        assert!(!cancelled_trace.contains(&key.to_string()));
        let recovery = transaction(
            &cancel_dice,
            MODULE,
            EXTENSION_A,
            true,
            Some(cancel_tracker.clone()),
        )
        .await;
        let (_, recovered, recovered_child) =
            completed_observed_state(context, recovery, cancel_tracker.clone()).await;
        assert_eq!(recovered.result(), a.result());
        assert_eq!(recovered_child.result(), a_child.result());

        let trace = format!(
            "{:?}{:?}",
            *cancel_tracker.activations.lock().unwrap(),
            *cancel_tracker.dependencies.lock().unwrap()
        );
        for forbidden in [
            "HostRootApparentRepositorySourcePathInputKey",
            "HostRootApparentRepositorySourceObservationKey",
            "root-repository-route:",
            "repository-package-source:",
            "repository-source-file:",
            "host-repository-source-file:",
            "build-command-root:",
            "RootModuleBootstrap",
            "bootstrap",
        ] {
            assert!(!trace.contains(forbidden));
        }
        let producer = include_str!("root_apparent_repository_source_input.rs");
        let producer = &producer[producer
            .find("struct HostRootApparentRepositorySourceInputObservationKey")
            .unwrap()..producer.find("#[cfg(test)]").unwrap()];
        for absent in [
            "SourcePathInputKey",
            "SourceObservationKey",
            "RootRepositoryRouteKey",
            "BuildCommand",
            "RootModuleBootstrap",
            "bootstrap",
        ] {
            assert!(!producer.contains(absent));
        }
    }

    fn production_edge_is_only_route_then_pure_projection() {
        let source = include_str!("root_apparent_repository_source_input.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for (needle, count) in [
            (".compute(", 2),
            ("HostRootApparentRepositoryRouteKey::new", 1),
            ("HostRootApparentRepositoryRouteObservationKey::new", 1),
            ("host_repository_source_input(capability)", 1),
            (
                "RootApparentRepositorySourceInputObservationError::Route(error)",
                1,
            ),
            (
                "HostRootApparentRepositorySourceInputObservationError(error)",
                1,
            ),
        ] {
            assert_eq!(production.matches(needle).count(), count, "{needle}");
        }
        for forbidden in [
            ["RepositoryMaterialization", "ResultKey"].concat(),
            ["RepositoryMaterialization", "Key"].concat(),
            ["HostRepository", "PathKey"].concat(),
            ["HostRepositorySource", "FileKey"].concat(),
            ["RepositorySource", "FileKey"].concat(),
            ["RootRepositoryRoute", "Key"].concat(),
            ["RegistryFile", "Key"].concat(),
            ["RepositoryPackage", "SourceKey"].concat(),
            ["RepositoryPackage", "LoadKey"].concat(),
            ["PathObservation", "EpochKey"].concat(),
            ["std::", "fs"].concat(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "forbidden edge: {forbidden}"
            );
        }
        let route_source = include_str!("root_apparent_repository_route.rs");
        for evidence in [
            "HostRootApparentRepositoryRouteKind::SelectedRegistry",
            "Some(HostRepositoryLocalPathPolicy::LocalUnsupported)",
            "HostRepositorySourceCapability::from_repo_spec",
            "view.repo_spec?",
            "view.local_path_policy()?",
        ] {
            assert!(
                route_source.contains(evidence),
                "missing evidence: {evidence}"
            );
        }
    }
}
