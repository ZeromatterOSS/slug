use std::path::PathBuf;
use std::sync::Arc;
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
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::RootModuleCommandPolicy;
use slug_bzlmod_v2::RootModuleCommandPolicyKey;
use slug_bzlmod_v2::RootModuleEnvironmentPolicy;
use slug_bzlmod_v2::RootModuleEnvironmentPolicyKey;
use slug_bzlmod_v2::RootModuleFilesKey;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootModuleLockfileMode;
use slug_bzlmod_v2::RootModuleLockfileModeKey;
use slug_bzlmod_v2::VisibleLockfileKey;
use slug_bzlmod_v2::VisibleLockfileRead;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileKey;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::sorted_map::SortedMap;

#[derive(Clone)]
struct RequestInputs {
    command: Option<BzlmodCommandPolicyKey>,
    environment: Option<BzlmodEnvironmentPolicyKey>,
    lockfile_mode: Option<LockfileMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvaluationActivation {
    Evaluated,
    Reused,
}

#[derive(Default)]
struct RootEvaluationTracker {
    events: Mutex<Vec<EvaluationActivation>>,
}

impl RootEvaluationTracker {
    fn take(&self) -> Vec<EvaluationActivation> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }

    fn record(&self, activation: EvaluationActivation) {
        let mut events = self.events.lock().unwrap();
        if events.last() != Some(&activation) {
            events.push(activation);
        }
    }
}

impl ActivationTracker for RootEvaluationTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        let activation = match activation {
            ActivationData::Evaluated(_) => EvaluationActivation::Evaluated,
            ActivationData::Reused => EvaluationActivation::Reused,
        };
        if key.to_string().starts_with("root-module-evaluation:") {
            self.record(activation);
            return;
        }
        // When the pure join itself is reused DICE does not reactivate its
        // dependencies. Its recorded dependency edge still proves that the
        // private composed evaluation was reused rather than evaluated.
        let mut reused_composed_evaluation = false;
        if key.downcast_ref::<RootModuleFilesKey>().is_some()
            && activation == EvaluationActivation::Reused
        {
            while let Some(dependency) = deps.next() {
                if dependency
                    .to_string()
                    .starts_with("root-module-evaluation:")
                {
                    reused_composed_evaluation = true;
                    break;
                }
            }
        }
        if reused_composed_evaluation {
            self.record(EvaluationActivation::Reused);
            return;
        }
        // A downstream-only policy change can reevaluate the graph while DICE
        // skips activating the unchanged join entirely. The retained
        // RootModuleFilesKey -> private evaluation edge was established by the
        // initial activation; an evaluated graph with that unchanged join and
        // no direct evaluation event is therefore a composed-evaluation reuse.
        if key.downcast_ref::<RootModuleGraphKey>().is_some()
            && activation == EvaluationActivation::Evaluated
            && self.events.lock().unwrap().is_empty()
        {
            while let Some(dependency) = deps.next() {
                if dependency.downcast_ref::<RootModuleFilesKey>().is_some() {
                    self.record(EvaluationActivation::Reused);
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootEventActivation {
    key: String,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct RootEventTracker {
    activations: Mutex<Vec<RootEventActivation>>,
}

impl RootEventTracker {
    fn take(&self) -> Vec<RootEventActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }

    fn record(&self, activation: RootEventActivation) {
        self.activations.lock().unwrap().push(activation);
    }
}

impl ActivationTracker for RootEventTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        if !matches!(activation, ActivationData::Reused) {
            return;
        }
        if key.downcast_ref::<RootModuleFilesKey>().is_some() {
            while let Some(dependency) = deps.next() {
                let dependency = dependency.to_string();
                if dependency.starts_with("root-module-evaluation:") {
                    self.record(RootEventActivation {
                        key: dependency,
                        kind: ActivationKind::Reused,
                        batch: None,
                    });
                    break;
                }
            }
        }
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.record(RootEventActivation {
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
struct LockfileDependencyTracker {
    raw_edges: Mutex<Vec<bool>>,
}

impl LockfileDependencyTracker {
    fn take(&self) -> Vec<bool> {
        std::mem::take(&mut *self.raw_edges.lock().unwrap())
    }
}

impl ActivationTracker for LockfileDependencyTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        if key.downcast_ref::<VisibleLockfileKey>().is_some() {
            let mut has_raw_edge = false;
            while let Some(dependency) = deps.next() {
                if dependency.downcast_ref::<WorkspaceRawFileKey>().is_some() {
                    has_raw_edge = true;
                    break;
                }
            }
            self.raw_edges.lock().unwrap().push(has_raw_edge);
        }
    }
}

impl RequestInputs {
    fn defaults() -> Self {
        Self {
            command: Some(BzlmodCommandPolicyKey::from_flags(None, false).unwrap()),
            environment: Some(
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            ),
            lockfile_mode: Some(LockfileMode::Update),
        }
    }
}

fn workspace() -> PathBuf {
    PathBuf::from("/root-module-dice-test")
}

fn snapshot(
    entries: impl IntoIterator<Item = (&'static str, WorkspaceFileValue)>,
) -> Arc<WorkspaceSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(
            entries
                .into_iter()
                .map(|(path, value)| (workspace.join(path), value))
                .collect::<SortedMap<_, _>>(),
        ),
    })
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

fn raw_snapshot(
    entries: impl IntoIterator<Item = (&'static str, WorkspaceRawFileValue)>,
) -> Arc<WorkspaceRawSnapshot> {
    let workspace = workspace();
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(
            entries
                .into_iter()
                .map(|(path, value)| (workspace.join(path), value))
                .collect::<SortedMap<_, _>>(),
        ),
    })
}

async fn graph_value(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
) -> Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>> {
    graph_and_module_value(dice, files, RequestInputs::defaults())
        .await
        .0
}

async fn graph_and_module_value(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    inputs: RequestInputs,
) -> (
    Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>>,
    Arc<Result<slug_bzlmod_v2::RootModuleFiles, compact_str::CompactString>>,
) {
    graph_and_module_value_tracked(dice, files, inputs, None).await
}

async fn graph_and_module_value_tracked(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    inputs: RequestInputs,
    tracker: Option<Arc<dyn ActivationTracker>>,
) -> (
    Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>>,
    Arc<Result<slug_bzlmod_v2::RootModuleFiles, compact_str::CompactString>>,
) {
    graph_and_module_value_observed(dice, files, inputs, tracker, false).await
}

async fn graph_and_module_value_observed(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    inputs: RequestInputs,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> (
    Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>>,
    Arc<Result<slug_bzlmod_v2::RootModuleFiles, compact_str::CompactString>>,
) {
    let raw_files = raw_snapshot_from_text(&files);
    graph_and_module_value_with_raw_observed(
        dice,
        files,
        raw_files,
        inputs,
        tracker,
        capture_events,
    )
    .await
}

async fn graph_and_module_value_with_raw_observed(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    raw_files: Arc<WorkspaceRawSnapshot>,
    inputs: RequestInputs,
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> (
    Arc<Result<slug_bzlmod_v2::RootModuleGraph, compact_str::CompactString>>,
    Arc<Result<slug_bzlmod_v2::RootModuleFiles, compact_str::CompactString>>,
) {
    let workspace = workspace();
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
                workspace: workspace.clone(),
            },
            files,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.clone(),
            },
            raw_files,
        )])
        .unwrap();
    if let Some(command) = inputs.command {
        updater
            .changed_to(vec![(
                RootModuleCommandPolicyKey {
                    workspace: workspace.clone(),
                },
                RootModuleCommandPolicy::from(command),
            )])
            .unwrap();
    }
    if let Some(environment) = inputs.environment {
        updater
            .changed_to(vec![(
                RootModuleEnvironmentPolicyKey {
                    workspace: workspace.clone(),
                },
                RootModuleEnvironmentPolicy::from(environment),
            )])
            .unwrap();
    }
    if let Some(lockfile_mode) = inputs.lockfile_mode {
        updater
            .changed_to(vec![(
                RootModuleLockfileModeKey {
                    workspace: workspace.clone(),
                },
                RootModuleLockfileMode::from(lockfile_mode),
            )])
            .unwrap();
    }
    let mut transaction = updater.commit().await;
    let graph = transaction
        .compute(&RootModuleGraphKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    let module = transaction
        .compute(&RootModuleFilesKey {
            workspace: workspace.clone(),
        })
        .await
        .unwrap();
    (graph, module)
}

async fn graph(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
) -> Result<slug_bzlmod_v2::RootModuleGraph, String> {
    graph_value(dice, files)
        .await
        .as_ref()
        .clone()
        .map_err(|error| error.to_string())
}

fn event_texts(batch: &EventBatch) -> Vec<&str> {
    batch
        .events()
        .iter()
        .map(|event| match event {
            EvaluationEvent::StarlarkPrint { text } => text.as_str(),
            EvaluationEvent::Diagnostic { .. } => {
                unreachable!("diagnostic events are not produced by this packet")
            }
        })
        .collect()
}

fn root_event_activations(events: &[RootEventActivation]) -> Vec<&RootEventActivation> {
    events
        .iter()
        .filter(|event| event.key.starts_with("root-module-evaluation:"))
        .collect()
}

fn event_bearing_activations(events: &[RootEventActivation]) -> Vec<&RootEventActivation> {
    events
        .iter()
        .filter(|event| event.batch.is_some())
        .collect()
}

fn assert_single_root_batch(events: &[RootEventActivation], expected: &[&str]) {
    let event_bearing = event_bearing_activations(events);
    assert_eq!(event_bearing.len(), 1, "{events:#?}");
    let event = event_bearing[0];
    assert!(
        event.key.starts_with("root-module-evaluation:"),
        "{events:#?}"
    );
    assert_eq!(event.kind, ActivationKind::Evaluated);
    assert_eq!(event_texts(event.batch.as_ref().unwrap()), expected);
}

async fn observed_graph(
    dice: &Arc<Dice>,
    files: Arc<WorkspaceSnapshot>,
    inputs: RequestInputs,
    tracker: &Arc<RootEventTracker>,
    capture_events: bool,
) -> Result<slug_bzlmod_v2::RootModuleGraph, String> {
    graph_and_module_value_observed(dice, files, inputs, Some(tracker.clone()), capture_events)
        .await
        .0
        .as_ref()
        .clone()
        .map_err(|error| error.to_string())
}

#[tokio::test]
async fn root_graph_requires_explicit_injected_request_values() {
    let files = || {
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new("module(name = 'root')".to_owned())),
        )])
    };
    let cases = [
        (
            RequestInputs {
                command: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module command policy",
        ),
        (
            RequestInputs {
                environment: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module environment policy",
        ),
        (
            RequestInputs {
                lockfile_mode: None,
                ..RequestInputs::defaults()
            },
            "missing injected root module lockfile mode",
        ),
    ];
    for (inputs, expected) in cases {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph_and_module_value(&dice, files(), inputs)
            .await
            .0
            .as_ref()
            .as_ref()
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn request_input_helper_populates_the_complete_fail_closed_triplet() {
    let workspace = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'root')".to_owned())),
            )]),
        )])
        .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        &workspace,
        BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("dep@1.0")).unwrap(),
        LockfileMode::Off,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let graph = transaction
        .compute(&RootModuleGraphKey { workspace })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    assert!(graph.command_policy.ignore_dev_dependency());
}

#[tokio::test]
async fn root_graph_allows_an_omitted_module_declaration() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let graph = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "bazel_dep(name = 'dep', version = '1.0')".to_owned(),
            )),
        )]),
    )
    .await
    .unwrap();
    assert!(graph.module.header.is_none());
    assert_eq!(graph.module.dependencies[0].name, "dep");
}

#[tokio::test]
async fn root_graph_discovers_breadth_first_but_executes_inline_with_isolated_bindings() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = WorkspaceFileValue::Present(Arc::new("module(name = 'root')\nversion = '1.0'\ninclude('//:one.MODULE.bazel')\ninclude('//:two.MODULE.bazel')\nbazel_dep(name = 'dep', version = version)\nlocal_path_override(module_name = 'dep', path = '../dep')\n".to_owned()));
    let one = WorkspaceFileValue::Present(Arc::new(
        "version = 'one-local'\ninclude('//:nested.MODULE.bazel')\nbazel_dep(name = 'one', version = version)\n".to_owned(),
    ));
    let two = WorkspaceFileValue::Present(Arc::new(
        "version = '2.0'\nbazel_dep(name = 'two', version = version)\n".to_owned(),
    ));
    let nested = WorkspaceFileValue::Present(Arc::new(
        "version = '3.0'\nbazel_dep(name = 'nested', version = version)".to_owned(),
    ));
    let first = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(first.module.header.as_ref().unwrap().name, "root");
    assert_eq!(
        first
            .module
            .dependencies
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        ["nested", "one", "two", "dep"]
    );
    assert_eq!(
        first
            .module
            .dependencies
            .iter()
            .map(|dependency| dependency.version.as_str())
            .collect::<Vec<_>>(),
        ["3.0", "one-local", "2.0", "1.0"]
    );
    assert_eq!(
        first.module_file_paths.as_ref(),
        [
            PathBuf::from("MODULE.bazel"),
            PathBuf::from("nested.MODULE.bazel"),
            PathBuf::from("one.MODULE.bazel"),
            PathBuf::from("two.MODULE.bazel"),
        ]
    );

    let unchanged = graph_value(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await;
    let first_value = graph_value(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", one.clone()),
            ("two.MODULE.bazel", two.clone()),
            ("nested.MODULE.bazel", nested.clone()),
        ]),
    )
    .await;
    assert!(Arc::ptr_eq(&unchanged, &first_value));

    let root_absent = graph(
        &dice,
        snapshot([("MODULE.bazel", WorkspaceFileValue::Absent)]),
    )
    .await
    .unwrap_err();
    assert!(
        root_absent.contains("workspace file is absent"),
        "{root_absent}"
    );
    let root_read_error = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::ReadError(Arc::new("root denied".to_owned())),
        )]),
    )
    .await
    .unwrap_err();
    assert_eq!(root_read_error, "root denied");

    let absent = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            ("one.MODULE.bazel", WorkspaceFileValue::Absent),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap_err();
    assert!(absent.contains("workspace file is absent"), "{absent}");
    let read_error = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            (
                "one.MODULE.bazel",
                WorkspaceFileValue::ReadError(Arc::new("denied".to_owned())),
            ),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap_err();
    assert_eq!(read_error, "denied");

    let edited = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root.clone()),
            (
                "one.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "bazel_dep(name = 'changed', version = '4.0')".to_owned(),
                )),
            ),
            ("two.MODULE.bazel", two.clone()),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        edited.module_file_paths.as_ref(),
        [
            PathBuf::from("MODULE.bazel"),
            PathBuf::from("one.MODULE.bazel"),
            PathBuf::from("two.MODULE.bazel"),
        ]
    );
    assert_eq!(edited.module.dependencies[0].name, "changed");
    let recreated = graph(
        &dice,
        snapshot([
            ("MODULE.bazel", root),
            ("one.MODULE.bazel", one),
            ("two.MODULE.bazel", two),
            ("nested.MODULE.bazel", nested),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(recreated, first);
}

#[tokio::test]
async fn root_graph_prepares_the_complete_closure_before_any_execution() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let error = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "fail('root runtime must not execute')\ninclude('//:child.MODULE.bazel')\n"
                        .to_owned(),
                )),
            ),
            (
                "child.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:nested.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "nested.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "bazel_dep(name = undefined_module_name)\n".to_owned(),
                )),
            ),
        ]),
    )
    .await
    .unwrap_err();
    assert!(error.contains("undefined_module_name"), "{error}");
    assert!(error.contains("nested.MODULE.bazel"), "{error}");
    assert!(!error.contains("root runtime must not execute"), "{error}");
}

#[tokio::test]
async fn root_graph_reexecutes_repeated_includes_and_restores_error_context() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let repeated = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:repeat.MODULE.bazel')\ninclude('//:repeat.MODULE.bazel')\n"
                        .to_owned(),
                )),
            ),
            (
                "repeat.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("local_binding = 'safe'\n".to_owned())),
            ),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        repeated.module_file_paths.as_ref(),
        [
            PathBuf::from("MODULE.bazel"),
            PathBuf::from("repeat.MODULE.bazel")
        ]
    );

    let duplicate = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:repeat.MODULE.bazel')\ninclude('//:repeat.MODULE.bazel')\n"
                        .to_owned(),
                )),
            ),
            (
                "repeat.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "single_version_override(module_name = 'repeated')\n".to_owned(),
                )),
            ),
        ]),
    )
    .await
    .unwrap_err();
    assert!(
        duplicate.contains("multiple overrides for module repeated"),
        "{duplicate}"
    );

    let nested_error = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:child.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "child.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:nested.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "nested.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("fail('nested runtime')\n".to_owned())),
            ),
        ]),
    )
    .await
    .unwrap_err();
    assert!(nested_error.contains("nested runtime"), "{nested_error}");
    let root_frame = nested_error
        .rfind("* /root-module-dice-test/MODULE.bazel:1, in <module>")
        .unwrap_or_else(|| panic!("{nested_error}"));
    let child_frame = nested_error[root_frame..]
        .find("* /root-module-dice-test/child.MODULE.bazel:1, in include")
        .map(|offset| root_frame + offset)
        .unwrap_or_else(|| panic!("{nested_error}"));
    let nested_frame = nested_error[child_frame..]
        .find("* /root-module-dice-test/nested.MODULE.bazel:1, in include")
        .map(|offset| child_frame + offset)
        .unwrap_or_else(|| panic!("{nested_error}"));
    assert!(root_frame < child_frame && child_frame < nested_frame);
    assert_eq!(
        nested_error[root_frame..].matches(", in include").count(),
        2
    );
}

#[tokio::test]
async fn root_event_batch_preserves_inline_order_and_never_replays() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootEventTracker::default());
    let files = |nested: &str| {
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "module(name = 'root')\n\
                     print('ROOT_BEFORE')\n\
                     include('//:deps.MODULE.bazel')\n\
                     print('ROOT_AFTER')\n"
                        .to_owned(),
                )),
            ),
            (
                "deps.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "print('DEPS_BEFORE')\n\
                     include('//:nested.MODULE.bazel')\n\
                     print('DEPS_BETWEEN')\n\
                     include('//:nested.MODULE.bazel')\n\
                     print('DEPS_AFTER')\n"
                        .to_owned(),
                )),
            ),
            (
                "nested.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(format!("print('{nested}')\n"))),
            ),
        ])
    };
    let v1 = [
        "ROOT_BEFORE",
        "DEPS_BEFORE",
        "NESTED_V1",
        "DEPS_BETWEEN",
        "NESTED_V1",
        "DEPS_AFTER",
        "ROOT_AFTER",
    ];
    let v2 = [
        "ROOT_BEFORE",
        "DEPS_BEFORE",
        "NESTED_V2",
        "DEPS_BETWEEN",
        "NESTED_V2",
        "DEPS_AFTER",
        "ROOT_AFTER",
    ];

    let graph_v1 = observed_graph(
        &dice,
        files("NESTED_V1"),
        RequestInputs::defaults(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    let events = tracker.take();
    assert_single_root_batch(&events, &v1);
    assert_eq!(event_bearing_activations(&events).len(), 1);

    let warm = observed_graph(
        &dice,
        files("NESTED_V1"),
        RequestInputs::defaults(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(warm, graph_v1);
    let events = tracker.take();
    assert!(event_bearing_activations(&events).is_empty(), "{events:#?}");
    assert!(
        root_event_activations(&events)
            .iter()
            .all(|event| event.kind == ActivationKind::Reused),
        "{events:#?}"
    );

    let fresh_tracker = Arc::new(RootEventTracker::default());
    let fresh_owner = observed_graph(
        &dice,
        files("NESTED_V1"),
        RequestInputs::defaults(),
        &fresh_tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(fresh_owner, graph_v1);
    let events = fresh_tracker.take();
    assert!(event_bearing_activations(&events).is_empty(), "{events:#?}");

    let graph_v2 = observed_graph(
        &dice,
        files("NESTED_V2"),
        RequestInputs::defaults(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(graph_v2, graph_v1);
    assert_single_root_batch(&tracker.take(), &v2);

    let graph_v1_again = observed_graph(
        &dice,
        files("NESTED_V1"),
        RequestInputs::defaults(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(graph_v1_again, graph_v1);
    assert_single_root_batch(&tracker.take(), &v1);
}

#[tokio::test]
async fn root_event_marker_is_untracked_and_failures_store_one_local_batch() {
    let marker_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let marker_tracker = Arc::new(RootEventTracker::default());
    let printed = |text: &str| {
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(format!(
                "module(name = 'root')\nprint('{text}')\n"
            ))),
        )])
    };

    observed_graph(
        &marker_dice,
        printed("DIRECT_V1"),
        RequestInputs::defaults(),
        &marker_tracker,
        false,
    )
    .await
    .unwrap();
    let events = marker_tracker.take();
    let private = root_event_activations(&events);
    assert!(
        private
            .iter()
            .any(|event| event.kind == ActivationKind::Evaluated && event.batch.is_none()),
        "{events:#?}"
    );
    assert!(event_bearing_activations(&events).is_empty());

    observed_graph(
        &marker_dice,
        printed("DIRECT_V1"),
        RequestInputs::defaults(),
        &marker_tracker,
        true,
    )
    .await
    .unwrap();
    let events = marker_tracker.take();
    assert!(event_bearing_activations(&events).is_empty(), "{events:#?}");
    assert!(
        root_event_activations(&events)
            .iter()
            .all(|event| event.kind == ActivationKind::Reused),
        "{events:#?}"
    );

    observed_graph(
        &marker_dice,
        printed("DIRECT_V2"),
        RequestInputs::defaults(),
        &marker_tracker,
        true,
    )
    .await
    .unwrap();
    assert_single_root_batch(&marker_tracker.take(), &["DIRECT_V2"]);

    let failure_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let failure_tracker = Arc::new(RootEventTracker::default());
    let cases = [
        (
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'root')\n".to_owned())),
            )]),
            true,
        ),
        (
            snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "module(name = 'root')\n\
                         print('ROOT_MUST_BE_SUPPRESSED')\n\
                         include('//:child.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                ("child.MODULE.bazel", WorkspaceFileValue::Absent),
            ]),
            false,
        ),
        (
            snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "module(name = 'root')\n\
                         print('ROOT_MUST_BE_SUPPRESSED')\n\
                         include('//:child.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                (
                    "child.MODULE.bazel",
                    WorkspaceFileValue::ReadError(Arc::new("denied".to_owned())),
                ),
            ]),
            false,
        ),
        (
            snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "module(name = 'root')\n\
                         print('ROOT_MUST_BE_SUPPRESSED')\n\
                         include('//:child.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                (
                    "child.MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "print('CHILD_MUST_BE_SUPPRESSED')\n\
                         include('//:nested.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                (
                    "nested.MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new("module(name = 'nested'\n".to_owned())),
                ),
            ]),
            false,
        ),
        (
            snapshot([
                (
                    "MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "module(name = 'root')\n\
                         print('ROOT_MUST_BE_SUPPRESSED')\n\
                         include('//:child.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                (
                    "child.MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "print('CHILD_MUST_BE_SUPPRESSED')\n\
                         include('//:nested.MODULE.bazel')\n"
                            .to_owned(),
                    )),
                ),
                (
                    "nested.MODULE.bazel",
                    WorkspaceFileValue::Present(Arc::new(
                        "module(name = undefined_module_name)\n".to_owned(),
                    )),
                ),
            ]),
            false,
        ),
    ];
    for (files, succeeds) in cases {
        let result = observed_graph(
            &failure_dice,
            files,
            RequestInputs::defaults(),
            &failure_tracker,
            true,
        )
        .await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        assert_single_root_batch(&failure_tracker.take(), &[]);
    }

    let runtime_files = |nested: &str| {
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "module(name = 'root')\n\
                     print('ROOT_BEFORE')\n\
                     include('//:deps.MODULE.bazel')\n\
                     print('ROOT_AFTER')\n"
                        .to_owned(),
                )),
            ),
            (
                "deps.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "print('DEPS_BEFORE')\n\
                     include('//:nested.MODULE.bazel')\n\
                     print('DEPS_BETWEEN')\n\
                     include('//:nested.MODULE.bazel')\n\
                     print('DEPS_AFTER')\n"
                        .to_owned(),
                )),
            ),
            (
                "nested.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(nested.to_owned())),
            ),
        ])
    };
    let runtime = observed_graph(
        &failure_dice,
        runtime_files(
            "print('NESTED_RUNTIME_PREFIX')\nfail('nested runtime')\nprint('NESTED_AFTER')\n",
        ),
        RequestInputs::defaults(),
        &failure_tracker,
        true,
    )
    .await;
    assert!(runtime.unwrap_err().contains("nested runtime"));
    assert_single_root_batch(
        &failure_tracker.take(),
        &["ROOT_BEFORE", "DEPS_BEFORE", "NESTED_RUNTIME_PREFIX"],
    );

    observed_graph(
        &failure_dice,
        runtime_files("print('NESTED_V2')\n"),
        RequestInputs::defaults(),
        &failure_tracker,
        true,
    )
    .await
    .unwrap();
    assert_single_root_batch(
        &failure_tracker.take(),
        &[
            "ROOT_BEFORE",
            "DEPS_BEFORE",
            "NESTED_V2",
            "DEPS_BETWEEN",
            "NESTED_V2",
            "DEPS_AFTER",
            "ROOT_AFTER",
        ],
    );
}

#[tokio::test]
async fn root_event_replay_follows_only_ignore_dev_and_source_changes() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootEventTracker::default());
    let files = |printed: &str, version: &str, with_lockfile: bool| {
        let mut entries = vec![(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(format!(
                "module(name = 'root')\n\
                 print('{printed}')\n\
                 bazel_dep(name = 'semantic_dep', version = '{version}')\n\
                 bazel_dep(name = 'dev_dep', dev_dependency = True)\n"
            ))),
        )];
        if with_lockfile {
            entries.push((
                "MODULE.bazel.lock",
                WorkspaceFileValue::Present(Arc::new(
                    "{\"lockFileVersion\":28,\"selectedYankedVersions\":{\"yyy@1.0.0\":\"reason\"}}\n"
                        .to_owned(),
                )),
            ));
        }
        snapshot(entries)
    };

    observed_graph(
        &dice,
        files("POLICY_V1", "1.0", false),
        RequestInputs::defaults(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_single_root_batch(&tracker.take(), &["POLICY_V1"]);

    let command_yanked = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        ..RequestInputs::defaults()
    };
    observed_graph(
        &dice,
        files("POLICY_V1", "1.0", false),
        command_yanked,
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert!(event_bearing_activations(&tracker.take()).is_empty());

    let environment_only = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        ..RequestInputs::defaults()
    };
    observed_graph(
        &dice,
        files("POLICY_V1", "1.0", false),
        environment_only,
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert!(event_bearing_activations(&tracker.take()).is_empty());

    let lockfile_only = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        ..RequestInputs::defaults()
    };
    observed_graph(
        &dice,
        files("POLICY_V1", "1.0", true),
        lockfile_only,
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert!(event_bearing_activations(&tracker.take()).is_empty());

    let ignored = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        ..RequestInputs::defaults()
    };
    let ignored_graph = observed_graph(
        &dice,
        files("POLICY_V1", "1.0", true),
        ignored,
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        ignored_graph
            .module
            .dependencies
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        ["semantic_dep"]
    );
    assert_single_root_batch(&tracker.take(), &["POLICY_V1"]);

    let active = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        ..RequestInputs::defaults()
    };
    let graph_a = observed_graph(
        &dice,
        files("POLICY_V1", "1.0", true),
        active.clone(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(graph_a.module.dependencies.len(), 2);
    assert_single_root_batch(&tracker.take(), &["POLICY_V1"]);

    let graph_b = observed_graph(
        &dice,
        files("POLICY_V2", "2.0", true),
        active.clone(),
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_ne!(graph_b, graph_a);
    assert_single_root_batch(&tracker.take(), &["POLICY_V2"]);

    let graph_a_again = observed_graph(
        &dice,
        files("POLICY_V1", "1.0", true),
        active,
        &tracker,
        true,
    )
    .await
    .unwrap();
    assert_eq!(graph_a_again, graph_a);
    assert_single_root_batch(&tracker.take(), &["POLICY_V1"]);
}

#[tokio::test]
async fn root_graph_preserves_bazel_global_shapes_and_repository_mapping() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = snapshot([
        (
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root', version = '1.2.3', compatibility_level = 7, repo_name = 'self_repo', bazel_compatibility = ['>=9.0.0', '-9.1.0'])\n\
                 include('//deps:parts/nested.MODULE.bazel')\n\
                 bazel_dep(name = 'root_dep', version = '1.0', max_compatibility_level = 12)\n\
                 bazel_dep(name = 'aliased_dep', version = '2.0', repo_name = 'alias')\n\
                 bazel_dep(name = 'nodep_dep', version = '3.0', repo_name = None)\n\
                 bazel_dep(name = 'dev_dep', version = '4.0', dev_dependency = True)\n\
                 local_path_override(module_name = 'root_dep', path = '../root_dep')\n"
                    .to_owned(),
            )),
        ),
        (
            "deps/parts/nested.MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "bazel_dep(name = 'included_dep', version = '5.0')\n".to_owned(),
            )),
        ),
    ]);

    let default = graph(&dice, files.clone()).await.unwrap();
    assert_eq!(default.module.header.as_ref().unwrap().name, "root");
    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(local)) =
        default.overrides.get("root_dep")
    else {
        panic!("root_dep local_path_override was not captured");
    };
    assert_eq!(
        local.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:local.bzl"
    );
    assert_eq!(local.rule_id.rule_name, "local_repository");
    assert!(matches!(
        local.attributes.get("path"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::String(path))
            if path == "../root_dep"
    ));
    assert!(
        default
            .module
            .dependencies
            .iter()
            .any(|dependency| dependency.name == "nodep_dep" && dependency.nodep)
    );
    for (apparent, canonical) in [
        ("root_dep", "root_dep+"),
        ("alias", "aliased_dep+"),
        ("dev_dep", "dev_dep+"),
        ("included_dep", "included_dep+"),
        ("nodep_dep", "nodep_dep"),
    ] {
        assert_eq!(
            default
                .repository_mapping
                .resolve(&ApparentRepoName::new(apparent).unwrap())
                .as_str(),
            canonical
        );
    }

    let ignored = graph_and_module_value(
        &dice,
        files,
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(None, true).unwrap()),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0
    .as_ref()
    .clone()
    .unwrap();
    assert!(
        ignored
            .module
            .dependencies
            .iter()
            .all(|dependency| dependency.name != "dev_dep")
    );
    assert_eq!(ignored.overrides.iter().count(), 0);
    assert_eq!(
        ignored
            .repository_mapping
            .resolve(&ApparentRepoName::new("dev_dep").unwrap())
            .as_str(),
        "dev_dep"
    );
}

#[tokio::test]
async fn root_override_owner_captures_all_forms_and_defaults() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let graph = graph(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root')\n\
                 local_path_override(module_name = 'local', path = 'third_party/local')\n\
                 single_version_override(module_name = 'single', patches = ['//:one.patch', ':short.patch', 'bare.patch', '@root//:own.patch', '@@visible//:two.patch'], patch_cmds = ['first', 'second'], patch_strip = -1)\n\
                 multiple_version_override(module_name = 'multiple', versions = ['1.0.0', '2.0.0'])\n\
                 archive_override(module_name = 'archive', urls = ['file:///archive'], patches = [':archive.patch'], nested = {'flag': True, 'number': -2147483648, 'items': ('value', None)})\n\
                 git_override(module_name = 'git', remote = 'file:///repo', commit = 'abc', patches = ('git.patch',), options = {'enabled': False})\n"
                    .to_owned(),
            )),
        )]),
    )
    .await
    .unwrap();

    assert_eq!(graph.overrides.iter().count(), 5);
    let Some(slug_bzlmod_v2::RootModuleOverride::RegistrySingle(single)) =
        graph.overrides.get("single")
    else {
        panic!("single override was not captured");
    };
    assert_eq!(single.version, "");
    assert_eq!(single.registry, "");
    assert_eq!(single.patch_strip, -1);
    assert_eq!(single.patch_cmds.as_ref(), ["first", "second"]);
    assert_eq!(single.patches[0].to_string(), "@@//:one.patch");
    assert_eq!(single.patches[1].to_string(), "@@//:short.patch");
    assert_eq!(single.patches[2].to_string(), "@@//:bare.patch");
    assert_eq!(single.patches[3].to_string(), "@@//:own.patch");
    assert_eq!(single.patches[4].to_string(), "@@visible//:two.patch");

    let Some(slug_bzlmod_v2::RootModuleOverride::RegistryMultiple(multiple)) =
        graph.overrides.get("multiple")
    else {
        panic!("multiple override was not captured");
    };
    assert_eq!(multiple.versions.as_ref(), ["1.0.0", "2.0.0"]);
    assert_eq!(multiple.registry, "");

    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(archive)) =
        graph.overrides.get("archive")
    else {
        panic!("archive override was not captured");
    };
    assert_eq!(
        archive.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:http.bzl"
    );
    assert_eq!(archive.rule_id.rule_name, "http_archive");
    assert!(matches!(
        archive.attributes.get("nested"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::Map(_))
    ));
    assert!(matches!(
        archive.attributes.get("patches"),
        Some(slug_bzlmod_v2::OverrideAttributeValue::Iterable(patches))
            if matches!(
                patches.as_ref(),
                [slug_bzlmod_v2::OverrideAttributeValue::String(patch)]
                    if patch == ":archive.patch"
            )
    ));
    let Some(slug_bzlmod_v2::RootModuleOverride::NonRegistry(git)) = graph.overrides.get("git")
    else {
        panic!("git override was not captured");
    };
    assert_eq!(
        git.rule_id.bzl_file.to_string(),
        "@@bazel_tools//tools/build_defs/repo:git.bzl"
    );
    assert_eq!(git.rule_id.rule_name, "git_repository");
}

#[tokio::test]
async fn root_override_owner_rejects_duplicates_and_invalid_boundaries() {
    let cases = [
        (
            "single_version_override(module_name = 'dup')\nsingle_version_override(module_name = 'dup')",
            "multiple overrides for module dup",
        ),
        (
            "multiple_version_override(module_name = 'versions', versions = ['1.0.0'])",
            "at least two versions",
        ),
        (
            "bazel_dep(name = 'visible', version = '1.0.0')\nsingle_version_override(module_name = 'label', patches = ['@visible//:patch'])",
            "not visible",
        ),
        (
            "archive_override(module_name = 'label', patches = ['@invisible//:patch'])",
            "not visible",
        ),
        (
            "single_version_override(module_name = 'version', version = 'not valid')",
            "Invalid version",
        ),
        (
            "archive_override(module_name = 'integer', value = 2147483648)",
            "unsupported repository override attribute value",
        ),
        (
            "cycle = []\ncycle.append(cycle)\narchive_override(module_name = 'cycle', value = cycle)",
            "must not contain cyclic values",
        ),
    ];
    for (directives, expected) in cases {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph(
            &dice,
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(format!(
                    "module(name = 'root')\n{directives}\n"
                ))),
            )]),
        )
        .await
        .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn root_override_owner_merges_includes_and_replays_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = |root: &str| {
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(root.to_owned())),
            ),
            (
                "included.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "single_version_override(module_name = 'included', patches = ['@root//:included.patch'])\n"
                        .to_owned(),
                )),
            ),
        ])
    };
    let a_source = "module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'route', registry = 'file:///a')\n";
    let b_source = "module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'route', version = '2.0.0', registry = 'file:///b')\n";
    let a = graph(&dice, files(a_source)).await.unwrap();
    let b = graph(&dice, files(b_source)).await.unwrap();
    let a_again = graph(&dice, files(a_source)).await.unwrap();
    assert_eq!(a.overrides.iter().count(), 2);
    let Some(slug_bzlmod_v2::RootModuleOverride::RegistrySingle(included)) =
        a.overrides.get("included")
    else {
        panic!("included override was not captured");
    };
    assert_eq!(included.patches[0].to_string(), "@@//:included.patch");
    assert_ne!(a.overrides, b.overrides);
    assert_eq!(a.overrides, a_again.overrides);

    let ordered = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'alpha')\nsingle_version_override(module_name = 'omega')\n"),
    )
    .await
    .unwrap();
    let reordered = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'omega')\nsingle_version_override(module_name = 'alpha')\n"),
    )
    .await
    .unwrap();
    assert_eq!(ordered.overrides, reordered.overrides);

    let duplicate = graph(
        &dice,
        files("module(name = 'root')\ninclude('//:included.MODULE.bazel')\nsingle_version_override(module_name = 'included')\n"),
    )
    .await
    .unwrap_err();
    assert!(duplicate.contains("multiple overrides for module included"));
}

#[tokio::test]
async fn root_evaluation_projects_only_ignore_dev_and_tracks_a_b_a() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootEvaluationTracker::default());
    let files = |suffix: &str| {
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(format!(
                "module(name = 'root')\n\
                 bazel_dep(name = 'dev_dep', dev_dependency = True)\n\
                 local_path_override(module_name = 'dev_dep', path = '../dev')\n\
                 {suffix}\n"
            ))),
        )])
    };

    let (_, evaluated) = graph_and_module_value_tracked(
        &dice,
        files(""),
        RequestInputs::defaults(),
        Some(tracker.clone()),
    )
    .await;
    assert_eq!(tracker.take(), [EvaluationActivation::Evaluated]);
    let evaluated = evaluated.as_ref().as_ref().unwrap();
    assert_eq!(evaluated.module.dependencies.len(), 1);
    assert_eq!(evaluated.overrides.iter().count(), 1);

    let command_yanked = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        ..RequestInputs::defaults()
    };
    graph_and_module_value_tracked(&dice, files(""), command_yanked, Some(tracker.clone())).await;
    assert_eq!(tracker.take(), [EvaluationActivation::Reused]);

    let environment_only = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        ..RequestInputs::defaults()
    };
    graph_and_module_value_tracked(&dice, files(""), environment_only, Some(tracker.clone())).await;
    assert_eq!(tracker.take(), [EvaluationActivation::Reused]);

    let lockfile = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        lockfile_mode: Some(LockfileMode::Refresh),
        ..RequestInputs::defaults()
    };
    graph_and_module_value_tracked(&dice, files(""), lockfile, Some(tracker.clone())).await;
    assert_eq!(tracker.take(), [EvaluationActivation::Reused]);

    let ignored_inputs = RequestInputs {
        command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap()),
        environment: Some(
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
        ),
        lockfile_mode: Some(LockfileMode::Refresh),
        ..RequestInputs::defaults()
    };
    let (_, ignored) =
        graph_and_module_value_tracked(&dice, files(""), ignored_inputs, Some(tracker.clone()))
            .await;
    assert_eq!(tracker.take(), [EvaluationActivation::Evaluated]);
    let ignored = ignored.as_ref().as_ref().unwrap();
    assert!(ignored.module.dependencies.is_empty());
    assert_eq!(ignored.overrides.iter().count(), 0);

    graph_and_module_value_tracked(
        &dice,
        files(""),
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
            environment: Some(
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            ),
            lockfile_mode: Some(LockfileMode::Refresh),
        },
        Some(tracker.clone()),
    )
    .await;
    assert_eq!(tracker.take(), [EvaluationActivation::Evaluated]);

    graph_and_module_value_tracked(
        &dice,
        files("bazel_dep(name = 'source_change', version = '1.0')"),
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
            environment: Some(
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            ),
            lockfile_mode: Some(LockfileMode::Refresh),
        },
        Some(tracker.clone()),
    )
    .await;
    assert_eq!(tracker.take(), [EvaluationActivation::Evaluated]);

    graph_and_module_value_tracked(
        &dice,
        files(""),
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(Some("all"), false).unwrap()),
            environment: Some(
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            ),
            lockfile_mode: Some(LockfileMode::Refresh),
        },
        Some(tracker.clone()),
    )
    .await;
    assert_eq!(tracker.take(), [EvaluationActivation::Evaluated]);

    let invalid_while_ignored = graph_and_module_value_tracked(
        &dice,
        snapshot([(
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new(
                "module(name = 'root')\n\
                 single_version_override(module_name = 'invalid', version = 'not valid')\n"
                    .to_owned(),
            )),
        )]),
        RequestInputs {
            command: Some(BzlmodCommandPolicyKey::from_flags(None, true).unwrap()),
            ..RequestInputs::defaults()
        },
        Some(tracker.clone()),
    )
    .await
    .0;
    let error = invalid_while_ignored
        .as_ref()
        .as_ref()
        .unwrap_err()
        .to_string();
    assert!(error.contains("Invalid version"), "{error}");
}

#[tokio::test]
async fn visible_lockfile_transitions_are_semantic_and_recover_on_retained_dice() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let module = WorkspaceFileValue::Present(Arc::new("module(name = 'root')\n".to_owned()));
    let files = |lockfile: Option<WorkspaceFileValue>| {
        let mut entries = vec![("MODULE.bazel", module.clone())];
        if let Some(lockfile) = lockfile {
            entries.push(("MODULE.bazel.lock", lockfile));
        }
        snapshot(entries)
    };

    let (absent, module_absent) =
        graph_and_module_value(&dice, files(None), RequestInputs::defaults()).await;
    let absent = absent.as_ref().as_ref().unwrap().clone();
    assert_eq!(
        absent.visible_lockfile,
        VisibleLockfileRead::Parsed(slug_bzlmod_v2::empty_bazel_lockfile().into())
    );

    let (formatted, module_formatted) = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\n  \"facts\": {},\n  \"lockFileVersion\": 28,\n  \"moduleExtensions\": {},\n  \"registryFileHashes\": {},\n  \"selectedYankedVersions\": {}\n}\n"
                .to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await;
    assert_eq!(&absent, formatted.as_ref().as_ref().unwrap());
    assert_eq!(
        module_absent.as_ref().as_ref().unwrap().module,
        module_formatted.as_ref().as_ref().unwrap().module
    );

    let malformed = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28, nope".to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await
    .0;
    let error = malformed.as_ref().as_ref().unwrap_err().to_string();
    assert!(
        error.contains("Failed to read and parse the MODULE.bazel.lock file"),
        "{error}"
    );

    let restored = graph(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28}\n".to_owned(),
        )))),
    )
    .await
    .unwrap();
    assert_eq!(restored, absent);

    let (deleted, module_deleted) =
        graph_and_module_value(&dice, files(None), RequestInputs::defaults()).await;
    assert_eq!(deleted.as_ref().as_ref().unwrap(), &absent);
    assert_eq!(
        module_absent.as_ref().as_ref().unwrap().module,
        module_deleted.as_ref().as_ref().unwrap().module
    );

    let (recreated, module_recreated) = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":28}\n".to_owned(),
        )))),
        RequestInputs::defaults(),
    )
    .await;
    assert_eq!(recreated.as_ref().as_ref().unwrap(), &absent);
    assert_eq!(
        module_absent.as_ref().as_ref().unwrap().module,
        module_recreated.as_ref().as_ref().unwrap().module
    );

    for mode in [
        LockfileMode::Update,
        LockfileMode::Refresh,
        LockfileMode::Error,
    ] {
        let read_error = graph_and_module_value(
            &dice,
            files(Some(WorkspaceFileValue::ReadError(Arc::new(
                "permission denied".to_owned(),
            )))),
            RequestInputs {
                lockfile_mode: Some(mode),
                ..RequestInputs::defaults()
            },
        )
        .await
        .0;
        let error = read_error.as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("permission denied"), "{error}");
        assert!(error.contains("Try deleting it"), "{error}");
    }

    let stale_error = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::Present(Arc::new(
            "{\"lockFileVersion\":27, nope".to_owned(),
        )))),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Error),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0;
    let error = stale_error.as_ref().as_ref().unwrap_err().to_string();
    assert_eq!(
        error,
        "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    );

    let ignored = graph_and_module_value(
        &dice,
        files(Some(WorkspaceFileValue::ReadError(Arc::new(
            "permission denied".to_owned(),
        )))),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Off),
            ..RequestInputs::defaults()
        },
    )
    .await
    .0;
    assert_eq!(
        ignored.as_ref().as_ref().unwrap().visible_lockfile,
        VisibleLockfileRead::Ignored
    );
}

#[tokio::test]
async fn visible_lockfile_raw_bytes_drive_retained_dice_and_off_has_no_raw_edge() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let files = snapshot([
        (
            "MODULE.bazel",
            WorkspaceFileValue::Present(Arc::new("module(name = 'root')\n".to_owned())),
        ),
        (
            "MODULE.bazel.lock",
            WorkspaceFileValue::ReadError(Arc::new("text lockfile must not be read".to_owned())),
        ),
    ]);
    let module_bytes =
        WorkspaceRawFileValue::Present(Arc::from(b"module(name = 'root')\n".as_slice()));
    let lockfile_a = WorkspaceRawFileValue::Present(Arc::from(
        b"{\"unknown\":\"\xff\",\"lockFileVersion\":28}".as_slice(),
    ));
    let raw_files = |lockfile| {
        raw_snapshot([
            ("MODULE.bazel", module_bytes.clone()),
            ("MODULE.bazel.lock", lockfile),
        ])
    };
    let tracker = Arc::new(LockfileDependencyTracker::default());

    let first = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(lockfile_a.clone()),
        RequestInputs::defaults(),
        Some(tracker.clone()),
        false,
    )
    .await
    .0;
    let first = first.as_ref().as_ref().unwrap().clone();
    assert_eq!(
        first.visible_lockfile,
        VisibleLockfileRead::Parsed(slug_bzlmod_v2::empty_bazel_lockfile().into())
    );
    assert_eq!(tracker.take(), [true]);

    let reuse_tracker = Arc::new(RootEvaluationTracker::default());
    let formatting_equivalent = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(WorkspaceRawFileValue::Present(Arc::from(
            b"{\n  \"facts\": {},\n  \"lockFileVersion\": 28,\n  \"moduleExtensions\": {},\n  \"registryFileHashes\": {},\n  \"selectedYankedVersions\": {}\n}\n"
                .as_slice(),
        ))),
        RequestInputs::defaults(),
        Some(reuse_tracker.clone()),
        false,
    )
    .await
    .0;
    assert_eq!(formatting_equivalent.as_ref().as_ref().unwrap(), &first);
    assert_eq!(reuse_tracker.take(), [EvaluationActivation::Reused]);

    let malformed = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(WorkspaceRawFileValue::Present(Arc::from(
            b"{\"lockFileVersion\":28, nope".as_slice(),
        ))),
        RequestInputs::defaults(),
        None,
        false,
    )
    .await
    .0;
    let error = malformed.as_ref().as_ref().unwrap_err().to_string();
    assert!(
        error.contains("Failed to read and parse the MODULE.bazel.lock file"),
        "{error}"
    );

    for bytes in [
        b"{\"lockFileVersion\":28,\"registryFileHashes\":{\"u\":\"bad\"}}".as_slice(),
        br#"{"lockFileVersion":28,"moduleExtensions":{"//:ext.bzl":{"general":{}}}}"#.as_slice(),
    ] {
        let direct = graph_and_module_value_with_raw_observed(
            &dice,
            files.clone(),
            raw_files(WorkspaceRawFileValue::Present(Arc::from(bytes))),
            RequestInputs::defaults(),
            None,
            false,
        )
        .await
        .0;
        let error = direct.as_ref().as_ref().unwrap_err().to_string();
        assert!(
            !error.contains("Failed to read and parse the MODULE.bazel.lock file"),
            "{error}"
        );
    }

    let leading_zero_current = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(WorkspaceRawFileValue::Present(Arc::from(
            br#"{"decoy":{"lockFileVersion":028},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#
                .as_slice(),
        ))),
        RequestInputs::defaults(),
        None,
        false,
    )
    .await
    .0;
    assert_eq!(
        leading_zero_current
            .as_ref()
            .as_ref()
            .unwrap()
            .visible_lockfile
            .parsed()
            .unwrap()
            .registry_file_expectation("u")
            .unwrap(),
        slug_bzlmod_v2::RegistryFileExpectation::RecordedAbsent
    );

    let noncurrent = WorkspaceRawFileValue::Present(Arc::from(
        br#"{"decoy":{"lockFileVersion":027},"lockFileVersion":28,"registryFileHashes":{"u":"not found"}}"#
            .as_slice(),
    ));
    for mode in [LockfileMode::Update, LockfileMode::Refresh] {
        let empty = graph_and_module_value_with_raw_observed(
            &dice,
            files.clone(),
            raw_files(noncurrent.clone()),
            RequestInputs {
                lockfile_mode: Some(mode),
                ..RequestInputs::defaults()
            },
            None,
            false,
        )
        .await
        .0;
        assert_eq!(
            empty.as_ref().as_ref().unwrap().visible_lockfile,
            first.visible_lockfile
        );
    }
    let noncurrent_error = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(noncurrent),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Error),
            ..RequestInputs::defaults()
        },
        None,
        false,
    )
    .await
    .0;
    assert_eq!(
        noncurrent_error.as_ref().as_ref().unwrap_err().as_str(),
        "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    );

    let semantic_b = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(WorkspaceRawFileValue::Present(Arc::from(
            b"{\"lockFileVersion\":28,\"registryFileHashes\":{\"u\":\"not found\"}}".as_slice(),
        ))),
        RequestInputs::defaults(),
        None,
        false,
    )
    .await
    .0;
    assert_ne!(semantic_b.as_ref().as_ref().unwrap(), &first);

    let restored = graph_and_module_value_with_raw_observed(
        &dice,
        files.clone(),
        raw_files(lockfile_a),
        RequestInputs::defaults(),
        None,
        false,
    )
    .await
    .0;
    assert_eq!(restored.as_ref().as_ref().unwrap(), &first);

    let off_tracker = Arc::new(LockfileDependencyTracker::default());
    let off_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let off = graph_and_module_value_with_raw_observed(
        &off_dice,
        files,
        raw_snapshot([
            ("MODULE.bazel", module_bytes),
            (
                "MODULE.bazel.lock",
                WorkspaceRawFileValue::ReadError(Arc::new(
                    "raw lockfile must not be read in off mode".to_owned(),
                )),
            ),
        ]),
        RequestInputs {
            lockfile_mode: Some(LockfileMode::Off),
            ..RequestInputs::defaults()
        },
        Some(off_tracker.clone()),
        false,
    )
    .await
    .0;
    assert_eq!(
        off.as_ref().as_ref().unwrap().visible_lockfile,
        VisibleLockfileRead::Ignored
    );
    assert_eq!(off_tracker.take(), [false]);
}

#[tokio::test]
async fn module_and_include_errors_precede_visible_lockfile_errors() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let cases = [
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "bazel_dep(name = 'dep')\nmodule(name = 'root')\n".to_owned(),
                )),
            ),
            (
                "MODULE.bazel.lock",
                WorkspaceFileValue::Present(Arc::new("{\"lockFileVersion\":28, nope".to_owned())),
            ),
        ]),
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "module(name = 'root')\ninclude('//pkg:child.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "pkg/child.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'included')\n".to_owned())),
            ),
            (
                "MODULE.bazel.lock",
                WorkspaceFileValue::Present(Arc::new("{\"lockFileVersion\":28, nope".to_owned())),
            ),
        ]),
    ];
    for files in cases {
        let error = graph_and_module_value(
            &dice,
            files,
            RequestInputs {
                lockfile_mode: Some(LockfileMode::Error),
                ..RequestInputs::defaults()
            },
        )
        .await
        .0;
        let error = error.as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains("module()"), "{error}");
        assert!(!error.contains("MODULE.bazel.lock"), "{error}");
    }
}

#[tokio::test]
async fn root_graph_rejects_invalid_global_calls_and_include_labels() {
    let invalid_sources = [
        "bazel_dep(name = 'dep')\nmodule(name = 'root')\n",
        "module(name = 'Bad')\n",
        "bazel_dep(name = '')\n",
        "module(name = 'root', version = '1..0')\n",
        "module(name = 'root', repo_name = 'bad+')\n",
        "bazel_dep(version = '1.0')\n",
        "include('@repo//pkg:file.MODULE.bazel')\n",
        "include(':relative.MODULE.bazel')\n",
        r#"include('//pkg:bad\name.MODULE.bazel')"#,
        "include('//pkg:../bad.MODULE.bazel')\n",
        "include('//pkg:.MODULE.bazel')\n",
        "include('//pkg:bad.txt')\n",
        "include('//pkg::bad.MODULE.bazel')\n",
        "include('//...:ok.MODULE.bazel')\n",
    ];
    for source in invalid_sources {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = graph(
            &dice,
            snapshot([(
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(source.to_owned())),
            )]),
        )
        .await
        .unwrap_err();
        assert!(!error.is_empty(), "source unexpectedly passed: {source}");
    }

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let included_module = graph(
        &dice,
        snapshot([
            (
                "MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new(
                    "include('//:part.MODULE.bazel')\n".to_owned(),
                )),
            ),
            (
                "part.MODULE.bazel",
                WorkspaceFileValue::Present(Arc::new("module(name = 'part')\n".to_owned())),
            ),
        ]),
    )
    .await
    .unwrap_err();
    assert!(
        included_module.contains("module() is called"),
        "{included_module}"
    );
}
