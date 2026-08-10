/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceNodeId;
use dice::DynKey;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_analysis_v2::AnalysisError;
use slug_analysis_v2::AnalysisErrorKind;
use slug_analysis_v2::AnalysisPreparationOutcome;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredEdgeKind;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeKind;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::key::RootStringSettingValue;
use slug_analysis_v2::prepare_configured_node_analysis;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ProviderId;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
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
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
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
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use starlark_map::small_map::SmallMap;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum EventKind {
    Evaluated,
    Reused,
}

#[derive(Default)]
struct AnalysisTracker {
    events: Mutex<Vec<AnalysisActivation>>,
}

#[derive(Debug)]
struct AnalysisActivation {
    label: String,
    kind: EventKind,
    node: DiceNodeId,
    dependencies: Vec<DiceNodeId>,
}

impl AnalysisTracker {
    fn take(&self) -> Vec<AnalysisActivation> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl ActivationTracker for AnalysisTracker {
    fn key_activated(
        &self,
        _key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation_data: ActivationData,
    ) {
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() else {
            return;
        };
        let kind = match activation.kind() {
            ActivationKind::Evaluated => EventKind::Evaluated,
            ActivationKind::Reused => EventKind::Reused,
        };
        self.events.lock().unwrap().push(AnalysisActivation {
            label: key.node().label().to_string(),
            kind,
            node: activation.node(),
            dependencies: activation.dependencies().to_vec(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnalysisEventActivation {
    workspace: PathBuf,
    configured_target: ConfiguredTargetKey,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct AnalysisEventTracker {
    activations: Mutex<Vec<AnalysisEventActivation>>,
}

impl AnalysisEventTracker {
    fn take(&self) -> Vec<AnalysisEventActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for AnalysisEventTracker {
    fn key_activated(
        &self,
        _key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation_data: ActivationData,
    ) {
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() else {
            return;
        };
        let Some(configured_target) = key.configured_target() else {
            return;
        };
        self.activations
            .lock()
            .unwrap()
            .push(AnalysisEventActivation {
                workspace: key.workspace().as_path().to_path_buf(),
                configured_target: configured_target.clone(),
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
    }
}

fn event_texts(batch: &EventBatch) -> Vec<&str> {
    batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => {
                unreachable!("diagnostic events are not produced by this packet")
            }
        })
        .collect()
}

fn analysis_event<'a>(
    activations: &'a [AnalysisEventActivation],
    workspace: &std::path::Path,
    configured_target: &ConfiguredTargetKey,
) -> &'a AnalysisEventActivation {
    let mut matching = activations.iter().filter(|activation| {
        activation.kind == ActivationKind::Evaluated
            && activation.workspace == workspace
            && &activation.configured_target == configured_target
    });
    let activation = matching
        .next()
        .unwrap_or_else(|| panic!("missing evaluated activation: {activations:#?}"));
    assert!(
        matching.next().is_none(),
        "duplicate evaluated activation: {activations:#?}"
    );
    activation
}

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-analysis-rule-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn directory_snapshot(root: &std::path::Path) -> WorkspaceDirectorySnapshot {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let kind = if file_type.is_file() {
                WorkspaceDirectoryEntryKind::RegularFile
            } else if file_type.is_dir() {
                pending.push(entry.path());
                WorkspaceDirectoryEntryKind::Directory
            } else if file_type.is_symlink() {
                WorkspaceDirectoryEntryKind::Symlink
            } else {
                WorkspaceDirectoryEntryKind::Other
            };
            entries.push(WorkspaceDirectoryEntry {
                name: entry.file_name().to_str().unwrap().into(),
                kind,
            });
        }
        directories.push((directory, WorkspaceDirectoryValue::present(entries)));
    }
    WorkspaceDirectorySnapshot {
        directories: Arc::new(directories.into_iter().collect()),
    }
}

fn workspace_snapshot(root: &std::path::Path) -> WorkspaceSnapshot {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else if entry.file_type().unwrap().is_file() {
                files.push((
                    entry.path(),
                    WorkspaceFileValue::Present(Arc::new(
                        fs::read_to_string(entry.path()).unwrap(),
                    )),
                ));
            }
        }
    }
    WorkspaceSnapshot {
        files: Arc::new(files.into_iter().collect()),
    }
}

fn raw_snapshot_from_text(snapshot: &WorkspaceSnapshot) -> Arc<WorkspaceRawSnapshot> {
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(
            snapshot
                .files
                .iter()
                .map(|(path, value)| {
                    let value = match value {
                        WorkspaceFileValue::Present(source) => {
                            WorkspaceRawFileValue::Present(Arc::from(source.as_bytes()))
                        }
                        WorkspaceFileValue::Absent => WorkspaceRawFileValue::Absent,
                        WorkspaceFileValue::ReadError(error) => {
                            WorkspaceRawFileValue::ReadError(error.clone())
                        }
                    };
                    (path.clone(), value)
                })
                .collect(),
        ),
    })
}

fn root_epoch(root: &std::path::Path) -> PathObservationEpoch {
    root_epoch_with_missing(root, std::iter::empty::<PathBuf>())
}

fn root_epoch_with_missing(
    root: &std::path::Path,
    missing: impl IntoIterator<Item = PathBuf>,
) -> PathObservationEpoch {
    let mut entries = SmallMap::new();
    let snapshot = workspace_snapshot(root);
    let mut directories = BTreeSet::from([root.to_path_buf()]);
    for (path, value) in snapshot.files.iter() {
        let demand = |operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path.clone()).unwrap(),
                operation,
            )
        };
        entries.insert(
            demand(PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                1,
                1,
                1,
                0o644,
            ))),
        );
        if let WorkspaceFileValue::Present(value) = value {
            entries.insert(
                demand(PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    value.as_bytes(),
                ))),
            );
        }
        let mut parent = path.parent();
        while let Some(directory) = parent.filter(|directory| directory.starts_with(root)) {
            directories.insert(directory.to_path_buf());
            entries.insert(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(directory.to_path_buf()).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::Directory,
                    1,
                    1,
                    1,
                    1,
                    0o755,
                ))),
            );
            parent = directory.parent();
        }
    }
    let mut path = Some(root);
    while let Some(directory) = path {
        entries.insert(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(directory.to_path_buf()).unwrap(),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                1,
                1,
                1,
                1,
                0o755,
            ))),
        );
        path = directory.parent();
    }
    for name in ["REPO.bazel", ".bazelignore", "BUILD"] {
        let path = root.join(name);
        if snapshot.files.get(&path).is_none() {
            entries.insert(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            );
        }
    }
    for directory in directories {
        for name in ["BUILD", "BUILD.bazel"] {
            let path = directory.join(name);
            if snapshot.files.get(&path).is_none() {
                entries.insert(
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                );
            }
        }
    }
    for path in missing {
        entries.insert(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }
    PathObservationEpoch::new(entries).unwrap()
}

#[derive(Default)]
struct RootActivationTracker {
    activations: Mutex<Vec<(String, ActivationKind)>>,
    batches: Mutex<Vec<(String, EventBatch)>>,
    nodes: Mutex<Vec<(String, DiceNodeId, Vec<DiceNodeId>)>>,
    all_loading: bool,
}

impl RootActivationTracker {
    fn with_loading() -> Self {
        Self {
            all_loading: true,
            ..Default::default()
        }
    }
    fn take(
        &self,
    ) -> (
        Vec<(String, ActivationKind)>,
        Vec<(String, EventBatch)>,
        Vec<(String, DiceNodeId, Vec<DiceNodeId>)>,
    ) {
        (
            std::mem::take(&mut *self.activations.lock().unwrap()),
            std::mem::take(&mut *self.batches.lock().unwrap()),
            std::mem::take(&mut *self.nodes.lock().unwrap()),
        )
    }
}

fn root_activation_identity(key: &ConfiguredNodeAnalysisKey) -> String {
    format!(
        "resolved/{}={}",
        key.configured_target()
            .expect("root-string analysis only activates configured targets")
            .label(),
        key.configured_target()
            .expect("root-string analysis only activates configured targets")
            .configuration()
            .root_string_setting()
            .map_or("<default>", RootStringSettingValue::as_str)
    )
}

fn test_configuration() -> ConfigurationKey {
    ConfigurationKey::from_slug(
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
        .unwrap(),
    )
}

async fn prepared_analysis_key(
    transaction: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    target: CanonicalLabel,
    base_configuration: ConfigurationKey,
    explicit: Option<RootStringSettingValue>,
) -> Result<ConfiguredNodeAnalysisKey, String> {
    match prepare_configured_node_analysis(
        transaction,
        workspace,
        target,
        base_configuration,
        explicit,
    )
    .await
    {
        AnalysisPreparationOutcome::Need(_) => Err("root request returned Needs".to_owned()),
        AnalysisPreparationOutcome::Complete(Ok(key)) => Ok(key),
        AnalysisPreparationOutcome::Complete(Err(error)) => Err(error.to_string()),
    }
}

fn activation_codes(activations: &[(String, ActivationKind)]) -> Vec<String> {
    let mut codes = activations
        .iter()
        .map(|(identity, kind)| {
            format!(
                "{identity}:{}",
                match kind {
                    ActivationKind::Evaluated => 'E',
                    ActivationKind::Reused => 'R',
                }
            )
        })
        .collect::<Vec<_>>();
    codes.sort();
    codes
}

impl ActivationTracker for RootActivationTracker {
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
        if let Some(root_key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() {
            let identity = root_activation_identity(root_key);
            self.activations
                .lock()
                .unwrap()
                .push((identity.clone(), activation.kind()));
            self.nodes.lock().unwrap().push((
                identity.clone(),
                activation.node(),
                activation.dependencies().to_vec(),
            ));
            if let Some(batch) = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
            {
                self.batches.lock().unwrap().push((identity, batch.dupe()));
            }
        } else if self.all_loading {
            let identity = if let Some(key) = key.downcast_ref::<RootPackageLoadKey>() {
                Some(format!("package/{key}"))
            } else if key.downcast_ref::<RootModuleLoadingAnchorKey>().is_some() {
                Some("anchor".to_owned())
            } else {
                None
            };
            if let Some(identity) = identity {
                self.activations
                    .lock()
                    .unwrap()
                    .push((identity.clone(), activation.kind()));
                self.nodes.lock().unwrap().push((
                    identity,
                    activation.node(),
                    activation.dependencies().to_vec(),
                ));
            }
        }
    }
}

async fn root_string_request_result(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    explicit: Option<&str>,
    tracker: Arc<RootActivationTracker>,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_string_request_result_with_explicit(
        dice,
        workspace,
        target,
        explicit.map(RootStringSettingValue::new),
        tracker,
    )
    .await
}

async fn root_string_request_result_with_explicit(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    explicit: Option<RootStringSettingValue>,
    tracker: Arc<RootActivationTracker>,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    });
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(workspace))])
        .unwrap();
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            root.clone(),
            [root],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let analysis_key = prepared_analysis_key(
        &mut transaction,
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        CanonicalLabel::parse(target).unwrap(),
        test_configuration(),
        explicit,
    )
    .await?;
    let outcome = transaction
        .compute(&analysis_key)
        .await
        .map_err(|error| error.to_string())?;
    let AnalysisPreparationOutcome::Complete(value) = outcome else {
        return Err("root request returned Needs".to_owned());
    };
    value
        .as_ref()
        .as_ref()
        .cloned()
        .map_err(|error| error.to_string())
}

async fn root_string_request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    explicit: Option<&str>,
    tracker: Arc<RootActivationTracker>,
) -> Arc<ConfiguredNodeResult> {
    root_string_request_result(dice, workspace, target, explicit, tracker)
        .await
        .unwrap()
}

async fn root_target_request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    tracker: Arc<RootActivationTracker>,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(dice, workspace, target, test_configuration(), tracker)
        .await
}

async fn root_target_request_with_configuration(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<RootActivationTracker>,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut user_data = UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    user_data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(workspace))])
        .unwrap();
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            root.clone(),
            [root],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let analysis_key = prepared_analysis_key(
        &mut transaction,
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        CanonicalLabel::parse(target).unwrap(),
        configuration,
        None,
    )
    .await?;
    let value = transaction
        .compute(&analysis_key)
        .await
        .map_err(|error| error.to_string())?;
    let AnalysisPreparationOutcome::Complete(value) = value else {
        return Err("root target returned Needs".to_owned());
    };
    value
        .as_ref()
        .as_ref()
        .cloned()
        .map_err(|error| error.to_string())
}

fn provider_value(result: &ConfiguredNodeResult, provider: &ProviderId) -> String {
    result
        .providers()
        .user(provider)
        .unwrap()
        .field("value")
        .unwrap()
        .to_owned()
}

fn candidate_labels(result: &ConfiguredNodeResult) -> Vec<String> {
    result
        .toolchain_topology()
        .expect("toolchain topology is retained")
        .candidate_execution_platforms()
        .iter()
        .map(|platform| platform.label().to_string())
        .collect()
}

fn root_setting_value(key: &ConfiguredTargetKey) -> &str {
    key.configuration().root_string_setting().unwrap().as_str()
}

const TOOLCHAIN_MODULE: &str = "module(name = \"root\")\nregister_execution_platforms(\"//:platform\")\nregister_toolchains(\"//:second\", \"//:first\")\n";
const TOOLCHAIN_DEFS: &str = r#"ConsumerInfo = provider(fields = {"value": ""})
def _first(ctx):
    print("FIRST_LOCAL")
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]
def _second(ctx):
    print("SECOND_LOCAL")
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]
def _request(ctx):
    print("REQUEST_LOCAL")
    return [ConsumerInfo(value = ctx.toolchains["//:type"].marker)]
first_impl = rule(implementation = _first, attrs = {"marker": attr.string(mandatory = True)})
second_impl = rule(implementation = _second, attrs = {"marker": attr.string(mandatory = True)})
request = rule(implementation = _request, toolchains = ["//:type"])
"#;
const TOOLCHAIN_BUILD: &str = "load(\":defs.bzl\", \"first_impl\", \"request\", \"second_impl\")\nconstraint_setting(name = \"setting\")\nconstraint_value(name = \"linux\", constraint_setting = \":setting\")\nconstraint_value(name = \"other\", constraint_setting = \":setting\")\nplatform(name = \"platform\", constraint_values = [\":linux\"])\ntoolchain_type(name = \"type\")\nfirst_impl(name = \"first_impl\", marker = \"first\")\nsecond_impl(name = \"second_impl\", marker = \"second\")\ntoolchain(name = \"first\", toolchain_type = \":type\", toolchain = \":first_impl\", exec_compatible_with = [\":linux\"])\ntoolchain(name = \"second\", toolchain_type = \":type\", toolchain = \":second_impl\", exec_compatible_with = [\":linux\"])\nrequest(name = \"request\")\n";
const TOPOLOGY_MODULE: &str = "module(name = \"root\")\nregister_execution_platforms(\"//:first_platform\", \"//:second_platform\")\nregister_toolchains(\"//:first_toolchain\", \"//:second_toolchain\")\n";
const TOPOLOGY_BUILD: &str = "load(\":defs.bzl\", \"first_impl\", \"request\", \"second_impl\")\nconstraint_setting(name = \"selection\")\nconstraint_value(name = \"first\", constraint_setting = \":selection\")\nconstraint_value(name = \"second\", constraint_setting = \":selection\")\nplatform(name = \"first_platform\", constraint_values = [\":first\"])\nplatform(name = \"second_platform\", constraint_values = [\":second\"])\ntoolchain_type(name = \"type\")\nfirst_impl(name = \"first_impl\", marker = \"first\")\nsecond_impl(name = \"second_impl\", marker = \"second\")\nfirst_impl(name = \"orphan\", marker = \"orphan\")\ntoolchain(name = \"first_toolchain\", toolchain_type = \":type\", toolchain = \":first_impl\", exec_compatible_with = [\":first\"])\ntoolchain(name = \"second_toolchain\", toolchain_type = \":type\", toolchain = \":second_impl\", exec_compatible_with = [\":second\"])\nrequest(name = \"request\")\n";

async fn toolchain_case(
    module: &str,
    defs: &str,
    build: &str,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), module).unwrap();
    fs::write(workspace.join("defs.bzl"), defs).unwrap();
    fs::write(workspace.join("BUILD.bazel"), build).unwrap();
    root_target_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

#[tokio::test]
async fn root_toolchain_selection_prepares_builtin_marker_context_in_registration_order() {
    let workspace = scratch();
    let module = workspace.join("MODULE.bazel");
    let defs = workspace.join("defs.bzl");
    let build = workspace.join("BUILD.bazel");
    fs::write(&module, TOOLCHAIN_MODULE).unwrap();
    fs::write(&defs, TOOLCHAIN_DEFS).unwrap();
    fs::write(&build, TOOLCHAIN_BUILD).unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    let first = root_target_request(&dice, &workspace, "@@//:request", tracker.clone())
        .await
        .unwrap();
    assert_eq!(provider_value(&first, &consumer), "second");
    assert!(first.actions().is_empty());
    assert!(first.declared_outputs().is_empty());
    assert!(first.diagnostics().is_empty());
    assert_eq!(candidate_labels(&first), vec!["@@//:platform"]);
    let selection = first.toolchain_topology().unwrap().selection().unwrap();
    assert_eq!(
        selection.execution_platform().label().to_string(),
        "@@//:platform"
    );
    assert_eq!(selection.declaration().to_string(), "@@//:second");
    assert_eq!(selection.toolchain_type().label().to_string(), "@@//:type");
    assert_eq!(
        selection.implementation().label().to_string(),
        "@@//:second_impl"
    );
    assert_eq!(
        first
            .edges()
            .iter()
            .map(|edge| edge.kind())
            .collect::<Vec<_>>(),
        vec![
            &ConfiguredEdgeKind::ToolchainRequirement,
            &ConfiguredEdgeKind::SelectedToolchainImplementation,
            &ConfiguredEdgeKind::CandidateExecutionPlatform { index: 0 },
        ]
    );
    assert!(
        first
            .edges()
            .iter()
            .all(|edge| edge.implicit() && !edge.tool())
    );
    let warm = root_target_request(&dice, &workspace, "@@//:request", tracker.clone())
        .await
        .unwrap();
    assert_eq!(warm, first);

    fs::write(
        &module,
        TOOLCHAIN_MODULE.replacen(
            "\"//:second\", \"//:first\"",
            "\"//:first\", \"//:second\"",
            1,
        ),
    )
    .unwrap();
    let reordered = root_target_request(&dice, &workspace, "@@//:request", tracker.clone())
        .await
        .unwrap();
    assert_eq!(provider_value(&reordered, &consumer), "first");
    assert_eq!(
        reordered
            .toolchain_topology()
            .unwrap()
            .selection()
            .unwrap()
            .declaration()
            .to_string(),
        "@@//:first"
    );

    fs::write(&module, TOOLCHAIN_MODULE).unwrap();
    let restored = root_target_request(&dice, &workspace, "@@//:request", tracker.clone())
        .await
        .unwrap();
    assert_eq!(provider_value(&restored, &consumer), "second");
    assert_eq!(restored, first);
    let (activations, batches, nodes) = tracker.take();
    let identities = activations
        .iter()
        .map(|(identity, _)| identity.as_str())
        .collect::<Vec<_>>();
    assert!(
        identities
            .iter()
            .any(|identity| identity.contains("@@//:request")),
        "{identities:#?}"
    );
    let local = |label: &str| {
        batches
            .iter()
            .find(|(identity, _)| identity.contains(label))
            .map(|(_, batch)| event_texts(batch))
    };
    assert_eq!(local("@@//:request"), Some(vec!["REQUEST_LOCAL"]));
    assert_eq!(local("@@//:second_impl"), Some(vec!["SECOND_LOCAL"]));
    let request = nodes
        .iter()
        .find(|(identity, _, _)| identity.contains("@@//:request"))
        .unwrap();
    let selected = nodes
        .iter()
        .find(|(identity, _, _)| identity.contains("@@//:second_impl"))
        .unwrap();
    assert!(request.2.contains(&selected.1));
    assert!(
        identities
            .iter()
            .any(|identity| identity.contains("@@//:first_impl")),
        "{identities:#?}"
    );
    assert!(
        identities
            .iter()
            .any(|identity| identity.contains("@@//:second_impl")),
        "{identities:#?}"
    );
    assert!(identities.contains(&"anchor"), "{identities:#?}");
    assert!(
        identities
            .iter()
            .any(|identity| identity.starts_with("package/")),
        "{identities:#?}"
    );

    fs::write(
        &build,
        TOOLCHAIN_BUILD.replacen("marker = \"second\"", "marker = \"edited\"", 1),
    )
    .unwrap();
    let edited = root_target_request(
        &dice,
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&edited, &consumer), "edited");
    fs::write(&build, TOOLCHAIN_BUILD).unwrap();
    let marker_restored = root_target_request(
        &dice,
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(marker_restored, first);

    fs::remove_file(&build).unwrap();
    assert!(
        root_target_request(
            &dice,
            &workspace,
            "@@//:request",
            Arc::new(RootActivationTracker::default())
        )
        .await
        .is_err()
    );
    fs::write(&build, TOOLCHAIN_BUILD).unwrap();
    let recreated = root_target_request(
        &dice,
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(recreated, first);
}

#[tokio::test]
async fn root_toolchain_topology_retains_intrinsic_candidates_selection_and_constraint_chain() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), TOPOLOGY_MODULE).unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(workspace.join("BUILD.bazel"), TOPOLOGY_BUILD).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = || Arc::new(RootActivationTracker::default());

    let direct_impl = root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
        .await
        .unwrap();
    assert_eq!(
        candidate_labels(&direct_impl),
        vec!["@@//:first_platform", "@@//:second_platform"]
    );
    assert!(
        direct_impl
            .toolchain_topology()
            .unwrap()
            .selection()
            .is_none()
    );
    assert_eq!(
        direct_impl
            .edges()
            .iter()
            .map(|edge| edge.kind())
            .collect::<Vec<_>>(),
        vec![
            &ConfiguredEdgeKind::CandidateExecutionPlatform { index: 0 },
            &ConfiguredEdgeKind::CandidateExecutionPlatform { index: 1 },
        ]
    );
    assert!(direct_impl.edges().iter().all(|edge| {
        edge.configured_target().is_some_and(|target| {
            target.configuration().kind() == slug_analysis_v2::ConfigurationKind::Exec
        })
    }));

    let first = root_target_request(&dice, &workspace, "@@//:request", tracker())
        .await
        .unwrap();
    let first_selection = first.toolchain_topology().unwrap().selection().unwrap();
    assert_eq!(
        first_selection.declaration().to_string(),
        "@@//:first_toolchain"
    );
    assert_eq!(
        first_selection.implementation().label().to_string(),
        "@@//:first_impl"
    );
    assert_eq!(
        first_selection.execution_platform().label().to_string(),
        "@@//:first_platform"
    );
    assert_eq!(
        root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
            .await
            .unwrap(),
        direct_impl
    );

    fs::write(
        workspace.join("MODULE.bazel"),
        TOPOLOGY_MODULE.replace(
            "\"//:first_platform\", \"//:second_platform\"",
            "\"//:second_platform\", \"//:first_platform\"",
        ),
    )
    .unwrap();
    let second = root_target_request(&dice, &workspace, "@@//:request", tracker())
        .await
        .unwrap();
    let second_selection = second.toolchain_topology().unwrap().selection().unwrap();
    assert_eq!(
        second_selection.declaration().to_string(),
        "@@//:second_toolchain"
    );
    assert_eq!(
        second_selection.execution_platform().label().to_string(),
        "@@//:second_platform"
    );
    fs::write(workspace.join("MODULE.bazel"), TOPOLOGY_MODULE).unwrap();
    assert_eq!(
        root_target_request(&dice, &workspace, "@@//:request", tracker())
            .await
            .unwrap(),
        first
    );

    let target_configuration = test_configuration();
    let exec_configuration = ConfigurationKey::from_slug(
        target_configuration
            .slug_configuration()
            .unwrap()
            .to_exec()
            .unwrap(),
    );
    let platform = root_target_request_with_configuration(
        &dice,
        &workspace,
        "@@//:first_platform",
        exec_configuration.clone(),
        tracker(),
    )
    .await
    .unwrap();
    assert_eq!(platform.kind(), &ConfiguredNodeKind::Platform);
    assert_eq!(platform.edges().len(), 1);
    assert_eq!(
        platform.edges()[0].kind(),
        &ConfiguredEdgeKind::PlatformConstraint { index: 0 }
    );
    let value = root_target_request_with_configuration(
        &dice,
        &workspace,
        "@@//:first",
        exec_configuration.clone(),
        tracker(),
    )
    .await
    .unwrap();
    assert_eq!(value.kind(), &ConfiguredNodeKind::ConstraintValue);
    assert_eq!(
        value.edges()[0].kind(),
        &ConfiguredEdgeKind::ConstraintSetting
    );
    let setting = root_target_request_with_configuration(
        &dice,
        &workspace,
        "@@//:selection",
        exec_configuration,
        tracker(),
    )
    .await
    .unwrap();
    assert_eq!(setting.kind(), &ConfiguredNodeKind::ConstraintSetting);
    assert!(setting.edges().is_empty());

    assert!(
        root_target_request(&dice, &workspace, "@@//:first_platform", tracker())
            .await
            .unwrap_err()
            .contains("incompatible with target configuration")
    );
    assert!(
        root_target_request(&dice, &workspace, "@@//:first_toolchain", tracker())
            .await
            .unwrap_err()
            .contains("toolchain declaration nodes are not supported")
    );
    let orphan_tracker = Arc::new(RootActivationTracker::with_loading());
    let orphan = root_target_request(&dice, &workspace, "@@//:orphan", orphan_tracker.clone())
        .await
        .unwrap();
    assert!(orphan.toolchain_topology().is_none());
    assert!(orphan.edges().is_empty());
    assert!(
        orphan_tracker
            .take()
            .0
            .iter()
            .all(|(identity, _)| identity != "anchor")
    );

    fs::write(
        workspace.join("MODULE.bazel"),
        TOPOLOGY_MODULE.replace(
            "register_execution_platforms(",
            "register_execution_platforms(\"@external//:platform\", ",
        ),
    )
    .unwrap();
    assert!(
        root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
            .await
            .unwrap_err()
            .contains("external toolchain topology registration")
    );
}

#[tokio::test]
async fn root_toolchain_resolution_rejects_every_native_reference_and_selection_mismatch() {
    let cases = vec![
        (
            "required kind",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.replace("[\"//:type\"]", "[\"//:setting\"]"),
            TOOLCHAIN_BUILD.to_owned(),
            "required toolchain type",
        ),
        (
            "platform kind",
            TOOLCHAIN_MODULE.replace("//:platform", "//:type"),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.to_owned(),
            "not platform",
        ),
        (
            "toolchain kind",
            TOOLCHAIN_MODULE.replace("//:second", "//:type"),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.to_owned(),
            "not toolchain",
        ),
        (
            "setting reference",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replacen(
                "constraint_setting = \":setting\"",
                "constraint_setting = \":type\"",
                1,
            ),
            "non-constraint setting",
        ),
        (
            "type reference",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replacen(
                "toolchain_type = \":type\"",
                "toolchain_type = \":setting\"",
                1,
            ),
            "non-toolchain type",
        ),
        (
            "implementation reference",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replacen("toolchain = \":first_impl\"", "toolchain = \":type\"", 1),
            "not a Starlark rule",
        ),
        (
            "exec reference",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replacen(
                "exec_compatible_with = [\":linux\"]",
                "exec_compatible_with = [\":type\"]",
                1,
            ),
            "expected constraint_value",
        ),
        (
            "platform duplicate",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replace(
                "constraint_values = [\":linux\"]",
                "constraint_values = [\":linux\", \":other\"]",
            ),
            "duplicate constraint setting",
        ),
        (
            "exec duplicate",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replacen(
                "exec_compatible_with = [\":linux\"]",
                "exec_compatible_with = [\":linux\", \":other\"]",
                1,
            ),
            "duplicate execution constraint setting",
        ),
        (
            "no pair",
            TOOLCHAIN_MODULE.to_owned(),
            TOOLCHAIN_DEFS.to_owned(),
            TOOLCHAIN_BUILD.replace(
                "exec_compatible_with = [\":linux\"]",
                "exec_compatible_with = [\":other\"]",
            ),
            "no compatible toolchain",
        ),
    ];
    for (name, module, defs, build, expected) in cases {
        let error = toolchain_case(&module, &defs, &build).await.unwrap_err();
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[tokio::test]
async fn root_toolchain_resolution_rejects_leaf_provider_callable_and_context_escapes() {
    let first_module =
        TOOLCHAIN_MODULE.replace("\"//:second\", \"//:first\"", "\"//:first\", \"//:second\"");
    let dependency_defs = TOOLCHAIN_DEFS.replace(
        "attrs = {\"marker\": attr.string(mandatory = True)})",
        "attrs = {\"marker\": attr.string(mandatory = True), \"dep\": attr.label()})",
    );
    let transition_defs = format!(
        "def _cfg(settings, attr): return {{\"//:setting\": \"value\"}}\nleaf_cfg = transition(implementation = _cfg, inputs = [], outputs = [\"//:setting\"])\n{}",
        TOOLCHAIN_DEFS.replacen(
            "{\"marker\": attr.string(mandatory = True)}",
            "{\"marker\": attr.string(mandatory = True), \"dep\": attr.label(cfg = leaf_cfg)}",
            1,
        )
    );
    let cases = vec![
        ("dependency leaf", first_module.clone(), dependency_defs, TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", dep = \":type\")", 1), "marker leaf"),
        ("required leaf", first_module.clone(), TOOLCHAIN_DEFS.replacen("first_impl = rule(implementation = _first, attrs = {\"marker\": attr.string(mandatory = True)})", "first_impl = rule(implementation = _first, attrs = {\"marker\": attr.string(mandatory = True)}, toolchains = [\":type\"])", 1), TOOLCHAIN_BUILD.to_owned(), "marker leaf"),
        ("transition leaf", first_module.clone(), transition_defs, TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", dep = \":type\")", 1), "marker leaf"),
        ("build-setting leaf", first_module.clone(), TOOLCHAIN_DEFS.replacen("attrs = {\"marker\": attr.string(mandatory = True)})", "attrs = {\"marker\": attr.string(mandatory = True)}, build_setting = config.string(flag = True))", 1), TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", build_setting_default = \"bad\")", 1), "marker leaf"),
        ("explicit tags", first_module.clone(), TOOLCHAIN_DEFS.to_owned(), TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", tags = [])", 1), "marker leaf"),
        ("nonempty builtin", first_module.clone(), TOOLCHAIN_DEFS.to_owned(), TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", features = [\"bad\"])", 1), "marker leaf"),
        ("omitted optional marker", first_module.clone(), TOOLCHAIN_DEFS.replacen("attr.string(mandatory = True)", "attr.string()", 1), TOOLCHAIN_BUILD.replacen("first_impl(name = \"first_impl\", marker = \"first\")", "first_impl(name = \"first_impl\")", 1), "marker leaf"),
        ("extra scalar", first_module.clone(), TOOLCHAIN_DEFS.replacen("{\"marker\": attr.string(mandatory = True)}", "{\"marker\": attr.string(mandatory = True), \"extra\": attr.string()}", 1), TOOLCHAIN_BUILD.replacen("marker = \"first\")", "marker = \"first\", extra = \"bad\")", 1), "marker leaf"),
        ("executable capability", first_module.clone(), TOOLCHAIN_DEFS.replacen("first_impl = rule(implementation = _first, attrs = {\"marker\": attr.string(mandatory = True)})", "first_impl = rule(implementation = _first, attrs = {\"marker\": attr.string(mandatory = True)}, executable = True)", 1), TOOLCHAIN_BUILD.to_owned(), "marker leaf"),
        ("test capability", first_module.clone(), TOOLCHAIN_DEFS.replace("first_impl", "first_impl_test").replacen("attrs = {\"marker\": attr.string(mandatory = True)})", "attrs = {\"marker\": attr.string(mandatory = True)}, test = True)", 1), TOOLCHAIN_BUILD.replace("first_impl", "first_impl_test"), "marker leaf"),
        ("missing callable marker", first_module.clone(), TOOLCHAIN_DEFS.replace("platform_common.ToolchainInfo(marker = ctx.attr.marker)", "platform_common.ToolchainInfo()"), TOOLCHAIN_BUILD.to_owned(), "exactly one named"),
        ("positional callable marker", first_module.clone(), TOOLCHAIN_DEFS.replace("platform_common.ToolchainInfo(marker = ctx.attr.marker)", "platform_common.ToolchainInfo(ctx.attr.marker)"), TOOLCHAIN_BUILD.to_owned(), "positional"),
        ("typed callable marker", first_module.clone(), TOOLCHAIN_DEFS.replace("platform_common.ToolchainInfo(marker = ctx.attr.marker)", "platform_common.ToolchainInfo(marker = 1)"), TOOLCHAIN_BUILD.to_owned(), "must be a string"),
        ("wrong callable name", first_module.clone(), TOOLCHAIN_DEFS.replace("platform_common.ToolchainInfo(marker = ctx.attr.marker)", "platform_common.ToolchainInfo(value = ctx.attr.marker)"), TOOLCHAIN_BUILD.to_owned(), "named argument `marker`"),
        ("extra callable name", first_module.clone(), TOOLCHAIN_DEFS.replace("platform_common.ToolchainInfo(marker = ctx.attr.marker)", "platform_common.ToolchainInfo(marker = ctx.attr.marker, extra = \"bad\")"), TOOLCHAIN_BUILD.to_owned(), "exactly one named"),
        ("context index", TOOLCHAIN_MODULE.to_owned(), TOOLCHAIN_DEFS.replace("ctx.toolchains[\"//:type\"]", "ctx.toolchains[\"//:missing\"]"), TOOLCHAIN_BUILD.to_owned(), "only contains //:type"),
        ("action postguard", first_module.clone(), TOOLCHAIN_DEFS.replacen("    print(\"FIRST_LOCAL\")", "    out = ctx.actions.declare_file(\"bad\")\n    ctx.actions.write(out, \"bad\")", 1), TOOLCHAIN_BUILD.to_owned(), "must return only"),
        ("nonempty DefaultInfo", first_module.clone(), TOOLCHAIN_DEFS.replacen("    print(\"FIRST_LOCAL\")\n    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]", "    out = ctx.actions.declare_file(\"bad\")\n    return [DefaultInfo(files = depset([out])), platform_common.ToolchainInfo(marker = ctx.attr.marker)]", 1), TOOLCHAIN_BUILD.to_owned(), "must return only"),
        ("user ToolchainInfo", first_module.clone(), format!("ToolchainInfo = provider(fields = {{}})\n{}", TOOLCHAIN_DEFS.replacen("return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]", "return [platform_common.ToolchainInfo(marker = ctx.attr.marker), ToolchainInfo()]", 1)), TOOLCHAIN_BUILD.to_owned(), "must return only"),
        ("user DefaultInfo", first_module.clone(), format!("DefaultInfo = provider(fields = {{}})\n{}", TOOLCHAIN_DEFS.replacen("return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]", "return [platform_common.ToolchainInfo(marker = ctx.attr.marker), DefaultInfo()]", 1)), TOOLCHAIN_BUILD.to_owned(), "must return only"),
        ("provider cardinality", first_module, format!("Extra = provider(fields = {{\"value\": \"\"}})\n{}", TOOLCHAIN_DEFS.replacen("return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]", "return [platform_common.ToolchainInfo(marker = ctx.attr.marker), Extra(value = \"bad\")]", 1)), TOOLCHAIN_BUILD.to_owned(), "must return only"),
    ];
    for (name, module, defs, build, expected) in cases {
        let error = toolchain_case(&module, &defs, &build).await.unwrap_err();
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[tokio::test]
async fn zero_toolchain_requirement_bypasses_registration_resolution() {
    let workspace = scratch();
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"root\")\nregister_toolchains(\"@external//:invalid\")\n",
    )
    .unwrap();
    fs::write(workspace.join("defs.bzl"), "ConsumerInfo = provider(fields = {\"value\": \"\"})\ndef _request(ctx): return [ConsumerInfo(value = \"zero\")]\nrequest = rule(implementation = _request)\n").unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"request\")\nrequest(name = \"request\")\n",
    )
    .unwrap();
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let result = root_target_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:request",
        tracker.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        provider_value(
            &result,
            &ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap()
        ),
        "zero"
    );
    assert!(result.configured_dependencies().next().is_none());
    let (activations, _, nodes) = tracker.take();
    assert!(
        activations
            .iter()
            .any(|(identity, _)| identity.starts_with("package/"))
    );
    let request = nodes
        .iter()
        .find(|(identity, _, _)| identity.contains("@@//:request"))
        .unwrap();
    let anchor = nodes
        .iter()
        .find(|(identity, _, _)| identity == "anchor")
        .unwrap();
    assert!(!request.2.contains(&anchor.1));
}

#[tokio::test]
async fn root_toolchain_resolution_loads_reachable_cross_package_references() {
    let workspace = scratch();
    for package in ["platforms", "tools", "constraints", "impl", "rules"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\nregister_execution_platforms(\"//platforms:p\")\nregister_toolchains(\"//tools:tc\")\n").unwrap();
    fs::write(workspace.join("defs.bzl"), "ConsumerInfo = provider(fields = {\"value\": \"\"})\ndef _request(ctx): return [ConsumerInfo(value = ctx.toolchains[\"//tools:type\"].marker)]\nrequest = rule(implementation = _request, toolchains = [\"//tools:type\"])\n").unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"request\")\nrequest(name = \"request\")\n",
    )
    .unwrap();
    fs::write(workspace.join("constraints/BUILD.bazel"), "constraint_setting(name = \"setting\")\nconstraint_value(name = \"linux\", constraint_setting = \":setting\")\n").unwrap();
    fs::write(
        workspace.join("platforms/BUILD.bazel"),
        "platform(name = \"p\", constraint_values = [\"//constraints:linux\"])\n",
    )
    .unwrap();
    fs::write(workspace.join("rules/defs.bzl"), "def _impl(ctx): return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]\nimpl = rule(implementation = _impl, attrs = {\"marker\": attr.string(mandatory = True)})\n").unwrap();
    fs::write(workspace.join("rules/BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("impl/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"impl\")\nimpl(name = \"chosen\", marker = \"cross\")\n",
    )
    .unwrap();
    fs::write(workspace.join("tools/BUILD.bazel"), "toolchain_type(name = \"type\")\ntoolchain(name = \"tc\", toolchain_type = \":type\", toolchain = \"//impl:chosen\", exec_compatible_with = [\"//constraints:linux\"])\n").unwrap();
    let result = root_target_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(
        provider_value(
            &result,
            &ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap()
        ),
        "cross"
    );
}

#[tokio::test]
async fn external_registration_error_yields_to_root_package_needs() {
    let missing_module = "module(name = \"root\")\nregister_execution_platforms(\"@external//:p\", \"//platforms:p\")\nregister_toolchains(\"//tools:tc\")\n";
    let required_defs = TOOLCHAIN_DEFS.replace("[\"//:type\"]", "[\"//:type\"]");
    let need = toolchain_case(missing_module, &required_defs, TOOLCHAIN_BUILD)
        .await
        .unwrap_err();
    assert!(need.contains("Needs"), "{need}");

    let external = toolchain_case(
        &TOOLCHAIN_MODULE.replacen(
            "register_execution_platforms(",
            "register_execution_platforms(\"@external//:p\", ",
            1,
        ),
        TOOLCHAIN_DEFS,
        TOOLCHAIN_BUILD,
    )
    .await
    .unwrap_err();
    assert!(
        external.contains("external toolchain registration"),
        "{external}"
    );
}

#[tokio::test]
async fn later_reference_round_need_yields_to_root_semantic_error() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), TOOLCHAIN_MODULE).unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        TOOLCHAIN_BUILD.replace(
            "constraint_values = [\":linux\"]",
            "constraint_values = [\"//missing:value\", \":type\"]",
        ),
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let request = || {
        root_target_request(
            &dice,
            &workspace,
            "@@//:request",
            Arc::new(RootActivationTracker::default()),
        )
    };
    let need = request().await.unwrap_err();
    assert!(need.contains("Needs"), "{need}");

    fs::create_dir(workspace.join("missing")).unwrap();
    fs::write(workspace.join("missing/BUILD.bazel"), "constraint_setting(name = \"setting\")\nconstraint_value(name = \"value\", constraint_setting = \":setting\")\n").unwrap();
    let semantic = request().await.unwrap_err();
    assert!(
        semantic.contains("expected constraint_value at @@//:type"),
        "{semantic}"
    );
}

#[tokio::test]
async fn root_string_setting_preparation_preserves_lifecycle_transition_and_identity() {
    let workspace = scratch();
    let defs = workspace.join("defs.bzl");
    let build = workspace.join("BUILD.bazel");
    let defs_source = r#"ConsumerInfo = provider(fields = {"value": "value"})
ParentInfo = provider(fields = {"value": "value"})
SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx): return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx): return [ConsumerInfo(value = ctx.attr._setting[SettingInfo].value)]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
def _unrelated(ctx): return []
unrelated = rule(implementation = _unrelated)
def _left_transition(settings, attr): return {"//:setting": "left"}
def _right_transition(settings, attr): return {"//:setting": "right"}
left_transition = transition(implementation = _left_transition, inputs = [], outputs = ["//:setting"])
right_transition = transition(implementation = _right_transition, inputs = [], outputs = ["//:setting"])
def _parent(ctx): return [ParentInfo(value = ctx.attr.left[0][ConsumerInfo].value + "," + ctx.attr.right[0][ConsumerInfo].value)]
parent = rule(implementation = _parent, attrs = {"left": attr.label(cfg = left_transition), "right": attr.label(cfg = right_transition)})
"#;
    let build_source = "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\", \"unrelated\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nunrelated(name = \"unrelated\")\nparent(name = \"parent\", left = \":consumer\", right = \":consumer\")\n";
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(&defs, defs_source).unwrap();
    fs::write(&build, build_source).unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootActivationTracker::default());
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    let parent = ProviderId::new("//:defs.bzl", "ParentInfo").unwrap();
    let need_before_missing = root_string_request_result_with_explicit(
        &dice,
        &workspace,
        "@@//:missing",
        Some(RootStringSettingValue::new_for_label(
            "@@//missing_settings:setting",
            "command",
        )),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap_err();
    assert!(
        need_before_missing.contains("Needs"),
        "explicit setting Need must precede missing-target error: {need_before_missing}"
    );
    let legacy_base = test_configuration();
    let legacy_setting = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:setting").unwrap(),
            legacy_base.clone(),
        ),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        root_setting_value(legacy_setting.configured_target_key().unwrap()),
        "default"
    );
    let legacy_parent = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(CanonicalLabel::parse("@@//:parent").unwrap(), legacy_base),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&legacy_parent, &parent), "left,right");
    assert_eq!(
        root_setting_value(legacy_parent.configured_target_key().unwrap()),
        "default"
    );
    let default =
        root_string_request(&dice, &workspace, "@@//:consumer", None, tracker.clone()).await;
    assert_eq!(provider_value(&default, &consumer), "default");
    let default_key = default.key().clone();
    let warm_default =
        root_string_request(&dice, &workspace, "@@//:consumer", None, tracker.clone()).await;
    assert_eq!(provider_value(&warm_default, &consumer), "default");
    assert_eq!(default_key, *warm_default.key());
    let command = root_string_request(
        &dice,
        &workspace,
        "@@//:consumer",
        Some("command"),
        tracker.clone(),
    )
    .await;
    assert_eq!(provider_value(&command, &consumer), "command");
    assert_ne!(default_key, *command.key());
    let restored_command = root_string_request(
        &dice,
        &workspace,
        "@@//:consumer",
        Some("default"),
        tracker.clone(),
    )
    .await;
    assert_eq!(provider_value(&restored_command, &consumer), "default");
    assert_eq!(default_key, *restored_command.key());
    let original_parent =
        root_string_request(&dice, &workspace, "@@//:parent", None, tracker.clone()).await;
    assert_eq!(provider_value(&original_parent, &parent), "left,right");
    let original_parent_key = original_parent.key().clone();
    let parent_deps = original_parent
        .configured_dependencies()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(parent_deps.len(), 2);
    assert_eq!(parent_deps[0].label(), parent_deps[1].label());
    assert_ne!(
        parent_deps[0].configuration(),
        parent_deps[1].configuration()
    );
    assert_eq!(
        parent_deps
            .iter()
            .map(root_setting_value)
            .collect::<Vec<_>>(),
        ["left", "right"]
    );
    fs::write(&defs, defs_source.replacen("\"left\"", "\"changed\"", 1)).unwrap();
    let edited_parent =
        root_string_request(&dice, &workspace, "@@//:parent", None, tracker.clone()).await;
    assert_eq!(provider_value(&edited_parent, &parent), "changed,right");
    let edited_deps = edited_parent
        .configured_dependencies()
        .cloned()
        .collect::<Vec<_>>();
    assert_ne!(edited_deps[0], parent_deps[0]);
    assert_eq!(edited_deps[1], parent_deps[1]);

    fs::write(&defs, defs_source).unwrap();
    let restored_parent =
        root_string_request(&dice, &workspace, "@@//:parent", None, tracker.clone()).await;
    assert_eq!(provider_value(&restored_parent, &parent), "left,right");
    assert_eq!(original_parent_key, *restored_parent.key());
    assert_eq!(
        parent_deps,
        restored_parent
            .configured_dependencies()
            .cloned()
            .collect::<Vec<_>>()
    );

    fs::write(
        &build,
        build_source.replacen("\"default\"", "\"edited-default\"", 1),
    )
    .unwrap();
    let edited_default =
        root_string_request(&dice, &workspace, "@@//:consumer", None, tracker.clone()).await;
    assert_eq!(provider_value(&edited_default, &consumer), "edited-default");

    fs::write(&build, build_source).unwrap();
    let restored_default =
        root_string_request(&dice, &workspace, "@@//:consumer", None, tracker.clone()).await;
    assert_eq!(provider_value(&restored_default, &consumer), "default");
    assert_eq!(default_key, *restored_default.key());

    let unrelated = root_string_request(
        &dice,
        &workspace,
        "@@//:unrelated",
        Some("command"),
        Arc::new(RootActivationTracker::default()),
    )
    .await;
    assert_eq!(
        root_setting_value(unrelated.configured_target_key().unwrap()),
        "command"
    );

    let (activations, _, _) = tracker.take();
    assert_eq!(
        activation_codes(&activations),
        r#"resolved/@@//:consumer=changed:E resolved/@@//:consumer=command:E resolved/@@//:consumer=default:E resolved/@@//:consumer=default:E
resolved/@@//:consumer=default:R resolved/@@//:consumer=default:R resolved/@@//:consumer=edited-default:E resolved/@@//:consumer=left:E
resolved/@@//:consumer=right:E resolved/@@//:consumer=right:E resolved/@@//:parent=default:E resolved/@@//:parent=default:E
resolved/@@//:parent=default:R resolved/@@//:setting=changed:E resolved/@@//:setting=command:E resolved/@@//:setting=default:E
resolved/@@//:setting=default:R resolved/@@//:setting=edited-default:E resolved/@@//:setting=left:E resolved/@@//:setting=right:E
resolved/@@//:setting=right:E"#
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    );

    fs::write(
        &build,
        build_source.replacen(
            "string_setting(name = \"setting\", build_setting_default = \"default\")",
            "consumer(name = \"setting\")",
            1,
        ),
    )
    .unwrap();
    assert!(
        root_string_request_result(
            &dice,
            &workspace,
            "@@//:consumer",
            Some("command"),
            Arc::new(RootActivationTracker::default()),
        )
        .await
        .is_err()
    );
    let unrelated_error = root_string_request_result(
        &dice,
        &workspace,
        "@@//:unrelated",
        Some("command"),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap_err();
    assert!(
        unrelated_error.contains("root string build setting @@//:setting is missing"),
        "{unrelated_error}"
    );
}

#[tokio::test]
async fn transitioned_edges_converge_on_resolved_child_node_but_retain_origins() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _setting(ctx): return []
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _child(ctx): return []
child = rule(implementation = _child, attrs = {"_setting": attr.label(default = "//:setting")})
def _first(settings, attr): return {"//:setting": "same"}
def _second(settings, attr): return {"//:setting": "same"}
first = transition(implementation = _first, inputs = [], outputs = ["//:setting"])
second = transition(implementation = _second, inputs = [], outputs = ["//:setting"])
def _parent(ctx): return []
parent = rule(implementation = _parent, attrs = {"left": attr.label(cfg = first), "right": attr.label(cfg = second)})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"child\", \"parent\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nchild(name = \"child\")\nparent(name = \"parent\", left = \":child\", right = \":child\")\n",
    )
    .unwrap();

    let result = root_target_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:parent",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(result.edges().len(), 2);
    assert_eq!(result.edges()[0].target(), result.edges()[1].target());
    assert_ne!(result.edges()[0].kind(), result.edges()[1].kind());
    assert!(matches!(
        result.edges()[0].kind(),
        slug_analysis_v2::ConfiguredEdgeKind::TransitionedAttribute { attribute, index: 0, .. }
        if attribute == "left"
    ));
    assert!(matches!(
        result.edges()[1].kind(),
        slug_analysis_v2::ConfiguredEdgeKind::TransitionedAttribute { attribute, index: 0, .. }
        if attribute == "right"
    ));
}

#[tokio::test]
async fn fixture_proven_delegating_nodes_retain_identity_edges_and_source_attribute() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"SeenInfo = provider(fields = {"value": "observed source label"})
def _ordinary(ctx):
    value = ctx.attr.src.label if ctx.label.name == "root" else ctx.label.name
    return [DefaultInfo(), SeenInfo(value = value)]
ordinary_rule = rule(
    implementation = _ordinary,
    attrs = {
        "normal": attr.label(),
        "aliased": attr.label(),
        "src": attr.label(allow_single_file = True),
        "generated": attr.label(allow_single_file = True),
        "out": attr.output(),
    },
)
"#,
    )
    .unwrap();
    let build = r#"load(":defs.bzl", "ordinary_rule")
ordinary_rule(name = "ordinary")
ordinary_rule(name = "producer", out = "producer.out")
alias(name = "alias_inner", actual = ":ordinary")
alias(name = "alias_outer", actual = ":alias_inner")
package_group(name = "vis_leaf", packages = ["//..."])
package_group(name = "vis_top", includes = [":vis_leaf"])
ordinary_rule(
    name = "root",
    normal = ":ordinary",
    aliased = ":alias_outer",
    src = "source.txt",
    generated = ":producer.out",
    visibility = [":vis_top"],
)
"#;
    fs::write(workspace.join("BUILD.bazel"), build).unwrap();
    fs::write(workspace.join("source.txt"), "source\n").unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = test_configuration();
    let configured = |label: &str| {
        ConfiguredTargetKey::new(CanonicalLabel::parse(label).unwrap(), configuration.clone())
    };
    let root = analyze_request(&dice, &workspace, &configured("@@//:root"), None, false)
        .await
        .unwrap();

    assert_eq!(root.kind(), &ConfiguredNodeKind::Rule);
    let seen = ProviderId::new("//:defs.bzl", "SeenInfo").unwrap();
    assert_eq!(
        root.providers().user(&seen).unwrap().field("value"),
        Some("@@//:source.txt")
    );
    assert_eq!(root.edges().len(), 5);
    assert!(matches!(
        root.edges()[0].kind(),
        ConfiguredEdgeKind::OrdinaryAttribute { attribute, index: 0 }
        if attribute == "normal"
    ));
    assert_eq!(
        root.edges()[0].target(),
        &configured("@@//:ordinary").into()
    );
    assert!(matches!(
        root.edges()[1].kind(),
        ConfiguredEdgeKind::OrdinaryAttribute { attribute, index: 0 }
        if attribute == "aliased"
    ));
    assert_eq!(
        root.edges()[1].target(),
        &configured("@@//:alias_outer").into()
    );
    assert_eq!(root.edges()[2].kind(), &ConfiguredEdgeKind::Source);
    assert_eq!(
        root.edges()[2].target(),
        &ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap())
    );
    assert!(matches!(
        root.edges()[3].kind(),
        ConfiguredEdgeKind::OrdinaryAttribute { attribute, index: 0 }
        if attribute == "generated"
    ));
    assert_eq!(
        root.edges()[3].target(),
        &configured("@@//:producer.out").into()
    );
    assert_eq!(
        root.edges()[4].kind(),
        &ConfiguredEdgeKind::DeclaringVisibility
    );
    assert_eq!(
        root.edges()[4].target(),
        &ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:vis_top").unwrap())
    );

    let outer = analyze_request(
        &dice,
        &workspace,
        &configured("@@//:alias_outer"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(outer.kind(), &ConfiguredNodeKind::Alias);
    assert!(outer.providers().default_info().is_some());
    assert_eq!(provider_value(&outer, &seen), "ordinary");
    assert_eq!(outer.edges().len(), 1);
    assert_eq!(outer.edges()[0].kind(), &ConfiguredEdgeKind::AliasActual);
    assert_eq!(
        outer.edges()[0].target(),
        &configured("@@//:alias_inner").into()
    );

    let inner = analyze_request(
        &dice,
        &workspace,
        &configured("@@//:alias_inner"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(inner.kind(), &ConfiguredNodeKind::Alias);
    assert_eq!(provider_value(&inner, &seen), "ordinary");
    assert_eq!(
        inner.edges()[0].target(),
        &configured("@@//:ordinary").into()
    );

    fs::write(
        workspace.join("BUILD.bazel"),
        build.replace(
            "alias(name = \"alias_outer\", actual = \":alias_inner\")",
            "alias(name = \"alias_outer\", actual = \":producer\")",
        ),
    )
    .unwrap();
    let edited_outer = analyze_request(
        &dice,
        &workspace,
        &configured("@@//:alias_outer"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&edited_outer, &seen), "producer");
    assert_eq!(
        edited_outer.edges()[0].target(),
        &configured("@@//:producer").into()
    );
    fs::write(workspace.join("BUILD.bazel"), build).unwrap();
    let restored_outer = analyze_request(
        &dice,
        &workspace,
        &configured("@@//:alias_outer"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(restored_outer, outer);

    let generated = analyze_request(
        &dice,
        &workspace,
        &configured("@@//:producer.out"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(generated.kind(), &ConfiguredNodeKind::GeneratedFile);
    assert_eq!(generated.edges().len(), 1);
    assert_eq!(
        generated.edges()[0].kind(),
        &ConfiguredEdgeKind::GeneratedBy
    );
    assert_eq!(
        generated.edges()[0].target(),
        &configured("@@//:producer").into()
    );

    let source = analyze_node_request_typed(
        &dice,
        &workspace,
        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap()),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(source.kind(), &ConfiguredNodeKind::SourceFile);
    assert!(source.providers().default_info().is_none());
    assert!(source.edges().is_empty());

    let source_path = workspace.join("source.txt");
    fs::remove_file(&source_path).unwrap();
    let missing_source = analyze_node_request_typed_with_epoch(
        &dice,
        &workspace,
        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap()),
        None,
        false,
        root_epoch_with_missing(&workspace, [source_path.clone()]),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        missing_source.kind(),
        AnalysisErrorKind::TargetNotFound { label, .. }
        if label.to_string() == "@@//:source.txt"
    ));
    fs::write(&source_path, "source\n").unwrap();
    let restored_source = analyze_node_request_typed(
        &dice,
        &workspace,
        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap()),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(restored_source, source);

    let undeclared_source = analyze_node_request_typed(
        &dice,
        &workspace,
        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:not_declared.txt").unwrap()),
        None,
        false,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        undeclared_source.kind(),
        AnalysisErrorKind::TargetNotFound { label, .. }
        if label.to_string() == "@@//:not_declared.txt"
    ));

    let vis_top = analyze_node_request_typed(
        &dice,
        &workspace,
        ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:vis_top").unwrap()),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(vis_top.kind(), &ConfiguredNodeKind::PackageGroup);
    assert_eq!(vis_top.edges().len(), 1);
    assert_eq!(
        vis_top.edges()[0].kind(),
        &ConfiguredEdgeKind::PackageGroupInclude { index: 0 }
    );
    assert!(vis_top.edges()[0].implicit());
    assert!(!vis_top.edges()[0].tool());
    assert_eq!(
        vis_top.edges()[0].target(),
        &ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:vis_leaf").unwrap())
    );
}

#[tokio::test]
async fn transition_output_label_selects_the_carried_string_setting() {
    let workspace = scratch();
    fs::create_dir(workspace.join("settings")).unwrap();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"SettingInfo = provider(fields = {"value": "value"})
ParentInfo = provider(fields = {"value": "value"})
def _setting(ctx): return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx): return [SettingInfo(value = ctx.attr._setting[SettingInfo].value)]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//settings")})
def _transition(settings, attr): return {"//settings": "transitioned"}
setting_transition = transition(implementation = _transition, inputs = [], outputs = ["//settings"])
def _parent(ctx): return [ParentInfo(value = ctx.attr.dep[0][SettingInfo].value)]
parent = rule(implementation = _parent, attrs = {"dep": attr.label(cfg = setting_transition)})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"consumer\", \"parent\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", dep = \":consumer\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("settings/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"string_setting\")\nstring_setting(name = \"settings\", build_setting_default = \"default\")\n",
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mismatched = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:parent").unwrap(),
            test_configuration().with_root_string_setting(RootStringSettingValue::new("unrelated")),
        ),
        None,
        false,
    )
    .await
    .unwrap_err();
    assert!(
        mismatched.contains(
            "multiple string build settings are not supported: @@//:setting and @@//settings:settings"
        ),
        "{mismatched}"
    );
    let result = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:parent").unwrap(),
            test_configuration(),
        ),
        None,
        false,
    )
    .await
    .unwrap();
    let parent = ProviderId::new("//:defs.bzl", "ParentInfo").unwrap();
    assert_eq!(provider_value(&result, &parent), "transitioned");
    assert_eq!(
        result
            .configured_target_key()
            .unwrap()
            .configuration()
            .root_string_setting()
            .unwrap()
            .label(),
        "@@//settings:settings"
    );
}

async fn analyze_revision(
    dice: &Arc<Dice>,
    tracker: &Arc<AnalysisTracker>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
) -> (
    Result<ConfiguredNodeResult, String>,
    Vec<AnalysisActivation>,
) {
    let result = analyze_request(dice, workspace, key, Some(tracker.clone()), false).await;
    (result, tracker.take())
}

async fn analyze_request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> Result<ConfiguredNodeResult, String> {
    analyze_request_typed(dice, workspace, key, tracker, capture_events)
        .await
        .map_err(|error| error.to_string())
}

async fn analyze_request_typed(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> Result<ConfiguredNodeResult, AnalysisError> {
    analyze_node_request_typed(
        dice,
        workspace,
        ConfiguredNodeKey::configured(key.clone()),
        tracker,
        capture_events,
    )
    .await
}

async fn analyze_node_request_typed(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    node: ConfiguredNodeKey,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> Result<ConfiguredNodeResult, AnalysisError> {
    analyze_node_request_typed_with_epoch(
        dice,
        workspace,
        node,
        tracker,
        capture_events,
        root_epoch(workspace),
    )
    .await
}

async fn analyze_node_request_typed_with_epoch(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    node: ConfiguredNodeKey,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
    epoch: PathObservationEpoch,
) -> Result<ConfiguredNodeResult, AnalysisError> {
    let text = Arc::new(workspace_snapshot(workspace));
    let raw = raw_snapshot_from_text(&text);
    let mut user_data = UserComputationData {
        activation_tracker: tracker,
        ..Default::default()
    };
    if capture_events {
        user_data.data.set(CaptureEvaluationEvents);
    }
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            text,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            Arc::new(directory_snapshot(workspace)),
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            root.clone(),
            [root],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let workspace_identity = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
    let analysis_key = match &node {
        ConfiguredNodeKey::Configured(key) => match prepare_configured_node_analysis(
            &mut transaction,
            workspace_identity,
            key.label().clone(),
            key.configuration().clone(),
            None,
        )
        .await
        {
            AnalysisPreparationOutcome::Need(_) => {
                return Err(AnalysisError::message(
                    "configured target analysis retained Needs during preparation",
                ));
            }
            AnalysisPreparationOutcome::Complete(Ok(key)) => key,
            AnalysisPreparationOutcome::Complete(Err(error)) => return Err(error),
        },
        ConfiguredNodeKey::Null(_) => ConfiguredNodeAnalysisKey::new(workspace_identity, node)?,
    };
    let outcome = transaction
        .compute(&analysis_key)
        .await
        .expect("configured target analysis DICE compute succeeds");
    let AnalysisPreparationOutcome::Complete(result) = outcome else {
        panic!("configured target analysis retained Needs: {outcome:?}");
    };
    result
        .as_ref()
        .as_ref()
        .map(|value| value.as_ref().clone())
        .map_err(Clone::clone)
}

fn assert_analysis_events(events: &[AnalysisActivation], expected: &[(&str, EventKind)]) {
    let mut actual = events
        .iter()
        .map(|event| (event.label.clone(), event.kind))
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected
        .iter()
        .map(|(label, kind)| ((*label).to_owned(), *kind))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);

    let parent = events
        .iter()
        .find(|event| event.label == "@@//parent:parent")
        .expect("parent analysis activation is present");
    for child in events
        .iter()
        .filter(|event| event.label.starts_with("@@//leaf:"))
    {
        assert!(
            parent.dependencies.contains(&child.node),
            "parent activation did not retain child {} dependency: {events:#?}",
            child.label
        );
    }
}

#[test]
fn frozen_loaded_rule_evaluates_into_default_info_and_write_action() {
    let workspace = scratch();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.write(out, \"hello from an action\\n\")\n    return [DefaultInfo(files = depset([out]))]\n\nwrite_file = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"write_file\")\nwrite_file(name = \"write_file\")\n",
    )
    .unwrap();

    let files = [
        workspace.join("MODULE.bazel"),
        package.join("BUILD.bazel"),
        package.join("BUILD"),
        package.join("defs.bzl"),
    ]
    .into_iter()
    .map(|path| {
        let value = match fs::read_to_string(&path) {
            Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                WorkspaceFileValue::Absent
            }
            Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
        };
        (path, value)
    })
    .collect();
    let text = Arc::new(WorkspaceSnapshot {
        files: Arc::new(files),
    });
    let raw = raw_snapshot_from_text(&text);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:write_file").unwrap(),
        test_configuration(),
    );
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(
                    (WorkspaceSnapshotKey {
                        workspace: workspace.clone(),
                    }),
                    text,
                )])
                .unwrap();
            updater
                .changed_to(vec![(
                    WorkspaceRawSnapshotKey {
                        workspace: workspace.clone(),
                    },
                    raw,
                )])
                .unwrap();
            updater
                .changed_to(vec![(
                    (WorkspaceDirectorySnapshotKey {
                        workspace: workspace.clone(),
                    }),
                    Arc::new(directory_snapshot(&workspace)),
                )])
                .unwrap();
            updater
                .changed_to(vec![(PathObservationEpochKey, root_epoch(&workspace))])
                .unwrap();
            let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
            inject_root_package_policy_inputs(
                &mut updater,
                RootPackagePolicyInputs::new(
                    root.clone(),
                    [root],
                    std::iter::empty::<&str>(),
                    None,
                    Some("warning"),
                )
                .unwrap(),
            )
            .unwrap();
            inject_root_module_request_inputs(
                &mut updater,
                &workspace,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .unwrap();
            let mut transaction = updater.commit().await;
            let outcome = transaction
                .compute(
                    &ConfiguredNodeAnalysisKey::new(
                        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
                        key,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            let AnalysisPreparationOutcome::Complete(value) = outcome else {
                panic!("configured target analysis retained Needs");
            };
            value.as_ref().as_ref().unwrap().clone()
        });

    assert_eq!(result.declared_outputs(), &["pkg/write_file.txt"]);
    assert_eq!(
        result.providers().default_info().unwrap().files.to_list(),
        ["pkg/write_file.txt"]
    );
    assert_eq!(result.actions().len(), 1);
    assert_eq!(
        result.actions()[0].outputs()[0].path(),
        "pkg/write_file.txt"
    );
    assert_eq!(
        result.actions()[0].kind(),
        &ActionKind::Write {
            content: "hello from an action\n".to_owned(),
            is_executable: false,
        }
    );
}

#[tokio::test]
async fn custom_only_starlark_rule_gets_implicit_empty_default_info() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "CustomInfo = provider(fields = {\"value\": \"custom value\"})\n\ndef _impl(ctx):\n    return [CustomInfo(value = \"custom\")]\n\ncustom_rule = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"custom_rule\")\ncustom_rule(name = \"custom\")\n",
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:custom").unwrap(),
        test_configuration(),
    );
    let result = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();

    let custom_id = ProviderId::new("//:defs.bzl", "CustomInfo").unwrap();
    assert_eq!(
        result.providers().user(&custom_id).unwrap().field("value"),
        Some("custom")
    );
    assert_eq!(
        result.providers().default_info(),
        Some(&slug_build_api_v2::DefaultInfo::empty())
    );
    assert!(result.declared_outputs().is_empty());
    assert!(result.actions().is_empty());
}

#[tokio::test]
async fn default_info_executable_is_narrow_and_requires_an_executable_for_executable_rules() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _implicit(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "tool\n")
    return [DefaultInfo(executable = out)]

def _explicit(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    extra = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "tool\n")
    ctx.actions.write(extra, "extra\n")
    return [DefaultInfo(files = depset([extra]), executable = out)]

def _omitted(ctx):
    return [DefaultInfo()]

def _none(ctx):
    return [DefaultInfo(files = None, executable = None)]

def _wrong_files(ctx):
    out = ctx.actions.declare_file("wrong-files")
    return [DefaultInfo(files = out)]

def _wrong_executable(ctx):
    return [DefaultInfo(executable = "not-a-file")]

def _missing(ctx):
    return [DefaultInfo()]

implicit = rule(implementation = _implicit, executable = True)
explicit = rule(implementation = _explicit, executable = True)
omitted = rule(implementation = _omitted)
none = rule(implementation = _none)
wrong_files = rule(implementation = _wrong_files)
wrong_executable = rule(implementation = _wrong_executable)
missing = rule(implementation = _missing, executable = True)
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"explicit\", \"implicit\", \"missing\", \"none\", \"omitted\", \"wrong_executable\", \"wrong_files\")\nimplicit(name = \"implicit\")\nexplicit(name = \"explicit\")\nomitted(name = \"omitted\")\nnone(name = \"none\")\nwrong_files(name = \"wrong_files\")\nwrong_executable(name = \"wrong_executable\")\nmissing(name = \"missing\")\n",
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = |name: &str| {
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
            test_configuration(),
        )
    };

    let implicit = analyze_request(&dice, &workspace, &key("implicit"), None, false)
        .await
        .unwrap();
    let implicit = implicit.providers().default_info().unwrap();
    assert_eq!(implicit.files.to_list(), ["implicit"]);
    assert_eq!(implicit.executable.as_deref(), Some("implicit"));
    assert_eq!(
        implicit.files_to_run.executable.as_deref(),
        Some("implicit")
    );
    assert_eq!(implicit.default_runfiles.files.to_list(), ["implicit"]);
    assert_eq!(implicit.data_runfiles.files.to_list(), ["implicit"]);

    let explicit = analyze_request(&dice, &workspace, &key("explicit"), None, false)
        .await
        .unwrap();
    let explicit = explicit.providers().default_info().unwrap();
    assert_eq!(explicit.files.to_list(), ["explicit.txt"]);
    assert_eq!(explicit.executable.as_deref(), Some("explicit"));
    assert_eq!(explicit.default_runfiles.files.to_list(), ["explicit"]);
    assert_eq!(explicit.data_runfiles.files.to_list(), ["explicit"]);

    let omitted = analyze_request(&dice, &workspace, &key("omitted"), None, false)
        .await
        .unwrap();
    let none = analyze_request(&dice, &workspace, &key("none"), None, false)
        .await
        .unwrap();
    assert_eq!(
        omitted.providers().default_info(),
        none.providers().default_info()
    );

    for (target, expected) in [
        (
            "wrong_files",
            "DefaultInfo.files must be the result of depset([...])",
        ),
        (
            "wrong_executable",
            "DefaultInfo.executable must be a declared file",
        ),
    ] {
        let error = analyze_request(&dice, &workspace, &key(target), None, false)
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{target}: {error}");
    }

    let error = analyze_request_typed(&dice, &workspace, &key("missing"), None, false)
        .await
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        AnalysisErrorKind::ExecutableRuleMissingExecutable { rule_class }
            if rule_class == "missing"
    ));
    assert_eq!(
        error.to_string(),
        "The rule 'missing' is executable. It needs to create an executable File and pass it as the 'executable' parameter to the DefaultInfo it returns."
    );
}

#[tokio::test]
async fn executable_default_info_recomputes_and_restores_structural_results() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let definitions = |explicit_files: bool| {
        let files = explicit_files
            .then_some(
                "    extra = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.write(extra, \"extra\\n\")\n    return [DefaultInfo(files = depset([extra]), executable = out)]",
            )
            .unwrap_or("    return [DefaultInfo(executable = out)]");
        format!(
            "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name)\n    ctx.actions.write(out, \"tool\\n\")\n{files}\n\nprobe = rule(implementation = _impl, executable = True)\n"
        )
    };
    let defs = workspace.join("defs.bzl");
    fs::write(&defs, definitions(false)).unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n",
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:probe").unwrap(),
        test_configuration(),
    );

    let initial = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();
    fs::write(&defs, definitions(true)).unwrap();
    let changed = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();
    fs::write(&defs, definitions(false)).unwrap();
    let restored = analyze_request(&dice, &workspace, &key, None, false)
        .await
        .unwrap();

    assert_eq!(
        initial.providers().default_info().unwrap().files.to_list(),
        ["probe"]
    );
    assert_eq!(
        changed.providers().default_info().unwrap().files.to_list(),
        ["probe.txt"]
    );
    assert_ne!(initial, changed);
    assert_eq!(initial, restored);
}

#[tokio::test]
async fn recursive_custom_rules_preserve_provider_identity_dependency_order_and_local_actions() {
    let workspace = scratch();
    for package in ["rules", "leaf", "parent"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(workspace.join("rules/BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("rules/defs.bzl"),
        r#"LeafInfo = provider(fields = {"value": "leaf target name"})
ParentInfo = provider(fields = {"value": "dependency leaf names in declaration order"})

def _leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ctx.label.name + "\n")
    return [DefaultInfo(files = depset([out])), LeafInfo(value = ctx.label.name)]

def _parent_impl(ctx):
    values = [dep[LeafInfo].value for dep in ctx.attr.deps]
    out = ctx.actions.declare_file("parent.txt")
    ctx.actions.write(out, ",".join(values) + "\n")
    return [DefaultInfo(files = depset([out])), ParentInfo(value = ",".join(values))]

leaf_rule = rule(implementation = _leaf_impl)
parent_rule = rule(implementation = _parent_impl, attrs = {"deps": attr.label_list()})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"leaf_rule\")\nleaf_rule(name = \"first\")\nleaf_rule(name = \"second\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("parent/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"parent_rule\")\nparent_rule(name = \"parent\", deps = [\"//leaf:second\", \"//leaf:first\"])\n",
    )
    .unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let text = Arc::new(workspace_snapshot(&workspace));
    let raw = raw_snapshot_from_text(&text);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            text,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.clone(),
            },
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceDirectorySnapshotKey {
                workspace: workspace.clone(),
            },
            Arc::new(directory_snapshot(&workspace)),
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(&workspace))])
        .unwrap();
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            root.clone(),
            [root],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let configuration = test_configuration();
    let outcome = transaction
        .compute(
            &ConfiguredNodeAnalysisKey::new(
                NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
                ConfiguredTargetKey::new(
                    CanonicalLabel::parse("@@//parent:parent").unwrap(),
                    configuration.clone(),
                ),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let AnalysisPreparationOutcome::Complete(result) = outcome else {
        panic!("configured target analysis retained Needs");
    };
    let result = result.as_ref().as_ref().unwrap();

    assert_eq!(
        result
            .configured_dependencies()
            .cloned()
            .collect::<Vec<_>>(),
        [
            ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//leaf:second").unwrap(),
                configuration.clone(),
            ),
            ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//leaf:first").unwrap(),
                configuration.clone(),
            ),
        ]
    );
    assert!(matches!(
        result.edges()[0].kind(),
        slug_analysis_v2::ConfiguredEdgeKind::OrdinaryAttribute {
            attribute,
            index: 0
        } if attribute == "deps"
    ));
    assert!(matches!(
        result.edges()[1].kind(),
        slug_analysis_v2::ConfiguredEdgeKind::OrdinaryAttribute {
            attribute,
            index: 1
        } if attribute == "deps"
    ));
    let parent_id = ProviderId::new("//rules:defs.bzl", "ParentInfo").unwrap();
    assert_eq!(
        result.providers().user(&parent_id).unwrap().field("value"),
        Some("second,first")
    );
    assert_eq!(result.declared_outputs(), ["parent/parent.txt"]);
    assert_eq!(
        result.providers().default_info().unwrap().files.to_list(),
        ["parent/parent.txt"]
    );
    assert_eq!(result.actions().len(), 1);
    assert_eq!(result.actions()[0].outputs()[0].path(), "parent/parent.txt");
    assert_eq!(
        result.actions()[0].kind(),
        &ActionKind::Write {
            content: "second,first\n".to_owned(),
            is_executable: false,
        }
    );

    let outcome = transaction
        .compute(
            &ConfiguredNodeAnalysisKey::new(
                NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
                ConfiguredTargetKey::new(
                    CanonicalLabel::parse("@@//leaf:second").unwrap(),
                    configuration,
                ),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let AnalysisPreparationOutcome::Complete(leaf) = outcome else {
        panic!("configured target analysis retained Needs");
    };
    let leaf = leaf.as_ref().as_ref().unwrap();
    let leaf_id = ProviderId::new("//rules:defs.bzl", "LeafInfo").unwrap();
    assert_eq!(
        leaf.providers().user(&leaf_id).unwrap().field("value"),
        Some("second")
    );
    assert_eq!(leaf.actions().len(), 1);
    assert_eq!(leaf.actions()[0].outputs()[0].path(), "leaf/second.txt");
}

#[tokio::test]
async fn analysis_event_capture_is_target_local_empty_replacing_and_failure_prefix_preserving() {
    let workspace = scratch();
    for package in ["rules", "leaf", "parent"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let definitions = |leaf_body: &str, parent_body: &str| {
        format!(
            r#"LeafInfo = provider(fields = {{"value": "leaf target name"}})
ParentInfo = provider(fields = {{"value": "dependency leaf names"}})

def _leaf_impl(ctx):
{leaf_body}    return [DefaultInfo(files = depset([])), LeafInfo(value = ctx.label.name)]

def _parent_impl(ctx):
{parent_body}    values = [dep[LeafInfo].value for dep in ctx.attr.deps]
    return [DefaultInfo(files = depset([])), ParentInfo(value = ",".join(values))]

leaf = rule(implementation = _leaf_impl)
parent = rule(implementation = _parent_impl, attrs = {{"deps": attr.label_list()}})
"#
        )
    };
    fs::write(
        workspace.join("rules/defs.bzl"),
        definitions(
            "    print(\"LEAF_LOCAL\")\n",
            "    print(\"PARENT_LOCAL\")\n",
        ),
    )
    .unwrap();
    fs::write(workspace.join("rules/BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"leaf\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("parent/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"parent\")\nparent(name = \"parent\", deps = [\"//leaf:leaf\"])\n",
    )
    .unwrap();
    let configuration = test_configuration();
    let leaf_key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//leaf:leaf").unwrap(),
        configuration.clone(),
    );
    let parent_key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//parent:parent").unwrap(),
        configuration,
    );

    let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let direct_tracker = Arc::new(AnalysisEventTracker::default());
    analyze_request(
        &direct_dice,
        &workspace,
        &parent_key,
        Some(direct_tracker.clone()),
        false,
    )
    .await
    .unwrap();
    let direct = direct_tracker.take();
    assert!(
        analysis_event(&direct, &workspace, &leaf_key)
            .batch
            .is_none(),
        "{direct:#?}"
    );
    assert!(
        analysis_event(&direct, &workspace, &parent_key)
            .batch
            .is_none(),
        "{direct:#?}"
    );

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(AnalysisEventTracker::default());
    analyze_request(&dice, &workspace, &parent_key, Some(tracker.clone()), true)
        .await
        .unwrap();
    let initial = tracker.take();
    assert_eq!(
        analysis_event(&initial, &workspace, &leaf_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(vec!["LEAF_LOCAL"])
    );
    assert_eq!(
        analysis_event(&initial, &workspace, &parent_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(vec!["PARENT_LOCAL"])
    );
    let leaf_batch = analysis_event(&initial, &workspace, &leaf_key)
        .batch
        .as_ref()
        .unwrap();
    let parent_batch = analysis_event(&initial, &workspace, &parent_key)
        .batch
        .as_ref()
        .unwrap();
    assert!(matches!(
        leaf_batch.events(),
        [EvaluationEvent::StarlarkPrint { location, text }]
            if text == "LEAF_LOCAL"
                && location.to_string()
                    == format!("{}:5:10", workspace.join("rules/defs.bzl").display())
    ));
    assert!(matches!(
        parent_batch.events(),
        [EvaluationEvent::StarlarkPrint { location, text }]
            if text == "PARENT_LOCAL"
                && location.to_string()
                    == format!("{}:9:10", workspace.join("rules/defs.bzl").display())
    ));

    fs::write(
        workspace.join("rules/defs.bzl"),
        definitions("    print(\"LEAF_LOCAL\")\n", ""),
    )
    .unwrap();
    analyze_request(&dice, &workspace, &parent_key, Some(tracker.clone()), true)
        .await
        .unwrap();
    let empty = tracker.take();
    assert_eq!(
        analysis_event(&empty, &workspace, &parent_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(Vec::new())
    );

    fs::write(
        workspace.join("rules/defs.bzl"),
        definitions(
            "    print(\"LEAF_LOCAL\")\n",
            "    print(\"PARENT_RUNTIME_PREFIX\")\n    fail(\"parent runtime\")\n    print(\"PARENT_RUNTIME_AFTER\")\n",
        ),
    )
    .unwrap();
    let error = analyze_request(&dice, &workspace, &parent_key, Some(tracker.clone()), true)
        .await
        .unwrap_err();
    assert!(error.contains("parent runtime"), "{error}");
    let local_failure = tracker.take();
    assert_eq!(
        analysis_event(&local_failure, &workspace, &parent_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(vec!["PARENT_RUNTIME_PREFIX"])
    );

    fs::write(
        workspace.join("rules/defs.bzl"),
        definitions(
            "    print(\"LEAF_LOCAL\")\n",
            "    print(\"PARENT_RECOVERED\")\n",
        ),
    )
    .unwrap();
    analyze_request(&dice, &workspace, &parent_key, Some(tracker.clone()), true)
        .await
        .unwrap();
    let recovered = tracker.take();
    assert_eq!(
        analysis_event(&recovered, &workspace, &parent_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(vec!["PARENT_RECOVERED"])
    );

    fs::write(
        workspace.join("rules/defs.bzl"),
        definitions(
            "    print(\"LEAF_RUNTIME_PREFIX\")\n    fail(\"leaf runtime\")\n    print(\"LEAF_RUNTIME_AFTER\")\n",
            "    print(\"PARENT_MUST_NOT_RUN\")\n",
        ),
    )
    .unwrap();
    let error = analyze_request(&dice, &workspace, &parent_key, Some(tracker.clone()), true)
        .await
        .unwrap_err();
    assert!(error.contains("leaf runtime"), "{error}");
    let failed = tracker.take();
    assert_eq!(
        analysis_event(&failed, &workspace, &leaf_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(vec!["LEAF_RUNTIME_PREFIX"])
    );
    assert_eq!(
        analysis_event(&failed, &workspace, &parent_key)
            .batch
            .as_ref()
            .map(event_texts),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn retained_dice_recomputes_recursive_analysis_only_for_semantic_revisions() {
    let workspace = scratch();
    for package in ["rules", "leaf", "parent", "unrelated"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let definitions = |prefix: &str| {
        format!(
            r#"LeafInfo = provider(fields = {{"value": "leaf value"}})
ParentInfo = provider(fields = {{"value": "ordered leaf values"}})

def _leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "{prefix}" + ctx.label.name + "\n")
    return [DefaultInfo(files = depset([out])), LeafInfo(value = "{prefix}" + ctx.label.name)]

def _parent_impl(ctx):
    values = [dep[LeafInfo].value for dep in ctx.attr.deps]
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, ",".join(values) + "\n")
    return [DefaultInfo(files = depset([out])), ParentInfo(value = ",".join(values))]

leaf = rule(implementation = _leaf_impl)
parent = rule(implementation = _parent_impl, attrs = {{"deps": attr.label_list()}})
"#
        )
    };
    fs::write(workspace.join("rules/defs.bzl"), definitions("")).unwrap();
    fs::write(workspace.join("rules/BUILD.bazel"), "").unwrap();
    let complete_leaf_build =
        "load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"first\")\nleaf(name = \"second\")\n";
    fs::write(workspace.join("leaf/BUILD.bazel"), complete_leaf_build).unwrap();
    fs::write(
        workspace.join("parent/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"parent\")\nparent(name = \"parent\", deps = [\"//leaf:second\", \"//leaf:first\"])\n",
    )
    .unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(AnalysisTracker::default());
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//parent:parent").unwrap(),
        test_configuration(),
    );

    let (initial, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    let parent_id = ProviderId::new("//rules:defs.bzl", "ParentInfo").unwrap();
    assert_eq!(
        initial
            .unwrap()
            .providers()
            .user(&parent_id)
            .unwrap()
            .field("value"),
        Some("second,first")
    );
    assert_analysis_events(
        &events,
        &[
            ("@@//leaf:first", EventKind::Evaluated),
            ("@@//leaf:second", EventKind::Evaluated),
            ("@@//parent:parent", EventKind::Evaluated),
        ],
    );

    let (identical, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    assert_eq!(
        identical
            .unwrap()
            .providers()
            .user(&parent_id)
            .unwrap()
            .field("value"),
        Some("second,first")
    );
    assert_analysis_events(&events, &[("@@//parent:parent", EventKind::Reused)]);

    fs::write(workspace.join("unrelated/file.txt"), "unrelated\n").unwrap();
    let (unrelated, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    assert_eq!(
        unrelated
            .unwrap()
            .providers()
            .user(&parent_id)
            .unwrap()
            .field("value"),
        Some("second,first")
    );
    assert_analysis_events(
        &events,
        &[
            ("@@//leaf:first", EventKind::Reused),
            ("@@//leaf:second", EventKind::Reused),
            ("@@//parent:parent", EventKind::Reused),
        ],
    );

    fs::write(workspace.join("rules/defs.bzl"), definitions("edited-")).unwrap();
    let (edited, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    assert_eq!(
        edited
            .unwrap()
            .providers()
            .user(&parent_id)
            .unwrap()
            .field("value"),
        Some("edited-second,edited-first")
    );
    assert_analysis_events(
        &events,
        &[
            ("@@//leaf:first", EventKind::Evaluated),
            ("@@//leaf:second", EventKind::Evaluated),
            ("@@//parent:parent", EventKind::Evaluated),
        ],
    );

    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"second\")\n",
    )
    .unwrap();
    let (deleted, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    let error = deleted.unwrap_err();
    assert!(
        error.contains("target `@@//leaf:first` was not found"),
        "{error}"
    );
    assert_analysis_events(&events, &[("@@//parent:parent", EventKind::Evaluated)]);

    fs::write(workspace.join("leaf/BUILD.bazel"), complete_leaf_build).unwrap();
    let (recreated, events) = analyze_revision(&dice, &tracker, &workspace, &key).await;
    assert_eq!(
        recreated
            .unwrap()
            .providers()
            .user(&parent_id)
            .unwrap()
            .field("value"),
        Some("edited-second,edited-first")
    );
    assert_analysis_events(
        &events,
        &[
            ("@@//leaf:first", EventKind::Evaluated),
            ("@@//leaf:second", EventKind::Evaluated),
            ("@@//parent:parent", EventKind::Evaluated),
        ],
    );
}
