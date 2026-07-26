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
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::VisibilitySource;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_query_v2::QueryEdgeKind;
use slug_query_v2::QueryNodeKind;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryPolicy;
use slug_query_v2::SubtreePackageSetKey;
use slug_query_v2::UnconfiguredPackageGraphKey;
use slug_query_v2::evaluate_loading_query;
use slug_query_v2::evaluate_loading_query_with_policy;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;

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

async fn transaction(dice: &Arc<Dice>, workspace: &Path) -> dice::DiceTransaction {
    let (files, directories) = observations(workspace);
    let raw_files = raw_snapshot_from_text(&files);
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
            WorkspaceRawSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            raw_files,
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
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
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
    query_revision_order_with_policy(
        dice,
        tracker,
        workspace,
        expression,
        order,
        QueryPolicy::default(),
    )
    .await
}

async fn query_revision_order_with_policy(
    dice: &Arc<Dice>,
    tracker: &Arc<QueryTracker>,
    workspace: &Path,
    expression: &str,
    order: QueryOrder,
    policy: QueryPolicy,
) -> (
    Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError>,
    Vec<(QueryKeyIdentity, ActivationKind)>,
) {
    let (files, directories) = observations(workspace);
    let raw_files = raw_snapshot_from_text(&files);
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
            WorkspaceRawSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            raw_files,
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
    inject_root_module_request_inputs(
        &mut updater,
        workspace,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let result = evaluate_loading_query_with_policy(
        &mut transaction,
        workspace.to_path_buf(),
        expression,
        order,
        policy,
    )
    .await;
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
async fn tests_function_source_critical_discriminators_and_strict_policy_are_request_local() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/tests-query-expansion/workspace")
        .canonicalize()
        .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        (
            "tests(//source_critical:parent_requires_parent_tag)",
            "//source_critical:nested_unfiltered_test\n",
        ),
        (
            "tests(//source_critical:filtered_direct_then_nested)",
            "//source_critical:shared_blocked_test\n",
        ),
        (
            "tests(//source_critical:exclude_literal_plus_tag)",
            "//source_critical:plain_tag_test\n",
        ),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }

    let default = evaluate_loading_query(
        &mut transaction,
        workspace.clone(),
        "tests(//strict:non_test_member)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert!(default.stdout().is_empty());

    let strict = evaluate_loading_query_with_policy(
        &mut transaction,
        workspace,
        "tests(//strict:non_test_member)",
        QueryOrder::Auto,
        QueryPolicy {
            strict_test_suite: true,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        strict.to_string(),
        "The label '//strict:plain' in the test_suite '//strict:non_test_member' does not refer to a test or test_suite rule!"
    );
}

#[tokio::test]
async fn tests_function_matches_all_twenty_one_non_build_oracle_rows() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/tests-query-expansion/workspace")
        .canonicalize()
        .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let successes: &[(&str, QueryOrder, &[&str])] = &[
        (
            "tests(set(//direct:direct_test //direct:plain))",
            QueryOrder::Auto,
            &["//direct:direct_test"],
        ),
        (
            "tests(//implicit:empty)",
            QueryOrder::Auto,
            &["//implicit:alpha_test", "//implicit:large_test"],
        ),
        (
            "tests(//explicit:root_suite)",
            QueryOrder::Auto,
            &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
        ),
        (
            "tests(//explicit:only_direct)",
            QueryOrder::Auto,
            &["//explicit:direct_test"],
        ),
        ("tests(//cycle_a:a)", QueryOrder::Auto, &[]),
        (
            "tests(//dedup:root)",
            QueryOrder::Auto,
            &["//dedup:shared_test"],
        ),
        (
            "tests(//filters:bare)",
            QueryOrder::Auto,
            &["//filters:fast_test"],
        ),
        (
            "tests(//filters:plus)",
            QueryOrder::Auto,
            &["//filters:fast_test"],
        ),
        (
            "tests(//filters:exclude_slow)",
            QueryOrder::Auto,
            &["//filters:fast_test"],
        ),
        (
            "tests(//filters:manual_suite)",
            QueryOrder::Auto,
            &["//filters:plain_test"],
        ),
        (
            "tests(//filters:large)",
            QueryOrder::Auto,
            &["//filters:large_test"],
        ),
        ("tests(//strict:non_test_member)", QueryOrder::Auto, &[]),
        (
            "tests(//explicit:root_suite)",
            QueryOrder::Auto,
            &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
        ),
        (
            "tests(//explicit:root_suite)",
            QueryOrder::Full,
            &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
        ),
        (
            "tests(//provenance:omitted)",
            QueryOrder::Auto,
            &["//provenance:member_test"],
        ),
        (
            "tests(//provenance:explicit_empty)",
            QueryOrder::Auto,
            &["//provenance:member_test"],
        ),
        (
            "tests(//source_critical:parent_requires_parent_tag)",
            QueryOrder::Auto,
            &["//source_critical:nested_unfiltered_test"],
        ),
        (
            "tests(//source_critical:filtered_direct_then_nested)",
            QueryOrder::Auto,
            &["//source_critical:shared_blocked_test"],
        ),
        (
            "tests(//source_critical:exclude_literal_plus_tag)",
            QueryOrder::Auto,
            &["//source_critical:plain_tag_test"],
        ),
    ];
    assert_eq!(successes.len(), 19);
    for (expression, order, expected) in successes {
        let output =
            evaluate_loading_query(&mut transaction, workspace.clone(), expression, *order)
                .await
                .unwrap();
        let mut actual = output
            .labels
            .iter()
            .map(AsRef::<str>::as_ref)
            .collect::<Vec<_>>();
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{expression}");
    }

    let strict = evaluate_loading_query_with_policy(
        &mut transaction,
        workspace.clone(),
        "tests(//strict:non_test_member)",
        QueryOrder::Auto,
        QueryPolicy {
            strict_test_suite: true,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(strict.exit_code, 7);
    assert_eq!(
        strict.to_string(),
        "The label '//strict:plain' in the test_suite '//strict:non_test_member' does not refer to a test or test_suite rule!"
    );

    let missing = evaluate_loading_query(
        &mut transaction,
        workspace,
        "tests(//missing:broken)",
        QueryOrder::Auto,
    )
    .await
    .unwrap_err();
    assert_eq!(missing.exit_code, 7);
    assert_eq!(
        missing.to_string(),
        "couldn't expand 'tests' attribute of test_suite //missing:broken: no such target '//missing_target:missing_target': target 'missing_target' not declared in package 'missing_target'"
    );
}

#[tokio::test]
async fn tests_function_keeps_fake_and_top_level_other_targets_outside_strict_lookup() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(workspace.join("pkg/defs.bzl"), "VALUE = 1\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"VALUE\")\nfilegroup(name = \"plain\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    for expression in [
        "tests(loadfiles(//pkg:plain))",
        "tests(//pkg:plain)",
        "tests(set(//pkg:plain //pkg:BUILD.bazel))",
    ] {
        let output = evaluate_loading_query_with_policy(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
            QueryPolicy {
                strict_test_suite: true,
            },
        )
        .await
        .unwrap();
        assert!(output.stdout().is_empty(), "{expression}");
    }
}

#[tokio::test]
async fn tests_named_attribute_resolution_records_suite_evaluation_edges() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/tests-query-expansion/workspace")
        .canonicalize()
        .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let output = evaluate_loading_query(
        &mut transaction,
        workspace,
        "tests(//explicit:root_suite) union set(//explicit:root_suite //explicit:nested_suite //cross:cross_suite)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    let graph = output.graph_stdout(false, true);
    for edge in [
        "\"//explicit:root_suite\" -> \"//explicit:direct_test\"",
        "\"//explicit:root_suite\" -> \"//explicit:nested_suite\"",
        "\"//explicit:root_suite\" -> \"//cross:cross_suite\"",
        "\"//explicit:nested_suite\" -> \"//explicit:nested_test\"",
        "\"//cross:cross_suite\" -> \"//cross:cross_test\"",
    ] {
        assert!(graph.contains(edge), "{edge} missing from:\n{graph}");
    }
    assert!(!graph.contains("unlisted_test"), "{graph}");
}

#[tokio::test]
async fn strict_policy_toggle_reuses_the_same_unconfigured_package_graph() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"plain\")\ntest_suite(name = \"suite\", tests = [\":plain\"])\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let expression = "tests(//pkg:suite)";

    let (default, events) = query_revision_order_with_policy(
        &dice,
        &tracker,
        &workspace,
        expression,
        QueryOrder::Auto,
        QueryPolicy::default(),
    )
    .await;
    assert!(default.unwrap().stdout().is_empty());
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    let (strict, events) = query_revision_order_with_policy(
        &dice,
        &tracker,
        &workspace,
        expression,
        QueryOrder::Auto,
        QueryPolicy {
            strict_test_suite: true,
        },
    )
    .await;
    assert!(
        strict
            .unwrap_err()
            .to_string()
            .contains("does not refer to a test or test_suite rule")
    );
    // The identical observations do not create another DICE version, so the
    // retained graph is returned without activating or recomputing the key.
    assert!(events.is_empty(), "{events:?}");

    let (default_again, events) = query_revision_order_with_policy(
        &dice,
        &tracker,
        &workspace,
        expression,
        QueryOrder::Auto,
        QueryPolicy::default(),
    )
    .await;
    assert!(default_again.unwrap().stdout().is_empty());
    assert!(events.is_empty(), "{events:?}");
}

#[tokio::test]
async fn siblings_exposes_only_the_active_build_file_node_and_all_package_nodes() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("BUILD.bazel"),
        "filegroup(name = \"root_rule\")\n",
    );
    write(
        workspace.join("modern/BUILD.bazel"),
        "exports_files([\"BUILD.bazel\"])\nfilegroup(name = \"rule\")\n",
    );
    write(
        workspace.join("fallback/BUILD"),
        "filegroup(name = \"only\")\n",
    );
    write(
        workspace.join("dual/BUILD.bazel"),
        "filegroup(name = \"preferred\")\n",
    );
    write(
        workspace.join("dual/BUILD"),
        "filegroup(name = \"ignored\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    for (expression, order, expected) in [
        (
            "//modern:BUILD.bazel",
            QueryOrder::Auto,
            "//modern:BUILD.bazel\n",
        ),
        ("//fallback:BUILD", QueryOrder::Auto, "//fallback:BUILD\n"),
        ("//:BUILD.bazel", QueryOrder::Auto, "//:BUILD.bazel\n"),
        (
            "siblings(//fallback:BUILD)",
            QueryOrder::Auto,
            "//fallback:BUILD\n//fallback:only\n",
        ),
        (
            "deps(//modern:BUILD.bazel)",
            QueryOrder::Auto,
            "//modern:BUILD.bazel\n",
        ),
        (
            "siblings(//dual:BUILD.bazel)",
            QueryOrder::Auto,
            "//dual:BUILD.bazel\n//dual:preferred\n",
        ),
    ] {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
    let package_all = evaluate_loading_query(
        &mut transaction,
        workspace.clone(),
        "//modern:all",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert!(!package_all.stdout().contains("BUILD.bazel"));
    let recursive = evaluate_loading_query(
        &mut transaction,
        workspace.clone(),
        "//...",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert!(!recursive.stdout().contains(":BUILD"));
    assert!(recursive.stdout().contains("//modern:rule"));
    assert!(recursive.stdout().contains("//fallback:only"));
    for expression in ["//modern:BUILD", "//fallback:BUILD.bazel", "//dual:BUILD"] {
        assert_eq!(
            evaluate_loading_query(
                &mut transaction,
                workspace.clone(),
                expression,
                QueryOrder::Auto,
            )
            .await
            .unwrap_err()
            .exit_code,
            7
        );
    }
}

#[tokio::test]
async fn loading_files_preserve_fake_owners_and_projection_boundaries() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("a/BUILD.bazel"),
        "load(\"//shared:defs.bzl\", \"defs\")\nfilegroup(name = \"a\")\n",
    );
    write(
        workspace.join("b/BUILD.bazel"),
        "load(\"//shared:defs.bzl\", \"defs\")\nfilegroup(name = \"b\")\n",
    );
    write(
        workspace.join("shared/BUILD.bazel"),
        "exports_files([\"defs.bzl\"])\nfilegroup(name = \"shared\")\n",
    );
    write(
        workspace.join("shared/defs.bzl"),
        "load(\"//left:left.bzl\", \"left\")\n\
         load(\"//right:right.bzl\", \"right\")\n\
         defs = left + right\n",
    );
    write(
        workspace.join("left/BUILD.bazel"),
        "exports_files([\"left.bzl\"])\n",
    );
    write(
        workspace.join("left/left.bzl"),
        "load(\"//leaf:leaf.bzl\", \"leaf\")\nleft = leaf\n",
    );
    write(
        workspace.join("right/BUILD.bazel"),
        "exports_files([\"right.bzl\"])\n",
    );
    write(
        workspace.join("right/right.bzl"),
        "load(\"//leaf:leaf.bzl\", \"leaf\")\nright = leaf\n",
    );
    write(
        workspace.join("leaf/BUILD.bazel"),
        "exports_files([\"leaf.bzl\"])\n",
    );
    write(workspace.join("leaf/leaf.bzl"), "leaf = 1\n");

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        (
            "loadfiles(//a:a)",
            "//leaf:leaf.bzl\n//left:left.bzl\n//right:right.bzl\n//shared:defs.bzl\n",
        ),
        (
            "loadfiles(loadfiles(//a:a))",
            "//leaf:leaf.bzl\n//left:left.bzl\n//right:right.bzl\n//shared:defs.bzl\n",
        ),
        ("loadfiles(set())", ""),
        (
            "buildfiles(//a:a)",
            "//a:BUILD.bazel\n//leaf:BUILD.bazel\n//leaf:leaf.bzl\n//left:BUILD.bazel\n//left:left.bzl\n//right:BUILD.bazel\n//right:right.bzl\n//shared:BUILD.bazel\n//shared:defs.bzl\n",
        ),
        (
            "buildfiles(buildfiles(//a:a))",
            "//a:BUILD.bazel\n//leaf:BUILD.bazel\n//leaf:leaf.bzl\n//left:BUILD.bazel\n//left:left.bzl\n//right:BUILD.bazel\n//right:right.bzl\n//shared:BUILD.bazel\n//shared:defs.bzl\n",
        ),
        (
            "deps(loadfiles(//a:a))",
            "//leaf:leaf.bzl\n//left:left.bzl\n//right:right.bzl\n//shared:defs.bzl\n",
        ),
        (
            "siblings(loadfiles(//a:a) union loadfiles(//b:b))",
            "//a:BUILD.bazel\n//a:a\n//b:BUILD.bazel\n//b:b\n",
        ),
        (
            "siblings(loadfiles(//b:b) union loadfiles(//a:a))",
            "//a:BUILD.bazel\n//a:a\n//b:BUILD.bazel\n//b:b\n",
        ),
        (
            "siblings(loadfiles(//a:a) intersect //shared:defs.bzl)",
            "//a:BUILD.bazel\n//a:a\n",
        ),
        (
            "siblings(//shared:defs.bzl intersect loadfiles(//a:a))",
            "//shared:BUILD.bazel\n//shared:defs.bzl\n//shared:shared\n",
        ),
        (
            "loadfiles(//a:a) except //shared:defs.bzl",
            "//leaf:leaf.bzl\n//left:left.bzl\n//right:right.bzl\n",
        ),
        ("//shared:defs.bzl except loadfiles(//a:a)", ""),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }

    for (expression, expected) in [
        (
            "loadfiles(//a:a)",
            "//shared:defs.bzl\n//right:right.bzl\n//left:left.bzl\n//leaf:leaf.bzl\n",
        ),
        (
            "buildfiles(//a:a)",
            "//shared:defs.bzl\n//shared:BUILD.bazel\n//right:right.bzl\n//right:BUILD.bazel\n//left:left.bzl\n//left:BUILD.bazel\n//leaf:leaf.bzl\n//leaf:BUILD.bazel\n",
        ),
        (
            "deps(buildfiles(//a:a))",
            "//shared:defs.bzl\n//shared:BUILD.bazel\n//right:right.bzl\n//right:BUILD.bazel\n//left:left.bzl\n//left:BUILD.bazel\n//leaf:leaf.bzl\n//leaf:BUILD.bazel\n//a:BUILD.bazel\n",
        ),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Full,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn buildfiles_discovers_a_broken_load_package_companion_without_loading_it() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("app/BUILD.bazel"),
        "load(\"//broken:defs.bzl\", \"defs\")\nfilegroup(name = \"app\")\n",
    );
    write(
        workspace.join("broken/defs.bzl"),
        "load(\"//no_build:leaf.bzl\", \"leaf\")\ndefs = leaf\n",
    );
    write(workspace.join("broken/BUILD.bazel"), "this is not valid(\n");
    write(workspace.join("no_build/leaf.bzl"), "leaf = 1\n");

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let output = evaluate_loading_query(
        &mut transaction,
        workspace.clone(),
        "buildfiles(//app:app)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert_eq!(
        output.stdout(),
        "//app:BUILD.bazel\n//broken:BUILD.bazel\n//broken:defs.bzl\n//no_build:leaf.bzl\n"
    );
}

#[tokio::test]
async fn package_graph_has_one_zero_edge_build_file_node_synthesized_or_coalesced() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("modern/BUILD.bazel"),
        "exports_files([\"BUILD.bazel\"])\nfilegroup(name = \"rule\")\n",
    );
    write(
        workspace.join("fallback/BUILD"),
        "filegroup(name = \"only\")\n",
    );
    write(
        workspace.join("dual/BUILD.bazel"),
        "filegroup(name = \"preferred\")\n",
    );
    write(
        workspace.join("dual/BUILD"),
        "filegroup(name = \"ignored\")\n",
    );

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    for (package_path, expected_build, expected_labels) in [
        (
            "modern",
            "//modern:BUILD.bazel",
            vec!["//modern:BUILD.bazel", "//modern:rule"],
        ),
        (
            "fallback",
            "//fallback:BUILD",
            vec!["//fallback:BUILD", "//fallback:only"],
        ),
        (
            "dual",
            "//dual:BUILD.bazel",
            vec!["//dual:BUILD.bazel", "//dual:preferred"],
        ),
    ] {
        let graph = transaction
            .compute(&UnconfiguredPackageGraphKey {
                workspace: workspace.clone(),
                package: PathBuf::from(package_path),
            })
            .await
            .unwrap();
        let graph = graph.as_ref().as_ref().unwrap();
        let mut labels = graph
            .nodes
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        labels.sort();
        assert_eq!(labels, expected_labels, "{package_path}");
        let build_nodes = graph
            .nodes
            .values()
            .filter(|node| node.kind == QueryNodeKind::BuildFile)
            .collect::<Vec<_>>();
        assert_eq!(build_nodes.len(), 1, "{package_path}");
        assert_eq!(build_nodes[0].label.to_string(), expected_build);
        assert!(build_nodes[0].edges.is_empty(), "{package_path}");
        assert!(!build_nodes[0].kind.is_rule(), "{package_path}");
    }
}

#[tokio::test]
async fn visibility_graph_projects_raw_effective_and_ordered_tagged_edges() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nemit = rule(implementation = _impl, attrs = {\"out\": attr.output(mandatory = True)})\n",
    );
    write(
        workspace.join("pkg/BUILD.bazel"),
        r#"
load(":defs.bzl", "emit")
package(default_visibility = [":default_group"])
package_group(name = "default_group", packages = ["//viewer"], includes = [":included", ":missing_include"])
package_group(name = "included", packages = ["//other"])
exports_files(["public.txt"])
filegroup(name = "omitted", srcs = ["implicit.txt"])
filegroup(name = "declared", srcs = [":default_group"], visibility = [":default_group", "//viewer:__pkg__"])
filegroup(name = "ordinary_missing", srcs = [":missing_ordinary"])
filegroup(name = "visibility_missing", visibility = [":missing_visibility"])
config_setting(name = "config_public", values = {"define": "visibility_probe=1"})
config_setting(name = "config_restricted", values = {"define": "visibility_probe=1"}, visibility = [":default_group"])
emit(name = "generator", out = "generated.txt", visibility = [":default_group"])
"#,
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace,
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let node = |name: &str| {
        graph
            .nodes
            .values()
            .find(|node| node.label.target() == name)
            .unwrap()
    };

    let visibility = |name: &str| {
        node(name)
            .attributes
            .iter()
            .find(|attribute| attribute.name == "visibility")
            .unwrap()
    };
    assert!(!visibility("omitted").explicit);
    assert!(visibility("omitted").labels.is_empty());
    assert!(matches!(
        node("omitted").effective_visibility,
        RuleVisibility::Restricted(_)
    ));
    assert_eq!(
        node("omitted").visibility_source,
        VisibilitySource::PackageDefault
    );
    assert!(visibility("declared").explicit);
    assert_eq!(
        visibility("declared")
            .labels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["//pkg:default_group", "//viewer:__pkg__"]
    );
    assert_eq!(
        node("declared")
            .edges
            .iter()
            .map(|edge| (edge.kind, edge.target.to_string()))
            .collect::<Vec<_>>(),
        [
            (
                QueryEdgeKind::VisibilityNodep,
                "//pkg:default_group".to_owned(),
            ),
            (QueryEdgeKind::Ordinary, "//pkg:default_group".to_owned(),),
        ]
    );
    assert_eq!(
        node("default_group")
            .edges
            .iter()
            .map(|edge| (edge.kind, edge.target.to_string()))
            .collect::<Vec<_>>(),
        [
            (
                QueryEdgeKind::PackageGroupInclude,
                "//pkg:included".to_owned(),
            ),
            (
                QueryEdgeKind::PackageGroupInclude,
                "//pkg:missing_include".to_owned(),
            ),
        ]
    );
    assert_eq!(node("default_group").kind, QueryNodeKind::PackageGroup);
    assert!(node("default_group").package_group_contents.is_some());
    assert_eq!(
        node("implicit.txt")
            .edges
            .iter()
            .map(|edge| (edge.kind, edge.target.to_string()))
            .collect::<Vec<_>>(),
        [(
            QueryEdgeKind::VisibilityNodep,
            "//pkg:default_group".to_owned(),
        )]
    );
    assert!(node("public.txt").edges.is_empty());
    assert_eq!(
        node("public.txt").effective_visibility,
        RuleVisibility::Public
    );
    assert_eq!(
        node("config_public").effective_visibility,
        RuleVisibility::Public
    );
    assert_eq!(
        node("config_public").visibility_source,
        VisibilitySource::AlwaysPublic
    );
    assert!(matches!(
        node("config_restricted").effective_visibility,
        RuleVisibility::Restricted(_)
    ));
    assert!(matches!(
        node("config_restricted").visibility_source,
        VisibilitySource::Declared(_)
    ));
    assert_eq!(
        node("generated.txt")
            .edges
            .iter()
            .map(|edge| (edge.kind, edge.target.to_string()))
            .collect::<Vec<_>>(),
        [
            (QueryEdgeKind::GeneratingRule, "//pkg:generator".to_owned(),),
            (
                QueryEdgeKind::VisibilityNodep,
                "//pkg:default_group".to_owned(),
            ),
        ]
    );
    assert_eq!(
        node("generated.txt").visibility_source,
        VisibilitySource::GeneratingRule
    );
    assert_eq!(
        node("generated.txt").effective_visibility,
        node("generator").effective_visibility
    );
    assert!(
        graph
            .nodes
            .keys()
            .any(|label| label.target() == "missing_ordinary")
    );
    assert!(
        !graph
            .nodes
            .keys()
            .any(|label| label.target() == "missing_visibility")
    );
    assert!(
        !graph
            .nodes
            .keys()
            .any(|label| label.target() == "missing_include")
    );
}

#[tokio::test]
async fn visible_filters_with_request_local_visibility_without_a_query_edge() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("owner/BUILD.bazel"),
        "filegroup(name = \"private\", visibility = [\"//visibility:private\"])\nfilegroup(name = \"public\", visibility = [\"//visibility:public\"])\n",
    );
    write(
        workspace.join("viewer/BUILD.bazel"),
        "filegroup(name = \"caller\")\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let (output, events) = query_revision(
        &dice,
        &tracker,
        &workspace,
        "visible(//viewer:caller, set(//owner:private //owner:public))",
    )
    .await;
    let output = output.unwrap();
    assert_eq!(output.labels.as_ref(), ["//owner:public"]);
    assert_eq!(
        output.graph_stdout(false, true),
        "digraph mygraph {\n  node [shape=box];\n  \"//owner:public\"\n}\n"
    );
    assert_eq!(
        events,
        vec![
            package("owner", ActivationKind::Evaluated),
            package("viewer", ActivationKind::Evaluated),
        ]
    );
}

#[tokio::test]
async fn visible_reuses_cross_package_graphs_and_recovers_when_an_included_group_is_recreated() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("viewer/BUILD.bazel"),
        "filegroup(name = \"caller\")\n",
    );
    write(
        workspace.join("target/BUILD.bazel"),
        "filegroup(name = \"item\", visibility = [\"//top:group\"])\n",
    );
    write(
        workspace.join("top/BUILD.bazel"),
        "package_group(name = \"group\", includes = [\"//leaf:friends\"])\n",
    );
    let leaf = workspace.join("leaf/BUILD.bazel");
    write(
        &leaf,
        "package_group(name = \"friends\", packages = [\"//viewer\"])\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let expression = "visible(//viewer:caller, //target:item)";

    let (initial, _) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(initial.unwrap().labels.as_ref(), ["//target:item"]);
    write(
        &leaf,
        "# formatting-only edit\npackage_group( name = \"friends\", packages = [\"//viewer\"] )\n",
    );
    let (formatted, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(formatted.unwrap().labels.as_ref(), ["//target:item"]);
    assert_eq!(
        events,
        [
            package("leaf", ActivationKind::Reused),
            package("target", ActivationKind::Reused),
            package("top", ActivationKind::Reused),
            package("viewer", ActivationKind::Reused),
        ]
    );

    write(
        &leaf,
        "package_group(name = \"friends\", packages = [\"//other\"])\n",
    );
    let (changed, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(changed.unwrap().labels.is_empty());
    assert_eq!(
        events,
        [
            package("leaf", ActivationKind::Evaluated),
            package("target", ActivationKind::Reused),
            package("top", ActivationKind::Reused),
            package("viewer", ActivationKind::Reused),
        ]
    );

    write(&leaf, "filegroup(name = \"unrelated\")\n");
    let (missing, _) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("Invalid visibility label '//top:group': no such target '//leaf:friends'")
    );

    write(
        &leaf,
        "package_group(name = \"friends\", packages = [\"//viewer\"])\n",
    );
    let (recreated, _) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(recreated.unwrap().labels.as_ref(), ["//target:item"]);
}

#[tokio::test]
async fn visible_same_package_access_does_not_mask_a_missing_restricted_group() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("owner/BUILD.bazel"),
        "filegroup(name = \"caller\")\nfilegroup(name = \"target\", visibility = [\":missing\"])\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let (result, _) = query_revision(
        &dice,
        &tracker,
        &workspace,
        "visible(//owner:caller, //owner:target)",
    )
    .await;
    assert!(
        result.unwrap_err().to_string().contains(
            "Invalid visibility label '//owner:missing': no such target '//owner:missing'"
        )
    );
}

#[tokio::test]
async fn config_setting_is_a_loading_rule_without_configuration_evaluation() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let node = graph
        .nodes
        .values()
        .find(|node| node.label.to_string() == "//pkg:linux")
        .unwrap();
    assert_eq!(node.kind, QueryNodeKind::Rule("config_setting rule".into()));
    assert!(node.edges.is_empty());

    let output =
        evaluate_loading_query(&mut transaction, workspace, "//pkg:linux", QueryOrder::Auto)
            .await
            .unwrap();
    assert_eq!(output.stdout(), "//pkg:linux\n");
}

#[tokio::test]
async fn executable_capability_projection_keeps_exported_class_and_non_rule_boundaries() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\n\nexec_arbitrary = rule(implementation = _impl, executable = True)\nplain_rule = rule(implementation = _impl)\nimplicit_test_test = rule(implementation = _impl, test = True)\noutput_rule = rule(implementation = _impl, attrs = {\"outs\": attr.output_list()})\n",
    );
    write(
        workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"exec_arbitrary\", \"implicit_test_test\", \"output_rule\", \"plain_rule\")\nexports_files([\"data.txt\"])\nexec_arbitrary(name = \"target_test\")\nplain_rule(name = \"plain\")\nimplicit_test_test(name = \"ordinary_target\")\nfilegroup(name = \"files\", srcs = [\":data.txt\"])\nalias(name = \"alias_exec\", actual = \":target_test\")\nconfig_setting(name = \"setting\", values = {\"cpu\": \"k8\"})\noutput_rule(name = \"generated_owner\", outs = [\"generated.txt\"])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let capability = |label: &str| {
        graph
            .nodes
            .values()
            .find(|node| node.label.to_string() == label)
            .unwrap()
            .rule_capability
            .as_ref()
            .map(|value| (value.rule_class.as_str(), value.executable))
    };
    assert_eq!(
        capability("//pkg:target_test"),
        Some(("exec_arbitrary", true))
    );
    assert_eq!(capability("//pkg:plain"), Some(("plain_rule", false)));
    assert_eq!(
        capability("//pkg:ordinary_target"),
        Some(("implicit_test_test", true))
    );
    assert_eq!(capability("//pkg:files"), Some(("filegroup", false)));
    assert_eq!(capability("//pkg:alias_exec"), Some(("alias", false)));
    assert_eq!(capability("//pkg:setting"), Some(("config_setting", false)));
    for label in ["//pkg:data.txt", "//pkg:BUILD.bazel", "//pkg:generated.txt"] {
        assert_eq!(capability(label), None, "{label}");
    }

    // The target name alone never determines executable/test classification.
    let executable = evaluate_loading_query(
        &mut transaction,
        workspace,
        "executables(//pkg:target_test)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert_eq!(executable.stdout(), "//pkg:target_test\n");
}

#[tokio::test]
async fn executables_capability_transitions_invalidate_or_reuse_the_retained_package_graph() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let definition = |name: &str, arguments: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\n\n{name} = rule(implementation = _impl{arguments})\n"
        )
    };
    let build_file = |rule: &str, target: &str| {
        format!("load(\":defs.bzl\", \"{rule}\")\n{rule}(name = \"{target}\")\n")
    };
    write(&defs, &definition("probe", ", executable = False"));
    write(&build, &build_file("probe", "item"));
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());

    let (initial, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(initial.unwrap().stdout(), "");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&defs, &definition("probe", ", executable = True"));
    let (executable, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(executable.unwrap().stdout(), "//pkg:item\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    let (renamed_export, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(renamed_export.unwrap().stdout(), "//pkg:item\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&defs, &definition("renamed_exec", ", executable = False"));
    let (non_executable_again, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(non_executable_again.unwrap().stdout(), "");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&defs, &definition("probe_test", ", test = True"));
    write(&build, &build_file("probe_test", "item"));
    let (test_rule, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(test_rule.unwrap().stdout(), "");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    let (non_test_again, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item)").await;
    assert_eq!(non_test_again.unwrap().stdout(), "//pkg:item\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&build, &build_file("renamed_exec", "item_test"));
    let (renamed_target, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item_test)").await;
    assert_eq!(renamed_target.unwrap().stdout(), "//pkg:item_test\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "# whitespace-only\nload( \":defs.bzl\", \"renamed_exec\" )\nrenamed_exec( name = \"item_test\" )\n",
    );
    let (formatted, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item_test)").await;
    assert_eq!(formatted.unwrap().stdout(), "//pkg:item_test\n");
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    fs::remove_file(&build).unwrap();
    let (deleted, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item_test)").await;
    assert!(
        deleted
            .unwrap_err()
            .to_string()
            .contains("no such package 'pkg'")
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&build, &build_file("renamed_exec", "item_test"));
    let (recreated, events) =
        query_revision(&dice, &tracker, &workspace, "executables(//pkg:item_test)").await;
    assert_eq!(recreated.unwrap().stdout(), "//pkg:item_test\n");
    // The graph recomputes after deletion, but its recreated semantic value
    // equals the retained one, so DICE reports reuse at this projection key.
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);
}

#[tokio::test]
async fn active_build_basename_non_export_target_collision_is_a_query_error() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("collision/BUILD.bazel"),
        "filegroup(name = \"BUILD.bazel\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace,
            package: PathBuf::from("collision"),
        })
        .await
        .unwrap();
    let error = graph.as_ref().as_ref().unwrap_err();
    assert_eq!(
        error.to_string(),
        "target '//collision:BUILD.bazel' collides with active BUILD file"
    );
}

#[tokio::test]
async fn output_targets_are_generated_files_with_only_generator_edges() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"out\": attr.output(mandatory = True), \"outs\": attr.output_list(mandatory = True)})\n",
    );
    write(
        workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", out = \"dir/one.out\", outs = [\":two.out\", \"//pkg:three.out\"])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace,
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let mut labels = graph
        .nodes
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    labels.sort();
    assert_eq!(
        labels,
        vec![
            "//pkg:BUILD.bazel",
            "//pkg:dir/one.out",
            "//pkg:rule",
            "//pkg:three.out",
            "//pkg:two.out",
        ]
    );
    let rule = graph
        .nodes
        .values()
        .find(|node| node.label.to_string() == "//pkg:rule")
        .unwrap();
    assert!(rule.edges.is_empty());
    for output in ["//pkg:dir/one.out", "//pkg:two.out", "//pkg:three.out"] {
        let output = graph
            .nodes
            .values()
            .find(|node| node.label.to_string() == output)
            .unwrap();
        assert_eq!(output.kind, QueryNodeKind::GeneratedFile);
        assert_eq!(
            output
                .edges
                .iter()
                .map(|edge| edge.target.to_string())
                .collect::<Vec<_>>(),
            ["//pkg:rule"]
        );
    }
}

#[tokio::test]
async fn labels_projects_supported_native_filegroup_and_alias_attributes() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"group\", srcs = [\"local.txt\", \"dir/local.txt\", \":colon.txt\", \"//other:cross.txt\"])\nalias(name = \"redirect\", actual = \"group\")\n",
    );
    write(
        workspace.join("other/BUILD.bazel"),
        "exports_files([\"cross.txt\"])\n",
    );
    write(workspace.join("pkg/local.txt"), "local\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        (
            "labels(srcs, //pkg:group)",
            "//other:cross.txt\n//pkg:colon.txt\n//pkg:dir/local.txt\n//pkg:local.txt\n",
        ),
        ("labels(actual, //pkg:redirect)", "//pkg:group\n"),
        ("labels(srcs, //pkg:local.txt)", ""),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn graph_projects_test_suite_membership_scalars_edges_and_total_explicitness() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"explicit\": attr.label(), \"defaulted\": attr.label(default = \":default.txt\"), \"_implicit\": attr.label(default = \":implicit.txt\"), \"many\": attr.label_list()})\nprobe_test = rule(implementation = _impl, test = True)\n",
    );
    let build = workspace.join("pkg/BUILD.bazel");
    write(
        &build,
        "load(\":defs.bzl\", \"probe\", \"probe_test\")\n\
         filegroup(name = \"omitted\")\n\
         filegroup(name = \"empty\", srcs = [])\n\
         alias(name = \"redirect\", actual = \":auto\")\n\
         test_suite(name = \"implicit\")\n\
         test_suite(name = \"empty_suite\", tests = [])\n\
         test_suite(name = \"explicit_suite\", tests = [\":auto\", \":manual_test\"], tags = [\"suite\", \"manual\"])\n\
         probe(name = \"attrs\", explicit = \":explicit.txt\", many = select({\":condition\": [\":dupe.txt\"]}) + [\":dupe.txt\"])\n\
         probe_test(name = \"auto\", tags = [\"z\", \"a\", \"z\"])\n\
         probe_test(name = \"manual_test\", tags = [\"manual\", \"-+tag\"], size = \"large\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let node = |name: &str| {
        graph
            .nodes
            .values()
            .find(|node| node.label.to_string() == format!("//pkg:{name}"))
            .unwrap()
    };
    let attribute = |name: &str, attribute: &str| {
        node(name)
            .attributes
            .iter()
            .find(|value| value.name == attribute)
            .unwrap()
    };

    assert!(!attribute("omitted", "srcs").explicit);
    assert!(attribute("empty", "srcs").explicit);
    assert!(attribute("redirect", "actual").explicit);
    assert!(attribute("attrs", "explicit").explicit);
    assert!(!attribute("attrs", "defaulted").explicit);
    assert!(!attribute("attrs", "$implicit").explicit);
    assert_eq!(
        attribute("attrs", "many")
            .labels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["//pkg:dupe.txt", "//pkg:dupe.txt"]
    );
    assert_eq!(
        node("attrs")
            .edges
            .iter()
            .filter(|edge| edge.target.to_string() == "//pkg:dupe.txt")
            .count(),
        1
    );

    for (suite, tests_explicit) in [
        ("implicit", false),
        ("empty_suite", true),
        ("explicit_suite", true),
    ] {
        assert_eq!(attribute(suite, "tests").explicit, tests_explicit);
        assert!(attribute(suite, "$implicit_tests").explicit);
    }
    assert_eq!(
        attribute("implicit", "$implicit_tests")
            .labels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["//pkg:auto"]
    );
    assert!(attribute("implicit", "tests").labels.is_empty());
    assert_eq!(
        attribute("explicit_suite", "tests")
            .labels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["//pkg:auto", "//pkg:manual_test"]
    );
    assert!(
        attribute("explicit_suite", "$implicit_tests")
            .labels
            .is_empty()
    );
    assert_eq!(
        node("explicit_suite")
            .edges
            .iter()
            .map(|edge| edge.target.to_string())
            .collect::<Vec<_>>(),
        ["//pkg:auto", "//pkg:manual_test"]
    );
    assert_eq!(
        node("implicit")
            .edges
            .iter()
            .map(|edge| edge.target.to_string())
            .collect::<Vec<_>>(),
        ["//pkg:auto"]
    );
    assert_eq!(
        node("explicit_suite")
            .rule_capability
            .as_ref()
            .map(|capability| (
                capability.rule_class.as_str(),
                capability.executable,
                capability.test_kind,
            )),
        Some((
            "test_suite",
            false,
            Some(slug_loading_v2::TestRuleKind::Suite),
        ))
    );
    assert_eq!(
        node("auto")
            .rule_capability
            .as_ref()
            .and_then(|capability| capability.test_kind),
        Some(slug_loading_v2::TestRuleKind::Test)
    );
    let suite = node("explicit_suite").test_metadata.as_ref().unwrap();
    assert_eq!(suite.tags.as_ref(), ["manual", "suite"]);
    assert_eq!(suite.size, None);
    assert!(suite.manual);
    let test = node("manual_test").test_metadata.as_ref().unwrap();
    assert_eq!(test.tags.as_ref(), ["-+tag", "manual"]);
    assert_eq!(test.size.as_deref(), Some("large"));
    assert!(test.manual);
    assert!(node("attrs").test_metadata.is_none());

    let output = evaluate_loading_query(
        &mut transaction,
        workspace.clone(),
        "tests(//pkg:implicit)",
        QueryOrder::Auto,
    )
    .await
    .unwrap();
    assert_eq!(output.stdout(), "//pkg:auto\n");
}

#[tokio::test]
async fn test_suite_metadata_reuses_reorders_and_recovers_across_same_dice_lifecycle() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe_test = rule(implementation = _impl, test = True)\n",
    );
    let build = workspace.join("pkg/BUILD.bazel");
    let explicit = |tests: &str, tags: &str| {
        format!(
            "load(\":defs.bzl\", \"probe_test\")\nprobe_test(name = \"one\")\nprobe_test(name = \"two\")\ntest_suite(name = \"suite\", tests = {tests}, tags = {tags})\n"
        )
    };
    write(
        &build,
        &explicit("[\":two\", \":one\"]", "[\"z\", \"a\", \"z\"]"),
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let expression = "deps(//pkg:suite, 1)";
    let expected = "//pkg:one\n//pkg:suite\n//pkg:two\n";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(initial.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        &explicit("[\":one\", \":two\"]", "[\"z\", \"z\", \"a\"]"),
    );
    let (reordered, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(reordered.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    write(&build, &explicit("[\":one\", \":two\"]", "[\"a\", \"z\"]"));
    let (duplicate_removed, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(duplicate_removed.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        &explicit("[\":two\", \":one\"]", "[\"z\", \"a\", \"z\"]"),
    );
    let (duplicate_restored, events) =
        query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(duplicate_restored.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        &explicit("[\":one\", \"one\"]", "[\"z\", \"a\", \"z\"]"),
    );
    let (duplicate, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(
        duplicate
            .unwrap_err()
            .to_string()
            .contains("Label '//pkg:one' is duplicated in the 'tests' attribute of rule 'suite'")
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        &explicit("[\":two\", \":one\"]", "[\"z\", \"a\", \"z\"]"),
    );
    let (recovered, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(recovered.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    write(
        &build,
        "load(\":defs.bzl\", \"probe_test\")\nprobe_test(name = \"one\")\nprobe_test(name = \"two\")\ntest_suite(name = \"suite\")\n",
    );
    let (implicit, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(implicit.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "load(\":defs.bzl\", \"probe_test\")\nprobe_test(name = \"one\")\nprobe_test(name = \"two\")\ntest_suite(name = \"suite\", tests = [])\n",
    );
    let (explicit_empty, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(explicit_empty.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    fs::remove_file(&build).unwrap();
    let (deleted, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(deleted.is_err());
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "load(\":defs.bzl\", \"probe_test\")\nprobe_test(name = \"one\")\nprobe_test(name = \"two\")\ntest_suite(name = \"suite\", tests = [])\n",
    );
    let (recreated, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(recreated.unwrap().stdout(), expected);
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);
}

#[tokio::test]
async fn native_label_canonicalization_reuses_rejects_duplicates_and_recovers() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let build = workspace.join("pkg/BUILD.bazel");
    let build_for = |srcs: &str, actual: &str| {
        format!(
            "filegroup(name = \"group\", srcs = {srcs})\nalias(name = \"redirect\", actual = \"{actual}\")\n"
        )
    };
    write(&build, &build_for("[\"one.txt\", \":two.txt\"]", "group"));
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let expression = "labels(srcs, //pkg:group)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(initial.unwrap().stdout(), "//pkg:one.txt\n//pkg:two.txt\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&build, &build_for("[\":one.txt\", \"two.txt\"]", ":group"));
    let (equivalent, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        equivalent.unwrap().stdout(),
        "//pkg:one.txt\n//pkg:two.txt\n"
    );
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    write(
        &build,
        &build_for("[\"one.txt\", \"two.txt\", \":one.txt\"]", "group"),
    );
    let (duplicate, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    let duplicate = duplicate.unwrap_err().to_string();
    assert!(
        duplicate.contains(
            "Label '//pkg:one.txt' is duplicated in the 'srcs' attribute of rule 'group'"
        ),
        "{duplicate}"
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&build, &build_for("[\"two.txt\", \"one.txt\"]", "group"));
    let (recovered, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        recovered.unwrap().stdout(),
        "//pkg:one.txt\n//pkg:two.txt\n"
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: PathBuf::from("pkg"),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let group = graph
        .nodes
        .values()
        .find(|node| node.label.to_string() == "//pkg:group")
        .unwrap();
    assert_eq!(
        group.attributes[0]
            .labels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["//pkg:two.txt", "//pkg:one.txt"]
    );
    assert_eq!(
        group
            .edges
            .iter()
            .map(|edge| edge.target.to_string())
            .collect::<Vec<_>>(),
        ["//pkg:two.txt", "//pkg:one.txt"]
    );

    fs::remove_file(&build).unwrap();
    let (deleted, _) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert!(deleted.is_err());
    write(&build, &build_for("[\"two.txt\", \"one.txt\"]", "group"));
    let (recreated, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        recreated.unwrap().stdout(),
        "//pkg:one.txt\n//pkg:two.txt\n"
    );
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);
}

#[tokio::test]
async fn invalid_relative_package_target_fails_in_loading_label_conversion() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"dep\": attr.label(mandatory = True)})\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (raw, expected) in [
        (
            "pkg:target",
            "invalid label 'pkg:target': absolute label must begin with '@' or '//'",
        ),
        (
            "...",
            "invalid label '...': package name cannot contain '...'",
        ),
        (
            "foo/...:all",
            "invalid label 'foo/...:all': package name cannot contain '...'",
        ),
        (
            "//foo/...",
            "invalid label '//foo/...': package name cannot contain '...'",
        ),
    ] {
        write(
            workspace.join("pkg/BUILD.bazel"),
            &format!("load(\":defs.bzl\", \"probe\")\nprobe(name = \"bad\", dep = \"{raw}\")\n"),
        );
        let mut transaction = transaction(&dice, &workspace).await;
        let graph = transaction
            .compute(&UnconfiguredPackageGraphKey {
                workspace: workspace.clone(),
                package: PathBuf::from("pkg"),
            })
            .await
            .unwrap();
        let error = graph.as_ref().as_ref().unwrap_err().to_string();
        assert!(error.contains(expected), "{raw}: {error}");
    }
}

#[tokio::test]
async fn starlark_package_context_labels_use_build_and_definition_packages() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("definitions/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"explicit\": attr.label_list(mandatory = True), \"defaulted\": attr.label(default = \"default.txt\")})\n",
    );
    write(
        workspace.join("definitions/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"default_owner\", explicit = [])\n",
    );
    write(
        workspace.join("consumer/BUILD.bazel"),
        "load(\"//definitions:defs.bzl\", \"probe\")\nprobe(name = \"consumer\", explicit = [\"bare.txt\", \"dir/bare.txt\", \":colon.txt\", \"//other:cross.txt\", \"//:root.txt\"])\n",
    );
    write(
        workspace.join("other/BUILD.bazel"),
        "exports_files([\"cross.txt\"])\n",
    );
    write(
        workspace.join("BUILD.bazel"),
        "exports_files([\"root.txt\"])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        (
            "labels(explicit, //consumer:consumer)",
            "//:root.txt\n//consumer:bare.txt\n//consumer:colon.txt\n//consumer:dir/bare.txt\n//other:cross.txt\n",
        ),
        (
            "labels(defaulted, //consumer:consumer)",
            "//definitions:default.txt\n",
        ),
        (
            "deps(//consumer:consumer, 1)",
            "//:root.txt\n//consumer:bare.txt\n//consumer:colon.txt\n//consumer:consumer\n//consumer:dir/bare.txt\n//definitions:default.txt\n//other:cross.txt\n",
        ),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn labels_metadata_changes_invalidate_and_semantic_formatting_reuses_the_package_graph() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let schema = |default| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"dep\": attr.label(default = \":{default}\"), \"out\": attr.output(mandatory = True)}})\n"
        )
    };
    write(&defs, &schema("one.txt"));
    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", out = \"one.out\")\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(QueryTracker::default());
    let labels = "labels(dep, //pkg:rule)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, labels).await;
    assert_eq!(initial.unwrap().stdout(), "//pkg:one.txt\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "# semantic no-op\nconfig_setting( name = \"linux\", values = {\"cpu\": \"k8\"} )\nload(\":defs.bzl\", \"probe\")\nprobe( name = \"rule\", out = \"one.out\" )\n",
    );
    let (formatted, events) = query_revision(&dice, &tracker, &workspace, labels).await;
    assert_eq!(formatted.unwrap().stdout(), "//pkg:one.txt\n");
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    write(&defs, &schema("two.txt"));
    let (default_changed, events) = query_revision(&dice, &tracker, &workspace, labels).await;
    assert_eq!(default_changed.unwrap().stdout(), "//pkg:two.txt\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = \":three.txt\", out = \"one.out\")\n",
    );
    let (value_changed, events) = query_revision(&dice, &tracker, &workspace, labels).await;
    assert_eq!(value_changed.unwrap().stdout(), "//pkg:three.txt\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"one.out\")\n",
    );
    let (selector_changed, events) = query_revision(&dice, &tracker, &workspace, labels).await;
    assert_eq!(
        selector_changed.unwrap().stdout(),
        "//pkg:one.txt\n//pkg:two.txt\n"
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    let (output_initial, events) =
        query_revision(&dice, &tracker, &workspace, "labels(out, //pkg:rule)").await;
    assert_eq!(output_initial.unwrap().stdout(), "//pkg:one.out\n");
    assert!(events.is_empty());

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"two.out\")\n",
    );
    let (output_changed, events) =
        query_revision(&dice, &tracker, &workspace, "labels(out, //pkg:rule)").await;
    assert_eq!(output_changed.unwrap().stdout(), "//pkg:two.out\n");
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);
}

#[tokio::test]
async fn build_file_zero_edges_and_full_siblings_order_match_bazel_oracle() {
    let workspace = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/query-siblings-build-file-node/workspace"),
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, order, expected) in [
        (
            "rdeps(siblings(//modern:rule), //modern:BUILD.bazel)",
            QueryOrder::Auto,
            "//modern:BUILD.bazel\n",
        ),
        (
            "same_pkg_direct_rdeps(//modern:BUILD.bazel)",
            QueryOrder::Auto,
            "",
        ),
        (
            "siblings(//modern:rule)",
            QueryOrder::Full,
            "//modern:rule\n//modern:leaf\n//modern:implicit.txt\n//modern:explicit.txt\n//modern:cycle_b\n//modern:cycle_a\n//modern:custom\n//modern:alias\n//modern:BUILD.bazel\n",
        ),
        (
            "siblings(//modern:cycle_a)",
            QueryOrder::Full,
            "//modern:rule\n//modern:leaf\n//modern:implicit.txt\n//modern:explicit.txt\n//modern:cycle_b\n//modern:cycle_a\n//modern:custom\n//modern:alias\n//modern:BUILD.bazel\n",
        ),
    ] {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn full_siblings_uses_only_edges_recorded_while_evaluating_the_operand() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("provenance/BUILD.bazel"),
        "filegroup(name = \"z\", srcs = [\":a\"])\n\
         filegroup(name = \"a\", srcs = [\":zz\"])\n\
         filegroup(name = \"zz\")\n\
         filegroup(name = \"y\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        (
            "siblings(//provenance:a)",
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            "siblings(//provenance:a) union set()",
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            "siblings(deps(//provenance:a))",
            "//provenance:z\n//provenance:y\n//provenance:a\n//provenance:zz\n//provenance:BUILD.bazel\n",
        ),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Full,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn siblings_has_exact_dice_target_and_build_content_lifecycle() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let build = workspace.join("cand/BUILD.bazel");
    write(&build, "filegroup(name = \"one\")\n");
    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"unrelated\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());
    let expression = "siblings(//cand:BUILD.bazel)";

    let (initial, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        initial.unwrap().labels.as_ref(),
        ["//cand:BUILD.bazel", "//cand:one"]
    );
    assert_eq!(events, [package("cand", ActivationKind::Evaluated)]);

    let (identical, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        identical.unwrap().labels.as_ref(),
        ["//cand:BUILD.bazel", "//cand:one"]
    );
    assert!(events.is_empty(), "{events:?}");

    write(
        workspace.join("outside/BUILD.bazel"),
        "filegroup(name = \"changed\")\n",
    );
    let (unrelated, events) = query_revision(&dice, &tracker, &workspace, expression).await;
    assert_eq!(
        unrelated.unwrap().labels.as_ref(),
        ["//cand:BUILD.bazel", "//cand:one"]
    );
    assert_eq!(events, [package("cand", ActivationKind::Reused)]);

    for (content, expected) in [
        (
            "filegroup(name = \"one\")\nfilegroup(name = \"two\")\n",
            vec!["//cand:BUILD.bazel", "//cand:one", "//cand:two"],
        ),
        (
            "filegroup(name = \"one\")\nfilegroup(name = \"middle\")\n",
            vec!["//cand:BUILD.bazel", "//cand:middle", "//cand:one"],
        ),
        (
            "filegroup(name = \"one\")\n",
            vec!["//cand:BUILD.bazel", "//cand:one"],
        ),
        (
            "filegroup(name = \"one\")\nfilegroup(name = \"zeta\")\n",
            vec!["//cand:BUILD.bazel", "//cand:one", "//cand:zeta"],
        ),
    ] {
        write(&build, content);
        let (result, events) = query_revision(&dice, &tracker, &workspace, expression).await;
        assert_eq!(result.unwrap().labels.as_ref(), expected, "{content}");
        assert_eq!(
            events,
            [package("cand", ActivationKind::Evaluated)],
            "{content}"
        );
    }
}

#[tokio::test]
async fn siblings_has_exact_dice_build_basename_priority_and_package_lifecycle() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    let package_dir = workspace.join("pkg");
    let modern = package_dir.join("BUILD.bazel");
    let fallback = package_dir.join("BUILD");
    write(&modern, "filegroup(name = \"modern\")\n");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let tracker = Arc::new(QueryTracker::default());

    let (initial, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(
        initial.unwrap().labels.as_ref(),
        ["//pkg:BUILD.bazel", "//pkg:modern"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    fs::rename(&modern, &fallback).unwrap();
    let (to_fallback, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD)").await;
    assert_eq!(
        to_fallback.unwrap().labels.as_ref(),
        ["//pkg:BUILD", "//pkg:modern"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    fs::rename(&fallback, &modern).unwrap();
    let (to_modern, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(
        to_modern.unwrap().labels.as_ref(),
        ["//pkg:BUILD.bazel", "//pkg:modern"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&fallback, "filegroup(name = \"ignored\")\n");
    let (dual, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(
        dual.unwrap().labels.as_ref(),
        ["//pkg:BUILD.bazel", "//pkg:modern"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    write(&fallback, "filegroup(name = \"still_ignored\")\n");
    let (ignored_edit, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(
        ignored_edit.unwrap().labels.as_ref(),
        ["//pkg:BUILD.bazel", "//pkg:modern"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Reused)]);

    fs::remove_file(&modern).unwrap();
    let (priority_fallback, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD)").await;
    assert_eq!(
        priority_fallback.unwrap().labels.as_ref(),
        ["//pkg:BUILD", "//pkg:still_ignored"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&modern, "filegroup(name = \"restored\")\n");
    let (priority_restored, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(
        priority_restored.unwrap().labels.as_ref(),
        ["//pkg:BUILD.bazel", "//pkg:restored"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    fs::remove_file(&modern).unwrap();
    fs::remove_file(&fallback).unwrap();
    let (missing, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD.bazel)").await;
    assert_eq!(missing.unwrap_err().exit_code, 7);
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);

    write(&fallback, "filegroup(name = \"recreated\")\n");
    let (recreated, events) =
        query_revision(&dice, &tracker, &workspace, "siblings(//pkg:BUILD)").await;
    assert_eq!(
        recreated.unwrap().labels.as_ref(),
        ["//pkg:BUILD", "//pkg:recreated"]
    );
    assert_eq!(events, [package("pkg", ActivationKind::Evaluated)]);
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
    assert_eq!(
        labels,
        ["//app:bin", "//app:BUILD.bazel", "//app:local.txt"]
    );

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
