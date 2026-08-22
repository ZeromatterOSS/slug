/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositoryRelativePath;
use slug_bzlmod_v2::HostRepositoryRelativePathError;
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_relative_path;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputDispositionView;
use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputKey;
use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputObservationError;
use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputObservationKey;
use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputResult;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourcePathInput {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    predecessor: Arc<HostRootApparentRepositorySourceInputResult>,
    relative_path: HostRepositoryRelativePath,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HostRootApparentRepositorySourcePathInputDispositionView<'a> {
    Main,
    Input(&'a HostRepositorySourceInput),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HostRootApparentRepositorySourcePathInputView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    relative_path: &'a HostRepositoryRelativePath,
    disposition: HostRootApparentRepositorySourcePathInputDispositionView<'a>,
}

impl HostRootApparentRepositorySourcePathInput {
    pub(super) fn view(&self) -> Option<HostRootApparentRepositorySourcePathInputView<'_>> {
        let source = self.predecessor.as_ref().as_ref().ok()?;
        let source_view = source.view()?;
        if source.workspace() != &self.workspace
            || source_view.apparent_repo() != &self.apparent_repo
        {
            return None;
        }
        let disposition = match source_view.disposition() {
            HostRootApparentRepositorySourceInputDispositionView::Main => {
                HostRootApparentRepositorySourcePathInputDispositionView::Main
            }
            HostRootApparentRepositorySourceInputDispositionView::Input(input) => {
                HostRootApparentRepositorySourcePathInputDispositionView::Input(input)
            }
        };
        Some(HostRootApparentRepositorySourcePathInputView {
            apparent_repo: source_view.apparent_repo(),
            canonical_repo: source_view.canonical_repo(),
            relative_path: &self.relative_path,
            disposition,
        })
    }
}

impl<'a> HostRootApparentRepositorySourcePathInputView<'a> {
    pub(super) fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }

    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }

    pub(super) fn relative_path(self) -> &'a HostRepositoryRelativePath {
        self.relative_path
    }

    pub(super) fn disposition(
        self,
    ) -> HostRootApparentRepositorySourcePathInputDispositionView<'a> {
        self.disposition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourcePathInputErrorKind {
    Path(HostRepositoryRelativePathError),
    Source {
        relative_path: HostRepositoryRelativePath,
        predecessor: Arc<HostRootApparentRepositorySourceInputResult>,
    },
    InvalidSource {
        relative_path: HostRepositoryRelativePath,
        predecessor: Arc<HostRootApparentRepositorySourceInputResult>,
    },
    Compute {
        relative_path: HostRepositoryRelativePath,
        message: Arc<str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourcePathInputError {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    kind: HostRootApparentRepositorySourcePathInputErrorKind,
}

impl fmt::Display for HostRootApparentRepositorySourcePathInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostRootApparentRepositorySourcePathInputError {}

pub(super) type HostRootApparentRepositorySourcePathInputResult = Result<
    HostRootApparentRepositorySourcePathInput,
    HostRootApparentRepositorySourcePathInputError,
>;
pub(super) type HostRootApparentRepositorySourcePathInputOutcome =
    SourcePreparationOutcome<Arc<HostRootApparentRepositorySourcePathInputResult>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositorySourcePathInputKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    requested_path: PathBuf,
}

impl HostRootApparentRepositorySourcePathInputKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        requested_path: PathBuf,
    ) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self {
            workspace,
            apparent_repo,
            requested_path,
        })
    }
}

impl fmt::Display for HostRootApparentRepositorySourcePathInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostRootApparentRepositorySourcePathInputObservationKey(
    HostRootApparentRepositorySourcePathInputKey,
);
impl HostRootApparentRepositorySourcePathInputObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        requested_path: PathBuf,
    ) -> Option<Self> {
        HostRootApparentRepositorySourcePathInputKey::new(workspace, apparent_repo, requested_path)
            .map(Self)
    }
}
impl fmt::Display for HostRootApparentRepositorySourcePathInputObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedHostRootApparentRepositorySourcePathInput {
    result: Arc<HostRootApparentRepositorySourcePathInputResult>,
    observations: PathObservationEpoch,
}
impl ObservedHostRootApparentRepositorySourcePathInput {
    pub(super) fn result(&self) -> &Arc<HostRootApparentRepositorySourcePathInputResult> {
        &self.result
    }
    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootApparentRepositorySourcePathInputObservationError {
    Source(HostRootApparentRepositorySourceInputObservationError),
}
impl Dupe for RootApparentRepositorySourcePathInputObservationError {}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositorySourcePathInputObservationError(
    RootApparentRepositorySourcePathInputObservationError,
);
impl Dupe for HostRootApparentRepositorySourcePathInputObservationError {}
enum RootApparentRepositorySourcePathInputMode {
    Legacy,
    Observed,
}
type RootApparentRepositorySourcePathInputDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<HostRootApparentRepositorySourcePathInputResult>,
            PathObservationEpoch,
        ),
        RootApparentRepositorySourcePathInputObservationError,
    >,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletedSourceDisposition {
    Source,
    InvalidSource,
    Success,
}

fn completed_source_disposition(
    predecessor_is_success: bool,
    has_view: bool,
) -> CompletedSourceDisposition {
    match (predecessor_is_success, has_view) {
        (false, _) => CompletedSourceDisposition::Source,
        (true, false) => CompletedSourceDisposition::InvalidSource,
        (true, true) => CompletedSourceDisposition::Success,
    }
}

fn completed_source_error(
    workspace: &NormalizedAbsolutePath,
    apparent_repo: &ApparentRepoName,
    relative_path: HostRepositoryRelativePath,
    predecessor: Arc<HostRootApparentRepositorySourceInputResult>,
    disposition: CompletedSourceDisposition,
) -> HostRootApparentRepositorySourcePathInputError {
    let kind = match disposition {
        CompletedSourceDisposition::Source => {
            HostRootApparentRepositorySourcePathInputErrorKind::Source {
                relative_path,
                predecessor,
            }
        }
        CompletedSourceDisposition::InvalidSource => {
            HostRootApparentRepositorySourcePathInputErrorKind::InvalidSource {
                relative_path,
                predecessor,
            }
        }
        CompletedSourceDisposition::Success => unreachable!("success is not a terminal"),
    };
    HostRootApparentRepositorySourcePathInputError {
        workspace: workspace.clone(),
        apparent_repo: apparent_repo.clone(),
        kind,
    }
}

fn complete_source_path_error(
    key: &HostRootApparentRepositorySourcePathInputKey,
    kind: HostRootApparentRepositorySourcePathInputErrorKind,
) -> RootApparentRepositorySourcePathInputDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(Err(HostRootApparentRepositorySourcePathInputError {
            workspace: key.workspace.clone(),
            apparent_repo: key.apparent_repo.clone(),
            kind,
        })),
        PathObservationEpoch::empty(),
    )))
}

fn finish_root_apparent_repository_source_path_input(
    key: &HostRootApparentRepositorySourcePathInputKey,
    relative_path: HostRepositoryRelativePath,
    predecessor: Arc<HostRootApparentRepositorySourceInputResult>,
    observations: PathObservationEpoch,
) -> (
    Arc<HostRootApparentRepositorySourcePathInputResult>,
    PathObservationEpoch,
) {
    let source_view = predecessor
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|source| source.view());
    let disposition = completed_source_disposition(predecessor.is_ok(), source_view.is_some());
    if disposition != CompletedSourceDisposition::Success {
        return (
            Arc::new(Err(completed_source_error(
                &key.workspace,
                &key.apparent_repo,
                relative_path,
                predecessor,
                disposition,
            ))),
            observations,
        );
    }
    let certificate = HostRootApparentRepositorySourcePathInput {
        workspace: key.workspace.clone(),
        apparent_repo: key.apparent_repo.clone(),
        predecessor: predecessor.clone(),
        relative_path,
    };
    let result = if certificate.view().is_none() {
        Err(completed_source_error(
            &key.workspace,
            &key.apparent_repo,
            certificate.relative_path,
            predecessor,
            CompletedSourceDisposition::InvalidSource,
        ))
    } else {
        Ok(certificate)
    };
    (Arc::new(result), observations)
}

async fn compute_root_apparent_repository_source_path_input(
    key: &HostRootApparentRepositorySourcePathInputKey,
    mode: RootApparentRepositorySourcePathInputMode,
    ctx: &mut DiceComputations<'_>,
) -> RootApparentRepositorySourcePathInputDriverOutcome {
    let relative_path = match host_repository_relative_path(key.requested_path.clone()) {
        Ok(relative_path) => relative_path,
        Err(error) => {
            return complete_source_path_error(
                key,
                HostRootApparentRepositorySourcePathInputErrorKind::Path(error),
            );
        }
    };
    let (predecessor, observations) = match mode {
        RootApparentRepositorySourcePathInputMode::Legacy => match ctx
            .compute(
                &HostRootApparentRepositorySourceInputKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("source-path key rejects root apparent names"),
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
                return complete_source_path_error(
                    key,
                    HostRootApparentRepositorySourcePathInputErrorKind::Compute {
                        relative_path,
                        message: error.to_string().into(),
                    },
                );
            }
        },
        RootApparentRepositorySourcePathInputMode::Observed => match ctx
            .compute(
                &HostRootApparentRepositorySourceInputObservationKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                )
                .expect("source-path key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    RootApparentRepositorySourcePathInputObservationError::Source(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().clone())
            }
            Err(error) => {
                return complete_source_path_error(
                    key,
                    HostRootApparentRepositorySourcePathInputErrorKind::Compute {
                        relative_path,
                        message: error.to_string().into(),
                    },
                );
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok(finish_root_apparent_repository_source_path_input(
        key,
        relative_path,
        predecessor,
        observations,
    )))
}

#[async_trait]
impl Key for HostRootApparentRepositorySourcePathInputKey {
    type Value = HostRootApparentRepositorySourcePathInputOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_path_input(
            self,
            RootApparentRepositorySourcePathInputMode::Legacy,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy source path has no observation outer")
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
impl Key for HostRootApparentRepositorySourcePathInputObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostRootApparentRepositorySourcePathInput,
            HostRootApparentRepositorySourcePathInputObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_path_input(
            &self.0,
            RootApparentRepositorySourcePathInputMode::Observed,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(
                    HostRootApparentRepositorySourcePathInputObservationError(error),
                ))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostRootApparentRepositorySourcePathInput {
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
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::Path;
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
    use slug_bzlmod_v2::RepositoryMaterializationKey;
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
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRouteKey;
    use super::super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInput;
    use super::super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputError;
    use super::super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputObservationError;
    use super::super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputObservationKey;
    use super::super::root_apparent_repository_source_input::ObservedHostRootApparentRepositorySourceInput;
    use super::super::root_apparent_repository_source_input::tests::complete_local;
    use super::super::root_apparent_repository_source_input::tests::corrupt_workspace;
    use super::super::root_apparent_repository_source_input::tests::value as source_value;
    use super::*;

    type ObservedContext<'a> = (
        &'a Arc<Arc<Dice>>,
        &'a NormalizedAbsolutePath,
        &'a HostRootApparentRepositorySourcePathInputObservationKey,
        &'a HostRootApparentRepositorySourceInputObservationKey,
    );
    type ObservedPath = ObservedHostRootApparentRepositorySourcePathInput;
    type ObservedSource = ObservedHostRootApparentRepositorySourceInput;
    type ObservedState = (dice::DiceTransaction, ObservedPath, ObservedSource);

    #[test]
    fn root_apparent_repository_source_input_observation_surface_is_sibling_usable() {
        let key = HostRootApparentRepositorySourceInputObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\") }"
        );

        fn inspect(
            _: &<HostRootApparentRepositorySourceInputObservationKey as Key>::Value,
            observed: &ObservedHostRootApparentRepositorySourceInput,
            _: &HostRootApparentRepositorySourceInputObservationError,
        ) {
            let _: &Arc<
                Result<
                    HostRootApparentRepositorySourceInput,
                    HostRootApparentRepositorySourceInputError,
                >,
            > = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &SourcePreparationOutcome<
                    Result<
                        ObservedHostRootApparentRepositorySourceInput,
                        HostRootApparentRepositorySourceInputObservationError,
                    >,
                >,
                &ObservedHostRootApparentRepositorySourceInput,
                &HostRootApparentRepositorySourceInputObservationError,
            );
    }

    #[derive(Default)]
    struct Tracker {
        order: Mutex<Vec<(&'static str, ActivationKind)>>,
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
            let path = key
                .downcast_ref::<HostRootApparentRepositorySourcePathInputKey>()
                .is_some();
            let source = key
                .downcast_ref::<HostRootApparentRepositorySourceInputKey>()
                .is_some();
            let route = key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some();
            let observed = key
                .downcast_ref::<HostRootApparentRepositorySourcePathInputObservationKey>()
                .is_some()
                || key
                    .downcast_ref::<HostRootApparentRepositorySourceInputObservationKey>()
                    .is_some();
            if !observed && (path || source || route) {
                self.order.lock().unwrap().push((
                    if path {
                        "path"
                    } else if source {
                        "source"
                    } else {
                        "route"
                    },
                    kind,
                ));
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
                self.forbidden.lock().unwrap().push("source-file");
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                self.forbidden.lock().unwrap().push("filesystem");
            }
        }
    }

    impl Tracker {
        fn clear(&self) {
            self.order.lock().unwrap().clear();
            self.activations.lock().unwrap().clear();
            self.dependencies.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
            self.forbidden.lock().unwrap().clear();
        }
    }

    fn value(
        outcome: &HostRootApparentRepositorySourcePathInputOutcome,
    ) -> &HostRootApparentRepositorySourcePathInput {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("source path must complete: {outcome:?}")
        };
        value.as_ref().as_ref().unwrap()
    }

    async fn tracked_complete(
        dice: &Arc<Dice>,
        key: &HostRootApparentRepositorySourcePathInputKey,
        source_key: &HostRootApparentRepositorySourceInputKey,
    ) -> (
        HostRootApparentRepositorySourcePathInputOutcome,
        Arc<HostRootApparentRepositorySourceInputResult>,
    ) {
        let mut tx = dice.updater().commit().await;
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(source_key).await.unwrap()
        else {
            panic!("source predecessor must be complete")
        };
        let tracker = Arc::new(Tracker::default());
        tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let outcome = tx.compute(key).await.unwrap();
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [
                ("source", ActivationKind::Reused),
                ("path", ActivationKind::Evaluated)
            ]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.clear();
        tx.compute(key).await.unwrap();
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [("path", ActivationKind::Reused)]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        (outcome, predecessor)
    }

    fn observed_path_value(
        outcome: &<HostRootApparentRepositorySourcePathInputObservationKey as Key>::Value,
    ) -> &ObservedPath {
        match outcome {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("observed source path must have a carrier: {value:?}"),
        }
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
    async fn completed_observed_path_state(
        context: ObservedContext<'_>,
        tx: dice::DiceTransaction,
        tracker: Arc<Tracker>,
    ) -> ObservedState {
        let mut tx = tx;
        let mut outcome = tx.compute(context.2).await.unwrap();
        if matches!(outcome, SourcePreparationOutcome::Need(_)) {
            let legacy_child = HostRootApparentRepositorySourceInputKey::new(
                context.1.clone(),
                context.2.0.apparent_repo.clone(),
            )
            .unwrap();
            let _ = complete_local(context.0.as_ref(), context.1, &legacy_child).await;
            tx = context
                .0
                .updater_with_data(UserComputationData {
                    cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
                    activation_tracker: Some(tracker.clone()),
                    ..Default::default()
                })
                .commit()
                .await;
            tracker.clear();
            outcome = tx.compute(context.2).await.unwrap();
        }
        let parent = observed_path_value(&outcome).dupe();
        let child = tx.compute(context.3).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("observed source-input child must have a carrier")
        };
        let global = tx.compute(&PathObservationEpochKey).await.unwrap();
        assert_eq!(parent.observations(), child.observations());
        for (demand, result) in parent.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        (tx, parent, child)
    }
    async fn changed_observed_path_state(
        context: ObservedContext<'_>,
        module: &str,
        extension: &str,
    ) -> (ObservedPath, ObservedSource) {
        let tx = transaction(context.0.as_ref(), module, extension, true, None).await;
        let (_, parent, child) =
            completed_observed_path_state(context, tx, Arc::new(Tracker::default())).await;
        (parent, child)
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RealPathFamily {
        Generated,
        SelectedWorkspace,
        SelectedCommand,
        MappingFailure,
        Missing,
        Main,
        Builtin,
    }
    async fn real_path_transaction(
        dice: &Arc<Dice>,
        family: RealPathFamily,
        tracker: Arc<Tracker>,
    ) -> dice::DiceTransaction {
        if family == RealPathFamily::Builtin {
            prepare_builtin(dice, &NormalizedAbsolutePath::new(WORKSPACE).unwrap()).await;
        }
        if family == RealPathFamily::SelectedCommand {
            let module = "module(name='bazel_tools')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
            let _ = transaction_with_command_override(dice, module, EXTENSION_A, "local").await;
        }
        if matches!(
            family,
            RealPathFamily::Builtin | RealPathFamily::SelectedCommand
        ) {
            return dice
                .updater_with_data(UserComputationData {
                    cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
                    activation_tracker: Some(tracker),
                    ..Default::default()
                })
                .commit()
                .await;
        }
        let module = match family {
            RealPathFamily::Generated | RealPathFamily::Missing => MODULE,
            RealPathFamily::SelectedWorkspace => {
                "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n"
            }
            RealPathFamily::MappingFailure => "this is not valid Starlark\n",
            RealPathFamily::Main => "module(name='bazel_tools', repo_name='root_self')\n",
            _ => unreachable!(),
        };
        let tx = transaction(dice, module, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        tx
    }
    #[tokio::test]
    async fn observed_root_apparent_repository_source_path_input_identity_finisher_and_terminal_algebra()
     {
        type K = HostRootApparentRepositorySourcePathInputKey;
        type O = HostRootApparentRepositorySourcePathInputObservationKey;
        type S = HostRootApparentRepositorySourceInputObservationKey;
        type E = HostRootApparentRepositorySourcePathInputErrorKind;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = |name| ApparentRepoName::new(name).unwrap();
        let observed = |name, path| O::new(workspace.clone(), apparent(name), path).unwrap();
        let display = O::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            apparent("first"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        assert_eq!(
            display.to_string(),
            "observed-HostRootApparentRepositorySourcePathInputKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\"), requested_path: \"pkg/file.bzl\" }"
        );
        assert!(
            O::new(
                workspace.clone(),
                ApparentRepoName::root(),
                "pkg/file.bzl".into()
            )
            .is_none()
        );
        let key = observed("first", "pkg/file.bzl".into());
        let same = observed("first", "pkg/file.bzl".into());
        let other = observed("first", "pkg/other.bzl".into());
        let hash = |value: &O| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));
        let invalid = observed("absent", "../escape".into());
        let invalid_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut invalid_tx =
            real_path_transaction(&invalid_dice, RealPathFamily::Generated, tracker.clone()).await;
        tracker.clear();
        let invalid_outcome = invalid_tx.compute(&invalid).await.unwrap();
        let invalid_carrier = observed_path_value(&invalid_outcome);
        assert!(O::validity(&invalid_outcome));
        assert!(O::equality(&invalid_outcome, &invalid_outcome));
        assert_eq!(
            invalid_carrier.observations(),
            &PathObservationEpoch::empty()
        );
        assert!(
            matches!(invalid_carrier.result().as_ref(), Err(error) if matches!(error.kind, E::Path(_)))
        );
        assert!(dependency_row!(&tracker, &invalid.to_string()).is_empty());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let child_key = S::new(workspace.clone(), apparent("first")).unwrap();
        let context = (&dice, &workspace, &key, &child_key);
        let (mut tx, carrier, child) =
            completed_observed_path_state(context, tx, tracker.clone()).await;
        assert_eq!(
            dependency_row!(&tracker, &key.to_string()),
            [child_key.to_string()]
        );
        let Err(error) = carrier.result().as_ref() else {
            panic!("generated source must remain a source terminal")
        };
        let E::Source { predecessor, .. } = &error.kind else {
            panic!("generated source must retain its child")
        };
        assert!(Arc::ptr_eq(predecessor, child.result()));
        let legacy_key =
            K::new(workspace.clone(), apparent("first"), "pkg/file.bzl".into()).unwrap();
        tracker.clear();
        let _ = tx.compute(&legacy_key).await.unwrap();
        let legacy_child =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent("first"))
                .unwrap();
        assert_eq!(
            dependency_row!(&tracker, &legacy_key.to_string()),
            [legacy_child.to_string()]
        );
        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let mut need_tx = transaction(&need_dice, module, EXTENSION_A, true, None).await;
        let need_key = observed("local_alias", "pkg/file.bzl".into());
        let need_child = S::new(workspace.clone(), apparent("local_alias")).unwrap();
        let path_need = need_tx.compute(&need_key).await.unwrap();
        let child_need = need_tx.compute(&need_child).await.unwrap();
        assert!(!O::validity(&path_need));
        assert!(!O::equality(&path_need, &path_need));
        let (SourcePreparationOutcome::Need(path_need), SourcePreparationOutcome::Need(child_need)) =
            (&path_need, &child_need)
        else {
            panic!("both owners must forward Need")
        };
        assert_eq!(
            path_need.repository_materializations(),
            child_need.repository_materializations()
        );
    }
    #[tokio::test]
    async fn observed_root_apparent_repository_source_path_input_real_families_events_and_parity() {
        type K = HostRootApparentRepositorySourcePathInputKey;
        type O = HostRootApparentRepositorySourcePathInputObservationKey;
        type S = HostRootApparentRepositorySourceInputObservationKey;
        type L = HostRootApparentRepositorySourceInputKey;
        type F = RealPathFamily;
        type E = HostRootApparentRepositorySourcePathInputErrorKind;
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
            let key = O::new(workspace.clone(), apparent.clone(), "pkg/file.bzl".into()).unwrap();
            let child_key = S::new(workspace.clone(), apparent.clone()).unwrap();
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let context = (&dice, &workspace, &key, &child_key);
            let tracker = Arc::new(Tracker::default());
            let tx = real_path_transaction(&dice, family, tracker.clone()).await;
            let (mut tx, carrier, child) =
                completed_observed_path_state(context, tx, tracker.clone()).await;
            assert_eq!(
                dependency_row!(&tracker, &key.to_string()),
                [child_key.to_string()],
                "{family:?}"
            );
            let activations = tracker.activations.lock().unwrap();
            let parent = activations
                .iter()
                .find(|(name, _, _)| name == &key.to_string())
                .unwrap();
            assert_eq!(parent.1, ActivationKind::Evaluated);
            assert!(parent.2.is_none());
            drop(activations);
            let parent_events = event_rows!(&tracker);
            match (family, carrier.result().as_ref()) {
                (F::Generated | F::MappingFailure | F::Missing, Err(error)) => {
                    assert!(matches!(error.kind, E::Source { .. }))
                }
                (F::Main | F::Builtin, Ok(_)) => {}
                (F::SelectedWorkspace | F::SelectedCommand, Ok(value)) => {
                    let HostRootApparentRepositorySourcePathInputDispositionView::Input(input) =
                        value.view().unwrap().disposition()
                    else {
                        panic!("selected input")
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
                value => panic!("unexpected {family:?} path terminal: {value:?}"),
            }
            match carrier.result().as_ref() {
                Ok(value) => {
                    assert!(Arc::ptr_eq(&value.predecessor, child.result()));
                    assert_eq!(value.relative_path.as_path(), Path::new("pkg/file.bzl"));
                }
                Err(HostRootApparentRepositorySourcePathInputError {
                    kind:
                        E::Source {
                            relative_path,
                            predecessor,
                        }
                        | E::InvalidSource {
                            relative_path,
                            predecessor,
                        },
                    ..
                }) => {
                    assert!(Arc::ptr_eq(predecessor, child.result()));
                    assert_eq!(relative_path.as_path(), Path::new("pkg/file.bzl"));
                }
                value => panic!("unexpected retained path terminal: {value:?}"),
            }
            let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let direct_tracker = Arc::new(Tracker::default());
            let direct_tx =
                real_path_transaction(&direct_dice, family, direct_tracker.clone()).await;
            direct_tracker.clear();
            let direct_context = (&direct_dice, &workspace, &key, &child_key);
            let _ =
                completed_observed_path_state(direct_context, direct_tx, direct_tracker.clone())
                    .await;
            assert_eq!(parent_events, event_rows!(&direct_tracker), "{family:?}");
            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy_tracker = Arc::new(Tracker::default());
            let _ = real_path_transaction(&legacy_dice, family, legacy_tracker).await;
            let legacy_key =
                K::new(workspace.clone(), apparent.clone(), "pkg/file.bzl".into()).unwrap();
            let legacy_child = L::new(workspace.clone(), apparent).unwrap();
            let _ = complete_local(&legacy_dice, &workspace, &legacy_child).await;
            let mut legacy_tx = legacy_dice.updater().commit().await;
            let legacy = legacy_tx.compute(&legacy_key).await.unwrap();
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("legacy path")
            };
            assert_eq!(legacy.as_ref(), carrier.result().as_ref(), "{family:?}");
            tracker.clear();
            let warm = tx.compute(&key).await.unwrap();
            assert!(Arc::ptr_eq(
                observed_path_value(&warm).result(),
                carrier.result()
            ));
            let warm = tracker.activations.lock().unwrap();
            assert!(!warm.is_empty());
            assert!(
                warm.iter()
                    .all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none())
            );
            drop(warm);
            assert!(event_rows!(&tracker).is_empty());
        }
    }
    #[tokio::test]
    async fn observed_root_apparent_repository_source_path_input_lifecycle_cancellation_and_nonactivation()
     {
        type F = RealPathFamily;
        type O = HostRootApparentRepositorySourcePathInputObservationKey;
        type S = HostRootApparentRepositorySourceInputObservationKey;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = O::new(workspace.clone(), apparent.clone(), "pkg/file.bzl".into()).unwrap();
        let child_key = S::new(workspace.clone(), apparent).unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let context = (&dice, &workspace, &key, &child_key);
        let tracker = Arc::new(Tracker::default());
        let a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (a_tx, a, a_child) =
            completed_observed_path_state(context, a_tx, tracker.clone()).await;
        assert_eq!(
            dependency_row!(&tracker, &key.to_string()),
            [child_key.to_string()]
        );
        let a_result = a.result().clone();
        let a_epoch = a.observations().clone();
        let a_child_result = a_child.result().clone();
        let a_child_epoch = a_child.observations().clone();
        tracker.clear();
        let (_a_tx, warm, warm_child) =
            completed_observed_path_state(context, a_tx, tracker.clone()).await;
        let warm_rows = tracker.activations.lock().unwrap();
        assert!(
            warm_rows
                .iter()
                .all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none())
        );
        assert!(
            warm_rows
                .iter()
                .any(|(name, _, _)| name == &key.to_string())
        );
        assert!(
            warm_rows
                .iter()
                .any(|(name, _, _)| name == &child_key.to_string())
        );
        drop(warm_rows);
        assert!(Arc::ptr_eq(warm.result(), a.result()));
        assert!(Arc::ptr_eq(warm_child.result(), a_child.result()));
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
            let (changed, changed_child) =
                changed_observed_path_state(context, module, extension).await;
            assert_ne!(changed.result(), a.result());
            assert_ne!(changed_child.result(), a_child.result());
            let (restored, restored_child) =
                changed_observed_path_state(context, MODULE, EXTENSION_A).await;
            assert_eq!(restored.result(), a.result());
            assert_eq!(restored_child.result(), a_child.result());
            assert!(!Arc::ptr_eq(restored.result(), a.result()));
            assert!(!Arc::ptr_eq(restored_child.result(), a_child.result()));
        }
        let neutral_module = format!("{MODULE}\n");
        let (neutral, neutral_child) =
            changed_observed_path_state(context, &neutral_module, EXTENSION_A).await;
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
        let ld = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let repo = ApparentRepoName::new("local_alias").unwrap();
        let lk = O::new(workspace.clone(), repo.clone(), "pkg/file.bzl".into()).unwrap();
        let lck = S::new(workspace.clone(), repo).unwrap();
        let lc = (&ld, &workspace, &lk, &lck);
        let lt = Arc::new(Tracker::default());
        let ltx = real_path_transaction(&ld, F::SelectedWorkspace, lt.clone()).await;
        let (_, base, base_child) = completed_observed_path_state(lc, ltx, lt.clone()).await;
        let ctx = real_path_transaction(&ld, F::SelectedCommand, lt.clone()).await;
        let (_, command, command_child) = completed_observed_path_state(lc, ctx, lt.clone()).await;
        assert_ne!(command.result(), base.result());
        assert_ne!(command_child.result(), base_child.result());
        let rtx = real_path_transaction(&ld, F::SelectedWorkspace, lt.clone()).await;
        let (_, restored, restored_child) = completed_observed_path_state(lc, rtx, lt).await;
        assert_eq!(restored.result(), base.result());
        assert_eq!(restored_child.result(), base_child.result());
        assert!(!Arc::ptr_eq(restored.result(), base.result()));
        assert!(!Arc::ptr_eq(restored_child.result(), base_child.result()));
        let cd = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let ct = Arc::new(Tracker::default());
        let cc = (&cd, &workspace, &key, &child_key);
        let mut cancelled = real_path_transaction(&cd, F::Generated, ct.clone()).await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        let cancelled_trace = format!(
            "{:?}{:?}",
            *ct.activations.lock().unwrap(),
            *ct.dependencies.lock().unwrap()
        );
        assert!(!cancelled_trace.contains(&key.to_string()));
        let recovery = real_path_transaction(&cd, F::Generated, ct.clone()).await;
        let (_, recovered, recovered_child) =
            completed_observed_path_state(cc, recovery, ct.clone()).await;
        assert_eq!(recovered.result(), a.result());
        assert_eq!(recovered_child.result(), a_child.result());
        let trace = format!(
            "{:?}{:?}",
            *ct.activations.lock().unwrap(),
            *ct.dependencies.lock().unwrap()
        );
        for forbidden in [
            "HostRootApparentRepositorySourceObservationKey",
            "build-command-root:",
            "RootModuleBootstrap",
            "bootstrap",
        ] {
            assert!(!trace.contains(forbidden));
        }
        let producer = include_str!("root_apparent_repository_source_path_input.rs");
        let producer = producer.split("#[cfg(test)]").next().unwrap();
        for absent in [
            "SourceObservationKey",
            "BuildCommand",
            "RootModuleBootstrap",
            "bootstrap",
        ] {
            assert!(!producer.contains(absent));
        }
    }

    #[test]
    fn key_and_completed_dispatch_are_fail_closed() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        assert!(
            HostRootApparentRepositorySourcePathInputKey::new(
                workspace.clone(),
                ApparentRepoName::root(),
                "dep/file".into(),
            )
            .is_none()
        );
        let key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace,
            ApparentRepoName::new("dep").unwrap(),
            "dep/file".into(),
        )
        .unwrap();
        assert!(key.to_string().contains("dep/file"));
        for (success, view, expected) in [
            (false, false, CompletedSourceDisposition::Source),
            (false, true, CompletedSourceDisposition::Source),
            (true, false, CompletedSourceDisposition::InvalidSource),
            (true, true, CompletedSourceDisposition::Success),
        ] {
            assert_eq!(completed_source_disposition(success, view), expected);
        }
    }

    #[tokio::test]
    async fn invalid_path_precedes_every_source_activation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let key = HostRootApparentRepositorySourcePathInputKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ApparentRepoName::new("absent").unwrap(),
            "../escape".into(),
        )
        .unwrap();
        let outcome = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await
            .compute(&key)
            .await
            .unwrap();
        assert_eq!(
            *tracker.order.lock().unwrap(),
            [("path", ActivationKind::Evaluated)]
        );
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let SourcePreparationOutcome::Complete(result) = outcome else {
            unreachable!()
        };
        let error = result.as_ref().as_ref().unwrap_err();
        assert_eq!(error.apparent_repo.as_str(), "absent");
        let HostRootApparentRepositorySourcePathInputErrorKind::Path(path) = &error.kind else {
            unreachable!()
        };
        assert_eq!(path.requested_path(), Path::new("../escape"));
    }

    #[tokio::test]
    async fn valid_path_forwards_source_need_exactly() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let apparent = ApparentRepoName::new("local_alias").unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace, apparent).unwrap();
        let mut tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let path_need = tx.compute(&path_key).await.unwrap();
        let source_need = tx.compute(&source_key).await.unwrap();
        assert!(!HostRootApparentRepositorySourcePathInputKey::validity(
            &path_need
        ));
        assert!(!HostRootApparentRepositorySourcePathInputKey::equality(
            &path_need, &path_need
        ));
        let (
            SourcePreparationOutcome::Need(path_need),
            SourcePreparationOutcome::Need(source_need),
        ) = (&path_need, &source_need)
        else {
            unreachable!()
        };
        assert_eq!(
            path_need.repository_materializations(),
            source_need.repository_materializations()
        );
    }

    #[tokio::test]
    async fn real_main_builtin_and_source_terminal_retain_exact_predecessors() {
        type E = HostRootApparentRepositorySourcePathInputErrorKind;
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
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) =
            tx.compute(&source_key).await.unwrap()
        else {
            unreachable!()
        };
        let main = tx.compute(&path_key).await.unwrap();
        assert!(Arc::ptr_eq(&value(&main).predecessor, &predecessor));
        let view = value(&main).view().unwrap();
        assert_eq!(view.apparent_repo().as_str(), "root_self");
        assert!(view.canonical_repo().is_root());
        assert_eq!(view.relative_path().as_path(), Path::new("pkg/file.bzl"));
        assert!(matches!(
            view.disposition(),
            HostRootApparentRepositorySourcePathInputDispositionView::Main
        ));
        let observed_key = HostRootApparentRepositorySourceInputObservationKey::new(
            workspace.clone(),
            ApparentRepoName::new("root_self").unwrap(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            tx.compute(&observed_key).await.unwrap()
        else {
            panic!("main child")
        };
        let relative = host_repository_relative_path("pkg/file.bzl".into()).unwrap();
        let (finished, epoch) = finish_root_apparent_repository_source_path_input(
            &path_key,
            relative.clone(),
            observed.result().clone(),
            observed.observations().clone(),
        );
        assert!(finished.is_ok());
        assert_eq!(&epoch, observed.observations());
        let mismatch = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            ApparentRepoName::new("second").unwrap(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let (invalid_source, invalid_epoch) = finish_root_apparent_repository_source_path_input(
            &mismatch,
            relative.clone(),
            observed.result().clone(),
            observed.observations().clone(),
        );
        assert_eq!(&invalid_epoch, observed.observations());
        assert!(
            matches!(invalid_source.as_ref(), Err(error) if matches!(error.kind, E::InvalidSource { .. }))
        );
        let compute = complete_source_path_error(
            &path_key,
            E::Compute {
                relative_path: relative,
                message: "boom".into(),
            },
        );
        let SourcePreparationOutcome::Complete(Ok((compute, compute_epoch))) = compute else {
            unreachable!()
        };
        assert!(matches!(compute.as_ref(), Err(error) if matches!(error.kind, E::Compute { .. })));
        assert_eq!(compute_epoch, PathObservationEpoch::empty());
        let corrupt = Arc::new(Ok(corrupt_workspace(
            predecessor.as_ref().as_ref().unwrap(),
        )));
        let path = host_repository_relative_path("pkg/file.bzl".into()).unwrap();
        let source_error = completed_source_error(
            &workspace,
            &ApparentRepoName::new("root_self").unwrap(),
            path.clone(),
            predecessor,
            CompletedSourceDisposition::Source,
        );
        let invalid = completed_source_error(
            &workspace,
            &ApparentRepoName::new("root_self").unwrap(),
            path.clone(),
            corrupt.clone(),
            completed_source_disposition(true, false),
        );
        assert_ne!(source_error, invalid);
        let compute = HostRootApparentRepositorySourcePathInputError {
            workspace: workspace.clone(),
            apparent_repo: ApparentRepoName::new("root_self").unwrap(),
            kind: HostRootApparentRepositorySourcePathInputErrorKind::Compute {
                relative_path: path,
                message: "boom".into(),
            },
        };
        assert_ne!(source_error, compute);
        assert!(compute.to_string().contains("boom"));
        let HostRootApparentRepositorySourcePathInputErrorKind::InvalidSource {
            relative_path,
            predecessor: retained,
        } = invalid.kind
        else {
            unreachable!()
        };
        assert_eq!(relative_path.as_path(), Path::new("pkg/file.bzl"));
        assert!(Arc::ptr_eq(&retained, &corrupt));

        prepare_builtin(&dice, &workspace).await;
        let apparent = ApparentRepoName::new("bazel_tools").unwrap();
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent,
            "tools/build_defs.bzl".into(),
        )
        .unwrap();
        let (builtin, predecessor) = tracked_complete(&dice, &path_key, &source_key).await;
        assert!(Arc::ptr_eq(&value(&builtin).predecessor, &predecessor));
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(input) =
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
        let apparent = ApparentRepoName::new("first").unwrap();
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace,
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) =
            generated_tx.compute(&source_key).await.unwrap()
        else {
            unreachable!()
        };
        let SourcePreparationOutcome::Complete(generated) =
            generated_tx.compute(&path_key).await.unwrap()
        else {
            unreachable!()
        };
        let error = generated.as_ref().as_ref().unwrap_err();
        let HostRootApparentRepositorySourcePathInputErrorKind::Source {
            relative_path,
            predecessor: retained,
        } = &error.kind
        else {
            unreachable!()
        };
        assert_eq!(relative_path.as_path(), Path::new("pkg/file.bzl"));
        assert!(Arc::ptr_eq(retained, &predecessor));
    }

    #[tokio::test]
    async fn selected_policies_path_identity_and_request_arc_restore() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let apparent = ApparentRepoName::new("local_alias").unwrap();
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/a.bzl".into(),
        )
        .unwrap();
        let _tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let root_source = complete_local(&dice, &workspace, &source_key).await;
        let (root, predecessor) = tracked_complete(&dice, &path_key, &source_key).await;
        assert!(Arc::ptr_eq(&value(&root).predecessor, &predecessor));
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(root_input) =
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
        let HostRootApparentRepositorySourceInputDispositionView::Input(source_input) =
            source_value(&root_source).view().unwrap().disposition()
        else {
            unreachable!()
        };
        let HostRepositorySourceInputDispositionView::Request(source_request) =
            source_input.view().disposition()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(root_request, source_request));
        let cloned = value(&root).clone();
        assert!(Arc::ptr_eq(
            value(&root).relative_path.path_arc(),
            cloned.relative_path.path_arc()
        ));

        let changed_path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/b.bzl".into(),
        )
        .unwrap();
        let changed_path = dice
            .updater()
            .commit()
            .await
            .compute(&changed_path_key)
            .await
            .unwrap();
        assert!(!HostRootApparentRepositorySourcePathInputKey::equality(
            &root,
            &changed_path
        ));
        let restored = dice
            .updater()
            .commit()
            .await
            .compute(&path_key)
            .await
            .unwrap();
        assert!(HostRootApparentRepositorySourcePathInputKey::equality(
            &root, &restored
        ));

        let command_module = "module(name='bazel_tools')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let _tx =
            transaction_with_command_override(&dice, command_module, EXTENSION_A, "local").await;
        let _command_source = complete_local(&dice, &workspace, &source_key).await;
        let command = dice
            .updater()
            .commit()
            .await
            .compute(&path_key)
            .await
            .unwrap();
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(command_input) =
            value(&command).view().unwrap().disposition()
        else {
            unreachable!()
        };
        assert_eq!(
            command_input.view().capability().local_path_policy(),
            Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::CommandAbsolute)
        );
        assert!(!HostRootApparentRepositorySourcePathInputKey::equality(
            &root, &command
        ));
        let _tx = transaction(&dice, module, EXTENSION_A, true, None).await;
        let _restored_source = complete_local(&dice, &workspace, &source_key).await;
        let restored = dice
            .updater()
            .commit()
            .await
            .compute(&path_key)
            .await
            .unwrap();
        assert!(HostRootApparentRepositorySourcePathInputKey::equality(
            &root, &restored
        ));
    }

    #[test]
    fn production_edge_is_path_then_source_input_only() {
        let source = include_str!("root_apparent_repository_source_path_input.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches(".compute(").count(), 2);
        assert_eq!(
            production.matches("host_repository_relative_path(").count(),
            1
        );
        assert_eq!(
            production
                .matches("HostRootApparentRepositorySourceInputKey::new")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("HostRootApparentRepositorySourceInputObservationKey::new")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("RootApparentRepositorySourcePathInputObservationError::Source(error)")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("HostRootApparentRepositorySourcePathInputObservationError(error)")
                .count(),
            1
        );
        assert!(
            production.contains("predecessor: Arc<HostRootApparentRepositorySourceInputResult>")
        );
        for forbidden in [
            ["HostRepository", "PathKey"].concat(),
            ["HostRepositorySource", "FileKey"].concat(),
            ["RepositoryMaterialization", "ResultKey"].concat(),
            ["RepositoryPackage", "LoadKey"].concat(),
            ["RootRepositoryRoute", "Key"].concat(),
            ["std::", "fs"].concat(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "forbidden edge: {forbidden}"
            );
        }
    }
}
