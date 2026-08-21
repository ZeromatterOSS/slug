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
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::HostRepositorySourceInputError;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_source_input;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::root_apparent_repository_route::HostRootApparentRepositoryRouteKey;
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
fn complete(
    value: Result<
        HostRootApparentRepositorySourceInput,
        HostRootApparentRepositorySourceInputError,
    >,
) -> HostRootApparentRepositorySourceInputOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostRootApparentRepositorySourceInputKey {
    type Value = HostRootApparentRepositorySourceInputOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let predecessor = match ctx
            .compute(
                &HostRootApparentRepositoryRouteKey::new(
                    self.workspace.clone(),
                    self.apparent_repo.clone(),
                )
                .expect("source-input key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(predecessor)) => predecessor,
            Err(error) => {
                return complete(Err(HostRootApparentRepositorySourceInputError {
                    workspace: self.workspace.clone(),
                    apparent_repo: self.apparent_repo.clone(),
                    kind: HostRootApparentRepositorySourceInputErrorKind::Compute(
                        error.to_string().into(),
                    ),
                }));
            }
        };
        let terminal = |kind| {
            complete(Err(HostRootApparentRepositorySourceInputError {
                workspace: self.workspace.clone(),
                apparent_repo: self.apparent_repo.clone(),
                kind,
            }))
        };
        let source = predecessor
            .as_ref()
            .as_ref()
            .ok()
            .and_then(|route| route.source_capability());
        match completed_route_disposition(predecessor.is_ok(), source.is_some()) {
            CompletedRouteDisposition::Route => {
                return terminal(HostRootApparentRepositorySourceInputErrorKind::Route(
                    predecessor,
                ));
            }
            CompletedRouteDisposition::InvalidRoute => {
                return terminal(
                    HostRootApparentRepositorySourceInputErrorKind::InvalidRoute(predecessor),
                );
            }
            CompletedRouteDisposition::Source => {}
        };
        let disposition = match source.expect("completed disposition checked source presence") {
            HostRootApparentRepositorySourceDisposition::Main => {
                HostRootApparentRepositorySourceInputDisposition::Main
            }
            HostRootApparentRepositorySourceDisposition::Capability(capability) => {
                match host_repository_source_input(capability) {
                    Ok(input) => HostRootApparentRepositorySourceInputDisposition::Input(input),
                    Err(error) => {
                        return terminal(
                            HostRootApparentRepositorySourceInputErrorKind::Projection {
                                predecessor,
                                error,
                            },
                        );
                    }
                }
            }
        };
        let certificate = HostRootApparentRepositorySourceInput {
            workspace: self.workspace.clone(),
            apparent_repo: self.apparent_repo.clone(),
            predecessor: predecessor.clone(),
            disposition,
        };
        if certificate.view().is_none() {
            return terminal(
                HostRootApparentRepositorySourceInputErrorKind::InvalidRoute(predecessor),
            );
        }
        complete(Ok(certificate))
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
        events: Mutex<usize>,
        forbidden: Mutex<Vec<&'static str>>,
    }
    impl ActivationTracker for Tracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }
        fn tracks_rich_activations(&self) -> bool {
            true
        }
        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let owner = key
                .downcast_ref::<HostRootApparentRepositorySourceInputKey>()
                .is_some();
            let route = key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some();
            if owner || route {
                self.order
                    .lock()
                    .unwrap()
                    .push((if owner { "owner" } else { "route" }, activation.kind()));
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
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
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
    #[test]
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
    async fn real_main_builtin_and_generated_projection_are_owned_once() {
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
    async fn selected_need_policy_and_route_terminal_are_retained() {
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

    #[test]
    fn production_edge_is_only_route_then_pure_projection() {
        let source = include_str!("root_apparent_repository_source_input.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches(".compute(").count(), 1);
        assert!(production.contains("HostRootApparentRepositoryRouteKey::new"));
        assert!(production.contains("host_repository_source_input(capability)"));
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
    }
}
