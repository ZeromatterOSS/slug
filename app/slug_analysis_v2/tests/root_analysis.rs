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
use slug_analysis_v2::ConfiguredNodeAnalysisObservationKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeKind;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::analysis_cycle_detector;
use slug_analysis_v2::key::StarlarkOption;
use slug_analysis_v2::key::StarlarkOptionScope;
use slug_analysis_v2::prepare_configured_node_analysis_observed;
use slug_build_api_v2::ProviderId;
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
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorObservationKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::CommandRegistrationExpansionKey;
use slug_loading_v2::CommandRegistrationExpansionObservationKey;
use slug_loading_v2::HostPackageInventoryKey;
use slug_loading_v2::HostPackageInventoryObservationError;
use slug_loading_v2::HostPackageInventoryObservationKey;
use slug_loading_v2::ModuleRegistrationExpansionKey;
use slug_loading_v2::ModuleRegistrationExpansionObservationKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
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
            r#"print("BZL_LOADING")
MarkerInfo = provider(fields = {{"value": "ordered marker"}})

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
        let parent_build = format!(
            r#"print("BUILD_LOADING")
{parent_build}"#
        );
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file(
            "/workspace/MODULE.bazel",
            r#"print("MODULE_LOADING")"#,
            variant,
        );
        builder.missing("/workspace/MODULE.bazel.lock");
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder.package("rules", "", variant);
        builder.file("/workspace/rules/defs.bzl", definitions, variant);
        builder.package("parent", &parent_build, variant);
        builder.package(".slug_test_host", "platform(name = \"host\")\n", variant);
        builder.directory("/workspace/.slug_builtin", variant);
        builder.directory("/workspace/.slug_builtin/bazel_tools", variant);
        builder.file(
            "/workspace/.slug_builtin/bazel_tools/MODULE.bazel",
            "module(name = \"bazel_tools\")\n",
            variant,
        );
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

fn workspace_snapshots(
    epoch: &PathObservationEpoch,
) -> (Arc<WorkspaceSnapshot>, Arc<WorkspaceRawSnapshot>) {
    let mut text = Vec::new();
    let mut raw = Vec::new();
    for (demand, result) in epoch.observations() {
        let PathObservationResult::FileBytes(result) = result.as_ref() else {
            continue;
        };
        let path = demand.path().as_path().to_path_buf();
        match result {
            PathOperationResult::Present(bytes) => {
                raw.push((path.clone(), WorkspaceRawFileValue::Present(bytes.dupe())));
                text.push((
                    path,
                    WorkspaceFileValue::Present(Arc::new(
                        String::from_utf8(bytes.to_vec()).expect("test source is UTF-8"),
                    )),
                ));
            }
            PathOperationResult::Missing => {
                raw.push((path.clone(), WorkspaceRawFileValue::Absent));
                text.push((path, WorkspaceFileValue::Absent));
            }
            PathOperationResult::Error(error) => {
                let error = Arc::new(format!("{error:?}"));
                raw.push((path.clone(), WorkspaceRawFileValue::ReadError(error.dupe())));
                text.push((path, WorkspaceFileValue::ReadError(error)));
            }
        }
    }
    (
        Arc::new(WorkspaceSnapshot {
            files: Arc::new(text.into_iter().collect()),
        }),
        Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(raw.into_iter().collect()),
        }),
    )
}

#[derive(Debug, Clone)]
struct TrackedAnalysis {
    key: ConfiguredTargetKey,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Debug, Clone)]
struct TrackedBatch(String, ActivationKind, EventBatch);

#[derive(Default)]
struct AnalysisTracker {
    activations: Mutex<Vec<TrackedAnalysis>>,
    families: Mutex<Vec<String>>,
    batches: Mutex<Vec<TrackedBatch>>,
    dependencies: Mutex<Vec<(String, Vec<String>)>>,
}

impl AnalysisTracker {
    fn take(&self) -> Vec<TrackedAnalysis> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }

    fn take_families(&self) -> Vec<String> {
        std::mem::take(&mut *self.families.lock().unwrap())
    }

    fn take_batches(&self) -> Vec<TrackedBatch> {
        std::mem::take(&mut *self.batches.lock().unwrap())
    }

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
}

impl ActivationTracker for AnalysisTracker {
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
        if let Some(batch) = activation
            .evaluation_data()
            .and_then(|data| data.downcast_ref::<EventBatch>())
        {
            self.batches.lock().unwrap().push(TrackedBatch(
                key.to_string(),
                activation.kind(),
                batch.dupe(),
            ));
        }
        let configured = if let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() {
            self.families.lock().unwrap().push("analysis/legacy".into());
            key.configured_target()
        } else if let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisObservationKey>() {
            self.families
                .lock()
                .unwrap()
                .push("analysis/observed".into());
            key.configured_target()
        } else {
            let family = if key.downcast_ref::<HostPackageInventoryKey>().is_some() {
                Some("package/legacy")
            } else if key
                .downcast_ref::<HostPackageInventoryObservationKey>()
                .is_some()
            {
                Some("package/observed")
            } else if key.downcast_ref::<RootModuleLoadingAnchorKey>().is_some() {
                Some("anchor/legacy")
            } else if key
                .downcast_ref::<RootModuleLoadingAnchorObservationKey>()
                .is_some()
            {
                Some("anchor/observed")
            } else if key.downcast_ref::<ResolvedPathKey>().is_some() {
                Some("resolved/legacy")
            } else if key.downcast_ref::<ResolvedPathObservationKey>().is_some() {
                Some("resolved/observed")
            } else if key
                .downcast_ref::<ModuleRegistrationExpansionKey>()
                .is_some()
            {
                Some("registration/legacy")
            } else if key
                .downcast_ref::<ModuleRegistrationExpansionObservationKey>()
                .is_some()
            {
                Some("registration/observed")
            } else if key
                .downcast_ref::<CommandRegistrationExpansionKey>()
                .is_some()
            {
                Some("registration/command-legacy")
            } else if key
                .downcast_ref::<CommandRegistrationExpansionObservationKey>()
                .is_some()
            {
                Some("registration/command-observed")
            } else {
                None
            };
            if let Some(family) = family {
                self.families.lock().unwrap().push(family.into());
            }
            return;
        };
        let Some(configured_target) = configured else {
            return;
        };
        self.activations.lock().unwrap().push(TrackedAnalysis {
            key: configured_target.clone(),
            kind: activation.kind(),
            batch: activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe),
        });
    }
}

fn configured(label: &str) -> ConfiguredTargetKey {
    let configuration = SlugConfiguration::default_target(
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
    .with_host_platform_label(&CanonicalLabel::parse("@@//.slug_test_host:host").unwrap());
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(label).unwrap(),
        ConfigurationKey::from_slug(configuration),
    )
}

fn parent_key() -> ConfiguredNodeAnalysisKey {
    ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:parent")).unwrap()
}

async fn transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<AnalysisTracker>,
) -> DiceTransaction {
    let mut user_data = UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        activation_tracker: Some(tracker as Arc<dyn ActivationTracker>),
        ..Default::default()
    };
    user_data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(user_data);
    let (text, raw) = workspace_snapshots(&epoch);
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: "/workspace".into(),
            },
            text,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: "/workspace".into(),
            },
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut attributes = SmallMap::new();
    attributes.insert(
        "path".into(),
        OverrideAttributeValue::String("/workspace/.slug_builtin/bazel_tools".into()),
    );
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new("bazel_tools+").unwrap(),
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
            logical_root: NormalizedAbsolutePath::new("/workspace/.slug_builtin/bazel_tools")
                .unwrap(),
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
    inject_root_package_policy_inputs(&mut updater, package_policy()).unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace().as_path(),
        BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            workspace().as_path(),
            ["bazel_tools=/workspace/.slug_builtin/bazel_tools"],
        )
        .unwrap(),
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

async fn observed_key(
    transaction: &mut DiceTransaction,
    label: &str,
) -> ConfiguredNodeAnalysisObservationKey {
    observed_key_with_setting(transaction, label, None).await
}

async fn observed_key_with_setting(
    transaction: &mut DiceTransaction,
    label: &str,
    explicit: Option<StarlarkOption>,
) -> ConfiguredNodeAnalysisObservationKey {
    let configured = configured(label);
    let configuration = explicit.map_or_else(
        || configured.configuration().clone(),
        |explicit| configured.configuration().with_starlark_option(explicit),
    );
    match prepare_configured_node_analysis_observed(
        transaction,
        workspace(),
        configured.label().clone(),
        configuration,
    )
    .await
    {
        AnalysisPreparationOutcome::Need(_) => panic!("observed preparation returned Need"),
        AnalysisPreparationOutcome::Complete(Err(error)) => {
            panic!("observed preparation returned outer error: {error}")
        }
        AnalysisPreparationOutcome::Complete(Ok(Err(error))) => {
            panic!("observed preparation returned semantic error: {error}")
        }
        AnalysisPreparationOutcome::Complete(Ok(Ok(key))) => key,
    }
}

fn observed_result(
    outcome: &<ConfiguredNodeAnalysisObservationKey as Key>::Value,
) -> &Arc<ConfiguredNodeResult> {
    let AnalysisPreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("observed analysis did not complete semantically: {outcome:#?}");
    };
    value.as_ref().as_ref().unwrap()
}

fn observed_marker_value(
    outcome: &<ConfiguredNodeAnalysisObservationKey as Key>::Value,
    provider: &ProviderId,
) -> String {
    observed_result(outcome)
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
    let key =
        ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:missing")).unwrap();
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
        .unwrap()
    );
    assert_ne!(
        key,
        ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//parent:other")).unwrap(),
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
        .configured_dependencies()
        .map(|key| key.label().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        dependencies,
        [
            "@@//right:right",
            "@@//left:left",
            "@@//.slug_test_host:host"
        ]
    );
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

#[tokio::test]
async fn observed_analysis_is_family_isolated_recursive_and_arc_stable() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("observed-", &["//right:right", "//left:left"], 10);
    epoch.add_leaf("left", 10);
    epoch.add_leaf("right", 10);
    let epoch = epoch.build();
    let mut observed_transaction = transaction(&dice, epoch.dupe(), tracker.dupe()).await;
    let key = observed_key(&mut observed_transaction, "@@//parent:parent").await;
    assert_eq!(
        key.node(),
        &ConfiguredNodeKey::configured(configured("@@//parent:parent"))
    );

    let first = observed_transaction.compute(&key).await.unwrap();
    assert!(ConfiguredNodeAnalysisObservationKey::validity(&first));
    assert!(ConfiguredNodeAnalysisObservationKey::equality(
        &first, &first
    ));
    let warm = observed_transaction.compute(&key).await.unwrap();
    let (
        AnalysisPreparationOutcome::Complete(Ok(first_carrier)),
        AnalysisPreparationOutcome::Complete(Ok(warm_carrier)),
    ) = (&first, &warm)
    else {
        panic!("observed analysis did not produce complete carriers")
    };
    assert!(Arc::ptr_eq(first_carrier, warm_carrier));
    let provider = ProviderId::new("//rules:defs.bzl", "MarkerInfo").unwrap();
    assert_eq!(
        observed_marker_value(&first, &provider),
        "observed-right,observed-left"
    );
    assert_eq!(
        observed_result(&first)
            .configured_dependencies()
            .map(|key| key.label().to_string())
            .collect::<Vec<_>>(),
        [
            "@@//right:right",
            "@@//left:left",
            "@@//.slug_test_host:host"
        ]
    );

    let events = tracker.take();
    assert_eq!(
        analysis_batch(&events, "@@//left:left").map(event_texts),
        Some(vec!["LEAF_ANALYSIS"])
    );
    assert_eq!(
        analysis_batch(&events, "@@//right:right").map(event_texts),
        Some(vec!["LEAF_ANALYSIS"])
    );
    assert_eq!(
        analysis_batch(&events, "@@//parent:parent").map(event_texts),
        Some(vec!["PARENT_ANALYSIS"])
    );
    let batches = tracker.take_batches();
    let batch_texts = batches
        .iter()
        .flat_map(|tracked| event_texts(&tracked.2))
        .collect::<Vec<_>>();
    assert_eq!(
        batch_texts,
        [
            "MODULE_LOADING",
            "BZL_LOADING",
            "BUILD_LOADING",
            "LEAF_ANALYSIS",
            "LEAF_ANALYSIS",
            "PARENT_ANALYSIS",
        ],
        "{batches:#?}"
    );
    assert!(
        batches
            .iter()
            .filter(|tracked| !tracked.2.events().is_empty())
            .all(|tracked| tracked.1 == ActivationKind::Evaluated),
        "{batches:#?}"
    );
    let families = tracker.take_families();
    assert!(families.iter().any(|family| family == "package/observed"));
    assert!(
        families
            .iter()
            .filter(|family| family.as_str() == "analysis/observed")
            .count()
            >= 3
    );
    assert!(
        families.iter().all(|family| !family.ends_with("/legacy")),
        "{families:#?}"
    );
    let dependencies = tracker.dependencies(&key);
    assert!(
        dependencies
            .iter()
            .any(|dependency| dependency.starts_with("observed-host-package-inventory:"))
    );
    assert!(
        dependencies
            .iter()
            .all(|dependency| !dependency.starts_with("observed-host-package-load:"))
    );
    assert!(
        families
            .iter()
            .any(|family| family == "registration/observed")
    );
    assert!(
        families
            .iter()
            .any(|family| family == "registration/command-observed")
    );

    let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let legacy_tracker = Arc::new(AnalysisTracker::default());
    let mut legacy_transaction = transaction(&legacy_dice, epoch, legacy_tracker.dupe()).await;
    let legacy = legacy_transaction.compute(&parent_key()).await.unwrap();
    let AnalysisPreparationOutcome::Complete(legacy) = legacy else {
        panic!("legacy parity analysis returned Need")
    };
    assert_eq!(
        legacy.as_ref().as_ref().unwrap().as_ref(),
        observed_result(&first).as_ref()
    );
    let legacy_dependencies = legacy_tracker.dependencies(&parent_key());
    assert!(
        legacy_dependencies
            .iter()
            .any(|dependency| dependency.starts_with("host-package-inventory:"))
    );
    assert!(
        legacy_dependencies
            .iter()
            .all(|dependency| !dependency.starts_with("host-package-load:"))
    );
    let legacy_families = legacy_tracker.take_families();
    assert!(
        legacy_families
            .iter()
            .any(|family| family == "registration/legacy")
    );
    assert!(
        legacy_families
            .iter()
            .any(|family| family == "registration/command-legacy")
    );
}

#[tokio::test]
async fn observed_null_source_uses_only_observed_resolution() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("source-", &[], 20);
    epoch.package(
        "source",
        r#"load("//rules:defs.bzl", "parent")
parent(name = "declares", deps = [":data.txt"])
"#,
        20,
    );
    epoch.file("/workspace/source/data.txt", b"payload", 20);
    let mut transaction = transaction(&dice, epoch.build(), tracker.dupe()).await;
    let label = CanonicalLabel::parse("@@//source:data.txt").unwrap();
    let key =
        ConfiguredNodeAnalysisObservationKey::new(workspace(), ConfiguredNodeKey::null(label))
            .unwrap();
    let outcome = transaction.compute(&key).await.unwrap();
    assert_eq!(
        observed_result(&outcome).kind(),
        &ConfiguredNodeKind::SourceFile
    );
    let families = tracker.take_families();
    assert!(families.iter().any(|family| family == "resolved/observed"));
    assert!(families.iter().any(|family| family == "package/observed"));
    assert!(
        families.iter().all(|family| !family.ends_with("/legacy")),
        "{families:#?}"
    );
}

const OBSERVED_TOOLCHAIN_MODULE: &str = r#"module(name = "root")
register_execution_platforms("//:platform")
register_toolchains("//:toolchain")
"#;
const OBSERVED_TOOLCHAIN_DEFS: &str = r#"ConsumerInfo = provider(fields = {"value": ""})
def _implementation(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]
def _request(ctx):
    return [ConsumerInfo(value = ctx.toolchains["//:type"].marker)]
implementation = rule(implementation = _implementation, attrs = {"marker": attr.string(mandatory = True)})
request = rule(implementation = _request, toolchains = ["//:type"])
"#;
const OBSERVED_TOOLCHAIN_BUILD: &str = r#"load(":defs.bzl", "implementation", "request")
constraint_setting(name = "setting")
constraint_value(name = "linux", constraint_setting = ":setting")
platform(name = "platform", constraint_values = [":linux"])
toolchain_type(name = "type")
implementation(name = "implementation", marker = "selected")
toolchain(name = "toolchain", toolchain_type = ":type", toolchain = ":implementation", exec_compatible_with = [":linux"])
request(name = "request")
"#;

#[tokio::test]
async fn observed_toolchain_closure_depends_on_both_sources_and_families_once() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("toolchain-", &[], 30);
    epoch.file("/workspace/MODULE.bazel", OBSERVED_TOOLCHAIN_MODULE, 30);
    epoch.file("/workspace/defs.bzl", OBSERVED_TOOLCHAIN_DEFS, 30);
    epoch.file("/workspace/BUILD.bazel", OBSERVED_TOOLCHAIN_BUILD, 30);
    let mut transaction = transaction(&dice, epoch.build(), tracker.dupe()).await;
    let key = observed_key(&mut transaction, "@@//:request").await;
    let outcome = transaction.compute(&key).await.unwrap();
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    assert_eq!(
        observed_result(&outcome)
            .providers()
            .user(&consumer)
            .unwrap()
            .field("value"),
        Some("selected")
    );
    let dependencies = tracker.dependencies(&key);
    let registrations = dependencies
        .iter()
        .filter(|dependency| dependency.contains("registration-expansion:"))
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), 4, "{dependencies:#?}");
    assert!(registrations[0].starts_with("observed-command-registration-expansion:"));
    assert!(registrations[0].ends_with(":execution-platforms"));
    assert!(registrations[1].starts_with("observed-module-registration-expansion:"));
    assert!(registrations[1].ends_with(":execution-platforms"));
    assert!(registrations[2].starts_with("observed-command-registration-expansion:"));
    assert!(registrations[2].ends_with(":toolchains"));
    assert!(registrations[3].starts_with("observed-module-registration-expansion:"));
    assert!(registrations[3].ends_with(":toolchains"));
    assert!(
        dependencies
            .iter()
            .all(|dependency| !dependency.starts_with("observed-root-module-loading-anchor:"))
    );
    let families = tracker.take_families();
    assert!(families.iter().any(|family| family == "anchor/observed"));
    assert!(
        families
            .iter()
            .filter(|family| family.as_str() == "registration/observed")
            .count()
            >= 2
    );
    assert!(
        families
            .iter()
            .filter(|family| family.as_str() == "registration/command-observed")
            .count()
            >= 2
    );
    assert!(families.iter().any(|family| family == "package/observed"));
    assert!(
        families
            .iter()
            .filter(|family| family.as_str() == "analysis/observed")
            .count()
            >= 2
    );
    assert!(
        families.iter().all(|family| !family.ends_with("/legacy")),
        "{families:#?}"
    );
}

#[tokio::test]
async fn observed_analysis_unions_needs_without_publishing_events() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let epoch = EpochBuilder::base("need-", &["//right:right", "//left:left"], 40).build();
    let mut transaction = transaction(&dice, epoch, tracker.dupe()).await;
    let key = observed_key(&mut transaction, "@@//parent:parent").await;
    let outcome = transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Need(needs) = &outcome else {
        panic!("missing observed dependency packages did not produce Need")
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
    assert!(!ConfiguredNodeAnalysisObservationKey::validity(&outcome));
    assert!(!ConfiguredNodeAnalysisObservationKey::equality(
        &outcome, &outcome
    ));
    assert!(analysis_batch(&tracker.take(), "@@//parent:parent").is_none());
}

#[tokio::test]
async fn observed_outer_wins_need_and_semantic_while_semantic_error_publishes_once() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base(
        "terminal-",
        &["//need:leaf", "//semantic:missing", "//outer:leaf"],
        41,
    );
    epoch.package("semantic", "", 41);
    let transaction = transaction(&dice, epoch.build(), tracker.dupe()).await;
    let outer = ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(
        EpochBuilder::demand("/outer", PathObservationOperation::Lstat),
    ));
    let outer_value: <HostPackageInventoryObservationKey as Key>::Value =
        AnalysisPreparationOutcome::Complete(Err(HostPackageInventoryObservationError::Frontier(
            outer.clone(),
        )));
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            HostPackageInventoryObservationKey::new(
                workspace(),
                configured("@@//outer:leaf").label().package().clone(),
            ),
            outer_value,
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let key = observed_key(&mut transaction, "@@//parent:parent").await;
    tracker.take_batches();
    let outcome = transaction.compute(&key).await.unwrap();
    assert!(matches!(
        &outcome,
        AnalysisPreparationOutcome::Complete(Err(error)) if error == &outer
    ));
    assert!(ConfiguredNodeAnalysisObservationKey::validity(&outcome));
    assert!(ConfiguredNodeAnalysisObservationKey::equality(
        &outcome, &outcome
    ));
    assert!(
        tracker
            .take_batches()
            .iter()
            .all(|tracked| { !tracked.0.starts_with("observed-configured-node-analysis:") })
    );
    let error_key =
        ConfiguredNodeAnalysisObservationKey::new(workspace(), configured("@@//parent:missing"))
            .unwrap();
    let error = transaction.compute(&error_key).await.unwrap();
    assert!(!ConfiguredNodeAnalysisObservationKey::validity(&error));
    assert!(!ConfiguredNodeAnalysisObservationKey::equality(
        &error, &error
    ));
    assert_eq!(
        analysis_batch(&tracker.take(), "@@//parent:missing").map(event_texts),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn combined_detector_preserves_bzl_cycle_diagnostics() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("cycle-", &[], 41);
    epoch.file(
        "/workspace/rules/defs.bzl",
        "load(\"//rules:other.bzl\", \"other\")\nleaf = other\nparent = other\n",
        41,
    );
    epoch.file(
        "/workspace/rules/other.bzl",
        "load(\"//rules:defs.bzl\", \"leaf\")\nother = leaf\n",
        41,
    );
    let mut transaction = transaction(&dice, epoch.build(), tracker).await;
    let outcome = transaction.compute(&parent_key()).await.unwrap();
    let AnalysisPreparationOutcome::Complete(result) = outcome else {
        panic!("bzl cycle returned Need");
    };
    let error = result.as_ref().as_ref().unwrap_err().to_string();
    assert!(
        error.contains("cycle detected in extension files"),
        "{error}"
    );
    assert!(error.contains("//rules:defs.bzl"), "{error}");
    assert!(error.contains("//rules:other.bzl"), "{error}");
}

#[tokio::test]
async fn legacy_alias_cycle_fails_and_same_graph_repair_recovers() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut cycle = EpochBuilder::base("cycle-", &[], 42);
    cycle.file(
        "/workspace/BUILD.bazel",
        "alias(name = \"a\", actual = \":b\")\nalias(name = \"b\", actual = \":a\")\n",
        42,
    );
    let key = ConfiguredNodeAnalysisKey::new(workspace(), configured("@@//:a")).unwrap();
    let cycle = cycle.build();
    let mut cancelled = transaction(&dice, cycle.dupe(), tracker.dupe()).await;
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    drop(cancelled);
    let mut cycle_transaction = transaction(&dice, cycle, tracker.dupe()).await;
    let outcome = cycle_transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Complete(result) = outcome else {
        panic!("legacy alias cycle returned Need");
    };
    assert!(
        result
            .as_ref()
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("configured alias cycle")
    );

    let mut repaired = EpochBuilder::base("cycle-", &[], 43);
    repaired.file(
        "/workspace/BUILD.bazel",
        "load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"good\")\nalias(name = \"a\", actual = \":good\")\n",
        43,
    );
    let mut repaired_transaction = transaction(&dice, repaired.build(), tracker).await;
    let result = repaired_transaction.compute(&key).await.unwrap();
    let AnalysisPreparationOutcome::Complete(result) = result else {
        panic!("repaired alias returned Need");
    };
    assert_eq!(
        result.as_ref().as_ref().unwrap().kind(),
        &ConfiguredNodeKind::Alias
    );
}

#[tokio::test]
async fn observed_alias_and_generated_edges_do_not_escape_the_family() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("shape-", &[], 43);
    epoch.file(
        "/workspace/shape.bzl",
        r#"def _producer(ctx): return [DefaultInfo()]
producer = rule(implementation = _producer, attrs = {"out": attr.output()})
"#,
        43,
    );
    epoch.file(
        "/workspace/BUILD.bazel",
        r#"load(":shape.bzl", "producer")
producer(name = "producer", out = "producer.out")
alias(name = "alias", actual = ":producer")
"#,
        43,
    );
    let mut transaction = transaction(&dice, epoch.build(), tracker.dupe()).await;
    let alias_key = observed_key(&mut transaction, "@@//:alias").await;
    let alias = transaction.compute(&alias_key).await.unwrap();
    assert_eq!(observed_result(&alias).kind(), &ConfiguredNodeKind::Alias);
    let generated_key = observed_key(&mut transaction, "@@//:producer.out").await;
    let generated = transaction.compute(&generated_key).await.unwrap();
    assert_eq!(
        observed_result(&generated).kind(),
        &ConfiguredNodeKind::GeneratedFile
    );
    assert!(
        tracker
            .take_families()
            .iter()
            .all(|family| !family.ends_with("/legacy"))
    );
}

#[tokio::test]
async fn observed_cancellation_publishes_no_parent_and_recovers() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let mut epoch = EpochBuilder::base("cancel-", &["//left:left"], 44);
    epoch.add_leaf("left", 44);
    let epoch = epoch.build();
    let mut cancelled = transaction(&dice, epoch.dupe(), tracker.dupe()).await;
    let key = observed_key(&mut cancelled, "@@//parent:parent").await;
    tracker.take_batches();
    let mut future = Box::pin(cancelled.compute(&key));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(
        tracker
            .take_batches()
            .iter()
            .all(|tracked| !tracked.0.contains("@@//parent:parent"))
    );
    drop(cancelled);
    let mut recovered = transaction(&dice, epoch, tracker).await;
    assert!(
        observed_result(&recovered.compute(&key).await.unwrap())
            .providers()
            .default_info()
            .is_some()
    );
}

const OBSERVED_SETTING_DEFS: &str = r#"ConsumerInfo = provider(fields = {"value": ""}); SettingInfo = provider(fields = {"value": ""})
def _setting(ctx): return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx): return [ConsumerInfo(value = ctx.attr._setting[SettingInfo].value)]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
"#;

fn observed_setting_epoch(default: &str, variant: i64) -> PathObservationEpoch {
    let mut epoch = EpochBuilder::base("setting-", &[], variant);
    epoch.file("/workspace/defs.bzl", OBSERVED_SETTING_DEFS, variant);
    epoch.file(
        "/workspace/BUILD.bazel",
        format!(
            r#"load(":defs.bzl", "consumer", "string_setting")
string_setting(name = "setting", build_setting_default = "{default}")
consumer(name = "consumer")
"#
        ),
        variant,
    );
    epoch.build()
}

#[tokio::test]
async fn observed_analysis_preserves_typed_configuration_and_restore_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisTracker::default());
    let provider = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();

    let default_epoch = observed_setting_epoch("default", 50);
    let mut default_transaction = transaction(&dice, default_epoch.dupe(), tracker.dupe()).await;
    let default_key = observed_key(&mut default_transaction, "@@//:consumer").await;
    let default = default_transaction.compute(&default_key).await.unwrap();
    assert_eq!(observed_marker_value(&default, &provider), "default");

    let mut explicit_transaction = transaction(&dice, default_epoch, tracker.dupe()).await;
    let explicit_key = observed_key_with_setting(
        &mut explicit_transaction,
        "@@//:consumer",
        Some(StarlarkOption::string(
            CanonicalLabel::parse("@@//:setting").unwrap(),
            "command",
            StarlarkOptionScope::Default,
        )),
    )
    .await;
    let explicit = explicit_transaction.compute(&explicit_key).await.unwrap();
    assert_eq!(observed_marker_value(&explicit, &provider), "command");

    let mut edited_transaction =
        transaction(&dice, observed_setting_epoch("edited", 51), tracker.dupe()).await;
    let edited_key = observed_key(&mut edited_transaction, "@@//:consumer").await;
    let edited = edited_transaction.compute(&edited_key).await.unwrap();
    assert_eq!(observed_marker_value(&edited, &provider), "edited");

    let mut restored_transaction =
        transaction(&dice, observed_setting_epoch("default", 52), tracker.dupe()).await;
    let restored_key = observed_key(&mut restored_transaction, "@@//:consumer").await;
    let restored = restored_transaction.compute(&restored_key).await.unwrap();
    assert_eq!(restored_key, default_key);
    assert_eq!(observed_marker_value(&restored, &provider), "default");
    assert_eq!(observed_result(&restored), observed_result(&default));

    let families = tracker.take_families();
    assert!(families.iter().any(|family| family == "package/observed"));
    assert!(
        families.iter().all(|family| !family.ends_with("/legacy")),
        "{families:#?}"
    );
}
