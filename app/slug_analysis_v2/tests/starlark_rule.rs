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
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceNodeId;
use dice::DiceTransactionUpdater;
use dice::DynKey;
use dice::Key;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use num_bigint::BigInt;
use slug_analysis_v2::AnalysisError;
use slug_analysis_v2::AnalysisErrorKind;
use slug_analysis_v2::AnalysisPreparationOutcome;
use slug_analysis_v2::CommandConfigurationPreparationKey;
use slug_analysis_v2::CommandConfigurationPreparationObservationKey;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionExecutionState as ActionExecutionState;
use slug_analysis_v2::ConfiguredConditionKey;
use slug_analysis_v2::ConfiguredConditionMatch;
use slug_analysis_v2::ConfiguredEdgeKind;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeAnalysisObservationKey;
use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeKind;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredPlatform;
use slug_analysis_v2::ConfiguredPlatformKey;
use slug_analysis_v2::ConfiguredPlatformOutcome;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::ConfiguredTargetPlatformKey;
use slug_analysis_v2::analysis_cycle_detector;
use slug_analysis_v2::key::StarlarkOption;
use slug_analysis_v2::key::StarlarkOptionScope;
use slug_analysis_v2::key::StarlarkOptionValue;
use slug_analysis_v2::prepare_configured_node_analysis;
use slug_build_api_v2::ActionKind;
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
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_loading_v2::CommandRegistrationExpansionKey;
use slug_loading_v2::CommandRegistrationExpansionObservationKey;
use slug_loading_v2::HostPackageInventoryKey;
use slug_loading_v2::HostPackageInventoryObservationError;
use slug_loading_v2::HostPackageInventoryObservationKey;
use slug_loading_v2::ModuleRegistrationExpansionKey;
use slug_loading_v2::ModuleRegistrationExpansionObservationError;
use slug_loading_v2::ModuleRegistrationExpansionObservationKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
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
    let mut files = snapshot
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
        .collect::<Vec<_>>();
    if let Some(module) = snapshot
        .files
        .keys()
        .find(|path| path.file_name().is_some_and(|name| name == "MODULE.bazel"))
    {
        let lockfile = module.with_file_name("MODULE.bazel.lock");
        if !snapshot.files.contains_key(&lockfile) {
            files.push((lockfile, WorkspaceRawFileValue::Absent));
        }
    }
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(files.into_iter().collect()),
    })
}

fn local_repository_materializations(
    workspace: &NormalizedAbsolutePath,
    repositories: &[(&str, &str)],
) -> RepositoryMaterializationResultEpoch {
    let entries = repositories.iter().map(|(canonical_repo, path)| {
        let mut attributes = SmallMap::new();
        attributes.insert(
            "path".into(),
            OverrideAttributeValue::String((*path).into()),
        );
        RepositoryMaterializationEpochEntry {
            request: Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.dupe(),
                    canonical_repo: CanonicalRepoName::new(*canonical_repo).unwrap(),
                },
                repo_spec: RepoSpec {
                    rule_id: RepoRuleId {
                        bzl_file: CanonicalLabel::parse(
                            "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        )
                        .unwrap(),
                        rule_name: "local_repository".into(),
                    },
                    attributes: Arc::new(attributes),
                },
                kind: RepositoryMaterializationKind::Local {
                    logical_root: NormalizedAbsolutePath::new(workspace.as_path().join(path))
                        .unwrap(),
                },
            }),
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Local,
            ),
        }
    });
    RepositoryMaterializationResultEpoch::new(workspace.dupe(), entries).unwrap()
}

fn root_epoch(root: &std::path::Path) -> PathObservationEpoch {
    root_epoch_with_missing(root, std::iter::empty::<PathBuf>())
}

fn root_epoch_with_listings(
    root: &std::path::Path,
    directories: &[PathBuf],
) -> PathObservationEpoch {
    let base = root_epoch(root);
    let listings = directories.iter().map(|directory| {
        let entries = fs::read_dir(directory).unwrap().map(|entry| {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let kind = if file_type.is_file() {
                PathDirectoryEntryKind::File
            } else if file_type.is_dir() {
                PathDirectoryEntryKind::Directory
            } else if file_type.is_symlink() {
                PathDirectoryEntryKind::Symlink
            } else {
                PathDirectoryEntryKind::Unknown
            };
            PathDirectoryEntry::new(PathDirectoryName::new(entry.file_name()).unwrap(), kind)
        });
        (
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(directory.clone()).unwrap(),
                PathObservationOperation::DirectoryEntries,
            ),
            Arc::new(PathObservationResult::DirectoryEntries(
                PathOperationResult::Present(PathDirectoryEntries::new(entries)),
            )),
        )
    });
    PathObservationEpoch::from_shared(
        base.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(listings),
    )
    .unwrap()
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
    let lockfile = NormalizedAbsolutePath::new(root.join("MODULE.bazel.lock")).unwrap();
    entries.insert(
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            lockfile.clone(),
            PathObservationOperation::Lstat,
        ),
        PathObservationResult::Lstat(PathOperationResult::Missing),
    );
    entries.insert(
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            lockfile,
            PathObservationOperation::FileBytes,
        ),
        PathObservationResult::FileBytes(PathOperationResult::Missing),
    );
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
    loading_gate: Option<LoadingActivationGate>,
}

struct LoadingActivationGate {
    needle: &'static str,
    reached: Mutex<Option<SyncSender<()>>>,
    release: Mutex<Receiver<()>>,
}

impl RootActivationTracker {
    fn with_loading() -> Self {
        Self {
            all_loading: true,
            ..Default::default()
        }
    }

    fn with_loading_gate(
        needle: &'static str,
        reached: SyncSender<()>,
        release: Receiver<()>,
    ) -> Self {
        Self {
            all_loading: true,
            loading_gate: Some(LoadingActivationGate {
                needle,
                reached: Mutex::new(Some(reached)),
                release: Mutex::new(release),
            }),
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

fn root_activation_identity(key: &ConfiguredTargetKey) -> String {
    let setting = CanonicalLabel::parse("@@//:setting").unwrap();
    format!(
        "resolved/{}={}",
        key.label(),
        key.configuration()
            .starlark_option(&setting)
            .and_then(|option| option.value().as_str())
            .unwrap_or("<default>")
    )
}

fn string_option(label: &str, value: impl Into<compact_str::CompactString>) -> StarlarkOption {
    StarlarkOption::string(
        CanonicalLabel::parse(label).unwrap(),
        value,
        StarlarkOptionScope::Default,
    )
}

fn root_string_option(value: impl Into<compact_str::CompactString>) -> StarlarkOption {
    string_option("@@//:setting", value)
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
    explicit: Option<StarlarkOption>,
) -> Result<ConfiguredNodeAnalysisKey, String> {
    let configuration = explicit.map_or(base_configuration.clone(), |explicit| {
        base_configuration.with_starlark_option(explicit)
    });
    match prepare_configured_node_analysis(transaction, workspace, target, configuration).await {
        AnalysisPreparationOutcome::Need(_) => Err("root request returned Needs".to_owned()),
        AnalysisPreparationOutcome::Complete(Ok(key)) => Ok(key),
        AnalysisPreparationOutcome::Complete(Err(error)) => Err(error.to_string()),
    }
}

async fn configured_condition_request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<ConfiguredConditionMatch, String> {
    configured_condition_request_with_inputs(
        dice,
        workspace,
        target,
        configuration,
        root_epoch(workspace),
        &[],
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn configured_condition_request_with_inputs(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
    epoch: PathObservationEpoch,
    repositories: &[(&str, &str)],
    tracker: Arc<RootActivationTracker>,
) -> Result<ConfiguredConditionMatch, String> {
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, workspace, epoch, repositories);
    let key = ConfiguredConditionKey::new(
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        ConfiguredTargetKey::new(CanonicalLabel::parse(target).unwrap(), configuration),
    )
    .map_err(|error| error.to_string())?;
    let mut transaction = updater.commit().await;
    match transaction
        .compute(&key)
        .await
        .map_err(|error| error.to_string())?
    {
        AnalysisPreparationOutcome::Need(need) => {
            Err(format!("configured condition returned Needs: {need:?}"))
        }
        AnalysisPreparationOutcome::Complete(Err(error)) => Err(error.to_string()),
        AnalysisPreparationOutcome::Complete(Ok(result)) => {
            result.as_ref().clone().map_err(|error| error.to_string())
        }
    }
}

fn configured_platform_result(
    outcome: ConfiguredPlatformOutcome,
) -> Result<Arc<ConfiguredPlatform>, String> {
    match outcome {
        AnalysisPreparationOutcome::Need(need) => {
            Err(format!("configured platform returned Needs: {need:?}"))
        }
        AnalysisPreparationOutcome::Complete(Err(error)) => Err(error.to_string()),
        AnalysisPreparationOutcome::Complete(Ok(result)) => {
            result.as_ref().clone().map_err(|error| error.to_string())
        }
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
        if let Some(key) = key.downcast_ref::<ConfiguredConditionKey>() {
            let identity = format!("condition/{}", key.target());
            self.activations
                .lock()
                .unwrap()
                .push((identity.clone(), activation.kind()));
            self.nodes.lock().unwrap().push((
                identity,
                activation.node(),
                activation.dependencies().to_vec(),
            ));
            return;
        }
        let analysis = key
            .downcast_ref::<ConfiguredNodeAnalysisKey>()
            .and_then(|key| key.configured_target().map(|target| (target, false)))
            .or_else(|| {
                key.downcast_ref::<ConfiguredNodeAnalysisObservationKey>()
                    .and_then(|key| key.configured_target().map(|target| (target, true)))
            });
        if let Some((target, observed)) = analysis {
            let mut identity = root_activation_identity(target);
            if observed {
                identity.insert_str(0, "observed/");
            }
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
            let identity = if let Some(key) = key.downcast_ref::<HostPackageInventoryKey>() {
                Some(format!("package/{key}"))
            } else if let Some(key) = key.downcast_ref::<HostPackageInventoryObservationKey>() {
                Some(format!("package/{key}"))
            } else if let Some(key) = key.downcast_ref::<ModuleRegistrationExpansionKey>() {
                Some(format!("registration/legacy/{}", key.family()))
            } else if let Some(key) =
                key.downcast_ref::<ModuleRegistrationExpansionObservationKey>()
            {
                Some(format!("registration/observed/{}", key.family()))
            } else if let Some(key) = key.downcast_ref::<CommandRegistrationExpansionKey>() {
                Some(format!("registration/command-legacy/{}", key.family()))
            } else if let Some(key) =
                key.downcast_ref::<CommandRegistrationExpansionObservationKey>()
            {
                Some(format!("registration/command-observed/{}", key.family()))
            } else if key
                .downcast_ref::<CommandConfigurationPreparationObservationKey>()
                .is_some()
            {
                Some("command-configuration/observed".to_owned())
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
                    identity.clone(),
                    activation.node(),
                    activation.dependencies().to_vec(),
                ));
                if let Some(gate) = &self.loading_gate
                    && identity.contains(gate.needle)
                    && let Some(reached) = gate.reached.lock().unwrap().take()
                {
                    reached.send(()).unwrap();
                    gate.release.lock().unwrap().recv().unwrap();
                }
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
    command_configuration_request_result(
        dice,
        workspace,
        target,
        explicit
            .map(|value| CommandConfigurationOccurrence::starlark("//:setting", Some(value), false))
            .into_iter()
            .collect::<Vec<_>>()
            .into(),
        tracker,
    )
    .await
}

async fn command_configuration_request_result(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    overlay: CommandConfigurationOverlay,
    tracker: Arc<RootActivationTracker>,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    command_configuration_request_result_with_inputs(dice, workspace, target, overlay, tracker, &[])
        .await
}

async fn command_configuration_request_result_with_inputs(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    overlay: CommandConfigurationOverlay,
    tracker: Arc<RootActivationTracker>,
    repositories: &[(&str, &str)],
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, workspace, root_epoch(workspace), repositories);
    let mut transaction = updater.commit().await;
    let preparation = CommandConfigurationPreparationKey::new(
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        test_configuration(),
        overlay,
    )
    .map_err(|error| error.to_string())?;
    let configuration = match transaction
        .compute(&CommandConfigurationPreparationObservationKey::new(
            preparation,
        ))
        .await
        .map_err(|error| error.to_string())?
    {
        AnalysisPreparationOutcome::Need(_) => {
            return Err("command configuration preparation returned Needs".to_owned());
        }
        AnalysisPreparationOutcome::Complete(Err(error)) => return Err(error.to_string()),
        AnalysisPreparationOutcome::Complete(Ok(observed)) => observed
            .result()
            .as_ref()
            .cloned()
            .map_err(|error| error.to_string())?,
    };
    let analysis_key = prepared_analysis_key(
        &mut transaction,
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        CanonicalLabel::parse(target).unwrap(),
        configuration,
        None,
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
    root_target_request_with_inputs(
        dice,
        workspace,
        target,
        configuration,
        tracker,
        root_epoch(workspace),
        &[],
    )
    .await
}

fn inject_root_target_inputs(
    updater: &mut DiceTransactionUpdater,
    workspace: &std::path::Path,
    epoch: PathObservationEpoch,
    repositories: &[(&str, &str)],
) {
    let text = Arc::new(workspace_snapshot(workspace));
    let raw = raw_snapshot_from_text(&text);
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
    if !repositories.is_empty() {
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: root.dupe(),
                },
                local_repository_materializations(&root, repositories),
            )])
            .unwrap();
    }
    inject_root_package_policy_inputs(
        updater,
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
        updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn root_target_request_with_inputs(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<RootActivationTracker>,
    epoch: PathObservationEpoch,
    repositories: &[(&str, &str)],
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_explicit_inputs(
        dice,
        workspace,
        target,
        configuration,
        None,
        tracker,
        epoch,
        repositories,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn root_target_request_with_explicit_inputs(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
    explicit: Option<StarlarkOption>,
    tracker: Arc<RootActivationTracker>,
    epoch: PathObservationEpoch,
    repositories: &[(&str, &str)],
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut user_data = UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    user_data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(user_data);
    inject_root_target_inputs(&mut updater, workspace, epoch, repositories);
    let mut transaction = updater.commit().await;
    let analysis_key = prepared_analysis_key(
        &mut transaction,
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        CanonicalLabel::parse(target).unwrap(),
        configuration,
        explicit,
    )
    .await?;
    let value = transaction
        .compute(&analysis_key)
        .await
        .map_err(|error| error.to_string())?;
    let AnalysisPreparationOutcome::Complete(value) = value else {
        return Err(format!("root target returned Needs: {value:#?}"));
    };
    value
        .as_ref()
        .as_ref()
        .cloned()
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn observed_root_target_request_with_inputs(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<RootActivationTracker>,
    epoch: PathObservationEpoch,
    repositories: &[(&str, &str)],
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let mut data = UserComputationData {
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    inject_root_target_inputs(&mut updater, workspace, epoch, repositories);
    let mut transaction = updater.commit().await;
    let key = ConfiguredNodeAnalysisObservationKey::new(
        NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
        ConfiguredTargetKey::new(CanonicalLabel::parse(target).unwrap(), configuration),
    )
    .unwrap();
    let value = transaction
        .compute(&key)
        .await
        .map_err(|error| error.to_string())?;
    let AnalysisPreparationOutcome::Complete(Ok(value)) = value else {
        return Err(format!("observed root target did not complete: {value:#?}"));
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

fn root_setting_value(key: &ConfiguredTargetKey) -> Option<&str> {
    let setting = CanonicalLabel::parse("@@//:setting").unwrap();
    key.configuration()
        .starlark_option(&setting)
        .and_then(|option| option.value().as_str())
}

const TOOLCHAIN_MODULE: &str = "module(name = \"bazel_tools\")\nregister_execution_platforms(\"//:platform\")\nregister_toolchains(\"//:second\", \"//:first\")\n";
const TOOLCHAIN_DEFS: &str = r#"ConsumerInfo = provider(fields = {"value": ""})
def _first(ctx):
    print("FIRST_LOCAL")
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]
def _second(ctx):
    print("SECOND_LOCAL")
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]
def _request(ctx):
    print("REQUEST_LOCAL")
    out = ctx.actions.declare_file("request.out")
    ctx.actions.write(out, "configured action")
    return [ConsumerInfo(value = ctx.toolchains["//:type"].marker), DefaultInfo(files = depset([out]))]
first_impl = rule(implementation = _first, attrs = {"marker": attr.string(mandatory = True)})
second_impl = rule(implementation = _second, attrs = {"marker": attr.string(mandatory = True)})
request = rule(implementation = _request, toolchains = ["//:type"])
"#;
const TOOLCHAIN_BUILD: &str = "load(\":defs.bzl\", \"first_impl\", \"request\", \"second_impl\")\nconstraint_setting(name = \"setting\")\nconstraint_value(name = \"linux\", constraint_setting = \":setting\")\nconstraint_value(name = \"other\", constraint_setting = \":setting\")\nplatform(name = \"platform\", constraint_values = [\":linux\"])\ntoolchain_type(name = \"type\")\nfirst_impl(name = \"first_impl\", marker = \"first\")\nsecond_impl(name = \"second_impl\", marker = \"second\")\ntoolchain(name = \"first\", toolchain_type = \":type\", toolchain = \":first_impl\", exec_compatible_with = [\":linux\"])\ntoolchain(name = \"second\", toolchain_type = \":type\", toolchain = \":second_impl\", exec_compatible_with = [\":linux\"])\nrequest(name = \"request\")\n";
const TOPOLOGY_MODULE: &str = "module(name = \"bazel_tools\")\nregister_execution_platforms(\"//:first_platform\", \"//:second_platform\")\nregister_toolchains(\"//:first_toolchain\", \"//:second_toolchain\")\n";
const TOPOLOGY_BUILD: &str = "load(\":defs.bzl\", \"first_impl\", \"request\", \"second_impl\")\nconstraint_setting(name = \"selection\")\nconstraint_value(name = \"first\", constraint_setting = \":selection\")\nconstraint_value(name = \"second\", constraint_setting = \":selection\")\nplatform(name = \"first_platform\", constraint_values = [\":first\"], exec_properties = {\"z\": \"last\", \"a\": \"first\"})\nplatform(name = \"second_platform\", constraint_values = [\":second\"])\ntoolchain_type(name = \"type\")\nfirst_impl(name = \"first_impl\", marker = \"first\")\nsecond_impl(name = \"second_impl\", marker = \"second\")\nfirst_impl(name = \"orphan\", marker = \"orphan\")\ntoolchain(name = \"first_toolchain\", toolchain_type = \":type\", toolchain = \":first_impl\", exec_compatible_with = [\":first\"])\ntoolchain(name = \"second_toolchain\", toolchain_type = \":type\", toolchain = \":second_impl\", exec_compatible_with = [\":second\"])\nrequest(name = \"request\")\n";

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

async fn topology_platform(
    dice: &Arc<Dice>,
    workspace: &PathBuf,
    configuration: &ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        workspace,
        "@@//:first_platform",
        configuration.clone(),
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
    assert_eq!(first.actions().len(), 1);
    assert_eq!(first.declared_outputs(), &["request.out"]);
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
    let context = first.actions()[0].context();
    assert_eq!(
        context.execution_state(),
        ActionExecutionState::SelectedToolchain
    );
    assert_eq!(context.owner(), first.configured_target_key().unwrap());
    assert_eq!(
        context.exec_group(),
        &slug_analysis_v2::ConfiguredActionExecGroup::Default
    );
    assert_eq!(context.toolchain().unwrap().marker(), "second");
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
async fn command_registrations_precede_module_and_empty_overlay_restores_module_only_result() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), TOOLCHAIN_MODULE).unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        TOOLCHAIN_BUILD.replacen(
            "platform(name = \"platform\", constraint_values = [\":linux\"])",
            "platform(name = \"platform\", constraint_values = [\":linux\"])\nplatform(name = \"command_platform\", constraint_values = [\":linux\"])",
            1,
        ),
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    let overlay: CommandConfigurationOverlay = vec![
        CommandConfigurationOccurrence::extra_toolchains("//:first,-//:first"),
        CommandConfigurationOccurrence::extra_execution_platforms(
            "//:command_platform,-//:command_platform,//:command_platform",
        ),
    ]
    .into();
    let first = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:request",
        overlay.clone(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&first, &consumer), "first");
    assert_eq!(
        candidate_labels(&first),
        ["@@//:command_platform", "@@//:platform"]
    );
    assert_eq!(
        first
            .toolchain_topology()
            .unwrap()
            .selection()
            .unwrap()
            .declaration()
            .to_string(),
        "@@//:first"
    );

    let module_only = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:request",
        CommandConfigurationOverlay::default(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&module_only, &consumer), "second");
    assert_eq!(candidate_labels(&module_only), ["@@//:platform"]);

    let restored = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:request",
        overlay,
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(restored, first);
}

#[tokio::test]
async fn root_toolchain_topology_retains_intrinsic_candidates_selection_and_constraint_chain() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), TOPOLOGY_MODULE).unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(workspace.join("BUILD.bazel"), TOPOLOGY_BUILD).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = || Arc::new(RootActivationTracker::default());

    let direct_tracker = Arc::new(RootActivationTracker::with_loading());
    let direct_impl =
        root_target_request(&dice, &workspace, "@@//:first_impl", direct_tracker.clone())
            .await
            .unwrap();
    let direct_loading = direct_tracker.take().0;
    assert_eq!(
        direct_loading
            .iter()
            .filter(|(identity, _)| identity.starts_with("registration/legacy/"))
            .map(|(identity, _)| identity.as_str())
            .collect::<Vec<_>>(),
        [
            "registration/legacy/execution-platforms",
            "registration/legacy/toolchains",
        ]
    );
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
    let first_context = first.actions()[0].context();
    let first_selection = first.toolchain_topology().unwrap().selection().unwrap();
    let target_configuration = test_configuration();
    let exec_configuration = ConfigurationKey::from_slug(
        target_configuration
            .slug_configuration()
            .unwrap()
            .to_exec_for_platform(&CanonicalLabel::parse("@@//:first_platform").unwrap())
            .unwrap(),
    );
    assert_eq!(
        first_selection.execution_platform().configuration(),
        &exec_configuration
    );
    let platform = topology_platform(&dice, &workspace, &exec_configuration)
        .await
        .unwrap();
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

    assert_eq!(platform.kind(), &ConfiguredNodeKind::Platform);
    assert_eq!(
        platform
            .platform_semantic_fact()
            .unwrap()
            .exec_properties
            .as_ref(),
        &[("a".into(), "first".into()), ("z".into(), "last".into())]
    );
    assert_eq!(platform.edges().len(), 1);
    assert_eq!(
        first_context.platform_fact(),
        platform.platform_semantic_fact()
    );
    assert_eq!(first_context.toolchain().unwrap().marker(), "first");
    assert_eq!(first_context.platform_constraints().len(), 1);
    assert_eq!(
        first_context.platform_constraints()[0]
            .constraint_value()
            .label()
            .to_string(),
        "@@//:first"
    );
    assert_eq!(
        first_context.platform_constraints()[0]
            .constraint_setting()
            .label()
            .to_string(),
        "@@//:selection"
    );
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
        exec_configuration.clone(),
        tracker(),
    )
    .await
    .unwrap();
    assert_eq!(setting.kind(), &ConfiguredNodeKind::ConstraintSetting);
    assert!(setting.edges().is_empty());

    let reordered_source = TOPOLOGY_BUILD.replace(
        "{\"z\": \"last\", \"a\": \"first\"}",
        "{\"a\": \"first\", \"z\": \"last\"}",
    );
    fs::write(workspace.join("BUILD.bazel"), &reordered_source).unwrap();
    let reordered = topology_platform(&dice, &workspace, &exec_configuration)
        .await
        .unwrap();
    assert_eq!(reordered, platform);
    fs::write(
        workspace.join("BUILD.bazel"),
        reordered_source.replace("\"last\"", "\"edited\""),
    )
    .unwrap();
    assert_ne!(
        topology_platform(&dice, &workspace, &exec_configuration)
            .await
            .unwrap(),
        platform
    );
    assert_ne!(
        root_target_request(&dice, &workspace, "@@//:request", tracker())
            .await
            .unwrap(),
        first
    );
    fs::write(workspace.join("BUILD.bazel"), TOPOLOGY_BUILD).unwrap();
    assert_eq!(
        topology_platform(&dice, &workspace, &exec_configuration)
            .await
            .unwrap(),
        platform
    );
    assert_eq!(
        root_target_request(&dice, &workspace, "@@//:request", tracker())
            .await
            .unwrap(),
        first
    );
    let direct_platform = root_target_request(&dice, &workspace, "@@//:first_platform", tracker())
        .await
        .unwrap();
    assert_eq!(direct_platform.kind(), &ConfiguredNodeKind::Platform);
    let direct_toolchain =
        root_target_request(&dice, &workspace, "@@//:first_toolchain", tracker())
            .await
            .unwrap();
    assert_eq!(
        direct_toolchain.kind(),
        &ConfiguredNodeKind::ToolchainDeclaration
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
            .contains("target pattern repository '@external' is not visible")
    );
    fs::write(
        workspace.join("BUILD.bazel"),
        TOPOLOGY_BUILD.replace(
            "exec_properties = {\"z\": \"last\", \"a\": \"first\"}",
            "exec_properties = {\"z\": \"last\", \"a\": \"first\"}, remote_execution_properties = \"legacy\"",
        ),
    )
    .unwrap();
    assert!(
        topology_platform(&dice, &workspace, &exec_configuration)
            .await
            .unwrap_err()
            .contains("unsupported nondefault attribute remote_execution_properties")
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
async fn root_toolchain_resolution_fails_closed_on_unimplemented_target_compatibility() {
    let cases = [
        (
            "target compatibility",
            TOOLCHAIN_BUILD.replacen(
                "exec_compatible_with = [\":linux\"])",
                "exec_compatible_with = [\":linux\"], target_compatible_with = [\":linux\"])",
                1,
            ),
        ),
        (
            "target platform constraints",
            TOOLCHAIN_BUILD.replacen(
                "exec_compatible_with = [\":linux\"])",
                "exec_compatible_with = [\":linux\"], use_target_platform_constraints = True)",
                1,
            ),
        ),
    ];
    for (name, build) in cases {
        let workspace = scratch();
        fs::write(workspace.join("MODULE.bazel"), TOOLCHAIN_MODULE).unwrap();
        fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
        fs::write(workspace.join("BUILD.bazel"), build).unwrap();
        for observed in [false, true] {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let error = if observed {
                observed_root_target_request_with_inputs(
                    &dice,
                    &workspace,
                    "@@//:request",
                    test_configuration(),
                    Arc::new(RootActivationTracker::default()),
                    root_epoch(&workspace),
                    &[],
                )
                .await
            } else {
                root_target_request(
                    &dice,
                    &workspace,
                    "@@//:request",
                    Arc::new(RootActivationTracker::default()),
                )
                .await
            }
            .unwrap_err();
            assert!(
                error.contains("registered toolchain uses unsupported target compatibility"),
                "{name} observed={observed}: {error}"
            );
        }
    }
}

#[tokio::test]
async fn configurable_toolchain_target_settings_use_distinct_selector_and_selected_conditions() {
    let module =
        TOOLCHAIN_MODULE.replace("\"//:second\", \"//:first\"", "\"//:first\", \"//:second\"");
    for (selected_mode, expected) in [("fastbuild", "@@//:first"), ("opt", "@@//:second")] {
        let build = format!(
            "config_setting(name = \"choose_settings\", values = {{\"compilation_mode\": \"fastbuild\"}})\n\
             config_setting(name = \"selected_setting\", values = {{\"compilation_mode\": \"{selected_mode}\"}})\n{}",
            TOOLCHAIN_BUILD.replacen(
                "exec_compatible_with = [\":linux\"])",
                "exec_compatible_with = [\":linux\"], target_settings = select({\":choose_settings\": [\":selected_setting\"], \"//conditions:default\": [\":unselected_missing\"]}))",
                1,
            )
        );
        let result = toolchain_case(&module, TOOLCHAIN_DEFS, &build)
            .await
            .unwrap();
        assert_eq!(
            result
                .toolchain_topology()
                .unwrap()
                .selection()
                .unwrap()
                .declaration()
                .to_string(),
            expected
        );
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
async fn selected_toolchain_accepts_declared_actions_and_default_outputs() {
    let workspace = scratch();
    let defs = TOOLCHAIN_DEFS.replacen(
        "    print(\"FIRST_LOCAL\")\n    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]",
        "    out = ctx.actions.declare_file(\"toolchain.txt\")\n    ctx.actions.write(out, \"toolchain\")\n    return [DefaultInfo(files = depset([out])), platform_common.ToolchainInfo(marker = ctx.attr.marker)]",
        1,
    );
    let module =
        TOOLCHAIN_MODULE.replace("\"//:second\", \"//:first\"", "\"//:first\", \"//:second\"");
    fs::write(workspace.join("MODULE.bazel"), module).unwrap();
    fs::write(workspace.join("defs.bzl"), &defs).unwrap();
    fs::write(workspace.join("BUILD.bazel"), TOOLCHAIN_BUILD).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = || Arc::new(RootActivationTracker::default());
    let result = root_target_request(&dice, &workspace, "@@//:request", tracker())
        .await
        .unwrap();
    assert!(result.edges().iter().any(|edge| {
        edge.kind() == &slug_analysis_v2::ConfiguredEdgeKind::SelectedToolchainImplementation
    }));
    let direct = root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
        .await
        .unwrap();
    assert_eq!(direct.actions().len(), 1);
    assert_eq!(
        direct.actions()[0].context().execution_state(),
        ActionExecutionState::SelectedPlatformOnly
    );
    assert!(direct.actions()[0].context().toolchain().is_none());
    assert_eq!(direct.configured_file_write_actions().unwrap().len(), 1);
    fs::write(
        workspace.join("BUILD.bazel"),
        TOOLCHAIN_BUILD.replace(
            "platform(name = \"platform\", constraint_values = [\":linux\"])",
            "platform(name = \"platform\", constraint_values = [\":linux\"], exec_properties = {\"mode\": \"edited\"})",
        ),
    )
    .unwrap();
    assert_ne!(
        root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
            .await
            .unwrap(),
        direct
    );
    fs::write(workspace.join("BUILD.bazel"), TOOLCHAIN_BUILD).unwrap();
    assert_eq!(
        root_target_request(&dice, &workspace, "@@//:first_impl", tracker())
            .await
            .unwrap(),
        direct
    );
}

#[tokio::test]
async fn selected_platform_terminals_suppress_implementation_and_rule_evaluation() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), TOOLCHAIN_MODULE).unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(workspace.join("BUILD.bazel"), TOOLCHAIN_BUILD).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let seed = root_target_request(
        &dice,
        &workspace,
        "@@//:request",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let platform = seed
        .toolchain_topology()
        .unwrap()
        .selection()
        .unwrap()
        .execution_platform();
    let platform_key =
        ConfiguredNodeAnalysisObservationKey::new(root.clone(), platform.clone()).unwrap();
    let root_key = ConfiguredNodeAnalysisObservationKey::new(
        root,
        seed.configured_target_key().unwrap().clone(),
    )
    .unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("platform-terminal")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let outer =
        ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(demand.dupe()));
    let cases: [(&str, <ConfiguredNodeAnalysisObservationKey as Key>::Value); 2] = [
        (
            "outer",
            AnalysisPreparationOutcome::Complete(Err(outer.clone())),
        ),
        (
            "semantic",
            AnalysisPreparationOutcome::Complete(Ok(Arc::new(Ok(seed.clone())))),
        ),
    ];
    for (name, value) in cases {
        let tracker = Arc::new(RootActivationTracker::with_loading());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
        updater
            .changed_to(vec![(platform_key.clone(), value)])
            .unwrap();
        let mut transaction = updater.commit().await;
        let outcome = transaction.compute(&root_key).await.unwrap();
        match name {
            "outer" => assert!(
                matches!(&outcome, AnalysisPreparationOutcome::Complete(Err(error)) if error == &outer),
                "{outcome:#?}"
            ),
            "semantic" => assert!(matches!(
                &outcome,
                AnalysisPreparationOutcome::Complete(Ok(result)) if result.as_ref().is_err()
            )),
            _ => unreachable!(),
        }
        assert_eq!(
            ConfiguredNodeAnalysisObservationKey::validity(&outcome),
            name == "outer"
        );
        assert_eq!(
            ConfiguredNodeAnalysisObservationKey::equality(&outcome, &outcome),
            name == "outer"
        );
        let (activations, batches, _) = tracker.take();
        let identities = activations
            .iter()
            .map(|(identity, _)| identity)
            .collect::<Vec<_>>();
        assert!(
            identities
                .iter()
                .any(|identity| identity.contains("@@//:platform"))
        );
        assert!(identities.iter().all(|identity| {
            !identity.contains("@@//:first_impl") && !identity.contains("@@//:second_impl")
        }));
        assert!(
            identities
                .iter()
                .filter(|identity| identity.contains("resolved/"))
                .all(|identity| identity.starts_with("observed/")),
            "{name}: {identities:#?}"
        );
        assert!(
            batches
                .iter()
                .all(|(_, batch)| { !event_texts(batch).contains(&"REQUEST_LOCAL") })
        );
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
    let defs = "ConsumerInfo = provider(fields = {\"value\": \"\"})\ndef _request(ctx):\n    out = ctx.actions.declare_file(\"zero.txt\")\n    ctx.actions.write(out, \"zero\")\n    return [ConsumerInfo(value = \"zero\"), DefaultInfo(files = depset([out]))]\nrequest = rule(implementation = _request)\n";
    fs::write(workspace.join("defs.bzl"), defs).unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"request\")\nrequest(name = \"request\")\n",
    )
    .unwrap();
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let quiet_tracker = || Arc::new(RootActivationTracker::default());
    let result = root_target_request(&dice, &workspace, "@@//:request", tracker.clone())
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
    assert_eq!(result.actions().len(), 1);
    assert_eq!(
        result.actions()[0].context().execution_state(),
        ActionExecutionState::UnresolvedDefault
    );
    assert!(result.configured_file_write_actions().is_err());
    fs::write(
        workspace.join("defs.bzl"),
        defs.replace("\"zero\")", "\"edited\")"),
    )
    .unwrap();
    assert_ne!(
        root_target_request(&dice, &workspace, "@@//:request", quiet_tracker())
            .await
            .unwrap(),
        result
    );
    fs::write(workspace.join("defs.bzl"), defs).unwrap();
    assert_eq!(
        root_target_request(&dice, &workspace, "@@//:request", quiet_tracker())
            .await
            .unwrap(),
        result
    );
    let (activations, _, nodes) = tracker.take();
    assert!(
        activations
            .iter()
            .any(|(identity, _)| identity.starts_with("package/"))
    );
    assert!(
        activations
            .iter()
            .all(|(identity, _)| !identity.starts_with("registration/"))
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
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"bazel_tools\")\nregister_execution_platforms(\"//platforms:p\")\nregister_toolchains(\"//tools:tc\")\n").unwrap();
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
async fn selected_nonroot_registrations_preserve_canonical_repository_identity() {
    let workspace = scratch();
    for repository in ["dep_a", "dep_b"] {
        fs::create_dir_all(workspace.join(repository).join("shared")).unwrap();
        fs::write(workspace.join(repository).join("REPO.bazel"), "").unwrap();
        fs::write(workspace.join(repository).join(".bazelignore"), "").unwrap();
        fs::write(
            workspace.join(repository).join("shared/defs.bzl"),
            "def _impl(ctx): return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]\nimpl = rule(implementation = _impl, attrs = {\"marker\": attr.string(mandatory = True)})\n",
        )
        .unwrap();
    }
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"bazel_tools\")\nbazel_dep(name = \"dep_a\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep_a\", path = \"dep_a\")\nbazel_dep(name = \"dep_b\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep_b\", path = \"dep_b\")\nregister_toolchains(\"@dep_b//shared:selected\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "ConsumerInfo = provider(fields = {\"value\": \"\"})\ndef _request(ctx): return [ConsumerInfo(value = ctx.toolchains[\"//:type\"].marker)]\nrequest = rule(implementation = _request, toolchains = [\"//:type\"])\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"request\")\ntoolchain_type(name = \"type\")\nrequest(name = \"request\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep_a/MODULE.bazel"),
        "module(name = \"dep_a\", version = \"1.0.0\")\nregister_execution_platforms(\"//shared:platform\")\nregister_toolchains(\"//shared:all\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep_a/shared/BUILD.bazel"),
        "load(\":defs.bzl\", \"impl\")\nconstraint_setting(name = \"setting\")\nconstraint_value(name = \"value\", constraint_setting = \":setting\")\nplatform(name = \"platform\", constraint_values = [\":value\"])\ntoolchain_type(name = \"type\")\nimpl(name = \"impl\", marker = \"a\")\ntoolchain(name = \"toolchain\", toolchain_type = \":type\", toolchain = \":impl\", exec_compatible_with = [\":value\"])\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep_b/MODULE.bazel"),
        "module(name = \"dep_b\", version = \"1.0.0\")\nregister_execution_platforms(\"//shared/...\")\nregister_toolchains(\"//shared/...\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep_b/shared/BUILD.bazel"),
        "load(\":defs.bzl\", \"impl\")\nconstraint_setting(name = \"setting\")\nconstraint_value(name = \"value\", constraint_setting = \":setting\")\nplatform(name = \"platform\", constraint_values = [\":value\"])\ntoolchain_type(name = \"type\")\nimpl(name = \"impl\", marker = \"b\")\nimpl(name = \"unused_impl\", marker = \"unused\")\ntoolchain(name = \"toolchain\", toolchain_type = \":type\", toolchain = \":unused_impl\", exec_compatible_with = [\":value\"])\ntoolchain(name = \"selected\", toolchain_type = \"@@//:type\", toolchain = \":impl\", exec_compatible_with = [\":value\"])\n",
    )
    .unwrap();
    let repositories = [("dep_a+", "dep_a"), ("dep_b+", "dep_b")];
    let epoch = root_epoch_with_listings(&workspace, &[workspace.join("dep_b").join("shared")]);
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let legacy = root_target_request_with_inputs(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:request",
        test_configuration(),
        tracker.clone(),
        epoch.dupe(),
        &repositories,
    )
    .await
    .unwrap();
    let observed = observed_root_target_request_with_inputs(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:request",
        test_configuration(),
        Arc::new(RootActivationTracker::default()),
        epoch,
        &repositories,
    )
    .await
    .unwrap();
    assert_eq!(observed, legacy);
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    assert_eq!(provider_value(&legacy, &consumer), "b");
    assert_eq!(
        candidate_labels(&legacy),
        ["@@dep_a+//shared:platform", "@@dep_b+//shared:platform",]
    );
    let selection = legacy.toolchain_topology().unwrap().selection().unwrap();
    assert_eq!(
        selection.declaration().to_string(),
        "@@dep_b+//shared:selected"
    );
    assert_eq!(
        selection.implementation().label().to_string(),
        "@@dep_b+//shared:impl"
    );
    let packages = tracker
        .take()
        .0
        .into_iter()
        .filter(|(identity, _)| identity.starts_with("package/"))
        .map(|(identity, _)| identity)
        .collect::<Vec<_>>();
    assert!(
        packages
            .iter()
            .any(|identity| identity.contains("@@dep_a+//shared"))
    );
    assert!(
        packages
            .iter()
            .any(|identity| identity.contains("@@dep_b+//shared"))
    );
}

#[tokio::test]
async fn registration_family_need_precedes_earlier_semantic_error() {
    let module = "module(name = \"bazel_tools\")\nregister_execution_platforms(\"@external//:p\")\nregister_toolchains(\"//missing:tc\")\n";
    let need = toolchain_case(module, TOOLCHAIN_DEFS, TOOLCHAIN_BUILD)
        .await
        .unwrap_err();
    assert!(need.starts_with("root target returned Needs:"), "{need}");

    let semantic = toolchain_case(
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
        semantic.contains("target pattern repository '@external' is not visible"),
        "{semantic}"
    );
}

#[tokio::test]
async fn registration_family_outer_precedes_earlier_semantic_error() {
    let workspace = scratch();
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"bazel_tools\")\nregister_execution_platforms(\"@external//:p\")\nregister_toolchains(\"//:second\")\n",
    )
    .unwrap();
    fs::write(workspace.join("defs.bzl"), TOOLCHAIN_DEFS).unwrap();
    fs::write(workspace.join("BUILD.bazel"), TOOLCHAIN_BUILD).unwrap();
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("registration-terminal")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let outer = ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(demand));
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let mut data = UserComputationData {
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut updater = dice.updater_with_data(data);
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    updater
        .changed_to(vec![(
            ModuleRegistrationExpansionObservationKey::toolchains(root.dupe()),
            AnalysisPreparationOutcome::Complete(Err(
                ModuleRegistrationExpansionObservationError::Frontier(outer.clone()),
            )),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let key = ConfiguredNodeAnalysisObservationKey::new(
        root,
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:request").unwrap(),
            test_configuration(),
        ),
    )
    .unwrap();
    let outcome = transaction.compute(&key).await.unwrap();
    assert!(
        matches!(&outcome, AnalysisPreparationOutcome::Complete(Err(error)) if error == &outer),
        "{outcome:#?}"
    );
    let registrations = tracker
        .take()
        .0
        .into_iter()
        .filter(|(identity, _)| identity.starts_with("registration/"))
        .map(|(identity, _)| identity)
        .collect::<Vec<_>>();
    assert_eq!(
        registrations,
        [
            "registration/command-observed/execution-platforms",
            "registration/observed/execution-platforms",
            "registration/command-observed/toolchains",
            "registration/observed/toolchains",
        ]
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
async fn command_configuration_preparation_preserves_lifecycle_transition_and_identity() {
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
    let need_before_missing = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:missing",
        vec![CommandConfigurationOccurrence::starlark(
            "//missing_settings:setting",
            Some("command"),
            false,
        )]
        .into(),
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
        None
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
        None
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
        [Some("left"), Some("right")]
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
        Some("command")
    );

    let (activations, _, _) = tracker.take();
    assert_eq!(
        activation_codes(&activations),
        r#"resolved/@@//:consumer=<default>:E resolved/@@//:consumer=<default>:E resolved/@@//:consumer=<default>:E
resolved/@@//:consumer=<default>:R resolved/@@//:consumer=<default>:R resolved/@@//:consumer=changed:E
resolved/@@//:consumer=command:E resolved/@@//:consumer=left:E resolved/@@//:consumer=right:E resolved/@@//:consumer=right:E
resolved/@@//:parent=<default>:E resolved/@@//:parent=<default>:E resolved/@@//:parent=<default>:R
resolved/@@//:setting=<default>:E resolved/@@//:setting=<default>:E resolved/@@//:setting=<default>:R
resolved/@@//:setting=changed:E resolved/@@//:setting=command:E resolved/@@//:setting=left:E resolved/@@//:setting=right:E
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
        unrelated_error.contains("target @@//:setting is not a Starlark build setting"),
        "{unrelated_error}"
    );
}

#[tokio::test]
async fn command_configuration_prepares_every_admitted_build_setting_shape() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _impl(ctx): return []
integer = rule(implementation = _impl, build_setting = config.int(flag = True))
boolean = rule(implementation = _impl, build_setting = config.bool(flag = True))
string = rule(implementation = _impl, build_setting = config.string(flag = True))
multi = rule(implementation = _impl, build_setting = config.string(flag = True, allow_multiple = True))
string_list = rule(implementation = _impl, build_setting = config.string_list(flag = True, repeatable = True))
string_set = rule(implementation = _impl, build_setting = config.string_set(flag = True, repeatable = True))
not_flag = rule(implementation = _impl, build_setting = config.string())
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "boolean", "integer", "multi", "not_flag", "string", "string_list", "string_set")
integer(name = "integer", build_setting_default = 7)
boolean(name = "boolean", build_setting_default = False)
string(name = "string", build_setting_default = "default")
multi(name = "multi", build_setting_default = "one")
string_list(name = "list", build_setting_default = [])
string_set(name = "set", build_setting_default = set())
not_flag(name = "not_flag", build_setting_default = "hidden")
"#,
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let row = |label: &'static str, value: Option<&'static str>, negated| {
        CommandConfigurationOccurrence::starlark(label, value, negated)
    };
    let result = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:integer",
        vec![
            row("//:integer", Some("8"), false),
            row("//:integer", Some("9"), false),
            row("//:boolean", None, false),
            row("//:boolean", None, true),
            row("//:string", Some("changed"), false),
            row("//:string", Some("default"), false),
            row("//:multi", Some("x"), false),
            row("//:multi", Some("x"), false),
            row("//:list", Some("a,b"), false),
            row("//:list", Some("c"), false),
            row("//:set", Some("z"), false),
            row("//:set", Some("a"), false),
            row("//:set", Some("z"), false),
        ]
        .into(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    let configuration = result.configured_target_key().unwrap().configuration();
    let option = |name: &str| {
        configuration
            .starlark_option(&CanonicalLabel::parse(&format!("@@//:{name}")).unwrap())
            .map(|option| option.value().clone())
    };
    assert_eq!(
        option("integer"),
        Some(StarlarkOptionValue::Integer(9.into()))
    );
    assert_eq!(option("boolean"), None);
    assert_eq!(option("string"), None);
    assert_eq!(
        option("multi"),
        Some(StarlarkOptionValue::string_list(["x", "x"]))
    );
    assert_eq!(
        option("list"),
        Some(StarlarkOptionValue::string_list(["a,b", "c"]))
    );
    assert_eq!(
        option("set"),
        Some(StarlarkOptionValue::string_set(["a", "z"]))
    );

    let malformed = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:integer",
        vec![
            row("//:integer", Some("bad"), false),
            row("//:integer", Some("10"), false),
        ]
        .into(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap_err();
    assert!(malformed.contains("not an integer"), "{malformed}");

    let non_flag = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//:integer",
        vec![row("//:not_flag", Some("visible"), false)].into(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap_err();
    assert!(non_flag.contains("not a command-line flag"), "{non_flag}");
}

#[tokio::test]
async fn typed_build_settings_resolve_all_value_shapes_scopes_and_defaults() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"Info = provider(fields = {"value": "value"})
def _integer(ctx): return [Info(value = "int:" + str(ctx.build_setting_value))]
def _boolean(ctx): return [Info(value = "bool:" + str(ctx.build_setting_value))]
def _string(ctx): return [Info(value = "string:" + ctx.build_setting_value)]
def _list(ctx): return [Info(value = "list:" + ",".join(ctx.build_setting_value))]
def _set(ctx): return [Info(value = "set:" + str(len(ctx.build_setting_value)) + ":" + str("a" in ctx.build_setting_value) + ":" + str("z" in ctx.build_setting_value))]
integer = rule(implementation = _integer, attrs = {"scope": attr.string()}, build_setting = config.int(flag = True))
boolean = rule(implementation = _boolean, attrs = {"scope": attr.string()}, build_setting = config.bool(flag = True))
string = rule(implementation = _string, attrs = {"scope": attr.string()}, build_setting = config.string(flag = True))
multi = rule(implementation = _list, attrs = {"scope": attr.string()}, build_setting = config.string(flag = True, allow_multiple = True))
string_list = rule(implementation = _list, attrs = {"scope": attr.string()}, build_setting = config.string_list(flag = True))
string_set = rule(implementation = _set, attrs = {"scope": attr.string()}, build_setting = config.string_set(flag = True))
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "boolean", "integer", "multi", "string", "string_list", "string_set")
integer(name = "integer", build_setting_default = 7, scope = "target")
boolean(name = "boolean", build_setting_default = False, scope = "universal")
string(name = "string", build_setting_default = "", scope = "project")
multi(name = "multi", build_setting_default = "one")
string_list(name = "list", build_setting_default = ["b", "a", "b"])
string_set(name = "set", build_setting_default = set(["b", "a"]))
"#,
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let info = ProviderId::new("//:defs.bzl", "Info").unwrap();
    let request = |target: &'static str, configuration, explicit| {
        root_target_request_with_explicit_inputs(
            &dice,
            &workspace,
            target,
            configuration,
            explicit,
            Arc::new(RootActivationTracker::default()),
            root_epoch(&workspace),
            &[],
        )
    };
    for (target, expected) in [
        ("@@//:integer", "int:7"),
        ("@@//:boolean", "bool:False"),
        ("@@//:string", "string:"),
        ("@@//:multi", "list:one"),
        ("@@//:list", "list:b,a,b"),
        ("@@//:set", "set:2:True:False"),
    ] {
        let result = request(target, test_configuration(), None)
            .await
            .unwrap_or_else(|error| panic!("{target}: {error}"));
        assert_eq!(provider_value(&result, &info), expected);
        assert!(
            result
                .configured_target_key()
                .unwrap()
                .configuration()
                .starlark_options()
                .iter()
                .next()
                .is_none(),
            "absent {target} must use its declaration default without a row"
        );
    }

    let cases = [
        (
            "@@//:integer",
            StarlarkOptionValue::Integer(
                BigInt::parse_bytes(b"9223372036854775808123", 10).unwrap(),
            ),
            StarlarkOptionScope::Target,
            "int:9223372036854775808123",
        ),
        (
            "@@//:boolean",
            StarlarkOptionValue::Boolean(true),
            StarlarkOptionScope::Universal,
            "bool:True",
        ),
        (
            "@@//:string",
            StarlarkOptionValue::string("changed"),
            StarlarkOptionScope::Project,
            "string:changed",
        ),
        (
            "@@//:multi",
            StarlarkOptionValue::string_list(["x", "x"]),
            StarlarkOptionScope::Default,
            "list:x,x",
        ),
        (
            "@@//:list",
            StarlarkOptionValue::string_list(["a", "b", "a"]),
            StarlarkOptionScope::Default,
            "list:a,b,a",
        ),
        (
            "@@//:set",
            StarlarkOptionValue::StringSet(Arc::from([
                compact_str::CompactString::from("z"),
                compact_str::CompactString::from("a"),
                compact_str::CompactString::from("z"),
            ])),
            StarlarkOptionScope::Default,
            "set:2:True:True",
        ),
    ];
    for (target, value, expected_scope, expected_provider) in cases {
        let label = CanonicalLabel::parse(target).unwrap();
        let explicit = StarlarkOption::new(label.clone(), value, expected_scope);
        let result = request(target, test_configuration(), Some(explicit))
            .await
            .unwrap();
        assert_eq!(provider_value(&result, &info), expected_provider);
        let retained = result
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&label)
            .unwrap();
        assert_eq!(retained.scope(), expected_scope);
    }

    let integer = CanonicalLabel::parse("@@//:integer").unwrap();
    let unrelated = CanonicalLabel::parse("@@//:unrelated").unwrap();
    let base = test_configuration().with_starlark_option(StarlarkOption::string(
        unrelated.clone(),
        "kept",
        StarlarkOptionScope::Universal,
    ));
    let first = request("@@//:integer", base.clone(), None).await.unwrap();
    let changed = request(
        "@@//:integer",
        base.clone(),
        Some(StarlarkOption::new(
            integer.clone(),
            StarlarkOptionValue::Integer(BigInt::from(8)),
            StarlarkOptionScope::Target,
        )),
    )
    .await
    .unwrap();
    let restored = request("@@//:integer", base, None).await.unwrap();
    assert_ne!(first.key(), changed.key());
    assert_eq!(first.key(), restored.key());
    assert!(
        restored
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&integer)
            .is_none()
    );
    assert_eq!(
        restored
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&unrelated)
            .unwrap()
            .value()
            .as_str(),
        Some("kept")
    );

    let error = request(
        "@@//:boolean",
        test_configuration(),
        Some(StarlarkOption::string(
            CanonicalLabel::parse("@@//:boolean").unwrap(),
            "true",
            StarlarkOptionScope::Default,
        )),
    )
    .await
    .unwrap_err();
    assert!(error.contains("expects Boolean, not string"), "{error}");
}

#[tokio::test]
async fn direct_config_settings_match_native_define_and_every_typed_flag_shape() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _empty(ctx): return []
integer = rule(implementation = _empty, build_setting = config.int(flag = True))
boolean = rule(implementation = _empty, build_setting = config.bool(flag = True))
string = rule(implementation = _empty, build_setting = config.string(flag = True))
multi = rule(implementation = _empty, build_setting = config.string(flag = True, allow_multiple = True))
string_list = rule(implementation = _empty, build_setting = config.string_list(flag = True))
string_set = rule(implementation = _empty, build_setting = config.string_set(flag = True))
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "boolean", "integer", "multi", "string", "string_list", "string_set")
integer(name = "integer", build_setting_default = 16)
boolean(name = "boolean", build_setting_default = False)
string(name = "string", build_setting_default = "text")
multi(name = "multi", build_setting_default = "one")
string_list(name = "list", build_setting_default = ["a", "b", "a"])
string_set(name = "set", build_setting_default = set(["a", "b"]))
filegroup(name = "ordinary")

config_setting(name = "native_match", values = {"compilation_mode": "fastbuild", "stamp": "false"})
config_setting(name = "native_no_match", values = {"compilation_mode": "opt"})
config_setting(name = "define_no_match", define_values = {"name": "value"})
config_setting(name = "combined", values = {"compilation_mode": "fastbuild"}, flag_values = {":integer": "0x10", ":boolean": "no"})
config_setting(name = "int_big", flag_values = {":integer": "0x10000000000000000"})
config_setting(name = "int_invalid", flag_values = {":integer": "010"})
config_setting(name = "bool_invalid", flag_values = {":boolean": "maybe"})
config_setting(name = "string_match", flag_values = {":string": "text"})
config_setting(name = "multi_match", flag_values = {":multi": "one"})
config_setting(name = "list_match", flag_values = {":list": "b"})
config_setting(name = "list_invalid", flag_values = {":list": "a,b"})
config_setting(name = "set_match", flag_values = {":set": "a,a"})
config_setting(name = "set_invalid", flag_values = {":set": ""})
config_setting(name = "wrong_flag", flag_values = {":ordinary": "x"})
config_setting(name = "empty")
constraint_setting(name = "constraint")
constraint_value(name = "value", constraint_setting = ":constraint")
platform(name = "condition_platform", constraint_values = [":value"])
config_setting(name = "constraint_match", constraint_values = [":value"])

config_setting(name = "bf_false", flag_values = {":boolean": "false"})
config_setting(name = "bf_zero", flag_values = {":boolean": "0"})
config_setting(name = "bf_no", flag_values = {":boolean": "no"})
config_setting(name = "bf_f", flag_values = {":boolean": "f"})
config_setting(name = "bf_n", flag_values = {":boolean": "n"})
config_setting(name = "bf_null", flag_values = {":boolean": "null"})
config_setting(name = "bt_true", flag_values = {":boolean": "true"})
config_setting(name = "bt_one", flag_values = {":boolean": "1"})
config_setting(name = "bt_yes", flag_values = {":boolean": "yes"})
config_setting(name = "bt_t", flag_values = {":boolean": "t"})
config_setting(name = "bt_y", flag_values = {":boolean": "y"})
"#,
    )
    .unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    for target in [
        "native_match",
        "combined",
        "string_match",
        "multi_match",
        "list_match",
        "set_match",
        "bf_false",
        "bf_zero",
        "bf_no",
        "bf_f",
        "bf_n",
    ] {
        assert_eq!(
            configured_condition_request(
                &dice,
                &workspace,
                &format!("@@//:{target}"),
                test_configuration(),
            )
            .await
            .unwrap_or_else(|error| panic!("{target}: {error}")),
            ConfiguredConditionMatch::Match,
            "{target}"
        );
    }
    for target in ["native_no_match", "define_no_match"] {
        assert_eq!(
            configured_condition_request(
                &dice,
                &workspace,
                &format!("@@//:{target}"),
                test_configuration(),
            )
            .await
            .unwrap(),
            ConfiguredConditionMatch::NoMatch,
            "{target}"
        );
    }

    let integer = CanonicalLabel::parse("@@//:integer").unwrap();
    let big = test_configuration().with_starlark_option(StarlarkOption::new(
        integer,
        StarlarkOptionValue::Integer(BigInt::parse_bytes(b"18446744073709551616", 10).unwrap()),
        StarlarkOptionScope::Default,
    ));
    assert_eq!(
        configured_condition_request(&dice, &workspace, "@@//:int_big", big)
            .await
            .unwrap(),
        ConfiguredConditionMatch::Match
    );
    let boolean = CanonicalLabel::parse("@@//:boolean").unwrap();
    let true_configuration = test_configuration().with_starlark_option(StarlarkOption::new(
        boolean,
        StarlarkOptionValue::Boolean(true),
        StarlarkOptionScope::Default,
    ));
    for target in ["bt_true", "bt_one", "bt_yes", "bt_t", "bt_y"] {
        assert_eq!(
            configured_condition_request(
                &dice,
                &workspace,
                &format!("@@//:{target}"),
                true_configuration.clone(),
            )
            .await
            .unwrap(),
            ConfiguredConditionMatch::Match,
            "{target}"
        );
    }

    for (target, expected) in [
        ("int_invalid", "cannot be converted to integer"),
        ("bool_invalid", "cannot be converted to Boolean"),
        ("bf_null", "cannot be converted to Boolean"),
        ("list_invalid", "single exact value"),
        ("set_invalid", "single exact value"),
        ("wrong_flag", "not a Starlark build setting"),
        ("empty", "at least one non-empty predicate"),
    ] {
        let error = configured_condition_request(
            &dice,
            &workspace,
            &format!("@@//:{target}"),
            test_configuration(),
        )
        .await
        .unwrap_err();
        assert!(error.contains(expected), "{target}: {error}");
    }
    let constraint_configuration = ConfigurationKey::from_slug(
        test_configuration()
            .slug_configuration()
            .unwrap()
            .to_exec_for_platform(&CanonicalLabel::parse("@@//:condition_platform").unwrap())
            .unwrap(),
    );
    assert_eq!(
        configured_condition_request(
            &dice,
            &workspace,
            "@@//:constraint_match",
            constraint_configuration,
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::Match
    );

    let integer = CanonicalLabel::parse("@@//:integer").unwrap();
    let wrong_kind = test_configuration().with_starlark_option(StarlarkOption::string(
        integer.clone(),
        "16",
        StarlarkOptionScope::Default,
    ));
    let error = configured_condition_request(&dice, &workspace, "@@//:combined", wrong_kind)
        .await
        .unwrap_err();
    assert!(error.contains("expects integer, not string"), "{error}");
    let wrong_scope = test_configuration().with_starlark_option(StarlarkOption::new(
        integer,
        StarlarkOptionValue::Integer(BigInt::from(16)),
        StarlarkOptionScope::Target,
    ));
    let error = configured_condition_request(&dice, &workspace, "@@//:combined", wrong_scope)
        .await
        .unwrap_err();
    assert!(
        error.contains("scope instead of declaration scope"),
        "{error}"
    );
}

#[tokio::test]
async fn configured_platform_normalizes_aliases_reuses_arc_and_matches_constraints() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"
constraint_setting(name = "setting")
alias(name = "setting_alias", actual = ":setting")
alias(name = "setting_alias_2", actual = ":setting_alias")
constraint_value(name = "value", constraint_setting = ":setting_alias_2")
constraint_value(name = "other_value", constraint_setting = ":setting")
alias(name = "value_alias", actual = ":value")
alias(name = "value_alias_2", actual = ":value_alias")
constraint_setting(name = "extra_setting")
constraint_value(name = "extra_value", constraint_setting = ":extra_setting")
platform(name = "platform", constraint_values = [":value_alias_2", ":extra_value"], exec_properties = {"worker": "local"})
alias(name = "platform_alias", actual = ":platform")
alias(name = "platform_alias_2", actual = ":platform_alias")
config_setting(name = "matches", constraint_values = [":value_alias_2"])
config_setting(name = "does_not_match", constraint_values = [":other_value"])
platform(name = "duplicate", constraint_values = [":value", ":value_alias_2"])
constraint_setting(name = "defaulted", default_constraint_value = ":default_value")
constraint_value(name = "default_value", constraint_setting = ":defaulted")
platform(name = "default_platform", constraint_values = [":default_value"])
platform(name = "wrong_platform", constraint_values = [":setting_alias"])
alias(name = "cycle_a", actual = ":cycle_b")
alias(name = "cycle_b", actual = ":cycle_a")
"#,
    )
    .unwrap();
    let selected = CanonicalLabel::parse("@@//:platform_alias_2").unwrap();
    let configuration = ConfigurationKey::from_slug(
        test_configuration()
            .slug_configuration()
            .unwrap()
            .to_exec_for_platform(&selected)
            .unwrap(),
    );
    let workspace_key = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let platform_key = ConfiguredPlatformKey::new(
        workspace_key.dupe(),
        ConfiguredTargetKey::new(selected.clone(), configuration.clone()),
    )
    .unwrap();
    let target_platform_key =
        ConfiguredTargetPlatformKey::new(workspace_key.dupe(), configuration.clone()).unwrap();
    let condition_key = ConfiguredConditionKey::new(
        workspace_key.dupe(),
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:matches").unwrap(),
            configuration.clone(),
        ),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let mut transaction = updater.commit().await;
    let first =
        configured_platform_result(transaction.compute(&platform_key).await.unwrap()).unwrap();
    let second =
        configured_platform_result(transaction.compute(&platform_key).await.unwrap()).unwrap();
    let target =
        configured_platform_result(transaction.compute(&target_platform_key).await.unwrap())
            .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &target));
    assert_eq!(
        first.requested().label().to_string(),
        "@@//:platform_alias_2"
    );
    assert_eq!(first.actual().label().to_string(), "@@//:platform");
    assert_eq!(
        first.fact().exec_properties.as_ref(),
        &[("worker".into(), "local".into())]
    );
    assert_eq!(first.constraints().len(), 2);
    assert_eq!(
        first.constraints()[0]
            .constraint_value()
            .label()
            .to_string(),
        "@@//:value"
    );
    assert_eq!(
        first.constraints()[0]
            .constraint_setting()
            .label()
            .to_string(),
        "@@//:setting"
    );
    let condition = transaction.compute(&condition_key).await.unwrap();
    assert!(
        matches!(condition, AnalysisPreparationOutcome::Complete(Ok(result)) if result.as_ref() == &Ok(ConfiguredConditionMatch::Match))
    );
    let no_match_key = ConfiguredConditionKey::new(
        workspace_key.dupe(),
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:does_not_match").unwrap(),
            configuration.clone(),
        ),
    )
    .unwrap();
    let no_match = transaction.compute(&no_match_key).await.unwrap();
    assert!(
        matches!(no_match, AnalysisPreparationOutcome::Complete(Ok(result)) if result.as_ref() == &Ok(ConfiguredConditionMatch::NoMatch))
    );
    let target_key = ConfiguredPlatformKey::new(
        workspace_key.dupe(),
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:platform").unwrap(),
            test_configuration(),
        ),
    )
    .unwrap();
    let target_platform =
        configured_platform_result(transaction.compute(&target_key).await.unwrap()).unwrap();
    assert_eq!(
        target_platform.actual().configuration().kind(),
        slug_analysis_v2::ConfigurationKind::Target
    );

    for (target, expected) in [
        ("duplicate", "duplicate constraint setting"),
        ("default_platform", "defaults are unsupported"),
        ("wrong_platform", "non-constraint value"),
        ("cycle_a", "configured alias cycle"),
    ] {
        let key = ConfiguredPlatformKey::new(
            workspace_key.dupe(),
            ConfiguredTargetKey::new(
                CanonicalLabel::parse(&format!("@@//:{target}")).unwrap(),
                configuration.clone(),
            ),
        )
        .unwrap();
        let error =
            configured_platform_result(transaction.compute(&key).await.unwrap()).unwrap_err();
        assert!(error.contains(expected), "{target}: {error}");
    }
}

#[tokio::test]
async fn default_host_platform_reaches_bcr_platform_through_exact_builtin_alias() {
    let workspace = scratch();
    let modules = [
        ("rules_license", "1.0.0"),
        ("buildozer", "8.5.1"),
        ("platforms", "1.0.0"),
        ("zlib", "1.3.1.bcr.5"),
        ("bazel_features", "1.42.1"),
        ("protobuf", "33.4"),
        ("rules_java", "9.1.0"),
        ("rules_cc", "0.2.17"),
        ("rules_python", "1.7.0"),
        ("rules_shell", "0.6.1"),
        ("apple_support", "1.24.2"),
        ("rules_apple", "4.1.0"),
        ("rules_swift", "3.1.2"),
        ("abseil-cpp", "20250814.1"),
    ];
    let mut root_module = "module(name = 'root')\n".to_owned();
    for (name, version) in modules {
        fs::create_dir_all(workspace.join(name)).unwrap();
        fs::write(
            workspace.join(name).join("MODULE.bazel"),
            format!("module(name = '{name}', version = '{version}')\n"),
        )
        .unwrap();
        root_module.push_str(&format!(
            "local_path_override(module_name = '{name}', path = '{name}')\n"
        ));
        if matches!(
            name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            root_module.push_str(&format!(
                "bazel_dep(name = '{name}', version = '{version}')\n"
            ));
        }
    }
    fs::write(workspace.join("MODULE.bazel"), root_module).unwrap();
    fs::write(workspace.join("platforms/REPO.bazel"), "").unwrap();
    fs::write(workspace.join("platforms/.bazelignore"), "").unwrap();
    fs::create_dir_all(workspace.join("platforms/host")).unwrap();
    fs::create_dir_all(workspace.join("platforms/cpu")).unwrap();
    fs::write(
        workspace.join("platforms/host/constraints.bzl"),
        "HOST_CONSTRAINTS = ['@platforms//cpu:x86_64']\n",
    )
    .unwrap();
    fs::write(
        workspace.join("platforms/host/BUILD.bazel"),
        "load(':constraints.bzl', 'HOST_CONSTRAINTS')\nplatform(name = 'host', constraint_values = HOST_CONSTRAINTS)\n",
    )
    .unwrap();
    fs::write(
        workspace.join("platforms/cpu/BUILD.bazel"),
        "constraint_setting(name = 'cpu')\nconstraint_value(name = 'x86_64', constraint_setting = ':cpu')\n",
    )
    .unwrap();

    let repositories = [
        ("rules_license+", "rules_license"),
        ("buildozer+", "buildozer"),
        ("platforms", "platforms"),
        ("platforms+", "platforms"),
        ("zlib+", "zlib"),
        ("bazel_features+", "bazel_features"),
        ("protobuf+", "protobuf"),
        ("rules_java+", "rules_java"),
        ("rules_cc+", "rules_cc"),
        ("rules_python+", "rules_python"),
        ("rules_shell+", "rules_shell"),
        ("apple_support+", "apple_support"),
        ("rules_apple+", "rules_apple"),
        ("rules_swift+", "rules_swift"),
        ("abseil-cpp+", "abseil-cpp"),
    ];
    let workspace_key = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let configuration = test_configuration();
    let key = ConfiguredTargetPlatformKey::new(workspace_key, configuration).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    });
    inject_root_target_inputs(
        &mut updater,
        &workspace,
        root_epoch(&workspace),
        &repositories,
    );
    let platform =
        configured_platform_result(updater.commit().await.compute(&key).await.unwrap()).unwrap();
    assert_eq!(
        platform.requested().label().to_string(),
        "@@bazel_tools//tools:host_platform"
    );
    assert_eq!(
        platform.actual().label().to_string(),
        "@@platforms//host:host"
    );
    assert_eq!(platform.constraints().len(), 1);
    assert_eq!(
        platform.constraints()[0]
            .constraint_value()
            .label()
            .to_string(),
        "@@platforms//cpu:x86_64"
    );
}

#[tokio::test]
async fn canonical_external_condition_and_flag_packages_invalidate_and_restore() {
    let workspace = scratch();
    fs::create_dir_all(workspace.join("dep/flags")).unwrap();
    fs::create_dir_all(workspace.join("dep/conditions")).unwrap();
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"bazel_tools\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":root_defs.bzl\", \"root_rule\")\nroot_rule(name = \"root\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("root_defs.bzl"),
        "def _root(ctx): return []\nroot_rule = rule(implementation = _root)\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    )
    .unwrap();
    fs::write(workspace.join("dep/REPO.bazel"), "").unwrap();
    fs::write(workspace.join("dep/.bazelignore"), "").unwrap();
    fs::write(
        workspace.join("dep/flags/defs.bzl"),
        "def _empty(ctx): return []\nsetting = rule(implementation = _empty, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    let flag_source = "load(\":defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"external-default\")\n";
    fs::write(workspace.join("dep/flags/BUILD.bazel"), flag_source).unwrap();
    let condition_source = "config_setting(name = \"match\", flag_values = {\"//flags:mode\": \"external-default\"})\n";
    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source,
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let repositories = [("dep+", "dep")];
    let prepared_external = command_configuration_request_result_with_inputs(
        &dice,
        &workspace,
        "@@//:root",
        vec![CommandConfigurationOccurrence::starlark(
            "@dep//flags:mode",
            Some("command"),
            false,
        )]
        .into(),
        Arc::new(RootActivationTracker::default()),
        &repositories,
    )
    .await
    .unwrap();
    assert_eq!(
        prepared_external
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@dep+//flags:mode").unwrap())
            .unwrap()
            .value()
            .as_str(),
        Some("command")
    );
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let request = |configuration, tracker| {
        configured_condition_request_with_inputs(
            &dice,
            &workspace,
            "@@dep+//conditions:match",
            configuration,
            root_epoch(&workspace),
            &repositories,
            tracker,
        )
    };
    assert_eq!(
        request(test_configuration(), tracker.clone())
            .await
            .unwrap(),
        ConfiguredConditionMatch::Match
    );
    let flag = CanonicalLabel::parse("@@dep+//flags:mode").unwrap();
    let changed = test_configuration().with_starlark_option(StarlarkOption::string(
        flag,
        "changed",
        StarlarkOptionScope::Default,
    ));
    assert_eq!(
        request(changed, Arc::new(RootActivationTracker::default()))
            .await
            .unwrap(),
        ConfiguredConditionMatch::NoMatch
    );
    assert_eq!(
        request(
            test_configuration(),
            Arc::new(RootActivationTracker::default())
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::Match
    );

    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source.replace("external-default", "other"),
    )
    .unwrap();
    assert_eq!(
        request(
            test_configuration(),
            Arc::new(RootActivationTracker::default())
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::NoMatch
    );
    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source,
    )
    .unwrap();
    assert_eq!(
        request(
            test_configuration(),
            Arc::new(RootActivationTracker::default())
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::Match
    );

    fs::write(
        workspace.join("dep/flags/BUILD.bazel"),
        flag_source.replace("external-default", "other"),
    )
    .unwrap();
    assert_eq!(
        request(
            test_configuration(),
            Arc::new(RootActivationTracker::default())
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::NoMatch
    );
    fs::write(workspace.join("dep/flags/BUILD.bazel"), flag_source).unwrap();
    let restored_tracker = Arc::new(RootActivationTracker::default());
    assert_eq!(
        request(test_configuration(), restored_tracker.clone())
            .await
            .unwrap(),
        ConfiguredConditionMatch::Match
    );

    let (activations, _, nodes) = tracker.take();
    for package in ["@@dep+//conditions", "@@dep+//flags"] {
        assert_eq!(
            activations
                .iter()
                .filter(|(identity, _)| {
                    identity.starts_with("package/") && identity.contains(package)
                })
                .count(),
            1,
            "canonical package must activate once: {package}: {activations:#?}"
        );
    }
    assert_eq!(
        activations
            .iter()
            .filter(|(identity, _)| identity.starts_with("condition/"))
            .count(),
        1
    );
    let condition_nodes = nodes
        .iter()
        .filter(|(identity, _, _)| identity.starts_with("condition/"))
        .collect::<Vec<_>>();
    assert_eq!(condition_nodes.len(), 1);
    assert!(
        !condition_nodes[0].2.is_empty(),
        "configured-condition node must retain its observed package frontier"
    );
    let restored_condition_nodes = restored_tracker
        .take()
        .2
        .into_iter()
        .filter(|(identity, _, _)| identity.starts_with("condition/"))
        .collect::<Vec<_>>();
    assert_eq!(restored_condition_nodes.len(), 1);
    assert_eq!(
        condition_nodes[0].1, restored_condition_nodes[0].1,
        "A/B/A restoration must return to the same configured-condition DICE identity"
    );
}

#[tokio::test]
async fn canonical_external_selector_declaration_and_selected_branch_restore_parent_identity() {
    let workspace = scratch();
    for package in ["dep/flags", "dep/conditions", "dep/leaf"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"bazel_tools\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"ParentInfo = provider(fields = {"value": ""})
def _local(ctx): return []
def _parent(ctx): return [ParentInfo(value = str(ctx.attr.dep.label))]
local = rule(implementation = _local)
parent = rule(implementation = _parent, attrs = {"dep": attr.label(mandatory = True)})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "local", "parent")
local(name = "local")
parent(name = "parent", dep = select({
    "@@dep+//conditions:match": "@@dep+//leaf:selected",
    "@@dep+//leaf:preload": "@@dep+//leaf:selected",
    "//conditions:default": ":local",
}))
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    )
    .unwrap();
    fs::write(workspace.join("dep/REPO.bazel"), "").unwrap();
    fs::write(workspace.join("dep/.bazelignore"), "").unwrap();
    fs::write(
        workspace.join("dep/marker.bzl"),
        r#"MarkerInfo = provider(fields = {"value": ""})
def _marker(ctx): return [MarkerInfo(value = ctx.attr.marker)]
marker = rule(implementation = _marker, attrs = {"marker": attr.string(mandatory = True)})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("dep/flags/defs.bzl"),
        "def _empty(ctx): return []\nsetting = rule(implementation = _empty, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    let flag_source = "load(\":defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"selected\")\n";
    fs::write(workspace.join("dep/flags/BUILD.bazel"), flag_source).unwrap();
    let condition_source =
        "config_setting(name = \"match\", flag_values = {\"//flags:mode\": \"selected\"})\n";
    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source,
    )
    .unwrap();
    let leaf_source = "load(\"//:marker.bzl\", \"marker\")\nconfig_setting(name = \"preload\", values = {\"compilation_mode\": \"opt\"})\nmarker(name = \"selected\", marker = \"branch-a\")\n";
    fs::write(workspace.join("dep/leaf/BUILD.bazel"), leaf_source).unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let repositories = [("dep+", "dep")];
    let parent = ProviderId::new("//:defs.bzl", "ParentInfo").unwrap();
    let request = |tracker| {
        root_target_request_with_inputs(
            &dice,
            &workspace,
            "@@//:parent",
            test_configuration(),
            tracker,
            root_epoch(&workspace),
            &repositories,
        )
    };

    let initial_tracker = Arc::new(RootActivationTracker::with_loading());
    let initial = request(initial_tracker.clone()).await.unwrap();
    assert_eq!(provider_value(&initial, &parent), "@@dep+//leaf:selected");
    assert_eq!(
        initial
            .configured_dependencies()
            .map(|key| key.label().to_string())
            .collect::<Vec<_>>(),
        ["@@dep+//leaf:selected"]
    );

    fs::write(
        workspace.join("dep/leaf/BUILD.bazel"),
        leaf_source.replace("branch-a", "branch-b"),
    )
    .unwrap();
    let changed_leaf_tracker = Arc::new(RootActivationTracker::with_loading());
    let changed_leaf = request(changed_leaf_tracker.clone()).await.unwrap();
    assert_eq!(
        provider_value(&changed_leaf, &parent),
        "@@dep+//leaf:selected"
    );
    let changed_leaf_activations = changed_leaf_tracker.take().0;
    assert!(changed_leaf_activations.iter().any(|(identity, kind)| {
        identity == "resolved/@@dep+//leaf:selected=<default>" && *kind == ActivationKind::Evaluated
    }));
    assert!(changed_leaf_activations.iter().any(|(identity, kind)| {
        identity == "resolved/@@//:parent=<default>" && *kind == ActivationKind::Evaluated
    }));
    fs::write(workspace.join("dep/leaf/BUILD.bazel"), leaf_source).unwrap();
    assert_eq!(
        provider_value(
            &request(Arc::new(RootActivationTracker::default()))
                .await
                .unwrap(),
            &parent,
        ),
        "@@dep+//leaf:selected"
    );

    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source.replace("selected", "other"),
    )
    .unwrap();
    assert_eq!(
        provider_value(
            &request(Arc::new(RootActivationTracker::default()))
                .await
                .unwrap(),
            &parent,
        ),
        "@@//:local"
    );
    fs::write(
        workspace.join("dep/conditions/BUILD.bazel"),
        condition_source,
    )
    .unwrap();

    fs::write(
        workspace.join("dep/flags/BUILD.bazel"),
        flag_source.replace("selected", "other"),
    )
    .unwrap();
    assert_eq!(
        provider_value(
            &request(Arc::new(RootActivationTracker::default()))
                .await
                .unwrap(),
            &parent,
        ),
        "@@//:local"
    );
    fs::write(workspace.join("dep/flags/BUILD.bazel"), flag_source).unwrap();
    let restored_tracker = Arc::new(RootActivationTracker::with_loading());
    let restored = request(restored_tracker.clone()).await.unwrap();
    assert_eq!(provider_value(&restored, &parent), "@@dep+//leaf:selected");

    let parent_identity = "resolved/@@//:parent=<default>";
    let node = |tracker: &Arc<RootActivationTracker>| {
        tracker
            .nodes
            .lock()
            .unwrap()
            .iter()
            .find(|(identity, _, _)| identity == parent_identity)
            .map(|(_, node, _)| *node)
            .unwrap()
    };
    assert_eq!(node(&initial_tracker), node(&restored_tracker));
    for package in ["@@dep+//conditions", "@@dep+//flags", "@@dep+//leaf"] {
        assert!(
            initial_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(identity, _)| identity.starts_with("package/") && identity.contains(package)),
            "missing canonical package activation for {package}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_condition_need_error_and_cancellation_recover_without_partial_result() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "def _empty(ctx): return []\nsetting = rule(implementation = _empty, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"config_setting(name = "match", flag_values = {"//settings:mode": "ready"})
config_setting(name = "precedence", values = {"not_a_native_option": "x"}, flag_values = {"//settings:mode": "ready"})
"#,
    )
    .unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let missing =
        configured_condition_request(&dice, &workspace, "@@//:precedence", test_configuration())
            .await
            .unwrap_err();
    assert!(missing.contains("Needs"), "{missing}");

    fs::create_dir(workspace.join("settings")).unwrap();
    fs::write(
        workspace.join("settings/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"ready\")\n",
    )
    .unwrap();
    let semantic =
        configured_condition_request(&dice, &workspace, "@@//:precedence", test_configuration())
            .await
            .unwrap_err();
    assert!(semantic.contains("not_a_native_option"), "{semantic}");
    assert_eq!(
        configured_condition_request(&dice, &workspace, "@@//:match", test_configuration())
            .await
            .unwrap(),
        ConfiguredConditionMatch::Match
    );

    let cancel_dice = Dice::builder().build(DetectCycles::Enabled);
    let (reached_sender, reached_receiver) = std::sync::mpsc::sync_channel(0);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
    let cancel_tracker = Arc::new(RootActivationTracker::with_loading_gate(
        "settings",
        reached_sender,
        release_receiver,
    ));
    let mut updater = cancel_dice.updater_with_data(UserComputationData {
        activation_tracker: Some(cancel_tracker.clone()),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let key = ConfiguredConditionKey::new(
        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:match").unwrap(),
            test_configuration(),
        ),
    )
    .unwrap();
    let mut cancelled = updater.commit().await;
    cancel_tracker.take();
    let computation = tokio::spawn(async move { cancelled.compute(&key).await });
    tokio::task::spawn_blocking(move || reached_receiver.recv().unwrap())
        .await
        .unwrap();
    let (cancelled_activations, _, _) = cancel_tracker.take();
    assert!(
        cancelled_activations.iter().any(|(identity, _)| {
            identity.starts_with("package/") && identity.contains("//settings")
        }),
        "build-setting package dependency was not reached: {cancelled_activations:#?}"
    );
    assert!(
        cancelled_activations
            .iter()
            .all(|(identity, _)| !identity.starts_with("condition/")),
        "configured-condition result published before cancellation: {cancelled_activations:#?}"
    );
    computation.abort();
    release_sender.send(()).unwrap();
    assert!(computation.await.unwrap_err().is_cancelled());

    let recovery_tracker = Arc::new(RootActivationTracker::default());
    assert_eq!(
        configured_condition_request_with_inputs(
            &cancel_dice,
            &workspace,
            "@@//:match",
            test_configuration(),
            root_epoch(&workspace),
            &[],
            recovery_tracker.clone(),
        )
        .await
        .unwrap(),
        ConfiguredConditionMatch::Match
    );
    assert_eq!(
        recovery_tracker
            .take()
            .0
            .iter()
            .filter(|(identity, _)| identity.starts_with("condition/"))
            .count(),
        1,
        "recovery must publish exactly one configured-condition result"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selector_batch_terminal_precedence_and_parent_cancellation_recover_selected_closure() {
    let workspace = scratch();
    for package in ["outer", "semantic_a", "semantic_b", "leaf"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"LeafInfo = provider(fields = {"value": ""})
ParentInfo = provider(fields = {"value": ""})
def _leaf(ctx): return [LeafInfo(value = ctx.label.name)]
def _parent(ctx):
    ctx.actions.write(ctx.outputs.out, ctx.attr.dep[LeafInfo].value)
    return [ParentInfo(value = ctx.attr.dep[LeafInfo].value)]
leaf = rule(implementation = _leaf)
parent = rule(implementation = _parent, attrs = {"dep": attr.label(), "out": attr.output()})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("outer/BUILD.bazel"),
        "config_setting(name = \"match\", values = {\"compilation_mode\": \"fastbuild\"})\n",
    )
    .unwrap();
    for package in ["semantic_a", "semantic_b"] {
        fs::write(
            workspace.join(package).join("BUILD.bazel"),
            format!(
                "config_setting(name = \"match\", values = {{\"not_a_native_option_{package}\": \"x\"}})\n"
            ),
        )
        .unwrap();
    }
    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"leaf\")\nleaf(name = \"selected\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "parent")
parent(
    name = "parent",
    dep = select({
        "//outer:match": "//leaf:selected",
        "//need:match": "//leaf:selected",
        "//semantic_a:match": "//leaf:selected",
        "//semantic_b:match": "//leaf:selected",
    }),
    out = "parent.txt",
)
"#,
    )
    .unwrap();

    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("selector-terminal")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let outer =
        ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(demand.dupe()));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first_tracker = Arc::new(RootActivationTracker::with_loading());
    let mut data = UserComputationData {
        activation_tracker: Some(first_tracker.clone()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = dice.updater_with_data(data);
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let outer_value: <HostPackageInventoryObservationKey as Key>::Value =
        AnalysisPreparationOutcome::Complete(Err(HostPackageInventoryObservationError::Frontier(
            outer.clone(),
        )));
    updater
        .changed_to(vec![(
            HostPackageInventoryObservationKey::new(
                root.clone(),
                PackageIdentifier::new(
                    CanonicalRepoName::root(),
                    PackagePath::parse("outer").unwrap(),
                ),
            ),
            outer_value,
        )])
        .unwrap();
    let key = ConfiguredNodeAnalysisObservationKey::new(
        root.clone(),
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:parent").unwrap(),
            test_configuration(),
        ),
    )
    .unwrap();
    let outcome = updater.commit().await.compute(&key).await.unwrap();
    assert!(
        matches!(&outcome, AnalysisPreparationOutcome::Complete(Err(error)) if error == &outer),
        "{outcome:#?}"
    );
    assert!(first_tracker.take().1.is_empty());

    let need = observed_root_target_request_with_inputs(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//:parent",
        test_configuration(),
        Arc::new(RootActivationTracker::with_loading()),
        root_epoch(&workspace),
        &[],
    )
    .await
    .unwrap_err();
    assert!(need.contains("Needs"), "{need}");

    fs::create_dir(workspace.join("need")).unwrap();
    fs::write(
        workspace.join("need/BUILD.bazel"),
        "config_setting(name = \"match\", values = {\"compilation_mode\": \"fastbuild\"})\n",
    )
    .unwrap();
    let semantic_dice = Dice::builder().build(DetectCycles::Enabled);
    let semantic_tracker = Arc::new(RootActivationTracker::with_loading());
    let semantic = observed_root_target_request_with_inputs(
        &semantic_dice,
        &workspace,
        "@@//:parent",
        test_configuration(),
        semantic_tracker.clone(),
        root_epoch(&workspace),
        &[],
    )
    .await
    .unwrap_err();
    assert!(
        semantic.contains("not_a_native_option_semantic_a"),
        "{semantic}"
    );
    let (activations, batches, _) = semantic_tracker.take();
    assert!(batches.iter().all(|(_, batch)| batch.events().is_empty()));
    for package in ["//semantic_a", "//semantic_b"] {
        assert!(activations.iter().any(|(identity, _)| {
            identity.starts_with("package/") && identity.contains(package)
        }));
    }
    assert!(
        activations
            .iter()
            .all(|(identity, _)| !identity.contains("//leaf:selected")),
        "unresolved selector published its branch child: {activations:#?}"
    );

    for package in ["semantic_a", "semantic_b"] {
        fs::write(
            workspace.join(package).join("BUILD.bazel"),
            "config_setting(name = \"match\", values = {\"compilation_mode\": \"fastbuild\"})\n",
        )
        .unwrap();
    }
    let recovery_tracker = Arc::new(RootActivationTracker::with_loading());
    let result = observed_root_target_request_with_inputs(
        &semantic_dice,
        &workspace,
        "@@//:parent",
        test_configuration(),
        recovery_tracker.clone(),
        root_epoch(&workspace),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        result
            .configured_dependencies()
            .map(|key| key.label().to_string())
            .collect::<Vec<_>>(),
        ["@@//leaf:selected"]
    );
    assert_eq!(result.actions().len(), 1);
    assert_eq!(
        provider_value(
            &result,
            &ProviderId::new("//:defs.bzl", "ParentInfo").unwrap()
        ),
        "selected"
    );
    assert_eq!(
        recovery_tracker
            .take()
            .0
            .iter()
            .filter(|(identity, kind)| {
                identity == "observed/resolved/@@//:parent=<default>"
                    && *kind == ActivationKind::Evaluated
            })
            .count(),
        1
    );

    let cancel_dice = Dice::builder().build(DetectCycles::Enabled);
    let (reached_sender, reached_receiver) = std::sync::mpsc::sync_channel(0);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
    let cancel_tracker = Arc::new(RootActivationTracker::with_loading_gate(
        "semantic_a",
        reached_sender,
        release_receiver,
    ));
    let mut data = UserComputationData {
        activation_tracker: Some(cancel_tracker.clone()),
        ..Default::default()
    };
    data.data.set(CaptureEvaluationEvents);
    let mut updater = cancel_dice.updater_with_data(data);
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let key = ConfiguredNodeAnalysisObservationKey::new(
        root,
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:parent").unwrap(),
            test_configuration(),
        ),
    )
    .unwrap();
    let mut cancelled = updater.commit().await;
    cancel_tracker.take();
    let computation = tokio::spawn(async move { cancelled.compute(&key).await });
    tokio::task::spawn_blocking(move || reached_receiver.recv().unwrap())
        .await
        .unwrap();
    let (activations, batches, _) = cancel_tracker.take();
    assert!(batches.is_empty());
    assert!(
        activations.iter().all(|(identity, _)| {
            !identity.starts_with("resolved/") && !identity.starts_with("observed/resolved/")
        }),
        "cancelled selector batch published analysis: {activations:#?}"
    );
    computation.abort();
    release_sender.send(()).unwrap();
    assert!(computation.await.unwrap_err().is_cancelled());

    let recovered_tracker = Arc::new(RootActivationTracker::with_loading());
    let recovered = observed_root_target_request_with_inputs(
        &cancel_dice,
        &workspace,
        "@@//:parent",
        test_configuration(),
        recovered_tracker.clone(),
        root_epoch(&workspace),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(recovered.actions().len(), 1);
    assert_eq!(recovered.configured_dependencies().count(), 1);
    let activations = recovered_tracker.take().0;
    assert_eq!(
        activations
            .iter()
            .filter(|(identity, kind)| {
                identity == "observed/resolved/@@//:parent=<default>"
                    && *kind == ActivationKind::Evaluated
            })
            .count(),
        1
    );
    assert_eq!(
        activations
            .iter()
            .filter(|(identity, kind)| {
                identity == "observed/resolved/@@//leaf:selected=<default>"
                    && *kind == ActivationKind::Evaluated
            })
            .count(),
        1
    );
}

#[tokio::test]
async fn typed_transitions_replace_one_row_elide_defaults_and_normalize_sets() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _empty(ctx): return []
empty = rule(implementation = _empty)
integer = rule(implementation = _empty, attrs = {"scope": attr.string()}, build_setting = config.int(flag = True))
boolean = rule(implementation = _empty, attrs = {"scope": attr.string()}, build_setting = config.bool(flag = True))
string = rule(implementation = _empty, build_setting = config.string(flag = True))
multi = rule(implementation = _empty, build_setting = config.string(flag = True, allow_multiple = True))
string_list = rule(implementation = _empty, build_setting = config.string_list(flag = True))
string_set = rule(implementation = _empty, build_setting = config.string_set(flag = True))
def _int(settings, attr): return {"//:integer": 7}
def _bool(settings, attr): return {"//:boolean": True}
def _string(settings, attr): return {"//:string": "changed"}
def _multi(settings, attr): return {"//:multi": ["x", "x"]}
def _list_a(settings, attr): return {"//:list": ["a", "b", "a"]}
def _list_b(settings, attr): return {"//:list": ["b", "a", "a"]}
def _set_list(settings, attr): return {"//:set": ["b", "a", "b"]}
def _set_set(settings, attr): return {"//:set": set(["a", "b"])}
t_int = transition(implementation = _int, inputs = [], outputs = ["//:integer"])
t_bool = transition(implementation = _bool, inputs = [], outputs = ["//:boolean"])
t_string = transition(implementation = _string, inputs = [], outputs = ["//:string"])
t_multi = transition(implementation = _multi, inputs = [], outputs = ["//:multi"])
t_list_a = transition(implementation = _list_a, inputs = [], outputs = ["//:list"])
t_list_b = transition(implementation = _list_b, inputs = [], outputs = ["//:list"])
t_set_list = transition(implementation = _set_list, inputs = [], outputs = ["//:set"])
t_set_set = transition(implementation = _set_set, inputs = [], outputs = ["//:set"])
parent = rule(implementation = _empty, attrs = {
    "int_dep": attr.label(cfg = t_int),
    "bool_dep": attr.label(cfg = t_bool),
    "string_dep": attr.label(cfg = t_string),
    "multi_dep": attr.label(cfg = t_multi),
    "list_a_dep": attr.label(cfg = t_list_a),
    "list_b_dep": attr.label(cfg = t_list_b),
    "set_list_dep": attr.label(cfg = t_set_list),
    "set_set_dep": attr.label(cfg = t_set_set),
})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "boolean", "empty", "integer", "multi", "parent", "string", "string_list", "string_set")
integer(name = "integer", build_setting_default = 7, scope = "target")
boolean(name = "boolean", build_setting_default = False, scope = "universal")
string(name = "string", build_setting_default = "default")
multi(name = "multi", build_setting_default = "one")
string_list(name = "list", build_setting_default = ["default"])
string_set(name = "set", build_setting_default = set(["default"]))
empty(name = "c_int")
empty(name = "c_bool")
empty(name = "c_string")
empty(name = "c_multi")
empty(name = "c_list_a")
empty(name = "c_list_b")
empty(name = "c_set_list")
empty(name = "c_set_set")
parent(
    name = "parent",
    int_dep = ":c_int",
    bool_dep = ":c_bool",
    string_dep = ":c_string",
    multi_dep = ":c_multi",
    list_a_dep = ":c_list_a",
    list_b_dep = ":c_list_b",
    set_list_dep = ":c_set_list",
    set_set_dep = ":c_set_set",
)
"#,
    )
    .unwrap();

    let integer = CanonicalLabel::parse("@@//:integer").unwrap();
    let unrelated = CanonicalLabel::parse("@@//:unrelated").unwrap();
    let base = test_configuration()
        .with_starlark_option(StarlarkOption::new(
            integer.clone(),
            StarlarkOptionValue::Integer(BigInt::from(8)),
            StarlarkOptionScope::Target,
        ))
        .with_starlark_option(StarlarkOption::string(
            unrelated.clone(),
            "kept",
            StarlarkOptionScope::Universal,
        ));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(CanonicalLabel::parse("@@//:parent").unwrap(), base),
        None,
        false,
    )
    .await
    .unwrap();
    let child = |name: &str| {
        result
            .configured_dependencies()
            .find(|key| key.label().target().as_str() == name)
            .unwrap()
            .configuration()
    };
    assert!(child("c_int").starlark_option(&integer).is_none());
    for name in [
        "c_int",
        "c_bool",
        "c_string",
        "c_multi",
        "c_list_a",
        "c_list_b",
        "c_set_list",
        "c_set_set",
    ] {
        assert_eq!(
            child(name)
                .starlark_option(&unrelated)
                .unwrap()
                .value()
                .as_str(),
            Some("kept")
        );
    }
    let value = |child_name: &str, setting: &str| {
        child(child_name)
            .starlark_option(&CanonicalLabel::parse(setting).unwrap())
            .unwrap()
            .value()
            .clone()
    };
    assert_eq!(
        value("c_bool", "@@//:boolean"),
        StarlarkOptionValue::Boolean(true)
    );
    assert_eq!(
        child("c_bool")
            .starlark_option(&CanonicalLabel::parse("@@//:boolean").unwrap())
            .unwrap()
            .scope(),
        StarlarkOptionScope::Universal
    );
    assert_eq!(
        value("c_string", "@@//:string"),
        StarlarkOptionValue::string("changed")
    );
    assert_eq!(
        value("c_multi", "@@//:multi"),
        StarlarkOptionValue::string_list(["x", "x"])
    );
    assert_eq!(
        value("c_list_a", "@@//:list"),
        StarlarkOptionValue::string_list(["a", "b", "a"])
    );
    assert_ne!(child("c_list_a"), child("c_list_b"));
    assert_eq!(child("c_set_list"), child("c_set_set"));
    assert_eq!(
        value("c_set_list", "@@//:set"),
        StarlarkOptionValue::string_set(["a", "b"])
    );
}

#[tokio::test]
async fn mixed_root_and_external_overlay_demands_root_package_before_mapping_need() {
    let workspace = scratch();
    fs::write(
        workspace.join("MODULE.bazel"),
        "module(name = \"bazel_tools\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    )
    .unwrap();
    fs::create_dir(workspace.join("dep")).unwrap();
    fs::write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    )
    .unwrap();
    fs::create_dir(workspace.join("unrelated")).unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "def _setting(ctx): return []\nsetting = rule(implementation = _setting, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"setting\")\nsetting(name = \"root_mode\", build_setting_default = \"default\")\n",
    )
    .unwrap();

    let tracker = Arc::new(RootActivationTracker::with_loading());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let error = command_configuration_request_result_with_inputs(
        &dice,
        &workspace,
        "@@//:root_mode",
        vec![
            CommandConfigurationOccurrence::starlark("//:root_mode", Some("root"), false),
            CommandConfigurationOccurrence::extra_toolchains("//toolchain:root"),
            CommandConfigurationOccurrence::starlark("@dep//flags:mode", Some("external"), false),
        ]
        .into(),
        tracker.clone(),
        &[("unrelated+", "unrelated")],
    )
    .await
    .unwrap_err();
    assert!(error.contains("Needs"), "{error}");
    let (activations, _, _) = tracker.take();
    assert!(
        activations
            .iter()
            .any(|(identity, _)| identity.starts_with("package/") && identity.contains("@@//")),
        "root declaration package was not demanded before mapping Need: {activations:#?}"
    );
    assert!(
        activations
            .iter()
            .any(|(identity, _)| identity == "command-configuration/observed"),
        "the invalid Need result must still record its completed driver activation: {activations:#?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_configuration_cancellation_publishes_no_partial_result_and_recovers() {
    let workspace = scratch();
    fs::create_dir(workspace.join("settings")).unwrap();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "def _setting(ctx): return []\nsetting = rule(implementation = _setting, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    fs::write(workspace.join("BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("settings/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"default\")\n",
    )
    .unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let (reached_sender, reached_receiver) = std::sync::mpsc::sync_channel(0);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
    let tracker = Arc::new(RootActivationTracker::with_loading_gate(
        "settings",
        reached_sender,
        release_receiver,
    ));
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let preparation = CommandConfigurationPreparationKey::new(
        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
        test_configuration(),
        vec![CommandConfigurationOccurrence::starlark(
            "//settings:mode",
            Some("command"),
            false,
        )]
        .into(),
    )
    .unwrap();
    let key = CommandConfigurationPreparationObservationKey::new(preparation);
    let mut transaction = updater.commit().await;
    tracker.take();
    let computation = tokio::spawn(async move { transaction.compute(&key).await });
    tokio::task::spawn_blocking(move || reached_receiver.recv().unwrap())
        .await
        .unwrap();
    let (activations, _, _) = tracker.take();
    assert!(
        activations
            .iter()
            .any(|(identity, _)| identity.starts_with("package/") && identity.contains("settings")),
        "declaration package was not reached before cancellation: {activations:#?}"
    );
    assert!(
        activations
            .iter()
            .all(|(identity, _)| identity != "command-configuration/observed"),
        "command configuration published before cancellation: {activations:#?}"
    );
    computation.abort();
    release_sender.send(()).unwrap();
    assert!(computation.await.unwrap_err().is_cancelled());

    let recovery_tracker = Arc::new(RootActivationTracker::with_loading());
    let recovered = command_configuration_request_result(
        &dice,
        &workspace,
        "@@//settings:mode",
        vec![CommandConfigurationOccurrence::starlark(
            "//settings:mode",
            Some("command"),
            false,
        )]
        .into(),
        recovery_tracker.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        recovered
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//settings:mode").unwrap())
            .unwrap()
            .value()
            .as_str(),
        Some("command")
    );
    assert_eq!(
        recovery_tracker
            .take()
            .0
            .iter()
            .filter(|(identity, _)| *identity == "command-configuration/observed")
            .count(),
        1
    );
}

#[tokio::test]
async fn explicit_external_setting_uses_its_canonical_package_declaration() {
    let workspace = scratch();
    fs::create_dir_all(workspace.join("dep/flags")).unwrap();
    fs::create_dir_all(workspace.join("other/flags")).unwrap();
    let module_a = "module(name = \"bazel_tools\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n";
    let module_b = "module(name = \"bazel_tools\")\nbazel_dep(name = \"other\", version = \"1.0.0\", repo_name = \"dep\")\nlocal_path_override(module_name = \"other\", path = \"other\")\n";
    fs::write(workspace.join("MODULE.bazel"), module_a).unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        "def _empty(ctx): return []\nempty = rule(implementation = _empty)\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"empty\")\nempty(name = \"request\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    )
    .unwrap();
    fs::write(workspace.join("dep/REPO.bazel"), "").unwrap();
    fs::write(workspace.join("dep/.bazelignore"), "").unwrap();
    fs::write(
        workspace.join("dep/flags/defs.bzl"),
        "def _setting(ctx): return []\nsetting = rule(implementation = _setting, attrs = {\"scope\": attr.string()}, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    fs::write(
        workspace.join("dep/flags/BUILD.bazel"),
        "load(\":defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"external-default\", scope = \"target\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("other/MODULE.bazel"),
        "module(name = \"other\", version = \"1.0.0\")\n",
    )
    .unwrap();
    fs::write(workspace.join("other/REPO.bazel"), "").unwrap();
    fs::write(workspace.join("other/.bazelignore"), "").unwrap();
    fs::write(
        workspace.join("other/flags/defs.bzl"),
        "def _setting(ctx): return []\nsetting = rule(implementation = _setting, attrs = {\"scope\": attr.string()}, build_setting = config.string(flag = True))\n",
    )
    .unwrap();
    fs::write(
        workspace.join("other/flags/BUILD.bazel"),
        "load(\":defs.bzl\", \"setting\")\nsetting(name = \"mode\", build_setting_default = \"other-default\", scope = \"target\")\n",
    )
    .unwrap();

    let repositories = [("dep+", "dep"), ("other+", "other")];
    let label = CanonicalLabel::parse("@@dep+//flags:mode").unwrap();
    let tracker = Arc::new(RootActivationTracker::with_loading());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let request = |value: &'static str, tracker| {
        command_configuration_request_result_with_inputs(
            &dice,
            &workspace,
            "@@//:request",
            vec![CommandConfigurationOccurrence::starlark(
                "@dep//flags:mode",
                Some(value),
                false,
            )]
            .into(),
            tracker,
            &repositories,
        )
    };
    let default = request("external-default", tracker.clone()).await.unwrap();
    assert!(
        default
            .configured_target_key()
            .unwrap()
            .configuration()
            .starlark_option(&label)
            .is_none()
    );
    let changed = request(
        "external-changed",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    let retained = changed
        .configured_target_key()
        .unwrap()
        .configuration()
        .starlark_option(&label)
        .unwrap();
    assert_eq!(retained.label(), &label);
    assert_eq!(retained.value().as_str(), Some("external-changed"));
    assert_eq!(retained.scope(), StarlarkOptionScope::Target);
    let configuration_a = changed
        .configured_target_key()
        .unwrap()
        .configuration()
        .clone();

    fs::write(workspace.join("MODULE.bazel"), module_b).unwrap();
    let mapped_b = request(
        "external-changed",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    let configuration_b = mapped_b.configured_target_key().unwrap().configuration();
    assert!(
        configuration_b
            .starlark_option(&CanonicalLabel::parse("@@dep+//flags:mode").unwrap())
            .is_none()
    );
    assert_eq!(
        configuration_b
            .starlark_option(&CanonicalLabel::parse("@@other+//flags:mode").unwrap())
            .unwrap()
            .value()
            .as_str(),
        Some("external-changed")
    );
    assert_ne!(&configuration_a, configuration_b);

    fs::write(workspace.join("MODULE.bazel"), module_a).unwrap();
    let restored = request(
        "external-changed",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    assert_eq!(
        &configuration_a,
        restored.configured_target_key().unwrap().configuration()
    );

    let (activations, _, _) = tracker.take();
    assert!(
        activations.iter().any(|(identity, _)| {
            identity.starts_with("package/") && identity.contains("@@dep+//flags")
        }),
        "canonical external declaration package must be an observable preparation dependency: {activations:#?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transition_declaration_need_errors_and_cancellation_publish_no_child_and_recover() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let definitions = r#"def _empty(ctx): return []
empty = rule(implementation = _empty)
string_list = rule(implementation = _empty, build_setting = config.string_list(flag = True))
def _transition(settings, attr): return {"//settings:mode": ["a", 1]}
t = transition(implementation = _transition, inputs = [], outputs = ["//settings:mode"])
parent = rule(implementation = _empty, attrs = {"dep": attr.label(cfg = t)})
"#;
    fs::write(workspace.join("defs.bzl"), definitions).unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"empty\", \"parent\")\nempty(name = \"child\")\nparent(name = \"parent\", dep = \":child\")\n",
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let request = |tracker| root_target_request(&dice, &workspace, "@@//:parent", tracker);

    let need_tracker = Arc::new(RootActivationTracker::default());
    let need = request(need_tracker.clone()).await.unwrap_err();
    assert!(need.contains("Needs"), "{need}");
    assert!(
        need_tracker
            .take()
            .0
            .iter()
            .all(|(identity, _)| !identity.contains("@@//:child"))
    );

    fs::create_dir(workspace.join("settings")).unwrap();
    fs::write(
        workspace.join("settings/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"empty\")\nempty(name = \"mode\")\n",
    )
    .unwrap();
    let target_tracker = Arc::new(RootActivationTracker::default());
    let target_error = request(target_tracker.clone()).await.unwrap_err();
    assert!(
        target_error.contains("target @@//settings:mode is not a Starlark build setting"),
        "{target_error}"
    );
    assert!(
        target_tracker
            .take()
            .0
            .iter()
            .all(|(identity, _)| !identity.contains("@@//:child"))
    );

    fs::write(
        workspace.join("settings/BUILD.bazel"),
        "load(\"//:defs.bzl\", \"string_list\")\nstring_list(name = \"mode\", build_setting_default = [\"default\"])\n",
    )
    .unwrap();
    let value_tracker = Arc::new(RootActivationTracker::default());
    let value_error = request(value_tracker.clone()).await.unwrap_err();
    assert!(
        value_error.contains("collection members must be strings"),
        "{value_error}"
    );
    assert!(
        value_tracker
            .take()
            .0
            .iter()
            .all(|(identity, _)| !identity.contains("@@//:child"))
    );

    fs::write(
        workspace.join("defs.bzl"),
        definitions.replace("[\"a\", 1]", "[\"a\", \"b\"]"),
    )
    .unwrap();
    let cancel_dice = Dice::builder().build(DetectCycles::Enabled);
    let (reached_sender, reached_receiver) = std::sync::mpsc::sync_channel(0);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
    let cancel_tracker = Arc::new(RootActivationTracker::with_loading_gate(
        "settings",
        reached_sender,
        release_receiver,
    ));
    let mut updater = cancel_dice.updater_with_data(UserComputationData {
        activation_tracker: Some(cancel_tracker.clone()),
        ..Default::default()
    });
    inject_root_target_inputs(&mut updater, &workspace, root_epoch(&workspace), &[]);
    let mut cancelled = updater.commit().await;
    let key = prepared_analysis_key(
        &mut cancelled,
        NormalizedAbsolutePath::new(workspace.clone()).unwrap(),
        CanonicalLabel::parse("@@//:parent").unwrap(),
        test_configuration(),
        None,
    )
    .await
    .unwrap();
    cancel_tracker.take();
    let computation = tokio::spawn(async move { cancelled.compute(&key).await });
    tokio::task::spawn_blocking(move || reached_receiver.recv().unwrap())
        .await
        .unwrap();
    let (cancelled_activations, _, _) = cancel_tracker.take();
    assert!(
        cancelled_activations.iter().any(|(identity, _)| {
            identity.starts_with("package/") && identity.contains("settings")
        }),
        "declaration package dependency was not observed: {cancelled_activations:#?}"
    );
    assert!(
        cancelled_activations
            .iter()
            .all(|(identity, _)| !identity.contains("@@//:child")),
        "transitioned child published before cancellation: {cancelled_activations:#?}"
    );
    computation.abort();
    release_sender.send(()).unwrap();
    assert!(computation.await.unwrap_err().is_cancelled());
    let recovery_tracker = Arc::new(RootActivationTracker::default());
    let recovered = root_target_request(
        &cancel_dice,
        &workspace,
        "@@//:parent",
        recovery_tracker.clone(),
    )
    .await
    .unwrap();
    let dependencies = recovered.configured_dependencies().collect::<Vec<_>>();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].label().to_string(), "@@//:child");
    assert_eq!(
        dependencies[0]
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//settings:mode").unwrap())
            .unwrap()
            .value(),
        &StarlarkOptionValue::string_list(["a", "b"])
    );
    assert_eq!(
        recovery_tracker
            .take()
            .0
            .iter()
            .filter(|(identity, _)| identity.contains("@@//:child"))
            .count(),
        1,
        "recovery must publish exactly one resolved child"
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
    let with_unrelated = analyze_request(
        &dice,
        &workspace,
        &ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:parent").unwrap(),
            test_configuration().with_starlark_option(root_string_option("unrelated")),
        ),
        None,
        false,
    )
    .await
    .unwrap();
    let parent = ProviderId::new("//:defs.bzl", "ParentInfo").unwrap();
    assert_eq!(provider_value(&with_unrelated, &parent), "transitioned");
    assert_eq!(
        root_setting_value(with_unrelated.configured_target_key().unwrap()),
        Some("unrelated")
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
    assert_eq!(provider_value(&result, &parent), "transitioned");
    let transitioned = result
        .configured_dependencies()
        .next()
        .expect("parent retains its transitioned child");
    assert_eq!(
        transitioned
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//settings:settings").unwrap())
            .unwrap()
            .label(),
        &CanonicalLabel::parse("@@//settings:settings").unwrap()
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
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.write(out, \"hello from an action\\n\", is_executable = True)\n    return [DefaultInfo(files = depset([out]))]\n\nwrite_file = rule(implementation = _impl)\n",
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
            is_executable: true,
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
async fn configured_attributes_select_specialize_allocate_dicts_and_predeclare_outputs() {
    let workspace = scratch();
    for package in ["rules", "leaf", "parent"] {
        fs::create_dir_all(workspace.join(package)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(workspace.join("rules/BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("rules/defs.bzl"),
        r#"LeafInfo = provider(fields = {"value": ""})
ParentInfo = provider(fields = {"value": ""})
def _leaf(ctx):
    return [LeafInfo(value = ctx.label.name)]

def _parent(ctx):
    value = "|".join([
        ctx.attr.text,
        ctx.attr.words[1],
        ctx.attr.dep[LeafInfo].value,
        ctx.attr.mapped["key"][LeafInfo].value,
        ctx.attr.reverse[ctx.attr.dep],
        str(ctx.attr.dep in ctx.attr.reverse),
        ctx.attr.grouped["group"][0][LeafInfo].value,
        ctx.outputs.out.path,
        ctx.outputs.outs[0].path,
    ])
    ctx.actions.write(ctx.outputs.out, value)
    return [ParentInfo(value = value), DefaultInfo(files = depset([ctx.outputs.out]))]

leaf = rule(implementation = _leaf)
parent = rule(
    implementation = _parent,
    attrs = {
        "text": attr.string(mandatory = True),
        "words": attr.string_list(mandatory = True),
        "dep": attr.label(mandatory = True),
        "mapped": attr.string_keyed_label_dict(mandatory = True),
        "reverse": attr.label_keyed_string_dict(mandatory = True),
        "grouped": attr.label_list_dict(mandatory = True),
        "out": attr.output(mandatory = True),
        "outs": attr.output_list(mandatory = True),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(\"//rules:defs.bzl\", \"leaf\")\nleaf(name = \"selected\")\nleaf(name = \"mapped\")\nleaf(name = \"grouped\")\n",
    )
    .unwrap();
    fs::write(
        workspace.join("parent/BUILD.bazel"),
        r#"load("//rules:defs.bzl", "parent")
config_setting(name = "base", values = {"compilation_mode": "fastbuild"})
config_setting(name = "specific", values = {"compilation_mode": "fastbuild", "stamp": "false"})
config_setting(name = "equal_a", values = {"compilation_mode": "fastbuild"})
config_setting(name = "equal_b", values = {"stamp": "false"})
parent(
    name = "parent",
    text = select({":base": "base", ":specific": "specific"}),
    words = select({":equal_a": select({":base": ["same"]}), ":equal_b": ["same"]}) + ["tail"],
    dep = select({":base": "//leaf:selected", "//conditions:default": "//leaf:unselected_missing"}),
    mapped = {"key": "//leaf:mapped"},
    reverse = {"//leaf:selected": "reverse"},
    grouped = {"group": ["//leaf:grouped"]},
    out = "result.txt",
    outs = ["extra.txt"],
)
"#,
    )
    .unwrap();

    let result = root_target_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &workspace,
        "@@//parent:parent",
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .unwrap();
    let parent_id = ProviderId::new("//rules:defs.bzl", "ParentInfo").unwrap();
    assert_eq!(
        result.providers().user(&parent_id).unwrap().field("value"),
        Some(
            "specific|tail|selected|mapped|reverse|True|grouped|parent/result.txt|parent/extra.txt"
        )
    );
    assert_eq!(
        result
            .configured_dependencies()
            .map(|key| key.label().to_string())
            .collect::<Vec<_>>(),
        [
            "@@//leaf:selected",
            "@@//leaf:mapped",
            "@@//leaf:selected",
            "@@//leaf:grouped",
        ]
    );
    assert_eq!(result.declared_outputs(), ["parent/result.txt"]);
}

#[tokio::test]
async fn configured_attribute_no_match_and_ambiguity_fail_before_rule_evaluation() {
    for (name, conditions, selection, expected) in [
        (
            "no_default",
            "config_setting(name = \"opt\", values = {\"compilation_mode\": \"opt\"})",
            "select({\":opt\": \"value\"})",
            "no matching condition and no default",
        ),
        (
            "ambiguous",
            "config_setting(name = \"mode\", values = {\"compilation_mode\": \"fastbuild\"})\nconfig_setting(name = \"stamp\", values = {\"stamp\": \"false\"})",
            "select({\":mode\": \"one\", \":stamp\": \"two\"})",
            "ambiguous matching conditions",
        ),
    ] {
        let workspace = scratch();
        fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        fs::write(
            workspace.join("defs.bzl"),
            "def _impl(ctx):\n    fail(\"RULE_EVALUATED\")\nprobe = rule(implementation = _impl, attrs = {\"value\": attr.string()})\n",
        )
        .unwrap();
        fs::write(
            workspace.join("BUILD.bazel"),
            format!(
                "load(\":defs.bzl\", \"probe\")\n{conditions}\nprobe(name = \"probe\", value = {selection})\n"
            ),
        )
        .unwrap();
        let error = root_target_request(
            &Dice::builder().build(DetectCycles::Enabled),
            &workspace,
            "@@//:probe",
            Arc::new(RootActivationTracker::default()),
        )
        .await
        .unwrap_err();
        assert!(error.contains(expected), "{name}: {error}");
        assert!(!error.contains("RULE_EVALUATED"), "{name}: {error}");
    }
}

#[tokio::test]
async fn selected_configurable_transition_ignores_unselected_branch_and_authenticates_output() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join("defs.bzl"),
        r#"def _empty(ctx): return []
setting = rule(implementation = _empty, build_setting = config.string(flag = True))
leaf = rule(implementation = _empty)
def _transition(settings, attr): return {"//:setting": "changed"}
configured = transition(implementation = _transition, inputs = [], outputs = ["//:setting"])
parent = rule(implementation = _empty, attrs = {"dep": attr.label(cfg = configured)})
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        r#"load(":defs.bzl", "leaf", "parent", "setting")
setting(name = "setting", build_setting_default = "default")
leaf(name = "selected")
config_setting(name = "choose", values = {"compilation_mode": "fastbuild"})
parent(name = "parent", dep = select({":choose": ":selected", "//conditions:default": ":unselected_missing"}))
"#,
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
    let dependency = result.configured_dependencies().next().unwrap();
    assert_eq!(dependency.label().to_string(), "@@//:selected");
    assert_eq!(
        dependency
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//:setting").unwrap())
            .and_then(|option| option.value().as_str()),
        Some("changed")
    );
    assert_eq!(result.configured_dependencies().count(), 1);
    assert!(matches!(
        result.edges()[0].kind(),
        slug_analysis_v2::ConfiguredEdgeKind::TransitionedAttribute {
            attribute,
            index: 0,
            output,
        } if attribute == "dep" && output == &CanonicalLabel::parse("@@//:setting").unwrap()
    ));
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
