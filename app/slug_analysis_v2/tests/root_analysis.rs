use std::path::Path;
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
use slug_analysis_v2::AnalysisErrorKind;
use slug_analysis_v2::AnalysisPreparationOutcome;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ProviderId;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::bzl_load_cycle_detector;
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

    fn package(&mut self, package: &str, build: &str, variant: i64) {
        let directory = format!("/workspace/{package}");
        self.directory(&directory, variant);
        self.file(&format!("{directory}/BUILD.bazel"), build, variant);
    }

    fn deleted_package(&mut self, package: &str, variant: i64) {
        let directory = format!("/workspace/{package}");
        self.directory(&directory, variant);
        self.missing(&format!("{directory}/BUILD.bazel"));
        self.missing(&format!("{directory}/BUILD"));
    }

    fn base(prefix: &str, parent_dependencies: &[&str], variant: i64) -> Self {
        let definitions = format!(
            r#"MarkerInfo = provider(fields = {{"value": "ordered marker"}})

def _leaf_impl(ctx):
    print("LEAF_ANALYSIS")
    return [DefaultInfo(files = depset([])), MarkerInfo(value = "{prefix}" + ctx.label.name)]

def _parent_impl(ctx):
    print("PARENT_ANALYSIS")
    values = [dep[MarkerInfo].value for dep in ctx.attr.deps]
    return [DefaultInfo(files = depset([])), MarkerInfo(value = ",".join(values))]

leaf = rule(implementation = _leaf_impl)
parent = rule(implementation = _parent_impl, attrs = {{"deps": attr.label_list()}})
"#
        );
        let dependencies = parent_dependencies
            .iter()
            .map(|label| format!("\"{label}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let parent_build = format!(
            "load(\"//rules:defs.bzl\", \"parent\")\n\
             parent(name = \"parent\", deps = [{dependencies}])\n"
        );
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", "", variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.package("rules", "", variant);
        builder.file("/workspace/rules/defs.bzl", definitions, variant);
        builder.package("parent", &parent_build, variant);
        builder
    }

    fn add_leaf(&mut self, package: &str, variant: i64) {
        self.package(
            package,
            &format!("load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"{package}\")\n"),
            variant,
        );
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

fn package_policy() -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        workspace(),
        [workspace()],
        std::iter::empty::<&str>(),
        None,
        Some("warning"),
    )
    .unwrap()
}

#[derive(Debug, Clone)]
struct TrackedAnalysis {
    key: ConfiguredTargetKey,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct AnalysisTracker {
    activations: Mutex<Vec<TrackedAnalysis>>,
}

impl AnalysisTracker {
    fn take(&self) -> Vec<TrackedAnalysis> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for AnalysisTracker {
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
        let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() else {
            return;
        };
        self.activations.lock().unwrap().push(TrackedAnalysis {
            key: key.configured_target().clone(),
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

fn configured(label: &str) -> ConfiguredTargetKey {
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(label).unwrap(),
        ConfigurationKey::target("root-analysis").unwrap(),
    )
}

fn parent_key() -> ConfiguredNodeAnalysisKey {
    ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:parent"))
}

async fn transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<AnalysisTracker>,
) -> DiceTransaction {
    let mut user_data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: Some(tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    user_data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    inject_root_package_policy_inputs(&mut updater, package_policy()).unwrap();
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

fn analysis_batch<'a>(activations: &'a [TrackedAnalysis], label: &str) -> Option<&'a EventBatch> {
    let activation = activations
        .iter()
        .find(|activation| activation.key.label().to_string() == label)
        .unwrap_or_else(|| panic!("missing analysis activation for {label}: {activations:#?}"));
    assert!(matches!(
        activation.kind,
        ActivationKind::Evaluated | ActivationKind::Reused
    ));
    activation.batch.as_ref()
}

fn marker_value(
    outcome: &<ConfiguredNodeAnalysisKey as Key>::Value,
    provider: &ProviderId,
) -> String {
    let AnalysisPreparationOutcome::Complete(value) = outcome else {
        panic!("complete root analysis epoch returned Need");
    };
    value
        .as_ref()
        .as_ref()
        .unwrap()
        .providers()
        .user(provider)
        .unwrap()
        .field("value")
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn root_analysis_preserves_typed_direct_target_missing() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let key = ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:missing"));
    let mut transaction =
        transaction(&dice, EpochBuilder::base("v1-", &[], 1).build(), tracker).await;
    let outcome = transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Complete(result) = outcome else {
        panic!("complete package lookup returned Need");
    };
    let error = result.as_ref().as_ref().unwrap_err();
    assert!(matches!(
        error.kind(),
        AnalysisErrorKind::TargetNotFound { label, build_file }
            if label.to_string() == "@@//parent:missing"
                && build_file == &std::path::PathBuf::from("/workspace/parent/BUILD.bazel")
    ));
    assert_eq!(
        error.to_string(),
        "target `@@//parent:missing` was not found in /workspace/parent/BUILD.bazel"
    );
}

#[tokio::test]
async fn root_analysis_unions_needs_and_replays_build_bzl_dependency_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let key = parent_key();
    assert_ne!(
        key,
        ConfiguredNodeAnalysisKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
            configured("@@//parent:parent"),
        )
    );
    assert_ne!(
        key,
        ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:other")),
    );

    let need_epoch = EpochBuilder::base("v1-", &["//right:right", "//left:left"], 1).build();
    let mut need_transaction = transaction(&dice, need_epoch, tracker.dupe()).await;
    let need = need_transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Need(needs) = &need else {
        panic!("missing dependency packages did not produce Need");
    };
    let paths = needs
        .path_observations()
        .unwrap()
        .demands()
        .iter()
        .map(|demand| demand.path().as_path())
        .collect::<Vec<_>>();
    assert!(paths.contains(&Path::new("/workspace/left")));
    assert!(paths.contains(&Path::new("/workspace/right")));
    assert!(!ConfiguredNodeAnalysisKey::validity(&need));
    assert!(!ConfiguredNodeAnalysisKey::equality(&need, &need));
    let need_events = tracker.take();
    assert!(analysis_batch(&need_events, "@@//parent:parent").is_none());

    let mut complete_epoch = EpochBuilder::base("v1-", &["//right:right", "//left:left"], 2);
    complete_epoch.add_leaf("left", 2);
    complete_epoch.add_leaf("right", 2);
    let mut complete_transaction = transaction(&dice, complete_epoch.build(), tracker.dupe()).await;
    let complete = complete_transaction.compute(&key).await.unwrap();
    assert!(ConfiguredNodeAnalysisKey::validity(&complete));
    assert!(ConfiguredNodeAnalysisKey::equality(&complete, &complete));
    let warm = complete_transaction.compute(&key).await.unwrap();
    let (
        AnalysisPreparationOutcome::Complete(complete_value),
        AnalysisPreparationOutcome::Complete(warm_value),
    ) = (&complete, &warm)
    else {
        unreachable!();
    };
    assert!(Arc::ptr_eq(
        complete_value.as_ref().as_ref().unwrap(),
        warm_value.as_ref().as_ref().unwrap(),
    ));
    let provider = ProviderId::new("//rules:defs.bzl", "MarkerInfo").unwrap();
    assert_eq!(marker_value(&complete, &provider), "v1-right,v1-left");
    let AnalysisPreparationOutcome::Complete(value) = &complete else {
        unreachable!();
    };
    let dependencies = value
        .as_ref()
        .as_ref()
        .unwrap()
        .direct_dependencies()
        .iter()
        .map(|key| key.label().to_string())
        .collect::<Vec<_>>();
    assert_eq!(dependencies, ["@@//right:right", "@@//left:left"]);
    let complete_events = tracker.take();
    assert_eq!(
        analysis_batch(&complete_events, "@@//left:left").map(event_texts),
        Some(vec!["LEAF_ANALYSIS"])
    );
    assert_eq!(
        analysis_batch(&complete_events, "@@//right:right").map(event_texts),
        Some(vec!["LEAF_ANALYSIS"])
    );
    assert_eq!(
        analysis_batch(&complete_events, "@@//parent:parent").map(event_texts),
        Some(vec!["PARENT_ANALYSIS"])
    );

    let mut edited_epoch = EpochBuilder::base("v2-", &["//left:left", "//right:right"], 3);
    edited_epoch.add_leaf("left", 3);
    edited_epoch.add_leaf("right", 3);
    let mut edited_transaction = transaction(&dice, edited_epoch.build(), tracker.dupe()).await;
    let edited = edited_transaction.compute(&key).await.unwrap();
    assert_eq!(marker_value(&edited, &provider), "v2-left,v2-right");
    tracker.take();

    let mut deleted_epoch = EpochBuilder::base("v2-", &["//left:left", "//right:right"], 4);
    deleted_epoch.add_leaf("left", 4);
    deleted_epoch.deleted_package("right", 4);
    let mut deleted_transaction = transaction(&dice, deleted_epoch.build(), tracker.dupe()).await;
    let deleted = deleted_transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Complete(deleted) = deleted else {
        panic!("deleted dependency package returned Need");
    };
    let error = deleted.as_ref().as_ref().unwrap_err();
    assert!(matches!(error.kind(), AnalysisErrorKind::Message(_)));
    let deleted_events = tracker.take();
    assert_eq!(
        analysis_batch(&deleted_events, "@@//parent:parent").map(event_texts),
        Some(Vec::new())
    );

    let mut restored_epoch = EpochBuilder::base("v1-", &["//right:right", "//left:left"], 5);
    restored_epoch.add_leaf("left", 5);
    restored_epoch.add_leaf("right", 5);
    let mut restored_transaction = transaction(&dice, restored_epoch.build(), tracker.dupe()).await;
    let restored = restored_transaction.compute(&key).await.unwrap();
    assert_eq!(marker_value(&restored, &provider), "v1-right,v1-left");
}
