/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

//! Proofs for the repository-aware host package inventory carrier.

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
use slug_bzlmod_v2::HostRepositorySourceRoute;
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
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
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

use crate::CoercedAttributeValue;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::HostPackageInventory;
use crate::HostPackageInventoryErrorRef;
use crate::HostPackageInventoryKey;
use crate::HostPackageInventoryObservationKey;
use crate::RootPackageLoadObservationKey;
use crate::bzl_module::RepositoryPackageInventoryObservationKey;
use crate::cycle_detector::bzl_load_cycle_detector;
use crate::package::NativeToolchainTarget;
use crate::package::PackageTargetKind;
use crate::package::package_context_label_with_repository;

const ROOT_MODULE: &str = concat!(
    "module(name = 'bazel_tools')\n",
    "bazel_dep(name = 'dep', version = '1.0.0')\n",
    "local_path_override(module_name = 'dep', path = 'dep')\n",
);
const BUILD_A: &str = "filegroup(name = 'a')\n";
const BUILD_B: &str = "filegroup(name = 'b')\n";

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

fn package(repo: &str, path: &str) -> PackageIdentifier {
    PackageIdentifier::new(
        if repo.is_empty() {
            CanonicalRepoName::root()
        } else {
            CanonicalRepoName::new(repo).unwrap()
        },
        PackagePath::parse(path).unwrap(),
    )
}

#[test]
fn canonical_package_context_resolves_repository_aware_label_matrix() {
    let context = package("dep+", "pkg");
    let mapping = [(
        ApparentRepoName::new("alias").unwrap(),
        CanonicalRepoName::new("mapped+").unwrap(),
    )];
    for (raw, expected) in [
        (":local", "@@dep+//pkg:local"),
        ("bare", "@@dep+//pkg:bare"),
        ("//same:target", "@@dep+//same:target"),
        ("@@//root:target", "@@//root:target"),
        ("@@other+//other:target", "@@other+//other:target"),
        ("@alias//mapped:target", "@@mapped+//mapped:target"),
        ("//conditions:default", "@@//conditions:default"),
        ("//visibility:public", "@@//visibility:public"),
    ] {
        assert_eq!(
            package_context_label_with_repository(&context, &mapping, raw).unwrap(),
            CanonicalLabel::parse(expected).unwrap(),
            "{raw}"
        );
    }

    let missing = package_context_label_with_repository(&context, &mapping, "@missing//:target")
        .unwrap_err()
        .to_string();
    assert!(missing.contains("no repository visible as '@missing'"));
    for raw in ["@@other+//tree/...:all", "@alias//tree/...:all"] {
        assert!(
            package_context_label_with_repository(&context, &mapping, raw)
                .unwrap_err()
                .to_string()
                .contains("package name cannot contain '...'")
        );
    }
    assert!(
        package_context_label_with_repository(&package("", "pkg"), &[], "@alias//:target").is_err()
    );
    assert_ne!(
        package_context_label_with_repository(&package("dep_a+", "pkg"), &[], ":same").unwrap(),
        package_context_label_with_repository(&package("dep_b+", "pkg"), &[], ":same").unwrap(),
    );
}

#[derive(Debug)]
struct TrackedActivation {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct InventoryTracker {
    dependencies: Mutex<Vec<(String, Vec<String>)>>,
    activations: Mutex<Vec<TrackedActivation>>,
}

impl InventoryTracker {
    fn dependencies(&self, key: &impl ToString) -> Vec<String> {
        let key = key.to_string();
        self.dependencies
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|(candidate, dependencies)| (candidate == &key).then(|| dependencies.clone()))
            .unwrap_or_default()
    }

    fn take(&self) -> Vec<TrackedActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for InventoryTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        dependencies: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
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

    fn file(&mut self, path: &str, source: &str, variant: i64) {
        self.node(path, PathNodeKind::RegularFile, variant);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_bytes(),
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

    fn root(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::base(module, variant);
        epoch.directory("/workspace/pkg", variant);
        epoch.file("/workspace/pkg/BUILD.bazel", build, variant);
        epoch.finish()
    }

    fn canonical(module: &str, build: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::base(module, variant);
        epoch.directory("/workspace/dep", variant);
        epoch.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = 'dep', version = '1.0.0')\n",
            variant,
        );
        epoch.missing("/workspace/dep/REPO.bazel");
        epoch.missing("/workspace/dep/.bazelignore");
        epoch.file("/workspace/dep/BUILD.bazel", build, variant);
        epoch.finish()
    }

    fn canonical_without_build(module: &str, variant: i64) -> PathObservationEpoch {
        let mut epoch = Self::base(module, variant);
        epoch.directory("/workspace/dep", variant);
        epoch.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = 'dep', version = '1.0.0')\n",
            variant,
        );
        epoch.missing("/workspace/dep/REPO.bazel");
        epoch.missing("/workspace/dep/.bazelignore");
        epoch.finish()
    }

    fn both(module: &str, root_build: &str, canonical_build: &str) -> PathObservationEpoch {
        let mut epoch = Self::base(module, 1);
        epoch.directory("/workspace/pkg", 1);
        epoch.file("/workspace/pkg/BUILD.bazel", root_build, 1);
        epoch.directory("/workspace/dep", 1);
        epoch.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = 'dep', version = '1.0.0')\n",
            1,
        );
        epoch.missing("/workspace/dep/REPO.bazel");
        epoch.missing("/workspace/dep/.bazelignore");
        epoch.file("/workspace/dep/BUILD.bazel", canonical_build, 1);
        epoch.finish()
    }

    fn base(module: &str, variant: i64) -> Self {
        let mut epoch = Self::default();
        epoch.directory("/", variant);
        epoch.directory("/workspace", variant);
        epoch.file("/workspace/MODULE.bazel", module, variant);
        epoch.missing("/workspace/MODULE.bazel.lock");
        epoch.missing("/workspace/REPO.bazel");
        epoch.missing("/workspace/.bazelignore");
        epoch.listing("/workspace", &["dep", "pkg"]);
        epoch
    }

    fn finish(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

async fn transaction_on(
    dice: &Arc<Dice>,
    module: &str,
    epoch: PathObservationEpoch,
    tracker: Option<Arc<InventoryTracker>>,
) -> DiceTransaction {
    let data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: tracker.map(|value| value as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    let mut data = data;
    data.data.set(CaptureEvaluationEvents);
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

fn complete_observed(
    value: &SourcePreparationOutcome<
        Result<crate::ObservedHostPackageInventory, crate::HostPackageInventoryObservationError>,
    >,
) -> &crate::ObservedHostPackageInventory {
    let SourcePreparationOutcome::Complete(Ok(value)) = value else {
        panic!("host inventory did not complete: {value:?}")
    };
    value
}

fn assert_epoch_arcs(value: &PathObservationEpoch, expected: &PathObservationEpoch) {
    assert!(!value.observations().is_empty());
    for (demand, result) in value.observations() {
        assert!(Arc::ptr_eq(result, expected.get(demand).unwrap()));
    }
}

#[test]
fn key_identity_includes_repository_and_package() {
    let root = HostPackageInventoryKey::new(workspace(), package("", "pkg"));
    let canonical = HostPackageInventoryKey::new(workspace(), package("dep+", "pkg"));
    let other_package = HostPackageInventoryKey::new(workspace(), package("", "other"));
    assert_ne!(root, canonical);
    assert_ne!(root, other_package);
    assert_ne!(root.to_string(), canonical.to_string());
    fn assert_allocative<T: allocative::Allocative>() {}
    assert_allocative::<HostPackageInventory>();
    assert_allocative::<HostPackageInventoryKey>();
}

#[tokio::test]
async fn canonical_inventory_retains_final_native_label_identities() {
    let build = concat!(
        "constraint_setting(name = 'setting')\n",
        "constraint_value(name = 'value', constraint_setting = '@dep//:setting')\n",
        "platform(name = 'platform', constraint_values = ['//:value'])\n",
        "toolchain_type(name = 'type')\n",
        "config_setting(name = 'condition', values = {'cpu': 'k8'})\n",
        "filegroup(name = 'impl')\n",
        "toolchain(\n",
        "    name = 'toolchain',\n",
        "    toolchain_type = '@@//:root_type',\n",
        "    toolchain = 'impl',\n",
        "    exec_compatible_with = [':value'],\n",
        "    target_compatible_with = ['//:value'],\n",
        "    use_target_platform_constraints = True,\n",
        "    target_settings = ['//:setting'] + select({\n",
        "        ':condition': ['@@//:root_setting'],\n",
        "        '//conditions:default': ['@dep//:fallback'],\n",
        "    }),\n",
        ")\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::canonical(ROOT_MODULE, build, 15),
        None,
    )
    .await;
    let value = tx
        .compute(&HostPackageInventoryKey::new(
            workspace(),
            package("dep+", ""),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(inventory) = value else {
        panic!("canonical inventory did not complete: {value:?}")
    };
    let loaded = inventory.loaded().unwrap();
    let native = |name: &str| {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::NativeToolchain(native) = &target.kind else {
            panic!("{name} was not retained as a native toolchain target")
        };
        native
    };

    let NativeToolchainTarget::ConstraintValue { constraint_setting } = native("value") else {
        panic!("value has the wrong native kind")
    };
    assert_eq!(constraint_setting.to_string(), "@@dep+//:setting");
    let NativeToolchainTarget::Platform { constraint_values } = native("platform") else {
        panic!("platform has the wrong native kind")
    };
    assert_eq!(constraint_values[0].to_string(), "@@dep+//:value");
    let NativeToolchainTarget::Toolchain {
        toolchain_type,
        implementation,
        exec_compatible_with,
        target_compatible_with,
        use_target_platform_constraints,
        target_settings,
    } = native("toolchain")
    else {
        panic!("toolchain has the wrong native kind")
    };
    assert_eq!(toolchain_type.to_string(), "@@//:root_type");
    assert_eq!(implementation.to_string(), "@@dep+//:impl");
    assert_eq!(
        exec_compatible_with.value()[0].to_string(),
        "@@dep+//:value"
    );
    assert_eq!(
        target_compatible_with.value()[0].to_string(),
        "@@dep+//:value"
    );
    assert!(*use_target_platform_constraints.value());
    assert!(matches!(
        target_settings.value(),
        CoercedAttributeValue::Concatenation(_, _)
    ));
    assert_eq!(
        native("toolchain")
            .semantic_references()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "@@//:root_type",
            "@@dep+//:impl",
            "@@dep+//:value",
            "@@dep+//:value",
            "@@dep+//:setting",
            "@@//:root_setting",
            "@@dep+//:fallback",
            "@@dep+//:condition",
        ]
    );
}

#[tokio::test]
async fn canonical_inventory_rejects_unmapped_apparent_label_before_publication() {
    let build = concat!(
        "constraint_setting(name = 'setting')\n",
        "constraint_value(\n",
        "    name = 'value',\n",
        "    constraint_setting = '@missing//:setting',\n",
        ")\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::canonical(ROOT_MODULE, build, 16),
        None,
    )
    .await;
    let value = tx
        .compute(&HostPackageInventoryKey::new(
            workspace(),
            package("dep+", ""),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(inventory) = value else {
        panic!("canonical inventory did not complete: {value:?}")
    };
    let HostPackageInventoryErrorRef::Canonical(error) = inventory.loaded().unwrap_err() else {
        panic!("unmapped canonical label produced the wrong inventory error")
    };
    let error = error.to_string();
    assert!(
        error.contains("no repository visible as '@missing'"),
        "{error}"
    );
}

#[tokio::test]
async fn observed_root_retains_child_and_epoch_arcs_without_canonical_activation() {
    let epoch = EpochBuilder::root(ROOT_MODULE, BUILD_A, 10);
    let expected = epoch.dupe();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(InventoryTracker::default());
    let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("", "pkg"));
    let value = tx.compute(&key).await.unwrap();
    let observed = complete_observed(&value);
    let child = tx
        .compute(&RootPackageLoadObservationKey::new(
            workspace(),
            PackagePath::parse("pkg").unwrap(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(child)) = child else {
        panic!("root child did not complete")
    };
    assert!(Arc::ptr_eq(
        observed.result().root_result().unwrap(),
        child.result()
    ));
    assert_epoch_arcs(observed.observations(), &expected);
    assert_eq!(tracker.dependencies(&key).len(), 1);
    assert!(tracker.dependencies(&key)[0].starts_with("observed-host-package-load:"));
    assert!(tracker.take().iter().all(|row| {
        !row.key
            .starts_with("observed-host-canonical-repository-load-route:")
            && !row
                .key
                .starts_with("observed-repository-package-inventory:")
    }));
}

#[tokio::test]
async fn observed_canonical_orders_route_then_inventory_and_retains_exact_arcs() {
    let epoch = EpochBuilder::canonical(ROOT_MODULE, BUILD_A, 20);
    let expected = epoch.dupe();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(InventoryTracker::default());
    let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("dep+", ""));
    let value = tx.compute(&key).await.unwrap();
    let observed = complete_observed(&value);
    let route_key = HostCanonicalRepositoryLoadRouteObservationKey::new(
        workspace(),
        CanonicalRepoName::new("dep+").unwrap(),
    );
    let route = tx.compute(&route_key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(route)) = route else {
        panic!("canonical route did not complete")
    };
    let input = route.result().as_ref().as_ref().unwrap().input().clone();
    let inventory_key = RepositoryPackageInventoryObservationKey::new(
        HostRepositorySourceRoute::canonical(input),
        PackagePath::parse("").unwrap(),
    );
    let inventory = tx.compute(&inventory_key).await.unwrap();
    let SourcePreparationOutcome::Complete(Ok(inventory)) = inventory else {
        panic!("canonical child inventory did not complete")
    };
    assert!(Arc::ptr_eq(
        observed.result().canonical_result().unwrap(),
        inventory.result()
    ));
    assert_epoch_arcs(observed.observations(), &expected);
    let dependencies = tracker.dependencies(&key);
    assert_eq!(dependencies.len(), 2, "{dependencies:#?}");
    assert!(dependencies[0].starts_with("observed-host-canonical-repository-load-route:"));
    assert!(dependencies[1].starts_with("observed-repository-package-inventory:"));
    assert!(
        tracker
            .take()
            .iter()
            .all(|row| !row.key.starts_with("observed-host-package-load:"))
    );
}

#[tokio::test]
async fn wrapper_owns_no_events_and_warm_result_is_reused() {
    let epoch = EpochBuilder::root(ROOT_MODULE, BUILD_A, 30);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(InventoryTracker::default());
    let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("", "pkg"));
    let first = tx.compute(&key).await.unwrap();
    let first_result = complete_observed(&first).result().dupe();
    let first_activations = tracker.take();
    let parent = first_activations
        .iter()
        .find(|row| row.key == key.to_string())
        .unwrap();
    assert_eq!(parent.kind, ActivationKind::Evaluated);
    assert!(parent.batch.is_none());
    let warm = tx.compute(&key).await.unwrap();
    assert!(HostPackageInventoryObservationKey::equality(&first, &warm));
    assert!(Arc::ptr_eq(
        &first_result,
        complete_observed(&warm).result()
    ));
    let warm_activations = tracker.take();
    let parent = warm_activations
        .iter()
        .find(|row| row.key == key.to_string())
        .unwrap();
    assert_eq!(parent.kind, ActivationKind::Reused);
    assert!(parent.batch.is_none());
}

#[tokio::test]
async fn route_root_and_canonical_package_failures_remain_typed() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let epoch = EpochBuilder::both(ROOT_MODULE, "this is not starlark\n", "also not starlark\n");
    let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, None).await;
    for (key, expected) in [
        (
            HostPackageInventoryKey::new(workspace(), package("", "pkg")),
            "root",
        ),
        (
            HostPackageInventoryKey::new(workspace(), package("dep+", "")),
            "canonical",
        ),
        (
            HostPackageInventoryKey::new(workspace(), package("unknown+", "")),
            "route",
        ),
    ] {
        let SourcePreparationOutcome::Complete(value) = tx.compute(&key).await.unwrap() else {
            panic!("{expected} failure returned Need")
        };
        match (expected, value.loaded().unwrap_err()) {
            ("root", HostPackageInventoryErrorRef::Root(_))
            | ("canonical", HostPackageInventoryErrorRef::Canonical(_))
            | ("route", HostPackageInventoryErrorRef::CanonicalRoute(_)) => {}
            actual => panic!("wrong terminal: {actual:?}"),
        }
    }
}

#[tokio::test]
async fn observed_failures_retain_exact_child_result_arcs() {
    let cases = [
        (package("", "pkg"), "this is not starlark\n", "root"),
        (package("dep+", ""), "also not starlark\n", "canonical"),
    ];
    for (package_id, bad_build, expected) in cases {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let epoch = if expected == "root" {
            EpochBuilder::both(ROOT_MODULE, bad_build, BUILD_A)
        } else {
            EpochBuilder::canonical(ROOT_MODULE, bad_build, 71)
        };
        let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, None).await;
        let key = HostPackageInventoryObservationKey::new(workspace(), package_id);
        let parent = tx.compute(&key).await.unwrap();
        let parent = complete_observed(&parent);
        if expected == "root" {
            let child = tx
                .compute(&RootPackageLoadObservationKey::new(
                    workspace(),
                    PackagePath::parse("pkg").unwrap(),
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else {
                panic!("root failure child did not complete")
            };
            assert!(Arc::ptr_eq(
                parent.result().root_result().unwrap(),
                child.result()
            ));
        } else {
            let route = tx
                .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                    workspace(),
                    CanonicalRepoName::new("dep+").unwrap(),
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(route)) = route else {
                panic!("canonical failure route did not complete")
            };
            let inventory = tx
                .compute(&RepositoryPackageInventoryObservationKey::new(
                    HostRepositorySourceRoute::canonical(
                        route.result().as_ref().as_ref().unwrap().input().clone(),
                    ),
                    PackagePath::parse("").unwrap(),
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(inventory)) = inventory else {
                panic!("canonical failure child did not complete")
            };
            assert!(Arc::ptr_eq(
                parent.result().canonical_result().unwrap(),
                inventory.result()
            ));
        }
    }

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::both(ROOT_MODULE, BUILD_A, BUILD_A),
        None,
    )
    .await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("unknown+", ""));
    let parent = tx.compute(&key).await.unwrap();
    let parent = complete_observed(&parent);
    let route = tx
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace(),
            CanonicalRepoName::new("unknown+").unwrap(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(route)) = route else {
        panic!("route failure child did not complete")
    };
    assert!(Arc::ptr_eq(
        parent.result().canonical_route_result().unwrap(),
        route.result()
    ));
}

#[tokio::test]
async fn canonical_route_and_inventory_needs_stop_at_decisive_prefix() {
    for (epoch, expected_dependencies) in [
        (PathObservationEpoch::empty(), 1),
        (EpochBuilder::canonical_without_build(ROOT_MODULE, 80), 2),
    ] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(InventoryTracker::default());
        let mut tx = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
        let key = HostPackageInventoryObservationKey::new(workspace(), package("dep+", ""));
        let value = tx.compute(&key).await.unwrap();
        assert!(matches!(value, SourcePreparationOutcome::Need(_)));
        let dependencies = tracker.dependencies(&key);
        assert_eq!(
            dependencies.len(),
            expected_dependencies,
            "{dependencies:#?}"
        );
        assert!(dependencies[0].starts_with("observed-host-canonical-repository-load-route:"));
        if expected_dependencies == 1 {
            assert!(tracker.take().iter().all(|row| {
                !row.key
                    .starts_with("observed-repository-package-inventory:")
            }));
        } else {
            assert!(dependencies[1].starts_with("observed-repository-package-inventory:"));
        }
    }
}

#[tokio::test]
async fn canonical_wrapper_owns_no_events_and_cancelled_parent_recovers() {
    let epoch = EpochBuilder::canonical(ROOT_MODULE, BUILD_A, 90);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(InventoryTracker::default());
    let key = HostPackageInventoryObservationKey::new(workspace(), package("dep+", ""));
    let mut cancelled =
        transaction_on(&dice, ROOT_MODULE, epoch.dupe(), Some(tracker.dupe())).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(tracker.take().iter().all(|row| row.key != key.to_string()));
    assert!(tracker.dependencies(&key).is_empty());
    drop(cancelled);

    let mut recovered = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
    let first = recovered.compute(&key).await.unwrap();
    let first_result = complete_observed(&first).result().dupe();
    let activations = tracker.take();
    let parent = activations
        .iter()
        .find(|row| row.key == key.to_string())
        .unwrap();
    assert_eq!(parent.kind, ActivationKind::Evaluated);
    assert!(parent.batch.is_none());
    let warm = recovered.compute(&key).await.unwrap();
    assert!(Arc::ptr_eq(
        &first_result,
        complete_observed(&warm).result()
    ));
}

#[tokio::test]
async fn conflicting_route_and_inventory_epochs_are_a_typed_frontier_error() {
    let foreign_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut foreign = transaction_on(
        &foreign_dice,
        ROOT_MODULE,
        EpochBuilder::canonical(ROOT_MODULE, BUILD_A, 101),
        None,
    )
    .await;
    let foreign_route = foreign
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            workspace(),
            CanonicalRepoName::new("dep+").unwrap(),
        ))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(Ok(foreign_route)) = foreign_route else {
        panic!("foreign route did not complete")
    };
    let foreign_inventory_key = RepositoryPackageInventoryObservationKey::new(
        HostRepositorySourceRoute::canonical(
            foreign_route
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .input()
                .clone(),
        ),
        PackagePath::parse("").unwrap(),
    );
    let foreign_inventory = foreign.compute(&foreign_inventory_key).await.unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::canonical(ROOT_MODULE, BUILD_A, 100),
        None,
    )
    .await;
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(foreign_inventory_key, foreign_inventory)])
        .unwrap();
    let mut tx = updater.commit().await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("dep+", ""));
    let value = tx.compute(&key).await.unwrap();
    assert!(matches!(
        value,
        SourcePreparationOutcome::Complete(Err(
            crate::HostPackageInventoryObservationError::Frontier(_)
        ))
    ));
}

#[tokio::test]
async fn need_is_transient_and_recovery_completes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut tx = transaction_on(&dice, ROOT_MODULE, PathObservationEpoch::empty(), None).await;
    let key = HostPackageInventoryObservationKey::new(workspace(), package("", "pkg"));
    let need = tx.compute(&key).await.unwrap();
    assert!(matches!(need, SourcePreparationOutcome::Need(_)));
    assert!(!HostPackageInventoryObservationKey::validity(&need));
    let mut recovered = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::root(ROOT_MODULE, BUILD_A, 40),
        None,
    )
    .await;
    assert!(matches!(
        recovered.compute(&key).await.unwrap(),
        SourcePreparationOutcome::Complete(Ok(_))
    ));
}

#[tokio::test]
async fn cancellation_does_not_publish_parent_and_recovery_reuses() {
    let epoch = EpochBuilder::root(ROOT_MODULE, BUILD_A, 50);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(InventoryTracker::default());
    let key = HostPackageInventoryObservationKey::new(workspace(), package("", "pkg"));
    let mut cancelled =
        transaction_on(&dice, ROOT_MODULE, epoch.dupe(), Some(tracker.dupe())).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(tracker.take().iter().all(|row| row.key != key.to_string()));
    assert!(tracker.dependencies(&key).is_empty());
    drop(cancelled);
    let mut recovered = transaction_on(&dice, ROOT_MODULE, epoch, Some(tracker.dupe())).await;
    let first = recovered.compute(&key).await.unwrap();
    assert!(matches!(first, SourcePreparationOutcome::Complete(Ok(_))));
    tracker.take();
    let warm = recovered.compute(&key).await.unwrap();
    assert!(HostPackageInventoryObservationKey::equality(&first, &warm));
}

#[tokio::test]
async fn root_package_result_restores_across_a_b_a_epochs() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = HostPackageInventoryKey::new(workspace(), package("", "pkg"));
    let mut a_tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::root(ROOT_MODULE, BUILD_A, 60),
        None,
    )
    .await;
    let a = a_tx.compute(&key).await.unwrap();
    let mut b_tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::root(ROOT_MODULE, BUILD_B, 61),
        None,
    )
    .await;
    let b = b_tx.compute(&key).await.unwrap();
    assert!(!HostPackageInventoryKey::equality(&a, &b));
    let mut restored_tx = transaction_on(
        &dice,
        ROOT_MODULE,
        EpochBuilder::root(ROOT_MODULE, BUILD_A, 60),
        None,
    )
    .await;
    let restored = restored_tx.compute(&key).await.unwrap();
    assert!(HostPackageInventoryKey::equality(&a, &restored));
}
