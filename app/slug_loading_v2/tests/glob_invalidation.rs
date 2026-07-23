use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::UserComputationData;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::keys::PackageListingKey;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Identity {
    Listing(PathBuf),
    Load(PathBuf),
    Directory(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    Evaluated,
    Reused,
}

#[derive(Default)]
struct Tracker {
    events: Mutex<Vec<(Identity, EventKind)>>,
}

impl Tracker {
    fn take(&self) -> Vec<(Identity, EventKind)> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl ActivationTracker for Tracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        activation_data: ActivationData,
    ) {
        let identity = if let Some(key) = key.downcast_ref::<PackageListingKey>() {
            Some(Identity::Listing(key.package.clone()))
        } else if let Some(key) = key.downcast_ref::<PackageLoadKey>() {
            Some(Identity::Load(key.package.clone()))
        } else {
            key.downcast_ref::<WorkspaceDirectoryKey>()
                .map(|key| Identity::Directory(key.directory.clone()))
        };
        if let Some(identity) = identity {
            let kind = match activation_data {
                ActivationData::Evaluated(_) => EventKind::Evaluated,
                ActivationData::Reused => EventKind::Reused,
            };
            self.events.lock().unwrap().push((identity, kind));
        }
    }
}

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-glob-invalidation-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn observations(root: &Path) -> (WorkspaceSnapshot, WorkspaceDirectorySnapshot) {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let file_type = entry.file_type().unwrap();
            let kind = if file_type.is_file() {
                files.push((
                    path,
                    WorkspaceFileValue::Present(Arc::new(
                        fs::read_to_string(entry.path()).unwrap(),
                    )),
                ));
                WorkspaceDirectoryEntryKind::RegularFile
            } else if file_type.is_dir() {
                pending.push(path);
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
    (
        WorkspaceSnapshot {
            files: Arc::new(files.into_iter().collect()),
        },
        WorkspaceDirectorySnapshot {
            directories: Arc::new(directories.into_iter().collect()),
        },
    )
}

async fn load_revision(
    dice: &Arc<Dice>,
    tracker: &Arc<Tracker>,
    evaluator: &BzlModuleEvaluator,
    workspace: &Path,
    package: &Path,
) -> (Vec<String>, Vec<(Identity, EventKind)>) {
    let (files, directories) = observations(workspace);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    });
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            Arc::new(files),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            Arc::new(directories),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction
        .compute(&PackageListingKey {
            workspace: workspace.to_path_buf(),
            package: package.to_path_buf(),
        })
        .await
        .unwrap();
    let loaded = evaluator
        .evaluate_package(&mut transaction, package)
        .await
        .unwrap();
    let srcs = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::Filegroup { srcs } if target.name == "all" => Some(
                srcs.iter()
                    .map(|label| label.target().to_string())
                    .collect(),
            ),
            _ => None,
        })
        .unwrap();
    (srcs, tracker.take())
}

fn assert_once(events: &[(Identity, EventKind)], identity: Identity, kind: EventKind) {
    assert_eq!(
        events
            .iter()
            .filter(|event| event == &&(identity.clone(), kind))
            .count(),
        1,
        "events: {events:#?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|(event_identity, _)| event_identity == &identity)
            .count(),
        1,
        "events: {events:#?}"
    );
}

fn assert_not_activated(events: &[(Identity, EventKind)], identity: Identity) {
    assert!(
        !events
            .iter()
            .any(|(event_identity, _)| event_identity == &identity),
        "{identity:?} was unexpectedly activated: {events:#?}"
    );
}

#[tokio::test]
async fn retained_dice_reuses_or_recomputes_globs_at_directory_boundaries() {
    let workspace = scratch();
    let package = workspace.join("pkg");
    let child = package.join("child");
    write(&workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        &package.join("BUILD.bazel"),
        "filegroup(name = \"all\", srcs = glob([\"*.txt\", \"child/*.txt\"], allow_empty = True))\n",
    );
    write(&package.join("keep.txt"), "keep\n");
    write(&child.join("child.txt"), "child\n");
    write(&package.join("subpackage/BUILD.bazel"), "# boundary\n");
    write(&package.join("subpackage/hidden.txt"), "first\n");
    fs::create_dir_all(workspace.join("sibling")).unwrap();

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(Tracker::default());
    let evaluator = BzlModuleEvaluator::new(&workspace).unwrap();

    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["child/child.txt", "keep.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Load(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Directory(package.clone()),
        EventKind::Evaluated,
    );

    let (_, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_not_activated(&events, Identity::Listing(package.clone()));
    assert_not_activated(&events, Identity::Load(package.clone()));

    write(&workspace.join("sibling/unrelated.txt"), "unrelated\n");
    let (_, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Reused,
    );
    assert_once(&events, Identity::Load(package.clone()), EventKind::Reused);

    write(
        &package.join("subpackage/hidden.txt"),
        "edited below boundary\n",
    );
    let (_, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_not_activated(&events, Identity::Listing(package.clone()));
    assert_once(&events, Identity::Load(package.clone()), EventKind::Reused);
    assert!(
        !events
            .iter()
            .any(|(identity, _)| { identity == &Identity::Directory(package.join("subpackage")) }),
        "subpackage descendants must not be observed: {events:#?}"
    );

    write(&package.join("created.txt"), "created\n");
    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["child/child.txt", "created.txt", "keep.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Load(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Directory(package.clone()),
        EventKind::Evaluated,
    );

    fs::rename(package.join("created.txt"), package.join("renamed.txt")).unwrap();
    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["child/child.txt", "keep.txt", "renamed.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Load(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Directory(package.clone()),
        EventKind::Evaluated,
    );

    fs::remove_file(package.join("renamed.txt")).unwrap();
    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["child/child.txt", "keep.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Load(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Directory(package.clone()),
        EventKind::Evaluated,
    );

    write(&child.join("BUILD.bazel"), "# new boundary\n");
    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["keep.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Load(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(
        &events,
        Identity::Directory(child.clone()),
        EventKind::Evaluated,
    );

    fs::remove_file(child.join("BUILD.bazel")).unwrap();
    let (srcs, events) = load_revision(&dice, &tracker, &evaluator, &workspace, &package).await;
    assert_eq!(srcs, ["child/child.txt", "keep.txt"]);
    assert_once(
        &events,
        Identity::Listing(package.clone()),
        EventKind::Evaluated,
    );
    assert_once(&events, Identity::Load(package), EventKind::Evaluated);
    assert_once(&events, Identity::Directory(child), EventKind::Evaluated);
}

async fn listing_error(workspace: &Path, package: &Path, value: WorkspaceDirectoryValue) -> String {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceDirectorySnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            Arc::new(WorkspaceDirectorySnapshot {
                directories: Arc::new([(package.to_path_buf(), value)].into_iter().collect()),
            }),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let value = transaction
        .compute(&PackageListingKey {
            workspace: workspace.to_path_buf(),
            package: package.to_path_buf(),
        })
        .await
        .unwrap();
    value.as_ref().as_ref().unwrap_err().to_string()
}

#[tokio::test]
async fn listing_fails_explicitly_for_absence_read_errors_and_symlinks() {
    let workspace = scratch();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();

    let absent = listing_error(&workspace, &package, WorkspaceDirectoryValue::Absent).await;
    assert!(absent.contains("package directory is absent"));

    let read_error = listing_error(
        &workspace,
        &package,
        WorkspaceDirectoryValue::ReadError(Arc::new("denied".to_owned())),
    )
    .await;
    assert!(read_error.contains("denied"));

    let symlink = listing_error(
        &workspace,
        &package,
        WorkspaceDirectoryValue::present(vec![WorkspaceDirectoryEntry {
            name: "link.txt".into(),
            kind: WorkspaceDirectoryEntryKind::Symlink,
        }]),
    )
    .await;
    assert!(symlink.contains("symlink entries are unsupported"));
}
