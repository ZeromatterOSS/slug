use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark_map::small_map::SmallMap;

use super::ExternalBzlModuleError;
use super::ExternalBzlModuleEvalKey;
use super::RepositoryBzlLabel;
use super::RepositoryPackageLoadError;
use super::RepositoryPackageLoadKey;
use super::resolve_external_load_label;
use super::resolve_host_load_label;
use crate::LoadingPreparationOutcome;
use crate::RootPackageLoadKey;
use crate::cycle_detector::bzl_load_cycle_detector;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
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

    fn missing(&mut self, path: &str) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
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

    fn workspace_sources(module: &str, build: &str, bzl: &[(&str, &str)], variant: i64) -> Self {
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", module, variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/pkg", variant);
        builder.file("/workspace/pkg/BUILD.bazel", build, variant);
        for (name, source) in bzl {
            builder.file(&format!("/workspace/pkg/{name}"), source, variant);
        }
        builder
    }

    fn external_sources(bzl: &[(&str, &[u8])], variant: i64) -> Self {
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file(
            "/workspace/MODULE.bazel",
            "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
            variant,
        );
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/dep", variant);
        builder.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = \"dep\", version = \"1.0.0\")\n",
            variant,
        );
        builder.missing("/workspace/dep/REPO.bazel");
        builder.missing("/workspace/dep/.bazelignore");
        for (name, source) in bzl {
            builder.file(&format!("/workspace/dep/{name}"), source, variant);
        }
        builder
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

fn package_policy() -> RootPackagePolicyInputs {
    package_policy_with_deleted(&[])
}

fn package_policy_with_deleted(deleted: &[&str]) -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        workspace(),
        [workspace()],
        deleted.iter().copied(),
        None,
        Some("warning"),
    )
    .unwrap()
}

#[derive(Debug, Clone)]
struct TrackedBatch {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct EventTracker {
    batches: Mutex<Vec<TrackedBatch>>,
}

impl EventTracker {
    fn take(&self) -> Vec<TrackedBatch> {
        std::mem::take(&mut *self.batches.lock().unwrap())
    }
}

impl ActivationTracker for EventTracker {
    fn key_activated(
        &self,
        _key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        let name = key.to_string();
        if !name.starts_with("host-root-module-file:")
            && !name.starts_with("host-bzl-module:")
            && !name.starts_with("host-package-load:")
            && !name.starts_with("external-bzl-module:")
            && !name.starts_with("host-route-repo-file:")
            && !name.starts_with("repository-package-source:")
            && !name.starts_with("repository-package-load:")
        {
            return;
        }
        self.batches.lock().unwrap().push(TrackedBatch {
            key: name,
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

async fn transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    capture_events: bool,
    tracker: Option<Arc<EventTracker>>,
) -> DiceTransaction {
    transaction_with_policy(dice, epoch, package_policy(), capture_events, tracker).await
}

async fn transaction_with_policy(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    policy: RootPackagePolicyInputs,
    capture_events: bool,
    tracker: Option<Arc<EventTracker>>,
) -> DiceTransaction {
    let mut user_data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    if capture_events {
        user_data.data.set(CaptureEvaluationEvents);
    }
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
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
    inject_root_package_policy_inputs(&mut updater, policy).unwrap();
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

fn package_key() -> RootPackageLoadKey {
    RootPackageLoadKey::new(workspace(), PackagePath::parse("pkg").unwrap())
}

async fn external_route(transaction: &mut DiceTransaction) -> RootRepositoryRoute {
    external_route_named(transaction, "dep").await
}

async fn external_route_named(
    transaction: &mut DiceTransaction,
    apparent_repo: &str,
) -> RootRepositoryRoute {
    let key = RootRepositoryRouteKey::new(
        workspace(),
        ApparentRepoName::new(apparent_repo).expect("valid apparent repository"),
    )
    .unwrap();
    match transaction.compute(&key).await.unwrap() {
        LoadingPreparationOutcome::Need(_) => panic!("complete external epoch returned Need"),
        LoadingPreparationOutcome::Complete(route) => route.as_ref().as_ref().unwrap().clone(),
    }
}

fn external_bzl_key(
    route: RootRepositoryRoute,
    package: &str,
    target: &str,
) -> ExternalBzlModuleEvalKey {
    let package = PackagePath::parse(package).unwrap();
    let label = resolve_external_load_label(&package, &format!(":{target}")).unwrap();
    ExternalBzlModuleEvalKey::new(route, label)
}

fn event_texts(batch: &EventBatch) -> Vec<&str> {
    batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
        })
        .collect()
}

type HostPackageOutcome = <RootPackageLoadKey as Key>::Value;

fn target_names(outcome: &HostPackageOutcome) -> Vec<&str> {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    value
        .as_ref()
        .as_ref()
        .unwrap()
        .targets
        .iter()
        .map(|target| target.name.as_str())
        .collect()
}

fn terminal_error(outcome: &HostPackageOutcome) -> String {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err().to_string()
}

async fn compute_package(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    policy: RootPackagePolicyInputs,
) -> HostPackageOutcome {
    transaction_with_policy(dice, epoch, policy, false, None)
        .await
        .compute(&package_key())
        .await
        .unwrap()
}

#[test]
fn root_load_resolution_is_mapping_free_and_rejects_path_escape() {
    let package = PackagePath::parse("pkg").unwrap();
    let expected = resolve_host_load_label(&package, ":defs/x.bzl").unwrap();
    for spelling in [
        "//pkg:defs/x.bzl",
        "@//pkg:defs/x.bzl",
        "@@//pkg:defs/x.bzl",
    ] {
        assert_eq!(
            resolve_host_load_label(&package, spelling).unwrap(),
            expected
        );
    }
    for invalid in [
        ":../x.bzl",
        ":./x.bzl",
        ":a/../x.bzl",
        ":a/./x.bzl",
        ":a//x.bzl",
        ":a\\x.bzl",
        ":a:x.bzl",
        ":a/x.bzl/",
        ":x.scl",
        "@repo//pkg:x.bzl",
        "@@repo//pkg:x.bzl",
        "//external:x.bzl",
    ] {
        assert!(
            resolve_host_load_label(&package, invalid).is_err(),
            "{invalid:?} entered Host key identity"
        );
    }
}

#[tokio::test]
async fn host_package_need_is_transient_and_root_anchor_precedes_source() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let key = package_key();
    let outcome = transaction.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(need) = &outcome else {
        panic!("empty Host epoch did not request the root observation");
    };
    assert_eq!(
        need.path_observations().unwrap().demands()[0]
            .path()
            .as_path(),
        std::path::Path::new("/")
    );
    assert_ne!(
        key,
        RootPackageLoadKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
            PackagePath::parse("pkg").unwrap(),
        )
    );
    assert_ne!(
        key,
        RootPackageLoadKey::new(workspace(), PackagePath::parse("other").unwrap())
    );
    assert!(!RootPackageLoadKey::validity(&outcome));
    assert!(!RootPackageLoadKey::equality(&outcome, &outcome));
}

#[tokio::test]
async fn host_package_loads_bzl_and_owns_only_local_complete_events() {
    let module = "print(\"ROOT\")\n";
    let build =
        "load(\":defs.bzl\", \"make\")\nprint(\"BUILD\")\nmake()\nfilegroup(name = \"x\")\n";
    let defs = "print(\"BZL\")\ndef make():\n    print(\"MACRO\")\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let mut transaction = transaction(
        &dice,
        EpochBuilder::workspace_sources(module, build, &[("defs.bzl", defs)], 1).build(),
        true,
        Some(tracker.dupe()),
    )
    .await;
    let outcome = transaction.compute(&package_key()).await.unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete Host source epoch returned Need");
    };
    let package = value.as_ref().as_ref().unwrap();
    assert_eq!(package.targets.len(), 1);
    assert_eq!(package.targets[0].name, "x");

    let batches = tracker.take();
    let batch = |prefix: &str| {
        let entry = batches
            .iter()
            .find(|entry| entry.key.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing activation for {prefix}: {batches:?}"));
        assert!(matches!(
            entry.kind,
            ActivationKind::Evaluated | ActivationKind::Reused
        ));
        event_texts(entry.batch.as_ref().unwrap())
    };
    assert_eq!(batch("host-root-module-file:"), ["ROOT"]);
    assert_eq!(batch("host-bzl-module:"), ["BZL"]);
    assert_eq!(batch("host-package-load:"), ["BUILD", "MACRO"]);
}

#[tokio::test]
async fn host_package_retained_graph_replays_all_input_lifecycles() {
    let module = "";
    let package_epoch = |build: Option<(&str, &str)>, bzl: &[(&str, &str)], variant| {
        let mut builder = EpochBuilder::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", module, variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.directory("/workspace/pkg", variant);
        match build {
            Some(("BUILD", source)) => {
                builder.missing("/workspace/pkg/BUILD.bazel");
                builder.file("/workspace/pkg/BUILD", source, variant);
            }
            Some(("BUILD.bazel", source)) => {
                builder.file("/workspace/pkg/BUILD.bazel", source, variant);
            }
            Some((name, _)) => panic!("unsupported BUILD name {name}"),
            None => {
                builder.missing("/workspace/pkg/BUILD.bazel");
                builder.missing("/workspace/pkg/BUILD");
            }
        }
        for (name, source) in bzl {
            builder.file(&format!("/workspace/pkg/{name}"), source, variant);
        }
        builder
    };

    let b_cycle = "load(\":a.bzl\", \"x\")\nx = 1\n";
    let b_fixed = "x = 1\n";
    let dice = Dice::builder().build(DetectCycles::Enabled);

    let missing_marker = compute_package(
        &dice,
        package_epoch(None, &[], 10).build(),
        package_policy(),
    )
    .await;
    assert_eq!(
        terminal_error(&missing_marker),
        "no BUILD.bazel or BUILD file in package //pkg"
    );

    let fallback_build = "load(\":a.bzl\", \"value\")\nfilegroup(name = value)\n";
    let fallback = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD", fallback_build)),
            &[("a.bzl", "value = \"fallback\"\n")],
            11,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&fallback), ["fallback"]);

    let edited_build = compute_package(
        &dice,
        package_epoch(
            Some((
                "BUILD.bazel",
                "load(\":a.bzl\", \"value\")\nfilegroup(name = \"edited_build\")\n",
            )),
            &[("a.bzl", "value = \"ignored\"\n")],
            12,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&edited_build), ["edited_build"]);

    let edited_bzl = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"edited_bzl\"\n")],
            13,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&edited_bzl), ["edited_bzl"]);

    let invalid_bzl = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value =\n")],
            14,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert!(terminal_error(&invalid_bzl).contains("parsing //pkg:a.bzl"));

    let nested_build = "load(\":sub/n.bzl\", \"value\")\nfilegroup(name = value)\n";
    let mut nested_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested\"\n")],
        15,
    );
    nested_epoch.directory("/workspace/pkg/sub", 15);
    nested_epoch.missing("/workspace/pkg/sub/BUILD.bazel");
    nested_epoch.missing("/workspace/pkg/sub/BUILD");
    let nested = compute_package(&dice, nested_epoch.build(), package_policy()).await;
    assert_eq!(target_names(&nested), ["nested"]);

    let mut crossing_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested\"\n")],
        16,
    );
    crossing_epoch.directory("/workspace/pkg/sub", 16);
    crossing_epoch.node(
        "/workspace/pkg/sub/BUILD.bazel",
        PathNodeKind::RegularFile,
        16,
    );
    let crossing = compute_package(&dice, crossing_epoch.build(), package_policy()).await;
    assert_eq!(
        terminal_error(&crossing),
        "loading `:sub/n.bzl`: label in package //pkg crosses boundary of subpackage //pkg/sub"
    );

    let mut restored_nested_epoch = package_epoch(
        Some(("BUILD.bazel", nested_build)),
        &[("sub/n.bzl", "value = \"nested_restored\"\n")],
        17,
    );
    restored_nested_epoch.directory("/workspace/pkg/sub", 17);
    restored_nested_epoch.missing("/workspace/pkg/sub/BUILD.bazel");
    restored_nested_epoch.missing("/workspace/pkg/sub/BUILD");
    let restored_nested =
        compute_package(&dice, restored_nested_epoch.build(), package_policy()).await;
    assert_eq!(target_names(&restored_nested), ["nested_restored"]);

    let load_edge = compute_package(
        &dice,
        package_epoch(
            Some((
                "BUILD.bazel",
                "load(\":b.bzl\", \"value\")\nfilegroup(name = value)\n",
            )),
            &[("a.bzl", "value =\n"), ("b.bzl", "value = \"new_edge\"\n")],
            18,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&load_edge), ["new_edge"]);

    let cycle_build = "load(\":a.bzl\", \"x\")\nfilegroup(name = \"cycle_ok\")\n";
    let a_cycle = "load(\":b.bzl\", \"x\")\nx = 1\n";
    let cycle = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_cycle)],
            19,
        )
        .build(),
        package_policy(),
    )
    .await;
    let cycle = terminal_error(&cycle);
    assert!(cycle.starts_with("cycle detected in extension files: \n    pkg/BUILD.bazel"));
    assert!(cycle.contains("//pkg:a.bzl"));
    assert!(cycle.contains("//pkg:b.bzl"));

    let fixed = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_fixed)],
            20,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&fixed), ["cycle_ok"]);

    let restored_cycle = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", cycle_build)),
            &[("a.bzl", a_cycle), ("b.bzl", b_cycle)],
            21,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert!(terminal_error(&restored_cycle).starts_with("cycle detected in extension files:"));

    let deleted = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"policy_restored\"\n")],
            22,
        )
        .build(),
        package_policy_with_deleted(&["//pkg"]),
    )
    .await;
    assert_eq!(
        terminal_error(&deleted),
        "package //pkg is deleted or ignored"
    );

    let restored = compute_package(
        &dice,
        package_epoch(
            Some(("BUILD.bazel", fallback_build)),
            &[("a.bzl", "value = \"policy_restored\"\n")],
            23,
        )
        .build(),
        package_policy(),
    )
    .await;
    assert_eq!(target_names(&restored), ["policy_restored"]);
}

type ExternalBzlOutcome = <ExternalBzlModuleEvalKey as Key>::Value;
type RepositoryPackageOutcome = <RepositoryPackageLoadKey as Key>::Value;

fn repository_package_terminal(outcome: &RepositoryPackageOutcome) -> &crate::LoadedPackage {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap()
}

fn repository_package_error(outcome: &RepositoryPackageOutcome) -> String {
    repository_package_typed_error(outcome).to_string()
}

fn repository_package_typed_error(
    outcome: &RepositoryPackageOutcome,
) -> &RepositoryPackageLoadError {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err()
}

fn external_terminal(outcome: &ExternalBzlOutcome) -> &super::FrozenBzlModule {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap()
}

fn external_error(outcome: &ExternalBzlOutcome) -> &ExternalBzlModuleError {
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("complete external source epoch returned Need");
    };
    value.as_ref().as_ref().unwrap_err()
}

#[test]
fn external_bzl_module_normalizes_exact_same_package_labels_before_source() {
    let root = PackagePath::parse("").unwrap();
    assert_eq!(
        resolve_external_load_label(&root, ":defs.bzl").unwrap(),
        resolve_external_load_label(&root, "//:defs.bzl").unwrap()
    );
    let package = PackagePath::parse("pkg").unwrap();
    assert_eq!(
        resolve_external_load_label(&package, ":defs.bzl").unwrap(),
        resolve_external_load_label(&package, "//pkg:defs.bzl").unwrap()
    );
    for rejected in [
        "@dep//pkg:defs.bzl",
        "@@dep+//pkg:defs.bzl",
        "@//pkg:defs.bzl",
        "@@//pkg:defs.bzl",
        "//other:defs.bzl",
        ":sub/defs.bzl",
        "//pkg:sub/defs.bzl",
        ":../defs.bzl",
        ":defs.star",
        "defs.bzl",
    ] {
        assert!(
            resolve_external_load_label(&package, rejected).is_err(),
            "{rejected:?} entered external key identity"
        );
    }
    let latin1 = RepositoryBzlLabel::new(
        package,
        slug_bzlmod_v2::RootPackageBzlTarget::parse("\u{00ff}.bzl").unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    assert_eq!(
        latin1.repository_relative_path().as_os_str().as_bytes(),
        b"pkg/\xff.bzl"
    );
}

#[tokio::test]
async fn external_bzl_module_full_route_keys_are_unequal_while_canonical_labels_match() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut direct = transaction(
        &dice,
        EpochBuilder::external_sources(&[("defs.bzl", b"VALUE = 1\n")], 39).build(),
        false,
        None,
    )
    .await;
    let direct_route = external_route_named(&mut direct, "dep").await;
    let direct_key = external_bzl_key(direct_route, "", "defs.bzl");

    let mut alias_epoch = EpochBuilder::external_sources(&[("defs.bzl", b"VALUE = 1\n")], 391);
    alias_epoch.file(
        "/workspace/MODULE.bazel",
        "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\", repo_name = \"alias\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
        391,
    );
    let mut alias = transaction(&dice, alias_epoch.build(), false, None).await;
    let alias_route = external_route_named(&mut alias, "alias").await;
    let alias_key = external_bzl_key(alias_route, "", "defs.bzl");

    assert_ne!(direct_key, alias_key);
    assert_eq!(direct_key.canonical_label(), alias_key.canonical_label());
    assert_eq!(
        direct_key.canonical_label().to_string(),
        "@@dep+//:defs.bzl"
    );
}

#[tokio::test]
async fn external_bzl_module_retains_canonical_manifest_lifetime_and_local_events() {
    let files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":left.bzl\", \"LEFT\")\nload(\"//:left.bzl\", \"LEFT_ALIAS\")\nload(\"//:right.bzl\", \"RIGHT\")\nprint(\"ROOT_BZL\")\nRESULT = LEFT + LEFT_ALIAS + RIGHT\n",
        ),
        (
            "left.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"LEFT_BZL\")\nLEFT = H\nLEFT_ALIAS = H\n",
        ),
        (
            "right.bzl",
            b"load(\":helper.bzl\", \"H\")\nprint(\"RIGHT_BZL\")\nRIGHT = H\n",
        ),
        ("helper.bzl", b"print(\"HELPER_BZL\")\nH = 1\n"),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(files, 40).build();
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = external_bzl_key(route, "", "root.bzl");
    let cold_value = cold.compute(&key).await.unwrap();
    let module = external_terminal(&cold_value);
    assert_eq!(module.path, PathBuf::from("/workspace/dep/root.bzl"));
    assert_eq!(module.loads, [":left.bzl", "//:left.bzl", "//:right.bzl"]);
    assert_eq!(
        module
            .manifest
            .direct_children
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:left.bzl", "@@dep+//:right.bzl"]
    );
    assert_eq!(
        module
            .manifest
            .reachable
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        [
            "@@dep+//:root.bzl",
            "@@dep+//:left.bzl",
            "@@dep+//:helper.bzl",
            "@@dep+//:right.bzl",
        ]
    );
    assert_eq!(module.retained_bzl_modules.len(), 3);
    assert!(ExternalBzlModuleEvalKey::validity(&cold_value));
    assert!(ExternalBzlModuleEvalKey::equality(&cold_value, &cold_value));

    let cold_batches = tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key.starts_with("external-bzl-module:"))
        .collect::<Vec<_>>();
    assert_eq!(cold_batches.len(), 6);
    assert_eq!(
        cold_batches
            .iter()
            .filter(|entry| entry.kind == ActivationKind::Evaluated)
            .count(),
        4
    );
    assert_eq!(
        cold_batches
            .iter()
            .filter(|entry| entry.kind == ActivationKind::Reused)
            .count(),
        2
    );
    for entry in &cold_batches {
        if entry.kind == ActivationKind::Evaluated {
            assert_eq!(event_texts(entry.batch.as_ref().unwrap()).len(), 1);
        }
    }

    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let warm_route = external_route(&mut warm).await;
    let warm_key = external_bzl_key(warm_route, "", "root.bzl");
    let warm_value = warm.compute(&warm_key).await.unwrap();
    assert!(ExternalBzlModuleEvalKey::equality(&cold_value, &warm_value));
    let warm_batches = warm_tracker
        .take()
        .into_iter()
        .filter(|entry| entry.key.starts_with("external-bzl-module:"))
        .collect::<Vec<_>>();
    assert_eq!(warm_batches.len(), 1);
    assert!(
        warm_batches
            .iter()
            .all(|entry| entry.kind == ActivationKind::Reused)
    );
    assert!(warm_batches.iter().all(|entry| entry.batch.is_none()));

    let empty_files: &[(&str, &[u8])] = &[
        (
            "root.bzl",
            b"load(\":left.bzl\", \"LEFT\")\nRESULT = LEFT\n",
        ),
        ("left.bzl", b"load(\":helper.bzl\", \"H\")\nLEFT = H\n"),
        ("helper.bzl", b"H = 1\n"),
    ];
    let empty_tracker = Arc::new(EventTracker::default());
    let mut empty = transaction(
        &dice,
        EpochBuilder::external_sources(empty_files, 41).build(),
        true,
        Some(empty_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut empty).await;
    external_terminal(
        &empty
            .compute(&external_bzl_key(route, "", "root.bzl"))
            .await
            .unwrap(),
    );
    let empty_root = empty_tracker
        .take()
        .into_iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:root.bzl")
        .unwrap();
    assert_eq!(
        event_texts(empty_root.batch.as_ref().unwrap()),
        Vec::<&str>::new()
    );

    let prefix_tracker = Arc::new(EventTracker::default());
    let mut prefix = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "root.bzl",
                b"print(\"PREFIX\")\nfail(\"terminal\")\nprint(\"AFTER\")\n",
            )],
            42,
        )
        .build(),
        true,
        Some(prefix_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut prefix).await;
    let failed = prefix
        .compute(&external_bzl_key(route, "", "root.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&failed),
        ExternalBzlModuleError::Evaluation { .. }
    ));
    let prefix_root = prefix_tracker
        .take()
        .into_iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:root.bzl")
        .unwrap();
    assert_eq!(event_texts(prefix_root.batch.as_ref().unwrap()), ["PREFIX"]);
}

#[tokio::test]
async fn external_bzl_module_prevalidates_loads_and_preserves_typed_terminal_equality() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut invalid_epoch = EpochBuilder::external_sources(
        &[(
            "root.bzl",
            b"load(\":unobserved.bzl\", \"X\")\nload(\"@other//:bad.bzl\", \"Y\")\n",
        )],
        50,
    );
    // There is deliberately no observation for unobserved.bzl. A Need here
    // would prove that validation happened after child source lookup.
    invalid_epoch.missing("/workspace/dep/BUILD.bazel");
    let mut invalid = transaction(&dice, invalid_epoch.build(), false, None).await;
    let route = external_route(&mut invalid).await;
    let key = external_bzl_key(route, "", "root.bzl");
    let invalid_value = invalid.compute(&key).await.unwrap();
    assert!(matches!(
        external_error(&invalid_value),
        ExternalBzlModuleError::LoadLabel { .. }
    ));
    assert!(ExternalBzlModuleEvalKey::equality(
        &invalid_value,
        &invalid_value
    ));

    let mut missing_epoch = EpochBuilder::external_sources(&[], 51);
    missing_epoch.missing("/workspace/dep/missing.bzl");
    let mut missing = transaction(&dice, missing_epoch.build(), false, None).await;
    let route = external_route(&mut missing).await;
    let missing_key = external_bzl_key(route, "", "missing.bzl");
    let missing_value = missing.compute(&missing_key).await.unwrap();
    assert!(matches!(
        external_error(&missing_value),
        ExternalBzlModuleError::Absent { .. }
    ));

    let mut parse = transaction(
        &dice,
        EpochBuilder::external_sources(&[("bad.bzl", b"VALUE =\n")], 52).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut parse).await;
    let parse_value = parse
        .compute(&external_bzl_key(route, "", "bad.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&parse_value),
        ExternalBzlModuleError::Parse { .. }
    ));

    let mut encoding = transaction(
        &dice,
        EpochBuilder::external_sources(&[("encoding.bzl", &[0xff])], 521).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut encoding).await;
    let encoding_value = encoding
        .compute(&external_bzl_key(route, "", "encoding.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&encoding_value),
        ExternalBzlModuleError::Encoding { .. }
    ));

    let mut child_epoch = EpochBuilder::external_sources(
        &[("parent.bzl", b"load(\":missing_child.bzl\", \"VALUE\")\n")],
        522,
    );
    child_epoch.missing("/workspace/dep/missing_child.bzl");
    let mut child = transaction(&dice, child_epoch.build(), false, None).await;
    let route = external_route(&mut child).await;
    let child_value = child
        .compute(&external_bzl_key(route, "", "parent.bzl"))
        .await
        .unwrap();
    assert!(matches!(
        external_error(&child_value),
        ExternalBzlModuleError::Child { .. }
    ));

    let mut evaluation = transaction(
        &dice,
        EpochBuilder::external_sources(&[("fail.bzl", b"fail(\"boom\")\n")], 53).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut evaluation).await;
    let eval_key = external_bzl_key(route, "", "fail.bzl");
    let eval_value = evaluation.compute(&eval_key).await.unwrap();
    assert!(matches!(
        external_error(&eval_value),
        ExternalBzlModuleError::Evaluation { .. }
    ));

    // Every value exposed by loading_globals implements Freeze. Retain a
    // typed equality discriminator for the future error path without adding
    // a non-freezable evaluator value solely for this dormant owner.
    let freeze_a = ExternalBzlModuleError::Freeze {
        label: eval_key.canonical_label(),
        message: Arc::from("freeze discriminator"),
    };
    assert_eq!(freeze_a, freeze_a.clone());

    let mut need = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let need_value = need.compute(&eval_key).await.unwrap();
    assert!(matches!(need_value, LoadingPreparationOutcome::Need(_)));
    assert!(!ExternalBzlModuleEvalKey::validity(&need_value));
    assert!(!ExternalBzlModuleEvalKey::equality(
        &need_value,
        &need_value
    ));
}

#[tokio::test]
async fn external_bzl_module_cycle_releases_and_recovers_with_fresh_detector() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let self_epoch = EpochBuilder::external_sources(
        &[("self.bzl", b"load(\":self.bzl\", \"VALUE\")\nVALUE = 1\n")],
        59,
    )
    .build();
    let mut self_cycle = transaction(&dice, self_epoch, false, None).await;
    let route = external_route(&mut self_cycle).await;
    let self_key = external_bzl_key(route, "", "self.bzl");
    let self_value = tokio::time::timeout(Duration::from_secs(5), self_cycle.compute(&self_key))
        .await
        .expect("external self-cycle detector must release recursive DICE wait")
        .unwrap();
    let self_detected = external_error(&self_value).cycle().unwrap();
    assert!(self_detected.path.is_empty());
    assert_eq!(
        self_detected
            .keys
            .iter()
            .map(ExternalBzlModuleEvalKey::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:self.bzl"]
    );

    let cycle_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"load(\":one.bzl\", \"ONE\")\nTWO = ONE\n"),
        ],
        60,
    )
    .build();
    let mut cycle = transaction(&dice, cycle_epoch, false, None).await;
    let route = external_route(&mut cycle).await;
    let key = external_bzl_key(route, "", "entry.bzl");
    let cycle_value = tokio::time::timeout(Duration::from_secs(5), cycle.compute(&key))
        .await
        .expect("external cycle detector must release recursive DICE wait")
        .unwrap();
    let detected = external_error(&cycle_value).cycle().unwrap();
    assert_eq!(
        detected
            .path
            .iter()
            .map(ExternalBzlModuleEvalKey::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:entry.bzl"]
    );
    assert_eq!(
        detected
            .keys
            .iter()
            .map(ExternalBzlModuleEvalKey::canonical_label)
            .map(|label| label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:one.bzl", "@@dep+//:two.bzl"]
    );

    let fixed_epoch = EpochBuilder::external_sources(
        &[
            ("entry.bzl", b"load(\":one.bzl\", \"ONE\")\nVALUE = ONE\n"),
            ("one.bzl", b"load(\":two.bzl\", \"TWO\")\nONE = TWO\n"),
            ("two.bzl", b"TWO = 2\n"),
        ],
        61,
    )
    .build();
    // transaction() installs a fresh request-scoped detector while retaining
    // the same DICE engine and all non-cycle dependencies.
    let mut fixed = transaction(&dice, fixed_epoch, false, None).await;
    let route = external_route(&mut fixed).await;
    let fixed_value = fixed
        .compute(&external_bzl_key(route, "", "entry.bzl"))
        .await
        .unwrap();
    assert_eq!(external_terminal(&fixed_value).manifest.reachable.len(), 3);
}

#[tokio::test]
async fn repository_package_load_activates_external_macro_manifest_lifetime_and_local_events() {
    let files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"make_filegroup\")\nprint(\"BUILD_TOP\")\nmake_filegroup(name = \"macro_files\")\n",
        ),
        (
            "defs.bzl",
            b"print(\"DEFS_TOP\")\ndef make_filegroup(name):\n    print(\"MACRO_BODY\")\n    native.filegroup(name = name)\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::external_sources(files, 70);
    epoch.file("/workspace/dep/REPO.bazel", b"print(\"REPO_TOP\")\n", 70);
    let epoch = epoch.build();
    let tracker = Arc::new(EventTracker::default());
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = RepositoryPackageLoadKey::new(route, PackagePath::parse("").unwrap());
    let cold_value = cold.compute(&key).await.unwrap();
    let loaded = repository_package_terminal(&cold_value);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["macro_files"]
    );
    assert_eq!(
        loaded
            .direct_load_roots
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:defs.bzl"]
    );
    assert_eq!(
        loaded
            .reachable_loads
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//:defs.bzl"]
    );
    assert_eq!(loaded.retained_bzl_module_count(), 1);
    assert_ne!(loaded.load_fingerprint, [0; 32]);
    assert!(RepositoryPackageLoadKey::validity(&cold_value));
    assert!(RepositoryPackageLoadKey::equality(&cold_value, &cold_value));

    let batches = tracker.take();
    let bzl = batches
        .iter()
        .find(|entry| entry.key == "external-bzl-module:@@dep+//:defs.bzl")
        .unwrap();
    assert_eq!(bzl.kind, ActivationKind::Evaluated);
    assert_eq!(event_texts(bzl.batch.as_ref().unwrap()), ["DEFS_TOP"]);
    let package = batches
        .iter()
        .find(|entry| entry.key.starts_with("repository-package-load:"))
        .unwrap();
    assert_eq!(package.kind, ActivationKind::Evaluated);
    assert_eq!(
        event_texts(package.batch.as_ref().unwrap()),
        ["BUILD_TOP", "MACRO_BODY"]
    );
    let selected_source = batches
        .iter()
        .find(|entry| entry.key.starts_with("repository-package-source:"))
        .unwrap();
    assert_eq!(selected_source.kind, ActivationKind::Evaluated);
    assert!(selected_source.batch.is_none());
    let route_policy = batches
        .iter()
        .find(|entry| entry.key.starts_with("host-route-repo-file:"))
        .unwrap();
    assert_eq!(route_policy.kind, ActivationKind::Evaluated);
    assert_eq!(
        event_texts(route_policy.batch.as_ref().unwrap()),
        ["REPO_TOP"]
    );

    let uncaptured_dice = Dice::builder().build(DetectCycles::Enabled);
    let uncaptured_tracker = Arc::new(EventTracker::default());
    let mut uncaptured = transaction(
        &uncaptured_dice,
        epoch.clone(),
        false,
        Some(uncaptured_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut uncaptured).await;
    repository_package_terminal(
        &uncaptured
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap(),
    );
    let uncaptured_batches = uncaptured_tracker.take();
    assert!(uncaptured_batches.iter().all(|entry| entry.batch.is_none()));
    assert!(
        uncaptured_batches
            .iter()
            .any(|entry| entry.key.starts_with("host-route-repo-file:"))
    );

    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let route = external_route(&mut warm).await;
    let warm_value = warm
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(RepositoryPackageLoadKey::equality(&cold_value, &warm_value));
    let warm_batches = warm_tracker.take();
    assert!(
        warm_batches
            .iter()
            .all(|entry| { entry.kind == ActivationKind::Reused && entry.batch.is_none() })
    );
}

#[tokio::test]
async fn repository_package_load_preserves_selected_source_error_display_and_chain() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::external_sources(&[], 79);
    epoch.missing("/workspace/dep/BUILD.bazel");
    epoch.missing("/workspace/dep/BUILD");
    let mut transaction = transaction(&dice, epoch.build(), false, None).await;
    let route = external_route(&mut transaction).await;
    let outcome = transaction
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    let error = repository_package_typed_error(&outcome);
    let expected = "no such package '@@dep+//': BUILD file not found in directory '' of external repository @@dep+. Add a BUILD file to a directory to mark it as a package.";
    assert_eq!(error.to_string(), expected);
    assert_eq!(
        std::error::Error::source(error).unwrap().to_string(),
        expected
    );
    assert!(RepositoryPackageLoadKey::validity(&outcome));
}

#[tokio::test]
async fn repository_package_load_accepts_only_dependency_free_public_starlark_rule_shape() {
    let valid: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"probe\")\nprint(\"RULE_BUILD\")\nprobe(name = \"probe\", empty = [], visibility = [\"//visibility:public\"])\n",
        ),
        (
            "defs.bzl",
            b"print(\"RULE_BZL\")\ndef _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"empty\": attr.label_list()})\n",
        ),
    ];
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let epoch = EpochBuilder::external_sources(valid, 80).build();
    let mut cold = transaction(&dice, epoch.clone(), true, Some(tracker.dupe())).await;
    let route = external_route(&mut cold).await;
    let key = RepositoryPackageLoadKey::new(route, PackagePath::parse("").unwrap());
    let cold_value = cold.compute(&key).await.unwrap();
    let loaded = repository_package_terminal(&cold_value);
    assert_eq!(loaded.targets.len(), 1);
    assert!(matches!(
        loaded.targets[0].kind,
        crate::PackageTargetKind::StarlarkRule(_)
    ));
    let capability = loaded.targets[0].rule_capability().unwrap();
    assert_eq!(capability.rule_class, "probe");
    assert!(!capability.executable);
    assert!(capability.test_kind.is_none());
    assert_eq!(loaded.retained_bzl_module_count(), 1);
    let batches = tracker.take();
    assert_eq!(
        event_texts(
            batches
                .iter()
                .find(|entry| entry.key == "external-bzl-module:@@dep+//:defs.bzl")
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["RULE_BZL"]
    );
    assert_eq!(
        event_texts(
            batches
                .iter()
                .find(|entry| entry.key.starts_with("repository-package-load:"))
                .unwrap()
                .batch
                .as_ref()
                .unwrap()
        ),
        ["RULE_BUILD"]
    );
    let warm_tracker = Arc::new(EventTracker::default());
    let mut warm = transaction(&dice, epoch, true, Some(warm_tracker.dupe())).await;
    let route = external_route(&mut warm).await;
    repository_package_terminal(
        &warm
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap(),
    );
    assert!(
        warm_tracker
            .take()
            .iter()
            .all(|entry| entry.kind == ActivationKind::Reused && entry.batch.is_none())
    );

    let cases = [
        (
            "probe(name = \"bad\")",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", visibility = [\"//visibility:private\"])",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", visibility = [\"//pkg:__pkg__\"])",
            "visibility is not explicitly public",
        ),
        (
            "probe(name = \"bad\", dep = \":dep.txt\", visibility = [\"//visibility:public\"])",
            "ordinary dependencies are deferred",
        ),
        (
            "probe(name = \"bad\", out = \"out.txt\", visibility = [\"//visibility:public\"])",
            "attribute `out` contains a reachable label",
        ),
        (
            "exports_files([\"extra.txt\"])\nprobe(name = \"bad\", visibility = [\"//visibility:public\"])",
            "package contains 2 targets",
        ),
    ];
    for (index, (invocation, expected)) in cases.into_iter().enumerate() {
        let build = format!("load(\":defs.bzl\", \"probe\")\n{invocation}\n");
        let defs = b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"dep\": attr.label(), \"out\": attr.output()})\n";
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(
                &[("BUILD.bazel", build.as_bytes()), ("defs.bzl", defs)],
                81 + index as i64,
            )
            .build(),
            false,
            None,
        )
        .await;
        let route = external_route(&mut transaction).await;
        let outcome = transaction
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap();
        assert!(
            repository_package_error(&outcome).contains(expected),
            "{expected}"
        );
    }
    for (source, symbol, expected) in [
        (
            b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe_test = rule(implementation = _impl, test = True)\n".as_slice(),
            "probe_test",
            "test rules are deferred",
        ),
        (
            b"def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, executable = True)\n".as_slice(),
            "probe",
            "executable rules are deferred",
        ),
    ] {
        let build = format!(
            "load(\":defs.bzl\", \"{symbol}\")\n{symbol}(name = \"bad\", visibility = [\"//visibility:public\"])\n"
        );
        let mut transaction = transaction(
            &dice,
            EpochBuilder::external_sources(
                &[("BUILD.bazel", build.as_bytes()), ("defs.bzl", source)],
                90,
            )
            .build(),
            false,
            None,
        )
        .await;
        let route = external_route(&mut transaction).await;
        let outcome = transaction
            .compute(&RepositoryPackageLoadKey::new(
                route,
                PackagePath::parse("").unwrap(),
            ))
            .await
            .unwrap();
        let error = repository_package_error(&outcome);
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[tokio::test]
async fn repository_package_load_prevalidates_all_loads_and_gates_loaded_target_kinds() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(EventTracker::default());
    let mut invalid = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "BUILD.bazel",
                b"load(\":unobserved.bzl\", \"FIRST\")\nload(\"@other//:bad.bzl\", \"SECOND\")\n",
            )],
            71,
        )
        .build(),
        false,
        Some(tracker.dupe()),
    )
    .await;
    let route = external_route(&mut invalid).await;
    let invalid_key = RepositoryPackageLoadKey::new(route.clone(), PackagePath::parse("").unwrap());
    let invalid_value = invalid.compute(&invalid_key).await.unwrap();
    assert!(
        repository_package_error(&invalid_value)
            .contains("repository-qualified external load is deferred: @other//:bad.bzl")
    );
    assert!(
        tracker
            .take()
            .iter()
            .all(|entry| !entry.key.starts_with("external-bzl-module:"))
    );
    assert!(RepositoryPackageLoadKey::validity(&invalid_value));
    assert!(RepositoryPackageLoadKey::equality(
        &invalid_value,
        &invalid_value
    ));

    let sequential_tracker = Arc::new(EventTracker::default());
    let mut sequential_need = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[
                (
                    "BUILD.bazel",
                    b"load(\":first.bzl\", \"FIRST\")\nload(\":second.bzl\", \"SECOND\")\n",
                ),
                ("second.bzl", b"SECOND = 2\n"),
            ],
            711,
        )
        .build(),
        false,
        Some(sequential_tracker.dupe()),
    )
    .await;
    let route = external_route(&mut sequential_need).await;
    let sequential_need = sequential_need
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        sequential_need,
        LoadingPreparationOutcome::Need(_)
    ));
    assert!(
        sequential_tracker
            .take()
            .iter()
            .all(|entry| entry.key != "external-bzl-module:@@dep+//:second.bzl")
    );

    let mut need = transaction(&dice, EpochBuilder::default().build(), false, None).await;
    let need_value = need.compute(&invalid_key).await.unwrap();
    assert!(matches!(need_value, LoadingPreparationOutcome::Need(_)));
    assert!(!RepositoryPackageLoadKey::validity(&need_value));
    assert!(!RepositoryPackageLoadKey::equality(
        &need_value,
        &need_value
    ));

    let mut rejected = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[
                (
                    "BUILD.bazel",
                    b"load(\":defs.bzl\", \"make_alias\")\nmake_alias(name = \"blocked\")\n",
                ),
                (
                    "defs.bzl",
                    b"def make_alias(name):\n    native.alias(name = name, actual = \":missing\")\n",
                ),
            ],
            72,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut rejected).await;
    let rejected = rejected
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert!(
        repository_package_error(&rejected)
            .contains("produced unsupported target `blocked` of kind alias")
    );

    let mut unloaded = transaction(
        &dice,
        EpochBuilder::external_sources(
            &[(
                "BUILD.bazel",
                b"alias(name = \"accepted\", actual = \":missing\")\n",
            )],
            73,
        )
        .build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut unloaded).await;
    let unloaded = unloaded
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(
        repository_package_terminal(&unloaded).targets[0].name,
        "accepted"
    );
}

#[tokio::test]
async fn repository_package_load_renders_missing_and_cycle_and_recovers_on_same_dice() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut missing_epoch = EpochBuilder::external_sources(
        &[(
            "BUILD.bazel",
            b"load(\":missing.bzl\", \"make\")\nmake(name = \"x\")\n",
        )],
        74,
    );
    missing_epoch.missing("/workspace/dep/missing.bzl");
    let mut missing = transaction(&dice, missing_epoch.build(), false, None).await;
    let route = external_route(&mut missing).await;
    let missing = missing
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(
        repository_package_error(&missing),
        "cannot load '@@dep+//:missing.bzl': no such file"
    );

    let cycle_files: &[(&str, &[u8])] = &[
        (
            "BUILD.bazel",
            b"load(\":defs.bzl\", \"make\")\nmake(name = \"x\")\n",
        ),
        (
            "defs.bzl",
            b"load(\":helper.bzl\", \"HELPER\")\ndef make(name):\n    native.filegroup(name = name)\n",
        ),
        (
            "helper.bzl",
            b"load(\":defs.bzl\", \"make\")\nHELPER = make\n",
        ),
    ];
    let mut cycle = transaction(
        &dice,
        EpochBuilder::external_sources(cycle_files, 75).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut cycle).await;
    let cycle = tokio::time::timeout(
        Duration::from_secs(5),
        cycle.compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        )),
    )
    .await
    .expect("external BUILD cycle detector must release the recursive DICE wait")
    .unwrap();
    let rendered = repository_package_error(&cycle);
    assert!(rendered.starts_with("cycle detected in extension files:"));
    assert!(rendered.contains("@@dep+///BUILD.bazel"));
    assert!(rendered.contains(".-> @@dep+//:defs.bzl"));
    assert!(rendered.contains("|   @@dep+//:helper.bzl"));
    assert!(rendered.contains("`-- @@dep+//:defs.bzl"));

    let fixed_files: &[(&str, &[u8])] = &[
        cycle_files[0],
        cycle_files[1],
        ("helper.bzl", b"HELPER = 1\n"),
    ];
    let mut fixed = transaction(
        &dice,
        EpochBuilder::external_sources(fixed_files, 76).build(),
        false,
        None,
    )
    .await;
    let route = external_route(&mut fixed).await;
    let fixed = fixed
        .compute(&RepositoryPackageLoadKey::new(
            route,
            PackagePath::parse("").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(repository_package_terminal(&fixed).targets[0].name, "x");
}
