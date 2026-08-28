/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceTransaction;
use dice::DynKey;
use dice::Key;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::StarlarkOptions;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationDiagnosticLevel;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
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
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::small_map::SmallMap;
use starlark_map::sorted_map::SortedMap;

use crate::CommandRegistrationExpansionKey;
use crate::CommandRegistrationExpansionObservationKey;
use crate::LoadingPreparationOutcome;
use crate::ModuleRegistrationExpansionErrorKind;
use crate::ModuleRegistrationExpansionKey;
use crate::ModuleRegistrationExpansionObservationKey;
use crate::ModuleRegistrationFamily;
use crate::cycle_detector::bzl_load_cycle_detector;
use crate::registration_expansion::package_postorder;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

fn default_configuration() -> SlugConfiguration {
    SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap(),
    )
    .unwrap()
}

fn command_configuration(
    occurrences: impl IntoIterator<Item = CommandConfigurationOccurrence>,
) -> SlugConfiguration {
    let overlay = CommandConfigurationOverlay::from(occurrences.into_iter().collect::<Vec<_>>());
    default_configuration()
        .with_command_configuration(StarlarkOptions::default(), &overlay)
        .unwrap()
}

#[derive(Debug)]
struct TrackedActivation {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct ExpansionTracker {
    dependencies: Mutex<Vec<(String, Vec<String>)>>,
    activations: Mutex<Vec<TrackedActivation>>,
}

impl ExpansionTracker {
    fn dependencies(&self, key: &str) -> Vec<String> {
        self.dependencies
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(candidate, dependencies)| (candidate == key).then(|| dependencies.clone()))
            .unwrap_or_default()
    }

    fn take(&self) -> Vec<TrackedActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for ExpansionTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        self.dependencies
            .lock()
            .unwrap()
            .push((key.to_string(), deps.map(ToString::to_string).collect()));
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.activations.lock().unwrap().push(TrackedActivation {
            key: key.to_string(),
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

#[derive(Default)]
struct EpochBuilder {
    entries: SmallMap<PathObservationDemand, PathObservationResult>,
}

impl EpochBuilder {
    fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    }

    fn node(&mut self, path: &str, kind: PathNodeKind, variant: i64) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            ))),
        );
    }

    fn directory(&mut self, path: &str, variant: i64) {
        self.node(path, PathNodeKind::Directory, variant);
    }

    fn file(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
        self.node(path, PathNodeKind::RegularFile, variant);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_ref(),
            ))),
        );
    }

    fn missing(&mut self, path: &str) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }

    fn listing(&mut self, path: &str, entries: &[&str]) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::DirectoryEntries),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new(entries.iter().map(|name| {
                    PathDirectoryEntry::new(
                        PathDirectoryName::new(OsString::from(name)).unwrap(),
                        PathDirectoryEntryKind::Directory,
                    )
                })),
            )),
        );
    }

    fn root_package(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::default();
        epoch.directory("/", variant);
        epoch.directory("/workspace", variant);
        epoch.file("/workspace/MODULE.bazel", module, variant);
        epoch.missing("/workspace/MODULE.bazel.lock");
        epoch.missing("/workspace/REPO.bazel");
        epoch.missing("/workspace/.bazelignore");
        epoch.directory("/workspace/pkg", variant);
        epoch.file("/workspace/pkg/BUILD.bazel", build, variant);
        PathObservationEpoch::new(epoch.entries).unwrap()
    }

    fn canonical_package(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        Self::canonical_package_with_dep_module(
            module,
            "module(name = 'dep', version = '1.0.0')\n",
            build,
            variant,
        )
    }

    fn canonical_package_with_dep_module(
        module: &str,
        dep_module: &str,
        build: &str,
        variant: i64,
    ) -> PathObservationEpoch {
        let mut epoch = Self::root_package_builder(module, variant);
        epoch.directory("/workspace/dep", variant);
        epoch.file("/workspace/dep/MODULE.bazel", dep_module, variant);
        epoch.missing("/workspace/dep/REPO.bazel");
        epoch.missing("/workspace/dep/.bazelignore");
        epoch.file("/workspace/dep/BUILD.bazel", build, variant);
        epoch.build()
    }

    fn canonical_tree(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::root_package_builder(module, variant);
        epoch.directory("/workspace/dep", variant);
        epoch.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = 'dep', version = '1.0.0')\n",
            variant,
        );
        epoch.missing("/workspace/dep/REPO.bazel");
        epoch.missing("/workspace/dep/.bazelignore");
        for path in [
            "/workspace/dep/tree",
            "/workspace/dep/tree/b",
            "/workspace/dep/tree/b/deep",
            "/workspace/dep/tree/c",
        ] {
            epoch.directory(path, variant);
            epoch.file(&format!("{path}/BUILD.bazel"), build, variant);
        }
        epoch.listing("/workspace/dep/tree", &["c", "b"]);
        epoch.listing("/workspace/dep/tree/b", &["deep"]);
        epoch.listing("/workspace/dep/tree/b/deep", &[]);
        epoch.listing("/workspace/dep/tree/c", &[]);
        epoch.build()
    }

    fn root_tree(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::root_package_builder(module, variant);
        for path in [
            "/workspace/tree",
            "/workspace/tree/b",
            "/workspace/tree/b/deep",
            "/workspace/tree/c",
        ] {
            epoch.directory(path, variant);
            epoch.file(&format!("{path}/BUILD.bazel"), build, variant);
        }
        epoch.listing("/workspace/tree", &["c", "b"]);
        epoch.listing("/workspace/tree/b", &["deep"]);
        epoch.listing("/workspace/tree/b/deep", &[]);
        epoch.listing("/workspace/tree/c", &[]);
        PathObservationEpoch::new(epoch.entries).unwrap()
    }

    fn root_package_builder(module: &str, variant: i64) -> Self {
        let mut epoch = Self::default();
        epoch.directory("/", variant);
        epoch.directory("/workspace", variant);
        epoch.file("/workspace/MODULE.bazel", module, variant);
        epoch.missing("/workspace/MODULE.bazel.lock");
        epoch.missing("/workspace/REPO.bazel");
        epoch.missing("/workspace/.bazelignore");
        epoch
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

async fn transaction(module: &str, epoch: PathObservationEpoch) -> DiceTransaction {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    transaction_on(&dice, module, epoch, false, None).await
}

async fn transaction_with_tracker(
    module: &str,
    epoch: PathObservationEpoch,
    capture_events: bool,
    tracker: Option<Arc<ExpansionTracker>>,
) -> DiceTransaction {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    transaction_on(&dice, module, epoch, capture_events, tracker).await
}

async fn transaction_on(
    dice: &Arc<Dice>,
    module: &str,
    epoch: PathObservationEpoch,
    capture_events: bool,
    tracker: Option<Arc<ExpansionTracker>>,
) -> DiceTransaction {
    let mut data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    if capture_events {
        data.data.set(CaptureEvaluationEvents);
    }
    let mut updater = dice.updater_with_data(data);
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace().as_path().to_owned(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel"),
                    WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace().as_path().to_owned(),
            },
            Arc::new(WorkspaceRawSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel.lock"),
                    WorkspaceRawFileValue::Absent,
                )])),
            }),
        )])
        .unwrap();
    let mut attributes = SmallMap::new();
    attributes.insert("path".into(), OverrideAttributeValue::String("dep".into()));
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(attributes),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
        },
    });
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            RepositoryMaterializationResultEpoch::new(
                workspace(),
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
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace(),
            [workspace()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace().as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater.commit().await
}

async fn update_module_epoch(
    transaction: DiceTransaction,
    module: &str,
    epoch: PathObservationEpoch,
) -> DiceTransaction {
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace().as_path().to_owned(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    PathBuf::from("/workspace/MODULE.bazel"),
                    WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                )])),
            }),
        )])
        .unwrap();
    updater.commit().await
}

fn labels(value: &Arc<crate::ModuleRegistrationExpansion>) -> Vec<String> {
    value
        .labels()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn activation_for<'a>(
    activations: &'a [TrackedActivation],
    key: &impl ToString,
) -> &'a TrackedActivation {
    let key = key.to_string();
    activations
        .iter()
        .find(|activation| activation.key == key)
        .unwrap_or_else(|| panic!("missing activation for {key}: {activations:#?}"))
}

fn assert_no_family_activation(
    activations: &[TrackedActivation],
    family: ModuleRegistrationFamily,
) {
    assert!(
        activations.iter().all(|activation| {
            !activation.key.contains("module-registration-expansion:")
                || !activation.key.ends_with(&family.to_string())
        }),
        "unexpected {family} activation: {activations:#?}"
    );
}

fn assert_no_activation_containing(activations: &[TrackedActivation], fragment: &str) {
    assert!(
        activations
            .iter()
            .all(|activation| !activation.key.contains(fragment)),
        "unexpected activation containing {fragment}: {activations:#?}"
    );
}

fn root_epoch_with_missing(
    module: &str,
    build: &str,
    missing: &str,
    variant: i64,
) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::root_package_builder(module, variant);
    epoch.directory("/workspace/pkg", variant);
    epoch.file("/workspace/pkg/BUILD.bazel", build, variant);
    epoch.missing(missing);
    epoch.build()
}

#[test]
fn recursive_package_order_is_lexical_sibling_postorder() {
    let mut packages = ["tree", "tree/c", "tree/b/deep", "tree/b", "z", ""]
        .map(|value| PackagePath::parse(value).unwrap());
    packages.sort_unstable_by(package_postorder);
    assert_eq!(
        packages.map(|package| package.as_str().to_owned()),
        ["tree/b/deep", "tree/b", "tree/c", "tree", "z", ""]
    );
    assert_eq!(
        package_postorder(
            &PackagePath::parse("same").unwrap(),
            &PackagePath::parse("same").unwrap()
        ),
        Ordering::Equal
    );
}

#[test]
fn family_key_identity_is_independent() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let toolchains = ModuleRegistrationExpansionKey::toolchains(workspace.clone());
    let platforms = ModuleRegistrationExpansionKey::execution_platforms(workspace);
    assert_eq!(toolchains.family(), ModuleRegistrationFamily::Toolchains);
    assert_eq!(
        platforms.family(),
        ModuleRegistrationFamily::ExecutionPlatforms
    );
    assert_ne!(toolchains, platforms);
    assert_ne!(toolchains.to_string(), platforms.to_string());
}

#[test]
fn command_key_identity_includes_configuration_and_family() {
    let empty = default_configuration();
    let configured =
        command_configuration([CommandConfigurationOccurrence::extra_toolchains("//pkg:tc")]);
    let empty_toolchains = CommandRegistrationExpansionKey::toolchains(workspace(), empty.clone());
    let configured_toolchains =
        CommandRegistrationExpansionKey::toolchains(workspace(), configured.clone());
    let configured_platforms =
        CommandRegistrationExpansionKey::execution_platforms(workspace(), configured);
    assert_ne!(empty_toolchains, configured_toolchains);
    assert_ne!(configured_toolchains, configured_platforms);
    assert_eq!(
        configured_toolchains.family(),
        ModuleRegistrationFamily::Toolchains
    );
    assert_eq!(
        configured_platforms.family(),
        ModuleRegistrationFamily::ExecutionPlatforms
    );
}

#[tokio::test]
async fn command_rows_apply_signed_order_and_family_specific_reversal() {
    let module = "module(name = 'bazel_tools')\n";
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'ta', toolchain_type = ':type', toolchain = ':impl')\n",
        "toolchain(name = 'tb', toolchain_type = ':type', toolchain = ':impl')\n",
        "platform(name = 'pa')\n",
        "platform(name = 'pb')\n",
    );
    let configuration = command_configuration([
        CommandConfigurationOccurrence::extra_toolchains("//pkg:ta,//pkg:tb"),
        CommandConfigurationOccurrence::extra_execution_platforms(
            "//pkg:pa,//pkg:pb,-//pkg:pa,//pkg:pa",
        ),
    ]);
    let mut tx = transaction(module, EpochBuilder::root_package(module, build, 50)).await;
    let toolchains = tx
        .compute(&CommandRegistrationExpansionKey::toolchains(
            workspace(),
            configuration.clone(),
        ))
        .await
        .unwrap();
    let platforms = tx
        .compute(&CommandRegistrationExpansionKey::execution_platforms(
            workspace(),
            configuration,
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(toolchains) = toolchains else {
        panic!("command toolchain expansion returned Need")
    };
    let LoadingPreparationOutcome::Complete(platforms) = platforms else {
        panic!("command execution-platform expansion returned Need")
    };
    assert_eq!(labels(&toolchains), ["@@//pkg:tb", "@@//pkg:ta"]);
    assert_eq!(labels(&platforms), ["@@//pkg:pb", "@@//pkg:pa"]);
}

#[tokio::test]
async fn command_recursive_negative_removal_and_reinsertion_reuse_package_walker() {
    let module = "module(name = 'bazel_tools')\n";
    let build = "platform(name = 'platform')\n";
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "//tree/...:all,-//tree/b/...:all,//tree/b:all",
        )]);
    let mut tx = transaction(module, EpochBuilder::root_tree(module, build, 51)).await;
    let outcome = tx
        .compute(&CommandRegistrationExpansionKey::execution_platforms(
            workspace(),
            configuration,
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("signed recursive command expansion returned Need")
    };
    assert_eq!(
        labels(&value),
        [
            "@@//tree/c:platform",
            "@@//tree:platform",
            "@@//tree/b:platform",
        ]
    );
}

#[tokio::test]
async fn command_apparent_external_rows_use_final_root_mapping() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
    );
    let build = "platform(name = 'platform')\n";
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "@dep//:platform",
        )]);
    let mut tx = transaction(module, EpochBuilder::canonical_package(module, build, 52)).await;
    let outcome = tx
        .compute(&CommandRegistrationExpansionKey::execution_platforms(
            workspace(),
            configuration,
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("mapped command expansion returned Need")
    };
    assert_eq!(labels(&value), ["@@dep+//:platform"]);
}

#[tokio::test]
async fn command_apparent_external_recursive_rows_reuse_canonical_subtree_walker() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
    );
    let build = "platform(name = 'platform')\n";
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "@dep//tree/...:all",
        )]);
    let mut tx = transaction(module, EpochBuilder::canonical_tree(module, build, 57)).await;
    let outcome = tx
        .compute(&CommandRegistrationExpansionKey::execution_platforms(
            workspace(),
            configuration,
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("mapped recursive command expansion returned Need")
    };
    assert_eq!(
        labels(&value),
        [
            "@@dep+//tree/b/deep:platform",
            "@@dep+//tree/b:platform",
            "@@dep+//tree/c:platform",
            "@@dep+//tree:platform",
        ]
    );
}

#[tokio::test]
async fn observed_command_mapping_route_and_package_frontiers_are_exact_and_ordered() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
    );
    let build = "platform(name = 'platform')\n";
    let epoch = EpochBuilder::canonical_package(module, build, 54);
    let expected_epoch = epoch.dupe();
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "@dep//:platform",
        )]);
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key =
        CommandRegistrationExpansionObservationKey::execution_platforms(workspace(), configuration);
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("observed mapped command expansion did not complete: {outcome:?}")
    };
    assert_eq!(labels(observed.result()), ["@@dep+//:platform"]);
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }
    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 3, "dependencies: {dependencies:#?}");
    assert!(dependencies[0].starts_with("observed-host-root-repository-mapping:"));
    assert!(dependencies[1].starts_with("observed-host-canonical-repository-load-route:"));
    assert!(dependencies[2].starts_with("observed-repository-package-inventory:"));
    let activations = tracker.take();
    assert!(
        activations.iter().all(|activation| {
            !activation.key.contains("command-registration-expansion:")
                || !activation.key.ends_with(":toolchains")
        }),
        "unexpected command toolchain activation: {activations:#?}"
    );
}

#[tokio::test]
async fn command_configuration_a_b_a_restores_equal_arc_backed_expansion() {
    let module = "module(name = 'bazel_tools')\n";
    let build = "platform(name = 'a')\nplatform(name = 'b')\n";
    let a_configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "//pkg:a",
        )]);
    let b_configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "//pkg:b",
        )]);
    let a_key = CommandRegistrationExpansionObservationKey::execution_platforms(
        workspace(),
        a_configuration,
    );
    let b_key = CommandRegistrationExpansionObservationKey::execution_platforms(
        workspace(),
        b_configuration,
    );
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        EpochBuilder::root_package(module, build, 55),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let a = tx.compute(&a_key).await.unwrap();
    let b = tx.compute(&b_key).await.unwrap();
    assert!(!CommandRegistrationExpansionObservationKey::equality(
        &a, &b
    ));
    tracker.take();
    let restored = tx.compute(&a_key).await.unwrap();
    assert!(CommandRegistrationExpansionObservationKey::equality(
        &a, &restored
    ));
    let (
        LoadingPreparationOutcome::Complete(Ok(a)),
        LoadingPreparationOutcome::Complete(Ok(restored)),
    ) = (&a, &restored)
    else {
        panic!("A/B/A command expansions did not complete")
    };
    assert!(Arc::ptr_eq(a.result(), restored.result()));
    assert_eq!(
        activation_for(&tracker.take(), &a_key).kind,
        ActivationKind::Reused
    );
}

#[tokio::test]
async fn observed_command_cancellation_publishes_nothing_then_recovers_and_reuses() {
    let module = "module(name = 'bazel_tools')\n";
    let build = "platform(name = 'platform')\n";
    let epoch = EpochBuilder::root_package(module, build, 56);
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "//pkg:platform",
        )]);
    let key =
        CommandRegistrationExpansionObservationKey::execution_platforms(workspace(), configuration);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(ExpansionTracker::default());
    let mut cancelled =
        transaction_on(&dice, module, epoch.dupe(), true, Some(tracker.dupe())).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(tracker.take().iter().all(|row| row.key != key.to_string()));
    assert!(tracker.dependencies(&key.to_string()).is_empty());
    drop(cancelled);

    let mut recovered = transaction_on(&dice, module, epoch, true, Some(tracker.dupe())).await;
    let first = recovered.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(first_value)) = &first else {
        panic!("command cancellation recovery did not complete: {first:?}")
    };
    assert_eq!(labels(first_value.result()), ["@@//pkg:platform"]);
    tracker.take();
    let warm = recovered.compute(&key).await.unwrap();
    assert!(CommandRegistrationExpansionObservationKey::equality(
        &first, &warm
    ));
    let LoadingPreparationOutcome::Complete(Ok(warm_value)) = &warm else {
        panic!("warm command recovery did not complete")
    };
    assert!(Arc::ptr_eq(first_value.result(), warm_value.result()));
    assert_eq!(
        activation_for(&tracker.take(), &key).kind,
        ActivationKind::Reused
    );
}

#[tokio::test]
async fn empty_command_list_activates_no_mapping_or_package_owner() {
    let module = "module(name = 'bazel_tools')\n";
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        PathObservationEpoch::new([]).unwrap(),
        false,
        Some(tracker.dupe()),
    )
    .await;
    let key = CommandRegistrationExpansionObservationKey::toolchains(
        workspace(),
        default_configuration(),
    );
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = outcome else {
        panic!("empty command expansion did not complete")
    };
    assert!(labels(observed.result()).is_empty());
    let activations = tracker.take();
    assert_no_activation_containing(&activations, "root-repository-mapping");
    assert_no_activation_containing(&activations, "package-load");
    assert_no_activation_containing(&activations, "package-inventory");
}

#[tokio::test]
async fn negative_exact_missing_target_remains_a_typed_row_error() {
    let module = "module(name = 'bazel_tools')\n";
    let configuration =
        command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
            "-//pkg:missing",
        )]);
    let mut tx = transaction(
        module,
        EpochBuilder::root_package(module, "platform(name = 'present')\n", 53),
    )
    .await;
    let outcome = tx
        .compute(&CommandRegistrationExpansionKey::execution_platforms(
            workspace(),
            configuration,
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("negative missing target returned Need")
    };
    let error = value.labels().unwrap_err();
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::MissingTarget(label)
            if label.to_string() == "@@//pkg:missing"
    ));
}

#[tokio::test]
async fn root_exact_and_wildcard_expansion_are_family_specific_and_stably_deduped() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:all', '//pkg:fake', '//pkg:all')\n",
        "register_execution_platforms('//pkg:*')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "filegroup(name = 'fake')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'tc', toolchain_type = ':type', toolchain = ':impl')\n",
        "platform(name = 'platform')\n",
        "alias(name = 'platform_alias', actual = ':platform')\n",
    );
    let mut tx = transaction(module, EpochBuilder::root_package(module, build, 1)).await;
    let toolchains = tx
        .compute(&ModuleRegistrationExpansionKey::toolchains(workspace()))
        .await
        .unwrap();
    let platforms = tx
        .compute(&ModuleRegistrationExpansionKey::execution_platforms(
            workspace(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(toolchains) = toolchains else {
        panic!("root toolchain expansion returned Need")
    };
    let LoadingPreparationOutcome::Complete(platforms) = platforms else {
        panic!("root execution-platform expansion returned Need")
    };
    assert_eq!(labels(&toolchains), ["@@//pkg:tc", "@@//pkg:fake"]);
    assert_eq!(
        labels(&platforms),
        ["@@//pkg:platform", "@@//pkg:platform_alias"]
    );
    assert!(toolchains.ambiguities().is_empty());
    assert!(platforms.ambiguities().is_empty());
}

#[tokio::test]
async fn mapped_and_canonical_rows_use_selected_nonroot_inventory() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
        "register_toolchains('@dep//:all', '@@dep+//:fake')\n",
        "register_execution_platforms('@dep//:*')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "filegroup(name = 'fake')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'tc', toolchain_type = ':type', toolchain = ':impl')\n",
        "platform(name = 'platform')\n",
        "alias(name = 'platform_alias', actual = ':platform')\n",
    );
    let epoch = EpochBuilder::canonical_package(module, build, 2);
    let mut tx = transaction(module, epoch).await;
    let toolchains = tx
        .compute(&ModuleRegistrationExpansionKey::toolchains(workspace()))
        .await
        .unwrap();
    let platforms = tx
        .compute(&ModuleRegistrationExpansionKey::execution_platforms(
            workspace(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(toolchains) = toolchains else {
        panic!("canonical toolchain expansion returned Need")
    };
    let LoadingPreparationOutcome::Complete(platforms) = platforms else {
        panic!("canonical execution-platform expansion returned Need")
    };
    assert_eq!(labels(&toolchains), ["@@dep+//:tc", "@@dep+//:fake"]);
    assert_eq!(
        labels(&platforms),
        ["@@dep+//:platform", "@@dep+//:platform_alias"]
    );
}

#[tokio::test]
async fn selected_module_and_declaration_order_drive_first_seen_deduplication() {
    let root_module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
        "register_toolchains('@dep//:second', '@dep//:overlap')\n",
    );
    let dep_module = concat!(
        "module(name = 'dep', version = '1.0.0')\n",
        "register_toolchains('//:overlap', '//:first')\n",
    );
    let build = concat!(
        "filegroup(name = 'first')\n",
        "filegroup(name = 'overlap')\n",
        "filegroup(name = 'second')\n",
    );
    let epoch = EpochBuilder::canonical_package_with_dep_module(root_module, dep_module, build, 31);
    let mut tx = transaction(root_module, epoch).await;
    let outcome = tx
        .compute(&ModuleRegistrationExpansionKey::toolchains(workspace()))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("multi-module registration expansion returned Need")
    };
    assert_eq!(
        labels(&value),
        ["@@dep+//:second", "@@dep+//:overlap", "@@dep+//:first"]
    );
}

#[tokio::test]
async fn recursive_expansion_uses_descendant_before_prefix_and_lexical_targets() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//tree/...:all')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'z', toolchain_type = ':type', toolchain = ':impl')\n",
        "toolchain(name = 'a', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let epoch = EpochBuilder::root_tree(module, build, 3);
    let mut tx = transaction(module, epoch).await;
    let value = tx
        .compute(&ModuleRegistrationExpansionKey::toolchains(workspace()))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = value else {
        panic!("recursive expansion returned Need")
    };
    assert_eq!(
        labels(&value),
        [
            "@@//tree/b/deep:a",
            "@@//tree/b/deep:z",
            "@@//tree/b:a",
            "@@//tree/b:z",
            "@@//tree/c:a",
            "@@//tree/c:z",
            "@@//tree:a",
            "@@//tree:z",
        ]
    );
}

#[tokio::test]
async fn observed_canonical_recursion_orders_route_subtree_and_inventories_with_exact_arcs() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
        "register_toolchains('@dep//tree/...:all')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'z', toolchain_type = ':type', toolchain = ':impl')\n",
        "toolchain(name = 'a', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let epoch = EpochBuilder::canonical_tree(module, build, 30);
    let expected_epoch = epoch.dupe();
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("observed canonical recursion did not complete: {outcome:?}")
    };
    assert_eq!(
        labels(observed.result()),
        [
            "@@dep+//tree/b/deep:a",
            "@@dep+//tree/b/deep:z",
            "@@dep+//tree/b:a",
            "@@dep+//tree/b:z",
            "@@dep+//tree/c:a",
            "@@dep+//tree/c:z",
            "@@dep+//tree:a",
            "@@dep+//tree:z",
        ]
    );
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }
    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 7, "dependencies: {dependencies:#?}");
    assert!(dependencies[0].starts_with("observed-host-selected-registration-patterns:"));
    assert!(dependencies[1].starts_with("observed-host-canonical-repository-load-route:"));
    assert!(dependencies[2].starts_with("observed-external-subtree-package-set:"));
    assert!(
        dependencies[3..]
            .iter()
            .all(|dependency| dependency.starts_with("observed-repository-package-inventory:"))
    );
    assert!(dependencies[3].ends_with("//tree/b/deep"));
    assert!(dependencies[4].ends_with("//tree/b"));
    assert!(dependencies[5].ends_with("//tree/c"));
    assert!(dependencies[6].ends_with("//tree"));
}

#[tokio::test]
async fn ambiguity_facts_participate_in_a_b_a_equality_when_labels_match() {
    const MODULE_A: &str = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:all')\n",
    );
    const MODULE_B: &str = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:*')\n",
    );
    const BUILD: &str = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'all', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let key = ModuleRegistrationExpansionKey::toolchains(workspace());
    let mut a_tx = transaction(MODULE_A, EpochBuilder::root_package(MODULE_A, BUILD, 10)).await;
    let a = a_tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(a_value) = &a else {
        panic!("conflict epoch returned Need")
    };
    assert_eq!(labels(a_value), ["@@//pkg:all"]);
    assert_eq!(a_value.ambiguities().len(), 1);
    assert_eq!(a_value.ambiguities()[0].raw_pattern(), "//pkg:all");
    let mut b_tx = update_module_epoch(
        a_tx,
        MODULE_B,
        EpochBuilder::root_package(MODULE_B, BUILD, 11),
    )
    .await;
    let b = b_tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(b_value) = &b else {
        panic!("wildcard epoch returned Need")
    };
    assert_eq!(labels(b_value), ["@@//pkg:all"]);
    assert!(b_value.ambiguities().is_empty());
    assert!(!ModuleRegistrationExpansionKey::equality(&a, &b));
    let mut restored_tx = update_module_epoch(
        b_tx,
        MODULE_A,
        EpochBuilder::root_package(MODULE_A, BUILD, 10),
    )
    .await;
    let restored = restored_tx.compute(&key).await.unwrap();
    assert!(ModuleRegistrationExpansionKey::equality(&a, &restored));
}

#[tokio::test]
async fn observed_expansion_preserves_epoch_identity_dependency_order_and_family_isolation() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "bazel_dep(name = 'dep', version = '1.0.0')\n",
        "local_path_override(module_name = 'dep', path = 'dep')\n",
        "register_toolchains('@dep//:all', '@@dep+//:fake')\n",
        "register_execution_platforms('@dep//platforms:*')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "filegroup(name = 'fake')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'tc', toolchain_type = ':type', toolchain = ':impl')\n",
        "platform(name = 'platform')\n",
    );
    let epoch = EpochBuilder::canonical_package(module, build, 20);
    let expected_epoch = epoch.dupe();
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("observed canonical expansion did not complete: {outcome:?}")
    };
    assert_eq!(labels(observed.result()), ["@@dep+//:tc", "@@dep+//:fake"]);
    assert!(!observed.observations().observations().is_empty());
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }

    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 3, "dependencies: {dependencies:#?}");
    assert!(dependencies[0].starts_with("observed-host-selected-registration-patterns:"));
    assert!(dependencies[1].starts_with("observed-host-canonical-repository-load-route:"));
    assert!(dependencies[2].starts_with("observed-repository-package-inventory:"));
    let activations = tracker.take();
    let parent = activation_for(&activations, &key);
    assert_eq!(parent.kind, ActivationKind::Evaluated);
    assert!(parent.batch.as_ref().unwrap().events().is_empty());
    assert_no_family_activation(&activations, ModuleRegistrationFamily::ExecutionPlatforms);
    assert_no_activation_containing(&activations, "//platforms");
}

#[tokio::test]
async fn ambiguity_warning_is_owned_by_requested_family_only() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:all')\n",
        "register_execution_platforms('//pkg:*')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'all', toolchain_type = ':type', toolchain = ':impl')\n",
        "platform(name = 'platform')\n",
    );
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        EpochBuilder::root_package(module, build, 21),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let key = ModuleRegistrationExpansionKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("warning expansion returned Need")
    };
    assert_eq!(value.ambiguities().len(), 1);
    let activations = tracker.take();
    let batch = activation_for(&activations, &key).batch.as_ref().unwrap();
    assert_eq!(batch.events().len(), 1);
    assert!(matches!(
        &batch.events()[0],
        EvaluationEvent::Diagnostic {
            level: EvaluationDiagnosticLevel::Warning,
            text,
        } if text.contains("//pkg:all")
    ));
    assert_no_family_activation(&activations, ModuleRegistrationFamily::ExecutionPlatforms);
}

#[tokio::test]
async fn exact_missing_target_is_a_typed_family_and_row_error() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:missing')\n",
    );
    let build = "filegroup(name = 'present')\n";
    let mut tx = transaction(module, EpochBuilder::root_package(module, build, 22)).await;
    let outcome = tx
        .compute(&ModuleRegistrationExpansionKey::toolchains(workspace()))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("missing target returned Need")
    };
    let error = value.labels().unwrap_err();
    assert_eq!(error.family(), ModuleRegistrationFamily::Toolchains);
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::MissingTarget(label)
            if label.to_string() == "@@//pkg:missing"
    ));
}

#[tokio::test]
async fn canonical_route_error_stops_before_later_package_row() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('@@unknown+//:bad', '//pkg:later')\n",
    );
    let tracker = Arc::new(ExpansionTracker::default());
    let epoch = EpochBuilder::root_package(module, "filegroup(name = 'later')\n", 40);
    let expected_epoch = epoch.dupe();
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("route terminal did not complete: {outcome:?}")
    };
    let error = observed.result().labels().unwrap_err();
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::CanonicalRoute(_)
    ));
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }
    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 2, "dependencies: {dependencies:#?}");
    assert!(dependencies[0].starts_with("observed-host-selected-registration-patterns:"));
    assert!(dependencies[1].starts_with("observed-host-canonical-repository-load-route:"));
    assert_no_activation_containing(&tracker.take(), "host-package-load:\"/workspace\"//pkg");
}

#[tokio::test]
async fn subtree_error_stops_before_later_package_row_and_retains_prefix() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//notdir/...:all', '//pkg:later')\n",
    );
    let mut builder = EpochBuilder::root_package_builder(module, 41);
    builder.directory("/workspace/pkg", 41);
    builder.file(
        "/workspace/pkg/BUILD.bazel",
        "filegroup(name = 'later')\n",
        41,
    );
    builder.file("/workspace/notdir", "not a directory\n", 41);
    let epoch = builder.build();
    let expected_epoch = epoch.dupe();
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("subtree terminal did not complete: {outcome:?}")
    };
    let error = observed.result().labels().unwrap_err();
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::RootSubtree(_)
    ));
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }
    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 2, "dependencies: {dependencies:#?}");
    assert!(dependencies[1].starts_with("observed-root-subtree-package-set://notdir"));
    assert_no_activation_containing(&tracker.take(), "host-package-load:\"/workspace\"//pkg");
}

#[tokio::test]
async fn package_error_stops_before_later_row_and_retains_decisive_epoch() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//missing:bad', '//pkg:later')\n",
    );
    let epoch = root_epoch_with_missing(
        module,
        "filegroup(name = 'later')\n",
        "/workspace/missing",
        42,
    );
    let expected_epoch = epoch.dupe();
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, epoch, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &outcome else {
        panic!("package terminal did not complete: {outcome:?}")
    };
    let error = observed.result().labels().unwrap_err();
    assert_eq!(error.row(), Some(0));
    assert!(matches!(
        error.kind(),
        ModuleRegistrationExpansionErrorKind::RootPackage(_)
    ));
    for (demand, result) in observed.observations().observations() {
        assert!(Arc::ptr_eq(result, expected_epoch.get(demand).unwrap()));
    }
    let dependencies = tracker.dependencies(&key.to_string());
    assert_eq!(dependencies.len(), 2, "dependencies: {dependencies:#?}");
    assert!(dependencies[1].starts_with("observed-host-package-load:\"/workspace\"//missing"));
    assert_no_activation_containing(&tracker.take(), "host-package-load:\"/workspace\"//pkg");
}

#[tokio::test]
async fn need_has_no_family_event_batch_and_recovers_with_complete_epoch() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:all')\n",
    );
    let tracker = Arc::new(ExpansionTracker::default());
    let incomplete = PathObservationEpoch::new([]).unwrap();
    let mut tx = transaction_with_tracker(module, incomplete, true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionKey::toolchains(workspace());
    let outcome = tx.compute(&key).await.unwrap();
    assert!(matches!(outcome, LoadingPreparationOutcome::Need(_)));
    assert!(
        tracker
            .take()
            .iter()
            .filter(|activation| activation.key == key.to_string())
            .all(|activation| activation.batch.is_none())
    );

    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'tc', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let mut recovered =
        update_module_epoch(tx, module, EpochBuilder::root_package(module, build, 23)).await;
    let LoadingPreparationOutcome::Complete(value) = recovered.compute(&key).await.unwrap() else {
        panic!("complete epoch did not recover")
    };
    assert_eq!(labels(&value), ["@@//pkg:tc"]);
}

#[tokio::test]
async fn observed_cancellation_publishes_no_family_owner_then_recovers_and_reuses() {
    let module = concat!(
        "module(name = 'bazel_tools')\n",
        "register_toolchains('//pkg:all')\n",
    );
    let build = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'tc', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let epoch = EpochBuilder::root_package(module, build, 24);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(ExpansionTracker::default());
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let mut cancelled =
        transaction_on(&dice, module, epoch.dupe(), true, Some(tracker.dupe())).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(tracker.take().iter().all(|row| row.key != key.to_string()));
    assert!(tracker.dependencies(&key.to_string()).is_empty());
    drop(cancelled);

    let mut recovered = transaction_on(&dice, module, epoch, true, Some(tracker.dupe())).await;
    let first = recovered.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(first_value)) = &first else {
        panic!("recovery did not complete: {first:?}")
    };
    assert_eq!(labels(first_value.result()), ["@@//pkg:tc"]);
    tracker.take();
    let warm = recovered.compute(&key).await.unwrap();
    assert!(ModuleRegistrationExpansionObservationKey::equality(
        &first, &warm
    ));
    let LoadingPreparationOutcome::Complete(Ok(warm_value)) = &warm else {
        panic!("warm recovery did not complete")
    };
    assert!(Arc::ptr_eq(first_value.result(), warm_value.result()));
    let activations = tracker.take();
    let parent = activation_for(&activations, &key);
    assert_eq!(parent.kind, ActivationKind::Reused);
    assert!(parent.batch.is_none());
}
