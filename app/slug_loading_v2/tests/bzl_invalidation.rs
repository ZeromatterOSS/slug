use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

use allocative::Allocative;
use async_trait::async_trait;
use dice::ActivationData;
use dice::ActivationKind;
use dice::ActivationTracker;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DynKey;
use dice::Key;
use dice::RichActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::bzl_load_cycle_detector;
use slug_loading_v2::keys::BzlModuleEvalKey;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_loading_v2::load_label::LoadLabel;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadActivation {
    Evaluated,
    Reused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AttributeKeyIdentity {
    BzlModuleEval(PathBuf),
    PackageLoad(PathBuf),
    Consumer(PathBuf),
    Observer(PathBuf),
    RuleCapabilityConsumer(PathBuf),
    RuleCapabilityObserver(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct AttributeConsumerKey {
    workspace: PathBuf,
    package: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct AttributeObserverKey(AttributeConsumerKey);

impl std::fmt::Display for AttributeObserverKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "attribute-observer:{}", self.0.package.display())
    }
}

impl std::fmt::Display for AttributeConsumerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "attribute-consumer:{}", self.package.display())
    }
}

#[async_trait]
impl Key for AttributeConsumerKey {
    type Value = Arc<Result<slug_loading_v2::LoadedPackage, String>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = ctx
            .compute(&PackageLoadKey {
                workspace: self.workspace.clone(),
                package: self.package.clone(),
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|value| {
                value
                    .as_ref()
                    .as_ref()
                    .cloned()
                    .map_err(ToString::to_string)
            });
        Arc::new(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for AttributeObserverKey {
    type Value = Arc<Result<slug_loading_v2::LoadedPackage, String>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&self.0)
            .await
            .expect("attribute consumer")
            .as_ref()
            .clone()
            .into()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RuleCapabilityConsumerKey {
    workspace: PathBuf,
    package: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RuleCapabilityObserverKey(RuleCapabilityConsumerKey);

impl std::fmt::Display for RuleCapabilityConsumerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule-capability-consumer:{}", self.package.display())
    }
}

impl std::fmt::Display for RuleCapabilityObserverKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule-capability-observer:{}", self.0.package.display())
    }
}

type RuleCapabilityProjection = Arc<[Option<slug_loading_v2::RuleCapability>]>;

#[async_trait]
impl Key for RuleCapabilityConsumerKey {
    type Value = Arc<Result<RuleCapabilityProjection, String>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = ctx
            .compute(&PackageLoadKey {
                workspace: self.workspace.clone(),
                package: self.package.clone(),
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|value| {
                value
                    .as_ref()
                    .as_ref()
                    .map(|package| {
                        package
                            .targets
                            .iter()
                            .map(|target| target.rule_capability().cloned())
                            .collect::<Vec<_>>()
                            .into()
                    })
                    .map_err(ToString::to_string)
            });
        Arc::new(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RuleCapabilityObserverKey {
    type Value = Arc<Result<RuleCapabilityProjection, String>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&self.0)
            .await
            .expect("rule capability consumer")
            .as_ref()
            .clone()
            .into()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Default)]
struct AttributeTracker {
    events: Mutex<Vec<(AttributeKeyIdentity, LoadActivation)>>,
}

impl AttributeTracker {
    fn take(&self) -> Vec<(AttributeKeyIdentity, LoadActivation)> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl ActivationTracker for AttributeTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        let identity = key
            .downcast_ref::<BzlModuleEvalKey>()
            .map(|key| AttributeKeyIdentity::BzlModuleEval(key.path.clone()))
            .or_else(|| {
                key.downcast_ref::<PackageLoadKey>()
                    .map(|key| AttributeKeyIdentity::PackageLoad(key.package.clone()))
            })
            .or_else(|| {
                key.downcast_ref::<AttributeConsumerKey>()
                    .map(|key| AttributeKeyIdentity::Consumer(key.package.clone()))
            })
            .or_else(|| {
                key.downcast_ref::<AttributeObserverKey>()
                    .map(|key| AttributeKeyIdentity::Observer(key.0.package.clone()))
            })
            .or_else(|| {
                key.downcast_ref::<RuleCapabilityConsumerKey>()
                    .map(|key| AttributeKeyIdentity::RuleCapabilityConsumer(key.package.clone()))
            })
            .or_else(|| {
                key.downcast_ref::<RuleCapabilityObserverKey>()
                    .map(|key| AttributeKeyIdentity::RuleCapabilityObserver(key.0.package.clone()))
            });
        if let Some(identity) = identity {
            let activation = match activation {
                ActivationData::Evaluated(_) => LoadActivation::Evaluated,
                ActivationData::Reused => LoadActivation::Reused,
            };
            self.events.lock().unwrap().push((identity, activation));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadingEventActivation {
    identity: AttributeKeyIdentity,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct LoadingEventTracker {
    activations: Mutex<Vec<LoadingEventActivation>>,
}

impl LoadingEventTracker {
    fn take(&self) -> Vec<LoadingEventActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for LoadingEventTracker {
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
        let identity = key
            .downcast_ref::<BzlModuleEvalKey>()
            .map(|key| AttributeKeyIdentity::BzlModuleEval(key.path.clone()))
            .or_else(|| {
                key.downcast_ref::<PackageLoadKey>()
                    .map(|key| AttributeKeyIdentity::PackageLoad(key.package.clone()))
            });
        if let Some(identity) = identity {
            self.activations
                .lock()
                .unwrap()
                .push(LoadingEventActivation {
                    identity,
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
        }
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

fn activation_texts<'a>(
    activations: &'a [LoadingEventActivation],
    identity: &AttributeKeyIdentity,
) -> Option<Vec<&'a str>> {
    activation_texts_for_kind(activations, identity, ActivationKind::Evaluated)
}

fn activation_texts_for_kind<'a>(
    activations: &'a [LoadingEventActivation],
    identity: &AttributeKeyIdentity,
    kind: ActivationKind,
) -> Option<Vec<&'a str>> {
    activations
        .iter()
        .find(|activation| activation.kind == kind && &activation.identity == identity)
        .and_then(|activation| activation.batch.as_ref())
        .map(event_texts)
}

fn activation_batch_for_kind<'a>(
    activations: &'a [LoadingEventActivation],
    identity: &AttributeKeyIdentity,
    kind: ActivationKind,
) -> Option<&'a EventBatch> {
    activations
        .iter()
        .find(|activation| activation.kind == kind && &activation.identity == identity)
        .and_then(|activation| activation.batch.as_ref())
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-bzl-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn directory_snapshot(root: &Path) -> WorkspaceDirectorySnapshot {
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

fn load_package(
    dice: &Arc<Dice>,
    runtime: &tokio::runtime::Runtime,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    runtime.block_on(load_package_request(
        dice, workspace, package, bzl_paths, None, false,
    ))
}

async fn load_package_request(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
    tracker: Option<Arc<dyn ActivationTracker>>,
    consume_metadata: bool,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    load_package_request_with_event_capture(
        dice,
        workspace,
        package,
        bzl_paths,
        tracker,
        consume_metadata,
        false,
    )
    .await
}

async fn load_package_request_with_event_capture(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
    tracker: Option<Arc<dyn ActivationTracker>>,
    consume_metadata: bool,
    capture_events: bool,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    let paths = [
        vec![
            workspace.join("MODULE.bazel"),
            workspace.join("BUILD.bazel"),
            package.join("BUILD.bazel"),
            package.join("BUILD"),
        ],
        bzl_paths.to_vec(),
    ]
    .concat();
    let files: starlark_map::sorted_map::SortedMap<PathBuf, WorkspaceFileValue> = paths
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
    let evaluator = BzlModuleEvaluator::new(workspace)?;
    let mut user_data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: tracker,
        ..Default::default()
    };
    if capture_events {
        user_data.data.set(CaptureEvaluationEvents);
    }
    let mut updater = dice.updater_with_data(user_data);
    updater.changed_to(vec![(
        (WorkspaceSnapshotKey {
            workspace: workspace.to_path_buf(),
        }),
        text,
    )])?;
    updater.changed_to(vec![(
        WorkspaceRawSnapshotKey {
            workspace: workspace.to_path_buf(),
        },
        raw,
    )])?;
    updater.changed_to(vec![(
        (WorkspaceDirectorySnapshotKey {
            workspace: workspace.to_path_buf(),
        }),
        Arc::new(directory_snapshot(workspace)),
    )])?;
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )?;
    let mut transaction = updater.commit().await;
    if consume_metadata {
        let value = transaction
            .compute(&AttributeObserverKey(AttributeConsumerKey {
                workspace: workspace.to_path_buf(),
                package: package.to_path_buf(),
            }))
            .await?;
        match value.as_ref().as_ref() {
            Ok(package) => Ok(package.clone()),
            Err(error) => Err(anyhow::anyhow!(error.clone())),
        }
    } else {
        evaluator.evaluate_package(&mut transaction, package).await
    }
}

async fn load_rule_capability_request(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
    tracker: Arc<dyn ActivationTracker>,
) -> anyhow::Result<RuleCapabilityProjection> {
    let paths = [
        vec![
            workspace.join("MODULE.bazel"),
            workspace.join("BUILD.bazel"),
            package.join("BUILD.bazel"),
            package.join("BUILD"),
        ],
        bzl_paths.to_vec(),
    ]
    .concat();
    let files: starlark_map::sorted_map::SortedMap<PathBuf, WorkspaceFileValue> = paths
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
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    });
    updater.changed_to(vec![(
        WorkspaceSnapshotKey {
            workspace: workspace.to_path_buf(),
        },
        text,
    )])?;
    updater.changed_to(vec![(
        WorkspaceRawSnapshotKey {
            workspace: workspace.to_path_buf(),
        },
        raw,
    )])?;
    updater.changed_to(vec![(
        WorkspaceDirectorySnapshotKey {
            workspace: workspace.to_path_buf(),
        },
        Arc::new(directory_snapshot(workspace)),
    )])?;
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )?;
    let mut transaction = updater.commit().await;
    let value = transaction
        .compute(&RuleCapabilityObserverKey(RuleCapabilityConsumerKey {
            workspace: workspace.to_path_buf(),
            package: package.to_path_buf(),
        }))
        .await?;
    match value.as_ref().as_ref() {
        Ok(projection) => Ok(projection.clone()),
        Err(error) => Err(anyhow::anyhow!("{error}")),
    }
}

fn evaluate_load(
    dice: &Arc<Dice>,
    runtime: &tokio::runtime::Runtime,
    workspace: &Path,
    package: &Path,
    bzl_paths: &[PathBuf],
    load: &str,
) -> anyhow::Result<slug_loading_v2::EvaluatedBzlModule> {
    let paths = [
        vec![
            workspace.join("MODULE.bazel"),
            workspace.join("BUILD.bazel"),
            package.join("BUILD.bazel"),
            package.join("BUILD"),
        ],
        bzl_paths.to_vec(),
    ]
    .concat();
    let files = paths
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
    let evaluator = BzlModuleEvaluator::new(workspace)?;
    runtime.block_on(async {
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater.changed_to(vec![(
            (WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(files),
            }),
        )])?;
        updater.changed_to(vec![(
            (WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            }),
            Arc::new(directory_snapshot(workspace)),
        )])?;
        let mut transaction = updater.commit().await;
        evaluator
            .evaluate_load(&mut transaction, package, load)
            .await
    })
}

#[test]
fn load_label_must_point_to_bzl_file() {
    let load = LoadLabel::parse("//pkg:defs.bzl").unwrap();
    assert_eq!(load.label().to_string(), "//pkg:defs.bzl");
    assert!(LoadLabel::parse("@repo//pkg:defs.bzl").is_ok());
    assert!(LoadLabel::parse("//pkg:not_defs.txt").is_err());
}

#[test]
fn malformed_bzl_reports_bazel_module_compilation_summary() {
    let workspace = scratch("malformed-module");
    let package = workspace.join("pkg");
    let malformed = package.join("bad.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&malformed, "this is not valid Starlark\n");

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = evaluate_load(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[malformed],
        ":bad.bzl",
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("Parse error"), "{error}");
    assert!(
        error.contains("compilation of module 'pkg/bad.bzl' failed"),
        "{error}"
    );
}

#[test]
fn bzl_load_cycles_report_bazel_shape_without_hanging_and_recover_in_same_dice() {
    let workspace = scratch("cycle-recovery");
    let package = workspace.join("pkg");
    let one = package.join("one.bzl");
    let two = package.join("two.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":one.bzl\", \"one\")\nfilegroup(name = \"probe\")\n",
    );
    write(&one, "load(\":two.bzl\", \"two\")\none = two\n");
    write(&two, "load(\":one.bzl\", \"one\")\ntwo = one\n");

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(1),
                load_package_request(
                    &dice,
                    &workspace,
                    &package,
                    &[one.clone(), two.clone()],
                    None,
                    false,
                ),
            )
            .await
        })
        .expect("load-cycle detector must release the recursive DICE wait")
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "cycle detected in extension files: \n    pkg/BUILD\n.-> //pkg:one.bzl\n|   //pkg:two.bzl\n`-- //pkg:one.bzl"
    );

    write(&two, "two = 1\n");
    let loaded = load_package(&dice, &runtime, &workspace, &package, &[one, two]).unwrap();
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["probe"]
    );
}

#[test]
fn bzl_load_cycle_preserves_the_acyclic_path_from_the_build_file() {
    let workspace = scratch("cycle-prefix");
    let package = workspace.join("pkg");
    let entry = package.join("entry.bzl");
    let one = package.join("one.bzl");
    let two = package.join("two.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":entry.bzl\", \"entry\")\nfilegroup(name = \"probe\")\n",
    );
    write(&entry, "load(\":one.bzl\", \"one\")\nentry = one\n");
    write(&one, "load(\":two.bzl\", \"two\")\none = two\n");
    write(&two, "load(\":one.bzl\", \"one\")\ntwo = one\n");

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(1),
                load_package_request(&dice, &workspace, &package, &[entry, one, two], None, false),
            )
            .await
        })
        .expect("load-cycle detector must retain and release the acyclic prefix")
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "cycle detected in extension files: \n    pkg/BUILD\n    //pkg:entry.bzl\n.-> //pkg:one.bzl\n|   //pkg:two.bzl\n`-- //pkg:one.bzl"
    );
}

#[test]
fn bzl_self_cycle_uses_bazel_self_edge_shape() {
    let workspace = scratch("self-cycle");
    let package = workspace.join("pkg");
    let one = package.join("one.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":one.bzl\", \"one\")\nfilegroup(name = \"probe\")\n",
    );
    write(&one, "load(\":one.bzl\", \"one\")\none = 1\n");

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(1),
                load_package_request(&dice, &workspace, &package, &[one], None, false),
            )
            .await
        })
        .expect("load-cycle detector must release a self-recursive DICE wait")
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "cycle detected in extension files: \n    pkg/BUILD\n.-> //pkg:one.bzl [self-edge]\n`--"
    );
}

#[test]
fn bzl_load_diamond_is_not_reported_as_a_cycle() {
    let workspace = scratch("diamond-no-cycle");
    let package = workspace.join("pkg");
    let left = package.join("left.bzl");
    let right = package.join("right.bzl");
    let leaf = package.join("leaf.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":left.bzl\", \"left\")\nload(\":right.bzl\", \"right\")\nfilegroup(name = \"probe\")\n",
    );
    write(&left, "load(\":leaf.bzl\", \"leaf\")\nleft = leaf\n");
    write(&right, "load(\":leaf.bzl\", \"leaf\")\nright = leaf\n");
    write(&leaf, "leaf = 1\n");

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(1),
                load_package_request(
                    &dice,
                    &workspace,
                    &package,
                    &[left, right, leaf],
                    None,
                    false,
                ),
            )
            .await
        })
        .expect("diamond loading should complete")
        .unwrap();
    assert_eq!(loaded.targets.len(), 1);
}

#[test]
fn injected_bzl_create_edit_delete_replays_the_loaded_package() {
    let workspace = scratch("workspace-file-input");
    let package = workspace.join("pkg");
    let definitions = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let missing = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    );
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("cannot load '//pkg:defs.bzl': no such file")
    );

    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"before\", srcs = [])\n",
    );
    let initial = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(initial.targets[0].name, "before");

    write(
        &definitions,
        "def declare():\n    native.filegroup(name = \"after\", srcs = [])\n",
    );
    let edited = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(edited.targets[0].name, "after");

    fs::remove_file(&definitions).unwrap();
    let deleted = load_package(&dice, &runtime, &workspace, &package, &[definitions]);
    assert!(
        deleted
            .unwrap_err()
            .to_string()
            .contains("cannot load '//pkg:defs.bzl': no such file")
    );

    write(
        &package.join("defs.bzl"),
        "def declare():\n    native.filegroup(name = \"recreated\", srcs = [])\n",
    );
    let recreated = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[package.join("defs.bzl")],
    )
    .unwrap();
    assert_eq!(recreated.targets[0].name, "recreated");
}

#[test]
fn loading_event_capture_is_key_local_empty_replacing_and_failure_prefix_preserving() {
    let workspace = scratch("event-capture");
    let package = workspace.join("pkg");
    let dependency = package.join("dependency.bzl");
    let parent = package.join("parent.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":parent.bzl\", \"declare\")\nprint(\"BUILD_LOCAL\")\ndeclare()\n",
    );
    write(
        &dependency,
        "print(\"DEPENDENCY_LOCAL\")\nVALUE = \"probe\"\n",
    );
    write(
        &parent,
        "load(\":dependency.bzl\", \"VALUE\")\nprint(\"PARENT_LOCAL\")\ndef declare():\n    native.filegroup(name = VALUE)\n",
    );
    let paths = [dependency.clone(), parent.clone()];
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let direct_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let direct_tracker = Arc::new(LoadingEventTracker::default());
    runtime
        .block_on(load_package_request_with_event_capture(
            &direct_dice,
            &workspace,
            &package,
            &paths,
            Some(direct_tracker.clone()),
            false,
            false,
        ))
        .unwrap();
    let direct = direct_tracker.take();
    assert!(
        direct
            .iter()
            .filter(|activation| {
                matches!(
                    &activation.identity,
                    AttributeKeyIdentity::BzlModuleEval(_) | AttributeKeyIdentity::PackageLoad(_)
                )
            })
            .all(|activation| activation.batch.is_none()),
        "{direct:?}"
    );

    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(LoadingEventTracker::default());
    runtime
        .block_on(load_package_request_with_event_capture(
            &dice,
            &workspace,
            &package,
            &paths,
            Some(tracker.clone()),
            false,
            true,
        ))
        .unwrap();
    let initial = tracker.take();
    assert_eq!(
        activation_texts(
            &initial,
            &AttributeKeyIdentity::BzlModuleEval(dependency.clone())
        ),
        Some(vec!["DEPENDENCY_LOCAL"])
    );
    assert!(matches!(
        activation_batch_for_kind(
            &initial,
            &AttributeKeyIdentity::BzlModuleEval(dependency.clone()),
            ActivationKind::Evaluated,
        )
        .unwrap()
        .events(),
        [EvaluationEvent::StarlarkPrint { location, text }]
            if text == "DEPENDENCY_LOCAL"
                && location.to_string() == format!("{}:1:6", dependency.display())
    ));
    assert_eq!(
        activation_texts(
            &initial,
            &AttributeKeyIdentity::BzlModuleEval(parent.clone())
        ),
        Some(vec!["PARENT_LOCAL"])
    );
    assert_eq!(
        activation_texts(
            &initial,
            &AttributeKeyIdentity::PackageLoad(package.clone())
        ),
        Some(vec!["BUILD_LOCAL"])
    );

    write(
        &parent,
        "load(\":dependency.bzl\", \"VALUE\")\ndef declare():\n    native.filegroup(name = VALUE)\n",
    );
    runtime
        .block_on(load_package_request_with_event_capture(
            &dice,
            &workspace,
            &package,
            &paths,
            Some(tracker.clone()),
            false,
            true,
        ))
        .unwrap();
    let empty = tracker.take();
    assert_eq!(
        activation_texts(&empty, &AttributeKeyIdentity::BzlModuleEval(parent.clone())),
        Some(Vec::new())
    );

    write(
        &parent,
        "load(\":dependency.bzl\", \"VALUE\")\nprint(\"PARENT_RUNTIME_PREFIX\")\nfail(\"parent runtime\")\nprint(\"PARENT_RUNTIME_AFTER\")\ndef declare():\n    native.filegroup(name = VALUE)\n",
    );
    let error = runtime
        .block_on(load_package_request_with_event_capture(
            &dice,
            &workspace,
            &package,
            &paths,
            Some(tracker.clone()),
            false,
            true,
        ))
        .unwrap_err()
        .to_string();
    assert!(error.contains("parent runtime"), "{error}");
    let failed = tracker.take();
    assert_eq!(
        activation_texts(
            &failed,
            &AttributeKeyIdentity::BzlModuleEval(parent.clone())
        ),
        Some(vec!["PARENT_RUNTIME_PREFIX"])
    );
    assert_eq!(
        activation_texts(&failed, &AttributeKeyIdentity::PackageLoad(package)),
        Some(Vec::new())
    );
}

#[test]
fn injected_build_primary_absence_selects_build_fallback() {
    let workspace = scratch("build-fallback");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD"),
        "filegroup(name = \"fallback\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(loaded.targets[0].name, "fallback");
}

#[test]
fn local_loader_rejects_external_repository_before_mapping_exists() {
    let workspace = scratch("external");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\"@other//:defs.bzl\", \"declare\")\ndeclare()\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let error = load_package(&dice, &runtime, &workspace, &package, &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("external repository load"), "{error}");
}

#[test]
fn package_manifest_preserves_direct_edges_and_first_seen_diamond_closure() {
    let workspace = scratch("manifest-diamond");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":a.bzl\", \"declare_a\")\nload(\":b.bzl\", \"declare_b\")\ndeclare_a()\ndeclare_b()\n",
    );
    write(
        &package.join("a.bzl"),
        "load(\":shared.bzl\", \"first\")\nload(\":shared.bzl\", \"second\")\nload(\":other.bzl\", \"other\")\ndef declare_a():\n    native.filegroup(name = \"a\", srcs = [])\n",
    );
    write(
        &package.join("b.bzl"),
        "load(\":shared.bzl\", \"shared\")\ndef declare_b():\n    native.filegroup(name = \"b\", srcs = [])\n",
    );
    write(
        &package.join("shared.bzl"),
        "first = 1\nsecond = 2\nshared = 3\n",
    );
    write(&package.join("other.bzl"), "other = 3\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[
            package.join("a.bzl"),
            package.join("b.bzl"),
            package.join("shared.bzl"),
            package.join("other.bzl"),
        ],
    )
    .unwrap();

    assert_eq!(
        loaded
            .direct_load_roots
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:a.bzl", "@@//pkg:b.bzl"]
    );
    assert_eq!(
        loaded
            .reachable_loads
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec![
            "@@//pkg:a.bzl",
            "@@//pkg:shared.bzl",
            "@@//pkg:other.bzl",
            "@@//pkg:b.bzl",
        ]
    );
    assert_eq!(loaded.direct_load_roots.len(), 2);
    assert_eq!(loaded.reachable_loads.len(), 4);
    let a = evaluate_load(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[
            package.join("a.bzl"),
            package.join("shared.bzl"),
            package.join("other.bzl"),
        ],
        ":a.bzl",
    )
    .unwrap();
    assert_eq!(
        a.manifest
            .direct_children
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:shared.bzl", "@@//pkg:other.bzl"]
    );
    assert_eq!(
        a.manifest
            .reachable
            .iter()
            .map(|identity| identity.label.to_string())
            .collect::<Vec<_>>(),
        vec!["@@//pkg:a.bzl", "@@//pkg:shared.bzl", "@@//pkg:other.bzl",]
    );
}

#[test]
fn manifest_changes_when_leaf_content_or_load_edge_changes_without_target_change() {
    let workspace = scratch("manifest-equality");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&build, "load(\":defs.bzl\", \"declare\")\ndeclare()\n");
    write(
        &defs,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(
        &defs,
        "# semantic declaration unchanged\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let leaf_edited = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(initial.targets, leaf_edited.targets);
    assert_ne!(initial, leaf_edited);

    let shared = package.join("shared.bzl");
    write(&shared, "marker = 1\n");
    write(
        &defs,
        "load(\":shared.bzl\", \"marker\")\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let transitive_edge_changed = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[defs.clone(), shared.clone()],
    )
    .unwrap();
    assert_eq!(initial.targets, transitive_edge_changed.targets);
    assert_ne!(leaf_edited, transitive_edge_changed);

    let alternate = package.join("alternate.bzl");
    write(
        &alternate,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    write(&build, "load(\":alternate.bzl\", \"declare\")\ndeclare()\n");
    let edge_changed = load_package(&dice, &runtime, &workspace, &package, &[alternate]).unwrap();
    assert_eq!(initial.targets, edge_changed.targets);
    assert_ne!(transitive_edge_changed, edge_changed);
}

#[test]
fn same_dice_load_edges_invalidate_and_restore_without_target_changes() {
    let workspace = scratch("same-dice-load-edges");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    let shared = package.join("shared.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let no_direct_load = "filegroup(name = \"same\", srcs = [])\n";
    write(&build, no_direct_load);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let direct_absent = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();

    write(&defs, "marker = 1\n");
    let direct_load = "load(\":defs.bzl\", \"marker\")\nfilegroup(name = \"same\", srcs = [])\n";
    write(&build, direct_load);
    let direct_created =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(direct_absent.targets, direct_created.targets);
    assert_ne!(direct_absent, direct_created);

    write(&build, no_direct_load);
    let direct_deleted = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(direct_absent, direct_deleted);

    write(&build, direct_load);
    let direct_recreated =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(direct_created, direct_recreated);

    let transitive_build = "load(\":defs.bzl\", \"declare\")\ndeclare()\n";
    let no_transitive_load = "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n";
    write(&build, transitive_build);
    write(&defs, no_transitive_load);
    let transitive_absent =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(&shared, "marker = 1\n");
    let with_transitive_load = "load(\":shared.bzl\", \"marker\")\ndef declare():\n    native.filegroup(name = \"same\", srcs = [])\n";
    write(&defs, with_transitive_load);
    let transitive_created = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[defs.clone(), shared.clone()],
    )
    .unwrap();
    assert_eq!(transitive_absent.targets, transitive_created.targets);
    assert_ne!(transitive_absent, transitive_created);

    write(&defs, no_transitive_load);
    let transitive_deleted =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(transitive_absent, transitive_deleted);

    write(&defs, with_transitive_load);
    let transitive_recreated =
        load_package(&dice, &runtime, &workspace, &package, &[defs, shared]).unwrap();
    assert_eq!(transitive_created, transitive_recreated);
}

#[test]
fn same_dice_bzl_visibility_source_and_imported_policy_invalidate_and_restore() {
    let workspace = scratch("bzl-visibility-invalidation");
    let importer_package = workspace.join("a");
    let entry = importer_package.join("entry.bzl");
    let dependency = workspace.join("b/dep.bzl");
    let policy = workspace.join("policy/policy.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&entry, "load(\"//b:dep.bzl\", \"VALUE\")\nRESULT = VALUE\n");
    write(&policy, "POLICY = [\"//a\"]\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let evaluate = || {
        evaluate_load(
            &dice,
            &runtime,
            &workspace,
            &importer_package,
            &[entry.clone(), dependency.clone(), policy.clone()],
            ":entry.bzl",
        )
    };

    let public_source = "visibility(\"public\")\nVALUE = 1\n";
    write(&dependency, public_source);
    let direct_a = evaluate().unwrap();
    write(&dependency, "visibility(\"private\")\nVALUE = 1\n");
    let error = evaluate().unwrap_err().to_string();
    assert!(error.contains("@@//b:dep.bzl is not visible"), "{error}");
    assert!(error.contains("package @@//a"), "{error}");
    write(&dependency, public_source);
    assert_eq!(direct_a, evaluate().unwrap());

    write(
        &dependency,
        concat!(
            "load(\"//policy:policy.bzl\", \"POLICY\")\n",
            "visibility(POLICY)\nVALUE = 1\n",
        ),
    );
    let imported_a = evaluate().unwrap();
    write(&policy, "POLICY = [\"//other\"]\n");
    let error = evaluate().unwrap_err().to_string();
    assert!(error.contains("@@//b:dep.bzl is not visible"), "{error}");
    assert!(error.contains("package @@//a"), "{error}");
    write(&policy, "POLICY = [\"//a\"]\n");
    assert_eq!(imported_a, evaluate().unwrap());
}

#[test]
fn build_comment_and_whitespace_edits_do_not_change_loaded_package() {
    let workspace = scratch("build-comment-equality");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(&build, "filegroup(name = \"same\", srcs = [])\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    write(
        &build,
        "# formatting-only edit\nfilegroup( name = \"same\", srcs = [] )\n",
    );
    let formatted = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(initial, formatted);
}

#[test]
fn same_dice_config_setting_values_are_package_semantics() {
    let workspace = scratch("config-setting-values");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let build = package.join("BUILD.bazel");
    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\", \"compilation_mode\": \"opt\"})\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"compilation_mode\": \"opt\", \"cpu\": \"k8\"})\n",
    );
    let reordered = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(initial, reordered);

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"arm\", \"compilation_mode\": \"opt\"})\n",
    );
    let changed = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_ne!(reordered, changed);

    write(
        &build,
        "# formatting only\nconfig_setting( name = \"linux\", values = {\"cpu\": \"arm\", \"compilation_mode\": \"opt\"} )\n",
    );
    let formatted = load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();
    assert_eq!(changed, formatted);
}

#[test]
fn same_dice_native_toolchain_declaration_semantics_invalidate_and_restore() {
    let workspace = scratch("native-toolchain-declaration-semantics");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let declaration = |target: &str,
                       platform_flag: Option<bool>,
                       condition: &str,
                       setting: &str| {
        let platform_flag = platform_flag
            .map(|value| {
                format!(
                    "    use_target_platform_constraints = {},\n",
                    if value { "True" } else { "False" }
                )
            })
            .unwrap_or_default();
        format!(
            concat!(
                "constraint_setting(name = \"constraint\")\n",
                "constraint_value(name = \"target_a\", constraint_setting = \":constraint\")\n",
                "constraint_value(name = \"target_b\", constraint_setting = \":constraint\")\n",
                "config_setting(name = \"condition_a\", values = {{\"cpu\": \"k8\"}})\n",
                "config_setting(name = \"condition_b\", values = {{\"cpu\": \"arm\"}})\n",
                "config_setting(name = \"setting_a\", values = {{\"compilation_mode\": \"opt\"}})\n",
                "config_setting(name = \"setting_b\", values = {{\"compilation_mode\": \"dbg\"}})\n",
                "toolchain_type(name = \"type\")\n",
                "filegroup(name = \"impl\")\n",
                "toolchain(\n",
                "    name = \"chain\",\n",
                "    toolchain_type = \":type\",\n",
                "    toolchain = \":impl\",\n",
                "    exec_compatible_with = [],\n",
                "    target_compatible_with = [\":{target}\"],\n",
                "{platform_flag}",
                "    target_settings = [\":setting_a\"] + select({{\n",
                "        \":{condition}\": [\":{setting}\"],\n",
                "        \"//conditions:default\": [],\n",
                "    }}),\n",
                ")\n",
            ),
            target = target,
            platform_flag = platform_flag,
            condition = condition,
            setting = setting,
        )
    };
    let source_a = declaration("target_a", Some(false), "condition_a", "setting_b");
    write(&build, &source_a);
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = || load_package(&dice, &runtime, &workspace, &package, &[]).unwrap();

    let initial = load();
    assert_eq!(initial, load());
    for changed_source in [
        declaration("target_b", Some(false), "condition_a", "setting_b"),
        declaration("target_a", None, "condition_a", "setting_b"),
        declaration("target_a", Some(true), "condition_a", "setting_b"),
        declaration("target_a", Some(false), "condition_b", "setting_a"),
    ] {
        write(&build, &changed_source);
        assert_ne!(initial, load());
        write(&build, &source_a);
        assert_eq!(initial, load());
    }
}

#[test]
fn same_dice_visibility_and_package_groups_are_semantic_and_recreate_cleanly() {
    let workspace = scratch("visibility-package-group-transitions");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let schema_v1 = r#"
def declare():
    native.filegroup(
        name = "from_bzl",
        srcs = [],
        visibility = [":friends"],
    )
"#;
    let build_v1 = r#"
load(":defs.bzl", "declare")
package(default_visibility = [":defaults"])
package_group(name = "defaults", packages = ["//pkg"])
package_group(
    name = "friends",
    packages = ["//viewer/...", "-//viewer/blocked/..."],
    includes = [":one", ":two"],
)
package_group(name = "other", packages = ["//other"])
package_group(name = "one", packages = ["//one"])
package_group(name = "two", packages = ["//two"])
declare()
"#;
    write(&defs, schema_v1);
    write(&build, build_v1);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(&build, &format!("# formatting only\n{build_v1}"));
    let formatted = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    // The added line changes the observable macro generator_location.
    assert_ne!(initial, formatted);

    let schema_v2 = schema_v1.replace(":friends", ":other");
    write(&defs, &schema_v2);
    write(&build, build_v1);
    let visibility_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(formatted, visibility_changed);

    let build_v2 = build_v1
        .replace("-//viewer/blocked/...", "-//viewer/private/...")
        .replace(
            "includes = [\":one\", \":two\"]",
            "includes = [\":two\", \":one\"]",
        );
    write(&build, &build_v2);
    let package_group_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(visibility_changed, package_group_changed);

    fs::remove_file(&defs).unwrap();
    assert!(load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).is_err());
    write(&defs, &schema_v2);
    let recreated = load_package(&dice, &runtime, &workspace, &package, &[defs]).unwrap();
    assert_eq!(package_group_changed, recreated);
}

#[test]
fn package_context_labels_have_same_dice_equality_and_definition_lifecycle() {
    let workspace = scratch("package-context-labels");
    let package = workspace.join("consumer");
    let definitions = workspace.join("definitions/defs.bzl");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let schema = |default: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"dep\": attr.label(default = \"{default}\")}})\n"
        )
    };
    write(&definitions, &schema("one.txt"));
    let build_for = |srcs: &str, actual: &str| {
        format!(
            "load(\"//definitions:defs.bzl\", \"probe\")\nprobe(name = \"rule\")\nfilegroup(name = \"group\", srcs = {srcs})\nalias(name = \"redirect\", actual = \"{actual}\")\n"
        )
    };
    write(&build, &build_for("[\"one.txt\", \":two.txt\"]", "group"));
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let initial = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    write(&build, &build_for("[\":one.txt\", \"two.txt\"]", ":group"));
    let equivalent = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(initial, equivalent);

    write(&build, &build_for("[\"two.txt\", \"one.txt\"]", "group"));
    let reordered = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_ne!(equivalent, reordered);

    write(
        &build,
        &build_for("[\"two.txt\", \"one.txt\", \":one.txt\"]", "group"),
    );
    let duplicate = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap_err()
    .to_string();
    assert!(
        duplicate.contains(
            "Label '//consumer:one.txt' is duplicated in the 'srcs' attribute of rule 'group'"
        ),
        "{duplicate}"
    );

    write(&build, &build_for("[\"two.txt\", \"one.txt\"]", "group"));
    let recovered = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_eq!(reordered, recovered);

    write(&definitions, &schema("two.txt"));
    let default_changed = load_package(
        &dice,
        &runtime,
        &workspace,
        &package,
        &[definitions.clone()],
    )
    .unwrap();
    assert_ne!(recovered, default_changed);

    fs::remove_file(&definitions).unwrap();
    assert!(
        load_package(
            &dice,
            &runtime,
            &workspace,
            &package,
            &[definitions.clone()]
        )
        .is_err()
    );
    write(&definitions, &schema("two.txt"));
    let recreated = load_package(&dice, &runtime, &workspace, &package, &[definitions]).unwrap();
    assert_eq!(default_changed, recreated);
}

#[test]
fn toolchain_requirements_have_definition_lifecycle_and_semantic_equality() {
    let workspace = scratch("toolchain-requirement-lifecycle");
    let package = workspace.join("consumer");
    let defs = workspace.join("definitions/defs.bzl");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let definitions = |requirement: &str, marker: &str| {
        format!(
            "def _impl(ctx):\n    return [platform_common.ToolchainInfo(marker = \"{marker}\")]\nprobe = rule(implementation = _impl, toolchains = [\"{requirement}\"])\n"
        )
    };
    write(&defs, &definitions(":type_a", "initial"));
    write(
        &build,
        "load(\"//definitions:defs.bzl\", \"probe\")\nprobe(name = \"subject\")\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = || load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    let initial = load();
    let requirement = initial
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(rule) if target.name == "subject" => {
                Some(rule.required_toolchains())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(requirement.len(), 1);
    assert_eq!(requirement[0].to_string(), "@@//definitions:type_a");

    let warm = load();
    assert_eq!(initial, warm);
    write(&defs, &definitions(":type_b", "initial"));
    let changed = load();
    assert_ne!(warm, changed);
    write(&defs, &definitions(":type_a", "initial"));
    let restored = load();
    assert_eq!(initial, restored);

    write(&defs, &definitions(":type_a", "marker edit"));
    let marker = load();
    assert_ne!(restored, marker);
    write(&defs, &definitions(":type_a", "initial"));
    let marker_restored = load();
    assert_eq!(restored, marker_restored);
}

#[test]
fn same_dice_attribute_metadata_edits_are_semantic_and_recreate_cleanly() {
    let workspace = scratch("attribute-metadata-transitions");
    let package = workspace.join("pkg");
    let build = package.join("BUILD.bazel");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let schema_v1 = r#"
def _impl(ctx):
    return [DefaultInfo()]
probe = rule(implementation = _impl, attrs = {
    "many": attr.label_list(default = [":default"]),
    "_implicit": attr.label(default = ":implicit"),
    "chosen": attr.label_list(),
    "out": attr.output(mandatory = True),
})
"#;
    write(&defs, schema_v1);
    let explicit = "load(\":defs.bzl\", \"probe\")\nprobe(name = \"metadata\", many = [\":explicit\"], out = \"one.out\")\n";
    write(&build, explicit);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let initial = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();

    write(
        &build,
        "# formatting only\nload(\":defs.bzl\", \"probe\")\nprobe( name = \"metadata\", many = [\":explicit\"], out = \"one.out\" )\n",
    );
    let formatted = load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_eq!(initial, formatted);

    write(
        &build,
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"metadata\", many = [\":changed\"], out = \"one.out\")\n",
    );
    let build_value_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(formatted, build_value_changed);

    let schema_default_changed = schema_v1.replace(":default", ":other_default");
    write(&defs, &schema_default_changed);
    let default_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(build_value_changed, default_changed);

    let schema_implicit_changed = schema_default_changed.replace(":implicit", ":other_implicit");
    write(&defs, &schema_implicit_changed);
    let implicit_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(default_changed, implicit_changed);

    let schema_select_type_name_changed = schema_implicit_changed.replace(
        "\"chosen\": attr.label_list(),",
        "\"renamed\": attr.label(),",
    );
    write(&defs, &schema_select_type_name_changed);
    write(
        &build,
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"metadata\", renamed = select({\":condition\": \":branch\", \"//conditions:default\": \":fallback\"}), out = \"one.out\")\n",
    );
    let selector_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(implicit_changed, selector_changed);

    write(
        &build,
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"metadata\", renamed = select({\":condition\": \":branch\", \"//conditions:default\": \":fallback\"}), out = \"two.out\")\n",
    );
    let output_changed =
        load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    assert_ne!(selector_changed, output_changed);
    assert!(
        output_changed
            .targets
            .iter()
            .any(|target| target.name == "two.out")
    );

    fs::remove_file(&defs).unwrap();
    assert!(load_package(&dice, &runtime, &workspace, &package, &[defs.clone()]).is_err());
    write(&defs, &schema_select_type_name_changed);
    let recreated = load_package(&dice, &runtime, &workspace, &package, &[defs]).unwrap();
    assert_eq!(output_changed, recreated);
}

#[test]
fn retained_attribute_metadata_loads_activate_or_reuse_by_semantics() {
    let workspace = scratch("attribute-metadata-activation");
    let package = workspace.join("pkg");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let schema = "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"dep\": attr.label(default = \":one\")})\n";
    write(&defs, schema);
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"metadata\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let tracker = Arc::new(AttributeTracker::default());
    let expected = |bzl_module, package_load, consumer, observer| {
        vec![
            (
                AttributeKeyIdentity::BzlModuleEval(defs.clone()),
                bzl_module,
            ),
            (
                AttributeKeyIdentity::PackageLoad(package.clone()),
                package_load,
            ),
            (AttributeKeyIdentity::Consumer(package.clone()), consumer),
            (AttributeKeyIdentity::Observer(package.clone()), observer),
        ]
    };
    let load = |tracker: Arc<AttributeTracker>| {
        runtime.block_on(load_package_request(
            &dice,
            &workspace,
            &package,
            &[defs.clone()],
            Some(tracker),
            true,
        ))
    };

    let initial = load(tracker.clone()).unwrap();
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );
    write(
        &package.join("BUILD.bazel"),
        "# non-semantic\nload(\":defs.bzl\", \"probe\")\nprobe( name = \"metadata\" )\n",
    );
    let formatted = load(tracker.clone()).unwrap();
    assert_eq!(initial, formatted);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Reused,
            LoadActivation::Evaluated,
            LoadActivation::Reused,
            LoadActivation::Reused,
        )
    );

    write(&defs, &schema.replace(":one", ":two"));
    let changed = load(tracker.clone()).unwrap();
    assert_ne!(formatted, changed);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    fs::remove_file(&defs).unwrap();
    assert!(load(tracker.clone()).is_err());
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );
    write(&defs, &schema.replace(":one", ":two"));
    let recreated = load(tracker.clone()).unwrap();
    assert_eq!(changed, recreated);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Reused,
            LoadActivation::Evaluated,
            LoadActivation::Reused,
            LoadActivation::Reused,
        )
    );
}

#[test]
fn retained_rule_capabilities_activate_or_reuse_by_semantics() {
    let workspace = scratch("rule-capability-activation");
    let package = workspace.join("pkg");
    let defs = package.join("defs.bzl");
    let build = package.join("BUILD.bazel");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    let definitions = |rule_name: &str, options: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\n{rule_name} = rule(implementation = _impl{options})\n"
        )
    };
    let build_for = |rule_name: &str, target_name: &str| {
        format!("load(\":defs.bzl\", \"{rule_name}\")\n{rule_name}(name = \"{target_name}\")\n")
    };
    write(&defs, &definitions("plain_rule", ""));
    write(&build, &build_for("plain_rule", "ordinary_target"));

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let tracker = Arc::new(AttributeTracker::default());
    let expected = |bzl_module, package_load, consumer, observer| {
        vec![
            (
                AttributeKeyIdentity::BzlModuleEval(defs.clone()),
                bzl_module,
            ),
            (
                AttributeKeyIdentity::PackageLoad(package.clone()),
                package_load,
            ),
            (
                AttributeKeyIdentity::RuleCapabilityConsumer(package.clone()),
                consumer,
            ),
            (
                AttributeKeyIdentity::RuleCapabilityObserver(package.clone()),
                observer,
            ),
        ]
    };
    let load = |tracker: Arc<AttributeTracker>| {
        runtime.block_on(load_rule_capability_request(
            &dice,
            &workspace,
            &package,
            &[defs.clone()],
            tracker,
        ))
    };
    let capability = |projection: &RuleCapabilityProjection| {
        projection[0]
            .as_ref()
            .expect("Starlark rule capability")
            .clone()
    };

    let initial = load(tracker.clone()).unwrap();
    assert_eq!(capability(&initial).rule_class, "plain_rule");
    assert!(!capability(&initial).executable);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    write(&defs, &definitions("plain_rule", ", executable = True"));
    let executable = load(tracker.clone()).unwrap();
    assert_ne!(initial, executable);
    assert_eq!(capability(&executable).rule_class, "plain_rule");
    assert!(capability(&executable).executable);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    write(&defs, &definitions("plain_rule", ""));
    let non_executable = load(tracker.clone()).unwrap();
    assert_ne!(executable, non_executable);
    assert_eq!(capability(&non_executable).rule_class, "plain_rule");
    assert!(!capability(&non_executable).executable);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    write(
        &defs,
        &definitions("plain_rule_test", ", test = True, executable = False"),
    );
    write(&build, &build_for("plain_rule_test", "ordinary_target"));
    let test_rule = load(tracker.clone()).unwrap();
    assert_ne!(non_executable, test_rule);
    assert_eq!(capability(&test_rule).rule_class, "plain_rule_test");
    assert!(capability(&test_rule).executable);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    write(&defs, &definitions("renamed_rule", ", executable = True"));
    write(&build, &build_for("renamed_rule", "ordinary_target"));
    let renamed = load(tracker.clone()).unwrap();
    assert_ne!(test_rule, renamed);
    assert_eq!(capability(&renamed).rule_class, "renamed_rule");
    assert!(capability(&renamed).executable);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    let renamed_package = load_package(
        &Dice::builder().build(DetectCycles::Enabled),
        &runtime,
        &workspace,
        &package,
        &[defs.clone()],
    )
    .unwrap();
    write(&build, &build_for("renamed_rule", "target_test"));
    let target_renamed_package = load_package(
        &Dice::builder().build(DetectCycles::Enabled),
        &runtime,
        &workspace,
        &package,
        &[defs.clone()],
    )
    .unwrap();
    let target_renamed = load(tracker.clone()).unwrap();
    assert_ne!(renamed_package, target_renamed_package);
    assert_eq!(renamed, target_renamed);
    assert_eq!(capability(&renamed), capability(&target_renamed));
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Reused,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Reused,
        )
    );

    let formatted_definitions = "# formatting only\ndef _impl(ctx):\n    return [DefaultInfo()]\n\nrenamed_rule = rule( implementation = _impl, executable = True )\n";
    write(&defs, formatted_definitions);
    let formatted = load(tracker.clone()).unwrap();
    assert_eq!(target_renamed, formatted);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Reused,
        )
    );

    fs::remove_file(&defs).unwrap();
    assert!(load(tracker.clone()).is_err());
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
            LoadActivation::Evaluated,
        )
    );

    write(&defs, formatted_definitions);
    let recreated = load(tracker.clone()).unwrap();
    assert_eq!(formatted, recreated);
    assert_eq!(
        tracker.take(),
        expected(
            LoadActivation::Reused,
            LoadActivation::Evaluated,
            LoadActivation::Reused,
            LoadActivation::Reused,
        )
    );
}

#[test]
fn package_equality_ignores_distinct_frozen_module_handles() {
    let workspace = scratch("manifest-frozen-equality");
    let package = workspace.join("pkg");
    let defs = package.join("defs.bzl");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"loading\")\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );
    write(
        &defs,
        "def declare():\n    native.filegroup(name = \"same\", srcs = [])\n",
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let left_dice = Dice::builder().build(DetectCycles::Enabled);
    let right_dice = Dice::builder().build(DetectCycles::Enabled);
    let left = load_package(&left_dice, &runtime, &workspace, &package, &[defs.clone()]).unwrap();
    let right = load_package(&right_dice, &runtime, &workspace, &package, &[defs]).unwrap();
    assert_eq!(left, right);
}

async fn discover_companion(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<Option<slug_loading_v2::BuildFileCompanion>> {
    let mut updater = dice.updater();
    updater.changed_to(vec![(
        (WorkspaceDirectorySnapshotKey {
            workspace: workspace.to_path_buf(),
        }),
        Arc::new(directory_snapshot(workspace)),
    )])?;
    let mut transaction = updater.commit().await;
    BzlModuleEvaluator::new(workspace)?
        .discover_build_file_companion(&mut transaction, package)
        .await
}

#[test]
fn companion_lookup_uses_only_directory_observation_and_never_loads_build_contents() {
    let workspace = scratch("build-companion");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();

    write(&package.join("BUILD"), "this is broken BUILD syntax");
    let fallback = runtime
        .block_on(discover_companion(&dice, &workspace, &package))
        .unwrap();
    let fallback = fallback.expect("fallback BUILD is present");
    assert_eq!(fallback.label.to_string(), "@@//pkg:BUILD");
    assert_eq!(fallback.path, package.join("BUILD"));

    write(
        &package.join("BUILD.bazel"),
        "also broken, but never parsed",
    );
    let primary = runtime
        .block_on(discover_companion(&dice, &workspace, &package))
        .unwrap();
    let primary = primary.expect("primary BUILD is present");
    assert_eq!(primary.label.to_string(), "@@//pkg:BUILD.bazel");

    write(&workspace.join("BUILD.bazel"), "broken root BUILD syntax");
    let root_primary = runtime
        .block_on(discover_companion(&dice, &workspace, &workspace))
        .unwrap();
    assert_eq!(
        root_primary
            .expect("root BUILD is present")
            .label
            .to_string(),
        "@@//:BUILD.bazel"
    );

    fs::remove_file(package.join("BUILD.bazel")).unwrap();
    fs::remove_file(package.join("BUILD")).unwrap();
    assert!(
        runtime
            .block_on(discover_companion(&dice, &workspace, &package))
            .unwrap()
            .is_none()
    );

    runtime.block_on(async {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                },
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            (package.clone()),
                            WorkspaceDirectoryValue::ReadError(Arc::new("denied".to_owned())),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let error = BzlModuleEvaluator::new(&workspace)
            .unwrap()
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("denied"), "{error}");
    });
}

#[test]
fn companion_lookup_accepts_injected_symlinks_and_rejects_non_normalized_paths() {
    let workspace = scratch("build-companion-symlink");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                (WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                }),
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            package.clone(),
                            WorkspaceDirectoryValue::present(vec![WorkspaceDirectoryEntry {
                                name: "BUILD".into(),
                                kind: WorkspaceDirectoryEntryKind::Symlink,
                            }]),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let evaluator = BzlModuleEvaluator::new(&workspace).unwrap();
        let fallback = evaluator
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap()
            .expect("injected fallback symlink is eligible");
        assert_eq!(fallback.label.to_string(), "@@//pkg:BUILD");
        drop(transaction);

        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                (WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                }),
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(
                        vec![(
                            package.clone(),
                            WorkspaceDirectoryValue::present(vec![
                                WorkspaceDirectoryEntry {
                                    name: "BUILD".into(),
                                    kind: WorkspaceDirectoryEntryKind::RegularFile,
                                },
                                WorkspaceDirectoryEntry {
                                    name: "BUILD.bazel".into(),
                                    kind: WorkspaceDirectoryEntryKind::Symlink,
                                },
                            ]),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                }),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let primary = evaluator
            .discover_build_file_companion(&mut transaction, &package)
            .await
            .unwrap()
            .expect("injected primary symlink is eligible");
        assert_eq!(primary.label.to_string(), "@@//pkg:BUILD.bazel");

        for invalid in [
            PathBuf::from(format!("{}/.", package.display())),
            PathBuf::from(format!("{}/nested/..", package.display())),
        ] {
            let error = evaluator
                .discover_build_file_companion(&mut transaction, invalid)
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains("normalized absolute path"), "{error}");
        }
    });
}
