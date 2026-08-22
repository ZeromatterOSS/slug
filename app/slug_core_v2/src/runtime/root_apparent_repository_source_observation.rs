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
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
use slug_bzlmod_v2::HostRepositorySourceObservation;
use slug_bzlmod_v2::HostRepositorySourceObservationKey;
use slug_bzlmod_v2::HostRepositorySourceObservationResult;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;

use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputDispositionView;
use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputKey;
use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputObservationError;
use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputObservationKey;
use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputResult;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostRootApparentRepositorySourceObservation {
    predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
    observation: Option<Arc<HostRepositorySourceObservationResult>>,
}

#[derive(Debug, Clone, Copy)]
enum HostRootApparentRepositorySourceObservationDispositionView<'a> {
    Main,
    Input {
        input: &'a HostRepositorySourceInput,
        observation: &'a HostRepositorySourceObservation,
    },
}

#[derive(Debug, Clone, Copy)]
struct HostRootApparentRepositorySourceObservationView<'a> {
    apparent_repo: &'a ApparentRepoName,
    canonical_repo: &'a CanonicalRepoName,
    relative_path: &'a slug_bzlmod_v2::HostRepositoryRelativePath,
    disposition: HostRootApparentRepositorySourceObservationDispositionView<'a>,
}

fn observation_matches_input(
    input: &HostRepositorySourceInput,
    observation: &HostRepositorySourceObservation,
) -> bool {
    use HostRepositorySourceInputDispositionView as Input;
    use HostRepositorySourceObservation as Observation;

    matches!(
        (input.view().disposition(), observation),
        (Input::Builtin(_), Observation::Builtin(_)) | (Input::Request(_), Observation::Request(_))
    )
}

impl HostRootApparentRepositorySourceObservation {
    fn view(&self) -> Option<HostRootApparentRepositorySourceObservationView<'_>> {
        let predecessor = self.predecessor.as_ref().as_ref().ok()?;
        let predecessor = predecessor.view()?;
        let disposition = match (predecessor.disposition(), &self.observation) {
            (HostRootApparentRepositorySourcePathInputDispositionView::Main, None) => {
                HostRootApparentRepositorySourceObservationDispositionView::Main
            }
            (
                HostRootApparentRepositorySourcePathInputDispositionView::Input(input),
                Some(observation),
            ) => {
                let observation = observation.as_ref().as_ref().ok()?;
                if !observation_matches_input(input, observation) {
                    return None;
                }
                HostRootApparentRepositorySourceObservationDispositionView::Input {
                    input,
                    observation,
                }
            }
            _ => return None,
        };
        Some(HostRootApparentRepositorySourceObservationView {
            apparent_repo: predecessor.apparent_repo(),
            canonical_repo: predecessor.canonical_repo(),
            relative_path: predecessor.relative_path(),
            disposition,
        })
    }
}

impl<'a> HostRootApparentRepositorySourceObservationView<'a> {
    fn apparent_repo(self) -> &'a ApparentRepoName {
        self.apparent_repo
    }

    fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }

    fn relative_path(self) -> &'a slug_bzlmod_v2::HostRepositoryRelativePath {
        self.relative_path
    }

    fn disposition(self) -> HostRootApparentRepositorySourceObservationDispositionView<'a> {
        self.disposition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourceObservationErrorKind {
    SourcePathCompute {
        message: Arc<str>,
    },
    SourcePath {
        predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
    },
    InvalidSourcePath {
        predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
    },
    ObservationCompute {
        predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
        message: Arc<str>,
    },
    Observation {
        predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
        observation: Arc<HostRepositorySourceObservationResult>,
    },
    InvalidObservation {
        predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
        observation: Arc<HostRepositorySourceObservationResult>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostRootApparentRepositorySourceObservationError {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    kind: HostRootApparentRepositorySourceObservationErrorKind,
}

impl fmt::Display for HostRootApparentRepositorySourceObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for HostRootApparentRepositorySourceObservationError {}

type HostRootApparentRepositorySourceObservationResult = Result<
    HostRootApparentRepositorySourceObservation,
    HostRootApparentRepositorySourceObservationError,
>;
type HostRootApparentRepositorySourceObservationOutcome =
    SourcePreparationOutcome<Arc<HostRootApparentRepositorySourceObservationResult>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostRootApparentRepositorySourceObservationKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    requested_path: PathBuf,
}

impl HostRootApparentRepositorySourceObservationKey {
    fn new(
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

    fn terminal(
        &self,
        kind: HostRootApparentRepositorySourceObservationErrorKind,
    ) -> HostRootApparentRepositorySourceObservationOutcome {
        complete(Err(HostRootApparentRepositorySourceObservationError {
            workspace: self.workspace.clone(),
            apparent_repo: self.apparent_repo.clone(),
            kind,
        }))
    }

    fn compute_terminal(
        &self,
        predecessor: Option<Arc<HostRootApparentRepositorySourcePathInputResult>>,
        message: Arc<str>,
    ) -> HostRootApparentRepositorySourceObservationOutcome {
        let kind = match predecessor {
            None => {
                HostRootApparentRepositorySourceObservationErrorKind::SourcePathCompute { message }
            }
            Some(predecessor) => {
                HostRootApparentRepositorySourceObservationErrorKind::ObservationCompute {
                    predecessor,
                    message,
                }
            }
        };
        self.terminal(kind)
    }
}

impl fmt::Display for HostRootApparentRepositorySourceObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostRootApparentRepositorySourceObservationObservationKey(
    HostRootApparentRepositorySourceObservationKey,
);

impl HostRootApparentRepositorySourceObservationObservationKey {
    fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        requested_path: PathBuf,
    ) -> Option<Self> {
        HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent_repo,
            requested_path,
        )
        .map(Self)
    }
}

impl fmt::Display for HostRootApparentRepositorySourceObservationObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct ObservedHostRootApparentRepositorySourceObservation {
    result: Arc<HostRootApparentRepositorySourceObservationResult>,
    observations: PathObservationEpoch,
}

impl ObservedHostRootApparentRepositorySourceObservation {
    fn result(&self) -> &Arc<HostRootApparentRepositorySourceObservationResult> {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostRootApparentRepositorySourceObservationObservationError {
    SourcePath(HostRootApparentRepositorySourcePathInputObservationError),
}

impl Dupe for HostRootApparentRepositorySourceObservationObservationError {}

enum HostRootApparentRepositorySourceObservationMode {
    Legacy,
    Observed,
}

type HostRootApparentRepositorySourceObservationDriverOutcome = SourcePreparationOutcome<
    Result<
        (
            Arc<HostRootApparentRepositorySourceObservationResult>,
            PathObservationEpoch,
        ),
        HostRootApparentRepositorySourceObservationObservationError,
    >,
>;

fn complete(
    value: HostRootApparentRepositorySourceObservationResult,
) -> HostRootApparentRepositorySourceObservationOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

fn finish_root_apparent_repository_source_observation(
    key: &HostRootApparentRepositorySourceObservationKey,
    predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>,
    observation: Option<Arc<HostRepositorySourceObservationResult>>,
    observations: PathObservationEpoch,
) -> (
    Arc<HostRootApparentRepositorySourceObservationResult>,
    PathObservationEpoch,
) {
    let kind = match predecessor.as_ref().as_ref() {
        Err(_) => Some(
            HostRootApparentRepositorySourceObservationErrorKind::SourcePath {
                predecessor: predecessor.clone(),
            },
        ),
        Ok(value) => match (value.view().map(|view| view.disposition()), &observation) {
            (None, _)
            | (Some(HostRootApparentRepositorySourcePathInputDispositionView::Input(_)), None) => {
                Some(
                    HostRootApparentRepositorySourceObservationErrorKind::InvalidSourcePath {
                        predecessor: predecessor.clone(),
                    },
                )
            }
            (
                Some(HostRootApparentRepositorySourcePathInputDispositionView::Main),
                Some(observation),
            ) => Some(
                HostRootApparentRepositorySourceObservationErrorKind::InvalidObservation {
                    predecessor: predecessor.clone(),
                    observation: observation.clone(),
                },
            ),
            (_, Some(observation)) if observation.as_ref().is_err() => Some(
                HostRootApparentRepositorySourceObservationErrorKind::Observation {
                    predecessor: predecessor.clone(),
                    observation: observation.clone(),
                },
            ),
            _ => None,
        },
    };
    let result = match kind {
        Some(kind) => Err(HostRootApparentRepositorySourceObservationError {
            workspace: key.workspace.clone(),
            apparent_repo: key.apparent_repo.clone(),
            kind,
        }),
        None => {
            let certificate = HostRootApparentRepositorySourceObservation {
                predecessor: predecessor.clone(),
                observation: observation.clone(),
            };
            if certificate.view().is_some() {
                Ok(certificate)
            } else {
                Err(HostRootApparentRepositorySourceObservationError {
                    workspace: key.workspace.clone(),
                    apparent_repo: key.apparent_repo.clone(),
                    kind: match observation {
                        None => {
                            HostRootApparentRepositorySourceObservationErrorKind::InvalidSourcePath {
                                predecessor,
                            }
                        }
                        Some(observation) => {
                            HostRootApparentRepositorySourceObservationErrorKind::InvalidObservation {
                                predecessor,
                                observation,
                            }
                        }
                    },
                })
            }
        }
    };
    (Arc::new(result), observations)
}

async fn compute_root_apparent_repository_source_observation(
    key: &HostRootApparentRepositorySourceObservationKey,
    mode: HostRootApparentRepositorySourceObservationMode,
    ctx: &mut DiceComputations<'_>,
) -> HostRootApparentRepositorySourceObservationDriverOutcome {
    let (predecessor, observations) = match mode {
        HostRootApparentRepositorySourceObservationMode::Legacy => match ctx
            .compute(
                &HostRootApparentRepositorySourcePathInputKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                    key.requested_path.clone(),
                )
                .expect("source-observation key rejects root apparent names"),
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
                let result = key.compute_terminal(None, error.to_string().into());
                let SourcePreparationOutcome::Complete(result) = result else {
                    unreachable!("semantic compute errors are complete")
                };
                return SourcePreparationOutcome::Complete(Ok((
                    result,
                    PathObservationEpoch::empty(),
                )));
            }
        },
        HostRootApparentRepositorySourceObservationMode::Observed => match ctx
            .compute(
                &HostRootApparentRepositorySourcePathInputObservationKey::new(
                    key.workspace.clone(),
                    key.apparent_repo.clone(),
                    key.requested_path.clone(),
                )
                .expect("source-observation key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostRootApparentRepositorySourceObservationObservationError::SourcePath(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().clone(), observed.observations().clone())
            }
            Err(error) => {
                let result = key.compute_terminal(None, error.to_string().into());
                let SourcePreparationOutcome::Complete(result) = result else {
                    unreachable!("semantic compute errors are complete")
                };
                return SourcePreparationOutcome::Complete(Ok((
                    result,
                    PathObservationEpoch::empty(),
                )));
            }
        },
    };

    let observation_input = predecessor
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|value| value.view())
        .and_then(|view| match view.disposition() {
            HostRootApparentRepositorySourcePathInputDispositionView::Main => None,
            HostRootApparentRepositorySourcePathInputDispositionView::Input(input) => {
                Some((input.clone(), view.relative_path().clone()))
            }
        });
    let Some((input, relative_path)) = observation_input else {
        return SourcePreparationOutcome::Complete(Ok(
            finish_root_apparent_repository_source_observation(
                key,
                predecessor,
                None,
                observations,
            ),
        ));
    };
    let observation = match ctx
        .compute(&HostRepositorySourceObservationKey::new(
            input,
            relative_path,
        ))
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(observation)) => observation,
        Err(error) => {
            let result = key.compute_terminal(Some(predecessor), error.to_string().into());
            let SourcePreparationOutcome::Complete(result) = result else {
                unreachable!("semantic compute errors are complete")
            };
            return SourcePreparationOutcome::Complete(Ok((result, observations)));
        }
    };
    SourcePreparationOutcome::Complete(Ok(finish_root_apparent_repository_source_observation(
        key,
        predecessor,
        Some(observation),
        observations,
    )))
}

#[async_trait]
impl Key for HostRootApparentRepositorySourceObservationKey {
    type Value = HostRootApparentRepositorySourceObservationOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_observation(
            self,
            HostRootApparentRepositorySourceObservationMode::Legacy,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy source observation has no observation outer")
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
impl Key for HostRootApparentRepositorySourceObservationObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostRootApparentRepositorySourceObservation,
            HostRootApparentRepositorySourceObservationObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_root_apparent_repository_source_observation(
            &self.0,
            HostRootApparentRepositorySourceObservationMode::Observed,
            ctx,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostRootApparentRepositorySourceObservation {
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
    use dupe::Dupe;
    use slug_bzlmod_v2::HostRepositorySourceCapabilitySource;
    use slug_bzlmod_v2::HostRepositorySourceFileValue;
    use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
    use slug_bzlmod_v2::HostRepositorySourceObservationView;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::transaction_with_command_override;
    use super::super::generated_repository_definition::tests::validated;
    use super::super::root_apparent_repository_definition::tests::prepare_builtin;
    use super::super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputKey;
    use super::super::root_apparent_repository_source_input::tests::complete_local;
    use super::super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputObservationError;
    use super::super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputObservationKey;
    use super::super::root_apparent_repository_source_path_input::ObservedHostRootApparentRepositorySourcePathInput;
    use super::*;

    #[test]
    fn root_apparent_repository_source_path_input_observation_surface_is_sibling_usable() {
        let key = HostRootApparentRepositorySourcePathInputObservationKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("first").unwrap(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "observed-HostRootApparentRepositorySourcePathInputKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\"), requested_path: \"pkg/file.bzl\" }"
        );

        fn inspect(
            _: &<HostRootApparentRepositorySourcePathInputObservationKey as Key>::Value,
            observed: &ObservedHostRootApparentRepositorySourcePathInput,
            _: &HostRootApparentRepositorySourcePathInputObservationError,
        ) {
            let _: &Arc<HostRootApparentRepositorySourcePathInputResult> = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }
        let _ = inspect
            as fn(
                &<HostRootApparentRepositorySourcePathInputObservationKey as Key>::Value,
                &ObservedHostRootApparentRepositorySourcePathInput,
                &HostRootApparentRepositorySourcePathInputObservationError,
            );
    }

    #[derive(Default)]
    struct Tracker {
        activations: Mutex<Vec<(&'static str, ActivationKind)>>,
        rich: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
        events: Mutex<usize>,
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
                .cloned();
            let name = if key
                .downcast_ref::<HostRootApparentRepositorySourceObservationObservationKey>()
                .is_some()
            {
                Some("observed-bridge")
            } else if key
                .downcast_ref::<HostRootApparentRepositorySourceObservationKey>()
                .is_some()
            {
                Some("bridge")
            } else if key
                .downcast_ref::<HostRootApparentRepositorySourcePathInputObservationKey>()
                .is_some()
            {
                Some("observed-path")
            } else if key
                .downcast_ref::<HostRootApparentRepositorySourcePathInputKey>()
                .is_some()
            {
                Some("path")
            } else if key
                .downcast_ref::<HostRepositorySourceObservationKey>()
                .is_some()
            {
                Some("observation")
            } else {
                None
            };
            if let Some(name) = name {
                self.activations.lock().unwrap().push((name, kind));
            }
            *self.events.lock().unwrap() += usize::from(batch.is_some());
            self.rich
                .lock()
                .unwrap()
                .push((key.to_string(), kind, batch));
        }
    }

    impl Tracker {
        fn count(&self, name: &str) -> usize {
            self.activations
                .lock()
                .unwrap()
                .iter()
                .filter(|(actual, _)| *actual == name)
                .count()
        }

        fn clear(&self) {
            self.activations.lock().unwrap().clear();
            self.rich.lock().unwrap().clear();
            self.dependencies.lock().unwrap().clear();
            *self.events.lock().unwrap() = 0;
        }
    }

    fn value(
        outcome: &HostRootApparentRepositorySourceObservationOutcome,
    ) -> &HostRootApparentRepositorySourceObservation {
        let SourcePreparationOutcome::Complete(value) = outcome else {
            panic!("source observation must complete: {outcome:?}")
        };
        value.as_ref().as_ref().unwrap()
    }

    async fn tracked(
        dice: &Arc<Dice>,
        key: &HostRootApparentRepositorySourceObservationKey,
    ) -> (
        HostRootApparentRepositorySourceObservationOutcome,
        Arc<Tracker>,
    ) {
        let tracker = Arc::new(Tracker::default());
        let outcome = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await
            .compute(key)
            .await
            .unwrap();
        (outcome, tracker)
    }

    fn observed_value(
        outcome: &<HostRootApparentRepositorySourceObservationObservationKey as Key>::Value,
    ) -> &ObservedHostRootApparentRepositorySourceObservation {
        let SourcePreparationOutcome::Complete(Ok(value)) = outcome else {
            panic!("observed source observation must complete: {outcome:?}")
        };
        value
    }

    async fn completed_observed_state(
        mut tx: dice::DiceTransaction,
        key: &HostRootApparentRepositorySourceObservationObservationKey,
        child_key: &HostRootApparentRepositorySourcePathInputObservationKey,
    ) -> (
        dice::DiceTransaction,
        ObservedHostRootApparentRepositorySourceObservation,
        ObservedHostRootApparentRepositorySourcePathInput,
    ) {
        let parent = observed_value(&tx.compute(key).await.unwrap()).dupe();
        let SourcePreparationOutcome::Complete(Ok(child)) = tx.compute(child_key).await.unwrap()
        else {
            panic!("observed path child must complete")
        };
        let global = tx.compute(&PathObservationEpochKey).await.unwrap();
        assert_eq!(parent.observations(), child.observations());
        for (demand, result) in parent.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        (tx, parent, child)
    }

    async fn completed_request_state(
        dice: &Arc<Dice>,
        workspace: &NormalizedAbsolutePath,
        key: &HostRootApparentRepositorySourceObservationObservationKey,
        child_key: &HostRootApparentRepositorySourcePathInputObservationKey,
        tracker: Arc<Tracker>,
        bytes: Option<&[u8]>,
    ) -> (
        dice::DiceTransaction,
        ObservedHostRootApparentRepositorySourceObservation,
        ObservedHostRootApparentRepositorySourcePathInput,
    ) {
        let source_key = HostRootApparentRepositorySourceInputKey::new(
            workspace.clone(),
            key.0.apparent_repo.clone(),
        )
        .unwrap();
        let _ = complete_local(dice, workspace, &source_key).await;
        let mut tx = dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(tracker),
                ..Default::default()
            })
            .commit()
            .await;
        for _ in 0..8 {
            match tx.compute(key).await.unwrap() {
                SourcePreparationOutcome::Complete(Ok(parent)) => {
                    let parent = parent.dupe();
                    let SourcePreparationOutcome::Complete(Ok(child)) =
                        tx.compute(child_key).await.unwrap()
                    else {
                        panic!("observed request path child must complete")
                    };
                    let global = tx.compute(&PathObservationEpochKey).await.unwrap();
                    assert_eq!(parent.observations(), child.observations());
                    for (demand, result) in parent.observations().observations() {
                        assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
                    }
                    return (tx, parent, child);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    panic!("unexpected observed outer: {error:?}")
                }
                SourcePreparationOutcome::Need(need) => {
                    let demands = need.path_observations().expect("request path observations");
                    let global = tx.compute(&PathObservationEpochKey).await.unwrap();
                    let supplied = demands.demands().iter().map(|demand| {
                        let target = demand
                            .path()
                            .as_path()
                            .ends_with(key.0.requested_path.as_path());
                        let result = match demand.operation() {
                            PathObservationOperation::Lstat => PathObservationResult::Lstat(
                                PathOperationResult::Present(PathLstat::new(
                                    if target {
                                        PathNodeKind::RegularFile
                                    } else {
                                        PathNodeKind::Directory
                                    },
                                    1,
                                    1,
                                    1,
                                    1,
                                    0o755,
                                )),
                            ),
                            PathObservationOperation::FileBytes => {
                                PathObservationResult::FileBytes(match bytes {
                                    Some(value) => PathOperationResult::Present(Arc::from(value)),
                                    None => PathOperationResult::Missing,
                                })
                            }
                            operation => panic!("unexpected request operation: {operation:?}"),
                        };
                        (demand.dupe(), Arc::new(result))
                    });
                    let epoch = PathObservationEpoch::from_shared(
                        global
                            .observations()
                            .iter()
                            .map(|(demand, result)| (demand.dupe(), result.dupe()))
                            .chain(supplied),
                    )
                    .unwrap();
                    let mut updater = tx.into_updater();
                    updater
                        .changed_to(vec![(PathObservationEpochKey, epoch)])
                        .unwrap();
                    tx = updater.commit().await;
                }
            }
        }
        panic!("request observation did not complete")
    }

    async fn assert_request_error_parity(workspace: &NormalizedAbsolutePath) {
        type E = HostRootApparentRepositorySourceObservationErrorKind;
        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let apparent = ApparentRepoName::new("local_alias").unwrap();
        let key = HostRootApparentRepositorySourceObservationObservationKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/missing.bzl".into(),
        )
        .unwrap();
        let child_key = HostRootApparentRepositorySourcePathInputObservationKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/missing.bzl".into(),
        )
        .unwrap();
        let legacy_key = HostRootApparentRepositorySourceObservationKey::new(
            workspace.clone(),
            apparent,
            "pkg/missing.bzl".into(),
        )
        .unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&dice, module, EXTENSION_A, true, None).await;
        let (mut tx, missing, _) = completed_request_state(
            &dice,
            workspace,
            &key,
            &child_key,
            Arc::new(Tracker::default()),
            None,
        )
        .await;
        assert!(
            matches!(missing.result().as_ref(), Err(error) if matches!(error.kind, E::Observation { .. }))
        );
        let SourcePreparationOutcome::Complete(direct) = tx.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy request error")
        };
        assert_eq!(direct.as_ref(), missing.result().as_ref());
    }

    async fn assert_cancellation_and_nonactivation(
        workspace: &NormalizedAbsolutePath,
        key: &HostRootApparentRepositorySourceObservationObservationKey,
        child_key: &HostRootApparentRepositorySourcePathInputObservationKey,
        expected: &ObservedHostRootApparentRepositorySourceObservation,
        expected_child: &ObservedHostRootApparentRepositorySourcePathInput,
    ) {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut cancelled =
            transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let mut future = Box::pin(cancelled.compute(key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            tracker
                .dependencies
                .lock()
                .unwrap()
                .iter()
                .all(|(name, _)| name != &key.to_string())
        );
        let recovery = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let (_, recovered, recovered_child) =
            completed_observed_state(recovery, key, child_key).await;
        assert_eq!(recovered.result(), expected.result());
        assert_eq!(recovered_child.result(), expected_child.result());
        assert_eq!(tracker.count("bridge"), 0);
        let trace = format!(
            "{:?}{:?}",
            *tracker.rich.lock().unwrap(),
            *tracker.dependencies.lock().unwrap()
        );
        for forbidden in ["root-repository-route:", "build-command-root:", "bootstrap"] {
            assert!(
                !trace.contains(forbidden),
                "forbidden activation: {forbidden}"
            );
        }
        for consumer in [
            include_str!("dice.rs"),
            include_str!("root_apparent_repository_route.rs"),
            include_str!("root_apparent_repository_source_input.rs"),
        ] {
            assert!(
                !consumer.contains("HostRootApparentRepositorySourceObservationObservationKey")
            );
        }
        let source = include_str!("root_apparent_repository_source_observation.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        let carrier = production
            .split("struct ObservedHostRootApparentRepositorySourceObservation")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(carrier.contains("result:") && carrier.contains("observations:"));
        for scratch in ["tracker", "event", "task", "lock", "view"] {
            assert!(!carrier.contains(scratch));
        }
        assert_eq!(workspace.as_path(), Path::new(WORKSPACE));
    }

    #[test]
    fn key_shape_and_production_edges_are_closed() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        assert!(
            HostRootApparentRepositorySourceObservationKey::new(
                workspace.clone(),
                ApparentRepoName::root(),
                "pkg/file.bzl".into(),
            )
            .is_none()
        );
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            ApparentRepoName::new("dep").unwrap(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        assert!(key.to_string().contains("pkg/file.bzl"));

        let source = include_str!("root_apparent_repository_source_observation.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches(".compute(").count(), 3);
        assert_eq!(
            production
                .matches("HostRootApparentRepositorySourcePathInputKey::new")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("HostRepositorySourceObservationKey::new")
                .count(),
            1
        );
        assert!(
            production
                .contains("predecessor: Arc<HostRootApparentRepositorySourcePathInputResult>")
        );
        assert!(
            production.contains("observation: Option<Arc<HostRepositorySourceObservationResult>>")
        );
        for forbidden in [
            ["HostRootApparentRepository", "RouteKey"].concat(),
            ["RepositoryMaterialization", "ResultKey"].concat(),
            ["RepositoryPackage", "LoadKey"].concat(),
            ["RepositorySource", "FileKey"].concat(),
            ["std::", "fs"].concat(),
            ["Evaluation", "Data"].concat(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "forbidden edge: {forbidden}"
            );
        }
    }

    #[tokio::test]
    async fn predecessor_need_and_terminal_complete_before_observation() {
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
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace.clone(),
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let _ = transaction(&dice, module, EXTENSION_A, true, None).await;
        let (outer_need, tracker) = tracked(&dice, &key).await;
        let path_need = dice
            .updater()
            .commit()
            .await
            .compute(&path_key)
            .await
            .unwrap();
        let (SourcePreparationOutcome::Need(outer_need), SourcePreparationOutcome::Need(path_need)) =
            (&outer_need, &path_need)
        else {
            unreachable!()
        };
        assert_eq!(outer_need, path_need);
        assert_eq!(tracker.count("observation"), 0);
        assert_eq!(*tracker.events.lock().unwrap(), 0);

        let generated = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut tx = transaction(&generated, MODULE, EXTENSION_A, true, None).await;
        validated(&mut tx).await;
        let apparent = ApparentRepoName::new("first").unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(&path_key).await.unwrap()
        else {
            unreachable!()
        };
        assert!(predecessor.as_ref().is_err());
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let (outcome, tracker) = tracked(&generated, &key).await;
        let SourcePreparationOutcome::Complete(outcome) = outcome else {
            unreachable!()
        };
        let HostRootApparentRepositorySourceObservationErrorKind::SourcePath {
            predecessor: retained,
        } = &outcome.as_ref().as_ref().unwrap_err().kind
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained, &predecessor));
        assert_eq!(tracker.count("observation"), 0);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn main_retains_predecessor_and_splits_compute_terminal_ownership() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("root_self").unwrap();
        let mut tx = transaction(
            &dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(&path_key).await.unwrap()
        else {
            unreachable!()
        };
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let (outcome, tracker) = tracked(&dice, &key).await;
        let certificate = value(&outcome);
        assert!(Arc::ptr_eq(&certificate.predecessor, &predecessor));
        assert!(certificate.observation.is_none());
        let view = certificate.view().unwrap();
        assert_eq!(view.apparent_repo().as_str(), "root_self");
        assert!(view.canonical_repo().is_root());
        assert_eq!(view.relative_path().as_path(), Path::new("pkg/file.bzl"));
        assert!(matches!(
            view.disposition(),
            HostRootApparentRepositorySourceObservationDispositionView::Main
        ));
        assert_eq!(tracker.count("observation"), 0);
        assert_eq!(*tracker.events.lock().unwrap(), 0);

        let SourcePreparationOutcome::Complete(source_compute) =
            key.compute_terminal(None, "path compute".into())
        else {
            unreachable!()
        };
        assert!(matches!(
            &source_compute.as_ref().as_ref().unwrap_err().kind,
            HostRootApparentRepositorySourceObservationErrorKind::SourcePathCompute {
                message
            } if message.as_ref() == "path compute"
        ));
        let SourcePreparationOutcome::Complete(observation_compute) =
            key.compute_terminal(Some(predecessor.clone()), "observation compute".into())
        else {
            unreachable!()
        };
        let HostRootApparentRepositorySourceObservationErrorKind::ObservationCompute {
            predecessor: retained,
            message,
        } = &observation_compute.as_ref().as_ref().unwrap_err().kind
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained, &predecessor));
        assert_eq!(message.as_ref(), "observation compute");

        let (warm, warm_tracker) = tracked(&dice, &key).await;
        assert!(HostRootApparentRepositorySourceObservationKey::equality(
            &outcome, &warm
        ));
        assert_eq!(
            &*warm_tracker.activations.lock().unwrap(),
            &[("bridge", ActivationKind::Reused)]
        );
        assert_eq!(warm_tracker.count("observation"), 0);
    }

    #[tokio::test]
    async fn builtin_retains_exact_arcs_and_fails_closed_on_corruption() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        prepare_builtin(&dice, &workspace).await;
        let apparent = ApparentRepoName::new("bazel_tools").unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "MODULE.bazel".into(),
        )
        .unwrap();
        let mut tx = dice.updater().commit().await;
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(&path_key).await.unwrap()
        else {
            unreachable!()
        };
        let path = predecessor.as_ref().as_ref().unwrap().view().unwrap();
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(input) =
            path.disposition()
        else {
            unreachable!()
        };
        let observation_key =
            HostRepositorySourceObservationKey::new(input.clone(), path.relative_path().clone());
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent,
            "MODULE.bazel".into(),
        )
        .unwrap();
        let (outcome, tracker) = tracked(&dice, &key).await;
        assert_eq!(tracker.count("observation"), 1);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        let certificate = value(&outcome);
        assert!(Arc::ptr_eq(&certificate.predecessor, &predecessor));
        let retained = certificate.observation.as_ref().unwrap();
        let SourcePreparationOutcome::Complete(direct) = dice
            .updater()
            .commit()
            .await
            .compute(&observation_key)
            .await
            .unwrap()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained, &direct));
        let HostRootApparentRepositorySourceObservationDispositionView::Input {
            input: retained_input,
            observation,
        } = certificate.view().unwrap().disposition()
        else {
            unreachable!()
        };
        assert_eq!(retained_input, input);
        assert!(matches!(
            observation.view(),
            HostRepositorySourceObservationView::Builtin(value)
                if value.path() == "MODULE.bazel" && !value.bytes().is_empty()
        ));

        let mut missing = certificate.clone();
        missing.observation = None;
        assert!(missing.view().is_none());
        let mut wrong_polarity = certificate.clone();
        wrong_polarity.observation = Some(Arc::new(Ok(HostRepositorySourceObservation::Request(
            HostRepositorySourceFileValue::Absent,
        ))));
        assert!(wrong_polarity.view().is_none());
    }

    #[tokio::test]
    async fn builtin_typed_terminal_retains_both_exact_arcs() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        prepare_builtin(&dice, &workspace).await;
        let apparent = ApparentRepoName::new("bazel_tools").unwrap();
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "not/in/catalog".into(),
        )
        .unwrap();
        let mut tx = dice.updater().commit().await;
        let SourcePreparationOutcome::Complete(predecessor) = tx.compute(&path_key).await.unwrap()
        else {
            unreachable!()
        };
        let path = predecessor.as_ref().as_ref().unwrap().view().unwrap();
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(input) =
            path.disposition()
        else {
            unreachable!()
        };
        let observation_key =
            HostRepositorySourceObservationKey::new(input.clone(), path.relative_path().clone());
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent,
            "not/in/catalog".into(),
        )
        .unwrap();
        let (outcome, tracker) = tracked(&dice, &key).await;
        assert_eq!(tracker.count("observation"), 1);
        let SourcePreparationOutcome::Complete(outcome) = outcome else {
            unreachable!()
        };
        let HostRootApparentRepositorySourceObservationErrorKind::Observation {
            predecessor: retained_predecessor,
            observation: retained_observation,
        } = &outcome.as_ref().as_ref().unwrap_err().kind
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained_predecessor, &predecessor));
        let SourcePreparationOutcome::Complete(direct) = dice
            .updater()
            .commit()
            .await
            .compute(&observation_key)
            .await
            .unwrap()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained_observation, &direct));
        let error = direct.as_ref().as_ref().unwrap_err();
        assert_eq!(error.input(), input);
        assert_eq!(error.relative_path().as_path(), Path::new("not/in/catalog"));
    }

    #[tokio::test]
    async fn request_first_need_and_clone_boundary_are_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\n\
            local_path_override(module_name='local', path='local')\n\
            bazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let apparent = ApparentRepoName::new("local_alias").unwrap();
        let source_key =
            HostRootApparentRepositorySourceInputKey::new(workspace.clone(), apparent.clone())
                .unwrap();
        let _ = transaction(&dice, module, EXTENSION_A, true, None).await;
        complete_local(&dice, &workspace, &source_key).await;
        let path_key = HostRootApparentRepositorySourcePathInputKey::new(
            workspace.clone(),
            apparent.clone(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let SourcePreparationOutcome::Complete(predecessor) = dice
            .updater()
            .commit()
            .await
            .compute(&path_key)
            .await
            .unwrap()
        else {
            unreachable!()
        };
        let path = predecessor.as_ref().as_ref().unwrap().view().unwrap();
        let HostRootApparentRepositorySourcePathInputDispositionView::Input(input) =
            path.disposition()
        else {
            unreachable!()
        };
        let cloned_input = input.clone();
        let cloned_path = path.relative_path().clone();
        let (
            HostRepositorySourceInputDispositionView::Request(request),
            HostRepositorySourceInputDispositionView::Request(cloned_request),
        ) = (
            input.view().disposition(),
            cloned_input.view().disposition(),
        )
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(request, cloned_request));
        let (
            HostRepositorySourceCapabilitySource::RepoSpec { repo_spec, .. },
            HostRepositorySourceCapabilitySource::RepoSpec {
                repo_spec: cloned_spec,
                ..
            },
        ) = (
            input.view().capability().source(),
            cloned_input.view().capability().source(),
        )
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(repo_spec, cloned_spec));
        assert!(Arc::ptr_eq(
            path.relative_path().path_arc(),
            cloned_path.path_arc()
        ));
        let observation_key = HostRepositorySourceObservationKey::new(cloned_input, cloned_path);
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace,
            apparent,
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let (outer_need, tracker) = tracked(&dice, &key).await;
        assert_eq!(tracker.count("observation"), 1);
        assert_eq!(*tracker.events.lock().unwrap(), 0);
        assert!(!HostRootApparentRepositorySourceObservationKey::validity(
            &outer_need
        ));
        assert!(!HostRootApparentRepositorySourceObservationKey::equality(
            &outer_need,
            &outer_need
        ));
        let direct_need = dice
            .updater()
            .commit()
            .await
            .compute(&observation_key)
            .await
            .unwrap();
        let (
            SourcePreparationOutcome::Need(outer_need),
            SourcePreparationOutcome::Need(direct_need),
        ) = (&outer_need, &direct_need)
        else {
            unreachable!()
        };
        assert_eq!(outer_need, direct_need);
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_observation_identity_staging_and_terminal_algebra()
     {
        type O = HostRootApparentRepositorySourceObservationObservationKey;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = |name| ApparentRepoName::new(name).unwrap();
        let observed = |name, path| O::new(workspace.clone(), apparent(name), path).unwrap();
        let key = observed("first", "pkg/file.bzl".into());
        let same = observed("first", "pkg/file.bzl".into());
        let other = observed("first", "pkg/other.bzl".into());
        let hash = |value: &O| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };
        let display = O::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            apparent("first"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        assert_eq!(
            display.to_string(),
            "observed-HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\"), requested_path: \"pkg/file.bzl\" }"
        );
        assert!(O::new(workspace.clone(), ApparentRepoName::root(), "x".into()).is_none());
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));
        let child_key = HostRootApparentRepositorySourcePathInputObservationKey::new(
            workspace.clone(),
            apparent("first"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (_, carrier, child) = completed_observed_state(tx, &key, &child_key).await;
        let complete = SourcePreparationOutcome::Complete(Ok(carrier.dupe()));
        assert!(O::validity(&complete));
        assert!(O::equality(&complete, &complete));
        let (finished, epoch) = finish_root_apparent_repository_source_observation(
            &key.0,
            child.result().clone(),
            None,
            child.observations().clone(),
        );
        let HostRootApparentRepositorySourceObservationErrorKind::SourcePath { predecessor } =
            &finished.as_ref().as_ref().unwrap_err().kind
        else {
            panic!("path semantic terminal")
        };
        assert!(Arc::ptr_eq(predecessor, child.result()));
        assert_eq!(&epoch, child.observations());
        assert_eq!(carrier.result().as_ref(), finished.as_ref());

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let need_key = observed("local_alias", "pkg/file.bzl".into());
        let mut need_tx = transaction(&need_dice, module, EXTENSION_A, true, None).await;
        let need = need_tx.compute(&need_key).await.unwrap();
        assert!(!O::validity(&need));
        assert!(!O::equality(&need, &need));

        let source = include_str!("root_apparent_repository_source_observation.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches("ObservationKey::new").count(), 3);
        assert!(production.contains("SourcePath(error)"));
        for absent in [
            "merge(",
            "OperationMismatch",
            "PathObservationEpochKey",
            "pub ",
        ] {
            assert!(
                !production.contains(absent),
                "forbidden producer shape: {absent}"
            );
        }
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_observation_real_order_events_and_parity() {
        type O = HostRootApparentRepositorySourceObservationObservationKey;
        type K = HostRootApparentRepositorySourceObservationKey;
        type E = HostRootApparentRepositorySourceObservationErrorKind;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = |name| ApparentRepoName::new(name).unwrap();
        let keys = |name, path: &str| {
            (
                O::new(workspace.clone(), apparent(name), path.into()).unwrap(),
                HostRootApparentRepositorySourcePathInputObservationKey::new(
                    workspace.clone(),
                    apparent(name),
                    path.into(),
                )
                .unwrap(),
                K::new(workspace.clone(), apparent(name), path.into()).unwrap(),
            )
        };

        let (key, child_key, legacy_key) = keys("first", "pkg/file.bzl");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (mut tx, generated, child) = completed_observed_state(tx, &key, &child_key).await;
        let E::SourcePath { predecessor } = &generated.result().as_ref().as_ref().unwrap_err().kind
        else {
            panic!("generated source-path terminal")
        };
        assert!(Arc::ptr_eq(predecessor, child.result()));
        let dependencies = tracker.dependencies.lock().unwrap();
        let row = dependencies
            .iter()
            .find(|(name, _)| name == &key.to_string())
            .unwrap();
        assert_eq!(row.1, [child_key.to_string()]);
        drop(dependencies);
        let SourcePreparationOutcome::Complete(legacy) = tx.compute(&legacy_key).await.unwrap()
        else {
            panic!("legacy generated terminal")
        };
        assert_eq!(legacy.as_ref(), generated.result().as_ref());
        assert_eq!(tracker.count("observation"), 0);

        let (main_key, main_child_key, main_legacy) = keys("root_self", "pkg/file.bzl");
        let main_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let main_tracker = Arc::new(Tracker::default());
        let main_tx = transaction(
            &main_dice,
            "module(name='bazel_tools', repo_name='root_self')\n",
            EXTENSION_A,
            true,
            Some(main_tracker.clone()),
        )
        .await;
        main_tracker.clear();
        let (mut main_tx, main, main_child) =
            completed_observed_state(main_tx, &main_key, &main_child_key).await;
        assert!(main.result().as_ref().is_ok());
        assert!(
            main.result()
                .as_ref()
                .as_ref()
                .unwrap()
                .observation
                .is_none()
        );
        let SourcePreparationOutcome::Complete(main_direct) =
            main_tx.compute(&main_legacy).await.unwrap()
        else {
            panic!("legacy main")
        };
        assert_eq!(main_direct.as_ref(), main.result().as_ref());
        assert_eq!(main_tracker.count("observation"), 0);

        let (builtin_key, builtin_child_key, builtin_legacy) = keys("bazel_tools", "MODULE.bazel");
        let builtin_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        prepare_builtin(&builtin_dice, &workspace).await;
        let builtin_tracker = Arc::new(Tracker::default());
        let builtin_tx = builtin_dice
            .updater_with_data(UserComputationData {
                activation_tracker: Some(builtin_tracker.clone()),
                ..Default::default()
            })
            .commit()
            .await;
        let (mut builtin_tx, builtin, _builtin_child) =
            completed_observed_state(builtin_tx, &builtin_key, &builtin_child_key).await;
        let builtin_value = builtin.result().as_ref().as_ref().unwrap();
        let builtin_observation = builtin_value.observation.as_ref().unwrap();
        assert!(matches!(
            builtin_value.view().unwrap().disposition(),
            HostRootApparentRepositorySourceObservationDispositionView::Input {
                observation,
                ..
            } if matches!(observation.view(), HostRepositorySourceObservationView::Builtin(_))
        ));
        let SourcePreparationOutcome::Complete(builtin_direct) =
            builtin_tx.compute(&builtin_legacy).await.unwrap()
        else {
            panic!("legacy builtin")
        };
        assert_eq!(builtin_direct.as_ref(), builtin.result().as_ref());
        let dependencies = builtin_tracker.dependencies.lock().unwrap();
        let row = dependencies
            .iter()
            .find(|(name, _)| name == &builtin_key.to_string())
            .unwrap();
        assert_eq!(row.1.len(), 2);
        assert_eq!(row.1[0], builtin_child_key.to_string());
        assert!(row.1[1].starts_with("host-repository-source-observation:"));
        drop(dependencies);

        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let (request_key, request_child_key, request_legacy) = keys("local_alias", "pkg/file.bzl");
        let request_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let request_tracker = Arc::new(Tracker::default());
        let _ = transaction(&request_dice, module, EXTENSION_A, true, None).await;
        let (mut request_tx, request, request_child) = completed_request_state(
            &request_dice,
            &workspace,
            &request_key,
            &request_child_key,
            request_tracker.clone(),
            Some(b"content"),
        )
        .await;
        assert!(matches!(
            request.result().as_ref().as_ref().unwrap().view().unwrap().disposition(),
            HostRootApparentRepositorySourceObservationDispositionView::Input {
                observation,
                ..
            } if matches!(observation.view(), HostRepositorySourceObservationView::Request(
                HostRepositorySourceFileValue::Present { bytes, .. }
            ) if bytes.as_ref() == b"content")
        ));
        let SourcePreparationOutcome::Complete(request_direct) =
            request_tx.compute(&request_legacy).await.unwrap()
        else {
            panic!("legacy request")
        };
        assert_eq!(request_direct.as_ref(), request.result().as_ref());

        assert_request_error_parity(&workspace).await;

        let (invalid_source, invalid_epoch) = finish_root_apparent_repository_source_observation(
            &request_key.0,
            request_child.result().clone(),
            None,
            request_child.observations().clone(),
        );
        assert_eq!(&invalid_epoch, request_child.observations());
        assert!(
            matches!(invalid_source.as_ref(), Err(error) if matches!(error.kind, E::InvalidSourcePath { .. }))
        );
        let (invalid_observation, main_epoch) = finish_root_apparent_repository_source_observation(
            &main_key.0,
            main_child.result().clone(),
            Some(builtin_observation.clone()),
            main_child.observations().clone(),
        );
        assert_eq!(&main_epoch, main_child.observations());
        assert!(
            matches!(invalid_observation.as_ref(), Err(error) if matches!(error.kind, E::InvalidObservation { .. }))
        );

        request_tracker.clear();
        let warm = request_tx.compute(&request_key).await.unwrap();
        assert!(Arc::ptr_eq(
            observed_value(&warm).result(),
            request.result()
        ));
        assert!(
            request_tracker
                .rich
                .lock()
                .unwrap()
                .iter()
                .all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none())
        );
        for (tracker, expected) in [tracker, main_tracker, builtin_tracker, request_tracker]
            .into_iter()
            .zip([8, 2, 0, 0])
        {
            assert_eq!(*tracker.events.lock().unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn observed_root_apparent_repository_source_observation_lifecycle_cancellation_and_nonactivation()
     {
        type O = HostRootApparentRepositorySourceObservationObservationKey;
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = |name| ApparentRepoName::new(name).unwrap();
        let key = O::new(workspace.clone(), apparent("first"), "pkg/file.bzl".into()).unwrap();
        let child_key = HostRootApparentRepositorySourcePathInputObservationKey::new(
            workspace.clone(),
            apparent("first"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let a_tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        tracker.clear();
        let (mut a_tx, a, a_child) = completed_observed_state(a_tx, &key, &child_key).await;
        tracker.clear();
        let warm = a_tx.compute(&key).await.unwrap();
        assert!(Arc::ptr_eq(observed_value(&warm).result(), a.result()));
        assert!(
            tracker
                .rich
                .lock()
                .unwrap()
                .iter()
                .all(|(_, kind, batch)| *kind == ActivationKind::Reused && batch.is_none())
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
            let tx = transaction(&dice, module, extension, true, None).await;
            let (_, changed, changed_child) = completed_observed_state(tx, &key, &child_key).await;
            assert_ne!(changed.result(), a.result());
            assert_ne!(changed_child.result(), a_child.result());
            let tx = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
            let (_, restored, restored_child) =
                completed_observed_state(tx, &key, &child_key).await;
            assert_eq!(restored.result(), a.result());
            assert_eq!(restored_child.result(), a_child.result());
            assert!(!Arc::ptr_eq(restored.result(), a.result()));
        }
        let neutral_module = format!("{MODULE}\n");
        let tx = transaction(&dice, &neutral_module, EXTENSION_A, true, None).await;
        let (_, neutral, neutral_child) = completed_observed_state(tx, &key, &child_key).await;
        assert_eq!(neutral.result(), a.result());
        assert_eq!(neutral_child.result(), a_child.result());
        assert_ne!(neutral.observations(), a.observations());
        assert_ne!(neutral, a);

        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_key = O::new(
            workspace.clone(),
            apparent("local_alias"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let local_child = HostRootApparentRepositorySourcePathInputObservationKey::new(
            workspace.clone(),
            apparent("local_alias"),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let local_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _ = transaction(&local_dice, module, EXTENSION_A, true, None).await;
        let (mut local_tx, local_a, _) = completed_request_state(
            &local_dice,
            &workspace,
            &local_key,
            &local_child,
            Arc::new(Tracker::default()),
            Some(b"a"),
        )
        .await;
        let content_epoch = |epoch: &PathObservationEpoch, bytes: &'static [u8]| {
            PathObservationEpoch::from_shared(epoch.observations().iter().map(
                |(demand, result)| {
                    let result = if demand.operation() == PathObservationOperation::FileBytes
                        && demand
                            .path()
                            .as_path()
                            .ends_with(local_key.0.requested_path.as_path())
                    {
                        Arc::new(PathObservationResult::FileBytes(
                            PathOperationResult::Present(Arc::from(bytes)),
                        ))
                    } else {
                        result.dupe()
                    };
                    (demand.dupe(), result)
                },
            ))
            .unwrap()
        };
        let global_a = local_tx.compute(&PathObservationEpochKey).await.unwrap();
        let mut updater = local_tx.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                content_epoch(&global_a, b"b"),
            )])
            .unwrap();
        let mut local_tx = updater.commit().await;
        let local_b = observed_value(&local_tx.compute(&local_key).await.unwrap()).dupe();
        assert_ne!(local_b.result(), local_a.result());
        assert_eq!(local_b.observations(), local_a.observations());
        let global_b = local_tx.compute(&PathObservationEpochKey).await.unwrap();
        let mut updater = local_tx.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                content_epoch(&global_b, b"a"),
            )])
            .unwrap();
        let mut local_tx = updater.commit().await;
        let local_restored = observed_value(&local_tx.compute(&local_key).await.unwrap()).dupe();
        assert_eq!(local_restored.result(), local_a.result());
        assert!(!Arc::ptr_eq(local_restored.result(), local_a.result()));

        let _ = transaction_with_command_override(
            &local_dice,
            "module(name='bazel_tools')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n",
            EXTENSION_A,
            "local",
        )
        .await;
        let (_, command, _) = completed_request_state(
            &local_dice,
            &workspace,
            &local_key,
            &local_child,
            Arc::new(Tracker::default()),
            Some(b"a"),
        )
        .await;
        assert_ne!(command.result(), local_a.result());
        let _ = transaction(&local_dice, module, EXTENSION_A, true, None).await;
        let (_, policy_restored, _) = completed_request_state(
            &local_dice,
            &workspace,
            &local_key,
            &local_child,
            Arc::new(Tracker::default()),
            Some(b"a"),
        )
        .await;
        assert_eq!(policy_restored.result(), local_a.result());

        assert_cancellation_and_nonactivation(&workspace, &key, &child_key, &a, &a_child).await;
    }
}
