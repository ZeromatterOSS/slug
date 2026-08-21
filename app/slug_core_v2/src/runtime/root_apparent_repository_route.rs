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
use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRepositorySourceCapability;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

use super::root_apparent_repository_definition::HostRootApparentRepositoryDeferredKind;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinition;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionError;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKey;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKind;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationError;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationKey;

type DefinitionResult =
    Result<HostRootApparentRepositoryDefinition, HostRootApparentRepositoryDefinitionError>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostRootApparentRepositoryRouteKind {
    Main,
    Builtin,
    SelectedRegistry,
    SelectedNonregistry,
    Generated,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HostRootApparentRepositoryRouteView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    kind: HostRootApparentRepositoryRouteKind,
    repo_spec: Option<&'a RepoSpec>,
    local_path_policy: Option<HostRepositoryLocalPathPolicy>,
}
impl<'a> HostRootApparentRepositoryRouteView<'a> {
    pub(super) fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }
    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }
    fn kind(self) -> HostRootApparentRepositoryRouteKind {
        self.kind
    }
    fn repo_spec(self) -> Option<&'a RepoSpec> {
        self.repo_spec
    }
    fn local_path_policy(self) -> Option<HostRepositoryLocalPathPolicy> {
        self.local_path_policy
    }
}
fn predecessor_view(
    predecessor: &DefinitionResult,
) -> Option<HostRootApparentRepositoryRouteView<'_>> {
    let (apparent_repo, canonical_repo, kind, repo_spec, local_path_policy) = match predecessor {
        Ok(value) => {
            let view = value.view()?;
            let kind = match view.kind() {
                HostRootApparentRepositoryDefinitionKind::SelectedRegistry => {
                    HostRootApparentRepositoryRouteKind::SelectedRegistry
                }
                HostRootApparentRepositoryDefinitionKind::SelectedNonregistry => {
                    HostRootApparentRepositoryRouteKind::SelectedNonregistry
                }
                HostRootApparentRepositoryDefinitionKind::Generated => {
                    HostRootApparentRepositoryRouteKind::Generated
                }
            };
            (
                view.apparent_repo(),
                view.canonical_repo(),
                kind,
                view.repo_spec(),
                Some(view.local_path_policy()),
            )
        }
        Err(error) => {
            let view = error.deferred_view()?;
            let kind = match view.kind() {
                HostRootApparentRepositoryDeferredKind::Main => {
                    HostRootApparentRepositoryRouteKind::Main
                }
                HostRootApparentRepositoryDeferredKind::Builtin => {
                    HostRootApparentRepositoryRouteKind::Builtin
                }
            };
            (
                view.apparent_repo(),
                view.canonical_repo(),
                kind,
                None,
                None,
            )
        }
    };
    Some(HostRootApparentRepositoryRouteView {
        apparent_repo,
        canonical_repo,
        kind,
        repo_spec,
        local_path_policy,
    })
}
fn view_is_consistent(
    request: &ApparentRepoName,
    view: HostRootApparentRepositoryRouteView<'_>,
) -> bool {
    if view.apparent_repo != request {
        return false;
    }
    match view.kind {
        HostRootApparentRepositoryRouteKind::Main => {
            view.canonical_repo.is_root()
                && view.repo_spec.is_none()
                && view.local_path_policy.is_none()
        }
        HostRootApparentRepositoryRouteKind::Builtin => {
            view.canonical_repo.as_str() == "bazel_tools"
                && view.repo_spec.is_none()
                && view.local_path_policy.is_none()
        }
        HostRootApparentRepositoryRouteKind::SelectedRegistry
        | HostRootApparentRepositoryRouteKind::Generated => {
            !view.canonical_repo.is_root()
                && view.canonical_repo.as_str() != "bazel_tools"
                && view.repo_spec.is_some()
                && view.local_path_policy == Some(HostRepositoryLocalPathPolicy::LocalUnsupported)
        }
        HostRootApparentRepositoryRouteKind::SelectedNonregistry => {
            !view.canonical_repo.is_root()
                && view.canonical_repo.as_str() != "bazel_tools"
                && view.repo_spec.is_some()
                && matches!(
                    view.local_path_policy,
                    Some(
                        HostRepositoryLocalPathPolicy::WorkspaceRelative
                            | HostRepositoryLocalPathPolicy::CommandAbsolute
                    )
                )
        }
    }
}
fn completed_disposition(is_success: bool, is_deferred: bool, has_view: bool) -> Option<bool> {
    has_view
        .then_some(true)
        .or_else(|| (is_success || is_deferred).then_some(false))
}
fn invalid_predecessor(
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    predecessor: Arc<DefinitionResult>,
) -> HostRootApparentRepositoryRouteError {
    HostRootApparentRepositoryRouteError {
        workspace,
        apparent_repo,
        kind: HostRootApparentRepositoryRouteErrorKind::InvalidPredecessor(predecessor),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryRoute {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    predecessor: Arc<DefinitionResult>,
}
impl HostRootApparentRepositoryRoute {
    pub(super) fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub(super) fn view(&self) -> Option<HostRootApparentRepositoryRouteView<'_>> {
        let view = predecessor_view(self.predecessor.as_ref())?;
        view_is_consistent(&self.apparent_repo, view).then_some(view)
    }

    pub(super) fn source_capability(&self) -> Option<HostRootApparentRepositorySourceDisposition> {
        source_capability_from_view(&self.workspace, self.view()?)
    }

    pub(super) fn source_capability_matches(
        &self,
        capability: &HostRepositorySourceCapability,
    ) -> bool {
        matches!(
            self.source_capability(),
            Some(HostRootApparentRepositorySourceDisposition::Capability(expected))
                if &expected == capability
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum HostRootApparentRepositorySourceDisposition {
    Main,
    Capability(HostRepositorySourceCapability),
}

fn source_capability_from_view(
    workspace: &NormalizedAbsolutePath,
    view: HostRootApparentRepositoryRouteView<'_>,
) -> Option<HostRootApparentRepositorySourceDisposition> {
    view_is_consistent(view.apparent_repo, view).then_some(())?;
    match view.kind {
        HostRootApparentRepositoryRouteKind::Main => {
            Some(HostRootApparentRepositorySourceDisposition::Main)
        }
        HostRootApparentRepositoryRouteKind::Builtin => HostRepositorySourceCapability::builtin(
            workspace.clone(),
            view.apparent_repo.clone(),
            view.canonical_repo.clone(),
            BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
        )
        .map(HostRootApparentRepositorySourceDisposition::Capability),
        HostRootApparentRepositoryRouteKind::SelectedRegistry
        | HostRootApparentRepositoryRouteKind::SelectedNonregistry
        | HostRootApparentRepositoryRouteKind::Generated => {
            HostRepositorySourceCapability::from_repo_spec(
                workspace.clone(),
                view.apparent_repo.clone(),
                view.canonical_repo.clone(),
                view.repo_spec?,
                view.local_path_policy()?,
            )
            .map(HostRootApparentRepositorySourceDisposition::Capability)
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositoryRouteErrorKind {
    Predecessor(Arc<DefinitionResult>),
    InvalidPredecessor(Arc<DefinitionResult>),
    Compute(Arc<str>),
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryRouteError {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    kind: HostRootApparentRepositoryRouteErrorKind,
}
impl fmt::Display for HostRootApparentRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostRootApparentRepositoryRouteError {}

pub(super) type HostRootApparentRepositoryRouteResult =
    Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>;
pub(super) type HostRootApparentRepositoryRouteOutcome =
    SourcePreparationOutcome<Arc<HostRootApparentRepositoryRouteResult>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositoryRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}

impl HostRootApparentRepositoryRouteKey {
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

impl fmt::Display for HostRootApparentRepositoryRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositoryRouteObservationKey(HostRootApparentRepositoryRouteKey);

impl HostRootApparentRepositoryRouteObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        HostRootApparentRepositoryRouteKey::new(workspace, apparent_repo).map(Self)
    }
}

impl fmt::Display for HostRootApparentRepositoryRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostRootApparentRepositoryRoute {
    result: Arc<HostRootApparentRepositoryRouteResult>,
    observations: PathObservationEpoch,
}

impl ObservedHostRootApparentRepositoryRoute {
    pub(super) fn result(
        &self,
    ) -> &Arc<Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>> {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootApparentRepositoryRouteObservationError {
    Definition(HostRootApparentRepositoryDefinitionObservationError),
}

impl Dupe for RootApparentRepositoryRouteObservationError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryRouteObservationError(
    RootApparentRepositoryRouteObservationError,
);

impl Dupe for HostRootApparentRepositoryRouteObservationError {}

#[derive(Clone, Copy)]
enum RootApparentRepositoryRouteMode {
    Legacy,
    Observed,
}

type RootApparentRepositoryRouteDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<HostRootApparentRepositoryRouteResult>,
            PathObservationEpoch,
        ),
        RootApparentRepositoryRouteObservationError,
    >,
>;

fn finish_root_apparent_repository_route(
    key: &HostRootApparentRepositoryRouteKey,
    predecessor: Arc<DefinitionResult>,
    observations: PathObservationEpoch,
) -> (
    Arc<HostRootApparentRepositoryRouteResult>,
    PathObservationEpoch,
) {
    let terminal = |kind| {
        Arc::new(Err(HostRootApparentRepositoryRouteError {
            workspace: key.workspace.clone(),
            apparent_repo: key.apparent_repo.clone(),
            kind,
        }))
    };
    let view = predecessor_view(predecessor.as_ref());
    let is_success = predecessor.is_ok();
    let is_deferred = matches!(predecessor.as_ref(), Err(error) if error.is_deferred());
    let result = match completed_disposition(is_success, is_deferred, view.is_some()) {
        None => terminal(HostRootApparentRepositoryRouteErrorKind::Predecessor(
            predecessor,
        )),
        Some(false) => Arc::new(Err(invalid_predecessor(
            key.workspace.clone(),
            key.apparent_repo.clone(),
            predecessor,
        ))),
        Some(true) => {
            let view = view.expect("success disposition has a view");
            if !view_is_consistent(&key.apparent_repo, view) {
                Arc::new(Err(invalid_predecessor(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                    predecessor,
                )))
            } else {
                Arc::new(Ok(HostRootApparentRepositoryRoute {
                    workspace: key.workspace.clone(),
                    apparent_repo: key.apparent_repo.clone(),
                    predecessor,
                }))
            }
        }
    };
    (result, observations)
}

async fn compute_root_apparent_repository_route(
    key: &HostRootApparentRepositoryRouteKey,
    mode: RootApparentRepositoryRouteMode,
    ctx: &mut DiceComputations<'_>,
) -> RootApparentRepositoryRouteDriverOutcome {
    let (predecessor, observations) = match mode {
        RootApparentRepositoryRouteMode::Legacy => match ctx
            .compute(
                &HostRootApparentRepositoryDefinitionKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("route key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostRootApparentRepositoryRouteError {
                        workspace: key.workspace.clone(),
                        apparent_repo: key.apparent_repo.clone(),
                        kind: HostRootApparentRepositoryRouteErrorKind::Compute(
                            error.to_string().into(),
                        ),
                    })),
                    PathObservationEpoch::empty(),
                )));
            }
        },
        RootApparentRepositoryRouteMode::Observed => match ctx
            .compute(
                &HostRootApparentRepositoryDefinitionObservationKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("route key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootApparentRepositoryRouteObservationError::Definition(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().clone())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostRootApparentRepositoryRouteError {
                        workspace: key.workspace.clone(),
                        apparent_repo: key.apparent_repo.clone(),
                        kind: HostRootApparentRepositoryRouteErrorKind::Compute(
                            error.to_string().into(),
                        ),
                    })),
                    PathObservationEpoch::empty(),
                )));
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok(finish_root_apparent_repository_route(
        key,
        predecessor,
        observations,
    )))
}

#[async_trait]
impl Key for HostRootApparentRepositoryRouteKey {
    type Value = HostRootApparentRepositoryRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_route(
            self,
            RootApparentRepositoryRouteMode::Legacy,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy route has no observation outer")
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
impl Key for HostRootApparentRepositoryRouteObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostRootApparentRepositoryRoute,
            HostRootApparentRepositoryRouteObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_route(
            &self.0,
            RootApparentRepositoryRouteMode::Observed,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostRootApparentRepositoryRouteObservationError(error)),
            ),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostRootApparentRepositoryRoute {
                    result,
                    observations,
                }))
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
mod tests {
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
    use slug_bzlmod_v2::HostRepositorySourceCapabilitySource;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
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
    use super::super::generated_repository_definition::tests::names;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::transaction_with_command_override;
    use super::super::generated_repository_definition::tests::validated;
    use super::super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationError;
    use super::super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationKey;
    use super::super::root_apparent_repository_definition::ObservedHostRootApparentRepositoryDefinition;
    use super::super::root_apparent_repository_definition::tests::prepare_builtin;
    use super::*;

    #[test]
    fn root_apparent_repository_definition_observation_surface_is_sibling_usable() {
        let key = HostRootApparentRepositoryDefinitionObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "observed-host-root-apparent-repository-definition:\"/workspace\":@first"
        );

        fn inspect(
            _: &<HostRootApparentRepositoryDefinitionObservationKey as Key>::Value,
            observed: &ObservedHostRootApparentRepositoryDefinition,
            _: &HostRootApparentRepositoryDefinitionObservationError,
        ) {
            let _: &Arc<
                Result<
                    HostRootApparentRepositoryDefinition,
                    HostRootApparentRepositoryDefinitionError,
                >,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostRootApparentRepositoryDefinition,
                        HostRootApparentRepositoryDefinitionObservationError,
                    >,
                >,
                &ObservedHostRootApparentRepositoryDefinition,
                &HostRootApparentRepositoryDefinitionObservationError,
            );
    }

    #[derive(Default)]
    struct Tracker {
        route: Mutex<Vec<ActivationKind>>,
        predecessor: Mutex<Vec<ActivationKind>>,
        observed_route: Mutex<Vec<ActivationKind>>,
        observed_predecessor: Mutex<Vec<ActivationKind>>,
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
            if key
                .downcast_ref::<HostRootApparentRepositoryRouteObservationKey>()
                .is_some()
            {
                self.observed_route.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionObservationKey>()
                .is_some()
            {
                self.observed_predecessor.lock().unwrap().push(kind);
            } else if key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some()
            {
                self.route.lock().unwrap().push(kind);
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            } else if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionKey>()
                .is_some()
            {
                self.predecessor.lock().unwrap().push(kind);
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
            self.route.lock().unwrap().clear();
            self.predecessor.lock().unwrap().clear();
            self.observed_route.lock().unwrap().clear();
            self.observed_predecessor.lock().unwrap().clear();
            self.activations.lock().unwrap().clear();
            self.dependencies.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
    }

    fn observed_route_value(
        outcome: &<HostRootApparentRepositoryRouteObservationKey as Key>::Value,
    ) -> &ObservedHostRootApparentRepositoryRoute {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("observed route must have a carrier: {value:?}"),
        }
    }

    fn dependency_row(tracker: &Tracker, key: &str) -> Vec<String> {
        tracker
            .dependencies
            .lock()
            .unwrap()
            .iter()
            .find(|(name, _)| name == key)
            .unwrap_or_else(|| panic!("missing dependency row for {key}"))
            .1
            .clone()
    }

    fn event_rows(tracker: &Tracker) -> Vec<(String, EventBatch)> {
        tracker
            .activations
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(owner, _, batch)| batch.dupe().map(|batch| (owner.clone(), batch)))
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RealRouteFamily {
        Generated,
        SelectedNonregistry,
        MappingFailure,
        Main,
        Builtin,
    }

    async fn real_route_transaction(
        dice: &Arc<Dice>,
        family: RealRouteFamily,
        tracker: Arc<Tracker>,
    ) -> dice::DiceTransaction {
        if family == RealRouteFamily::Builtin {
            prepare_builtin(dice, &NormalizedAbsolutePath::new(WORKSPACE).unwrap()).await;
            return dice
                .updater_with_data(UserComputationData {
                    cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
                    activation_tracker: Some(tracker),
                    ..Default::default()
                })
                .commit()
                .await;
        }
        let (module, extension) = match family {
            RealRouteFamily::Generated => (MODULE, EXTENSION_A),
            RealRouteFamily::SelectedNonregistry => (
                "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n",
                EXTENSION_A,
            ),
            RealRouteFamily::MappingFailure => ("this is not valid Starlark\n", EXTENSION_A),
            RealRouteFamily::Main => (
                "module(name='bazel_tools', repo_name='root_self')\n",
                EXTENSION_A,
            ),
            RealRouteFamily::Builtin => unreachable!(),
        };
        transaction(dice, module, extension, true, Some(tracker)).await
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

    async fn observed_route_state(
        transaction: &mut dice::DiceTransaction,
        key: &HostRootApparentRepositoryRouteObservationKey,
        child_key: &HostRootApparentRepositoryDefinitionObservationKey,
    ) -> (
        ObservedHostRootApparentRepositoryRoute,
        ObservedHostRootApparentRepositoryDefinition,
    ) {
        let outcome = transaction.compute(key).await.unwrap();
        let parent = observed_route_value(&outcome).dupe();
        let child = transaction.compute(child_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("observed definition child must have a carrier")
        };
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        assert_eq!(parent.observations(), child.observations());
        for (demand, result) in parent.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        (parent, child)
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_root_apparent_repository_route_identity_finisher_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let same = HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let other = HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), ApparentRepoName::new("second").unwrap()).unwrap();
        let display = HostRootApparentRepositoryRouteObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap(), apparent.clone()).unwrap();
        let hash = |value: &HostRootApparentRepositoryRouteObservationKey| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        assert_eq!(
            display.to_string(),
            "observed-HostRootApparentRepositoryRouteKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\") }"
        );
        assert!(HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), ApparentRepoName::root()).is_none());
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let outcome = tx.compute(&key).await.unwrap();
        let carrier = observed_route_value(&outcome);
        assert!(carrier.result().as_ref().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostRootApparentRepositoryRouteObservationKey::validity(&outcome));
        assert!(HostRootApparentRepositoryRouteObservationKey::equality(&outcome, &outcome));
        let child_key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        assert_eq!(dependency_row(&tracker, &key.to_string()), [child_key.to_string()]);
        let child = tx.compute(&child_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("definition carrier expected")
        };
        let (finished, observations) = finish_root_apparent_repository_route(&key.0, child.result().clone(), child.observations().clone());
        assert_eq!(&observations, child.observations());
        let finished = finished.as_ref().as_ref().unwrap();
        assert!(Arc::ptr_eq(&finished.predecessor, child.result()));
        assert_eq!(finished, carrier.result().as_ref().as_ref().unwrap());

        let missing_apparent = ApparentRepoName::new("absent").unwrap();
        let missing_route = HostRootApparentRepositoryRouteKey::new(workspace.clone(), missing_apparent.clone()).unwrap();
        let missing_child_key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), missing_apparent).unwrap();
        let missing_child = tx.compute(&missing_child_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(missing_child)) = missing_child else {
            panic!("missing definition carrier expected")
        };
        let (missing, missing_epoch) = finish_root_apparent_repository_route(&missing_route, missing_child.result().clone(), missing_child.observations().clone());
        assert_eq!(&missing_epoch, missing_child.observations());
        let HostRootApparentRepositoryRouteErrorKind::Predecessor(retained) = &missing.as_ref().as_ref().unwrap_err().kind else {
            panic!("missing definition must remain a predecessor terminal")
        };
        assert!(Arc::ptr_eq(retained, missing_child.result()));
        for row in [
            (true, false, true, Some(true)),
            (false, true, true, Some(true)),
            (false, false, false, None),
            (true, false, false, Some(false)),
            (false, true, false, Some(false)),
        ] {
            assert_eq!(completed_disposition(row.0, row.1, row.2), row.3);
        }

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&need_dice, MODULE, EXTENSION_A, true, None).await;
        let mut updater = need_dice.updater();
        updater.changed_to(vec![(PathObservationEpochKey, PathObservationEpoch::empty())]).unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostRootApparentRepositoryRouteObservationKey::validity(&need));
        assert!(!HostRootApparentRepositoryRouteObservationKey::equality(&need, &need));

        let source = include_str!("root_apparent_repository_route.rs");
        let producer = &source[source
            .find("struct HostRootApparentRepositoryRouteObservationKey")
            .unwrap()..source.find("#[cfg(test)]").unwrap()];
        assert_eq!(producer.matches("HostRootApparentRepositoryDefinitionObservationKey::new").count(), 1);
        assert_eq!(producer.matches("RootApparentRepositoryRouteObservationError::Definition(error)").count(), 1);
        assert_eq!(producer.matches("HostRootApparentRepositoryRouteObservationError(error)").count(), 1);
        for absent in [
            "PathObservationEpoch::from",
            "merge_observ",
            "OperationMismatch",
            "EventBatch",
            "store_evaluation_data",
        ] {
            assert!(!producer.contains(absent), "unexpected route owner shape: {absent}");
        }
        let definition_source = include_str!("root_apparent_repository_definition.rs");
        for evidence in [
            "HostRootApparentRepositoryDefinitionKind::SelectedRegistry",
            "canonical_repo: definition.canonical_repo()",
            "repo_spec: definition.repo_spec()",
            "local_path_policy",
            "observed_root_apparent_repository_definition_real_order_events_and_parity",
        ] {
            assert!(definition_source.contains(evidence));
        }
        let parent_route = &source[source.find("fn predecessor_view(").unwrap()..source.find("struct HostRootApparentRepositoryRouteObservationKey").unwrap()];
        for evidence in [
            "HostRootApparentRepositoryDefinitionKind::SelectedRegistry",
            "HostRootApparentRepositoryRouteKind::SelectedRegistry",
            "view.repo_spec.is_some()",
            "Some(HostRepositoryLocalPathPolicy::LocalUnsupported)",
            "HostRepositorySourceCapability::from_repo_spec",
            "view.repo_spec?",
            "view.local_path_policy()?",
        ] {
            assert!(parent_route.contains(evidence), "missing static SelectedRegistry route evidence: {evidence}");
        }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn observed_root_apparent_repository_route_real_families_events_and_parity() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        for family in [
            RealRouteFamily::Generated,
            RealRouteFamily::SelectedNonregistry,
            RealRouteFamily::MappingFailure,
            RealRouteFamily::Main,
            RealRouteFamily::Builtin,
        ] {
            let apparent = ApparentRepoName::new(match family {
                RealRouteFamily::Generated | RealRouteFamily::MappingFailure => "first",
                RealRouteFamily::SelectedNonregistry => "local_alias",
                RealRouteFamily::Main => "root_self",
                RealRouteFamily::Builtin => "bazel_tools",
            })
            .unwrap();
            let key = HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
            let child_key = HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent.clone()).unwrap();

            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let observed_tracker = Arc::new(Tracker::default());
            let mut observed_tx = real_route_transaction(&observed_dice, family, observed_tracker.clone()).await;
            let mut observed = observed_tx.compute(&key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &observed {
                assert_eq!(family, RealRouteFamily::SelectedNonregistry);
                let request = need.repository_materializations().values().next().unwrap().clone();
                observed_tx = materialized_transaction(&observed_dice, &workspace, request, observed_tracker.clone()).await;
                observed_tracker.clear();
                observed = observed_tx.compute(&key).await.unwrap();
            }
            let carrier = observed_route_value(&observed).dupe();
            assert_eq!(dependency_row(&observed_tracker, &key.to_string()), [child_key.to_string()], "{family:?}");
            let activations = observed_tracker.activations.lock().unwrap();
            let parent = activations.iter().find(|(name, _, _)| name == &key.to_string()).unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated);
            assert!(parent.2.is_none());
            drop(activations);
            let parent_events = event_rows(&observed_tracker);
            let child = observed_tx.compute(&child_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else {
                panic!("{family:?}: observed child must have a carrier")
            };
            assert_eq!(carrier.observations(), child.observations());
            let global = observed_tx.compute(&PathObservationEpochKey).await.unwrap();
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            match (family, carrier.result().as_ref()) {
                (RealRouteFamily::Generated, Ok(route)) => assert_eq!(
                    route.view().unwrap().kind(),
                    HostRootApparentRepositoryRouteKind::Generated
                ),
                (RealRouteFamily::SelectedNonregistry, Ok(route)) => assert_eq!(
                    route.view().unwrap().kind(),
                    HostRootApparentRepositoryRouteKind::SelectedNonregistry
                ),
                (RealRouteFamily::Main, Ok(route)) => assert_eq!(
                    route.view().unwrap().kind(),
                    HostRootApparentRepositoryRouteKind::Main
                ),
                (RealRouteFamily::Builtin, Ok(route)) => assert_eq!(
                    route.view().unwrap().kind(),
                    HostRootApparentRepositoryRouteKind::Builtin
                ),
                (RealRouteFamily::MappingFailure, Err(error)) => assert!(matches!(
                    error.kind,
                    HostRootApparentRepositoryRouteErrorKind::Predecessor(_)
                )),
                value => panic!("unexpected {family:?} route terminal: {value:?}"),
            }
            match carrier.result().as_ref() {
                Ok(route) => assert!(Arc::ptr_eq(&route.predecessor, child.result())),
                Err(HostRootApparentRepositoryRouteError {
                    kind: HostRootApparentRepositoryRouteErrorKind::Predecessor(retained),
                    ..
                }) => assert!(Arc::ptr_eq(retained, child.result())),
                value => panic!("unexpected retained terminal: {value:?}"),
            }

            let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let direct_tracker = Arc::new(Tracker::default());
            let mut direct_tx = real_route_transaction(&direct_dice, family, direct_tracker.clone()).await;
            let mut direct = direct_tx.compute(&child_key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &direct {
                let request = need.repository_materializations().values().next().unwrap().clone();
                direct_tx = materialized_transaction(&direct_dice, &workspace, request, direct_tracker.clone()).await;
                direct_tracker.clear();
                direct = direct_tx.compute(&child_key).await.unwrap();
            }
            assert!(matches!(direct, SourcePreparationOutcome::Complete(Ok(_))));
            assert_eq!(parent_events, event_rows(&direct_tracker), "{family:?}");

            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(Tracker::default());
            let mut legacy_tx = real_route_transaction(&legacy_dice, family, legacy_tracker.clone()).await;
            let legacy_key = HostRootApparentRepositoryRouteKey::new(workspace.clone(), apparent).unwrap();
            let mut legacy = legacy_tx.compute(&legacy_key).await.unwrap();
            if let SourcePreparationOutcome::Need(need) = &legacy {
                let request = need.repository_materializations().values().next().unwrap().clone();
                legacy_tx = materialized_transaction(&legacy_dice, &workspace, request, legacy_tracker).await;
                legacy = legacy_tx.compute(&legacy_key).await.unwrap();
            }
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("{family:?}: legacy route must complete")
            };
            assert_eq!(legacy.as_ref(), carrier.result().as_ref(), "{family:?}");

            observed_tracker.clear();
            let warm = observed_tx.compute(&key).await.unwrap();
            assert!(Arc::ptr_eq(observed_route_value(&warm).result(), carrier.result()));
            let warm = observed_tracker.activations.lock().unwrap();
            assert!(!warm.is_empty());
            assert!(warm.iter().all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none()));
            drop(warm);
            assert!(event_rows(&observed_tracker).is_empty());
        }
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_route_lifecycle_cancellation_and_nonactivation() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key =
            HostRootApparentRepositoryRouteObservationKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let child_key =
            HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent)
                .unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (a, a_child) = observed_route_state(&mut a_tx, &key, &child_key).await;
        let a_result = a.result().clone();
        let a_observations = a.observations().clone();
        let a_child_result = a_child.result().clone();
        let a_child_observations = a_child.observations().clone();

        tracker.clear();
        let (warm, warm_child) = observed_route_state(&mut a_tx, &key, &child_key).await;
        assert!(Arc::ptr_eq(warm.result(), a.result()));
        assert!(Arc::ptr_eq(warm_child.result(), a_child.result()));
        assert_eq!(
            tracker.observed_route.lock().unwrap().as_slice(),
            [ActivationKind::Reused]
        );
        assert_eq!(
            tracker.observed_predecessor.lock().unwrap().as_slice(),
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
        assert!(event_rows(&tracker).is_empty());

        let mapping_b_module = MODULE.replacen(
            "first='first', second='second'",
            "first='second', second='first'",
            1,
        );
        let mut mapping_b_tx = transaction(&dice, &mapping_b_module, EXTENSION_A, true, None).await;
        let (mapping_b, mapping_b_child) =
            observed_route_state(&mut mapping_b_tx, &key, &child_key).await;
        assert_ne!(mapping_b.result(), a.result());
        assert_ne!(mapping_b_child.result(), a_child.result());
        let mut mapping_restore_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let (mapping_restored, mapping_restored_child) =
            observed_route_state(&mut mapping_restore_tx, &key, &child_key).await;
        assert_eq!(mapping_restored.result(), a.result());
        assert_eq!(mapping_restored_child.result(), a_child.result());
        assert!(!Arc::ptr_eq(mapping_restored.result(), a.result()));
        assert!(!Arc::ptr_eq(
            mapping_restored_child.result(),
            a_child.result()
        ));

        let extension_b = EXTENSION_A.replacen("value='one'", "value='changed'", 1);
        let mut definition_b_tx = transaction(&dice, MODULE, &extension_b, true, None).await;
        let (definition_b, definition_b_child) =
            observed_route_state(&mut definition_b_tx, &key, &child_key).await;
        assert_ne!(definition_b.result(), a.result());
        assert_ne!(definition_b_child.result(), a_child.result());
        let mut definition_restore_tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let (definition_restored, definition_restored_child) =
            observed_route_state(&mut definition_restore_tx, &key, &child_key).await;
        assert_eq!(definition_restored.result(), a.result());
        assert_eq!(definition_restored_child.result(), a_child.result());
        assert!(!Arc::ptr_eq(definition_restored.result(), a.result()));
        assert!(!Arc::ptr_eq(
            definition_restored_child.result(),
            a_child.result()
        ));

        let neutral_module = format!("{MODULE}\n");
        let mut neutral_tx = transaction(&dice, &neutral_module, EXTENSION_A, true, None).await;
        let (neutral, neutral_child) =
            observed_route_state(&mut neutral_tx, &key, &child_key).await;
        assert_eq!(neutral.result(), a.result());
        assert_eq!(neutral_child.result(), a_child.result());
        assert_ne!(neutral.observations(), a.observations());
        assert_ne!(neutral_child.observations(), a_child.observations());
        assert!(!Arc::ptr_eq(neutral.result(), a.result()));
        assert!(!Arc::ptr_eq(neutral_child.result(), a_child.result()));
        assert_ne!(neutral, a);
        assert_ne!(neutral_child, a_child);
        assert_eq!(a.result(), &a_result);
        assert_eq!(a.observations(), &a_observations);
        assert_eq!(a_child.result(), &a_child_result);
        assert_eq!(a_child.observations(), &a_child_observations);

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
        assert!(
            cancel_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .all(|(name, _, _)| name != &key.to_string())
        );
        assert!(
            cancel_tracker
                .dependencies
                .lock()
                .unwrap()
                .iter()
                .all(|(name, _)| name != &key.to_string())
        );
        let mut recovery = transaction(
            &cancel_dice,
            MODULE,
            EXTENSION_A,
            true,
            Some(cancel_tracker.clone()),
        )
        .await;
        let (recovered, recovered_child) =
            observed_route_state(&mut recovery, &key, &child_key).await;
        assert_eq!(recovered.result(), a.result());
        assert_eq!(recovered_child.result(), a_child.result());

        let activations = cancel_tracker.activations.lock().unwrap();
        let dependencies = cancel_tracker.dependencies.lock().unwrap();
        for forbidden in [
            "HostRootApparentRepositorySourceInputKey",
            "HostRootApparentRepositorySourcePathInputKey",
            "HostRootApparentRepositorySourceObservationKey",
            "root-repository-route:",
            "repository-package-source:",
            "repository-source-file:",
            "host-repository-source-file:",
            "build-command-root:",
        ] {
            assert!(
                activations
                    .iter()
                    .all(|(name, _, _)| !name.contains(forbidden))
            );
            assert!(
                dependencies
                    .iter()
                    .all(|(name, children)| !name.contains(forbidden)
                        && children.iter().all(|child| !child.contains(forbidden)))
            );
        }
        assert!(activations.iter().all(|(name, _, _)| {
            !name.starts_with("HostRootApparentRepositoryRouteKey")
                && !name.starts_with("host-root-apparent-repository-definition:")
        }));
        assert!(dependencies.iter().all(|(name, children)| {
            !name.starts_with("HostRootApparentRepositoryRouteKey")
                && !name.starts_with("host-root-apparent-repository-definition:")
                && children.iter().all(|child| {
                    !child.starts_with("HostRootApparentRepositoryRouteKey")
                        && !child.starts_with("host-root-apparent-repository-definition:")
                })
        }));
        let producer = include_str!("root_apparent_repository_route.rs");
        let producer = &producer[producer
            .find("struct HostRootApparentRepositoryRouteObservationKey")
            .unwrap()..producer.find("#[cfg(test)]").unwrap()];
        for absent in [
            "SourceInputKey",
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

    fn route(value: &HostRootApparentRepositoryRouteOutcome) -> &HostRootApparentRepositoryRoute {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("route must complete: {value:?}")
        };
        value.as_ref().as_ref().unwrap()
    }

    async fn tracked_route(
        dice: &Arc<Dice>,
        key: &HostRootApparentRepositoryRouteKey,
    ) -> (
        HostRootApparentRepositoryRouteOutcome,
        Arc<DefinitionResult>,
        Arc<Tracker>,
    ) {
        let mut tx = dice.updater().commit().await;
        let predecessor = tx
            .compute(
                &HostRootApparentRepositoryDefinitionKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) = predecessor else {
            panic!("predecessor must complete")
        };
        let tracker = Arc::new(Tracker::default());
        tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        (tx.compute(key).await.unwrap(), predecessor, tracker)
    }

    #[test]
    fn consistency_is_fail_closed() {
        let apparent = ApparentRepoName::new("dep").unwrap();
        let other = ApparentRepoName::new("other").unwrap();
        let root = CanonicalRepoName::root();
        let builtin = CanonicalRepoName::new("bazel_tools").unwrap();
        let dep = CanonicalRepoName::new("dep+").unwrap();
        for (view, expected) in [
            (
                (
                    &apparent,
                    &root,
                    HostRootApparentRepositoryRouteKind::Main,
                    None,
                    None,
                ),
                true,
            ),
            (
                (
                    &apparent,
                    &builtin,
                    HostRootApparentRepositoryRouteKind::Builtin,
                    None,
                    None,
                ),
                true,
            ),
            (
                (
                    &other,
                    &root,
                    HostRootApparentRepositoryRouteKind::Main,
                    None,
                    None,
                ),
                false,
            ),
            (
                (
                    &apparent,
                    &builtin,
                    HostRootApparentRepositoryRouteKind::Main,
                    None,
                    None,
                ),
                false,
            ),
            (
                (
                    &apparent,
                    &root,
                    HostRootApparentRepositoryRouteKind::Builtin,
                    None,
                    None,
                ),
                false,
            ),
            (
                (
                    &apparent,
                    &dep,
                    HostRootApparentRepositoryRouteKind::Generated,
                    None,
                    None,
                ),
                false,
            ),
        ] {
            assert_eq!(
                view_is_consistent(
                    &apparent,
                    HostRootApparentRepositoryRouteView {
                        apparent_repo: view.0,
                        canonical_repo: view.1,
                        kind: view.2,
                        repo_spec: view.3,
                        local_path_policy: view.4,
                    },
                ),
                expected,
            );
        }
        for (success, deferred, view, expected) in [
            (true, false, true, Some(true)),
            (false, true, true, Some(true)),
            (false, false, false, None),
            (true, false, false, Some(false)),
            (false, true, false, Some(false)),
        ] {
            assert_eq!(completed_disposition(success, deferred, view), expected);
        }
    }

    #[tokio::test]
    async fn generated_route_borrows_original_definition() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let canonical = names(&validated(&mut tx).await)[0].clone();
        let key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let predecessor_key = HostRootApparentRepositoryDefinitionKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        tx.compute(&predecessor_key).await.unwrap();
        let tracker = Arc::new(Tracker::default());
        tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let a = tx.compute(&key).await.unwrap();
        assert_eq!(
            *tracker.predecessor.lock().unwrap(),
            [ActivationKind::Reused]
        );
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Evaluated]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        let mut warm_tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        warm_tx.compute(&key).await.unwrap();
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Reused]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        let certificate = route(&a);
        let view = certificate.view().unwrap();
        assert_eq!(view.apparent_repo().as_str(), "first");
        assert_eq!(view.canonical_repo(), &canonical);
        assert_eq!(view.kind(), HostRootApparentRepositoryRouteKind::Generated);
        let predecessor = certificate.predecessor.as_ref().as_ref().unwrap();
        assert!(std::ptr::eq(
            view.repo_spec().unwrap(),
            predecessor.view().unwrap().repo_spec().unwrap(),
        ));
        let HostRootApparentRepositorySourceDisposition::Capability(capability) =
            certificate.source_capability().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(capability.workspace(), &workspace);
        assert_eq!(capability.apparent_repo().as_str(), "first");
        assert_eq!(capability.canonical_repo(), &canonical);
        let HostRepositorySourceCapabilitySource::RepoSpec {
            repo_spec: spec,
            local_path_policy,
        } = capability.source()
        else {
            unreachable!()
        };
        assert_eq!(spec.as_ref(), view.repo_spec().unwrap());
        assert_eq!(
            *local_path_policy,
            HostRepositoryLocalPathPolicy::LocalUnsupported
        );
        let cloned = capability.clone();
        let HostRepositorySourceCapabilitySource::RepoSpec {
            repo_spec: cloned_spec,
            ..
        } = cloned.source()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(spec, cloned_spec));
        let repeated = certificate.source_capability().unwrap();
        assert_eq!(
            repeated,
            HostRootApparentRepositorySourceDisposition::Capability(capability)
        );
        let apparent_builtin = ApparentRepoName::new("bazel_tools").unwrap();
        assert!(matches!(
            source_capability_from_view(
                &workspace,
                HostRootApparentRepositoryRouteView {
                    apparent_repo: &apparent_builtin,
                    canonical_repo: view.canonical_repo(),
                    kind: view.kind(),
                    repo_spec: view.repo_spec(),
                    local_path_policy: view.local_path_policy(),
                },
            ),
            Some(HostRootApparentRepositorySourceDisposition::Capability(_))
        ));
        let root = CanonicalRepoName::root();
        let builtin = CanonicalRepoName::new("bazel_tools").unwrap();
        for kind in [
            HostRootApparentRepositoryRouteKind::Main,
            HostRootApparentRepositoryRouteKind::Builtin,
            HostRootApparentRepositoryRouteKind::SelectedRegistry,
            HostRootApparentRepositoryRouteKind::SelectedNonregistry,
            HostRootApparentRepositoryRouteKind::Generated,
        ] {
            for canonical_repo in [&root, &builtin, view.canonical_repo()] {
                for repo_spec in [None, view.repo_spec()] {
                    for local_path_policy in [
                        None,
                        Some(HostRepositoryLocalPathPolicy::WorkspaceRelative),
                        Some(HostRepositoryLocalPathPolicy::CommandAbsolute),
                        Some(HostRepositoryLocalPathPolicy::LocalUnsupported),
                    ] {
                        let apparent_repo = if kind == HostRootApparentRepositoryRouteKind::Builtin
                        {
                            &apparent_builtin
                        } else {
                            view.apparent_repo()
                        };
                        let expected = match kind {
                            HostRootApparentRepositoryRouteKind::Main => {
                                canonical_repo.is_root()
                                    && repo_spec.is_none()
                                    && local_path_policy.is_none()
                            }
                            HostRootApparentRepositoryRouteKind::Builtin => {
                                canonical_repo.as_str() == "bazel_tools"
                                    && repo_spec.is_none()
                                    && local_path_policy.is_none()
                            }
                            HostRootApparentRepositoryRouteKind::SelectedRegistry
                            | HostRootApparentRepositoryRouteKind::Generated => {
                                canonical_repo == view.canonical_repo()
                                    && repo_spec.is_some()
                                    && local_path_policy
                                        == Some(HostRepositoryLocalPathPolicy::LocalUnsupported)
                            }
                            HostRootApparentRepositoryRouteKind::SelectedNonregistry => {
                                canonical_repo == view.canonical_repo()
                                    && repo_spec.is_some()
                                    && matches!(
                                        local_path_policy,
                                        Some(
                                            HostRepositoryLocalPathPolicy::WorkspaceRelative
                                                | HostRepositoryLocalPathPolicy::CommandAbsolute
                                        )
                                    )
                            }
                        };
                        assert_eq!(
                            source_capability_from_view(
                                &workspace,
                                HostRootApparentRepositoryRouteView {
                                    apparent_repo,
                                    canonical_repo,
                                    kind,
                                    repo_spec,
                                    local_path_policy,
                                },
                            )
                            .is_some(),
                            expected,
                        );
                    }
                }
            }
        }
        let corrupt_request = HostRootApparentRepositoryRoute {
            workspace: workspace.clone(),
            apparent_repo: ApparentRepoName::new("other").unwrap(),
            predecessor: certificate.predecessor.clone(),
        };
        assert!(corrupt_request.source_capability().is_none());
        let invalid = invalid_predecessor(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
            certificate.predecessor.clone(),
        );
        assert_eq!(invalid.workspace, workspace);
        assert_eq!(invalid.apparent_repo.as_str(), "first");
        let HostRootApparentRepositoryRouteErrorKind::InvalidPredecessor(retained) = invalid.kind
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(&retained, &certificate.predecessor));
        tracker.clear();
        let warm = tx.compute(&key).await.unwrap();
        assert!(HostRootApparentRepositoryRouteKey::equality(&a, &warm));
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Reused]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
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
            let changed = transaction(&dice, &module, extension, true, None)
                .await
                .compute(&key)
                .await
                .unwrap();
            assert!(!HostRootApparentRepositoryRouteKey::equality(&a, &changed));
            let restored = transaction(&dice, MODULE, EXTENSION_A, true, None)
                .await
                .compute(&key)
                .await
                .unwrap();
            assert!(HostRootApparentRepositoryRouteKey::equality(&a, &restored));
        }
    }

    #[tokio::test]
    async fn selected_nonregistry_route_retains_original_spec() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("local_alias").unwrap(),
        )
        .unwrap();
        let mut tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let mut outcome = tx.compute(&key).await.unwrap();
        assert!(!HostRootApparentRepositoryRouteKey::validity(&outcome));
        assert!(!HostRootApparentRepositoryRouteKey::equality(
            &outcome, &outcome
        ));
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
            outcome = tx.compute(&key).await.unwrap();
        }
        let certificate = route(&outcome);
        let view = certificate.view().unwrap();
        assert_eq!(
            view.kind(),
            HostRootApparentRepositoryRouteKind::SelectedNonregistry
        );
        assert_eq!(view.canonical_repo().as_str(), "local+");
        assert_eq!(
            view.repo_spec().unwrap().rule_id.rule_name.as_str(),
            "local_repository",
        );
        let predecessor = certificate.predecessor.as_ref().as_ref().unwrap();
        assert!(std::ptr::eq(
            view.repo_spec().unwrap(),
            predecessor.view().unwrap().repo_spec().unwrap(),
        ));
        let HostRootApparentRepositorySourceDisposition::Capability(capability) =
            certificate.source_capability().unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            capability.source(),
            HostRepositorySourceCapabilitySource::RepoSpec { repo_spec, local_path_policy }
                if repo_spec.as_ref() == view.repo_spec().unwrap()
                    && *local_path_policy == HostRepositoryLocalPathPolicy::WorkspaceRelative
        ));

        let command_module = "module(name='bazel_tools')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let mut command_tx =
            transaction_with_command_override(&dice, command_module, EXTENSION_A, "local").await;
        let command_key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("local_alias").unwrap(),
        )
        .unwrap();
        let mut command = command_tx.compute(&command_key).await.unwrap();
        let mut command_requests = Vec::<Arc<RepositoryMaterializationRequest>>::new();
        while let SourcePreparationOutcome::Need(need) = &command {
            for request in need.repository_materializations().values() {
                if !command_requests.iter().any(|seen| seen.id == request.id) {
                    command_requests.push(request.clone());
                }
            }
            let entries = command_requests.iter().cloned().map(|request| {
                RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                }
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
            command_tx = updater.commit().await;
            command = command_tx.compute(&command_key).await.unwrap();
        }
        let command = route(&command);
        let command_view = command.view().unwrap();
        assert_eq!(
            command_view.kind(),
            HostRootApparentRepositoryRouteKind::SelectedNonregistry
        );
        assert_eq!(
            command_view.local_path_policy(),
            Some(HostRepositoryLocalPathPolicy::CommandAbsolute)
        );
        let HostRootApparentRepositorySourceDisposition::Capability(command_capability) =
            command.source_capability().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            command_capability.local_path_policy(),
            Some(HostRepositoryLocalPathPolicy::CommandAbsolute)
        );
    }

    #[tokio::test]
    async fn main_deferred_is_promoted_without_fallback() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let _tx = transaction(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("root_self").unwrap(),
        )
        .unwrap();
        let (outcome, predecessor, tracker) = tracked_route(&dice, &key).await;
        let certificate = route(&outcome);
        assert!(Arc::ptr_eq(&certificate.predecessor, &predecessor));
        let view = certificate.view().unwrap();
        assert_eq!(view.kind(), HostRootApparentRepositoryRouteKind::Main);
        assert!(view.canonical_repo().is_root());
        assert!(view.repo_spec().is_none());
        assert_eq!(view.local_path_policy(), None);
        assert_eq!(
            certificate.source_capability(),
            Some(HostRootApparentRepositorySourceDisposition::Main)
        );
        assert_eq!(
            *tracker.predecessor.lock().unwrap(),
            [ActivationKind::Reused]
        );
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Evaluated]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        let mut warm_tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        warm_tx.compute(&key).await.unwrap();
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Reused]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());

        prepare_builtin(&dice, &workspace).await;
        let builtin_key = HostRootApparentRepositoryRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        )
        .unwrap();
        let (builtin, predecessor, tracker) = tracked_route(&dice, &builtin_key).await;
        let certificate = route(&builtin);
        assert!(Arc::ptr_eq(&certificate.predecessor, &predecessor));
        let builtin = certificate.view().unwrap();
        assert_eq!(builtin.kind(), HostRootApparentRepositoryRouteKind::Builtin);
        assert_eq!(builtin.canonical_repo().as_str(), "bazel_tools");
        assert!(builtin.repo_spec().is_none());
        assert_eq!(builtin.local_path_policy(), None);
        let HostRootApparentRepositorySourceDisposition::Capability(capability) =
            certificate.source_capability().unwrap()
        else {
            unreachable!()
        };
        assert!(matches!(
            capability.source(),
            HostRepositorySourceCapabilitySource::Builtin(identity)
                if identity == &BuiltinBazelToolsSnapshot::CURRENT.route_identity()
        ));
        assert_eq!(capability.local_path_policy(), None);
        assert_eq!(
            *tracker.predecessor.lock().unwrap(),
            [ActivationKind::Reused]
        );
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Evaluated]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        let mut warm_tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        warm_tx.compute(&builtin_key).await.unwrap();
        assert_eq!(*tracker.route.lock().unwrap(), [ActivationKind::Reused]);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());

        let missing_key = HostRootApparentRepositoryRouteKey::new(
            workspace,
            ApparentRepoName::new("absent").unwrap(),
        )
        .unwrap();
        let missing = dice
            .updater()
            .commit()
            .await
            .compute(&missing_key)
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(missing) = missing else {
            panic!("missing route must complete")
        };
        let error = missing.as_ref().as_ref().unwrap_err();
        assert_eq!(
            error.workspace,
            NormalizedAbsolutePath::new(WORKSPACE).unwrap()
        );
        assert_eq!(error.apparent_repo.as_str(), "absent");
        let HostRootApparentRepositoryRouteErrorKind::Predecessor(predecessor) = &error.kind else {
            panic!("ordinary predecessor error must remain opaque")
        };
        assert!(predecessor.as_ref().is_err());
    }
}
