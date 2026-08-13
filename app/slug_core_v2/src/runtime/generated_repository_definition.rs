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
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::HostGeneratedRepositoryMapping;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecs;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecsError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesKey;
use slug_workspace_v2::NormalizedAbsolutePath;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinition {
    certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    ordinal: usize,
}

#[derive(Debug, Clone, Copy)]
struct HostGeneratedRepositoryDefinitionView<'a> {
    canonical_name: &'a CanonicalRepoName,
    internal_name: &'a str,
    repo_spec: &'a RepoSpec,
    mapping: HostGeneratedRepositoryMapping<'a>,
}

impl HostGeneratedRepositoryDefinition {
    fn view(&self) -> Option<HostGeneratedRepositoryDefinitionView<'_>> {
        self.certificate.iter().nth(self.ordinal).map(
            |(canonical_name, repo_spec, internal_name, mapping)| {
                HostGeneratedRepositoryDefinitionView {
                    canonical_name,
                    internal_name,
                    repo_spec,
                    mapping,
                }
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostGeneratedRepositoryDefinitionErrorKind {
    Loading(HostValidatedGeneratedRepositorySpecsError),
    LoadingCompute(Arc<str>),
    Missing {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    },
    Duplicate {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
        first: usize,
        conflicting: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinitionError {
    requested: CanonicalRepoName,
    kind: HostGeneratedRepositoryDefinitionErrorKind,
}

impl fmt::Display for HostGeneratedRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generated repository '{}': {:?}",
            self.requested, self.kind
        )
    }
}

impl std::error::Error for HostGeneratedRepositoryDefinitionError {}

type HostGeneratedRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostGeneratedRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

impl HostGeneratedRepositoryDefinitionKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostGeneratedRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-generated-repository-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

fn complete(
    value: Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>,
) -> HostGeneratedRepositoryDefinitionOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniqueOrdinalError {
    Missing,
    Duplicate { first: usize, conflicting: usize },
}

fn find_unique_ordinal<'a>(
    requested: &CanonicalRepoName,
    names: impl Iterator<Item = &'a CanonicalRepoName>,
) -> Result<usize, UniqueOrdinalError> {
    let mut first = None;
    let mut conflicting = None;
    for (ordinal, name) in names.enumerate() {
        if name != requested {
            continue;
        }
        if let Some(first) = first {
            conflicting.get_or_insert((first, ordinal));
        } else {
            first = Some(ordinal);
        }
    }
    match (first, conflicting) {
        (_, Some((first, conflicting))) => {
            Err(UniqueOrdinalError::Duplicate { first, conflicting })
        }
        (Some(first), None) => Ok(first),
        (None, None) => Err(UniqueOrdinalError::Missing),
    }
}

#[async_trait]
impl Key for HostGeneratedRepositoryDefinitionKey {
    type Value = HostGeneratedRepositoryDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let certificate = match ctx
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                self.workspace.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return complete(Err(HostGeneratedRepositoryDefinitionError {
                        requested: self.canonical_repo.clone(),
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(error.clone()),
                    }));
                }
            },
            Err(error) => {
                return complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(
                        error.to_string().into(),
                    ),
                }));
            }
        };

        match find_unique_ordinal(
            &self.canonical_repo,
            certificate.iter().map(|(canonical, _, _, _)| canonical),
        ) {
            Ok(ordinal) => complete(Ok(HostGeneratedRepositoryDefinition {
                certificate,
                ordinal,
            })),
            Err(UniqueOrdinalError::Missing) => {
                complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { certificate },
                }))
            }
            Err(UniqueOrdinalError::Duplicate { first, conflicting }) => {
                complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate {
                        certificate,
                        first,
                        conflicting,
                    },
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
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_loading_v2::HostValidatedGeneratedRepositorySpecsOutcome;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;

    const WORKSPACE: &str = "/generated-repository-definition";
    const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
    const EXTENSION_A: &str = r#"
repo=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;
    const EXTENSION_B: &str = r#"
other=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    other(name='first', value='one', target=':local')
    other(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    #[derive(Default)]
    struct LookupTracker {
        lookup: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
    }

    impl ActivationTracker for LookupTracker {
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
                .downcast_ref::<HostGeneratedRepositoryDefinitionKey>()
                .is_some()
            {
                self.lookup
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            } else if key.downcast_ref::<RepositoryPackageSourceKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
                || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
            {
                self.forbidden.lock().unwrap().push("source");
            }
        }
    }

    async fn transaction(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                        ),
                        (
                            workspace.as_path().join("ext.bzl"),
                            if extension_present {
                                WorkspaceFileValue::Present(Arc::new(extension.to_owned()))
                            } else {
                                WorkspaceFileValue::Absent
                            },
                        ),
                        (
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let observations = ["/", WORKSPACE]
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::Directory,
                        index as i64 + 1,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                )
            })
            .chain(
                ["REPO.bazel", ".bazelignore", "BUILD"]
                    .into_iter()
                    .map(|name| {
                        (
                            PathObservationDemand::new(
                                PathObservationNamespace::Host,
                                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                                PathObservationOperation::Lstat,
                            ),
                            PathObservationResult::Lstat(PathOperationResult::Missing),
                        )
                    }),
            )
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/BUILD.bazel")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    10,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(if extension_present {
                    PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        11,
                        1,
                        1,
                        1,
                        0o644,
                    ))
                } else {
                    PathOperationResult::Missing
                }),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(if extension_present {
                    PathOperationResult::Present(Arc::from(extension.as_bytes()))
                } else {
                    PathOperationResult::Missing
                }),
            )));
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations).unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    async fn validated(
        transaction: &mut dice::DiceTransaction,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    fn names(value: &HostValidatedGeneratedRepositorySpecsOutcome) -> Vec<CanonicalRepoName> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("validation must complete")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect()
    }

    async fn lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        requested: Option<&CanonicalRepoName>,
    ) -> HostGeneratedRepositoryDefinitionOutcome {
        let mut tx = transaction(dice, module, extension, true, None).await;
        let mut generated = names(&validated(&mut tx).await);
        let name = requested.cloned().unwrap_or_else(|| generated.remove(0));
        tx.compute(&HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            name,
        ))
        .await
        .unwrap()
    }

    fn snapshot(value: &HostGeneratedRepositoryDefinitionOutcome) -> Vec<String> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("lookup must complete")
        };
        let view = value.as_ref().as_ref().unwrap().view().unwrap();
        vec![
            view.canonical_name.as_str().to_owned(),
            view.internal_name.to_owned(),
            view.repo_spec.rule_id.rule_name.to_string(),
            format!("{:?}", view.repo_spec.attributes),
            view.mapping.context_repo().as_str().to_owned(),
            format!("{:?}", view.mapping.entries()),
        ]
    }

    #[test]
    fn complete_scan_rejects_missing_and_duplicate() {
        use std::cell::Cell;

        let requested = CanonicalRepoName::new("wanted").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        assert_eq!(
            find_unique_ordinal(&requested, [].iter()),
            Err(UniqueOrdinalError::Missing)
        );
        assert_eq!(
            find_unique_ordinal(&requested, [&other, &requested].into_iter()),
            Ok(1)
        );
        let consumed = Cell::new(0);
        let names = [&requested, &other, &requested, &other];
        assert_eq!(
            find_unique_ordinal(
                &requested,
                names
                    .into_iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            Err(UniqueOrdinalError::Duplicate {
                first: 0,
                conflicting: 2,
            })
        );
        assert_eq!(consumed.get(), names.len());
    }

    #[tokio::test]
    async fn real_lookup_borrows_exact_definition_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let validation = validated(&mut tx).await;
        let generated = names(&validation);
        assert_eq!(generated.len(), 2);
        tracker.forbidden.lock().unwrap().clear();

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let first_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        let second_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[1].clone());
        let first = tx.compute(&first_key).await.unwrap();
        let second = tx.compute(&second_key).await.unwrap();
        let warm = tx.compute(&first_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&first));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first, &warm
        ));

        let SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("lookup must complete")
        };
        let SourcePreparationOutcome::Complete(second_value) = &second else {
            panic!("lookup must complete")
        };
        let first_view = first_value.as_ref().as_ref().unwrap().view().unwrap();
        let second_view = second_value.as_ref().as_ref().unwrap().view().unwrap();
        assert_eq!(first_view.canonical_name, &generated[0]);
        assert_eq!(first_view.internal_name, "first");
        assert_eq!(first_view.repo_spec.rule_id.rule_name.as_str(), "repo");
        assert_eq!(second_view.internal_name, "second");
        assert!(matches!(
            second_view.repo_spec.attributes.get("value"),
            Some(slug_bzlmod_v2::OverrideAttributeValue::String(value)) if value == "two"
        ));
        assert!(std::ptr::eq(
            first_view.mapping.entries(),
            second_view.mapping.entries()
        ));
        assert_eq!(first_view.mapping.context_repo(), &generated[0]);
        assert_eq!(second_view.mapping.context_repo(), &generated[1]);

        let missing_key = HostGeneratedRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("missing").unwrap(),
        );
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { .. },
                        ..
                    })
                )
        ));

        let baseline = snapshot(&first);
        for (case, (module, extension, changed_fields)) in [
            (MODULE.to_owned(), EXTENSION_B.to_owned(), &[2][..]),
            (MODULE.replace("first='first'", "first='renamed'"), EXTENSION_A.replacen("name='first'", "name='renamed'", 1), &[0, 1, 4, 5]),
            (MODULE.to_owned(), EXTENSION_A.replace("value", "renamed_value"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one'", "value='changed'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one', target=':local'", "target=':local', value='one'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("target=':local'", "target=':changed'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("repo(name='first', value='one', target=':local')\n    repo(name='second', value='two', target='@first//:item')", "repo(name='second', value='two', target='@first//:item')\n    repo(name='first', value='one', target=':local')"), &[0, 1, 3, 4]),
        ]
        .into_iter()
        .enumerate()
        {
            let b = lookup(&dice, &module, &extension, None).await;
            assert!(!HostGeneratedRepositoryDefinitionKey::equality(&first, &b));
            let changed = snapshot(&b);
            assert!(changed_fields.iter().all(|index| baseline[*index] != changed[*index]), "case {case}: {baseline:?} == {changed:?}");
            let a2 = lookup(&dice, MODULE, EXTENSION_A, None).await;
            assert!(HostGeneratedRepositoryDefinitionKey::equality(&first, &a2));
        }

        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let mapping_a = lookup(&dice, &inject_a, EXTENSION_A, None).await;
        let mapping_b = lookup(&dice, &inject_b, EXTENSION_A, None).await;
        assert_ne!(snapshot(&mapping_a)[5], snapshot(&mapping_b)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &mapping_a,
            &lookup(&dice, &inject_a, EXTENSION_A, None).await,
        ));
        let overridden = lookup(
            &dice,
            &format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
            EXTENSION_A,
            None,
        )
        .await;
        assert_ne!(baseline[5], snapshot(&overridden)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first,
            &lookup(&dice, MODULE, EXTENSION_A, None).await,
        ));

        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let request_a = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let request_b = "module(name='bazel_tools')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\n";
        let order_a = lookup(&dice, request_a, &multi_extension, None).await;
        let fixed = CanonicalRepoName::new(&snapshot(&order_a)[0]).unwrap();
        let order_b = lookup(&dice, request_b, &multi_extension, Some(&fixed)).await;
        assert_eq!(&snapshot(&order_a)[..2], &snapshot(&order_b)[..2]);
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &order_a, &order_b
        ));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &order_a,
            &lookup(&dice, request_a, &multi_extension, Some(&fixed)).await,
        ));
        assert_eq!(
            *tracker.lookup.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Evaluated, false),
            ]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn predecessor_need_and_error_precede_lookup() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let key = HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
        );

        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let need = updater.commit().await.compute(&key).await.unwrap();
        assert!(!HostGeneratedRepositoryDefinitionKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &need, &need
        ));
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));

        let mut missing_source = transaction(&dice, MODULE, EXTENSION_A, false, None).await;
        let terminal = missing_source.compute(&key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&terminal));
        assert!(matches!(
            terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                        ..
                    })
                )
        ));
    }
}
