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
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
use slug_bzlmod_v2::HostRepositorySourceObservation;
use slug_bzlmod_v2::HostRepositorySourceObservationKey;
use slug_bzlmod_v2::HostRepositorySourceObservationResult;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputDispositionView;
use super::root_apparent_repository_source_path_input::HostRootApparentRepositorySourcePathInputKey;
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

fn complete(
    value: HostRootApparentRepositorySourceObservationResult,
) -> HostRootApparentRepositorySourceObservationOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostRootApparentRepositorySourceObservationKey {
    type Value = HostRootApparentRepositorySourceObservationOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let predecessor = match ctx
            .compute(
                &HostRootApparentRepositorySourcePathInputKey::new(
                    self.workspace.clone(),
                    self.apparent_repo.clone(),
                    self.requested_path.clone(),
                )
                .expect("source-observation key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(predecessor)) => predecessor,
            Err(error) => {
                return self.compute_terminal(None, error.to_string().into());
            }
        };

        if predecessor.as_ref().is_err() {
            return self.terminal(
                HostRootApparentRepositorySourceObservationErrorKind::SourcePath { predecessor },
            );
        }
        let Some(predecessor_view) = predecessor
            .as_ref()
            .as_ref()
            .ok()
            .and_then(|value| value.view())
        else {
            return self.terminal(
                HostRootApparentRepositorySourceObservationErrorKind::InvalidSourcePath {
                    predecessor,
                },
            );
        };
        let observation_input = match predecessor_view.disposition() {
            HostRootApparentRepositorySourcePathInputDispositionView::Main => None,
            HostRootApparentRepositorySourcePathInputDispositionView::Input(input) => {
                Some((input.clone(), predecessor_view.relative_path().clone()))
            }
        };

        let Some((input, relative_path)) = observation_input else {
            let certificate = HostRootApparentRepositorySourceObservation {
                predecessor: predecessor.clone(),
                observation: None,
            };
            return if certificate.view().is_some() {
                complete(Ok(certificate))
            } else {
                self.terminal(
                    HostRootApparentRepositorySourceObservationErrorKind::InvalidSourcePath {
                        predecessor,
                    },
                )
            };
        };

        let observation = match ctx
            .compute(&HostRepositorySourceObservationKey::new(
                input,
                relative_path,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(observation)) => observation,
            Err(error) => {
                return self.compute_terminal(Some(predecessor), error.to_string().into());
            }
        };
        if observation.as_ref().is_err() {
            return self.terminal(
                HostRootApparentRepositorySourceObservationErrorKind::Observation {
                    predecessor,
                    observation,
                },
            );
        }

        let certificate = HostRootApparentRepositorySourceObservation {
            predecessor: predecessor.clone(),
            observation: Some(observation.clone()),
        };
        if certificate.view().is_none() {
            return self.terminal(
                HostRootApparentRepositorySourceObservationErrorKind::InvalidObservation {
                    predecessor,
                    observation,
                },
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
mod tests {
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
    use slug_bzlmod_v2::HostRepositorySourceCapabilitySource;
    use slug_bzlmod_v2::HostRepositorySourceFileValue;
    use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
    use slug_bzlmod_v2::HostRepositorySourceObservationView;
    use slug_workspace_v2::PathObservationEpoch;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::transaction;
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
        events: Mutex<usize>,
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
            let name = if key
                .downcast_ref::<HostRootApparentRepositorySourceObservationKey>()
                .is_some()
            {
                Some("bridge")
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
                self.activations
                    .lock()
                    .unwrap()
                    .push((name, activation.kind()));
                *self.events.lock().unwrap() += usize::from(activation.evaluation_data().is_some());
            }
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
        assert_eq!(production.matches(".compute(").count(), 2);
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
}
