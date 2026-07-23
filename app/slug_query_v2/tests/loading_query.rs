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
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_query_v2::QueryOrder;
use slug_query_v2::SubtreePackageSetKey;
use slug_query_v2::UnconfiguredPackageGraphKey;
use slug_query_v2::evaluate_loading_query;

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-query-v2-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
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

async fn transaction(dice: &Arc<Dice>, workspace: &Path) -> dice::DiceTransaction {
    let (files, directories) = observations(workspace);
    let mut updater = dice.updater();
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
    updater.commit().await
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum QueryKeyIdentity {
    Package(PathBuf),
    SubtreePackageSet(PathBuf),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum ActivationKind {
    Evaluated,
    Reused,
}

#[derive(Default)]
struct QueryTracker {
    events: Mutex<Vec<(QueryKeyIdentity, ActivationKind)>>,
}

impl QueryTracker {
    fn take(&self) -> Vec<(QueryKeyIdentity, ActivationKind)> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl ActivationTracker for QueryTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        activation: ActivationData,
    ) {
        let identity = key
            .downcast_ref::<UnconfiguredPackageGraphKey>()
            .map(|key| QueryKeyIdentity::Package(key.package.clone()))
            .or_else(|| {
                key.downcast_ref::<SubtreePackageSetKey>()
                    .map(|key| QueryKeyIdentity::SubtreePackageSet(key.prefix.clone()))
            });
        if let Some(identity) = identity {
            let kind = match activation {
                ActivationData::Evaluated(_) => ActivationKind::Evaluated,
                ActivationData::Reused => ActivationKind::Reused,
            };
            self.events.lock().unwrap().push((identity, kind));
        }
    }
}

async fn query_revision(
    dice: &Arc<Dice>,
    tracker: &Arc<QueryTracker>,
    workspace: &Path,
    expression: &str,
) -> (
    Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError>,
    Vec<(QueryKeyIdentity, ActivationKind)>,
) {
    query_revision_order(dice, tracker, workspace, expression, QueryOrder::Auto).await
}

async fn query_revision_order(
    dice: &Arc<Dice>,
    tracker: &Arc<QueryTracker>,
    workspace: &Path,
    expression: &str,
    order: QueryOrder,
) -> (
    Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError>,
    Vec<(QueryKeyIdentity, ActivationKind)>,
) {
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
    let result =
        evaluate_loading_query(&mut transaction, workspace.to_path_buf(), expression, order).await;
    let mut events = tracker.take();
    events.sort();
    (result, events)
}

fn package(package: &str, kind: ActivationKind) -> (QueryKeyIdentity, ActivationKind) {
    (QueryKeyIdentity::Package(PathBuf::from(package)), kind)
}

fn subtree(prefix: &str, kind: ActivationKind) -> (QueryKeyIdentity, ActivationKind) {
    (
        QueryKeyIdentity::SubtreePackageSet(PathBuf::from(prefix)),
        kind,
    )
}

#[tokio::test]
async fn some_selects_once_in_set_order_and_signed_depths_are_empty() {
    let workspace = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/query-some-selection/workspace"),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, order, expected) in [
        ("some(//:single)", QueryOrder::Auto, "//:single\n"),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            QueryOrder::Auto,
            "//:alpha\n//:zeta\n",
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            QueryOrder::Full,
            "//:zeta\n//:alpha\n",
        ),
        (
            "set(//:zeta //:alpha //:middle)",
            QueryOrder::Full,
            "//:zeta\n//:middle\n//:alpha\n",
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 3)",
            QueryOrder::Full,
            "//:zeta\n//:middle\n//:alpha\n",
        ),
        (
            "some(set(//:zeta //:zeta //:alpha), 2)",
            QueryOrder::Full,
            "//:zeta\n//:alpha\n",
        ),
        (
            "some(//recursive/..., 2)",
            QueryOrder::Full,
            "//recursive/nested:rec_alpha\n//recursive:rec_zeta\n",
        ),
        (
            "some(deps(//:cycle_a), 10)",
            QueryOrder::Auto,
            "//:cycle_a\n//:cycle_b\n",
        ),
        (
            "deps(//:depth_root)",
            QueryOrder::Full,
            "//:depth_root\n//:depth_child\n",
        ),
        (
            "deps(//:cycle_a)",
            QueryOrder::Full,
            "//:cycle_a\n//:cycle_b\n",
        ),
        ("deps(//:depth_root, '-1')", QueryOrder::Auto, ""),
        (
            "rdeps(//:depth_root, //:depth_child, '-2147483648')",
            QueryOrder::Auto,
            "",
        ),
        (
            "deps(//:depth_root, 2147483647)",
            QueryOrder::Auto,
            "//:depth_child\n//:depth_root\n",
        ),
    ] {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }

    for expression in ["some(set())", "some(//:single, 0)", "some(//:single, '-1')"] {
        let error = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap_err();
        assert_eq!(error.exit_code, 7, "{expression}: {error}");
        assert!(error.to_string().contains("argument set is empty"));
    }
}

#[tokio::test]
async fn some_has_exact_operand_prevalidation_and_signed_depth_key_demand() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("first/BUILD.bazel"),
        "filegroup(name = \"a\")\n",
    );
    write(
        workspace.join("second/BUILD.bazel"),
        "filegroup(name = \"b\")\n",
    );
    let tracker = Arc::new(QueryTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);

    let (selected, events) = query_revision(
        &dice,
        &tracker,
        &workspace,
        "some(set(//second:b //first:a), 1)",
    )
    .await;
    assert_eq!(selected.unwrap().stdout(), "//second:b\n");
    assert_eq!(
        events,
        [
            package("first", ActivationKind::Evaluated),
            package("second", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("first/BUILD.bazel"),
        "filegroup(name = \"a\")\nfilegroup(name = \"changed\")\n",
    );
    let (empty, events) = query_revision(&dice, &tracker, &workspace, "some(//first:a, 0)").await;
    assert!(
        empty
            .unwrap_err()
            .to_string()
            .contains("argument set is empty")
    );
    assert_eq!(events, [package("first", ActivationKind::Evaluated)]);

    let (invalid, events) =
        query_revision(&dice, &tracker, &workspace, "some(//second:b, 2147483648)").await;
    assert_eq!(invalid.unwrap_err().exit_code, 2);
    assert!(events.is_empty(), "{events:?}");

    write(
        workspace.join("BUILD.bazel"),
        "filegroup(name = \"cycle_a\", srcs = [\":cycle_b\"])\n\
         filegroup(name = \"cycle_b\", srcs = [\":cycle_a\"])\n",
    );
    write(
        workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    write(
        workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        workspace.join("dest/BUILD.bazel"),
        "filegroup(name = \"end\")\n",
    );
    write(
        workspace.join("recursive/BUILD.bazel"),
        "filegroup(name = \"outer\")\n",
    );
    write(
        workspace.join("recursive/nested/BUILD.bazel"),
        "filegroup(name = \"inner\")\n",
    );

    for (expression, expected_events) in [
        (
            "deps(//origin:top, '-1')",
            vec![package("origin", ActivationKind::Evaluated)],
        ),
        (
            "rdeps(//origin:top, //dest:end, '-1')",
            vec![
                package("dest", ActivationKind::Evaluated),
                package("mid", ActivationKind::Evaluated),
                package("origin", ActivationKind::Evaluated),
            ],
        ),
        (
            "some(deps(//:cycle_a), 5)",
            vec![package("", ActivationKind::Evaluated)],
        ),
        (
            "some(//recursive/..., 5)",
            vec![
                package("recursive", ActivationKind::Evaluated),
                package("recursive/nested", ActivationKind::Evaluated),
                subtree("recursive", ActivationKind::Evaluated),
            ],
        ),
    ] {
        let tracker = Arc::new(QueryTracker::default());
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let (result, events) = query_revision(&dice, &tracker, &workspace, expression).await;
        result.unwrap();
        assert_eq!(events, expected_events, "{expression}");
    }

    let tracker = Arc::new(QueryTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let (full, events) = query_revision_order(
        &dice,
        &tracker,
        &workspace,
        "some(//origin:top, 1)",
        QueryOrder::Full,
    )
    .await;
    assert_eq!(full.unwrap().stdout(), "//origin:top\n");
    assert_eq!(events, [package("origin", ActivationKind::Evaluated)]);
}

#[tokio::test]
async fn some_all_candidates_tracks_retained_create_rename_delete_recreate() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let candidates = workspace.join("cand/BUILD.bazel");
    write(&candidates, "filegroup(name = \"one\")\n");
    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"outside\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());
    let expression = "some(//cand:all, 10)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(initial.unwrap().labels.as_ref(), ["//cand:one"]);
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);

    let (identical, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(identical.unwrap().labels.as_ref(), ["//cand:one"]);
    assert_eq!(events, []);

    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"changed_outside\")\n",
    );
    let (unrelated, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(unrelated.unwrap().labels.as_ref(), ["//cand:one"]);
    assert_eq!(events, [package("cand", ActivationKind::Reused)]);

    write(
        &candidates,
        "filegroup(name = \"one\")\nfilegroup(name = \"two\")\n",
    );
    let (created, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        created.unwrap().labels.as_ref(),
        ["//cand:one", "//cand:two"]
    );
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);

    write(
        &candidates,
        "filegroup(name = \"one\")\nfilegroup(name = \"middle\")\n",
    );
    let (renamed, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        renamed.unwrap().labels.as_ref(),
        ["//cand:middle", "//cand:one"]
    );
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);

    write(&candidates, "filegroup(name = \"one\")\n");
    let (deleted, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(deleted.unwrap().labels.as_ref(), ["//cand:one"]);
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);

    write(
        &candidates,
        "filegroup(name = \"one\")\nfilegroup(name = \"zeta\")\n",
    );
    let (recreated, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        recreated.unwrap().labels.as_ref(),
        ["//cand:one", "//cand:zeta"]
    );
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);
}

// Source nodes belong to the package that owns their referring attribute.
// A cross-package edge must not synthesize its destination into the source
// package graph; traversal must demand the destination graph separately.
#[tokio::test]
async fn implicit_sources_remain_package_owned_across_filegroup_edges() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"local.txt\", \"//child:child.txt\"])\n",
    );
    write(
        workspace.join("child/BUILD.bazel"),
        "exports_files([\"child.txt\"])\n",
    );
    write(workspace.join("app/local.txt"), "local\n");
    write(workspace.join("child/child.txt"), "child\n");

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: PathBuf::from("app"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let labels = graph
        .nodes
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(labels, ["//app:bin", "//app:local.txt"]);

    let output = evaluate_loading_query(
        &mut transaction,
        workspace,
        "deps(//app:bin)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert_eq!(
        output.labels.as_ref(),
        ["//app:bin", "//app:local.txt", "//child:child.txt"]
    );
}

#[tokio::test]
async fn exact_query_key_activation_multisets_cover_retained_transitions() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"local.txt\", \"//lib:lib\"])\n",
    );
    write(workspace.join("app/local.txt"), "local\n");
    write(
        workspace.join("lib/BUILD.bazel"),
        "filegroup(name = \"lib\", srcs = [\"lib.txt\"])\n",
    );
    write(workspace.join("lib/lib.txt"), "lib\n");
    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"other\", srcs = [])\n",
    );

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());

    let (initial, events) = query_revision(&dice, &tracker, &workspace, "deps(//app:bin)").await;
    assert!(initial.is_ok());
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Evaluated),
            package("lib", ActivationKind::Evaluated),
        ]
    );

    let (identical, events) = query_revision(&dice, &tracker, &workspace, "deps(//app:bin)").await;
    assert_eq!(
        identical.unwrap().labels.as_ref(),
        initial.unwrap().labels.as_ref()
    );
    assert_eq!(events, []);

    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"other2\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, "deps(//app:bin)").await;
    // These are DICE dependency-validation callbacks, not graph evaluation.
    // The values cut propagation as Reused, so the unrelated package cannot
    // alter either requested package graph.
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Reused),
            package("lib", ActivationKind::Reused),
        ]
    );

    let (recursive, events) = query_revision(&dice, &tracker, &workspace, "//...").await;
    assert_eq!(
        recursive.unwrap().labels.as_ref(),
        ["//app:bin", "//lib:lib", "//unrelated:other2"]
    );
    assert_eq!(
        events,
        [
            package("unrelated", ActivationKind::Evaluated),
            (
                QueryKeyIdentity::SubtreePackageSet(PathBuf::new()),
                ActivationKind::Evaluated,
            ),
        ]
    );

    write(
        workspace.join("dynamic/BUILD.bazel"),
        "filegroup(name = \"fresh\", srcs = [])\n",
    );
    let (recursive_created, events) = query_revision(&dice, &tracker, &workspace, "//...").await;
    assert_eq!(
        recursive_created.unwrap().labels.as_ref(),
        [
            "//app:bin",
            "//dynamic:fresh",
            "//lib:lib",
            "//unrelated:other2",
        ]
    );
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Reused),
            package("dynamic", ActivationKind::Evaluated),
            package("lib", ActivationKind::Reused),
            package("unrelated", ActivationKind::Reused),
            (
                QueryKeyIdentity::SubtreePackageSet(PathBuf::new()),
                ActivationKind::Evaluated,
            ),
        ]
    );

    fs::remove_file(workspace.join("dynamic/BUILD.bazel")).unwrap();
    let (recursive_deleted, events) = query_revision(&dice, &tracker, &workspace, "//...").await;
    assert_eq!(
        recursive_deleted.unwrap().labels.as_ref(),
        ["//app:bin", "//lib:lib", "//unrelated:other2"]
    );
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Reused),
            package("lib", ActivationKind::Reused),
            package("unrelated", ActivationKind::Reused),
            (
                QueryKeyIdentity::SubtreePackageSet(PathBuf::new()),
                ActivationKind::Evaluated,
            ),
        ]
    );

    write(
        workspace.join("dynamic/BUILD.bazel"),
        "filegroup(name = \"reborn\", srcs = [])\n",
    );
    let (recursive_recreated, events) = query_revision(&dice, &tracker, &workspace, "//...").await;
    assert_eq!(
        recursive_recreated.unwrap().labels.as_ref(),
        [
            "//app:bin",
            "//dynamic:reborn",
            "//lib:lib",
            "//unrelated:other2",
        ]
    );
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Reused),
            package("dynamic", ActivationKind::Evaluated),
            package("lib", ActivationKind::Reused),
            package("unrelated", ActivationKind::Reused),
            (
                QueryKeyIdentity::SubtreePackageSet(PathBuf::new()),
                ActivationKind::Evaluated,
            ),
        ]
    );

    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"local.txt\", \"new.txt\"])\n",
    );
    write(workspace.join("app/new.txt"), "new\n");
    let (changed, events) = query_revision(&dice, &tracker, &workspace, "deps(//app:bin)").await;
    assert_eq!(
        changed.unwrap().labels.as_ref(),
        ["//app:bin", "//app:local.txt", "//app:new.txt"]
    );
    assert_eq!(events, [package("app", ActivationKind::Evaluated)]);

    let missing_expression = "deps(//created:item)";
    let (missing, events) = query_revision(&dice, &tracker, &workspace, missing_expression).await;
    assert!(missing.unwrap_err().to_string().contains("no such package"));
    assert_eq!(events, [package("created", ActivationKind::Evaluated)]);

    write(
        workspace.join("created/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"item.txt\"])\n",
    );
    write(workspace.join("created/item.txt"), "created\n");
    let (created, events) = query_revision(&dice, &tracker, &workspace, missing_expression).await;
    assert!(created.is_ok());
    assert_eq!(events, [package("created", ActivationKind::Evaluated)]);

    fs::remove_file(workspace.join("created/BUILD.bazel")).unwrap();
    let (deleted, events) = query_revision(&dice, &tracker, &workspace, missing_expression).await;
    assert!(deleted.unwrap_err().to_string().contains("no such package"));
    assert_eq!(events, [package("created", ActivationKind::Evaluated)]);

    write(
        workspace.join("created/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"other.txt\"])\n",
    );
    write(workspace.join("created/other.txt"), "other\n");
    let (recreated, events) = query_revision(&dice, &tracker, &workspace, missing_expression).await;
    assert_eq!(
        recreated.unwrap().labels.as_ref(),
        ["//created:item", "//created:other.txt"]
    );
    assert_eq!(events, [package("created", ActivationKind::Evaluated)]);
}

#[tokio::test]
async fn subtree_rdeps_and_same_package_reverse_queries_match_bazel_oracle() {
    let workspace = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/query-rdeps-and-subtree-patterns/workspace"),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    let cases = [
        (
            "//tree/left/...",
            QueryOrder::Auto,
            "//tree/left:cross_only\n//tree/left:custom_parent\n//tree/left:cycle_a\n//tree/left:cycle_b\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:via_alias\n//tree/left/nested:nested\n",
        ),
        (
            "//nonpackage/...",
            QueryOrder::Auto,
            "//nonpackage/desc:desc\n",
        ),
        (
            "rdeps(//..., //tree/left:source.txt)",
            QueryOrder::Auto,
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            "rdeps(//..., //tree/left:source.txt, 0)",
            QueryOrder::Auto,
            "//tree/left:source.txt\n",
        ),
        (
            "rdeps(//..., //tree/left:source.txt, 1)",
            QueryOrder::Auto,
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            "rdeps(//..., //tree/left:source.txt, 2)",
            QueryOrder::Auto,
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            "rdeps(//tree/right:right_parent, //tree/left:source.txt)",
            QueryOrder::Auto,
            "",
        ),
        (
            "rdeps(set(//tree/left:parent_one //tree/right:right_parent), //tree/left:source.txt)",
            QueryOrder::Auto,
            "//tree/left:parent_one\n//tree/left:source.txt\n",
        ),
        (
            "rdeps(//tree/left:cycle_a, //tree/left:cycle_b)",
            QueryOrder::Auto,
            "//tree/left:cycle_a\n//tree/left:cycle_b\n",
        ),
        (
            "rdeps(//tree/..., //tree/left:leaf)",
            QueryOrder::Full,
            "//tree/left:custom_parent\n//tree/left:via_alias\n//tree/left:leaf\n",
        ),
        (
            "same_pkg_direct_rdeps(//tree/left:source.txt)",
            QueryOrder::Auto,
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n",
        ),
        (
            "same_pkg_direct_rdeps(set(//tree/left:source.txt //tree/right:right_source.txt))",
            QueryOrder::Auto,
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/right:right_both\n//tree/right:right_parent\n",
        ),
    ];
    for (expression, order, expected) in cases {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }

    for (expression, expected) in [
        ("//empty/...", "no targets found beneath 'empty'"),
        ("//missing/...", "no targets found beneath 'missing'"),
        (
            "rdeps(//..., 1)",
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            "same_pkg_direct_rdeps(1)",
            "no such target '//:1': target '1' not declared in package ''",
        ),
    ] {
        let error = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap_err();
        assert_eq!(error.exit_code, 7, "{expression}: {error}");
        assert!(
            error.to_string().contains(expected),
            "{expression}: {error}"
        );
    }
}

#[tokio::test]
async fn path_queries_share_topology_and_apply_only_root_somepath_auto_exception() {
    let workspace = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/query-path-topology/workspace"),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, order, expected) in [
        (
            "somepath(//:linear_start, //:linear_end)",
            QueryOrder::Auto,
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
        (
            "(somepath(//:linear_start, //:linear_end))",
            QueryOrder::Auto,
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
        (
            "somepath(//:linear_start, //:linear_end)",
            QueryOrder::Full,
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
        (
            "somepath(//:linear_start, //:linear_end) union //:disconnected",
            QueryOrder::Auto,
            "//:disconnected\n//:linear_end\n//:linear_mid\n//:linear_start\n",
        ),
        (
            "somepath(//:linear_start, //:linear_end) intersect set(//:linear_start //:linear_end)",
            QueryOrder::Auto,
            "//:linear_end\n//:linear_start\n",
        ),
        (
            "somepath(//:linear_start, //:linear_end) except //:linear_mid",
            QueryOrder::Auto,
            "//:linear_end\n//:linear_start\n",
        ),
        (
            "let p = somepath(//:linear_start, //:linear_end) in $p",
            QueryOrder::Auto,
            "//:linear_end\n//:linear_mid\n//:linear_start\n",
        ),
        (
            "allpaths(//:linear_start, //:linear_end)",
            QueryOrder::Auto,
            "//:linear_end\n//:linear_mid\n//:linear_start\n",
        ),
        (
            "allpaths(//:linear_start, //:linear_end)",
            QueryOrder::Full,
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
        (
            "allpaths(//:diamond_start, //:diamond_end)",
            QueryOrder::Auto,
            "//:diamond_end\n//:diamond_left\n//:diamond_right\n//:diamond_split\n//:diamond_start\n",
        ),
        (
            "somepath(//:cycle_a, //:cycle_end)",
            QueryOrder::Full,
            "//:cycle_a\n//:cycle_b\n//:cycle_end\n",
        ),
        (
            "somepath(//:linear_mid, //:linear_mid)",
            QueryOrder::Auto,
            "//:linear_mid\n",
        ),
        (
            "somepath(//:linear_start, //:disconnected)",
            QueryOrder::Auto,
            "",
        ),
    ] {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.stdout(), expected, "{expression} ({order})");
    }
}

#[tokio::test]
async fn reverse_query_keys_have_prefix_and_operand_local_activation_multisets() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("tree/base/BUILD.bazel"),
        "filegroup(name = \"base\", srcs = [])\n",
    );
    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"outside\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());

    let (initial, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(initial.unwrap().labels.as_ref(), ["//tree/base:base"]);
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Evaluated),
            subtree("tree", ActivationKind::Evaluated),
        ]
    );

    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(events, []);

    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"changed\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(events, [package("tree/base", ActivationKind::Reused)]);

    write(
        workspace.join("outside/new/BUILD.bazel"),
        "filegroup(name = \"new\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            subtree("tree", ActivationKind::Reused),
        ]
    );

    fs::remove_file(workspace.join("outside/new/BUILD.bazel")).unwrap();
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            subtree("tree", ActivationKind::Reused),
        ]
    );

    write(
        workspace.join("outside/new/BUILD.bazel"),
        "filegroup(name = \"reborn\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            subtree("tree", ActivationKind::Reused),
        ]
    );

    write(
        workspace.join("tree/dynamic/BUILD.bazel"),
        "filegroup(name = \"fresh\", srcs = [])\n",
    );
    let (created, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        created.unwrap().labels.as_ref(),
        ["//tree/base:base", "//tree/dynamic:fresh"]
    );
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            package("tree/dynamic", ActivationKind::Evaluated),
            subtree("tree", ActivationKind::Evaluated),
        ]
    );

    fs::remove_file(workspace.join("tree/dynamic/BUILD.bazel")).unwrap();
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            subtree("tree", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("tree/dynamic/BUILD.bazel"),
        "filegroup(name = \"reborn\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, "//tree/...").await;
    assert_eq!(
        events,
        [
            package("tree/base", ActivationKind::Reused),
            package("tree/dynamic", ActivationKind::Evaluated),
            subtree("tree", ActivationKind::Evaluated),
        ]
    );

    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//leaf:item\"])\n",
    );
    write(
        workspace.join("leaf/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"item.txt\"])\n",
    );
    write(workspace.join("leaf/item.txt"), "leaf\n");
    write(
        workspace.join("other/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"item.txt\"])\n",
    );
    write(workspace.join("other/item.txt"), "other\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());
    let expression = "rdeps(//app:top, //leaf:item)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        initial.unwrap().labels.as_ref(),
        ["//app:top", "//leaf:item"]
    );
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Evaluated),
            package("leaf", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//other:item\"])\n",
    );
    let (lost, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(lost.unwrap().labels.is_empty());
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Evaluated),
            package("leaf", ActivationKind::Reused),
            package("other", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//leaf:item\"])\n",
    );
    let (regained, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        regained.unwrap().labels.as_ref(),
        ["//app:top", "//leaf:item"]
    );
    assert_eq!(
        events,
        [
            package("app", ActivationKind::Evaluated),
            package("leaf", ActivationKind::Reused),
        ]
    );

    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("left/BUILD.bazel"),
        "exports_files([\"source.txt\"])\nfilegroup(name = \"local\", srcs = [\":source.txt\"])\n",
    );
    write(workspace.join("left/source.txt"), "left\n");
    write(
        workspace.join("right/BUILD.bazel"),
        "filegroup(name = \"cross\", srcs = [\"//left:source.txt\"])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());
    let expression = "same_pkg_direct_rdeps(//left:source.txt)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(initial.unwrap().labels.as_ref(), ["//left:local"]);
    assert_eq!(events, [package("left", ActivationKind::Evaluated)]);

    write(
        workspace.join("right/BUILD.bazel"),
        "filegroup(name = \"changed_cross\", srcs = [\"//left:source.txt\"])\n",
    );
    let (unchanged, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(unchanged.unwrap().labels.as_ref(), ["//left:local"]);
    assert_eq!(events, [package("left", ActivationKind::Reused)]);
}

#[tokio::test]
async fn path_queries_reuse_dice_graphs_and_invalidate_only_demanded_closure() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    write(
        workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        workspace.join("dest/BUILD.bazel"),
        "filegroup(name = \"end\", srcs = [])\n",
    );
    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"target\", srcs = [])\n",
    );
    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"unrelated\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());
    let expression = "allpaths(//origin:top, //dest:end)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        initial.unwrap().labels.as_ref(),
        ["//dest:end", "//mid:item", "//origin:top"]
    );
    assert_eq!(
        events,
        [
            package("dest", ActivationKind::Evaluated),
            package("mid", ActivationKind::Evaluated),
            package("origin", ActivationKind::Evaluated),
        ]
    );

    let (identical, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        identical.unwrap().labels.as_ref(),
        ["//dest:end", "//mid:item", "//origin:top"]
    );
    assert_eq!(events, []);

    write(
        workspace.join("unrelated/BUILD.bazel"),
        "filegroup(name = \"changed\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        events,
        [
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Reused),
            package("origin", ActivationKind::Reused),
        ]
    );

    write(
        workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [])\n",
    );
    let (lost, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(lost.unwrap().labels.is_empty());
    assert_eq!(
        events,
        [
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Evaluated),
            package("origin", ActivationKind::Reused),
        ]
    );

    write(
        workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    let (restored, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        restored.unwrap().labels.as_ref(),
        ["//dest:end", "//mid:item", "//origin:top"]
    );
    assert_eq!(
        events,
        [
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Evaluated),
            package("origin", ActivationKind::Reused),
        ]
    );

    write(
        workspace.join("branch/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\", \"//branch:item\"])\n",
    );
    let (gained, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        gained.unwrap().labels.as_ref(),
        ["//branch:item", "//dest:end", "//mid:item", "//origin:top",]
    );
    assert_eq!(
        events,
        [
            package("branch", ActivationKind::Evaluated),
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Reused),
            package("origin", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    let (removed, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        removed.unwrap().labels.as_ref(),
        ["//dest:end", "//mid:item", "//origin:top"]
    );
    assert_eq!(
        events,
        [
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Reused),
            package("origin", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\", \"//branch:item\"])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        events,
        [
            package("branch", ActivationKind::Reused),
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Reused),
            package("origin", ActivationKind::Evaluated),
        ]
    );

    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"changed_outside\", srcs = [])\n",
    );
    let (_, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        events,
        [
            package("branch", ActivationKind::Reused),
            package("dest", ActivationKind::Reused),
            package("mid", ActivationKind::Reused),
            package("origin", ActivationKind::Reused),
        ]
    );

    let outside_dice = Dice::builder().build(DetectCycles::Enabled);
    let outside_tracker = Arc::new(QueryTracker::default());
    let (outside, events) = query_revision(
        &outside_dice,
        &outside_tracker,
        &workspace,
        "somepath(//origin:top, //outside:changed_outside)",
    )
    .await;
    assert!(outside.unwrap().labels.is_empty());
    assert_eq!(
        events,
        [
            package("branch", ActivationKind::Evaluated),
            package("dest", ActivationKind::Evaluated),
            package("mid", ActivationKind::Evaluated),
            package("origin", ActivationKind::Evaluated),
            package("outside", ActivationKind::Evaluated),
        ]
    );
    assert!(
        events
            .iter()
            .all(|(identity, _)| !matches!(identity, QueryKeyIdentity::SubtreePackageSet(_)))
    );
}
