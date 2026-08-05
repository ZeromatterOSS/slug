#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationKind as DiceActivationKind;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceTransaction;
use dice::DynKey;
use dice::Key;
use dice::RichActivation;
use dice::RootActivation;
use dice::UserComputationData;
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
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::VisibilitySource;
use slug_loading_v2::bzl_load_cycle_detector;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileKey;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_query_v2::QueryEdgeKind;
use slug_query_v2::QueryNodeKind;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryOutputCompletion;
use slug_query_v2::QueryPolicy;
use slug_query_v2::QueryPreparationOutcome;
use slug_query_v2::RootQueryCommandKey;
use slug_query_v2::SubtreePackageSetKey;
use slug_query_v2::UnconfiguredPackageGraphKey;
use slug_query_v2::evaluate_loading_query;
use slug_query_v2::evaluate_loading_query_with_policy;
use slug_query_v2::evaluate_loading_query_with_policy_and_output_completion;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
#[cfg(unix)]
use slug_workspace_v2::PathIoErrorKind;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
#[cfg(unix)]
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::WorkspaceRawFileKey;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use starlark_map::small_map::SmallMap;

fn scratch() -> PathBuf {
    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let serial = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "slug-query-v2-{}-{nanos}-{serial}",
        std::process::id()
    ));
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
async fn label_kind_formats_retained_structural_kinds_in_text_order() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\ncustom = rule(implementation = _impl, attrs = {\"out\": attr.output(mandatory = True)})\n",
    );
    write(
        workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"custom\")\nexports_files([\"source.txt\"])\npackage_group(name = \"group\", packages = [\"//...\"])\nfilegroup(name = \"native\", srcs = [\":source.txt\"])\ncustom(name = \"starlark\", out = \"generated.out\")\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;

    for (expression, expected) in [
        ("//pkg:native", "filegroup rule //pkg:native\n"),
        ("//pkg:starlark", "custom rule //pkg:starlark\n"),
        ("//pkg:group", "package group //pkg:group\n"),
        ("//pkg:source.txt", "source file //pkg:source.txt\n"),
        ("//pkg:BUILD.bazel", "source file //pkg:BUILD.bazel\n"),
        (
            "labels(out, //pkg:starlark)",
            "generated file //pkg:generated.out\n",
        ),
        ("loadfiles(//pkg:native)", "source file //pkg:defs.bzl\n"),
        (
            "buildfiles(//pkg:native)",
            "source file //pkg:BUILD.bazel\nsource file //pkg:defs.bzl\n",
        ),
    ] {
        let output = evaluate_loading_query_with_policy_and_output_completion(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::LabelKind,
        )
        .await
        .unwrap();
        assert_eq!(output.label_kind_stdout(), expected, "{expression}, auto");
        for order in [QueryOrder::Auto, QueryOrder::Full] {
            let output = evaluate_loading_query_with_policy_and_output_completion(
                &mut transaction,
                workspace.clone(),
                expression,
                order,
                QueryPolicy::default(),
                QueryOutputCompletion::LabelKind,
            )
            .await
            .unwrap();
            assert_eq!(
                output
                    .label_kind_stdout()
                    .lines()
                    .map(|line| line.rsplit_once(' ').unwrap().1)
                    .collect::<Vec<_>>(),
                output.stdout().lines().collect::<Vec<_>>(),
                "{expression}, {order}"
            );
        }
    }
}

#[tokio::test]
async fn package_output_sorts_deduplicates_and_keeps_main_root_empty() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n");
    write(
        workspace.join("BUILD.bazel"),
        "filegroup(name = \"root\", srcs = [])\n",
    );
    write(
        workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"one\", srcs = [])\nfilegroup(name = \"two\", srcs = [])\n",
    );
    write(
        workspace.join("nested/BUILD.bazel"),
        "filegroup(name = \"leaf\", srcs = [])\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let expression = "set(//nested:leaf //:root //app:two //app:one)";

    for order in [QueryOrder::Auto, QueryOrder::Full] {
        let output = evaluate_loading_query(&mut transaction, workspace.clone(), expression, order)
            .await
            .unwrap();
        assert_eq!(output.package_stdout(), "\napp\nnested\n", "{order}");
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

#[derive(Default)]
struct RootQueryEpochBuilder {
    entries: SmallMap<PathObservationDemand, PathObservationResult>,
}

impl RootQueryEpochBuilder {
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

    fn directory_entries(&mut self, path: &str, names: &[&str]) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::DirectoryEntries),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new(names.iter().map(|name| {
                    PathDirectoryEntry::new(
                        PathDirectoryName::new(*name).unwrap(),
                        PathDirectoryEntryKind::Directory,
                    )
                })),
            )),
        );
    }

    fn missing(&mut self, path: &str) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }

    fn omit(&mut self, path: &str, operation: PathObservationOperation) {
        self.entries.shift_remove(&Self::demand(path, operation));
    }

    fn file(&mut self, path: &str, source: &str, variant: i64) {
        self.node(path, PathNodeKind::RegularFile, variant);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_bytes(),
            ))),
        );
    }

    fn symlink(&mut self, path: &str, target: &str, variant: i64) {
        self.node(path, PathNodeKind::Symlink, variant);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                target,
            )))),
        );
    }

    fn base(variant: i64) -> Self {
        let mut builder = Self::default();
        builder.directory("/", variant);
        builder.directory("/workspace", variant);
        builder.file("/workspace/MODULE.bazel", "", variant);
        builder.missing("/workspace/REPO.bazel");
        builder.missing("/workspace/.bazelignore");
        builder
    }

    fn external_package(variant: i64) -> Self {
        let mut builder = Self::base(variant);
        builder.file(
            "/workspace/MODULE.bazel",
            "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
            variant,
        );
        builder.directory("/workspace/dep", variant);
        builder.directory_entries("/workspace/dep", &[]);
        builder.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = \"dep\", version = \"1.0.0\")\n",
            variant,
        );
        builder.missing("/workspace/dep/REPO.bazel");
        builder.missing("/workspace/dep/.bazelignore");
        builder.file(
            "/workspace/dep/BUILD.bazel",
            "filegroup(name = \"rule\")\n",
            variant,
        );
        builder.package(
            "pkg",
            "filegroup(name = \"private\", visibility = [\"//visibility:private\"])\n",
            variant,
        );
        builder.package(
            "target",
            "filegroup(name = \"restricted\", visibility = [\"//pkg:__pkg__\"])\nfilegroup(name = \"group_restricted\", visibility = [\"//groups:root_pkg\"])\n",
            variant,
        );
        builder.package(
            "groups",
            "package_group(name = \"root_pkg\", packages = [\"//pkg\"])\n",
            variant,
        );
        builder.directory("/workspace/dep/pkg", variant);
        builder.directory_entries("/workspace/dep/pkg", &[]);
        builder.file(
            "/workspace/dep/pkg/BUILD.bazel",
            "filegroup(name = \"caller\")\n",
            variant,
        );
        builder
    }

    fn external_visibility_package(variant: i64, apparent: &str, build: &str) -> Self {
        let mut builder = Self::external_package(variant);
        builder.file(
            "/workspace/MODULE.bazel",
            &format!(
                "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\", repo_name = \"{apparent}\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n"
            ),
            variant,
        );
        builder.file("/workspace/dep/BUILD.bazel", build, variant);
        builder.package("viewer", "filegroup(name = \"caller\")\n", variant);
        builder.package_at(
            "/workspace/dep",
            "viewer",
            "BUILD.bazel",
            "filegroup(name = \"caller\")\n",
            variant,
        );
        builder
    }

    fn external_macro_package(variant: i64) -> Self {
        let mut builder = Self::external_package(variant);
        builder.directory("/workspace/dep/macro", variant);
        builder.directory_entries("/workspace/dep/macro", &[]);
        builder.file(
            "/workspace/dep/macro/BUILD.bazel",
            "load(\":defs.bzl\", \"make_filegroup\")\nmake_filegroup(name = \"macro_files\")\n",
            variant,
        );
        builder.file(
            "/workspace/dep/macro/defs.bzl",
            "def make_filegroup(name):\n    native.filegroup(name = name)\n",
            variant,
        );
        // A same-path root package makes accidental root companion discovery
        // observable without participating in the external package owner.
        builder.package("macro", "filegroup(name = \"root_sentinel\")\n", variant);
        builder
    }

    fn external_module_cycle(variant: i64) -> Self {
        let mut builder = Self::external_macro_package(variant);
        builder.file(
            "/workspace/dep/MODULE.bazel",
            "module(name = \"dep\", version = \"1.0.0\")\ninclude(\"//cycle:a.MODULE.bazel\")\n",
            variant,
        );
        builder.directory("/workspace/dep/cycle", variant);
        builder.directory_entries("/workspace/dep/cycle", &[]);
        builder.file("/workspace/dep/cycle/BUILD.bazel", "", variant);
        builder.file(
            "/workspace/dep/cycle/a.MODULE.bazel",
            "include(\"//cycle:b.MODULE.bazel\")\n",
            variant,
        );
        builder.file(
            "/workspace/dep/cycle/b.MODULE.bazel",
            "include(\"//cycle:a.MODULE.bazel\")\n",
            variant,
        );
        for path in [
            "/workspace/dep/macro",
            "/workspace/dep/macro/BUILD.bazel",
            "/workspace/dep/macro/defs.bzl",
        ] {
            builder.omit(path, PathObservationOperation::Lstat);
            builder.omit(path, PathObservationOperation::FileBytes);
        }
        builder.omit(
            "/workspace/dep/macro",
            PathObservationOperation::DirectoryEntries,
        );
        builder
    }

    fn external_starlark_package(variant: i64) -> Self {
        let mut builder = Self::external_package(variant);
        builder.directory("/workspace/dep/rule", variant);
        builder.directory_entries("/workspace/dep/rule", &[]);
        builder.file(
            "/workspace/dep/rule/BUILD.bazel",
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\", empty = [], visibility = [\"//visibility:public\"])\n",
            variant,
        );
        builder.file(
            "/workspace/dep/rule/defs.bzl",
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"empty\": attr.label_list()})\n",
            variant,
        );
        builder
    }

    fn package(&mut self, name: &str, source: &str, variant: i64) {
        self.package_at("/workspace", name, "BUILD.bazel", source, variant);
    }

    fn package_at(&mut self, root: &str, name: &str, marker: &str, source: &str, variant: i64) {
        let directory = if name.is_empty() {
            root.to_owned()
        } else {
            format!("{root}/{name}")
        };
        self.directory(&directory, variant);
        self.directory_entries(&directory, &[]);
        self.file(&format!("{directory}/{marker}"), source, variant);
    }

    fn rules(&mut self, implementation: &str, variant: i64) {
        self.rules_with_marker(
            implementation,
            "BUILD.bazel",
            PathNodeKind::RegularFile,
            variant,
        );
    }

    fn rules_with_marker(
        &mut self,
        implementation: &str,
        marker: &str,
        kind: PathNodeKind,
        variant: i64,
    ) {
        self.directory("/workspace/rules", variant);
        self.directory_entries("/workspace/rules", &[]);
        if marker == "BUILD" {
            self.missing("/workspace/rules/BUILD.bazel");
        }
        if kind == PathNodeKind::RegularFile {
            self.file(&format!("/workspace/rules/{marker}"), "", variant);
        } else {
            self.node(&format!("/workspace/rules/{marker}"), kind, variant);
        }
        self.file(
            "/workspace/rules/defs.bzl",
            &format!("def make(name):\n    native.{implementation}(name = name)\n"),
            variant,
        );
    }

    fn rules_without_marker(&mut self, variant: i64) {
        self.directory("/workspace/rules", variant);
        self.directory_entries("/workspace/rules", &[]);
        self.missing("/workspace/rules/BUILD.bazel");
        self.missing("/workspace/rules/BUILD");
        self.file(
            "/workspace/rules/defs.bzl",
            "def make(name):\n    native.filegroup(name = name)\n",
            variant,
        );
    }

    fn symlink_rules(&mut self, variant: i64) {
        self.directory("/workspace/rules", variant);
        self.directory_entries("/workspace/rules", &[]);
        self.file("/workspace/rules/real_build", "", variant);
        self.symlink("/workspace/rules/BUILD.bazel", "real_build", variant);
        self.file(
            "/workspace/rules/defs.bzl",
            "def make(name):\n    native.filegroup(name = name)\n",
            variant,
        );
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

fn root_query_workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

#[derive(Default)]
struct RootAnchorTracker {
    typed_roots: AtomicUsize,
    forbidden: AtomicUsize,
    root_evaluated: AtomicUsize,
    root_reused: AtomicUsize,
}

impl ActivationTracker for RootAnchorTracker {
    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        if key.downcast_ref::<RootQueryCommandKey>().is_some() {
            match activation.kind() {
                DiceActivationKind::Evaluated => &self.root_evaluated,
                DiceActivationKind::Reused => &self.root_reused,
            }
            .fetch_add(1, Ordering::Relaxed);
        }
    }

    fn root_activated(&self, key: &DynKey, _activation: RootActivation) {
        if key.downcast_ref::<RootQueryCommandKey>().is_some() {
            self.typed_roots.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn key_activated(
        &self,
        key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        if key.downcast_ref::<RootModuleGraphKey>().is_some()
            || key.downcast_ref::<PackageLoadKey>().is_some()
            || key.downcast_ref::<SubtreePackageSetKey>().is_some()
            || key.downcast_ref::<WorkspaceSnapshotKey>().is_some()
            || key.downcast_ref::<WorkspaceRawSnapshotKey>().is_some()
            || key
                .downcast_ref::<WorkspaceDirectorySnapshotKey>()
                .is_some()
            || key.downcast_ref::<WorkspaceFileKey>().is_some()
            || key.downcast_ref::<WorkspaceRawFileKey>().is_some()
            || key.downcast_ref::<WorkspaceDirectoryKey>().is_some()
        {
            self.forbidden.fetch_add(1, Ordering::Relaxed);
        }
    }
}

async fn compute_root_query(
    transaction: &mut DiceTransaction,
    key: &RootQueryCommandKey,
    tracker: &RootAnchorTracker,
) -> <RootQueryCommandKey as Key>::Value {
    let before = tracker.typed_roots.load(Ordering::Relaxed);
    let value = transaction.compute(key).await.unwrap();
    assert_eq!(
        tracker.typed_roots.load(Ordering::Relaxed),
        before + 1,
        "each command compute must activate exactly one typed query root"
    );
    assert_eq!(tracker.forbidden.load(Ordering::Relaxed), 0);
    value
}

async fn root_query_transaction(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<RootAnchorTracker>,
) -> DiceTransaction {
    root_query_transaction_with_roots(dice, epoch, tracker, vec![root_query_workspace()]).await
}

async fn root_query_transaction_with_roots(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<RootAnchorTracker>,
    package_roots: Vec<NormalizedAbsolutePath>,
) -> DiceTransaction {
    root_query_transaction_with_policy(dice, epoch, tracker, package_roots, &[]).await
}

async fn root_query_transaction_with_policy(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    tracker: Arc<RootAnchorTracker>,
    package_roots: Vec<NormalizedAbsolutePath>,
    deleted_packages: &[&str],
) -> DiceTransaction {
    let user_data = UserComputationData {
        cycle_detector: Some(bzl_load_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    let mut updater = dice.updater_with_data(user_data);
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            root_query_workspace(),
            package_roots,
            deleted_packages,
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        root_query_workspace().as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let mut attributes = SmallMap::new();
    attributes.insert("path".into(), OverrideAttributeValue::String("dep".into()));
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: root_query_workspace(),
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
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
            logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
        },
    });
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: root_query_workspace(),
            },
            RepositoryMaterializationResultEpoch::new(
                root_query_workspace(),
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
    updater.commit().await
}

fn root_query_key(source: &str) -> RootQueryCommandKey {
    RootQueryCommandKey::new(
        root_query_workspace(),
        source,
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap()
}

async fn external_visibility_transaction(
    dice: &Arc<Dice>,
    tracker: Arc<RootAnchorTracker>,
    variant: i64,
    apparent: &str,
    build: &str,
) -> DiceTransaction {
    root_query_transaction(
        dice,
        RootQueryEpochBuilder::external_visibility_package(variant, apparent, build).build(),
        tracker,
    )
    .await
}

async fn root_query_labels(
    transaction: &mut DiceTransaction,
    source: &str,
) -> Result<Vec<String>, String> {
    let QueryPreparationOutcome::Complete(result) =
        transaction.compute(&root_query_key(source)).await.unwrap()
    else {
        return Err("query requested preparation".to_owned());
    };
    result
        .as_ref()
        .as_ref()
        .map(|output| output.labels.iter().map(ToString::to_string).collect())
        .map_err(ToString::to_string)
}

#[tokio::test]
async fn external_owner_dispatches_siblings_rdeps_and_loading_files_without_root_fallback() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let mut transaction = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_package(1).build(),
        tracker,
    )
    .await;

    for (source, expected) in [
        (
            "siblings(@dep//:rule)",
            &["@dep//:BUILD.bazel", "@dep//:rule"][..],
        ),
        ("same_pkg_direct_rdeps(@dep//:rule)", &[][..]),
        ("buildfiles(@dep//:rule)", &["@dep//:BUILD.bazel"][..]),
        ("loadfiles(@dep//:rule)", &[][..]),
        (
            "visible(@dep//pkg:caller, //pkg:private)",
            &["//pkg:private"][..],
        ),
        ("visible(@dep//pkg:caller, //target:restricted)", &[][..]),
        (
            "visible(@dep//pkg:caller, //target:group_restricted)",
            &[][..],
        ),
    ] {
        let value = transaction.compute(&root_query_key(source)).await.unwrap();
        let QueryPreparationOutcome::Complete(result) = value else {
            panic!("{source} requested unexpected preparation: {value:?}")
        };
        assert_eq!(
            result.as_ref().as_ref().unwrap().labels.as_ref(),
            expected,
            "{source}"
        );
    }
}

#[tokio::test]
async fn external_module_cycle_is_typed_for_graph_and_loading_file_provenance() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_module_cycle(70).build(),
        Arc::new(RootAnchorTracker::default()),
    )
    .await;
    let expected = concat!(
        "Slug does not support MODULE.bazel include cycles in direct local_path_override ",
        "repository '@dep' for module 'dep': include \"//cycle:a.MODULE.bazel\" at ",
        "/workspace/dep/cycle/b.MODULE.bazel:1:1 repeats ancestor include ",
        "\"//cycle:a.MODULE.bazel\" at /workspace/dep/MODULE.bazel:2:1",
    );

    for source in [
        "@dep//macro:macro_files",
        "buildfiles(@dep//macro:defs.bzl)",
        "loadfiles(@dep//macro:defs.bzl)",
    ] {
        let value = transaction.compute(&root_query_key(source)).await.unwrap();
        let QueryPreparationOutcome::Complete(result) = value else {
            panic!("{source} requested preparation: {value:?}")
        };
        let error = result.as_ref().as_ref().unwrap_err();
        assert_eq!(error.exit_code, 7, "{source}");
        assert_eq!(
            error.error_kind(),
            "unsupported_feature",
            "{source}: {error:?}"
        );
        assert!(!error.needs_evaluation_context(), "{source}");
        assert_eq!(error.to_string(), expected, "{source}");
    }
}

#[tokio::test]
async fn external_restricted_visibility_projects_every_enabled_consumer_and_caller() {
    let build = concat!(
        "package_group(name = \"leaf\", packages = [\"//viewer\"])\n",
        "package_group(name = \"top\", includes = [\":leaf\"])\n",
        "filegroup(name = \"restricted\", srcs = [\":source.txt\"], visibility = [\":top\"])\n",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_visibility_package(20, "dep", build).build(),
        Arc::new(RootAnchorTracker::default()),
    )
    .await;
    for (source, expected) in [
        ("@dep//:restricted", &["@dep//:restricted"][..]),
        ("set(@dep//:restricted)", &["@dep//:restricted"]),
        ("let x = @dep//:restricted in $x", &["@dep//:restricted"]),
        (
            "@dep//:restricted union @dep//:restricted",
            &["@dep//:restricted"],
        ),
        ("labels(visibility, @dep//:restricted)", &["@dep//:top"]),
        (
            "deps(@dep//:restricted)",
            &[
                "@dep//:leaf",
                "@dep//:restricted",
                "@dep//:source.txt",
                "@dep//:top",
            ],
        ),
        (
            "deps(@dep//:restricted, 1)",
            &["@dep//:restricted", "@dep//:source.txt", "@dep//:top"],
        ),
        ("same_pkg_direct_rdeps(@dep//:top)", &["@dep//:restricted"]),
        (
            "rdeps(set(@dep//:leaf @dep//:top @dep//:restricted), @dep//:leaf)",
            &["@dep//:leaf", "@dep//:restricted", "@dep//:top"],
        ),
        (
            "allpaths(@dep//:restricted, @dep//:leaf)",
            &["@dep//:leaf", "@dep//:restricted", "@dep//:top"],
        ),
        (
            "somepath(@dep//:restricted, @dep//:leaf)",
            &["@dep//:restricted", "@dep//:top", "@dep//:leaf"],
        ),
        (
            "siblings(@dep//:restricted)",
            &[
                "@dep//:BUILD.bazel",
                "@dep//:leaf",
                "@dep//:restricted",
                "@dep//:source.txt",
                "@dep//:top",
            ],
        ),
        ("some(@dep//:restricted)", &["@dep//:restricted"]),
        ("labels(srcs, @dep//:restricted)", &["@dep//:source.txt"]),
        ("buildfiles(@dep//:restricted)", &["@dep//:BUILD.bazel"]),
        ("loadfiles(@dep//:restricted)", &[]),
        ("tests(@dep//:restricted)", &[]),
        ("executables(@dep//:restricted)", &[]),
        ("visible(//viewer:caller, @dep//:restricted)", &[]),
        (
            "visible(@dep//viewer:caller, @dep//:restricted)",
            &["@dep//:restricted"],
        ),
    ] {
        let QueryPreparationOutcome::Complete(result) =
            transaction.compute(&root_query_key(source)).await.unwrap()
        else {
            panic!("{source} requested preparation")
        };
        assert_eq!(
            result.as_ref().as_ref().unwrap().labels.as_ref(),
            expected,
            "{source}"
        );
    }
    let graph = transaction
        .compute(&root_query_key("deps(@dep//:restricted)"))
        .await
        .unwrap();
    let QueryPreparationOutcome::Complete(graph) = graph else {
        panic!("graph requested preparation")
    };
    assert_eq!(
        graph.as_ref().as_ref().unwrap().graph_stdout(true, true),
        "digraph mygraph {\n  node [shape=box];\n  \"@dep//:restricted\"\n  \"@dep//:restricted\" -> \"@dep//:source.txt\"\n  \"@dep//:restricted\" -> \"@dep//:top\"\n  \"@dep//:top\"\n  \"@dep//:top\" -> \"@dep//:leaf\"\n  \"@dep//:source.txt\"\n  \"@dep//:leaf\"\n}\n"
    );
    let full = RootQueryCommandKey::new(
        root_query_workspace(),
        "deps(@dep//:restricted)",
        QueryOrder::Full,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(full) = transaction.compute(&full).await.unwrap() else {
        panic!("full order requested preparation")
    };
    assert_eq!(
        full.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "@dep//:restricted",
            "@dep//:source.txt",
            "@dep//:top",
            "@dep//:leaf",
        ]
    );
    let kind = RootQueryCommandKey::new(
        root_query_workspace(),
        "@dep//:restricted",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::LabelKind,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(kind) = transaction.compute(&kind).await.unwrap() else {
        panic!("kind requested preparation")
    };
    let kind = kind.as_ref().as_ref().unwrap();
    assert_eq!(kind.stdout(), "@dep//:restricted\n");
    assert_eq!(
        kind.label_kind_stdout(),
        "filegroup rule @dep//:restricted\n"
    );
    assert_eq!(kind.package_stdout(), "@dep//\n");
}

#[tokio::test]
async fn external_restricted_visibility_rejections_precede_competing_synthesis_failures() {
    let sentinel = concat!(
        "alias(name = \"sentinel_a\", actual = \":sentinel_b\")\n",
        "alias(name = \"sentinel_b\", actual = \":source\")\n",
    );
    for (source, expected) in [
        (
            "package(default_visibility = [\":group\"])\npackage_group(name = \"group\")\nfilegroup(name = \"restricted\")\n",
            "Restricted package default visibility",
        ),
        (
            "package_group(name = \"group\")\nfilegroup(name = \"restricted\", visibility = [\":group\"])\nfilegroup(name = \"second\", visibility = [\":group\"])\n",
            "second external repository Restricted target",
        ),
        (
            "package_group(name = \"group\")\nalias(name = \"restricted\", actual = \":missing\", visibility = [\":group\"])\n",
            "deferred for non-filegroup",
        ),
        (
            "filegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\"//viewer:__pkg__\"])\n",
            "direct package visibility",
        ),
        (
            "filegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\"//other:group\"])\n",
            "cross-package group",
        ),
        (
            "filegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\"@dep//:group\"])\n",
            "external repository dependency labels are not supported",
        ),
        (
            "filegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\":missing\"])\n",
            "missing group",
        ),
        (
            "alias(name = \"group\", actual = \":missing\")\nfilegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\":group\"])\n",
            "alias group",
        ),
        (
            "filegroup(name = \"group\")\nfilegroup(name = \"restricted\", srcs = [\"//other:source\"], visibility = [\":group\"])\n",
            "wrong-kind group",
        ),
    ] {
        let build = format!("{source}{sentinel}");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction = root_query_transaction(
            &dice,
            RootQueryEpochBuilder::external_visibility_package(21, "dep", &build).build(),
            Arc::new(RootAnchorTracker::default()),
        )
        .await;
        let QueryPreparationOutcome::Complete(result) = transaction
            .compute(&root_query_key("@dep//:restricted"))
            .await
            .unwrap()
        else {
            panic!("rejection requested preparation")
        };
        let actual = result.as_ref().as_ref().unwrap_err().to_string();
        assert!(actual.contains(expected), "{expected}: {actual}");
        assert!(!actual.contains("alias chains are deferred"), "{actual}");
        assert!(!actual.contains("filegroup source is deferred"), "{actual}");
    }
}

#[tokio::test]
async fn external_restricted_visibility_tracks_every_edit_and_a_to_b_to_a_lifecycle() {
    let allowed = concat!(
        "package_group(name = \"base\", packages = [\"//viewer/...\", \"-//viewer\"], includes = [\":cycle\"])\n",
        "package_group(name = \"cycle\", includes = [\":base\"])\n",
        "package_group(name = \"reallow\", packages = [\"//viewer\"])\n",
        "package_group(name = \"alt\", packages = [\"//other\"])\n",
        "package_group(name = \"top\", includes = [\":base\", \":reallow\"])\n",
        "filegroup(name = \"restricted\", visibility = [\":top\"])\n",
    );
    let content_b = allowed.replace(
        "name = \"reallow\", packages = [\"//viewer\"]",
        "name = \"reallow\", packages = [\"//other\"]",
    );
    let visibility_b = allowed.replace("visibility = [\":top\"]", "visibility = [\":alt\"]");
    let include_b = allowed.replace(
        "includes = [\":base\", \":reallow\"]",
        "includes = [\":base\", \":alt\"]",
    );
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let visible = "visible(@dep//viewer:caller, @dep//:restricted)";
    let key = root_query_key(visible);
    let mut cold =
        external_visibility_transaction(&dice, tracker.clone(), 30, "dep", allowed).await;
    let initial = compute_root_query(&mut cold, &key, &tracker).await;
    let QueryPreparationOutcome::Complete(result) = &initial else {
        panic!("cold requested preparation")
    };
    assert_eq!(
        result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["@dep//:restricted"]
    );
    assert_eq!(tracker.root_evaluated.load(Ordering::Relaxed), 1);
    assert_eq!(tracker.root_reused.load(Ordering::Relaxed), 0);
    let warm = compute_root_query(&mut cold, &key, &tracker).await;
    assert_eq!(tracker.root_evaluated.load(Ordering::Relaxed), 1);
    assert_eq!(tracker.root_reused.load(Ordering::Relaxed), 1);
    assert!(RootQueryCommandKey::equality(&initial, &warm));

    let mut content =
        external_visibility_transaction(&dice, tracker.clone(), 31, "dep", &content_b).await;
    assert!(
        root_query_labels(&mut content, visible)
            .await
            .unwrap()
            .is_empty()
    );
    let mut content_a =
        external_visibility_transaction(&dice, tracker.clone(), 32, "dep", allowed).await;
    let content_a = content_a.compute(&key).await.unwrap();
    assert!(RootQueryCommandKey::equality(&initial, &content_a));

    let mut visibility =
        external_visibility_transaction(&dice, tracker.clone(), 33, "dep", &visibility_b).await;
    assert_eq!(
        root_query_labels(&mut visibility, "labels(visibility, @dep//:restricted)")
            .await
            .unwrap(),
        ["@dep//:alt"]
    );
    assert!(
        root_query_labels(&mut visibility, visible)
            .await
            .unwrap()
            .is_empty()
    );
    let mut visibility_a =
        external_visibility_transaction(&dice, tracker.clone(), 34, "dep", allowed).await;
    let visibility_a = visibility_a.compute(&key).await.unwrap();
    assert!(RootQueryCommandKey::equality(&initial, &visibility_a));

    let mut include =
        external_visibility_transaction(&dice, tracker.clone(), 35, "dep", &include_b).await;
    assert!(
        root_query_labels(&mut include, visible)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        root_query_labels(&mut include, "deps(@dep//:restricted)")
            .await
            .unwrap()
            .contains(&"@dep//:alt".to_owned())
    );
    let mut include_a =
        external_visibility_transaction(&dice, tracker.clone(), 36, "dep", allowed).await;
    let include_a = include_a.compute(&key).await.unwrap();
    assert!(RootQueryCommandKey::equality(&initial, &include_a));

    let mut deleted_epoch = RootQueryEpochBuilder::external_visibility_package(37, "dep", allowed);
    deleted_epoch.missing("/workspace/dep/BUILD.bazel");
    deleted_epoch.missing("/workspace/dep/BUILD");
    let mut deleted = root_query_transaction(&dice, deleted_epoch.build(), tracker.clone()).await;
    assert!(
        root_query_labels(&mut deleted, visible)
            .await
            .unwrap_err()
            .contains("BUILD file not found")
    );
    let mut recreated = external_visibility_transaction(&dice, tracker, 38, "dep", allowed).await;
    let recreated = recreated.compute(&key).await.unwrap();
    assert!(RootQueryCommandKey::equality(&initial, &recreated));
}

#[tokio::test]
async fn external_restricted_visibility_remaps_apparent_route_and_rejects_stale_route() {
    let build = "package_group(name = \"group\", packages = [\"//viewer\"])\nfilegroup(name = \"restricted\", visibility = [\":group\"])\n";
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let mut original =
        external_visibility_transaction(&dice, tracker.clone(), 40, "dep", build).await;
    assert_eq!(
        root_query_labels(&mut original, "labels(visibility, @dep//:restricted)")
            .await
            .unwrap(),
        ["@dep//:group"]
    );

    let mut renamed = external_visibility_transaction(&dice, tracker, 41, "friend", build).await;
    assert_eq!(
        root_query_labels(&mut renamed, "labels(visibility, @friend//:restricted)")
            .await
            .unwrap(),
        ["@friend//:group"]
    );
    assert_eq!(
        root_query_labels(
            &mut renamed,
            "visible(@friend//viewer:caller, @friend//:restricted)"
        )
        .await
        .unwrap(),
        ["@friend//:restricted"]
    );
    let stale = root_query_labels(&mut renamed, "@dep//:restricted")
        .await
        .unwrap_err();
    assert!(stale.contains("No repository visible as '@dep'"), "{stale}");
}

#[tokio::test]
async fn external_macro_loading_files_preserve_owner_fake_consumers_and_output_seams() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let mut transaction = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_macro_package(5).build(),
        tracker,
    )
    .await;

    for (source, expected) in [
        ("@dep//macro:macro_files", &["@dep//macro:macro_files"][..]),
        (
            "loadfiles(@dep//macro:macro_files)",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "buildfiles(@dep//macro:macro_files)",
            &["@dep//macro:BUILD.bazel", "@dep//macro:defs.bzl"][..],
        ),
        (
            "deps(loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "rdeps(loadfiles(@dep//macro:macro_files), loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "allpaths(loadfiles(@dep//macro:macro_files), loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "somepath(loadfiles(@dep//macro:macro_files), loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "some(loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "siblings(loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:BUILD.bazel", "@dep//macro:macro_files"][..],
        ),
        (
            "visible(@dep//macro:macro_files, loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "loadfiles(loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "buildfiles(loadfiles(@dep//macro:macro_files))",
            &["@dep//macro:BUILD.bazel", "@dep//macro:defs.bzl"][..],
        ),
        (
            "loadfiles(@dep//macro:macro_files) union loadfiles(@dep//macro:macro_files)",
            &["@dep//macro:defs.bzl"][..],
        ),
        (
            "same_pkg_direct_rdeps(loadfiles(@dep//macro:macro_files))",
            &[][..],
        ),
        ("labels(srcs, loadfiles(@dep//macro:macro_files))", &[][..]),
        ("executables(loadfiles(@dep//macro:macro_files))", &[][..]),
        ("tests(loadfiles(@dep//macro:macro_files))", &[][..]),
    ] {
        let value = transaction.compute(&root_query_key(source)).await.unwrap();
        let QueryPreparationOutcome::Complete(result) = value else {
            panic!("{source} requested unexpected preparation: {value:?}")
        };
        assert_eq!(
            result.as_ref().as_ref().unwrap().labels.as_ref(),
            expected,
            "{source}"
        );
    }

    let literal_kind = RootQueryCommandKey::new(
        root_query_workspace(),
        "@dep//macro:macro_files",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::LabelKind,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(literal_kind) =
        transaction.compute(&literal_kind).await.unwrap()
    else {
        panic!("external macro label_kind requested preparation")
    };
    assert_eq!(
        literal_kind.as_ref().as_ref().unwrap().label_kind_stdout(),
        "filegroup rule @dep//macro:macro_files\n"
    );

    let fake_kind = RootQueryCommandKey::new(
        root_query_workspace(),
        "loadfiles(@dep//macro:macro_files)",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::LabelKind,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(fake_kind) =
        transaction.compute(&fake_kind).await.unwrap()
    else {
        panic!("external fake label_kind requested preparation")
    };
    let fake = fake_kind.as_ref().as_ref().unwrap();
    assert_eq!(
        fake.label_kind_stdout(),
        "source file @dep//macro:defs.bzl\n"
    );
    assert_eq!(fake.package_stdout(), "@dep//macro\n");
    assert_eq!(
        fake.graph_stdout(false, true),
        concat!(
            "digraph mygraph {\n",
            "  node [shape=box];\n",
            "  \"@dep//macro:defs.bzl\"\n",
            "}\n",
        )
    );

    let full = RootQueryCommandKey::new(
        root_query_workspace(),
        "buildfiles(@dep//macro:macro_files)",
        QueryOrder::Full,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(full) = transaction.compute(&full).await.unwrap() else {
        panic!("external full buildfiles requested preparation")
    };
    assert_eq!(
        full.as_ref().as_ref().unwrap().labels.as_ref(),
        ["@dep//macro:defs.bzl"]
    );
}

#[tokio::test]
async fn external_dependency_free_starlark_rule_projects_all_enabled_consumers_and_formats() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_starlark_package(6).build(),
        Arc::new(RootAnchorTracker::default()),
    )
    .await;
    for (source, expected) in [
        ("@dep//rule:probe", &["@dep//rule:probe"][..]),
        ("deps(@dep//rule:probe)", &["@dep//rule:probe"][..]),
        ("some(@dep//rule:probe)", &["@dep//rule:probe"][..]),
        (
            "rdeps(@dep//rule:probe, @dep//rule:probe)",
            &["@dep//rule:probe"][..],
        ),
        (
            "allpaths(@dep//rule:probe, @dep//rule:probe)",
            &["@dep//rule:probe"][..],
        ),
        (
            "somepath(@dep//rule:probe, @dep//rule:probe)",
            &["@dep//rule:probe"][..],
        ),
        (
            "siblings(@dep//rule:probe)",
            &["@dep//rule:BUILD.bazel", "@dep//rule:probe"][..],
        ),
        ("same_pkg_direct_rdeps(@dep//rule:probe)", &[][..]),
        ("labels(empty, @dep//rule:probe)", &[][..]),
        ("labels(visibility, @dep//rule:probe)", &[][..]),
        ("tests(@dep//rule:probe)", &[][..]),
        ("executables(@dep//rule:probe)", &[][..]),
        (
            "visible(@dep//rule:probe, @dep//rule:probe)",
            &["@dep//rule:probe"][..],
        ),
        ("loadfiles(@dep//rule:probe)", &["@dep//rule:defs.bzl"][..]),
        (
            "buildfiles(@dep//rule:probe)",
            &["@dep//rule:BUILD.bazel", "@dep//rule:defs.bzl"][..],
        ),
    ] {
        let QueryPreparationOutcome::Complete(result) =
            transaction.compute(&root_query_key(source)).await.unwrap()
        else {
            panic!("{source} requested preparation")
        };
        assert_eq!(
            result.as_ref().as_ref().unwrap().labels.as_ref(),
            expected,
            "{source}"
        );
    }
    for (completion, expected) in [
        (
            QueryOutputCompletion::LabelKind,
            "probe rule @dep//rule:probe\n",
        ),
        (QueryOutputCompletion::Standard, "@dep//rule:probe\n"),
    ] {
        let key = RootQueryCommandKey::new(
            root_query_workspace(),
            "@dep//rule:probe",
            QueryOrder::Auto,
            QueryPolicy::default(),
            completion,
        )
        .unwrap();
        let QueryPreparationOutcome::Complete(result) = transaction.compute(&key).await.unwrap()
        else {
            panic!("format requested preparation")
        };
        let result = result.as_ref().as_ref().unwrap();
        let actual = if completion == QueryOutputCompletion::LabelKind {
            result.label_kind_stdout()
        } else {
            result.stdout()
        };
        assert_eq!(actual, expected);
        assert_eq!(result.package_stdout(), "@dep//rule\n");
        assert_eq!(
            result.graph_stdout(false, true),
            "digraph mygraph {\n  node [shape=box];\n  \"@dep//rule:probe\"\n}\n"
        );
    }
}

#[tokio::test]
async fn external_owner_route_lifecycle_reuses_edits_deletes_recreates_and_recovers() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let key = root_query_key("buildfiles(@dep//:rule)");

    let mut cold = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::external_package(10).build(),
        tracker.clone(),
    )
    .await;
    let cold_value = cold.compute(&key).await.unwrap();
    let QueryPreparationOutcome::Complete(cold_result) = &cold_value else {
        panic!("cold external owner requested preparation: {cold_value:?}")
    };
    assert_eq!(
        cold_result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["@dep//:BUILD.bazel"]
    );
    let warm_value = cold.compute(&key).await.unwrap();
    assert!(RootQueryCommandKey::equality(&cold_value, &warm_value));

    let mut edited_epoch = RootQueryEpochBuilder::external_package(11);
    edited_epoch.file(
        "/workspace/dep/BUILD.bazel",
        "filegroup(name = \"edited\")\n",
        11,
    );
    let mut edited = root_query_transaction(&dice, edited_epoch.build(), tracker.clone()).await;
    let edited_key = root_query_key("buildfiles(@dep//:edited)");
    let edited_value = edited.compute(&edited_key).await.unwrap();
    let QueryPreparationOutcome::Complete(edited_result) = &edited_value else {
        panic!("edited external owner requested preparation: {edited_value:?}")
    };
    assert_eq!(
        edited_result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["@dep//:BUILD.bazel"]
    );

    let mut deleted_epoch = RootQueryEpochBuilder::external_package(12);
    deleted_epoch.missing("/workspace/dep/BUILD.bazel");
    deleted_epoch.missing("/workspace/dep/BUILD");
    let mut deleted = root_query_transaction(&dice, deleted_epoch.build(), tracker.clone()).await;
    let deleted_value = deleted.compute(&edited_key).await.unwrap();
    let QueryPreparationOutcome::Complete(deleted_result) = &deleted_value else {
        panic!("deleted external owner requested preparation: {deleted_value:?}")
    };
    assert!(
        deleted_result
            .as_ref()
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("BUILD file not found")
    );

    let mut recreated_epoch = RootQueryEpochBuilder::external_package(13);
    recreated_epoch.file(
        "/workspace/dep/BUILD.bazel",
        "filegroup(name = \"edited\")\n",
        13,
    );
    let mut recreated = root_query_transaction(&dice, recreated_epoch.build(), tracker).await;
    let recreated_value = recreated.compute(&edited_key).await.unwrap();
    let QueryPreparationOutcome::Complete(recreated_result) = &recreated_value else {
        panic!("recreated external owner requested preparation: {recreated_value:?}")
    };
    assert_eq!(
        recreated_result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["@dep//:BUILD.bazel"]
    );
}

#[tokio::test]
async fn typed_root_query_anchors_empty_results_and_preserves_lazy_need_control() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    for invalid in ["deps(", "attr(name, value, //first:t)"] {
        assert!(
            RootQueryCommandKey::new(
                root_query_workspace(),
                invalid,
                QueryOrder::Auto,
                QueryPolicy::default(),
                QueryOutputCompletion::Standard,
            )
            .is_err()
        );
    }
    let empty_key = root_query_key("set()");
    let variants = [
        RootQueryCommandKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
            "set()",
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::Standard,
        )
        .unwrap(),
        RootQueryCommandKey::new(
            root_query_workspace(),
            "set(//first:t)",
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::Standard,
        )
        .unwrap(),
        RootQueryCommandKey::new(
            root_query_workspace(),
            "set()",
            QueryOrder::Full,
            QueryPolicy::default(),
            QueryOutputCompletion::Standard,
        )
        .unwrap(),
        RootQueryCommandKey::new(
            root_query_workspace(),
            "set()",
            QueryOrder::Auto,
            QueryPolicy {
                strict_test_suite: true,
            },
            QueryOutputCompletion::Standard,
        )
        .unwrap(),
        RootQueryCommandKey::new(
            root_query_workspace(),
            "set()",
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::LabelKind,
        )
        .unwrap(),
    ];
    assert!(variants.iter().all(|variant| variant != &empty_key));

    let mut no_anchor = root_query_transaction(
        &dice,
        PathObservationEpoch::new(SmallMap::new()).unwrap(),
        tracker.clone(),
    )
    .await;
    let empty_need = compute_root_query(&mut no_anchor, &empty_key, &tracker).await;
    assert!(matches!(empty_need, QueryPreparationOutcome::Need(_)));
    assert!(!RootQueryCommandKey::validity(&empty_need));
    assert!(!RootQueryCommandKey::equality(&empty_need, &empty_need));

    let mut base = root_query_transaction(
        &dice,
        RootQueryEpochBuilder::base(1).build(),
        tracker.clone(),
    )
    .await;
    let empty = compute_root_query(&mut base, &empty_key, &tracker).await;
    let QueryPreparationOutcome::Complete(empty_result) = &empty else {
        panic!("valid empty query did not complete after its root anchor");
    };
    assert!(empty_result.as_ref().as_ref().unwrap().labels.is_empty());
    assert!(RootQueryCommandKey::validity(&empty));
    assert!(RootQueryCommandKey::equality(&empty, &empty));

    let lazy_key = root_query_key("//first:t union //later:t");
    let lazy_need = compute_root_query(&mut base, &lazy_key, &tracker).await;
    let QueryPreparationOutcome::Need(needs) = &lazy_need else {
        panic!("missing first package escaped as a semantic QueryError");
    };
    let paths = needs
        .path_observations()
        .unwrap()
        .demands()
        .iter()
        .map(|demand| demand.path().as_path())
        .collect::<Vec<_>>();
    assert!(paths.contains(&Path::new("/workspace/first")));
    assert!(!paths.contains(&Path::new("/workspace/later")));

    let mut complete_epoch = RootQueryEpochBuilder::base(2);
    complete_epoch.rules("filegroup", 2);
    complete_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        2,
    );
    complete_epoch.package("later", "filegroup(name = \"t\")\n", 2);
    let mut complete = root_query_transaction(&dice, complete_epoch.build(), tracker.clone()).await;
    let result = compute_root_query(&mut complete, &lazy_key, &tracker).await;
    let QueryPreparationOutcome::Complete(query_result) = &result else {
        panic!("complete typed root query returned Need");
    };
    assert_eq!(
        query_result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//first:t", "//later:t"]
    );
    assert!(RootQueryCommandKey::equality(&result, &result));

    let full_key = RootQueryCommandKey::new(
        root_query_workspace(),
        "set(//later:t //first:t)",
        QueryOrder::Full,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(full) =
        compute_root_query(&mut complete, &full_key, &tracker).await
    else {
        panic!("Full order returned Need")
    };
    assert_eq!(
        full.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//later:t", "//first:t"]
    );

    let loadfiles_key = root_query_key("loadfiles(//first:t)");
    let QueryPreparationOutcome::Complete(loadfiles) =
        compute_root_query(&mut complete, &loadfiles_key, &tracker).await
    else {
        panic!("loadfiles returned Need")
    };
    assert_eq!(
        loadfiles.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//rules:defs.bzl"]
    );
    let buildfiles_key = root_query_key("buildfiles(//first:t)");
    let QueryPreparationOutcome::Complete(buildfiles) =
        compute_root_query(&mut complete, &buildfiles_key, &tracker).await
    else {
        panic!("buildfiles returned Need")
    };
    assert_eq!(
        buildfiles.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "//first:BUILD.bazel",
            "//rules:BUILD.bazel",
            "//rules:defs.bzl"
        ]
    );

    let mut broken_companion_epoch = RootQueryEpochBuilder::base(21);
    broken_companion_epoch.rules("filegroup", 21);
    broken_companion_epoch.file(
        "/workspace/rules/BUILD.bazel",
        "this is not valid BUILD syntax (",
        21,
    );
    broken_companion_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        21,
    );
    let mut broken_companion =
        root_query_transaction(&dice, broken_companion_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(broken_companion) =
        compute_root_query(&mut broken_companion, &buildfiles_key, &tracker).await
    else {
        panic!("broken companion BUILD returned Need")
    };
    assert_eq!(
        broken_companion.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "//first:BUILD.bazel",
            "//rules:BUILD.bazel",
            "//rules:defs.bzl"
        ]
    );

    let kind_key = RootQueryCommandKey::new(
        root_query_workspace(),
        "//first:t",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::LabelKind,
    )
    .unwrap();
    let QueryPreparationOutcome::Complete(v1) =
        compute_root_query(&mut complete, &kind_key, &tracker).await
    else {
        panic!("kind returned Need")
    };
    assert_eq!(
        v1.as_ref().as_ref().unwrap().label_kind_stdout(),
        "filegroup rule //first:t\n"
    );

    let mut edited_epoch = RootQueryEpochBuilder::base(3);
    edited_epoch.rules_with_marker("test_suite", "BUILD", PathNodeKind::RegularFile, 3);
    edited_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        3,
    );
    let mut edited = root_query_transaction(&dice, edited_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(v2) =
        compute_root_query(&mut edited, &kind_key, &tracker).await
    else {
        panic!("edit returned Need")
    };
    assert_eq!(
        v2.as_ref().as_ref().unwrap().label_kind_stdout(),
        "test_suite rule //first:t\n"
    );
    let QueryPreparationOutcome::Complete(fallback) =
        compute_root_query(&mut edited, &buildfiles_key, &tracker).await
    else {
        panic!("fallback companion returned Need")
    };
    assert_eq!(
        fallback.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//first:BUILD.bazel", "//rules:BUILD", "//rules:defs.bzl"]
    );

    let mut special_epoch = RootQueryEpochBuilder::base(31);
    special_epoch.rules_with_marker("filegroup", "BUILD.bazel", PathNodeKind::SpecialFile, 31);
    special_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        31,
    );
    let mut special = root_query_transaction(&dice, special_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(special) =
        compute_root_query(&mut special, &buildfiles_key, &tracker).await
    else {
        panic!("special-file companion returned Need")
    };
    assert_eq!(
        special.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "//first:BUILD.bazel",
            "//rules:BUILD.bazel",
            "//rules:defs.bzl"
        ]
    );

    let mut symlink_epoch = RootQueryEpochBuilder::base(32);
    symlink_epoch.symlink_rules(32);
    symlink_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        32,
    );
    let mut symlink = root_query_transaction(&dice, symlink_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(symlink) =
        compute_root_query(&mut symlink, &buildfiles_key, &tracker).await
    else {
        panic!("symlink companion returned Need")
    };
    assert_eq!(
        symlink.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "//first:BUILD.bazel",
            "//rules:BUILD.bazel",
            "//rules:defs.bzl"
        ]
    );

    let mut deleted_epoch = RootQueryEpochBuilder::base(4);
    deleted_epoch.rules_without_marker(4);
    deleted_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        4,
    );
    let mut deleted = root_query_transaction(&dice, deleted_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(deleted) =
        compute_root_query(&mut deleted, &buildfiles_key, &tracker).await
    else {
        panic!("missing companion marker returned Need")
    };
    assert!(deleted.as_ref().is_err());

    let mut restored_epoch = RootQueryEpochBuilder::base(5);
    restored_epoch.rules("filegroup", 5);
    restored_epoch.package(
        "first",
        "load(\"//rules:defs.bzl\", \"make\")\nmake(name = \"t\")\n",
        5,
    );
    let mut restored = root_query_transaction(&dice, restored_epoch.build(), tracker.clone()).await;
    let QueryPreparationOutcome::Complete(restored) =
        compute_root_query(&mut restored, &buildfiles_key, &tracker).await
    else {
        panic!("restored companion marker returned Need")
    };
    assert_eq!(
        restored.as_ref().as_ref().unwrap().labels.as_ref(),
        [
            "//first:BUILD.bazel",
            "//rules:BUILD.bazel",
            "//rules:defs.bzl"
        ]
    );
}

fn multi_root_epoch(variant: i64) -> RootQueryEpochBuilder {
    let mut epoch = RootQueryEpochBuilder::base(variant);
    for root in ["/root-a", "/root-b"] {
        epoch.directory(root, variant);
        epoch.missing(&format!("{root}/.bazelignore"));
        epoch.missing(&format!("{root}/BUILD.bazel"));
        epoch.missing(&format!("{root}/BUILD"));
    }
    epoch
}

#[tokio::test]
async fn typed_recursive_query_unions_package_roots_and_replays_package_lifecycle() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let roots = || {
        vec![
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]
    };
    let key = root_query_key("//...");

    let need_epoch = multi_root_epoch(10);
    let mut need =
        root_query_transaction_with_roots(&dice, need_epoch.build(), tracker.clone(), roots())
            .await;
    let QueryPreparationOutcome::Need(needs) = compute_root_query(&mut need, &key, &tracker).await
    else {
        panic!("missing root listings escaped as QueryError");
    };
    let paths = needs
        .path_observations()
        .unwrap()
        .demands()
        .iter()
        .map(|demand| demand.path().as_path())
        .collect::<Vec<_>>();
    assert!(paths.contains(&Path::new("/root-a")), "{paths:?}");
    assert!(paths.contains(&Path::new("/root-b")), "{paths:?}");

    let complete_epoch = |variant: i64, alpha_name: &str, include_beta: bool| {
        let mut epoch = multi_root_epoch(variant);
        epoch.directory_entries("/root-a", &["shared", "alpha"]);
        epoch.directory_entries("/root-b", &["beta", "shared"]);
        epoch.package_at(
            "/root-a",
            "alpha",
            "BUILD.bazel",
            &format!("filegroup(name = \"{alpha_name}\")\n"),
            variant,
        );
        epoch.missing("/root-b/alpha");
        epoch.missing("/root-a/beta");
        epoch.directory("/root-b/beta", variant);
        epoch.directory_entries("/root-b/beta", &[]);
        if include_beta {
            epoch.file(
                "/root-b/beta/BUILD.bazel",
                "filegroup(name = \"t\")\n",
                variant,
            );
        } else {
            epoch.missing("/root-b/beta/BUILD.bazel");
            epoch.missing("/root-b/beta/BUILD");
        }
        epoch.package_at(
            "/root-a",
            "shared",
            "BUILD",
            "filegroup(name = \"root_a\")\n",
            variant,
        );
        epoch.missing("/root-a/shared/BUILD.bazel");
        epoch.package_at(
            "/root-b",
            "shared",
            "BUILD.bazel",
            "filegroup(name = \"root_b\")\n",
            variant,
        );
        epoch
    };

    let mut complete = root_query_transaction_with_roots(
        &dice,
        complete_epoch(11, "t", true).build(),
        tracker.clone(),
        roots(),
    )
    .await;
    let QueryPreparationOutcome::Complete(result) =
        compute_root_query(&mut complete, &key, &tracker).await
    else {
        panic!("complete recursive query returned Need");
    };
    assert_eq!(
        result.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:t", "//beta:t", "//shared:root_a"]
    );
    assert_eq!(tracker.forbidden.load(Ordering::Relaxed), 0);

    let mut policy_deleted = root_query_transaction_with_policy(
        &dice,
        complete_epoch(111, "t", true).build(),
        tracker.clone(),
        roots(),
        &["//beta"],
    )
    .await;
    let QueryPreparationOutcome::Complete(policy_deleted) =
        compute_root_query(&mut policy_deleted, &key, &tracker).await
    else {
        panic!("deleted-package policy returned Need");
    };
    assert_eq!(
        policy_deleted.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:t", "//shared:root_a"]
    );

    let mut ignored_epoch = complete_epoch(112, "t", true);
    ignored_epoch.file("/root-a/.bazelignore", "beta\n", 112);
    let mut ignored =
        root_query_transaction_with_roots(&dice, ignored_epoch.build(), tracker.clone(), roots())
            .await;
    let QueryPreparationOutcome::Complete(ignored) =
        compute_root_query(&mut ignored, &key, &tracker).await
    else {
        panic!("ignored-package transition returned Need");
    };
    assert_eq!(
        ignored.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:t", "//shared:root_a"]
    );

    let mut edited = root_query_transaction_with_roots(
        &dice,
        complete_epoch(12, "edited", true).build(),
        tracker.clone(),
        roots(),
    )
    .await;
    let QueryPreparationOutcome::Complete(edited) =
        compute_root_query(&mut edited, &key, &tracker).await
    else {
        panic!("recursive edit returned Need");
    };
    assert_eq!(
        edited.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:edited", "//beta:t", "//shared:root_a"]
    );

    let mut deleted = root_query_transaction_with_roots(
        &dice,
        complete_epoch(13, "edited", false).build(),
        tracker.clone(),
        roots(),
    )
    .await;
    let QueryPreparationOutcome::Complete(deleted) =
        compute_root_query(&mut deleted, &key, &tracker).await
    else {
        panic!("recursive deletion returned Need");
    };
    assert_eq!(
        deleted.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:edited", "//shared:root_a"]
    );

    let mut restored = root_query_transaction_with_roots(
        &dice,
        complete_epoch(14, "t", true).build(),
        tracker.clone(),
        roots(),
    )
    .await;
    let QueryPreparationOutcome::Complete(restored) =
        compute_root_query(&mut restored, &key, &tracker).await
    else {
        panic!("recursive restore returned Need");
    };
    assert_eq!(
        restored.as_ref().as_ref().unwrap().labels.as_ref(),
        ["//alpha:t", "//beta:t", "//shared:root_a"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn typed_recursive_query_preserves_non_utf8_directory_identity_until_a_marker_exists() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(RootAnchorTracker::default());
    let key = root_query_key("//...");
    let bad_name = OsString::from_vec(vec![b'b', b'a', b'd', 0xff]);
    let roots = || {
        vec![
            NormalizedAbsolutePath::new("/root-a").unwrap(),
            NormalizedAbsolutePath::new("/root-b").unwrap(),
        ]
    };
    let demand = |path: PathBuf, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let epoch = |variant: i64, marker: bool| {
        let mut epoch = multi_root_epoch(variant);
        for root in ["/root-a", "/root-b"] {
            epoch.entries.insert(
                RootQueryEpochBuilder::demand(root, PathObservationOperation::DirectoryEntries),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                    PathDirectoryEntries::new([PathDirectoryEntry::new(
                        PathDirectoryName::new(bad_name.clone()).unwrap(),
                        PathDirectoryEntryKind::Directory,
                    )]),
                )),
            );
            let bad_directory = PathBuf::from(root).join(&bad_name);
            epoch.entries.insert(
                demand(bad_directory.clone(), PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::Directory,
                    variant,
                    variant,
                    variant,
                    variant,
                    0o755,
                ))),
            );
            epoch.entries.insert(
                demand(
                    bad_directory.clone(),
                    PathObservationOperation::DirectoryEntries,
                ),
                PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                    PathDirectoryEntries::new([]),
                )),
            );
            for basename in ["BUILD.bazel", "BUILD"] {
                epoch.entries.insert(
                    demand(
                        bad_directory.join(basename),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Missing),
                );
            }
        }
        if marker {
            let root_a_build = PathBuf::from("/root-a").join(&bad_name).join("BUILD");
            epoch.entries.insert(
                demand(root_a_build, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    variant,
                    variant,
                    variant,
                    variant,
                    0o644,
                ))),
            );
            let later_root_b_primary = PathBuf::from("/root-b").join(&bad_name).join("BUILD.bazel");
            epoch.entries.insert(
                demand(later_root_b_primary, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Error(
                    PathObservationError::Io {
                        kind: PathIoErrorKind::PermissionDenied,
                        raw_os_error: Some(13),
                    },
                )),
            );
        }
        epoch.build()
    };

    let mut without_marker =
        root_query_transaction_with_roots(&dice, epoch(20, false), tracker.clone(), roots()).await;
    let QueryPreparationOutcome::Complete(without_marker) =
        compute_root_query(&mut without_marker, &key, &tracker).await
    else {
        panic!("non-UTF8 non-package returned Need");
    };
    assert!(
        without_marker
            .as_ref()
            .as_ref()
            .unwrap_err()
            .message
            .contains("no targets found")
    );

    let mut with_marker =
        root_query_transaction_with_roots(&dice, epoch(21, true), tracker.clone(), roots()).await;
    let QueryPreparationOutcome::Complete(with_marker) =
        compute_root_query(&mut with_marker, &key, &tracker).await
    else {
        panic!("non-UTF8 package marker returned Need");
    };
    assert!(
        with_marker
            .as_ref()
            .as_ref()
            .unwrap_err()
            .message
            .contains("package path is not UTF-8")
    );
}
