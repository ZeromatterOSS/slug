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
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
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

use super::HostPackageLoadKey;
use super::resolve_host_load_label;
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

fn package_key() -> HostPackageLoadKey {
    HostPackageLoadKey::new(workspace(), PackagePath::parse("pkg").unwrap())
}

fn event_texts(batch: &EventBatch) -> Vec<&str> {
    batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
        })
        .collect()
}

type HostPackageOutcome = <HostPackageLoadKey as Key>::Value;

fn target_names(outcome: &HostPackageOutcome) -> Vec<&str> {
    let SourcePreparationOutcome::Complete(value) = outcome else {
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
    let SourcePreparationOutcome::Complete(value) = outcome else {
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
    let SourcePreparationOutcome::Need(need) = &outcome else {
        panic!("empty Host epoch did not request the root observation");
    };
    assert_eq!(
        need.path_observations().unwrap().demands()[0]
            .path()
            .as_path(),
        std::path::Path::new("/")
    );
    assert!(!HostPackageLoadKey::validity(&outcome));
    assert!(!HostPackageLoadKey::equality(&outcome, &outcome));
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
    let SourcePreparationOutcome::Complete(value) = outcome else {
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
