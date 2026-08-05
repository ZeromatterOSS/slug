/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_analysis_v2::AnalysisPreparationOutcome;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::RootConfiguredTargetAnalysisKey;
use slug_analysis_v2::key::RootStringSettingValue;
use slug_build_api_v2::ActionKind;
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
    events: Mutex<Vec<(String, EventKind)>>,
}

impl AnalysisTracker {
    fn take(&self) -> Vec<(String, EventKind)> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl ActivationTracker for AnalysisTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        activation_data: ActivationData,
    ) {
        let Some(key) = key.downcast_ref::<ConfiguredTargetAnalysisKey>() else {
            return;
        };
        let kind = match activation_data {
            ActivationData::Evaluated(_) => EventKind::Evaluated,
            ActivationData::Reused => EventKind::Reused,
        };
        self.events
            .lock()
            .unwrap()
            .push((key.configured_target.label().to_string(), kind));
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
        let Some(key) = key.downcast_ref::<ConfiguredTargetAnalysisKey>() else {
            return;
        };
        self.activations
            .lock()
            .unwrap()
            .push(AnalysisEventActivation {
                workspace: key.workspace.clone(),
                configured_target: key.configured_target.clone(),
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
    let mut entries = SmallMap::new();
    for (path, value) in workspace_snapshot(root).files.iter() {
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
        entries.insert(
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(root.join(name)).unwrap(),
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
    legacy: AtomicUsize,
}

impl RootActivationTracker {
    fn take(&self) -> (Vec<(String, ActivationKind)>, usize) {
        (
            std::mem::take(&mut *self.activations.lock().unwrap()),
            self.legacy.load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

fn root_activation_identity(key: &RootConfiguredTargetAnalysisKey) -> String {
    if let Some(configured_target) = key.resolved_configured_target() {
        return format!(
            "resolved/{}={}",
            configured_target.label(),
            configured_target
                .configuration()
                .root_string_setting()
                .unwrap()
                .as_str()
        );
    }
    let (requested, explicit) = key.root_string_setting_request_parts().unwrap();
    format!(
        "request/{requested}={}",
        explicit.map_or("<default>", RootStringSettingValue::as_str)
    )
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
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        if key.downcast_ref::<ConfiguredTargetAnalysisKey>().is_some() {
            self.legacy
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        if key.downcast_ref::<ConfiguredTargetAnalysisKey>().is_some() {
            self.legacy
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(root_key) = key.downcast_ref::<RootConfiguredTargetAnalysisKey>() {
            self.activations
                .lock()
                .unwrap()
                .push((root_activation_identity(root_key), activation.kind()));
        }
    }
}

async fn root_string_request_result(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    target: &str,
    explicit: Option<&str>,
    tracker: Arc<RootActivationTracker>,
) -> Result<AnalysisResult, String> {
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
    let outcome = transaction
        .compute(
            &RootConfiguredTargetAnalysisKey::root_string_setting_request(
                NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap(),
                CanonicalLabel::parse(target).unwrap(),
                explicit.map(RootStringSettingValue::new),
            ),
        )
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
) -> AnalysisResult {
    root_string_request_result(dice, workspace, target, explicit, tracker)
        .await
        .unwrap()
}

fn provider_value(result: &AnalysisResult, provider: &ProviderId) -> String {
    result
        .providers()
        .user(provider)
        .unwrap()
        .field("value")
        .unwrap()
        .to_owned()
}

fn root_setting_value(key: &ConfiguredTargetKey) -> &str {
    key.configuration().root_string_setting().unwrap().as_str()
}

#[tokio::test]
async fn root_string_setting_requests_preserve_lifecycle_transition_and_identity() {
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
def _left_transition(settings, attr): return {"//:setting": "left"}
def _right_transition(settings, attr): return {"//:setting": "right"}
left_transition = transition(implementation = _left_transition, inputs = [], outputs = ["//:setting"])
right_transition = transition(implementation = _right_transition, inputs = [], outputs = ["//:setting"])
def _parent(ctx): return [ParentInfo(value = ctx.attr.left[0][ConsumerInfo].value + "," + ctx.attr.right[0][ConsumerInfo].value)]
parent = rule(implementation = _parent, attrs = {"left": attr.label(cfg = left_transition), "right": attr.label(cfg = right_transition)})
"#;
    let build_source = "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", left = \":consumer\", right = \":consumer\")\n";
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    fs::write(&defs, defs_source).unwrap();
    fs::write(&build, build_source).unwrap();

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootActivationTracker::default());
    let consumer = ProviderId::new("//:defs.bzl", "ConsumerInfo").unwrap();
    let parent = ProviderId::new("//:defs.bzl", "ParentInfo").unwrap();
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
    let parent_deps = original_parent.direct_dependencies();
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
    let edited_deps = edited_parent.direct_dependencies();
    assert_ne!(edited_deps[0], parent_deps[0]);
    assert_eq!(edited_deps[1], parent_deps[1]);

    fs::write(&defs, defs_source).unwrap();
    let restored_parent =
        root_string_request(&dice, &workspace, "@@//:parent", None, tracker.clone()).await;
    assert_eq!(provider_value(&restored_parent, &parent), "left,right");
    assert_eq!(original_parent_key, *restored_parent.key());
    assert_eq!(parent_deps, restored_parent.direct_dependencies());

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

    let (activations, legacy) = tracker.take();
    assert_eq!(legacy, 0, "legacy analysis key activated: {activations:#?}");
    assert_eq!(
        activation_codes(&activations),
        r#"request/@@//:consumer=<default>:E request/@@//:consumer=<default>:E request/@@//:consumer=<default>:E request/@@//:consumer=<default>:R
request/@@//:consumer=command:E request/@@//:consumer=default:E request/@@//:parent=<default>:E request/@@//:parent=<default>:E request/@@//:parent=<default>:E
resolved/@@//:consumer=changed:E resolved/@@//:consumer=command:E resolved/@@//:consumer=default:E resolved/@@//:consumer=default:E resolved/@@//:consumer=default:R
resolved/@@//:consumer=edited-default:E resolved/@@//:consumer=left:E resolved/@@//:consumer=left:E resolved/@@//:consumer=right:E resolved/@@//:consumer=right:E resolved/@@//:consumer=right:E
resolved/@@//:parent=default:E resolved/@@//:parent=default:E resolved/@@//:parent=default:E resolved/@@//:setting=changed:E resolved/@@//:setting=command:E
resolved/@@//:setting=default:E resolved/@@//:setting=default:E resolved/@@//:setting=edited-default:E resolved/@@//:setting=left:E resolved/@@//:setting=left:E
resolved/@@//:setting=right:E resolved/@@//:setting=right:E resolved/@@//:setting=right:E"#
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
}

async fn analyze_revision(
    dice: &Arc<Dice>,
    tracker: &Arc<AnalysisTracker>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
) -> (Result<AnalysisResult, String>, Vec<(String, EventKind)>) {
    let result = analyze_request(dice, workspace, key, Some(tracker.clone()), false).await;
    (result, tracker.take())
}

async fn analyze_request(
    dice: &Arc<Dice>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> Result<AnalysisResult, String> {
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
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let result = transaction
        .compute(&ConfiguredTargetAnalysisKey {
            workspace: workspace.to_path_buf(),
            configured_target: key.clone(),
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|value| {
            value
                .as_ref()
                .as_ref()
                .cloned()
                .map_err(|error| error.to_string())
        });
    result
}

fn assert_analysis_events(events: &[(String, EventKind)], expected: &[(&str, EventKind)]) {
    let mut actual = events.to_vec();
    actual.sort();
    let mut expected = expected
        .iter()
        .map(|(label, kind)| ((*label).to_owned(), *kind))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
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
        ConfigurationKey::target("first-build").unwrap(),
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
            inject_root_module_request_inputs(
                &mut updater,
                &workspace,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
            )
            .unwrap();
            let mut transaction = updater.commit().await;
            let value = transaction
                .compute(&ConfiguredTargetAnalysisKey {
                    workspace: workspace.clone(),
                    configured_target: key,
                })
                .await
                .unwrap();
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
        ConfigurationKey::target("implicit-default").unwrap(),
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
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let configuration = ConfigurationKey::target("recursive").unwrap();
    let result = transaction
        .compute(&ConfiguredTargetAnalysisKey {
            workspace: workspace.clone(),
            configured_target: ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//parent:parent").unwrap(),
                configuration.clone(),
            ),
        })
        .await
        .unwrap();
    let result = result.as_ref().as_ref().unwrap();

    assert_eq!(
        result.direct_dependencies(),
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

    let leaf = transaction
        .compute(&ConfiguredTargetAnalysisKey {
            workspace: workspace.clone(),
            configured_target: ConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//leaf:second").unwrap(),
                configuration,
            ),
        })
        .await
        .unwrap();
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
    let configuration = ConfigurationKey::target("analysis-events").unwrap();
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
        ConfigurationKey::target("retained").unwrap(),
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
    assert_analysis_events(&events, &[]);

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
    assert_analysis_events(
        &events,
        &[
            ("@@//leaf:first", EventKind::Evaluated),
            ("@@//leaf:second", EventKind::Evaluated),
            ("@@//parent:parent", EventKind::Evaluated),
        ],
    );

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
            ("@@//parent:parent", EventKind::Reused),
        ],
    );
}
