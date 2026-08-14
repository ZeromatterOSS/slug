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
use slug_bzlmod_v2::HostRepositoryRelativePath;
use slug_bzlmod_v2::HostRepositoryRelativePathError;
use slug_bzlmod_v2::HostRepositorySourceInput;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::host_repository_relative_path;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputDispositionView;
use super::root_apparent_repository_source_input::HostRootApparentRepositorySourceInputKey;
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

fn complete(
    value: HostRootApparentRepositorySourcePathInputResult,
) -> HostRootApparentRepositorySourcePathInputOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

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

#[async_trait]
impl Key for HostRootApparentRepositorySourcePathInputKey {
    type Value = HostRootApparentRepositorySourcePathInputOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let relative_path = match host_repository_relative_path(self.requested_path.clone()) {
            Ok(relative_path) => relative_path,
            Err(error) => {
                return complete(Err(HostRootApparentRepositorySourcePathInputError {
                    workspace: self.workspace.clone(),
                    apparent_repo: self.apparent_repo.clone(),
                    kind: HostRootApparentRepositorySourcePathInputErrorKind::Path(error),
                }));
            }
        };
        let predecessor = match ctx
            .compute(
                &HostRootApparentRepositorySourceInputKey::new(
                    self.workspace.clone(),
                    self.apparent_repo.clone(),
                )
                .expect("source-path key rejects root apparent names"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(predecessor)) => predecessor,
            Err(error) => {
                return complete(Err(HostRootApparentRepositorySourcePathInputError {
                    workspace: self.workspace.clone(),
                    apparent_repo: self.apparent_repo.clone(),
                    kind: HostRootApparentRepositorySourcePathInputErrorKind::Compute {
                        relative_path,
                        message: error.to_string().into(),
                    },
                }));
            }
        };
        let source_view = predecessor
            .as_ref()
            .as_ref()
            .ok()
            .and_then(|source| source.view());
        let disposition = completed_source_disposition(predecessor.is_ok(), source_view.is_some());
        if disposition != CompletedSourceDisposition::Success {
            return complete(Err(completed_source_error(
                &self.workspace,
                &self.apparent_repo,
                relative_path,
                predecessor,
                disposition,
            )));
        }
        let certificate = HostRootApparentRepositorySourcePathInput {
            workspace: self.workspace.clone(),
            apparent_repo: self.apparent_repo.clone(),
            predecessor: predecessor.clone(),
            relative_path,
        };
        if certificate.view().is_none() {
            return complete(Err(HostRootApparentRepositorySourcePathInputError {
                workspace: self.workspace.clone(),
                apparent_repo: self.apparent_repo.clone(),
                kind: HostRootApparentRepositorySourcePathInputErrorKind::InvalidSource {
                    relative_path: certificate.relative_path,
                    predecessor,
                },
            }));
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
    use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_loading_v2::RepositoryPackageLoadKey;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::transaction_with_command_override;
    use super::super::generated_repository_definition::tests::validated;
    use super::super::root_apparent_repository_definition::tests::prepare_builtin;
    use super::super::root_apparent_repository_route::HostRootApparentRepositoryRouteKey;
    use super::super::root_apparent_repository_source_input::tests::complete_local;
    use super::super::root_apparent_repository_source_input::tests::corrupt_workspace;
    use super::super::root_apparent_repository_source_input::tests::value as source_value;
    use super::*;

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
            let path = key
                .downcast_ref::<HostRootApparentRepositorySourcePathInputKey>()
                .is_some();
            let source = key
                .downcast_ref::<HostRootApparentRepositorySourceInputKey>()
                .is_some();
            let route = key
                .downcast_ref::<HostRootApparentRepositoryRouteKey>()
                .is_some();
            if path || source || route {
                self.order.lock().unwrap().push((
                    if path {
                        "path"
                    } else if source {
                        "source"
                    } else {
                        "route"
                    },
                    activation.kind(),
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
        assert_eq!(production.matches(".compute(").count(), 1);
        assert_eq!(
            production.matches("host_repository_relative_path(").count(),
            1
        );
        assert!(production.contains("HostRootApparentRepositorySourceInputKey::new"));
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
