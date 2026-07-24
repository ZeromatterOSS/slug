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
use std::time::SystemTime;

use dice::ActivationData;
use dice::ActivationTracker;
use dice::DetectCycles;
use dice::Dice;
use dice::DynKey;
use dice::UserComputationData;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetAnalysisKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ProviderId;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;

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

async fn analyze_revision(
    dice: &Arc<Dice>,
    tracker: &Arc<AnalysisTracker>,
    workspace: &std::path::Path,
    key: &ConfiguredTargetKey,
) -> (Result<AnalysisResult, String>, Vec<(String, EventKind)>) {
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.clone()),
        ..Default::default()
    });
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.to_path_buf(),
            },
            Arc::new(workspace_snapshot(workspace)),
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
    (result, tracker.take())
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
                    Arc::new(WorkspaceSnapshot {
                        files: Arc::new(files),
                    }),
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
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            Arc::new(workspace_snapshot(&workspace)),
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
