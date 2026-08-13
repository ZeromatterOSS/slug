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
use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
use slug_bzlmod_v2::HostRepositorySourceCapability;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::root_apparent_repository_definition::HostRootApparentRepositoryDeferredKind;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinition;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionError;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKey;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKind;

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
struct HostRootApparentRepositoryRouteView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    kind: HostRootApparentRepositoryRouteKind,
    repo_spec: Option<&'a RepoSpec>,
}
impl<'a> HostRootApparentRepositoryRouteView<'a> {
    fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }
    fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }
    fn kind(self) -> HostRootApparentRepositoryRouteKind {
        self.kind
    }
    fn repo_spec(self) -> Option<&'a RepoSpec> {
        self.repo_spec
    }
}
fn predecessor_view(
    predecessor: &DefinitionResult,
) -> Option<HostRootApparentRepositoryRouteView<'_>> {
    let (apparent_repo, canonical_repo, kind, repo_spec) = match predecessor {
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
            (view.apparent_repo(), view.canonical_repo(), kind, None)
        }
    };
    Some(HostRootApparentRepositoryRouteView {
        apparent_repo,
        canonical_repo,
        kind,
        repo_spec,
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
            view.canonical_repo.is_root() && view.repo_spec.is_none()
        }
        HostRootApparentRepositoryRouteKind::Builtin => {
            view.canonical_repo.as_str() == "bazel_tools" && view.repo_spec.is_none()
        }
        HostRootApparentRepositoryRouteKind::SelectedRegistry
        | HostRootApparentRepositoryRouteKind::SelectedNonregistry
        | HostRootApparentRepositoryRouteKind::Generated => {
            !view.canonical_repo.is_root()
                && view.canonical_repo.as_str() != "bazel_tools"
                && view.repo_spec.is_some()
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
struct HostRootApparentRepositoryRoute {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    predecessor: Arc<DefinitionResult>,
}
impl HostRootApparentRepositoryRoute {
    fn view(&self) -> Option<HostRootApparentRepositoryRouteView<'_>> {
        let view = predecessor_view(self.predecessor.as_ref())?;
        view_is_consistent(&self.apparent_repo, view).then_some(view)
    }

    fn source_capability(&self) -> Option<HostRootApparentRepositorySourceDisposition> {
        source_capability_from_view(&self.workspace, self.view()?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourceDisposition {
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
struct HostRootApparentRepositoryRouteError {
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

type HostRootApparentRepositoryRouteOutcome = SourcePreparationOutcome<
    Arc<Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostRootApparentRepositoryRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}

impl HostRootApparentRepositoryRouteKey {
    fn new(workspace: NormalizedAbsolutePath, apparent_repo: ApparentRepoName) -> Option<Self> {
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

fn complete(
    value: Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>,
) -> HostRootApparentRepositoryRouteOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostRootApparentRepositoryRouteKey {
    type Value = HostRootApparentRepositoryRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let predecessor = match ctx
            .compute(
                &HostRootApparentRepositoryDefinitionKey::new(
                    self.workspace.clone(),
                    self.apparent_repo.clone(),
                )
                .expect("route key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => value,
            Err(error) => {
                return complete(Err(HostRootApparentRepositoryRouteError {
                    workspace: self.workspace.clone(),
                    apparent_repo: self.apparent_repo.clone(),
                    kind: HostRootApparentRepositoryRouteErrorKind::Compute(
                        error.to_string().into(),
                    ),
                }));
            }
        };
        let terminal = |kind| {
            complete(Err(HostRootApparentRepositoryRouteError {
                workspace: self.workspace.clone(),
                apparent_repo: self.apparent_repo.clone(),
                kind,
            }))
        };
        let view = predecessor_view(predecessor.as_ref());
        let is_success = predecessor.is_ok();
        let is_deferred = matches!(predecessor.as_ref(), Err(error) if error.is_deferred());
        match completed_disposition(is_success, is_deferred, view.is_some()) {
            Some(true) => {}
            None => {
                return terminal(HostRootApparentRepositoryRouteErrorKind::Predecessor(
                    predecessor,
                ));
            }
            Some(false) => {
                return complete(Err(invalid_predecessor(
                    self.workspace.clone(),
                    self.apparent_repo.clone(),
                    predecessor,
                )));
            }
        }
        let view = view.expect("success disposition has a view");
        if !view_is_consistent(&self.apparent_repo, view) {
            return complete(Err(invalid_predecessor(
                self.workspace.clone(),
                self.apparent_repo.clone(),
                predecessor,
            )));
        }
        complete(Ok(HostRootApparentRepositoryRoute {
            workspace: self.workspace.clone(),
            apparent_repo: self.apparent_repo.clone(),
            predecessor,
        }))
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
    use slug_loading_v2::RepositoryPackageLoadKey;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::names;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::validated;
    use super::super::root_apparent_repository_definition::tests::prepare_builtin;
    use super::*;

    #[derive(Default)]
    struct Tracker {
        route: Mutex<Vec<ActivationKind>>,
        predecessor: Mutex<Vec<ActivationKind>>,
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
            if key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some()
            {
                self.route.lock().unwrap().push(activation.kind());
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            } else if key
                .downcast_ref::<HostRootApparentRepositoryDefinitionKey>()
                .is_some()
            {
                self.predecessor.lock().unwrap().push(activation.kind());
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
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
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
                ),
                true,
            ),
            (
                (
                    &apparent,
                    &builtin,
                    HostRootApparentRepositoryRouteKind::Builtin,
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
                ),
                false,
            ),
            (
                (
                    &apparent,
                    &builtin,
                    HostRootApparentRepositoryRouteKind::Main,
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
                ),
                false,
            ),
            (
                (
                    &apparent,
                    &dep,
                    HostRootApparentRepositoryRouteKind::Generated,
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
        let HostRepositorySourceCapabilitySource::RepoSpec(spec) = capability.source() else {
            unreachable!()
        };
        assert_eq!(spec.as_ref(), view.repo_spec().unwrap());
        let cloned = capability.clone();
        let HostRepositorySourceCapabilitySource::RepoSpec(cloned_spec) = cloned.source() else {
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
                    let apparent_repo = if kind == HostRootApparentRepositoryRouteKind::Builtin {
                        &apparent_builtin
                    } else {
                        view.apparent_repo()
                    };
                    let expected = match kind {
                        HostRootApparentRepositoryRouteKind::Main => {
                            canonical_repo.is_root() && repo_spec.is_none()
                        }
                        HostRootApparentRepositoryRouteKind::Builtin => {
                            canonical_repo.as_str() == "bazel_tools" && repo_spec.is_none()
                        }
                        HostRootApparentRepositoryRouteKind::SelectedRegistry
                        | HostRootApparentRepositoryRouteKind::SelectedNonregistry
                        | HostRootApparentRepositoryRouteKind::Generated => {
                            canonical_repo == view.canonical_repo() && repo_spec.is_some()
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
                            },
                        )
                        .is_some(),
                        expected,
                    );
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
            HostRepositorySourceCapabilitySource::RepoSpec(spec)
                if spec.as_ref() == view.repo_spec().unwrap()
        ));
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
