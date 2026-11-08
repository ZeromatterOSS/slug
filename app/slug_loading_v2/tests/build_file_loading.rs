use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
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
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::AliasActual;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::BzlModuleIdentity;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::PackageTarget;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::TestSuiteMembership;
use slug_loading_v2::VisibilitySource;
use slug_loading_v2::attrs::AttributeDependencyConfiguration;
use slug_loading_v2::file_discovery::BUILD_FILE_FALLBACK;
use slug_loading_v2::file_discovery::BUILD_FILE_PRIMARY;
use slug_loading_v2::file_discovery::MODULE_FILE;
use slug_loading_v2::file_discovery::find_build_file;
use slug_loading_v2::file_discovery::find_workspace_root;
use slug_loading_v2::file_discovery::is_bazel_build_file;
use slug_loading_v2::file_discovery::is_bzl_file;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_loading_v2::package::BuildSettingDefault;
use slug_loading_v2::package::BuildSettingDefinition;
use slug_loading_v2::package::BuildSettingScope;
use slug_loading_v2::package::NativeToolchainTarget;
use slug_loading_v2::package::resolve_rule_definition_label;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-v2-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
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

fn load_package(workspace: &Path, package: &Path) -> slug_loading_v2::LegacyLoadedPackage {
    try_load_package(workspace, package).unwrap()
}

fn try_load_package(
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LegacyLoadedPackage> {
    try_load_package_with_extra_bzl(workspace, package, &[])
}

fn try_load_package_with_extra_bzl(
    workspace: &Path,
    package: &Path,
    extra_bzl: &[PathBuf],
) -> anyhow::Result<slug_loading_v2::LegacyLoadedPackage> {
    try_load_package_with_event_capture(workspace, package, extra_bzl, None, false)
}

fn try_load_package_with_event_capture(
    workspace: &Path,
    package: &Path,
    extra_bzl: &[PathBuf],
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> anyhow::Result<slug_loading_v2::LegacyLoadedPackage> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut paths = vec![
        workspace.join(MODULE_FILE),
        workspace.join(BUILD_FILE_PRIMARY),
        workspace.join(BUILD_FILE_FALLBACK),
        package.join(BUILD_FILE_PRIMARY),
        package.join(BUILD_FILE_FALLBACK),
    ];
    for entry in fs::read_dir(package).unwrap() {
        let path = entry.unwrap().path();
        if is_bzl_file(&path) {
            paths.push(path);
        }
    }
    paths.extend(extra_bzl.iter().cloned());
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
    let text = Arc::new(WorkspaceSnapshot {
        files: Arc::new(files),
    });
    let raw = raw_snapshot_from_text(&text);
    let evaluator = BzlModuleEvaluator::new(workspace).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
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
                    (WorkspaceSnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
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
                    (WorkspaceDirectorySnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
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
            evaluator.evaluate_package(&mut transaction, package).await
        })
}

async fn load_package_with_retained_dice(
    dice: &Arc<Dice>,
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LegacyLoadedPackage> {
    let mut paths = vec![
        workspace.join(MODULE_FILE),
        package.join(BUILD_FILE_PRIMARY),
        package.join("defs.bzl"),
    ];
    paths.extend(
        fs::read_dir(package)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .filter(|path| is_bzl_file(path)),
    );
    paths.sort();
    paths.dedup();
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
    let text = Arc::new(WorkspaceSnapshot {
        files: Arc::new(files),
    });
    let raw = raw_snapshot_from_text(&text);
    let mut updater = dice.updater();
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
    BzlModuleEvaluator::new(workspace)?
        .evaluate_package(&mut updater.commit().await, package)
        .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageEventActivation {
    package: PathBuf,
    kind: ActivationKind,
    batch: Option<EventBatch>,
}

#[derive(Default)]
struct PackageEventTracker {
    activations: Mutex<Vec<PackageEventActivation>>,
}

impl PackageEventTracker {
    fn take(&self) -> Vec<PackageEventActivation> {
        std::mem::take(&mut *self.activations.lock().unwrap())
    }
}

impl ActivationTracker for PackageEventTracker {
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
        if let Some(key) = key.downcast_ref::<PackageLoadKey>() {
            self.activations
                .lock()
                .unwrap()
                .push(PackageEventActivation {
                    package: key.package.clone(),
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
        }
    }
}

fn package_event_texts<'a>(
    activations: &'a [PackageEventActivation],
    package: &Path,
) -> Option<Vec<&'a str>> {
    activations
        .iter()
        .find(|activation| {
            activation.kind == ActivationKind::Evaluated && activation.package == package
        })
        .and_then(|activation| activation.batch.as_ref())
        .map(|batch| {
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
        })
}

#[test]
fn package_event_capture_is_local_and_preserves_empty_and_runtime_prefix_batches() {
    let workspace = scratch("package-events");
    let package = workspace.join("pkg");
    let dependency = package.join("defs.bzl");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        workspace.join(MODULE_FILE),
        "module(name = \"package_events\")\n",
    )
    .unwrap();
    fs::write(
        &dependency,
        "print(\"DEPENDENCY_LOCAL\")\nNAME = \"probe\"\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_LOCAL\")\nfilegroup(name = NAME)\n",
    )
    .unwrap();

    let direct_tracker = Arc::new(PackageEventTracker::default());
    try_load_package_with_event_capture(
        &workspace,
        &package,
        &[],
        Some(direct_tracker.clone()),
        false,
    )
    .unwrap();
    let direct = direct_tracker.take();
    assert!(
        direct
            .iter()
            .filter(|activation| activation.kind == ActivationKind::Evaluated)
            .all(|activation| activation.batch.is_none()),
        "{direct:?}"
    );

    let tracker = Arc::new(PackageEventTracker::default());
    try_load_package_with_event_capture(&workspace, &package, &[], Some(tracker.clone()), true)
        .unwrap();
    let captured = tracker.take();
    assert_eq!(
        package_event_texts(&captured, &package),
        Some(vec!["BUILD_LOCAL"])
    );
    let captured_batch = captured
        .iter()
        .find(|activation| {
            activation.kind == ActivationKind::Evaluated && activation.package == package
        })
        .and_then(|activation| activation.batch.as_ref())
        .unwrap();
    assert!(matches!(
        captured_batch.events(),
        [EvaluationEvent::StarlarkPrint { location, text }]
            if text == "BUILD_LOCAL"
                && location.to_string()
                    == format!("{}:2:6", package.join(BUILD_FILE_PRIMARY).display())
    ));

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"NAME\")\nfilegroup(name = NAME)\n",
    )
    .unwrap();
    try_load_package_with_event_capture(&workspace, &package, &[], Some(tracker.clone()), true)
        .unwrap();
    let empty = tracker.take();
    assert_eq!(package_event_texts(&empty, &package), Some(Vec::new()));

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_RUNTIME_PREFIX\")\nfail(\"build runtime\")\nprint(\"BUILD_RUNTIME_AFTER\")\nfilegroup(name = NAME)\n",
    )
    .unwrap();
    let error =
        try_load_package_with_event_capture(&workspace, &package, &[], Some(tracker.clone()), true)
            .unwrap_err()
            .to_string();
    assert!(error.contains("build runtime"), "{error}");
    let failed = tracker.take();
    assert_eq!(
        package_event_texts(&failed, &package),
        Some(vec!["BUILD_RUNTIME_PREFIX"])
    );
}

#[test]
fn workspace_root_requires_module_bazel() {
    let root = scratch("root");
    let pkg = root.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(root.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    assert_eq!(find_workspace_root(&pkg).unwrap().path(), root.as_path());

    let missing = scratch("missing");
    fs::write(missing.join("WORKSPACE"), "# ignored\n").unwrap();
    let err = find_workspace_root(&missing).unwrap_err();
    assert!(err.contains(MODULE_FILE));
}

#[test]
fn build_file_discovery_is_bazel_only() {
    let pkg = scratch("build-file");
    fs::write(pkg.join(BUILD_FILE_FALLBACK), "# fallback\n").unwrap();
    assert_eq!(
        find_build_file(&pkg).unwrap(),
        pkg.join(BUILD_FILE_FALLBACK)
    );
    fs::write(pkg.join(BUILD_FILE_PRIMARY), "# primary\n").unwrap();
    assert_eq!(find_build_file(&pkg).unwrap(), pkg.join(BUILD_FILE_PRIMARY));

    let other_1 = pkg.join(concat!("BU", "CK"));
    let other_2 = pkg.join(concat!("TAR", "GETS"));
    fs::write(&other_1, "# ignored\n").unwrap();
    fs::write(&other_2, "# ignored\n").unwrap();
    assert!(!is_bazel_build_file(&other_1));
    assert!(!is_bazel_build_file(&other_2));
}

#[test]
fn recognizes_bzl_extension_only() {
    assert!(is_bzl_file(&PathBuf::from("defs.bzl")));
    assert!(!is_bzl_file(&PathBuf::from("defs.star")));
}

#[test]
fn rule_capabilities_use_exported_class_names_and_keep_native_rules_non_executable() {
    let workspace = scratch("rule-capabilities");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

def _transition_impl(settings, attr):
    return {}

def legacy_macro(name):
    plain_rule(name = name)

def legacy_macro_override(name):
    plain_rule(name = name, generator_name = "macro_override")

exec_arbitrary = rule(implementation = _impl, executable = True)
plain_rule = rule(implementation = _impl)
implicit_test_test = rule(implementation = _impl, test = True)
explicit_test_test = rule(implementation = _impl, test = True, executable = False)
output_rule = rule(implementation = _impl, attrs = {"outs": attr.output_list()})
string_flag = rule(implementation = _impl, build_setting = config.string(flag = True))
transition_rule = rule(implementation = _impl, attrs = {"dep": attr.label(cfg = transition(implementation = _transition_impl, inputs = [], outputs = ["//attr:base_string_setting"]))})
"#,
    )
    .unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        r#"
load(":defs.bzl", "exec_arbitrary", "explicit_test_test", "implicit_test_test", "legacy_macro", "legacy_macro_override", "output_rule", "plain_rule", "string_flag", "transition_rule")

exports_files(["BUILD.bazel", "data.txt"])
exec_arbitrary(name = "arbitrary_target", args = ["z", "z", "a"], output_licenses = ["z", "a"], tags = ["z", "a"])
exec_arbitrary(name = "target_test")
plain_rule(name = "plain", visibility = ["//visibility:public", ":group"], transitive_configs = [":cfg"], generator_name = "direct_generator", toolchains = [":toolchain_one", ":toolchain_two"], features = ["z", "a"])
plain_rule(name = "direct_default")
legacy_macro(name = "macro_target")
legacy_macro_override(name = "macro_override_target")
implicit_test_test(name = "ordinary_target", timeout = "long", flaky = True, shard_count = 3, local = True)
explicit_test_test(name = "explicit_test_target")
output_rule(name = "generated_owner", outs = ["generated.txt"])
string_flag(name = "string_flag", build_setting_default = "plain", help = "fixture flag")
transition_rule(name = "transition_rule")
filegroup(name = "files", srcs = [":data.txt"])
alias(name = "alias_exec", actual = ":arbitrary_target")
config_setting(name = "setting", values = {"cpu": "k8"})
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let schema_len = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .and_then(|target| match &target.kind {
                PackageTargetKind::StarlarkRule(rule) => Some(rule.schema().len()),
                _ => None,
            })
            .unwrap()
    };
    let capability = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .and_then(|target| target.rule_capability())
            .cloned()
    };
    let expected = |rule_class: &str, executable| {
        Some(RuleCapability {
            rule_class: rule_class.into(),
            executable,
            test_kind: rule_class
                .ends_with("_test")
                .then_some(slug_loading_v2::TestRuleKind::Test),
        })
    };

    assert_eq!(
        capability("arbitrary_target"),
        expected("exec_arbitrary", true)
    );
    assert_eq!(capability("target_test"), expected("exec_arbitrary", true));
    assert_eq!(capability("plain"), expected("plain_rule", false));
    assert_eq!(
        capability("ordinary_target"),
        expected("implicit_test_test", true)
    );
    assert_eq!(
        capability("explicit_test_target"),
        expected("explicit_test_test", true)
    );
    assert_eq!(
        capability("generated_owner"),
        expected("output_rule", false)
    );
    assert_eq!(capability("files"), expected("filegroup", false));
    assert_eq!(capability("alias_exec"), expected("alias", false));
    assert_eq!(capability("setting"), expected("config_setting", false));
    assert_eq!(capability("BUILD.bazel"), None);
    assert_eq!(capability("data.txt"), None);
    assert_eq!(capability("generated.txt"), None);
    assert_eq!(schema_len("plain"), 22);
    assert_eq!(schema_len("arbitrary_target"), 25);
    assert_eq!(schema_len("ordinary_target"), 39);
    assert_eq!(schema_len("explicit_test_target"), 39);
    assert_eq!(schema_len("string_flag"), 24);
    assert_eq!(schema_len("transition_rule"), 24);
    let PackageTargetKind::StarlarkRule(string_flag) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "string_flag")
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    assert_eq!(
        string_flag.root_string_build_setting_default(),
        Some("plain")
    );
    assert!(matches!(
        string_flag
            .values()
            .iter()
            .find(|value| value.declaration_name == "help")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::String(value) if value == "fixture flag"
    ));
    let values = |target: &str| {
        let PackageTargetKind::StarlarkRule(rule) = &loaded
            .targets
            .iter()
            .find(|candidate| candidate.name == target)
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        rule.values()
    };
    let value = |target: &str, name: &str| {
        values(target)
            .iter()
            .find(|value| value.declaration_name == name)
            .unwrap()
    };
    assert!(matches!(
        value("plain", "transitive_configs").value.as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.len() == 1
    ));
    assert!(matches!(
        value("plain", "visibility").value.as_ref(),
        CoercedAttributeValue::LabelList(labels)
            if labels.as_ref() == [CanonicalLabel::parse("@@//visibility:public").unwrap()]
    ));
    assert!(matches!(
        &loaded
            .targets
            .iter()
            .find(|target| target.name == "plain")
            .unwrap()
            .visibility,
        VisibilitySource::Declared(slug_loading_v2::RuleVisibility::Public)
    ));
    assert!(matches!(
        value("plain", "generator_name").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "direct_generator"
    ));
    assert!(matches!(
        value("direct_default", "generator_name").value.as_ref(),
        CoercedAttributeValue::String(value) if value.is_empty()
    ));
    assert!(matches!(
        value("macro_target", "generator_name").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "macro_target"
    ));
    assert!(matches!(
        value("macro_target", "generator_function").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "legacy_macro"
    ));
    assert!(matches!(
        value("macro_override_target", "generator_name").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "macro_override"
    ));
    assert!(matches!(
        value("plain", "toolchains").value.as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.len() == 2
    ));
    assert!(matches!(
        value("arbitrary_target", "args").value.as_ref(),
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["z", "z", "a"]
    ));
    assert!(matches!(
        value("arbitrary_target", "tags").value.as_ref(),
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z"]
    ));
    assert!(matches!(
        value("plain", "features").value.as_ref(),
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z"]
    ));
    assert!(matches!(
        value("arbitrary_target", "output_licenses").value.as_ref(),
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["z", "a"]
    ));
    assert!(matches!(
        value("ordinary_target", "timeout").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "long"
    ));
    assert!(matches!(
        value("ordinary_target", "flaky").value.as_ref(),
        CoercedAttributeValue::Boolean(true)
    ));
    assert!(matches!(
        value("ordinary_target", "shard_count").value.as_ref(),
        CoercedAttributeValue::Integer(3)
    ));
    assert!(matches!(
        value("explicit_test_target", "size").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "medium"
    ));
    assert!(matches!(
        value("explicit_test_target", "timeout").value.as_ref(),
        CoercedAttributeValue::String(value) if value == "moderate"
    ));
    assert!(matches!(
        value("explicit_test_target", "flaky").value.as_ref(),
        CoercedAttributeValue::Boolean(false)
    ));
    assert!(matches!(
        value("explicit_test_target", "shard_count").value.as_ref(),
        CoercedAttributeValue::Integer(-1)
    ));
    assert!(matches!(
        value("explicit_test_target", "local").value.as_ref(),
        CoercedAttributeValue::Boolean(false)
    ));
    assert!(matches!(
        value("ordinary_target", ":run_under_exec_config")
            .value
            .as_ref(),
        CoercedAttributeValue::None
    ));
    assert!(matches!(
        value("ordinary_target", "$test_wrapper").value.as_ref(),
        CoercedAttributeValue::Label(label) if label.to_string() == "@@bazel_tools//tools/test:test_wrapper"
    ));
    for (name, expected) in [
        ("$test_wrapper", "@@bazel_tools//tools/test:test_wrapper"),
        ("$xml_writer", "@@bazel_tools//tools/test:xml_writer"),
        ("$test_runtime", "@@bazel_tools//tools/test:runtime"),
        ("$test_setup_script", "@@bazel_tools//tools/test:test_setup"),
        (
            "$xml_generator_script",
            "@@bazel_tools//tools/test:test_xml_generator",
        ),
        (
            "$collect_coverage_script",
            "@@bazel_tools//tools/test:collect_coverage",
        ),
        (
            ":coverage_support",
            "@@bazel_tools//tools/test:coverage_support",
        ),
        (
            ":coverage_report_generator",
            "@@bazel_tools//tools/test:coverage_report_generator",
        ),
    ] {
        let mut labels = Vec::new();
        value("explicit_test_target", name)
            .value
            .labels(&mut labels);
        assert_eq!(labels, [CanonicalLabel::parse(expected).unwrap()], "{name}");
    }
    for name in [":run_under_exec_config", ":run_under_target_config"] {
        assert!(matches!(
            value("explicit_test_target", name).value.as_ref(),
            CoercedAttributeValue::None
        ));
    }
    let PackageTargetKind::StarlarkRule(normal) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "direct_default")
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    let nonordinary_label_builtins = normal
        .schema()
        .iter()
        .filter(|schema| {
            schema.is_builtin()
                && matches!(
                    schema.kind(),
                    AttributeKind::Label
                        | AttributeKind::LabelList
                        | AttributeKind::StringKeyedLabelDict
                        | AttributeKind::LabelKeyedStringDict
                        | AttributeKind::LabelListDict
                )
                && !schema.ordinary_dependency()
        })
        .map(|schema| schema.declaration_name())
        .collect::<Vec<_>>();
    assert_eq!(
        nonordinary_label_builtins,
        ["visibility", "transitive_configs"]
    );
    assert!(matches!(
        value("transition_rule", "$allowlist_function_transition")
            .value
            .as_ref(),
        CoercedAttributeValue::Label(label)
            if label.to_string()
                == "@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist"
    ));
    let PackageTargetKind::StarlarkRule(transition_rule) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "transition_rule")
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    assert!(transition_rule.dependencies().contains(
        &CanonicalLabel::parse(
            "@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist",
        )
        .unwrap()
    ));
    assert_eq!(
        transition_rule
            .schema()
            .iter()
            .find(|schema| schema.declaration_name() == "dep")
            .unwrap()
            .transition()
            .unwrap()
            .outputs()[0]
            .declared(),
        "//attr:base_string_setting"
    );
}

#[test]
fn rule_export_rejects_test_suffix_mismatches_with_bazel_shape() {
    let cases = [
        ("not_test_suffix", "test = True"),
        ("suffix_test", "test = False"),
    ];
    for (class_name, test) in cases {
        let workspace = scratch(class_name);
        let package = workspace.join("pkg");
        fs::create_dir_all(&package).unwrap();
        fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
        fs::write(
            package.join("defs.bzl"),
            format!(
                "def _impl(ctx):\n    return [DefaultInfo()]\n{class_name} = rule(implementation = _impl, {test})\n"
            ),
        )
        .unwrap();
        fs::write(
            package.join("BUILD.bazel"),
            format!("load(\":defs.bzl\", \"{class_name}\")\n"),
        )
        .unwrap();

        let error = try_load_package(&workspace, &package).unwrap_err();
        assert!(error.to_string().contains(&format!(
            "Invalid rule class name '{class_name}', test rule class names must end with '_test' and other rule classes must not"
        )));
    }
}

#[test]
fn config_setting_retains_all_matching_fields_and_provenance() {
    let workspace = scratch("config-setting");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        concat!(
            "config_setting(\n",
            "    name = \"linux\",\n",
            "    values = {\"cpu\": \"k8\", \"compilation_mode\": \"opt\"},\n",
            "    define_values = {\"mode\": \"fast\"},\n",
            "    flag_values = {\":flag\": \"enabled\"},\n",
            "    constraint_values = [\":constraint_b\", \":constraint\"],\n",
            ")\n",
        ),
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::ConfigSetting { declaration } = &loaded.targets[0].kind else {
        panic!("expected config_setting")
    };
    assert_eq!(loaded.targets[0].name, "linux");
    assert_eq!(loaded.targets[0].visibility, VisibilitySource::AlwaysPublic);
    assert_eq!(
        declaration.values().value().as_ref(),
        [
            ("compilation_mode".into(), "opt".into()),
            ("cpu".into(), "k8".into())
        ]
    );
    assert_eq!(
        declaration.define_values().value().as_ref(),
        [("mode".into(), "fast".into())]
    );
    assert_eq!(
        declaration.flag_values().value().as_ref(),
        [(
            CanonicalLabel::parse("@@//pkg:flag").unwrap(),
            "enabled".into()
        )]
    );
    assert_eq!(
        declaration.constraint_values().value().as_ref(),
        [
            CanonicalLabel::parse("@@//pkg:constraint_b").unwrap(),
            CanonicalLabel::parse("@@//pkg:constraint").unwrap(),
        ]
    );
    for attribute in [
        declaration.values().provenance(),
        declaration.define_values().provenance(),
        declaration.flag_values().provenance(),
        declaration.constraint_values().provenance(),
    ] {
        assert_eq!(attribute, AttributeProvenance::Explicit);
    }
    assert_eq!(declaration.semantic_references().len(), 3);

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "config_setting(name = \"empty\")\n",
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::ConfigSetting { declaration } = &loaded.targets[0].kind else {
        panic!("expected config_setting")
    };
    assert!(declaration.semantic_references().is_empty());
    for provenance in [
        declaration.values().provenance(),
        declaration.define_values().provenance(),
        declaration.flag_values().provenance(),
        declaration.constraint_values().provenance(),
    ] {
        assert_eq!(provenance, AttributeProvenance::Default);
    }

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "config_setting(name = \"collision\", flag_values = {\":flag\": \"one\", \"//pkg:flag\": \"two\"})\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("duplicate canonical label dictionary key '@@//pkg:flag'"),
        "{error}"
    );
}

#[test]
fn build_setting_invocations_retain_complete_typed_declarations() {
    let workspace = scratch("typed-build-setting-declarations");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    fail("build setting implementation ran during loading")

int_flag = rule(implementation = _impl, attrs = {"scope": attr.string(default = "universal")}, build_setting = config.int(flag = True))
wrong_scope = rule(implementation = _impl, attrs = {"scope": attr.int()}, build_setting = config.int())
bool_setting = rule(implementation = _impl, build_setting = config.bool())
string_flag = rule(implementation = _impl, build_setting = config.string(flag = True))
multi_string = rule(implementation = _impl, build_setting = config.string(flag = True, allow_multiple = True))
string_list = rule(implementation = _impl, build_setting = config.string_list())
string_set = rule(implementation = _impl, build_setting = config.string_set(flag = True))
repeatable_set = rule(implementation = _impl, build_setting = config.string_set(flag = True, repeatable = True))
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "bool_setting", "int_flag", "multi_string", "repeatable_set", "string_flag", "string_list", "string_set", "wrong_scope")
config_setting(name = "condition", values = {})
int_flag(name = "integer", build_setting_default = 7, scope = "TaRgEt")
int_flag(name = "integer_universal", build_setting_default = 8, scope = "UnIvErSaL")
int_flag(name = "integer_project", build_setting_default = 9, scope = "PrOjEcT")
int_flag(name = "integer_default", build_setting_default = 10)
int_flag(name = "integer_selected", build_setting_default = 11, scope = select({":condition": "target", "//conditions:default": "universal"}))
wrong_scope(name = "integer_wrong_scope", build_setting_default = 12, scope = 1)
bool_setting(name = "boolean", build_setting_default = False)
string_flag(name = "string", build_setting_default = "plain")
multi_string(name = "multi", build_setting_default = "one")
string_list(name = "list", build_setting_default = ["b", "a"])
string_set(name = "set", build_setting_default = set(["b", "a"]))
repeatable_set(name = "repeatable", build_setting_default = set(["z"]))
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let declaration = |name: &str| {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
            panic!("expected Starlark build setting")
        };
        rule.build_setting_declaration().unwrap().unwrap()
    };
    let integer = declaration("integer");
    assert_eq!(
        integer.definition(),
        BuildSettingDefinition::Integer { flag: true }
    );
    assert_eq!(integer.default(), &BuildSettingDefault::Integer(7));
    assert_eq!(integer.scope(), BuildSettingScope::Target);
    assert_eq!(
        declaration("integer_universal").scope(),
        BuildSettingScope::Universal
    );
    assert_eq!(
        declaration("integer_project").scope(),
        BuildSettingScope::Project
    );
    assert_eq!(
        declaration("integer_default").scope(),
        BuildSettingScope::Default
    );
    assert_eq!(
        declaration("boolean").default(),
        &BuildSettingDefault::Boolean(false)
    );
    assert_eq!(
        declaration("string").default(),
        &BuildSettingDefault::String("plain".into())
    );
    assert_eq!(
        declaration("multi").default(),
        &BuildSettingDefault::String("one".into())
    );
    assert_eq!(
        declaration("list").default(),
        &BuildSettingDefault::StringList(Arc::from(["b".into(), "a".into()]))
    );
    let set = declaration("set");
    assert_eq!(
        set.definition(),
        BuildSettingDefinition::StringSet {
            flag: true,
            repeatable: false,
        }
    );
    assert_eq!(
        set.default(),
        &BuildSettingDefault::StringSet(Arc::from(["a".into(), "b".into()]))
    );
    assert_eq!(
        declaration("repeatable").definition(),
        BuildSettingDefinition::StringSet {
            flag: true,
            repeatable: true,
        }
    );
    for name in ["integer_selected", "integer_wrong_scope"] {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
            panic!("expected Starlark build setting")
        };
        assert!(rule.build_setting_declaration().is_err(), "{name}");
    }
}

#[test]
fn build_setting_invocations_reject_invalid_defaults_without_running_implementations() {
    let workspace = scratch("invalid-build-setting-defaults");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    fail("build setting implementation ran during loading")

int_flag = rule(implementation = _impl, build_setting = config.int(flag = True))
bool_flag = rule(implementation = _impl, build_setting = config.bool(flag = True))
string_flag = rule(implementation = _impl, build_setting = config.string(flag = True))
string_list = rule(implementation = _impl, build_setting = config.string_list(flag = True))
string_set = rule(implementation = _impl, build_setting = config.string_set(flag = True))
"#,
    )
    .unwrap();
    let load = "load(\":defs.bzl\", \"bool_flag\", \"int_flag\", \"string_flag\", \"string_list\", \"string_set\")\n";
    for invocation in [
        "int_flag(name = \"bad\", build_setting_default = 2147483648)\n",
        "bool_flag(name = \"bad\", build_setting_default = \"false\")\n",
        "string_flag(name = \"bad\", build_setting_default = 1)\n",
        "string_list(name = \"bad\", build_setting_default = set([\"one\"]))\n",
        "string_set(name = \"bad\", build_setting_default = [\"one\"])\n",
        "string_set(name = \"bad\", build_setting_default = set([\"one\", 1]))\n",
        "int_flag(name = \"bad\")\n",
    ] {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("{load}{invocation}"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("implementation ran during loading"),
            "{error}"
        );
    }
}

#[test]
fn native_toolchain_targets_retain_fixture_order_labels_and_capabilities() {
    let workspace = scratch("native-toolchain-targets");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
constraint_setting(name = "selection")
constraint_value(name = "first", constraint_setting = ":selection")
constraint_value(name = "second", constraint_setting = ":selection")
platform(name = "exec", constraint_values = [":second", ":first"])
toolchain_type(name = "demo_type")
toolchain(
    name = "demo_toolchain",
    exec_compatible_with = [":first", ":second"],
    toolchain = "@@external+//tools:implementation",
    toolchain_type = ":demo_type",
)
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        [
            "selection",
            "first",
            "second",
            "exec",
            "demo_type",
            "demo_toolchain",
        ]
    );
    assert!(matches!(
        loaded.targets[0].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintSetting {
            default_constraint_value: None,
        })
    ));
    assert!(matches!(
        &loaded.targets[1].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue {
            constraint_setting,
        }) if constraint_setting == &CanonicalLabel::parse("@@//pkg:selection").unwrap()
    ));
    assert!(matches!(
        &loaded.targets[2].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue {
            constraint_setting,
        }) if constraint_setting == &CanonicalLabel::parse("@@//pkg:selection").unwrap()
    ));
    assert!(matches!(
        &loaded.targets[3].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
            constraint_values,
        }) if constraint_values.as_ref() == [
            CanonicalLabel::parse("@@//pkg:second").unwrap(),
            CanonicalLabel::parse("@@//pkg:first").unwrap(),
        ]
    ));
    assert!(matches!(
        loaded.targets[4].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)
    ));
    assert!(matches!(
        &loaded.targets[5].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            toolchain_type,
            implementation,
            exec_compatible_with,
            target_compatible_with,
            use_target_platform_constraints,
            target_settings,
        }) if toolchain_type == &CanonicalLabel::parse("@@//pkg:demo_type").unwrap()
            && implementation == &CanonicalLabel::parse("@@external+//tools:implementation").unwrap()
            && exec_compatible_with.value().as_ref() == [
                CanonicalLabel::parse("@@//pkg:first").unwrap(),
                CanonicalLabel::parse("@@//pkg:second").unwrap(),
            ]
            && exec_compatible_with.provenance() == AttributeProvenance::Explicit
            && target_compatible_with.value().is_empty()
            && target_compatible_with.provenance() == AttributeProvenance::Default
            && !use_target_platform_constraints.value()
            && use_target_platform_constraints.provenance() == AttributeProvenance::Default
            && matches!(target_settings.value(), CoercedAttributeValue::LabelList(values) if values.is_empty())
            && target_settings.provenance() == AttributeProvenance::Default
    ));
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.rule_capability().unwrap().rule_class.as_str())
            .collect::<Vec<_>>(),
        [
            "constraint_setting",
            "constraint_value",
            "constraint_value",
            "platform",
            "toolchain_type",
            "toolchain",
        ]
    );
    assert!(loaded.targets.iter().all(|target| {
        let capability = target.rule_capability().unwrap();
        target.visibility == VisibilitySource::PackageDefault
            && !capability.executable
            && capability.test_kind.is_none()
    }));
}

#[test]
fn native_direct_label_parameters_accept_loaded_labels_and_strings() {
    let workspace = scratch("native-direct-label-parameters");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
TYPED = Label(":typed")
def native_emit():
    native.constraint_value(name = "native_value", constraint_setting = TYPED)
    native.platform(name = "native_platform", constraint_values = [TYPED, ":raw"])
    native.toolchain(name = "native_toolchain", toolchain = TYPED, toolchain_type = ":raw", exec_compatible_with = [TYPED])
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "TYPED", "native_emit")
native.package(default_testonly = True, default_package_metadata = [TYPED, ":metadata"])
package_group(name = "group", includes = [TYPED, ":included"])
alias(name = "alias", actual = TYPED)
test_suite(name = "suite", tests = [TYPED, ":test"])
config_setting(name = "config", flag_values = {TYPED: "typed", ":flag": "raw"}, constraint_values = [TYPED, ":constraint"])
constraint_value(name = "value", constraint_setting = TYPED)
platform(name = "platform", constraint_values = [TYPED, ":raw"])
toolchain(name = "tool", toolchain = TYPED, toolchain_type = ":raw", exec_compatible_with = [TYPED], target_compatible_with = [":target"])
native_emit()
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let target = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
    };
    assert!(matches!(
        &target("alias").kind,
        PackageTargetKind::Alias { actual: AliasActual::Visible(actual) }
            if actual == &CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//pkg:typed").unwrap()
            )
    ));
    let PackageTargetKind::PackageGroup { includes, .. } = &target("group").kind else {
        panic!("expected package group")
    };
    assert_eq!(
        includes.as_ref(),
        [
            CanonicalLabel::parse("@@//pkg:typed").unwrap(),
            CanonicalLabel::parse("@@//pkg:included").unwrap(),
        ]
    );
    assert!(matches!(
        &target("platform").kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform { constraint_values })
            if constraint_values.as_ref() == [
                CanonicalLabel::parse("@@//pkg:typed").unwrap(),
                CanonicalLabel::parse("@@//pkg:raw").unwrap(),
            ]
    ));
    for name in ["value", "native_value"] {
        assert!(matches!(
            &target(name).kind,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue { constraint_setting })
                if constraint_setting == &CanonicalLabel::parse("@@//pkg:typed").unwrap()
        ));
    }
    for name in ["native_platform", "platform"] {
        assert!(matches!(
            &target(name).kind,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform { constraint_values })
                if constraint_values.as_ref() == [
                    CanonicalLabel::parse("@@//pkg:typed").unwrap(),
                    CanonicalLabel::parse("@@//pkg:raw").unwrap(),
                ]
        ));
    }
    let PackageTargetKind::ConfigSetting { declaration } = &target("config").kind else {
        panic!("expected config setting")
    };
    assert_eq!(
        declaration.flag_values().value().as_ref(),
        [
            (CanonicalLabel::parse("@@//pkg:flag").unwrap(), "raw".into()),
            (
                CanonicalLabel::parse("@@//pkg:typed").unwrap(),
                "typed".into()
            ),
        ]
    );
    assert_eq!(
        declaration.constraint_values().value().as_ref(),
        [
            CanonicalLabel::parse("@@//pkg:typed").unwrap(),
            CanonicalLabel::parse("@@//pkg:constraint").unwrap(),
        ]
    );

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"TYPED\")\npackage(default_package_metadata = [TYPED, \":typed\"])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("duplicated")
    );
}

#[test]
fn native_direct_label_parameters_preserve_defining_label_identity() {
    let workspace = scratch("native-direct-label-defining-owner");
    let owner = workspace.join("owner");
    let package = workspace.join("pkg");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(owner.join(BUILD_FILE_PRIMARY), "").unwrap();
    fs::write(
        owner.join("defs.bzl"),
        r#"
TYPED = Label(":typed")
def emit():
    native.toolchain(name = "tool", toolchain = TYPED, toolchain_type = ":raw")
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\"//owner:defs.bzl\", \"TYPED\", \"emit\")\nalias(name = \"alias\", actual = TYPED)\nemit()\n",
    )
    .unwrap();

    let loaded =
        try_load_package_with_extra_bzl(&workspace, &package, &[owner.join("defs.bzl")]).unwrap();
    assert!(matches!(
        &loaded.targets[0].kind,
        PackageTargetKind::Alias { actual: AliasActual::Visible(actual) }
            if actual == &CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//owner:typed").unwrap()
            )
    ));
    assert!(matches!(
        &loaded.targets[1].kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            implementation,
            toolchain_type,
            ..
        }) if implementation == &CanonicalLabel::parse("@@//owner:typed").unwrap()
            && toolchain_type == &CanonicalLabel::parse("@@//pkg:raw").unwrap()
    ));
}

#[test]
fn imported_bzl_defs_resolve_label_from_the_lexical_source() {
    let workspace = scratch("imported-def-label-source");
    let leaf = workspace.join("leaf");
    let owner = workspace.join("wrapper");
    let consumer = workspace.join("consumer");
    fs::create_dir_all(&leaf).unwrap();
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(leaf.join(BUILD_FILE_PRIMARY), "").unwrap();
    fs::write(owner.join(BUILD_FILE_PRIMARY), "").unwrap();
    fs::write(
        leaf.join("defs.bzl"),
        "def labels():\n    relative = Label(':runtime')\n    if Label(relative) != relative: fail('Label identity changed')\n    return [relative, Label('@@//canonical:runtime')]\n",
    )
    .unwrap();
    fs::write(
        owner.join("bridge.bzl"),
        "load('//leaf:defs.bzl', 'labels')\ndef tiny(): return labels()\nREEXPORTED = tiny\n",
    )
    .unwrap();
    fs::write(
        owner.join("default_java_toolchain.bzl"),
        "load(':bridge.bzl', 'REEXPORTED')\ndef java_runtime_files(name):\n    native.filegroup(name = name + '_pre')\n    native.filegroup(name = name, srcs = REEXPORTED())\n",
    )
    .unwrap();
    fs::write(
        consumer.join(BUILD_FILE_FALLBACK),
        "load('//wrapper:default_java_toolchain.bzl', 'java_runtime_files')\njava_runtime_files(name = 'runtime')\n",
    )
    .unwrap();
    let loaded = try_load_package_with_extra_bzl(
        &workspace,
        &consumer,
        &[
            leaf.join("defs.bzl"),
            owner.join("bridge.bzl"),
            owner.join("default_java_toolchain.bzl"),
        ],
    )
    .unwrap();
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["runtime_pre", "runtime"]
    );
    let PackageTargetKind::Filegroup { srcs, .. } = &loaded.targets[1].kind else {
        panic!("runtime should be a filegroup")
    };
    assert_eq!(
        srcs.as_ref(),
        [
            CanonicalLabel::parse("@@//leaf:runtime").unwrap(),
            CanonicalLabel::parse("@@//canonical:runtime").unwrap(),
        ]
    );
}

#[test]
fn native_direct_label_parameters_fail_before_publication() {
    let workspace = scratch("native-direct-label-failures");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(package.join("defs.bzl"), "TYPED = Label(\":typed\")\n").unwrap();
    let cases = [
        "alias(name = 'bad', actual = 1)",
        "test_suite(name = 'bad', tests = [1])",
        "test_suite(name = 'bad', tests = [None])",
        "config_setting(name = 'bad', flag_values = {1: 'value'})",
        "config_setting(name = 'bad', flag_values = {':key': 1})",
        "config_setting(name = 'bad', flag_values = {None: 'value'})",
        "config_setting(name = 'bad', flag_values = {':key': None})",
        "platform(name = 'bad', constraint_values = None)",
        "package(default_package_metadata = [':same', '//pkg:same'])",
        "native.package(default_package_metadata = [':same', '//pkg:same'])",
        "load(':defs.bzl', 'TYPED')\nconfig_setting(name = 'bad', flag_values = {TYPED: 'one', ':typed': 'two'})",
    ];
    for (index, source) in cases.into_iter().enumerate() {
        fs::write(package.join(BUILD_FILE_PRIMARY), source).unwrap();
        assert!(
            try_load_package(&workspace, &package).is_err(),
            "failure {index} published a package: {source}"
        );
    }
}

#[test]
fn applicable_license_aliases_publish_only_canonical_metadata() {
    let workspace = scratch("applicable-license-aliases");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
TYPED = Label(":typed")
def _impl(ctx):
    return [DefaultInfo()]
probe = rule(implementation = _impl)
"#,
    )
    .unwrap();
    let source = r#"
load(":defs.bzl", "TYPED", "probe")
package(default_applicable_licenses = [TYPED, ":default"])
filegroup(name = "native_default")
filegroup(name = "native_alias", applicable_licenses = [":native_alias_value"])
filegroup(name = "native_empty", applicable_licenses = [])
filegroup(name = "native_none", applicable_licenses = None)
filegroup(name = "native_both", package_metadata = [":first"], applicable_licenses = [":second"])
filegroup(name = "native_none_last", applicable_licenses = [":first"], package_metadata = None)
probe(name = "starlark_alias", applicable_licenses = [":starlark_alias_value"])
probe(name = "starlark_both", applicable_licenses = [":first"], package_metadata = [":second"])
probe(name = "starlark_none", applicable_licenses = None)
"#;
    fs::write(package.join(BUILD_FILE_PRIMARY), source).unwrap();

    let loaded = load_package(&workspace, &package);
    let label = |name: &str| CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap();
    let native_metadata = |name: &str| {
        let attributes = loaded.native_attributes(name).unwrap();
        assert!(attributes.get("applicable_licenses").is_none());
        let value = &attributes.get("package_metadata").unwrap().1;
        let CoercedAttributeValue::LabelList(labels) = &value.value else {
            panic!("package_metadata must remain a label list")
        };
        (value.provenance, labels.to_vec())
    };
    assert_eq!(
        native_metadata("native_default"),
        (
            AttributeProvenance::Default,
            vec![label("typed"), label("default")]
        )
    );
    assert_eq!(
        native_metadata("native_alias"),
        (
            AttributeProvenance::Explicit,
            vec![label("native_alias_value")]
        )
    );
    assert_eq!(
        native_metadata("native_empty"),
        (AttributeProvenance::Explicit, Vec::new())
    );
    assert_eq!(
        native_metadata("native_none"),
        (
            AttributeProvenance::Default,
            vec![label("typed"), label("default")]
        )
    );
    assert_eq!(
        native_metadata("native_both"),
        (AttributeProvenance::Explicit, vec![label("second")])
    );
    assert_eq!(
        native_metadata("native_none_last"),
        (AttributeProvenance::Explicit, vec![label("first")])
    );

    let starlark_metadata = |name: &str| {
        let PackageTargetKind::StarlarkRule(rule) = &loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
            .kind
        else {
            panic!("expected Starlark rule")
        };
        assert!(
            rule.schema()
                .iter()
                .all(|schema| schema.declaration_name() != "applicable_licenses")
        );
        let value = rule
            .values()
            .iter()
            .find(|value| value.declaration_name == "package_metadata")
            .unwrap();
        let CoercedAttributeValue::LabelList(labels) = value.value.as_ref() else {
            panic!("package_metadata must remain a label list")
        };
        (value.provenance, labels.to_vec())
    };
    assert_eq!(
        starlark_metadata("starlark_alias"),
        (
            AttributeProvenance::Explicit,
            vec![label("starlark_alias_value")]
        )
    );
    assert_eq!(
        starlark_metadata("starlark_both"),
        (AttributeProvenance::Explicit, vec![label("second")])
    );
    assert_eq!(
        starlark_metadata("starlark_none"),
        (
            AttributeProvenance::Default,
            vec![label("typed"), label("default")]
        )
    );

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        source.replace(
            "package(default_applicable_licenses",
            "package(default_package_metadata",
        ),
    )
    .unwrap();
    assert_eq!(loaded, load_package(&workspace, &package));

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        source.replace(
            "package(default_applicable_licenses",
            "native.package(default_applicable_licenses",
        ),
    )
    .unwrap();
    assert_eq!(loaded, load_package(&workspace, &package));

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "package(default_applicable_licenses = [])\nfilegroup(name = 'empty_default')\n",
    )
    .unwrap();
    let empty = load_package(&workspace, &package);
    let value = &empty
        .native_attributes("empty_default")
        .unwrap()
        .get("package_metadata")
        .unwrap()
        .1;
    assert_eq!(value.provenance, AttributeProvenance::Default);
    assert!(matches!(
        &value.value,
        CoercedAttributeValue::LabelList(labels) if labels.is_empty()
    ));
}

#[test]
fn applicable_license_aliases_preserve_bazel_rejections() {
    let workspace = scratch("applicable-license-alias-errors");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
TYPED = Label(":typed")
def _macro(name, visibility, package_metadata):
    pass
M = macro(implementation = _macro, attrs = {"package_metadata": attr.label_list()})
"#,
    )
    .unwrap();
    let cases = [
        (
            "package(default_package_metadata = [], default_applicable_licenses = [])",
            "Can not set both default_package_metadata and default_applicable_licenses. Move all declarations to default_package_metadata.",
        ),
        (
            "native.package(default_applicable_licenses = [], default_package_metadata = [])",
            "Can not set both default_package_metadata and default_applicable_licenses. Move all declarations to default_package_metadata.",
        ),
        (
            "load(':defs.bzl', 'TYPED')\npackage(default_applicable_licenses = [TYPED, ':typed'])",
            "default_applicable_licenses",
        ),
        (
            "platform(name = 'bad', applicable_licenses = [])",
            "native attribute `package_metadata` is not declared",
        ),
        (
            "constraint_setting(name = 'bad', applicable_licenses = [])",
            "native attribute `package_metadata` is not declared",
        ),
        (
            "constraint_value(name = 'bad', constraint_setting = ':setting', applicable_licenses = [])",
            "native attribute `package_metadata` is not declared",
        ),
        (
            "load(':defs.bzl', 'M')\nM(name = 'bad', applicable_licenses = [])",
            "no such attribute 'applicable_licenses' in 'M' macro",
        ),
    ];
    for (index, (source, expected)) in cases.into_iter().enumerate() {
        fs::write(package.join(BUILD_FILE_PRIMARY), source).unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "failure {index}: {error}");
    }
}

#[test]
fn native_label_facades_retain_the_same_direct_values() {
    let workspace = scratch("native-direct-label-facades");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "TYPED = Label(\":typed\")\nSPECIALS = [Label(\":all\"), Label(\":all-targets\"), Label(\":*\"), Label(\":...\"), Label(\":sub/...\")]\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "TYPED", "SPECIALS")
native.package(default_package_metadata = [TYPED, ":metadata"])
native.package_group(name = "group", includes = [TYPED, ":included"])
native.alias(name = "alias", actual = TYPED)
native.test_suite(name = "suite", tests = [TYPED, ":test"])
native.config_setting(name = "config", flag_values = {TYPED: "typed"}, constraint_values = [TYPED])
platform(name = "raw_names", constraint_values = [":all", ":all-targets", ":*", ":...", ":sub/..."])
platform(name = "typed_names", constraint_values = SPECIALS)
toolchain(name = "direct_tool", toolchain = TYPED, toolchain_type = ":raw", exec_compatible_with = [TYPED, ":exec"], target_compatible_with = [":target"])
native.toolchain(name = "native_tool", toolchain = TYPED, toolchain_type = ":raw", exec_compatible_with = [TYPED, ":exec"], target_compatible_with = [":target"])
filegroup(name = "metadata")
"#,
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let target = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
    };
    assert!(
        matches!(&target("alias").kind, PackageTargetKind::Alias { actual: AliasActual::Visible(actual) }
        if actual == &CoercedAttributeValue::Label(
            CanonicalLabel::parse("@@//pkg:typed").unwrap()
        ))
    );
    assert!(
        matches!(&target("suite").kind, PackageTargetKind::TestSuite { membership: TestSuiteMembership::Explicit { tests }, .. }
        if tests.as_ref() == [CanonicalLabel::parse("@@//pkg:test").unwrap(), CanonicalLabel::parse("@@//pkg:typed").unwrap()])
    );
    assert!(
        matches!(&target("group").kind, PackageTargetKind::PackageGroup { includes, .. }
        if includes.as_ref() == [CanonicalLabel::parse("@@//pkg:typed").unwrap(), CanonicalLabel::parse("@@//pkg:included").unwrap()])
    );
    let PackageTargetKind::ConfigSetting { declaration } = &target("config").kind else {
        panic!("config")
    };
    assert_eq!(
        declaration.flag_values().value().as_ref(),
        [(
            CanonicalLabel::parse("@@//pkg:typed").unwrap(),
            "typed".into()
        )]
    );
    assert_eq!(
        declaration.constraint_values().value().as_ref(),
        [CanonicalLabel::parse("@@//pkg:typed").unwrap()]
    );
    assert!(matches!(
        &loaded.native_attributes("metadata").unwrap().get("package_metadata").unwrap().1.value,
        CoercedAttributeValue::LabelList(labels) if labels.as_ref() == [
            CanonicalLabel::parse("@@//pkg:typed").unwrap(),
            CanonicalLabel::parse("@@//pkg:metadata").unwrap(),
        ]
    ));
    let special_labels = ["all", "all-targets", "*", "...", "sub/..."]
        .map(|name| CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap());
    for name in ["raw_names", "typed_names"] {
        assert!(matches!(
            &target(name).kind,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform { constraint_values })
                if constraint_values.as_ref() == special_labels
        ));
    }
    for name in ["direct_tool", "native_tool"] {
        assert!(matches!(
            &target(name).kind,
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain { toolchain_type, implementation, exec_compatible_with, target_compatible_with, .. })
                if toolchain_type == &CanonicalLabel::parse("@@//pkg:raw").unwrap()
                    && implementation == &CanonicalLabel::parse("@@//pkg:typed").unwrap()
                    && exec_compatible_with.value().len() == 2
                    && target_compatible_with.value().as_ref() == [CanonicalLabel::parse("@@//pkg:target").unwrap()]
        ));
    }
}

#[test]
fn constraint_setting_retains_default_value_presence() {
    let workspace = scratch("constraint-setting-default");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        workspace.join(BUILD_FILE_PRIMARY),
        "constraint_setting(name = \"setting\", default_constraint_value = \":value\")\n\
         constraint_value(name = \"value\", constraint_setting = \":setting\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &workspace);
    let PackageTargetKind::NativeToolchain(setting) = &loaded.targets[0].kind else {
        panic!("setting must remain a native toolchain target")
    };
    assert!(matches!(
        setting,
        NativeToolchainTarget::ConstraintSetting {
            default_constraint_value: Some(label),
        } if label == &CanonicalLabel::parse("@@//:value").unwrap()
    ));
    assert!(setting.semantic_references().is_empty());
    let default = loaded
        .native_attributes("setting")
        .unwrap()
        .get("default_constraint_value")
        .unwrap();
    assert_eq!(default.1.provenance, AttributeProvenance::Explicit);
    assert!(matches!(
        &default.1.value,
        CoercedAttributeValue::Label(label)
            if label == &CanonicalLabel::parse("@@//:value").unwrap()
    ));
    let PackageTargetKind::NativeToolchain(value) = &loaded.targets[1].kind else {
        panic!("value must remain a native toolchain target")
    };
    assert_eq!(
        value.semantic_references(),
        [CanonicalLabel::parse("@@//:setting").unwrap()]
    );
}

#[test]
fn native_toolchain_target_retains_complete_configurable_declaration_semantics() {
    let workspace = scratch("native-toolchain-complete-semantics");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
constraint_setting(name = "target_setting")
constraint_value(name = "target_value", constraint_setting = ":target_setting")
config_setting(name = "condition", values = {"cpu": "k8"})
toolchain_type(name = "demo_type")
toolchain(
    name = "demo_toolchain",
    toolchain = ":implementation",
    toolchain_type = ":demo_type",
    exec_compatible_with = [],
    target_compatible_with = [":target_value"],
    use_target_platform_constraints = True,
    target_settings = [":always"] + select({
        ":condition": [":selected"],
        "//conditions:default": [":fallback"],
    }),
)
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let target = loaded
        .targets
        .iter()
        .find(|target| target.name == "demo_toolchain")
        .unwrap();
    let PackageTargetKind::NativeToolchain(native) = &target.kind else {
        panic!("demo_toolchain was not retained as a native toolchain")
    };
    let NativeToolchainTarget::Toolchain {
        toolchain_type,
        implementation,
        exec_compatible_with,
        target_compatible_with,
        use_target_platform_constraints,
        target_settings,
    } = native
    else {
        panic!("demo_toolchain has the wrong native kind")
    };
    assert_eq!(toolchain_type.to_string(), "@@//pkg:demo_type");
    assert_eq!(implementation.to_string(), "@@//pkg:implementation");
    assert!(exec_compatible_with.value().is_empty());
    assert_eq!(
        exec_compatible_with.provenance(),
        AttributeProvenance::Explicit
    );
    assert_eq!(
        target_compatible_with.value().as_ref(),
        [CanonicalLabel::parse("@@//pkg:target_value").unwrap()]
    );
    assert_eq!(
        target_compatible_with.provenance(),
        AttributeProvenance::Explicit
    );
    assert!(*use_target_platform_constraints.value());
    assert_eq!(
        use_target_platform_constraints.provenance(),
        AttributeProvenance::Explicit
    );
    assert!(matches!(
        target_settings.value(),
        CoercedAttributeValue::Concatenation(_, _)
    ));
    assert_eq!(target_settings.provenance(), AttributeProvenance::Explicit);
    assert_eq!(
        native.semantic_references(),
        [
            "@@//pkg:demo_type",
            "@@//pkg:implementation",
            "@@//pkg:target_value",
            "@@//pkg:always",
            "@@//pkg:selected",
            "@@//pkg:fallback",
            "@@//pkg:condition",
        ]
        .map(|label| CanonicalLabel::parse(label).unwrap())
    );

    let attributes = loaded.native_attributes("demo_toolchain").unwrap();
    assert!(matches!(
        &attributes.get("target_settings").unwrap().1.value,
        CoercedAttributeValue::Concatenation(_, _)
    ));
    assert_eq!(
        attributes.get("target_settings").unwrap().1.provenance,
        AttributeProvenance::Explicit
    );
    assert!(matches!(
        &attributes.get("$config_dependencies").unwrap().1.value,
        CoercedAttributeValue::LabelList(values)
            if values.as_ref() == [CanonicalLabel::parse("@@//pkg:condition").unwrap()]
    ));
    assert_eq!(
        attributes.get("$config_dependencies").unwrap().1.provenance,
        AttributeProvenance::Implicit
    );
}

#[test]
fn native_module_toolchain_rules_retain_kwargs_and_macro_generator_metadata() {
    let workspace = scratch("native-module-toolchain-rules");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def legacy_native_toolchains(name):
    native.config_setting(
        name = name + "_config",
        values = {"cpu": "k8"},
        define_values = {"mode": "fast"},
        tags = ["z", "a"],
    )
    native.constraint_setting(
        name = name + "_constraint",
        default_constraint_value = ":" + name + "_value",
    )
    native.constraint_value(
        name = name + "_value",
        constraint_setting = ":" + name + "_constraint",
        tags = ["z", "a"],
    )
    native.platform(
        name = name + "_platform",
        constraint_values = [":" + name + "_value"],
        exec_properties = {"key": "value"},
    )
    native.toolchain_type(
        name = name + "_type",
        no_match_error = "missing toolchain",
    )
    native.toolchain(
        name = name + "_toolchain",
        toolchain_type = ":" + name + "_type",
        toolchain = ":" + name + "_impl",
        exec_compatible_with = [":" + name + "_value"],
    )
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"legacy_native_toolchains\")\nlegacy_native_toolchains(name = \"macro_case\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let attrs = |name: &str| loaded.native_attributes(name).unwrap();
    assert!(matches!(
        &attrs("macro_case_config").get("define_values").unwrap().1.value,
        CoercedAttributeValue::StringDict(values)
            if values.as_ref() == [("mode".into(), "fast".into())]
    ));
    assert!(matches!(
        &attrs("macro_case_config").get("tags").unwrap().1.value,
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z"]
    ));
    assert!(matches!(
        &attrs("macro_case_constraint")
            .get("default_constraint_value")
            .unwrap()
            .1
            .value,
        CoercedAttributeValue::Label(value)
            if value == &CanonicalLabel::parse("@@//pkg:macro_case_value").unwrap()
    ));
    assert!(matches!(
        &attrs("macro_case_value").get("constraint_setting").unwrap().1.value,
        CoercedAttributeValue::Label(value)
            if value == &CanonicalLabel::parse("@@//pkg:macro_case_constraint").unwrap()
    ));
    assert!(matches!(
        &attrs("macro_case_platform").get("exec_properties").unwrap().1.value,
        CoercedAttributeValue::StringDict(values)
            if values.as_ref() == [("key".into(), "value".into())]
    ));
    assert!(matches!(
        &attrs("macro_case_type").get("no_match_error").unwrap().1.value,
        CoercedAttributeValue::String(value) if value == "missing toolchain"
    ));
    assert!(matches!(
        &attrs("macro_case_toolchain")
            .get("exec_compatible_with")
            .unwrap()
            .1
            .value,
        CoercedAttributeValue::LabelList(values)
            if values.as_ref() == [CanonicalLabel::parse("@@//pkg:macro_case_value").unwrap()]
    ));
    for name in [
        "macro_case_config",
        "macro_case_constraint",
        "macro_case_value",
        "macro_case_platform",
        "macro_case_type",
        "macro_case_toolchain",
    ] {
        assert!(matches!(
            &attrs(name).get("generator_name").unwrap().1.value,
            CoercedAttributeValue::String(value) if value == "macro_case"
        ));
        assert!(matches!(
            &attrs(name).get("generator_function").unwrap().1.value,
            CoercedAttributeValue::String(value) if value == "legacy_native_toolchains"
        ));
        assert!(matches!(
            &attrs(name).get("generator_location").unwrap().1.value,
            CoercedAttributeValue::String(value)
                if value.starts_with("pkg/BUILD.bazel:") && !value.ends_with(":0")
        ));
    }
}

#[test]
fn native_toolchain_targets_fail_closed_for_wrong_shapes_and_name_collisions() {
    let cases = [
        (
            "constraint_value(name = 'bad', constraint_setting = 1)",
            "constraint_setting",
        ),
        (
            "platform(name = 'bad', constraint_values = ':one')",
            "constraint_values",
        ),
        (
            "toolchain(name = 'bad', target_compatible_with = ':one', toolchain = ':impl', toolchain_type = ':type')",
            "target_compatible_with",
        ),
        (
            "toolchain(name = 'bad', use_target_platform_constraints = 'yes', toolchain = ':impl', toolchain_type = ':type')",
            "use_target_platform_constraints",
        ),
        (
            "toolchain(name = 'bad', target_settings = ':setting', toolchain = ':impl', toolchain_type = ':type')",
            "target_settings",
        ),
        (
            "toolchain(name = 'bad', exec_compatible_with = [], toolchain = '@repo//:impl', toolchain_type = ':type')",
            "no repository visible as '@repo'",
        ),
        (
            "toolchain_type(name = 'bad', data = [':leaf'])",
            "not declared by rule 'toolchain_type'",
        ),
        (
            "constraint_setting(name = 'same')\nplatform(name = 'same', constraint_values = [])",
            "declared more than once",
        ),
    ];
    for (index, (source, expected)) in cases.into_iter().enumerate() {
        let workspace = scratch(&format!("native-toolchain-bad-{index}"));
        let package = workspace.join("pkg");
        fs::create_dir_all(&package).unwrap();
        fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
        fs::write(package.join(BUILD_FILE_PRIMARY), source).unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{source}: {error}");
    }
}

#[test]
fn java_plugins_flag_alias_retains_native_loading_rule_and_empty_private_default() {
    // Bazel 9.2 JavaPluginsFlagAliasRule.java:47-69 adds :java_plugins to
    // NativeBuildRule; Attribute.java:1788-1805 loads [] for its late-bound default.
    let workspace = scratch("java-plugins-flag-alias-loading");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def emit(name):\n    native.java_plugins_flag_alias(name = name, tags = ['z', 'a'])\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        concat!(
            "load(':defs.bzl', 'emit')\n",
            "package(default_visibility = ['//visibility:public'])\n",
            "java_plugins_flag_alias(name = 'direct')\n",
            "java_plugins_flag_alias(name = 'private', visibility = ['//visibility:private'])\n",
            "emit(name = 'macro_case')\n",
        ),
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(loaded.default_visibility, RuleVisibility::Public);
    let expected_slots = [
        "name",
        "visibility",
        "transitive_configs",
        "deprecation",
        "tags",
        "generator_name",
        "generator_function",
        "generator_location",
        "testonly",
        "features",
        ":action_listener",
        "compatible_with",
        "restricted_to",
        "$config_dependencies",
        "package_metadata",
        "aspect_hints",
        "licenses",
        "distribs",
        "target_compatible_with",
        ":java_plugins",
    ];
    for name in ["direct", "private", "macro_case"] {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        assert!(matches!(
            target.kind,
            PackageTargetKind::JavaPluginsFlagAlias
        ));
        let capability = target.rule_capability().unwrap();
        assert_eq!(capability.rule_class, "java_plugins_flag_alias");
        assert!(!capability.executable && capability.test_kind.is_none());

        let attributes = loaded.native_attributes(name).unwrap();
        assert_eq!(
            attributes
                .iter()
                .map(|(schema, _)| schema.query_name())
                .collect::<Vec<_>>(),
            expected_slots
        );
        let plugins = attributes.get(":java_plugins").unwrap().1;
        assert_eq!(plugins.provenance, AttributeProvenance::Implicit);
        assert!(
            matches!(&plugins.value, CoercedAttributeValue::LabelList(values) if values.is_empty())
        );
        assert_eq!(
            target.visibility,
            if name == "private" {
                VisibilitySource::Declared(RuleVisibility::Private)
            } else {
                VisibilitySource::PackageDefault
            }
        );
    }

    let direct = loaded.native_attributes("direct").unwrap();
    let private = loaded.native_attributes("private").unwrap();
    let macro_case = loaded.native_attributes("macro_case").unwrap();
    assert!(matches!(
        &direct.get("visibility").unwrap().1.value,
        CoercedAttributeValue::LabelList(values)
            if values.as_ref() == [CanonicalLabel::parse("@@//visibility:public").unwrap()]
    ));
    assert_eq!(
        direct.get("visibility").unwrap().1.provenance,
        AttributeProvenance::Default
    );
    assert!(matches!(
        &private.get("visibility").unwrap().1.value,
        CoercedAttributeValue::LabelList(values)
            if values.as_ref() == [CanonicalLabel::parse("@@//visibility:private").unwrap()]
    ));
    assert_eq!(
        private.get("visibility").unwrap().1.provenance,
        AttributeProvenance::Explicit
    );
    assert!(
        matches!(&direct.get("generator_name").unwrap().1.value, CoercedAttributeValue::String(value) if value.is_empty())
    );
    assert!(
        matches!(&macro_case.get("generator_name").unwrap().1.value, CoercedAttributeValue::String(value) if value == "macro_case")
    );
    assert!(
        matches!(&macro_case.get("generator_function").unwrap().1.value, CoercedAttributeValue::String(value) if value == "emit")
    );
    assert!(
        matches!(&macro_case.get("generator_location").unwrap().1.value, CoercedAttributeValue::String(value) if value.starts_with("pkg/BUILD.bazel:") && !value.ends_with(":0"))
    );
    assert_eq!(
        macro_case.get("tags").unwrap().1.provenance,
        AttributeProvenance::Explicit
    );
    assert!(
        matches!(&macro_case.get("tags").unwrap().1.value, CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z"])
    );
}

#[test]
fn java_plugins_flag_alias_rejects_duplicate_unknown_and_private_attributes() {
    let workspace = scratch("java-plugins-flag-alias-invalid");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    for (source, diagnostic) in [
        (
            "java_plugins_flag_alias(name = 'same')\njava_plugins_flag_alias(name = 'same')\n",
            "declared more than once",
        ),
        (
            "java_plugins_flag_alias(name = 'bad', data = [])\n",
            "not declared by rule 'java_plugins_flag_alias'",
        ),
        (
            "java_plugins_flag_alias(name = 'bad', **{':java_plugins': []})\n",
            "implicit and cannot be set",
        ),
    ] {
        fs::write(package.join(BUILD_FILE_PRIMARY), source).unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(diagnostic), "{source}: {error}");
    }
}

#[test]
fn java_plugins_flag_alias_imported_source_restores_on_same_dice() {
    let workspace = scratch("java-plugins-flag-alias-source-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'emit')\nemit(name = 'subject')\n",
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |tag: &str| {
        fs::write(
            package.join("defs.bzl"),
            format!("def emit(name):\n    native.java_plugins_flag_alias(name = name, tags = ['{tag}'])\n"),
        )
        .unwrap();
        runtime
            .block_on(load_package_with_retained_dice(&dice, &workspace, &package))
            .unwrap()
    };
    let first = load("one");
    let changed = load("two");
    let restored = load("one");
    let tags = |loaded: &slug_loading_v2::LegacyLoadedPackage| {
        loaded
            .native_attributes("subject")
            .unwrap()
            .get("tags")
            .unwrap()
            .1
            .clone()
    };
    assert!(
        matches!(&tags(&first).value, CoercedAttributeValue::StringList(values) if values.as_ref() == ["one"])
    );
    assert!(
        matches!(&tags(&changed).value, CoercedAttributeValue::StringList(values) if values.as_ref() == ["two"])
    );
    assert_ne!(first, changed);
    assert_eq!(first, restored);
}

#[test]
fn visibility_and_package_group_shapes_retain_bazel_producer_provenance() {
    let workspace = scratch("typed-visibility");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"def _impl(ctx):
    return [DefaultInfo()]

emit = rule(implementation = _impl, attrs = {"out": attr.output(mandatory = True)})

def native_configs():
    native.config_setting(
        name = "native_config_default_public",
        values = {"define": "native_default=1"},
    )
    native.config_setting(
        name = "native_config_declared",
        values = {"define": "native_declared=1"},
        visibility = [":friends"],
    )
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "emit", "native_configs")
package(default_visibility = ["//visibility:private"])
package_group(
    name = "friends",
    packages = ["//viewer", "//viewer/...", "-//viewer/exact_blocked", "-//viewer/blocked/...", "public", "private"],
    includes = [":more", ":later"],
)
filegroup(name = "defaulted")
filegroup(name = "declared", visibility = [":friends", "//viewer:__pkg__", "//viewer:__subpackages__"])
filegroup(name = "explicit_public", visibility = ["//visibility:public", ":friends"])
filegroup(name = "explicit_private", visibility = ["//visibility:private"])
config_setting(name = "config_default_public", values = {"define": "visibility_probe=1"})
config_setting(name = "config_declared", values = {"define": "visibility_probe=1"}, visibility = [":friends"])
exports_files(["public.txt"])
exports_files(["restricted.txt"], visibility = [":friends"])
emit(name = "generator", out = "generated.txt", visibility = [":friends"])
emit(name = "default_generator", out = "default_generated.txt")
native_configs()
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(loaded.default_visibility, RuleVisibility::Private);

    let target = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
    };
    assert_eq!(
        target("defaulted").visibility,
        VisibilitySource::PackageDefault
    );
    assert_eq!(
        target("config_default_public").visibility,
        VisibilitySource::AlwaysPublic
    );
    assert_eq!(
        target("native_config_default_public").visibility,
        VisibilitySource::AlwaysPublic
    );
    assert!(
        target("native_config_default_public")
            .raw_visibility_labels()
            .is_empty()
    );
    assert_eq!(
        target("public.txt").visibility,
        VisibilitySource::AlwaysPublic
    );
    assert_eq!(
        target("generated.txt").visibility,
        VisibilitySource::GeneratingRule
    );
    assert_eq!(
        target("default_generator").visibility,
        VisibilitySource::PackageDefault
    );
    assert_eq!(
        target("default_generated.txt").visibility,
        VisibilitySource::GeneratingRule
    );
    assert!(matches!(
        target("declared").visibility,
        VisibilitySource::Declared(RuleVisibility::Restricted(_))
    ));
    assert!(matches!(
        target("config_declared").visibility,
        VisibilitySource::Declared(RuleVisibility::Restricted(_))
    ));
    assert!(matches!(
        target("native_config_declared").visibility,
        VisibilitySource::Declared(RuleVisibility::Restricted(_))
    ));
    assert_eq!(
        target("native_config_declared")
            .raw_visibility_labels()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["@@//pkg:friends"]
    );
    assert!(matches!(
        target("restricted.txt").visibility,
        VisibilitySource::Declared(RuleVisibility::Restricted(_))
    ));
    assert_eq!(
        target("explicit_public").visibility,
        VisibilitySource::Declared(RuleVisibility::Public)
    );
    assert!(target("explicit_public").visibility_explicit());
    assert!(target("explicit_public").raw_visibility_labels().is_empty());
    assert_eq!(
        target("explicit_private").visibility,
        VisibilitySource::Declared(RuleVisibility::Private)
    );
    assert!(
        target("explicit_private")
            .raw_visibility_labels()
            .is_empty()
    );
    assert_eq!(
        loaded.effective_visibility(target("generator")),
        loaded.effective_visibility(target("generated.txt"))
    );
    assert_eq!(
        loaded.effective_visibility(target("default_generator")),
        loaded.effective_visibility(target("default_generated.txt"))
    );
    assert_eq!(
        loaded.effective_visibility(target("config_default_public")),
        Some(RuleVisibility::Public)
    );
    assert_eq!(
        loaded.effective_visibility(target("native_config_default_public")),
        Some(RuleVisibility::Public)
    );
    assert_eq!(
        loaded.effective_visibility(target("native_config_declared")),
        loaded.effective_visibility(target("config_declared"))
    );
    assert!(matches!(
        loaded.effective_visibility(target("native_config_declared")),
        Some(RuleVisibility::Restricted(_))
    ));

    let PackageTargetKind::PackageGroup { contents, includes } = &target("friends").kind else {
        panic!("friends must be a first-class package group");
    };
    assert!(contents.positive_all());
    assert!(contents.has_private());
    assert_eq!(contents.exact_positive().len(), 1);
    assert_eq!(contents.exact_negative().len(), 1);
    assert_eq!(contents.subtree_positive().len(), 1);
    assert_eq!(contents.subtree_negative().len(), 1);
    assert_eq!(
        includes.as_ref(),
        [
            CanonicalLabel::parse("@@//pkg:more").unwrap(),
            CanonicalLabel::parse("@@//pkg:later").unwrap(),
        ]
    );
    assert_eq!(target("friends").visibility, VisibilitySource::AlwaysPublic);
    assert_eq!(
        loaded.effective_visibility(target("friends")),
        Some(RuleVisibility::Public)
    );
    let viewer = CanonicalLabel::parse("@@//viewer:probe")
        .unwrap()
        .package()
        .clone();
    let viewer_child = CanonicalLabel::parse("@@//viewer/child:probe")
        .unwrap()
        .package()
        .clone();
    let blocked = CanonicalLabel::parse("@@//viewer/blocked/reallowed:probe")
        .unwrap()
        .package()
        .clone();
    let blocked_subtree = CanonicalLabel::parse("@@//viewer/blocked:probe")
        .unwrap()
        .package()
        .clone();
    let exact_blocked = CanonicalLabel::parse("@@//viewer/exact_blocked:probe")
        .unwrap()
        .package()
        .clone();
    assert!(contents.exact_positive().contains(&viewer));
    assert!(contents.exact_negative().contains(&exact_blocked));
    assert_eq!(contents.subtree_positive(), [viewer.clone()]);
    assert_eq!(contents.subtree_negative(), [blocked_subtree]);
    assert!(contents.contains_package(&viewer));
    assert!(contents.contains_package(&viewer_child));
    assert!(!contents.contains_package(&exact_blocked));
    assert!(!contents.contains_package(&blocked));

    let VisibilitySource::Declared(RuleVisibility::Restricted(visibility)) =
        &target("declared").visibility
    else {
        unreachable!()
    };
    assert_eq!(
        visibility
            .declared_labels()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "@@//pkg:friends",
            "@@//viewer:__pkg__",
            "@@//viewer:__subpackages__",
        ]
    );
    assert_eq!(
        visibility
            .package_groups()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["@@//pkg:friends"]
    );
}

#[test]
fn visibility_parsing_reports_pinned_bazel_diagnostics() {
    let workspace = scratch("visibility-diagnostics");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "filegroup(name = \"bad\", visibility = [\"//visibility:plubic\"])\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "Invalid visibility label '//visibility:plubic'; did you mean //visibility:public or //visibility:private?"
        ),
        "{error}"
    );

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "package_group(name = \"bad\", packages = [\"not-a-package\"])\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "invalid package name 'not-a-package': must start with '//', '@', or be 'public' or 'private'"
        ),
        "{error}"
    );
}

#[test]
fn package_load_evaluates_loaded_macro_and_bazel_package_globals() {
    let workspace = scratch("package-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_export(name, src):\n    native.exports_files([src])\n    native.filegroup(name = name, srcs = [src])\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_export\")\npackage(default_visibility = [\"//visibility:public\"])\nexports_files([\"data.txt\"])\nfilegroup(name = \"fg\", srcs = [\"data.txt\"])\nalias(name = \"alias_fg\", actual = \":fg\")\nmake_export(name = \"macro_file\", src = \"macro.txt\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.default_visibility, RuleVisibility::Public);
    assert_eq!(
        loaded.targets,
        vec![
            PackageTarget {
                name: "data.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
                visibility: VisibilitySource::AlwaysPublic,
            },
            PackageTarget {
                name: "fg".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec![CanonicalLabel::parse("@@//pkg:data.txt").unwrap()].into(),
                    srcs_explicit: true,
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "alias_fg".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: AliasActual::Visible(CoercedAttributeValue::Label(
                        CanonicalLabel::parse("@@//pkg:fg").unwrap(),
                    )),
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "macro.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
                visibility: VisibilitySource::AlwaysPublic,
            },
            PackageTarget {
                name: "macro_file".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec![CanonicalLabel::parse("@@//pkg:macro.txt").unwrap()].into(),
                    srcs_explicit: true,
                },
                visibility: VisibilitySource::PackageDefault,
            },
        ]
    );
}

#[test]
fn native_labels_canonicalize_spelling_preserve_order_and_reject_duplicates() {
    let workspace = scratch("native-label-canonicalization");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    let build = package.join(BUILD_FILE_PRIMARY);
    let write_build = |srcs: &str, actual: &str| {
        fs::write(
            &build,
            format!(
                "filegroup(name = \"group\", srcs = {srcs})\nalias(name = \"redirect\", actual = \"{actual}\")\n"
            ),
        )
        .unwrap();
        load_package(&workspace, &package)
    };

    let initial = write_build(
        "[\"one.txt\", \":two.txt\", \"dir/name.txt\", \"//other:cross.txt\"]",
        "group",
    );
    let equivalent = write_build(
        "[\":one.txt\", \"two.txt\", \":dir/name.txt\", \"//other:cross.txt\"]",
        ":group",
    );
    assert_eq!(initial, equivalent);

    let PackageTargetKind::Filegroup { srcs, .. } = &equivalent.targets[0].kind else {
        panic!("expected filegroup")
    };
    assert_eq!(
        srcs.as_ref(),
        [
            CanonicalLabel::parse("@@//pkg:one.txt").unwrap(),
            CanonicalLabel::parse("@@//pkg:two.txt").unwrap(),
            CanonicalLabel::parse("@@//pkg:dir/name.txt").unwrap(),
            CanonicalLabel::parse("@@//other:cross.txt").unwrap(),
        ]
    );
    assert!(matches!(
        &equivalent.targets[1].kind,
        PackageTargetKind::Alias { actual: AliasActual::Visible(actual) }
            if actual == &CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//pkg:group").unwrap()
            )
    ));

    let reordered = write_build(
        "[\"two.txt\", \"one.txt\", \"dir/name.txt\", \"//other:cross.txt\"]",
        "group",
    );
    assert_ne!(equivalent, reordered);

    fs::write(
        &build,
        "filegroup(name = \"group\", srcs = [\"one.txt\", \"two.txt\", \":two.txt\", \":one.txt\"])\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "Label '//pkg:two.txt' is duplicated in the 'srcs' attribute of rule 'group'"
        ),
        "{error}"
    );

    let recovered = write_build(
        "[\"two.txt\", \"one.txt\", \"dir/name.txt\", \"//other:cross.txt\"]",
        "group",
    );
    assert_eq!(reordered, recovered);
}

#[test]
fn direct_starlark_label_lists_reject_explicit_and_materialized_default_duplicates() {
    let workspace = scratch("starlark-label-list-duplicates");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    let definitions = package.join("defs.bzl");
    let build = package.join(BUILD_FILE_PRIMARY);
    let definition = |default: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"deps\": attr.label_list(default = {default})}})\n"
        )
    };

    fs::write(&definitions, definition("[\":default.txt\"]")).unwrap();
    fs::write(
        &build,
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"explicit\", deps = [\"one.txt\", \":one.txt\"])\n",
    )
    .unwrap();
    let explicit = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        explicit.contains(
            "Label '//pkg:one.txt' is duplicated in the 'deps' attribute of rule 'explicit'"
        ),
        "{explicit}"
    );

    fs::write(
        &definitions,
        definition("[\"default.txt\", \":default.txt\"]"),
    )
    .unwrap();
    fs::write(
        &build,
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"defaulted\")\n",
    )
    .unwrap();
    let defaulted = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        defaulted.contains(
            "Label '//pkg:default.txt' is duplicated in the 'deps' attribute of rule 'defaulted'"
        ),
        "{defaulted}"
    );

    fs::write(
        &definitions,
        definition("[\"default.txt\", \":other.txt\"]"),
    )
    .unwrap();
    let recovered = try_load_package(&workspace, &package).unwrap();
    let PackageTargetKind::StarlarkRule(rule) = &recovered.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    assert_eq!(
        rule.dependencies(),
        [
            CanonicalLabel::parse("@@//pkg:default.txt").unwrap(),
            CanonicalLabel::parse("@@//pkg:other.txt").unwrap(),
        ]
    );
}

#[test]
fn test_metadata_retains_inherited_values_suite_provenance_and_bazel_ordering() {
    let workspace = scratch("test-metadata");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nplain = rule(implementation = _impl)\nsample_test = rule(implementation = _impl, test = True)\n",
    )
    .unwrap();
    let build = package.join(BUILD_FILE_PRIMARY);
    fs::write(
        &build,
        "load(\":defs.bzl\", \"plain\", \"sample_test\")\n\
         test_suite(name = \"implicit\")\n\
         test_suite(name = \"empty\", tests = [])\n\
         test_suite(name = \"filtered\", tags = [\"fast\", \"-slow\"])\n\
         test_suite(name = \"literal_plus_excluded\", tags = [\"-+tag\"])\n\
         test_suite(name = \"suite_manual_is_ignored\", tags = [\"manual\"])\n\
         test_suite(name = \"large_only\", tags = [\"large\"])\n\
         test_suite(name = \"explicit\", tests = [\"//ordering/a/b:a\", \"//ordering/a:b/c\", \"//ordering:𐀀\", \"//ordering:\"], tags = [\"z\", \"a\", \"z\"])\n\
         plain(name = \"ordinary\", tags = [\"z\", \"a\", \"z\"])\n\
         sample_test(name = \"auto\")\n\
         sample_test(name = \"fast_test\", tags = [\"fast\"])\n\
         sample_test(name = \"slow_test\", tags = [\"slow\", \"fast\"])\n\
         sample_test(name = \"plus_test\", tags = [\"+tag\"])\n\
         sample_test(name = \"large_test\", size = \"large\")\n\
         sample_test(name = \"manual_test\", tags = [\"𐀀\", \"\", \"𐀀\", \"ascii\", \"manual\", \"-+tag\"], size = \"small\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let target = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
    };
    let implicit_members = [
        CanonicalLabel::parse("@@//pkg:auto").unwrap(),
        CanonicalLabel::parse("@@//pkg:fast_test").unwrap(),
        CanonicalLabel::parse("@@//pkg:large_test").unwrap(),
        CanonicalLabel::parse("@@//pkg:plus_test").unwrap(),
        CanonicalLabel::parse("@@//pkg:slow_test").unwrap(),
    ];
    for (name, explicit) in [("implicit", false), ("empty", true)] {
        let PackageTargetKind::TestSuite { membership, .. } = &target(name).kind else {
            panic!("expected test_suite")
        };
        let TestSuiteMembership::Implicit {
            members,
            tests_explicit,
        } = membership
        else {
            panic!("expected implicit membership")
        };
        assert_eq!(members.as_ref(), implicit_members);
        assert_eq!(*tests_explicit, explicit);
    }
    let implicit = |name: &str| {
        let PackageTargetKind::TestSuite { membership, .. } = &target(name).kind else {
            panic!("expected test_suite")
        };
        membership
            .implicit_tests()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(implicit("filtered"), ["@@//pkg:fast_test"]);
    assert_eq!(
        implicit("literal_plus_excluded"),
        [
            "@@//pkg:auto",
            "@@//pkg:fast_test",
            "@@//pkg:large_test",
            "@@//pkg:slow_test",
        ]
    );
    assert_eq!(
        implicit("suite_manual_is_ignored"),
        [
            "@@//pkg:auto",
            "@@//pkg:fast_test",
            "@@//pkg:large_test",
            "@@//pkg:plus_test",
            "@@//pkg:slow_test",
        ]
    );
    assert_eq!(implicit("large_only"), ["@@//pkg:large_test"]);

    let PackageTargetKind::TestSuite { membership, tags } = &target("explicit").kind else {
        panic!("expected test_suite")
    };
    let TestSuiteMembership::Explicit { tests } = membership else {
        panic!("expected explicit membership")
    };
    assert_eq!(
        tests.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "@@//ordering:",
            "@@//ordering:𐀀",
            "@@//ordering/a:b/c",
            "@@//ordering/a/b:a",
        ]
    );
    assert_eq!(tags.as_ref(), ["a", "z", "z"]);

    let PackageTargetKind::StarlarkRule(ordinary) = &target("ordinary").kind else {
        panic!("expected Starlark rule")
    };
    assert!(matches!(
        ordinary
            .values()
            .iter()
            .find(|value| value.declaration_name == "tags")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z", "z"]
    ));
    assert!(
        ordinary
            .values()
            .iter()
            .all(|value| value.declaration_name != "size")
    );

    let manual = target("manual_test").test_metadata().unwrap();
    assert_eq!(
        manual.tags.as_ref(),
        ["-+tag", "ascii", "manual", "", "𐀀", "𐀀"]
    );
    assert_eq!(manual.size.as_deref(), Some("small"));
    assert!(manual.manual);
    let auto = target("auto").test_metadata().unwrap();
    assert!(auto.tags.is_empty());
    assert_eq!(auto.size.as_deref(), Some("medium"));
    assert!(!auto.manual);
    assert_eq!(
        target("auto").rule_capability().unwrap().test_kind,
        Some(slug_loading_v2::TestRuleKind::Test)
    );
    assert_eq!(
        target("explicit").rule_capability().unwrap().test_kind,
        Some(slug_loading_v2::TestRuleKind::Suite)
    );

    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nbad = rule(implementation = _impl, attrs = {\"tags\": attr.string()})\n",
    )
    .unwrap();
    fs::write(
        &build,
        "load(\":defs.bzl\", \"bad\")\nbad(name = \"bad\")\n",
    )
    .unwrap();
    let tags_redeclaration = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        tags_redeclaration.contains("rule attribute `tags` is built in and cannot be redeclared"),
        "{tags_redeclaration}"
    );

    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nbad_test = rule(implementation = _impl, test = True, attrs = {\"size\": attr.string()})\n",
    )
    .unwrap();
    fs::write(
        &build,
        "load(\":defs.bzl\", \"bad_test\")\nbad_test(name = \"bad\")\n",
    )
    .unwrap();
    let size_redeclaration = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        size_redeclaration.contains("rule attribute `size` is built in and cannot be redeclared"),
        "{size_redeclaration}"
    );

    fs::write(
        &build,
        "test_suite(name = \"bad\", tests = [\"one\", \":one\"])\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Label '//pkg:one' is duplicated in the 'tests' attribute of rule 'bad'"),
        "{error}"
    );
}

#[test]
fn package_load_registers_a_generic_starlark_rule_without_executing_it() {
    let workspace = scratch("rule-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    return [DefaultInfo(files = depset([out]))]\n\nexample = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"example\")\nexample(name = \"registered\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.targets.len(), 1,);
    assert_eq!(loaded.targets[0].name, "registered");
    assert!(matches!(
        loaded.targets[0].kind,
        PackageTargetKind::StarlarkRule(_)
    ));
    let PackageTargetKind::StarlarkRule(implementation) = &loaded.targets[0].kind else {
        unreachable!()
    };
    assert!(implementation.dependencies().is_empty());
}

#[test]
fn rule_predeclared_outputs_resolve_static_and_callback_declarations() {
    let workspace = scratch("rule-predeclared-outputs");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"def _impl(ctx): return [DefaultInfo()]
def _outputs(text, src = None): return {"callback": "%{text}-%{src}.out"}
static = rule(implementation = _impl, attrs = {"src": attr.label(), "words": attr.string_list(), "explicit": attr.output()}, outputs = {"zip": "%{name}.zip", "parts": "%{dirname}%{basename}-%{src}-%{words}.out"}, output_to_genfiles = True)
callback = rule(implementation = _impl, attrs = {"text": attr.string(), "src": attr.label(), "unused": attr.string()}, outputs = _outputs)
omitted = rule(implementation = _impl)
none = rule(implementation = _impl, outputs = None)
empty = rule(implementation = _impl, outputs = {})
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"load(":defs.bzl", "callback", "empty", "none", "omitted", "static")
static(name = "dir/probe", src = ":input.rs", words = ["same", "same"], explicit = "explicit.bin", visibility = ["//visibility:public"])
callback(name = "called", text = "value", src = ":source.rs", unused = select({"//conditions:default": "ignored"}))
omitted(name = "omitted")
none(name = "none")
empty(name = "empty")
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let rule = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(rule) if target.name == "dir/probe" => Some(rule),
            _ => None,
        })
        .unwrap();
    assert!(rule.output_to_genfiles);
    assert_eq!(
        rule.predeclared_outputs
            .iter()
            .map(|output| (output.key.as_str(), output.label.target().as_str()))
            .collect::<Vec<_>>(),
        [
            ("zip", "dir/probe.zip"),
            ("parts", "dir/probe-input-same.out")
        ]
    );
    assert_eq!(
        loaded
            .targets
            .iter()
            .filter_map(|target| match &target.kind {
                PackageTargetKind::GeneratedFile {
                    generating_rule, ..
                } if generating_rule == "dir/probe" => Some(target.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["dir/probe.zip", "dir/probe-input-same.out", "explicit.bin"]
    );
    let generated = loaded
        .targets
        .iter()
        .find(|target| target.name == "dir/probe.zip")
        .unwrap();
    assert_eq!(
        loaded.effective_visibility(generated),
        Some(RuleVisibility::Public)
    );
    let callback = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(rule) if target.name == "called" => Some(rule),
            _ => None,
        })
        .unwrap();
    assert_eq!(callback.predeclared_outputs[0].key, "callback");
    assert_eq!(
        callback.predeclared_outputs[0].label.target().as_str(),
        "value-source.out"
    );
    for name in ["omitted", "none", "empty"] {
        let rule = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::StarlarkRule(rule) = &rule.kind else {
            unreachable!()
        };
        assert!(rule.predeclared_outputs.is_empty() && !rule.output_to_genfiles);
    }
}

#[test]
fn imported_output_callbacks_receive_every_attribute_carrier_once_in_parameter_order() {
    let workspace = scratch("imported-rule-output-callbacks");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    let write = |name: &str, source: &str| fs::write(package.join(name), source).unwrap();
    write(
        "leaf.bzl",
        r#"def direct_outputs(text): return {"direct": "%{text}.direct"}
def residual_outputs(first, *args, named = "default", **kwargs):
    if first != "first-value" or named != "default" or args != ("named-value", "args-value", "kwargs-value") or kwargs != {}: fail("mixed residual carrier mismatch")
    return {"residual": "%{name}.residual"}
def all_outputs(none, boolean, integer, text, label, output, strings, labels, outputs, string_dict, string_lists, string_labels, label_strings, label_lists):
    if none != None or boolean != True or integer != 7 or label.name != "input.rs" or output.name != text + ".out": fail("scalar carrier mismatch")
    if strings != ["a", "b"] or labels[0].name != "input.rs" or outputs[0].name != text + ".list": fail("list carrier mismatch")
    if string_dict["k"] != "v" or string_lists["k"] != ["v"] or string_labels["k"].name != "input.rs" or label_strings[label] != "v" or label_lists["k"][0].name != "input.rs": fail("dictionary carrier mismatch")
    print("callback-" + text)
    return {"first": "%{text}.first", "second": "%{name}.second"}
"#,
    );
    write(
        "middle.bzl",
        "load(\":leaf.bzl\", \"all_outputs\")\ntransitive_outputs = all_outputs\n",
    );
    write(
        "defs.bzl",
        r#"load(":leaf.bzl", "direct_outputs", "residual_outputs")
load(":middle.bzl", "transitive_outputs")
def _impl(ctx): return [DefaultInfo()]
direct = rule(implementation = _impl, attrs = {"text": attr.string()}, outputs = direct_outputs)
residual = rule(implementation = _impl, attrs = {"first": attr.string(), "named": attr.string(), "args": attr.string(), "kwargs": attr.string()}, outputs = residual_outputs)
all_carriers = rule(implementation = _impl, attrs = {"none": attr.label(), "boolean": attr.bool(), "integer": attr.int(), "text": attr.string(), "label": attr.label(), "output": attr.output(), "strings": attr.string_list(), "labels": attr.label_list(), "outputs": attr.output_list(), "string_dict": attr.string_dict(), "string_lists": attr.string_list_dict(), "string_labels": attr.string_keyed_label_dict(), "label_strings": attr.label_keyed_string_dict(), "label_lists": attr.label_list_dict()}, outputs = transitive_outputs)
"#,
    );
    write(
        BUILD_FILE_PRIMARY,
        r#"load(":defs.bzl", "all_carriers", "direct", "residual")
all_carriers(name = "one", boolean = True, integer = 7, text = "one", label = ":input.rs", output = "one.out", strings = ["a", "b"], labels = [":input.rs"], outputs = ["one.list"], string_dict = {"k": "v"}, string_lists = {"k": ["v"]}, string_labels = {"k": ":input.rs"}, label_strings = {":input.rs": "v"}, label_lists = {"k": [":input.rs"]})
all_carriers(name = "two", boolean = True, integer = 7, text = "two", label = ":input.rs", output = "two.out", strings = ["a", "b"], labels = [":input.rs"], outputs = ["two.list"], string_dict = {"k": "v"}, string_lists = {"k": ["v"]}, string_labels = {"k": ":input.rs"}, label_strings = {":input.rs": "v"}, label_lists = {"k": [":input.rs"]})
direct(name = "direct", text = "direct")
residual(name = "residual", first = "first-value", named = "named-value", args = "args-value", kwargs = "kwargs-value")
"#,
    );
    let tracker = Arc::new(PackageEventTracker::default());
    let loaded =
        try_load_package_with_event_capture(&workspace, &package, &[], Some(tracker.clone()), true)
            .unwrap();
    let one = loaded
        .targets
        .iter()
        .position(|target| target.name == "one")
        .unwrap();
    assert_eq!(
        loaded.targets[one + 1..one + 3]
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        ["one.first", "one.second"]
    );
    let has_target = |name| loaded.targets.iter().any(|target| target.name == name);
    assert!(has_target("direct.direct"));
    assert!(has_target("residual.residual"));
    assert_eq!(
        package_event_texts(&tracker.take(), &package),
        Some(vec!["callback-one", "callback-two"])
    );
}

#[test]
fn rule_predeclared_outputs_reject_invalid_bindings_templates_callbacks_and_collisions() {
    let workspace = scratch("rule-predeclared-output-errors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    #[rustfmt::skip]
    let cases = [
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = len)\n", "bad(name = \"bad\")\n", "rule outputs must be a dict, Starlark function, or None"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(_impl, None, None, None, None, None)\n", "bad(name = \"bad\")\n", "positional"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = {1: '%{name}'})\n", "bad(name = \"bad\")\n", "keys must be strings"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = {'out': 1})\n", "bad(name = \"bad\")\n", "values must be strings"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, attrs = {'words': attr.string_list()}, outputs = {'out': '%{words}'})\n", "bad(name = \"bad\", words = [\"one\", \"two\"])\n", "Invalid placeholder(s) in template"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, attrs = {'text': attr.string()}, outputs = {'out': '%{text}'})\n", "bad(name = \"bad\", text = select({\"//conditions:default\": \"value\"}))\n", "is configurable and cannot be used in outputs"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(missing = 'default'): return {'out': '%{name}.out'}\nbad = rule(implementation = _impl, outputs = _out)\n", "bad(name = \"bad\")\n", "either doesn't exist or uses a select()"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(text): return {'out': '%{name}.out'}\nbad = rule(implementation = _impl, attrs = {'text': attr.string()}, outputs = _out)\n", "bad(name = \"bad\", text = select({\"//conditions:default\": \"value\"}))\n", "either doesn't exist or uses a select()"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(text): return []\nbad = rule(implementation = _impl, attrs = {'text': attr.string()}, outputs = _out)\n", "bad(name = \"bad\", text = \"value\")\n", "return value must be a dict"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(*, named): return {'out': '%{name}.out'}\nbad = rule(implementation = _impl, attrs = {'named': attr.string()}, outputs = _out)\n", "bad(name = \"bad\", named = \"value\")\n", "named-only parameter `named`"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(**kwargs): return {'out': '%{name}.out'}\nbad = rule(implementation = _impl, attrs = {'kwargs': attr.string()}, outputs = _out)\n", "bad(name = \"bad\", kwargs = \"value\")\n", "extra positional argument"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(): return {1: '%{name}'}\nbad = rule(implementation = _impl, outputs = _out)\n", "bad(name = \"bad\")\n", "keys must be strings"),
        ("def _impl(ctx): return [DefaultInfo()]\ndef _out(): return {'out': 1}\nbad = rule(implementation = _impl, outputs = _out)\n", "bad(name = \"bad\")\n", "values must be strings"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, attrs = {'out': attr.output()}, outputs = {'out': '%{name}.implicit'})\n", "bad(name = \"bad\", out = \"explicit.out\")\n", "multiple outputs with the same key: out"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = {'out': '//other:output'})\n", "bad(name = \"bad\")\n", "output label must name a valid target in this package"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = {'one': 'same', 'two': 'same'})\n", "bad(name = \"bad\")\n", "target 'same' declared more than once"),
        ("def _impl(ctx): return [DefaultInfo()]\nbad = rule(implementation = _impl, outputs = {'out': '%{name}.generated'})\n", "filegroup(name = \"bad.generated\")\nbad(name = \"bad\")\n", "target 'bad.generated' declared more than once"),
    ];
    for (definitions, invocation, expected) in cases {
        fs::write(package.join("defs.bzl"), definitions).unwrap();
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"bad\")\n{invocation}"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn rule_deps_schema_retains_exact_normalized_order_and_rejects_other_shapes() {
    let workspace = scratch("rule-deps");
    let package = workspace.join("parent");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\n\nwith_deps = rule(implementation = _impl, attrs = {\"deps\": attr.label_list()})\nwithout_deps = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"with_deps\")\nwith_deps(name = \"ordered\", deps = (\"//leaf:second\", \"bare\", \"dir/name\", \":local\", \"//leaf:first\", \"@@external+//leaf:item\"), visibility = [\"//visibility:public\"])\nwith_deps(name = \"omitted\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let dependencies = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "ordered" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        dependencies,
        [
            CanonicalLabel::parse("@@//leaf:second").unwrap(),
            CanonicalLabel::parse("@@//parent:bare").unwrap(),
            CanonicalLabel::parse("@@//parent:dir/name").unwrap(),
            CanonicalLabel::parse("@@//parent:local").unwrap(),
            CanonicalLabel::parse("@@//leaf:first").unwrap(),
            CanonicalLabel::parse("@@external+//leaf:item").unwrap(),
        ]
    );
    let omitted = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "omitted" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert!(omitted.is_empty());

    let bad_builds = [
        (
            "with_deps(name = \"bad\", unknown = [])\n",
            "unknown attribute `unknown`",
        ),
        (
            "without_deps(name = \"bad\", deps = [])\n",
            "unknown attribute `deps`",
        ),
        (
            "with_deps(name = \"bad\", deps = [1])\n",
            "attribute `label list` must be a string",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"pkg:target\"])\n",
            "absolute label must begin with '@' or '//'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"...\"])\n",
            "package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"foo/...\"])\n",
            "package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"...:all\"])\n",
            "absolute label must begin with '@' or '//'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"foo/...:all\"])\n",
            "absolute label must begin with '@' or '//'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"//foo/...\"])\n",
            "package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"//foo/...:all\"])\n",
            "package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@repo//leaf:one\"])\n",
            "no repository visible as '@repo'",
        ),
    ];
    for (invocation, expected) in bad_builds {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"with_deps\", \"without_deps\")\n{invocation}"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "error: {error}");
    }
}

#[test]
fn scalar_and_string_attr_descriptors_retain_typed_values() {
    let workspace = scratch("scalar-string-descriptors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

probe = rule(
    implementation = _impl,
    attrs = {
        "flag": attr.bool(default = True),
        "count": attr.int(default = -2),
        "words": attr.string_list(default = ("z", "z", "a")),
        "properties": attr.string_dict(default = {"z": "1", "a": "2"}),
        "word_map": attr.string_list_dict(default = {"z": ("p", "p", "a"), "a": ("x",)}),
        "label_map": attr.label_list_dict(default = {"z": (":leaf", ":leaf")}),
        "optional": attr.label(default = None, allow_single_file = True),
        "no_files": attr.label(allow_single_file = False),
        "extensions": attr.label(allow_single_file = (".txt", ".md")),
        "omitted_files": attr.label(),
        "explicit_none_files": attr.label(allow_single_file = None),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "probe")
probe(
    name = "subject",
    flag = False,
    count = 7,
    words = ("x", "x", "y"),
    properties = {"b": "2", "a": "1"},
    word_map = {"first": ("m", "m"), "second": ("n",)},
    optional = None,
)
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    let schema = |name: &str| {
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == name)
            .unwrap()
    };
    assert_eq!(schema("flag").kind(), AttributeKind::Boolean);
    assert_eq!(schema("count").kind(), AttributeKind::Integer);
    assert_eq!(schema("words").kind(), AttributeKind::StringList);
    assert_eq!(schema("properties").kind(), AttributeKind::StringDict);
    assert_eq!(schema("word_map").kind(), AttributeKind::StringListDict);
    assert_eq!(schema("label_map").kind(), AttributeKind::LabelListDict);
    assert!(schema("optional").file_admissibility().is_any_file());
    assert!(schema("optional").file_admissibility().single_artifact());
    assert!(schema("no_files").file_admissibility().is_no_files());
    assert!(schema("no_files").file_admissibility().single_artifact());
    assert_eq!(
        schema("extensions").file_admissibility().suffixes(),
        Some([".txt".into(), ".md".into()].as_slice())
    );
    assert!(schema("extensions").file_admissibility().single_artifact());
    assert!(schema("omitted_files").file_admissibility().is_no_files());
    assert!(
        !schema("omitted_files")
            .file_admissibility()
            .single_artifact()
    );
    assert!(
        schema("explicit_none_files")
            .file_admissibility()
            .is_no_files()
    );
    assert!(
        !schema("explicit_none_files")
            .file_admissibility()
            .single_artifact()
    );
    assert!(schema("flag").configurable());
    assert!(matches!(
        schema("optional").default(),
        Some(CoercedAttributeValue::None)
    ));
    assert!(matches!(
        schema("word_map").default(),
        Some(CoercedAttributeValue::StringListDict(values))
            if values.as_ref()
                == [
                    ("z".into(), Arc::from(["p".into(), "p".into(), "a".into()])),
                    ("a".into(), Arc::from(["x".into()])),
                ]
    ));

    let value = |name: &str| {
        rule.values()
            .iter()
            .find(|value| value.declaration_name == name)
            .unwrap()
            .value
            .as_ref()
    };
    assert!(matches!(
        value("flag"),
        CoercedAttributeValue::Boolean(false)
    ));
    assert!(matches!(value("count"), CoercedAttributeValue::Integer(7)));
    assert!(matches!(value("optional"), CoercedAttributeValue::None));
    assert_eq!(
        rule.values()
            .iter()
            .find(|value| value.declaration_name == "optional")
            .unwrap()
            .provenance,
        AttributeProvenance::Explicit
    );
    assert!(matches!(
        value("words"),
        CoercedAttributeValue::StringList(values)
            if values.as_ref() == ["x", "x", "y"]
    ));
    assert!(matches!(
        value("properties"),
        CoercedAttributeValue::StringDict(values)
            if values.as_ref() == [("b".into(), "2".into()), ("a".into(), "1".into())]
    ));
    assert!(matches!(
        value("word_map"),
        CoercedAttributeValue::StringListDict(values)
            if values.as_ref()
                == [
                    ("first".into(), Arc::from(["m".into(), "m".into()])),
                    ("second".into(), Arc::from(["n".into()])),
                ]
    ));
    let mut labels = Vec::new();
    value("word_map").labels(&mut labels);
    assert!(labels.is_empty());
    schema("label_map").default().unwrap().labels(&mut labels);
    assert_eq!(
        labels,
        [
            CanonicalLabel::parse("@@//pkg:leaf").unwrap(),
            CanonicalLabel::parse("@@//pkg:leaf").unwrap(),
        ]
    );
}

#[test]
fn direct_compile_input_flags_reach_instantiated_rule_schema() {
    let workspace = scratch("direct-compile-input-schema");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    let definitions = "def _impl(ctx): return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {'srcs': attr.label_list(flags = ['DIRECT_COMPILE_TIME_INPUT']), 'deps': attr.label_list()})\n";
    fs::write(package.join("defs.bzl"), definitions).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'probe')\nprobe(name = 'subject')\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    let flag = |name: &str| {
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == name)
            .unwrap()
            .direct_compile_time_input()
    };
    assert!(flag("srcs"));
    assert!(!flag("deps"));

    let dice = Dice::builder().build(DetectCycles::Enabled);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let first = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        fs::write(
            package.join("defs.bzl"),
            definitions.replace("DIRECT_COMPILE_TIME_INPUT", "SKIP_CONSTRAINTS_OVERRIDE"),
        )
        .unwrap();
        let changed = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_ne!(first, changed);
        fs::write(package.join("defs.bzl"), definitions).unwrap();
        let restored = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_eq!(first, restored);
    });
}

#[test]
fn rule_rejects_explicit_configurable_descriptor_arguments() {
    let workspace = scratch("explicit-configurable");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"subject\")\n",
    )
    .unwrap();
    for value in ["True", "False"] {
        fs::write(
            package.join("defs.bzl"),
            format!(
                "def _impl(ctx):\n    return [DefaultInfo()]\n\nprobe = rule(implementation = _impl, attrs = {{\"x\": attr.bool(configurable = {value})}})\n"
            ),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(
                "attribute 'x' has the 'configurable' argument set, which is not allowed in rule definitions"
            ),
            "error: {error}"
        );
    }
}

#[test]
fn non_label_descriptors_reject_none_defaults() {
    let workspace = scratch("non-label-none-default");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"subject\")\n",
    )
    .unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\n\nprobe = rule(implementation = _impl, attrs = {\"x\": attr.bool(default = None)})\n",
    )
    .unwrap();

    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "attribute values must contain strings, booleans, integers, lists, or dictionaries"
        ),
        "error: {error}"
    );
}

#[test]
fn attr_and_transition_parameters_are_named_only_and_transition_inputs_are_required() {
    let workspace = scratch("named-only-schema-parameters");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"subject\")\n",
    )
    .unwrap();

    let invalid_definitions = [
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {\"x\": attr.bool(True)})\n",
        "def _impl(ctx):\n    return [DefaultInfo()]\ndef _transition_impl(settings, attr):\n    return {}\nprobe = rule(implementation = _impl, attrs = {\"x\": attr.label(cfg = transition(_transition_impl, inputs = [], outputs = [\"//pkg:setting\"]))})\n",
        "def _impl(ctx):\n    return [DefaultInfo()]\ndef _transition_impl(settings, attr):\n    return {}\nprobe = rule(implementation = _impl, attrs = {\"x\": attr.label(cfg = transition(implementation = _transition_impl, outputs = [\"//pkg:setting\"]))})\n",
    ];
    for definitions in invalid_definitions {
        fs::write(package.join("defs.bzl"), definitions).unwrap();
        assert!(
            try_load_package(&workspace, &package).is_err(),
            "definition unexpectedly accepted: {definitions}"
        );
    }
}

#[test]
fn transition_declarations_retain_complete_sorted_identity_and_spelling() {
    let workspace = scratch("transition-declaration-identity");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"incoming\", \"probe\")\nprobe(name = \"subject\")\nincoming(name = \"incoming\")\n",
    )
    .unwrap();
    let definitions = r#"def _impl(ctx): return []
def _transition(settings, attr): return {}
complete = transition(
    implementation = _transition,
    inputs = ("@//pkg:zeta", "//pkg:alpha", "//command_line_option:compilation_mode"),
    outputs = [
        "//pkg:𐀀",
        "//pkg:",
        "//pkg:alpha",
        "@//command_line_option:experimental_alias",
        "@@//command_line_option:incompatible_alias",
        "//command_line_option:incompatible_enable_cc_toolchain_resolution",
        "//command_line_option:incompatible_enable_apple_toolchain_resolution",
    ],
)
empty = transition(implementation = _transition, inputs = [], outputs = [])
incoming = rule(implementation = _impl, cfg = complete)
probe = rule(implementation = _impl, attrs = {
    "complete": attr.label(cfg = complete),
    "empty": attr.label(cfg = empty),
})
"#;
    fs::write(package.join("defs.bzl"), definitions).unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "subject")
        .unwrap()
        .kind
    else {
        panic!("expected Starlark rule")
    };
    let transition = |name: &str| {
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == name)
            .unwrap()
            .transition()
            .unwrap()
    };
    let complete = transition("complete");
    assert_eq!(
        complete
            .inputs()
            .iter()
            .map(|setting| (setting.canonical().to_string(), setting.declared()))
            .collect::<Vec<_>>(),
        [
            (
                "@@//command_line_option:compilation_mode".to_owned(),
                "//command_line_option:compilation_mode",
            ),
            ("@@//pkg:alpha".to_owned(), "//pkg:alpha"),
            ("@@//pkg:zeta".to_owned(), "@//pkg:zeta"),
        ]
    );
    assert_eq!(
        complete
            .outputs()
            .iter()
            .map(|setting| setting.declared())
            .collect::<Vec<_>>(),
        [
            "@//command_line_option:experimental_alias",
            "@@//command_line_option:incompatible_alias",
            "//command_line_option:incompatible_enable_apple_toolchain_resolution",
            "//command_line_option:incompatible_enable_cc_toolchain_resolution",
            "//pkg:alpha",
            "//pkg:",
            "//pkg:𐀀",
        ]
    );
    assert!(
        complete.outputs()[..4]
            .iter()
            .all(|setting| setting.is_native_option())
    );
    assert_eq!(complete.inputs()[1], complete.outputs()[4]);
    assert!(transition("empty").inputs().is_empty());
    assert!(transition("empty").outputs().is_empty());
    let PackageTargetKind::StarlarkRule(incoming) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "incoming")
        .unwrap()
        .kind
    else {
        panic!("expected incoming-transition rule")
    };
    assert_eq!(incoming.incoming_transition(), Some(complete));

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let first = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        fs::write(
            package.join("defs.bzl"),
            definitions.replace(
                "inputs = (\"@//pkg:zeta\", \"//pkg:alpha\", \"//command_line_option:compilation_mode\")",
                "inputs = (\"//command_line_option:compilation_mode\", \"//pkg:alpha\", \"@//pkg:zeta\")",
            ),
        )
        .unwrap();
        let reordered = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_eq!(first.targets, reordered.targets);
        fs::write(
            package.join("defs.bzl"),
            definitions.replace("@//pkg:zeta", "@//pkg:beta"),
        )
        .unwrap();
        let changed = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_ne!(first.targets, changed.targets);
        fs::write(package.join("defs.bzl"), definitions).unwrap();
        let restored = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_eq!(first, restored);
    });
}

#[test]
fn rule_level_transition_binding_identity_and_allowlist_are_generic() {
    let workspace = scratch("rule-level-transition-attachment");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"def _impl(ctx): return []
def _transition(settings, attr): return {}
T = transition(implementation = _transition, inputs = ["//pkg:z", "//pkg:a"], outputs = ["//pkg:out"])
plain = rule(implementation = _impl, build_setting = None, cfg = None)
incoming = rule(implementation = _impl, build_setting = None, cfg = T)
attribute = rule(implementation = _impl, attrs = {"dep": attr.label(cfg = T)})
both = rule(implementation = _impl, cfg = T, attrs = {"dep": attr.label(cfg = T)})
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"load(":defs.bzl", "attribute", "both", "incoming", "plain")
plain(name = "plain")
incoming(name = "incoming")
attribute(name = "attribute")
both(name = "both")
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let rule = |name: &str| {
        let PackageTargetKind::StarlarkRule(rule) = &loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
            .kind
        else {
            panic!("expected Starlark rule {name}")
        };
        rule
    };
    let allowlist_count = |name: &str| {
        rule(name)
            .schema()
            .iter()
            .filter(|schema| schema.declaration_name() == "$allowlist_function_transition")
            .count()
    };
    assert!(rule("plain").incoming_transition().is_none());
    assert_eq!(allowlist_count("plain"), 0);
    assert!(rule("incoming").incoming_transition().is_some());
    assert_eq!(allowlist_count("incoming"), 1);
    assert!(rule("attribute").incoming_transition().is_none());
    assert_eq!(allowlist_count("attribute"), 1);
    assert_eq!(allowlist_count("both"), 1);

    let incoming = rule("both").incoming_transition().unwrap();
    let attribute = rule("both")
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "dep")
        .unwrap()
        .transition()
        .unwrap();
    assert!(
        incoming
            .implementation()
            .to_value()
            .ptr_eq(attribute.implementation().to_value())
    );
    assert_eq!(incoming, attribute);
    assert_eq!(
        incoming
            .inputs()
            .iter()
            .map(|setting| setting.declared())
            .collect::<Vec<_>>(),
        ["//pkg:a", "//pkg:z"]
    );
    let allowlist = CanonicalLabel::parse(
        "@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist",
    )
    .unwrap();
    for name in ["incoming", "attribute", "both"] {
        assert!(rule(name).dependencies().contains(&allowlist), "{name}");
    }

    let invalid = [
        (
            "invalid-cfg",
            "probe = rule(implementation = _impl, cfg = 'bad')",
            "`cfg` must be set to a transition object initialized by the transition() function.",
        ),
        (
            "conflict-precedes-cfg-conversion",
            "probe = rule(implementation = _impl, build_setting = config.string(), cfg = 'bad')",
            "Build setting rules cannot use the `cfg` param to apply transitions to themselves.",
        ),
        (
            "invalid-build-setting-precedes-conflict",
            "probe = rule(implementation = _impl, build_setting = 'bad', cfg = 'bad')",
            "rule build_setting must use config.int(), config.string(), config.bool(), config.string_list(), or config.string_set()",
        ),
    ];
    for (name, declaration, expected) in invalid {
        let workspace = scratch(name);
        let package = workspace.join("pkg");
        fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            "load(':defs.bzl', 'probe')\nprobe(name = 'subject')\n",
        )
        .unwrap();
        fs::write(
            package.join("defs.bzl"),
            format!("def _impl(ctx): return []\n{declaration}\n"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{name}: {error}");
    }

    let workspace = scratch("rule-cfg-named-only");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'probe')\nexports_files([])\n",
    )
    .unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx): return []\ndef _t(settings, attr): return {}\nT = transition(implementation = _t, inputs = [], outputs = [])\nprobe = rule(_impl, None, None, None, None, T)\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Found 1 extra positional argument(s) for call to rule"),
        "{error}"
    );
}

#[test]
fn transition_declaration_validation_matches_bazel_phase_order() {
    let workspace = scratch("transition-declaration-errors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"T\")\n",
    )
    .unwrap();
    let cases = [
        (
            "1, inputs = [1], outputs = [2]",
            "Type of parameter `implementation` doesn't match",
        ),
        (
            "_transition, inputs = [1], outputs = [2]",
            "Type of parameter `inputs` doesn't match",
        ),
        (
            "_transition, inputs = [\"//pkg:a\", \"//pkg:a\"], outputs = [\"//pkg:b\", \"//pkg:b\"]",
            "duplicate transition input '//pkg:a'",
        ),
        (
            "_transition, inputs = [], outputs = [\"//pkg:b\", \"//pkg:b\"]",
            "duplicate transition output '//pkg:b'",
        ),
        (
            "_transition, inputs = [\"@//pkg:a\", \"//pkg:a\"], outputs = [\"@//pkg:b\", \"//pkg:b\"]",
            "duplicate build setting '//pkg:b' in OUTPUTS (specified as '//pkg:b' and '@//pkg:b')",
        ),
        (
            "_transition, inputs = [\"@//pkg:a\", \"//pkg:a\"], outputs = []",
            "duplicate build setting '//pkg:a' in INPUTS (specified as '//pkg:a' and '@//pkg:a')",
        ),
        (
            "_transition, inputs = [], outputs = [\"not:a\"]",
            "invalid transition output 'not:a'",
        ),
        (
            "_transition, inputs = [], outputs = [\":relative\"]",
            "invalid transition output ':relative'",
        ),
        (
            "_transition, inputs = [], outputs = [\"@missing//:setting\"]",
            "no repo visible as @missing",
        ),
        (
            "_transition, inputs = [], outputs = [\"//pkg/...:setting\"]",
            "package name cannot contain '...'",
        ),
        (
            "_transition, inputs = [\"//command_line_option:experimental_bad\"], outputs = []",
            "Cannot transition on --experimental_* or --incompatible_* options",
        ),
        (
            "_transition, inputs = [], outputs = [\"//command_line_option:incompatible_bad\"]",
            "Cannot transition on --experimental_* or --incompatible_* options",
        ),
    ];
    for (arguments, expected) in cases {
        fs::write(
            package.join("defs.bzl"),
            format!(
                "def _transition(settings, attr): return {{}}\nT = transition(implementation = {arguments})\n"
            ),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(expected),
            "arguments: {arguments}\nerror: {error}"
        );
    }
}

#[test]
fn exec_and_executable_attributes_retain_loading_identity_and_restore() {
    let workspace = scratch("executable-exec-attribute-policy");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"leaf\", \"probe\")\nleaf(name = \"binary\")\nprobe(name = \"subject\", binary = \":binary\")\n",
    )
    .unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"def _impl(ctx): return [DefaultInfo()]
def _transition(settings, attr): return {}
leaf = rule(implementation = _impl)
probe = rule(implementation = _impl, attrs = {
    "target_dep": attr.label(default = ":binary"),
    "binary": attr.label(cfg = "exec", allow_single_file = True, mandatory = True),
    "executable_dep": attr.label(cfg = transition(implementation = _transition, inputs = [], outputs = ["//pkg:setting"]), executable = True, default = ":binary"),
})
"#,
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "subject")
        .unwrap()
        .kind
    else {
        panic!("expected Starlark rule")
    };
    let schema = |name: &str| {
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == name)
            .unwrap()
    };
    assert!(matches!(
        schema("target_dep").dependency_configuration(),
        AttributeDependencyConfiguration::Target
    ));
    assert!(matches!(
        schema("binary").dependency_configuration(),
        AttributeDependencyConfiguration::Exec
    ));
    assert!(schema("binary").file_admissibility().is_any_file());
    assert!(schema("binary").file_admissibility().single_artifact());
    assert!(schema("binary").mandatory());
    assert!(!schema("binary").executable());
    let AttributeDependencyConfiguration::Starlark(transition) =
        schema("executable_dep").dependency_configuration()
    else {
        panic!("custom transition identity was not retained")
    };
    assert_eq!(transition.outputs()[0].declared(), "//pkg:setting");
    assert!(schema("executable_dep").executable());
    assert_ne!(schema("target_dep"), schema("binary"));
    assert_ne!(schema("binary"), schema("executable_dep"));
    assert!(
        rule.dependencies()
            .iter()
            .any(|label| label.target().as_str() == "binary")
    );

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let restored = runtime.block_on(async {
        let first = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        let target_source = fs::read_to_string(package.join("defs.bzl"))
            .unwrap()
            .replace("cfg = \"exec\", ", "");
        fs::write(package.join("defs.bzl"), &target_source).unwrap();
        let target = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_ne!(first, target);
        let exec_source = target_source.replace(
            "attr.label(allow_single_file = True",
            "attr.label(cfg = \"exec\", allow_single_file = True",
        );
        fs::write(package.join("defs.bzl"), exec_source).unwrap();
        let restored = load_package_with_retained_dice(&dice, &workspace, &package)
            .await
            .unwrap();
        assert_eq!(first, restored);
        restored
    });
    assert_eq!(loaded, restored);

    for attribute in [
        "attr.label(False)",
        "attr.string(\"doc\")",
        "attr.label(doc = 1)",
        "attr.string(doc = 1)",
        "attr.label(executable = True)",
        "attr.label(cfg = 1)",
    ] {
        fs::write(
            package.join("defs.bzl"),
            format!(
                "def _impl(ctx): return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"x\": {attribute}}})\n"
            ),
        )
        .unwrap();
        assert!(
            try_load_package(&workspace, &package).is_err(),
            "{attribute}"
        );
    }
}

#[test]
fn transition_rejects_recursive_package_patterns_but_allows_ellipsis_in_target_names() {
    let workspace = scratch("transition-recursive-package-pattern");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"subject\")\n",
    )
    .unwrap();
    let definitions = |output: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\ndef _transition_impl(settings, attr):\n    return {{}}\nprobe = rule(implementation = _impl, attrs = {{\"x\": attr.label(cfg = transition(implementation = _transition_impl, inputs = [], outputs = [\"{output}\"]))}})\n"
        )
    };

    fs::write(
        package.join("defs.bzl"),
        definitions("//pkg:setting...variant"),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    assert_eq!(
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == "x")
            .unwrap()
            .transition()
            .unwrap()
            .outputs()[0]
            .declared(),
        "//pkg:setting...variant"
    );

    fs::write(package.join("defs.bzl"), definitions("//pkg/...:setting")).unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("package name cannot contain '...'"),
        "error: {error}"
    );
}

#[test]
fn label_defaults_use_the_defining_bzl_package_while_explicit_values_use_build_package() {
    let workspace = scratch("definition-package-defaults");
    let definitions = workspace.join("definitions");
    let consumer = workspace.join("consumer");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&definitions).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::write(
        definitions.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

probe = rule(
    implementation = _impl,
    attrs = {
        "explicit": attr.label_list(mandatory = True),
        "scalar": attr.label(default = "scalar.txt"),
        "defaulted": attr.label_list(default = ["default.txt", "dir/default.txt", ":colon.txt", "//:root.txt"]),
        "string_labels": attr.string_keyed_label_dict(default = {"key": "dict-value.txt"}),
        "label_strings": attr.label_keyed_string_dict(default = {"dict-key.txt": "value"}),
        "label_lists": attr.label_list_dict(default = {"key": ["list-value.txt"]}),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        consumer.join(BUILD_FILE_PRIMARY),
        "load(\"//definitions:defs.bzl\", \"probe\")\nprobe(name = \"consumer\", explicit = [\"bare.txt\", \"dir/bare.txt\", \":colon.txt\", \"//other:cross.txt\", \"//:root.txt\"])\n",
    )
    .unwrap();

    let loaded =
        try_load_package_with_extra_bzl(&workspace, &consumer, &[definitions.join("defs.bzl")])
            .unwrap();
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    let labels_for = |name| {
        let mut labels = Vec::new();
        rule.values()
            .iter()
            .find(|value| value.declaration_name == name)
            .unwrap()
            .value
            .labels(&mut labels);
        labels
    };
    assert_eq!(
        labels_for("explicit"),
        vec![
            CanonicalLabel::parse("@@//consumer:bare.txt").unwrap(),
            CanonicalLabel::parse("@@//consumer:dir/bare.txt").unwrap(),
            CanonicalLabel::parse("@@//consumer:colon.txt").unwrap(),
            CanonicalLabel::parse("@@//other:cross.txt").unwrap(),
            CanonicalLabel::parse("@@//:root.txt").unwrap(),
        ]
    );
    let defining_defaults = vec![
        CanonicalLabel::parse("@@//definitions:default.txt").unwrap(),
        CanonicalLabel::parse("@@//definitions:dir/default.txt").unwrap(),
        CanonicalLabel::parse("@@//definitions:colon.txt").unwrap(),
        CanonicalLabel::parse("@@//:root.txt").unwrap(),
    ];
    assert_eq!(
        labels_for("scalar"),
        [CanonicalLabel::parse("@@//definitions:scalar.txt").unwrap()]
    );
    assert_eq!(labels_for("defaulted"), defining_defaults);
    assert_eq!(
        labels_for("string_labels"),
        [CanonicalLabel::parse("@@//definitions:dict-value.txt").unwrap()]
    );
    assert_eq!(
        labels_for("label_strings"),
        [CanonicalLabel::parse("@@//definitions:dict-key.txt").unwrap()]
    );
    assert_eq!(
        labels_for("label_lists"),
        [CanonicalLabel::parse("@@//definitions:list-value.txt").unwrap()]
    );
    let defaulted_schema = rule
        .schema()
        .iter()
        .find(|schema| schema.declaration_name() == "defaulted")
        .unwrap();
    assert!(matches!(
        defaulted_schema.default(),
        Some(CoercedAttributeValue::LabelList(labels))
            if labels.as_ref() == defining_defaults.as_slice()
    ));
}

#[test]
fn build_label_keyed_dictionary_rejects_keys_that_canonicalize_alike() {
    let workspace = scratch("label-key-collision");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

probe = rule(
    implementation = _impl,
    attrs = {"values": attr.label_keyed_string_dict()},
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "probe")
probe(name = "subject", values = {":same": "first", "//pkg:same": "second"})
"#,
    )
    .unwrap();
    let error = try_load_package_with_extra_bzl(&workspace, &package, &[package.join("defs.bzl")])
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate canonical label"), "{error}");
}

#[test]
fn rule_attribute_schema_retains_provenance_selectors_dicts_and_generated_outputs() {
    let workspace = scratch("attribute-metadata");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

probe = rule(
    implementation = _impl,
    attrs = {
        "one": attr.label(),
        "many": attr.label_list(default = [":default"]),
        "note": attr.string(default = "text"),
        "_implicit": attr.label(default = ":implicit"),
        "chosen": attr.label_list(),
        "string_labels": attr.string_keyed_label_dict(),
        "label_strings": attr.label_keyed_string_dict(),
        "label_lists": attr.label_list_dict(),
        "out": attr.output(mandatory = True),
        "outs": attr.output_list(mandatory = True),
        "trailing": attr.label_list(),
        "chained": attr.label_list(),
        "nested_prefix": attr.label_list(),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "probe")
probe(
    name = "metadata",
    one = ":source",
    chosen = [":shared"] + select({":condition": [":linux"], "//conditions:default": [":fallback"]}),
    string_labels = {"local": ":source"},
    label_strings = {":default": "default"},
    label_lists = {"local": [":implicit", ":source"]},
    out = "dir/one.out",
    outs = [":two.out", "//pkg:three.out"],
    trailing = select({":condition": [":linux"], "//conditions:default": [":fallback"]}) + [":after"],
    chained = [":before"] + select({":condition": [":one"], "//conditions:default": [":one_default"]}) + select({":second_condition": [":two"], "//conditions:default": [":two_default"]}) + [":after"] + [":again"],
    nested_prefix = [":outer"] + ([":inner"] + select({":condition": [":selected"]})),
)
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    assert_eq!(
        rule.schema()[22..]
            .iter()
            .map(|schema| schema.kind())
            .collect::<Vec<_>>(),
        vec![
            AttributeKind::Label,
            AttributeKind::LabelList,
            AttributeKind::String,
            AttributeKind::Label,
            AttributeKind::LabelList,
            AttributeKind::StringKeyedLabelDict,
            AttributeKind::LabelKeyedStringDict,
            AttributeKind::LabelListDict,
            AttributeKind::Output,
            AttributeKind::OutputList,
            AttributeKind::LabelList,
            AttributeKind::LabelList,
            AttributeKind::LabelList,
        ]
    );
    let schema = &rule.schema()[22..];
    assert_eq!(schema[3].declaration_name(), "_implicit");
    assert_eq!(schema[3].query_name(), "$implicit");
    assert!(schema[3].default().is_some());
    assert!(schema[8].mandatory());
    assert!(schema[0].configurable());
    assert!(!schema[8].configurable());
    assert!(!schema[9].configurable());
    assert!(schema[2].dependency_reachable() == false);
    assert!(matches!(
        schema[0].default(),
        Some(CoercedAttributeValue::None)
    ));
    assert!(
        matches!(schema[4].default(), Some(CoercedAttributeValue::LabelList(values)) if values.is_empty())
    );
    assert!(
        matches!(schema[5].default(), Some(CoercedAttributeValue::StringKeyedLabelDict(values)) if values.is_empty())
    );
    assert!(
        matches!(schema[6].default(), Some(CoercedAttributeValue::LabelKeyedStringDict(values)) if values.is_empty())
    );
    assert!(
        matches!(schema[7].default(), Some(CoercedAttributeValue::LabelListDict(values)) if values.is_empty())
    );
    assert!(
        matches!(schema[9].default(), Some(CoercedAttributeValue::OutputList(values)) if values.is_empty())
    );

    let values = rule.values();
    let CoercedAttributeValue::LabelList(config_dependencies) = values
        .iter()
        .find(|value| value.declaration_name == "$config_dependencies")
        .unwrap()
        .value
        .as_ref()
    else {
        panic!("config dependency field should be a label list")
    };
    assert_eq!(
        config_dependencies
            .iter()
            .map(|label| label.target().as_str())
            .collect::<Vec<_>>(),
        ["condition", "second_condition"]
    );
    assert!(
        rule.dependencies()
            .contains(&CanonicalLabel::parse("@@//pkg:condition").unwrap())
    );
    let values = &values[22..];
    assert_eq!(values[0].provenance, AttributeProvenance::Explicit);
    assert_eq!(values[1].provenance, AttributeProvenance::Default);
    assert_eq!(values[2].provenance, AttributeProvenance::Default);
    assert_eq!(values[3].provenance, AttributeProvenance::Implicit);
    assert!(matches!(
        values.iter().find(|value| value.declaration_name == "note").unwrap().value.as_ref(),
        CoercedAttributeValue::String(value) if value == "text"
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "string_labels")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::StringKeyedLabelDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "label_strings")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::LabelKeyedStringDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "label_lists")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::LabelListDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "chosen")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::Concatenation(_, _)
    ));
    let mut chosen_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "chosen")
        .unwrap()
        .value
        .labels(&mut chosen_labels);
    assert_eq!(
        chosen_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:shared").unwrap(),
            CanonicalLabel::parse("@@//pkg:linux").unwrap(),
            CanonicalLabel::parse("@@//pkg:fallback").unwrap(),
        ]
    );
    assert!(!chosen_labels.contains(&CanonicalLabel::parse("@@//pkg:condition").unwrap()));
    let mut trailing_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "trailing")
        .unwrap()
        .value
        .labels(&mut trailing_labels);
    assert_eq!(
        trailing_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:linux").unwrap(),
            CanonicalLabel::parse("@@//pkg:fallback").unwrap(),
            CanonicalLabel::parse("@@//pkg:after").unwrap(),
        ]
    );
    let mut chained_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "chained")
        .unwrap()
        .value
        .labels(&mut chained_labels);
    assert_eq!(
        chained_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:before").unwrap(),
            CanonicalLabel::parse("@@//pkg:one").unwrap(),
            CanonicalLabel::parse("@@//pkg:one_default").unwrap(),
            CanonicalLabel::parse("@@//pkg:two").unwrap(),
            CanonicalLabel::parse("@@//pkg:two_default").unwrap(),
            CanonicalLabel::parse("@@//pkg:after").unwrap(),
            CanonicalLabel::parse("@@//pkg:again").unwrap(),
        ]
    );
    let mut nested_prefix_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "nested_prefix")
        .unwrap()
        .value
        .labels(&mut nested_prefix_labels);
    assert_eq!(
        nested_prefix_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:outer").unwrap(),
            CanonicalLabel::parse("@@//pkg:inner").unwrap(),
            CanonicalLabel::parse("@@//pkg:selected").unwrap(),
        ]
    );

    assert!(matches!(
        loaded.targets[1].kind,
        PackageTargetKind::GeneratedFile { ref generating_rule, ref label }
            if generating_rule == "metadata" && label == &CanonicalLabel::parse("@@//pkg:dir/one.out").unwrap()
    ));
    assert_eq!(loaded.targets[1].name, "dir/one.out");
    assert_eq!(loaded.targets[3].name, "three.out");

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"missing_output\")\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("missing value for mandatory attribute 'out'")
    );
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nexports_files([\"one.out\"])\nprobe(name = \"colliding_output\", out = \"one.out\", outs = [])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("target 'one.out' declared more than once")
    );
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"output_select\", out = select({\":condition\": \"one.out\"}), outs = [])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("attribute `out` is not configurable")
    );
    for invalid in ["//other:out", "../out", "bad:name", "@@//pkg:out", "@dep"] {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"probe\")\nprobe(name = \"bad_output\", out = \"{invalid}\", outs = [])\n"),
        )
        .unwrap();
        assert!(
            try_load_package(&workspace, &package)
                .unwrap_err()
                .to_string()
                .contains("output label must name a valid target in this package")
        );
    }
}

#[test]
fn build_and_macro_native_glob_share_the_prepared_package_listing() {
    let workspace = scratch("glob-callable");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(package.join("sub")).unwrap();
    fs::create_dir_all(package.join("subpackage")).unwrap();
    fs::write(package.join("keep.txt"), "keep\n").unwrap();
    fs::write(package.join("skip.txt"), "skip\n").unwrap();
    fs::write(package.join("sub/child.txt"), "child\n").unwrap();
    fs::write(package.join("subpackage/BUILD.bazel"), "# boundary\n").unwrap();
    fs::write(package.join("subpackage/hidden.txt"), "hidden\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_group():\n    native.filegroup(name = \"macro\", srcs = native.glob((\"sub/*.txt\",)))\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_group\")\nfilegroup(name = \"direct\", srcs = glob([\"*.txt\"], exclude = [\"skip.txt\"]))\nfilegroup(name = \"dirs\", srcs = glob([\"*\"], exclude_directories = 0))\nfilegroup(name = \"omitted\", srcs = glob(allow_empty = True))\nmake_group()\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| (target.name.clone(), target.kind.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                "direct".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec![CanonicalLabel::parse("@@//pkg:keep.txt").unwrap()].into(),
                    srcs_explicit: true,
                }
            ),
            (
                "dirs".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec![
                        CanonicalLabel::parse("@@//pkg:BUILD.bazel").unwrap(),
                        CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap(),
                        CanonicalLabel::parse("@@//pkg:keep.txt").unwrap(),
                        CanonicalLabel::parse("@@//pkg:skip.txt").unwrap(),
                        CanonicalLabel::parse("@@//pkg:sub").unwrap()
                    ]
                    .into(),
                    srcs_explicit: true,
                }
            ),
            (
                "omitted".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: Arc::from([]),
                    srcs_explicit: true,
                },
            ),
            (
                "macro".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec![CanonicalLabel::parse("@@//pkg:sub/child.txt").unwrap()].into(),
                    srcs_explicit: true,
                }
            ),
        ]
    );
    assert_eq!(loaded.used_globs.len(), 4);
}

#[test]
fn glob_reports_context_and_allow_empty_type_errors() {
    let workspace = scratch("glob-errors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "filegroup(name = \"bad\", srcs = glob([\"*.txt\"], allow_empty = 5))\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected boolean for argument `allow_empty`, got `5`"));

    fs::write(
        package.join("defs.bzl"),
        "BAD = glob([\"*.txt\"], allow_empty = True)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"BAD\")\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("glob() may only be called while evaluating a BUILD package"));
}

#[test]
fn rule_toolchain_requirements_are_definition_relative_and_not_dependencies() {
    let workspace = scratch("toolchain-resolution-first-platform-loading");
    let defs = workspace.join("defs.bzl");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        &defs,
        r#"ProbeInfo = provider(fields = {"marker": "selected toolchain marker"})

def _demo_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(marker = ctx.attr.marker)]

demo_toolchain_impl = rule(
    implementation = _demo_toolchain_impl,
    attrs = {
        "marker": attr.string(mandatory = True),
    },
)

def _probe_impl(ctx):
    return [ProbeInfo(marker = ctx.toolchains["//:demo_type"].marker)]

probe_rule = rule(
    implementation = _probe_impl,
    toolchains = ["//:demo_type"],
)
"#,
    )
    .unwrap();
    fs::write(
        workspace.join(BUILD_FILE_PRIMARY),
        r#"load(":defs.bzl", "demo_toolchain_impl", "probe_rule")

constraint_setting(name = "selection")

constraint_value(
    name = "first",
    constraint_setting = ":selection",
)

constraint_value(
    name = "second",
    constraint_setting = ":selection",
)

platform(
    name = "first_platform",
    constraint_values = [":first"],
)

platform(
    name = "second_platform",
    constraint_values = [":second"],
)

toolchain_type(name = "demo_type")

demo_toolchain_impl(
    name = "first_impl",
    marker = "first",
)

demo_toolchain_impl(
    name = "second_impl",
    marker = "second",
)

toolchain(
    name = "first_toolchain",
    exec_compatible_with = [":first"],
    toolchain = ":first_impl",
    toolchain_type = ":demo_type",
)

toolchain(
    name = "second_toolchain",
    exec_compatible_with = [":second"],
    toolchain = ":second_impl",
    toolchain_type = ":demo_type",
)

probe_rule(name = "probe")
"#,
    )
    .unwrap();

    let loaded = try_load_package(&workspace, &workspace).unwrap();
    let requirements = |loaded: &slug_loading_v2::LegacyLoadedPackage, name: &str| {
        loaded
            .targets
            .iter()
            .find_map(|target| match &target.kind {
                PackageTargetKind::StarlarkRule(rule) if target.name == name => {
                    Some(rule.required_toolchains().to_vec())
                }
                _ => None,
            })
            .unwrap()
    };
    let dependencies = |loaded: &slug_loading_v2::LegacyLoadedPackage, name: &str| {
        loaded
            .targets
            .iter()
            .find_map(|target| match &target.kind {
                PackageTargetKind::StarlarkRule(rule) if target.name == name => {
                    Some(rule.dependencies().to_vec())
                }
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(
        requirements(&loaded, "probe"),
        [CanonicalLabel::parse("@@//:demo_type").unwrap()]
    );
    assert!(dependencies(&loaded, "probe").is_empty());
    assert!(requirements(&loaded, "first_impl").is_empty());
    assert!(requirements(&loaded, "second_impl").is_empty());
    let probe = loaded
        .targets
        .iter()
        .find(|target| target.name == "probe")
        .unwrap();
    let PackageTargetKind::StarlarkRule(probe) = &probe.kind else {
        panic!("probe must remain a Starlark rule")
    };
    assert_eq!(
        probe.definition_source().label,
        CanonicalLabel::parse("@@//:defs.bzl").unwrap()
    );
    assert!(probe.definition_source().repository_mapping.is_empty());
    assert_eq!(
        resolve_rule_definition_label(":demo_type", probe.definition_source()).unwrap(),
        CanonicalLabel::parse("@@//:demo_type").unwrap()
    );

    let consumer = workspace.join("consumer");
    let empty_defs = workspace.join("empty.bzl");
    fs::write(
        &empty_defs,
        "def _empty(ctx):\n    return [DefaultInfo(files = depset([]))]\nempty_rule = rule(implementation = _empty, toolchains = [])\n",
    )
    .unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::write(
        consumer.join(BUILD_FILE_PRIMARY),
        "load(\"//:defs.bzl\", \"probe_rule\")\nload(\"//:empty.bzl\", \"empty_rule\")\nprobe_rule(name = \"consumer_probe\")\nempty_rule(name = \"empty\")\n",
    )
    .unwrap();
    let consumer =
        try_load_package_with_extra_bzl(&workspace, &consumer, &[defs, empty_defs]).unwrap();
    assert_eq!(
        requirements(&consumer, "consumer_probe"),
        [CanonicalLabel::parse("@@//:demo_type").unwrap()]
    );
    assert!(dependencies(&consumer, "consumer_probe").is_empty());
    assert!(requirements(&consumer, "empty").is_empty());
}

#[test]
fn rule_definition_resolver_uses_external_source_repository_mapping() {
    let source = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@rules+//pkg:defs.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            "/workspace/rules/pkg/defs.bzl".into(),
        ),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("toolchains").unwrap(),
            CanonicalRepoName::new("toolchains+1.0").unwrap(),
        )]),
    };
    assert_eq!(
        resolve_rule_definition_label("@toolchains//cc:type", &source).unwrap(),
        CanonicalLabel::parse("@@toolchains+1.0//cc:type").unwrap()
    );
    assert_eq!(
        resolve_rule_definition_label("//types:local", &source).unwrap(),
        CanonicalLabel::parse("@@rules+//types:local").unwrap()
    );
    assert!(
        resolve_rule_definition_label("@requester_only//cc:type", &source)
            .unwrap_err()
            .to_string()
            .contains("not visible")
    );
}

#[test]
fn rule_toolchains_and_toolchain_info_fail_closed_outside_the_fixture_subset() {
    let cases = [
        ("toolchains = \":demo_type\"", "list"),
        ("toolchains = (\":demo_type\",)", "list"),
        (
            "toolchains = [1]",
            "entries must be Strings, Labels, or toolchain_type values",
        ),
        ("toolchains = [\"@external//:type\"]", "not visible"),
        ("toolchains = [\"...\"]", "direct target label"),
        ("toolchains = [\":all\"]", "direct target label"),
    ];
    for (index, (options, expected)) in cases.into_iter().enumerate() {
        let workspace = scratch(&format!("bad-rule-toolchains-{index}"));
        let package = workspace.join("pkg");
        fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("defs.bzl"),
            format!("def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nbad = rule(implementation = _impl, {options})\n"),
        )
        .unwrap();
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            "load(\":defs.bzl\", \"bad\")\nbad(name = \"subject\")\n",
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }

    let workspace = scratch("toolchain-info-loading");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [platform_common.ToolchainInfo()]\nprobe = rule(implementation = _impl)\nempty = rule(implementation = _impl, toolchains = [])\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\", \"empty\")\nprobe(name = \"frozen\")\nempty(name = \"empty\")\n",
    )
    .unwrap();
    let loaded = try_load_package(&workspace, &package).unwrap();
    assert!(loaded.targets.iter().any(|target| matches!(
        &target.kind,
        PackageTargetKind::StarlarkRule(rule)
            if target.name == "empty" && rule.required_toolchains().is_empty()
    )));

    fs::write(
        package.join("defs.bzl"),
        "TOOLCHAIN_INFO = platform_common.ToolchainInfo()\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unsupported analysis builtin ToolchainInfo"),
        "{error}"
    );
}

#[test]
fn native_rule_attributes_keep_ruleclass_order_overrides_and_removals() {
    let workspace = scratch("native-rule-attributes");
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        concat!(
            "def legacy_macro(name, target_name):\n",
            "    native.filegroup(name = target_name)\n",
        ),
    )
    .unwrap();
    fs::write(
        package.join("BUILD.bazel"),
        concat!(
            "load(\":defs.bzl\", \"legacy_macro\")\n",
            "package(default_deprecation = \"deprecated\", default_testonly = True)\n",
            "licenses([\"notice\"])\n",
            "filegroup(name = \"leaf\")\n",
            "filegroup(name = \"files\", data = [\":leaf\"], output_group = \"group\", output_licenses = [\"z\", \"a\"], tags = [\"z\", \"a\"])\n",
            "filegroup(name = \"selected\", srcs = select({\":condition\": [\":leaf\"], \"//conditions:default\": []}))\n",
            "alias(name = \"redirect\", actual = \":leaf\")\n",
            "config_setting(name = \"setting\", values = {\"mode\": \"fast\"}, define_values = {\"feature\": \"on\"})\n",
            "config_setting(name = \"define_only\", define_values = {\"feature\": \"on\"})\n",
            "platform(name = \"platform\")\n",
            "toolchain_type(name = \"type\")\n",
            "toolchain(name = \"chain\", toolchain_type = \":type\", toolchain = \":leaf\")\n",
            "legacy_macro(name = \"macro_case\", target_name = \"macro_files\")\n",
        ),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let names = |target: &str| {
        loaded
            .native_attributes(target)
            .unwrap()
            .iter()
            .map(|(schema, _)| schema.query_name())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names("files"),
        [
            "name",
            "visibility",
            "transitive_configs",
            "deprecation",
            "tags",
            "generator_name",
            "generator_function",
            "generator_location",
            "testonly",
            "features",
            ":action_listener",
            "compatible_with",
            "restricted_to",
            "$config_dependencies",
            "package_metadata",
            "aspect_hints",
            "licenses",
            "distribs",
            "target_compatible_with",
            "srcs",
            "output_group",
            "data",
            "output_licenses",
        ]
    );
    let files = loaded.native_attributes("files").unwrap();
    assert!(
        matches!(&files.get("deprecation").unwrap().1.value, CoercedAttributeValue::String(value) if value == "deprecated")
    );
    assert!(
        matches!(&files.get("tags").unwrap().1.value, CoercedAttributeValue::StringList(values) if values.as_ref() == ["a", "z"])
    );
    assert!(
        matches!(&files.get("output_group").unwrap().1.value, CoercedAttributeValue::String(value) if value == "group")
    );
    assert!(
        matches!(&files.get("data").unwrap().1.value, CoercedAttributeValue::LabelList(values) if values.as_ref() == [CanonicalLabel::parse("@@//pkg:leaf").unwrap()])
    );
    assert!(
        matches!(&files.get("output_licenses").unwrap().1.value, CoercedAttributeValue::StringList(values) if values.as_ref() == ["z", "a"])
    );
    let alias_names = names("redirect");
    assert!(!alias_names.contains(&"licenses"));
    assert!(!alias_names.contains(&"distribs"));
    assert!(!alias_names.contains(&":action_listener"));
    let setting = loaded.native_attributes("setting").unwrap();
    assert_eq!(setting.iter().len(), 21);
    assert_eq!(
        setting
            .iter()
            .filter(|(schema, _)| schema.query_name() == "licenses")
            .count(),
        1
    );
    assert!(
        matches!(&setting.get("define_values").unwrap().1.value, CoercedAttributeValue::StringDict(values) if values.as_ref() == [("feature".into(), "on".into())])
    );
    assert_eq!(
        setting.get("define_values").unwrap().1.provenance,
        AttributeProvenance::Explicit
    );
    assert_eq!(
        setting.get("flag_values").unwrap().1.provenance,
        AttributeProvenance::Default
    );
    assert_eq!(
        setting.get("constraint_values").unwrap().1.provenance,
        AttributeProvenance::Default
    );
    assert!(setting.get("compatible_with").is_none());
    assert!(
        matches!(&loaded.native_attributes("define_only").unwrap().get("values").unwrap().1.value, CoercedAttributeValue::StringDict(values) if values.is_empty())
    );
    let selected = loaded.native_attributes("selected").unwrap();
    let selected_srcs = selected.get("srcs").unwrap().1;
    let CoercedAttributeValue::Selector { branches, default } = &selected_srcs.value else {
        panic!("selected srcs should retain its selector");
    };
    assert_eq!(branches.len(), 1);
    assert_eq!(
        branches[0].0,
        CanonicalLabel::parse("@@//pkg:condition").unwrap()
    );
    assert!(
        matches!(branches[0].1.as_ref(), CoercedAttributeValue::LabelList(values) if values.as_ref() == [CanonicalLabel::parse("@@//pkg:leaf").unwrap()])
    );
    assert!(
        matches!(default.as_deref(), Some(CoercedAttributeValue::LabelList(values)) if values.is_empty())
    );
    assert!(
        matches!(&selected.get("$config_dependencies").unwrap().1.value, CoercedAttributeValue::LabelList(values) if values.as_ref() == [CanonicalLabel::parse("@@//pkg:condition").unwrap()])
    );
    let PackageTargetKind::Filegroup { srcs, .. } = &loaded
        .targets
        .iter()
        .find(|target| target.name == "selected")
        .unwrap()
        .kind
    else {
        panic!("selected should be a filegroup");
    };
    assert_eq!(
        srcs.as_ref(),
        [CanonicalLabel::parse("@@//pkg:leaf").unwrap()]
    );
    assert!(
        matches!(&loaded.native_attributes("platform").unwrap().get("constraint_values").unwrap().1.value, CoercedAttributeValue::LabelList(values) if values.is_empty())
    );
    assert!(
        matches!(&loaded.native_attributes("chain").unwrap().get("exec_compatible_with").unwrap().1.value, CoercedAttributeValue::LabelList(values) if values.is_empty())
    );
    let direct = loaded.native_attributes("files").unwrap();
    assert!(
        matches!(&direct.get("generator_name").unwrap().1.value, CoercedAttributeValue::String(value) if value.is_empty())
    );
    let generated = loaded.native_attributes("macro_files").unwrap();
    assert!(
        matches!(&generated.get("generator_name").unwrap().1.value, CoercedAttributeValue::String(value) if value == "macro_case")
    );
    assert!(
        matches!(&generated.get("generator_function").unwrap().1.value, CoercedAttributeValue::String(value) if value == "legacy_macro")
    );
    assert!(
        matches!(&generated.get("generator_location").unwrap().1.value, CoercedAttributeValue::String(value) if value.starts_with("pkg/BUILD.bazel:") && !value.ends_with(":0"))
    );
}

#[test]
fn bzl_visibility_allows_same_package_and_rejects_build_cross_package_before_evaluation() {
    let workspace = scratch("bzl-load-visibility");
    fs::write(workspace.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
    let owner = workspace.join("owner");
    let consumer = workspace.join("consumer");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    let defs = owner.join("defs.bzl");
    fs::write(&defs, "visibility(\"private\")\nEXPORTED = 1\n").unwrap();
    fs::write(
        owner.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"EXPORTED\")\nfilegroup(name = \"same_package\")\n",
    )
    .unwrap();
    assert!(
        load_package(&workspace, &owner)
            .targets
            .iter()
            .any(|target| target.name == "same_package")
    );

    fs::write(
        consumer.join("BUILD.bazel"),
        concat!(
            "load(\"//owner:defs.bzl\", \"EXPORTED\")\n",
            "fail(\"BUILD evaluation must not start after a denied load\")\n",
        ),
    )
    .unwrap();
    let error = try_load_package_with_extra_bzl(&workspace, &consumer, &[defs])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("@@//owner:defs.bzl is not visible"),
        "{error}"
    );
    assert!(error.contains("package @@//consumer"), "{error}");
    assert!(
        !error.contains("BUILD evaluation must not start"),
        "{error}"
    );
}

#[test]
fn imported_native_genrule_retains_selected_declaration_and_generated_outputs() {
    let workspace = scratch("imported-native-genrule");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let defs = workspace.join("defs");
    let consumer = workspace.join("consumer");
    fs::create_dir_all(&defs).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    let helper = defs.join("helper.bzl");
    fs::write(
        &helper,
        concat!(
            "TYPED = Label(':typed.txt')\n",
            "def make_genrule(name, index):\n",
            "    native.filegroup(name = name + '_pre')\n",
            "    native.genrule(name = name, srcs = ['raw.txt', TYPED], toolchains = [TYPED], cmd = 'cp ' + str(index), outs = ['nested/' + name + '.out'], tags = ['z', 'manual', 'a'])\n",
            "def omitted(name): native.genrule(name = name, cmd = 'true', outs = [name + '.out'])\n",
            "def empty(name): native.genrule(name = name, cmd = 'true', outs = [name + '.out'], srcs = [], toolchains = [], tags = [])\n",
        ),
    )
    .unwrap();
    fs::write(
        consumer.join(BUILD_FILE_PRIMARY),
        concat!(
            "load('//defs:helper.bzl', 'make_genrule', 'omitted', 'empty')\n",
            "make_genrule(name = 'generated0', index = 0)\n",
            "make_genrule(name = 'generated1', index = 1)\n",
            "make_genrule(name = 'generated2', index = 2)\n",
            "make_genrule(name = 'generated3', index = 3)\n",
            "make_genrule(name = 'generated4', index = 4)\n",
            "make_genrule(name = 'generated5', index = 5)\n",
            "omitted(name = 'omitted')\nempty(name = 'empty')\n",
        ),
    )
    .unwrap();
    let loaded = try_load_package_with_extra_bzl(&workspace, &consumer, &[helper]).unwrap();
    let rules = loaded
        .targets
        .iter()
        .filter_map(|target| match &target.kind {
            PackageTargetKind::Genrule(rule) => Some((target, rule)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rules.len(), 8);
    let (target, selected) = rules[0];
    assert_eq!(
        selected.srcs.as_deref().unwrap(),
        [
            CanonicalLabel::parse("@@//consumer:raw.txt").unwrap(),
            CanonicalLabel::parse("@@//defs:typed.txt").unwrap(),
        ]
    );
    assert_eq!(
        selected.toolchains.as_deref().unwrap()[0].to_string(),
        "@@//defs:typed.txt"
    );
    assert_eq!(
        selected.outs[0].to_string(),
        "@@//consumer:nested/generated0.out"
    );
    assert_eq!(selected.tags.as_deref().unwrap(), ["a", "manual", "z"]);
    let generator = selected.generator.as_ref().unwrap();
    assert_eq!(
        (generator.0.as_str(), generator.1.as_str()),
        ("generated0", "make_genrule")
    );
    assert!(generator.2.starts_with("consumer/BUILD.bazel:2:"));
    let capability = target.rule_capability().unwrap();
    assert_eq!(
        (capability.rule_class.as_str(), capability.executable),
        ("genrule", false)
    );
    assert!(
        rules[6].1.srcs.is_none() && rules[6].1.toolchains.is_none() && rules[6].1.tags.is_none()
    );
    assert!(
        rules[7].1.srcs.as_ref().unwrap().is_empty()
            && rules[7].1.toolchains.as_ref().unwrap().is_empty()
            && rules[7].1.tags.as_ref().unwrap().is_empty()
    );
    let output = loaded
        .targets
        .iter()
        .find(|target| target.name == "nested/generated0.out")
        .unwrap();
    assert!(
        matches!(&output.kind, PackageTargetKind::GeneratedFile { generating_rule, .. } if generating_rule == "generated0")
    );
    assert_eq!(
        loaded.effective_visibility(output),
        Some(RuleVisibility::Private)
    );
}

fn starlark_rule_attr<'a>(
    loaded: &'a slug_loading_v2::LegacyLoadedPackage,
    target: &str,
    attribute: &str,
) -> &'a slug_loading_v2::AttributeValue {
    let PackageTargetKind::StarlarkRule(rule) = &loaded
        .targets
        .iter()
        .find(|candidate| candidate.name == target)
        .unwrap()
        .kind
    else {
        panic!("{target} was not recorded as a Starlark rule")
    };
    rule.values()
        .iter()
        .find(|value| value.declaration_name == attribute)
        .unwrap()
}

#[test]
fn empty_private_aspect_dependency_loads_without_propagation() {
    let workspace = scratch("empty-private-aspect-dependency");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _aspect_impl(target, ctx):
    return []

graph_structure_aspect = aspect(
    implementation = _aspect_impl,
    attr_aspects = ["*"],
)

def _impl(ctx):
    return []

def _init(name, **kwargs):
    if "dynamic_deps" in kwargs and len(kwargs["dynamic_deps"]) != 0:
        return kwargs | {"_deps_analyzed_by_graph_structure_aspect": kwargs["dynamic_deps"]}
    return kwargs | {"marker": "initializer-called"}

cc_binary = rule(
    implementation = _impl,
    initializer = _init,
    attrs = {
        "dynamic_deps": attr.label_list(),
        "_deps_analyzed_by_graph_structure_aspect": attr.label_list(aspects = [graph_structure_aspect]),
        "targets": attr.label_list(aspects = [graph_structure_aspect]),
        "marker": attr.string(),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'cc_binary')\ncc_binary(name = 'subject')\ncc_binary(name = 'explicit_empty', targets = [])\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "subject")
        .unwrap()
        .kind
    else {
        panic!("subject was not recorded as a Starlark rule")
    };
    let aspect_attribute = "_deps_analyzed_by_graph_structure_aspect";
    assert_eq!(
        rule.schema()
            .iter()
            .find(|schema| schema.declaration_name() == aspect_attribute)
            .unwrap()
            .query_name(),
        "$deps_analyzed_by_graph_structure_aspect"
    );
    let value = starlark_rule_attr(&loaded, "subject", aspect_attribute);
    assert_eq!(value.provenance, AttributeProvenance::Implicit);
    assert!(matches!(
        value.value.as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.is_empty()
    ));
    assert!(matches!(
        starlark_rule_attr(&loaded, "subject", "$config_dependencies")
            .value
            .as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.is_empty()
    ));
    assert!(rule.dependencies().is_empty());
    assert!(rule.attached_aspects().is_empty());
    let marker = starlark_rule_attr(&loaded, "subject", "marker");
    assert_eq!(marker.provenance, AttributeProvenance::Explicit);
    assert_eq!(
        marker.value.as_ref(),
        &CoercedAttributeValue::String("initializer-called".into())
    );
    let explicit = starlark_rule_attr(&loaded, "explicit_empty", "targets");
    assert_eq!(explicit.provenance, AttributeProvenance::Explicit);
    assert!(matches!(
        explicit.value.as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.is_empty()
    ));
}

#[test]
fn nonempty_cc_proto_aspect_dependency_retains_canonical_base_edge() {
    let workspace = scratch("nonempty-cc-proto-aspect-dependency");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
ProtoInfo = provider()
CcInfo = provider()

def _proto_impl(ctx):
    fail("proto implementation must stay lazy")

proto_library = rule(implementation = _proto_impl, provides = [ProtoInfo])

def _aspect_impl(target, ctx):
    fail("aspect implementation must stay lazy")

cc_proto_aspect = aspect(
    implementation = _aspect_impl,
    attr_aspects = ["deps"],
    fragments = ["cpp", "proto"],
    required_providers = [ProtoInfo],
    provides = [CcInfo],
)

def _cc_proto_impl(ctx):
    fail("cc_proto implementation must stay lazy")

cc_proto_library = rule(
    implementation = _cc_proto_impl,
    attrs = {
        "deps": attr.label_list(
            aspects = [cc_proto_aspect],
            providers = [ProtoInfo],
            allow_files = False,
        ),
    },
    provides = [CcInfo],
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'proto_library', 'cc_proto_library')\nproto_library(name = 'proto')\ncc_proto_library(name = 'subject', deps = [':proto'])\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded
        .targets
        .iter()
        .find(|target| target.name == "subject")
        .unwrap()
        .kind
    else {
        panic!("subject was not recorded as a Starlark rule")
    };
    let dependency = CanonicalLabel::parse("@@//pkg:proto").unwrap();
    assert_eq!(rule.dependencies(), [dependency.clone()]);
    let schema_index = rule
        .schema()
        .iter()
        .position(|schema| schema.declaration_name() == "deps")
        .unwrap();
    let schema = &rule.schema()[schema_index];
    assert_eq!(schema.kind(), AttributeKind::LabelList);
    assert_eq!(schema.required_providers().len(), 1);
    assert_eq!(schema.required_providers()[0].len(), 1);
    let required_provider = schema.required_providers()[0][0].user_id().unwrap();
    assert_eq!(required_provider.source_label(), "//pkg:defs.bzl");
    assert_eq!(required_provider.exported_name(), "ProtoInfo");
    let value = starlark_rule_attr(&loaded, "subject", "deps");
    assert_eq!(value.provenance, AttributeProvenance::Explicit);
    assert!(matches!(
        value.value.as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.as_ref() == [dependency]
    ));
    let [attachment] = rule.attached_aspects() else {
        panic!("expected exactly one retained attached aspect")
    };
    assert_eq!(attachment.schema_index() as usize, schema_index);
    assert_eq!(
        attachment.defining_label(),
        &CanonicalLabel::parse("@@//pkg:defs.bzl").unwrap()
    );
    assert_eq!(attachment.exported_name(), "cc_proto_aspect");
}

#[test]
fn imported_nonempty_aspect_attachment_restores_on_same_dice() {
    let workspace = scratch("imported-nonempty-aspect-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
load(':aspects.bzl', 'ProtoInfo', 'ACTIVE')

def _proto_impl(ctx):
    fail("proto implementation must stay lazy")

proto_library = rule(implementation = _proto_impl, provides = [ProtoInfo])

def _cc_proto_impl(ctx):
    fail("cc proto implementation must stay lazy")

cc_proto_library = rule(
    implementation = _cc_proto_impl,
    attrs = {"deps": attr.label_list(aspects = [ACTIVE], providers = [ProtoInfo], allow_files = False)},
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'proto_library', 'cc_proto_library')\nproto_library(name = 'proto')\ncc_proto_library(name = 'subject', deps = [':proto'])\n",
    )
    .unwrap();
    let aspect_source = package.join("aspects.bzl");
    let write_aspect = |selected: &str| {
        fs::write(
            &aspect_source,
            format!(
                "ProtoInfo = provider()\ndef _aspect_impl(target, ctx): fail('aspect implementation must stay lazy')\nFIRST = aspect(implementation = _aspect_impl, attr_aspects = ['deps'], required_providers = [ProtoInfo])\nSECOND = aspect(implementation = _aspect_impl, attr_aspects = ['deps'], required_providers = [ProtoInfo])\nACTIVE = {selected}\n"
            ),
        )
        .unwrap();
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = || runtime.block_on(load_package_with_retained_dice(&dice, &workspace, &package));

    write_aspect("FIRST");
    let first = load().unwrap();
    write_aspect("SECOND");
    let changed = load().unwrap();
    let attached_name = |loaded: &slug_loading_v2::LegacyLoadedPackage| {
        let PackageTargetKind::StarlarkRule(rule) = &loaded
            .targets
            .iter()
            .find(|target| target.name == "subject")
            .unwrap()
            .kind
        else {
            panic!("subject was not recorded as a Starlark rule")
        };
        let [attachment] = rule.attached_aspects() else {
            panic!("expected exactly one retained attached aspect")
        };
        assert_eq!(
            attachment.defining_label(),
            &CanonicalLabel::parse("@@//pkg:aspects.bzl").unwrap()
        );
        assert_eq!(
            rule.schema()[attachment.schema_index() as usize].declaration_name(),
            "deps"
        );
        attachment.exported_name().to_owned()
    };
    assert_eq!(attached_name(&first), "FIRST");
    assert_eq!(attached_name(&changed), "SECOND");
    assert_ne!(first.load_fingerprint, changed.load_fingerprint);
    assert_ne!(first, changed);
    assert_eq!(
        starlark_rule_attr(&first, "subject", "deps"),
        starlark_rule_attr(&changed, "subject", "deps")
    );
    write_aspect("FIRST");
    let restored = load().unwrap();
    assert_eq!(attached_name(&restored), "FIRST");
    assert_eq!(first, restored);
}

#[test]
fn aspect_dependency_rejects_selector_and_parameterized_values() {
    let workspace = scratch("aspect-dependency-rejections");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let definitions = package.join("defs.bzl");
    let build = package.join(BUILD_FILE_PRIMARY);
    for (case, aspect_attrs, rule_attrs, invocation, expected_error) in [
        (
            "empty-selector",
            "",
            "{'targets': attr.label_list(aspects = [A])}",
            "probe(name = 'blocked', targets = select({'//conditions:default': []}))",
            "target invocation for aspect-bearing attribute",
        ),
        (
            "nonempty-selector",
            "",
            "{'targets': attr.label_list(aspects = [A])}",
            "probe(name = 'blocked', targets = select({'//conditions:default': [':dep']}))",
            "target invocation for aspect-bearing attribute",
        ),
        (
            "selected-concatenation",
            "",
            "{'targets': attr.label_list(aspects = [A])}",
            "probe(name = 'blocked', targets = select({'//conditions:default': [':dep']}) + [':other'])",
            "target invocation for aspect-bearing attribute",
        ),
        (
            "parameterized-empty-default",
            "attrs = {'mode': attr.string(mandatory = True)},",
            "{'_deps_analyzed_by_graph_structure_aspect': attr.label_list(aspects = [A])}",
            "probe(name = 'blocked')",
            "target invocation for aspect-bearing attribute",
        ),
        (
            "multiple-nonempty-aspect-attrs",
            "",
            "{'first': attr.label_list(aspects = [A]), 'second': attr.label_list(aspects = [A])}",
            "probe(name = 'blocked', first = [':earlier'], second = [':earlier'])",
            "target invocation with multiple attached aspects is not supported",
        ),
    ] {
        fs::write(
            &definitions,
            format!(
                "def _aspect_impl(target, ctx): return []\nA = aspect(implementation = _aspect_impl, {aspect_attrs})\ndef _impl(ctx): return []\nprobe = rule(implementation = _impl, attrs = {rule_attrs})\n"
            ),
        )
        .unwrap();
        fs::write(
            &build,
            format!("load(':defs.bzl', 'probe')\nfilegroup(name = 'earlier')\n{invocation}\n"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected_error), "{case}: {error}");
    }
}

#[test]
fn empty_aspect_dependency_restores_after_nonempty_source_on_same_dice() {
    let workspace = scratch("empty-aspect-dependency-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'probe')\nprobe(name = 'subject')\n",
    )
    .unwrap();
    let definitions = package.join("defs.bzl");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |default: &str| {
        fs::write(
            &definitions,
            format!(
                "def _aspect_impl(target, ctx): return []\nA = aspect(implementation = _aspect_impl)\ndef _impl(ctx): return []\nprobe = rule(implementation = _impl, attrs = {{'_deps_analyzed_by_graph_structure_aspect': attr.label_list(default = {default}, aspects = [A])}})\n"
            ),
        )
        .unwrap();
        runtime.block_on(load_package_with_retained_dice(&dice, &workspace, &package))
    };
    let first = load("[]").unwrap();
    let changed = load("[':dep']").unwrap();
    let PackageTargetKind::StarlarkRule(rule) = &changed
        .targets
        .iter()
        .find(|target| target.name == "subject")
        .unwrap()
        .kind
    else {
        panic!("subject was not recorded as a Starlark rule")
    };
    let dependency = CanonicalLabel::parse("@@//pkg:dep").unwrap();
    assert_eq!(rule.dependencies(), [dependency.clone()]);
    assert_eq!(rule.attached_aspects().len(), 1);
    assert_eq!(rule.attached_aspects()[0].exported_name(), "A");
    assert!(matches!(
        starlark_rule_attr(&changed, "subject", "_deps_analyzed_by_graph_structure_aspect")
            .value
            .as_ref(),
        CoercedAttributeValue::LabelList(labels) if labels.as_ref() == [dependency]
    ));
    assert_ne!(first.load_fingerprint, changed.load_fingerprint);
    assert_ne!(first, changed);
    let restored = load("[]").unwrap();
    assert_eq!(
        starlark_rule_attr(
            &first,
            "subject",
            "_deps_analyzed_by_graph_structure_aspect"
        ),
        starlark_rule_attr(
            &restored,
            "subject",
            "_deps_analyzed_by_graph_structure_aspect"
        )
    );
    assert_eq!(first, restored);
}

#[test]
fn starlark_rule_license_wrapper_accepts_explicit_zero_testonly() {
    let workspace = scratch("starlark-rule-license-zero-testonly");
    let package = workspace.join("licenses");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("license.bzl"),
        r#"
def _license_impl(ctx):
    return []

_license = rule(implementation = _license_impl)

def license(name):
    _license(name = name, testonly = 0)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':license.bzl', 'license')\npackage(default_testonly = True)\nlicense(name = 'subject')\n",
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let testonly = starlark_rule_attr(&loaded, "subject", "testonly");
    assert_eq!(testonly.provenance, AttributeProvenance::Explicit);
    assert_eq!(
        testonly.value.as_ref(),
        &CoercedAttributeValue::Boolean(false)
    );
}

#[test]
fn starlark_rule_boolean_calls_accept_direct_values_and_reject_other_contexts() {
    let workspace = scratch("starlark-rule-boolean-direct");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let defs = package.join("defs.bzl");
    let build = package.join(BUILD_FILE_PRIMARY);
    fs::write(
        &defs,
        "def _impl(ctx): return []\nprobe = rule(implementation = _impl, attrs = {'flag': attr.bool(default = False)})\n",
    )
    .unwrap();
    fs::write(
        &build,
        concat!(
            "load(':defs.bzl', 'probe')\n",
            "probe(name = 'zero', flag = 0, testonly = False)\n",
            "probe(name = 'one', flag = 1, testonly = True)\n",
            "probe(name = 'bool_false', flag = False)\n",
            "probe(name = 'bool_true', flag = True)\n",
            "probe(name = 'omitted')\n",
        ),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    for (target, expected) in [
        ("zero", false),
        ("one", true),
        ("bool_false", false),
        ("bool_true", true),
    ] {
        let flag = starlark_rule_attr(&loaded, target, "flag");
        assert_eq!(flag.provenance, AttributeProvenance::Explicit);
        assert_eq!(
            flag.value.as_ref(),
            &CoercedAttributeValue::Boolean(expected)
        );
    }
    for (target, expected) in [("zero", false), ("one", true)] {
        let testonly = starlark_rule_attr(&loaded, target, "testonly");
        assert_eq!(testonly.provenance, AttributeProvenance::Explicit);
        assert_eq!(
            testonly.value.as_ref(),
            &CoercedAttributeValue::Boolean(expected)
        );
    }
    let omitted = starlark_rule_attr(&loaded, "omitted", "flag");
    assert_eq!(omitted.provenance, AttributeProvenance::Default);
    assert_eq!(
        omitted.value.as_ref(),
        &CoercedAttributeValue::Boolean(false)
    );
    for attribute in ["flag", "testonly"] {
        for bad in ["2", "-1", "'1'"] {
            fs::write(
                &build,
                format!("load(':defs.bzl', 'probe')\nprobe(name='bad', {attribute}={bad})\n"),
            )
            .unwrap();
            let error = try_load_package(&workspace, &package)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("attribute must be a bool"),
                "{attribute}={bad}: {error}"
            );
        }
    }
    fs::write(
        &build,
        "load(':defs.bzl', 'probe')\nprobe(name='bad', flag=select({'//conditions:default': 1}))\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("attribute must be a bool"), "{error}");

    // The constructor default fails during .bzl evaluation, with no rule call.
    fs::write(&build, "load(':defs.bzl', 'probe')\n").unwrap();
    fs::write(
        &defs,
        "def _impl(ctx): return []\nprobe = rule(implementation = _impl, attrs = {'flag': attr.bool(default = 0)})\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("attribute must be a bool"), "{error}");
}

#[test]
fn starlark_rule_boolean_direct_values_restore_on_same_dice() {
    let workspace = scratch("starlark-rule-boolean-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx): return []\nprobe = rule(implementation = _impl, attrs = {'flag': attr.bool(default = False)})\n",
    )
    .unwrap();
    let build = package.join(BUILD_FILE_PRIMARY);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |flag: i32, testonly: i32| {
        fs::write(
            &build,
            format!("load(':defs.bzl', 'probe')\nprobe(name='subject', flag={flag}, testonly={testonly})\n"),
        )
        .unwrap();
        runtime
            .block_on(load_package_with_retained_dice(&dice, &workspace, &package))
            .unwrap()
    };
    let a = load(0, 0);
    let flag_changed = load(1, 0);
    let testonly_changed = load(0, 1);
    let restored = load(0, 0);
    let pair = |loaded: &slug_loading_v2::LegacyLoadedPackage| {
        ["flag", "testonly"].map(|name| starlark_rule_attr(loaded, "subject", name).clone())
    };
    assert_eq!(
        pair(&a).map(|value| value.value.as_ref().clone()),
        [
            CoercedAttributeValue::Boolean(false),
            CoercedAttributeValue::Boolean(false),
        ]
    );
    assert_ne!(pair(&a), pair(&flag_changed));
    assert_ne!(pair(&a), pair(&testonly_changed));
    assert_eq!(pair(&a), pair(&restored));
    assert_ne!(a, flag_changed);
    assert_ne!(a, testonly_changed);
    assert_eq!(a, restored);
}

#[test]
fn initializer_preserves_imported_frozen_string_list_selectors() {
    let workspace = scratch("initializer-frozen-string-list-selectors");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
COPTS = select({
    ":msvc": ["/wd4065", "/wd4146"],
    "//conditions:default": ["-Wno-sign-compare"],
})
LINKOPTS = select({
    ":android": [],
    ":darwin": ["-framework", "CoreFoundation"],
    "//conditions:default": ["-lpthread", "-lm"],
})

def _impl(ctx):
    return []

def _init(name, **kwargs):
    if type(kwargs["copts"]) != "select" or type(kwargs["linkopts"]) != "select":
        fail("initializer lost unresolved selectors")
    if type(kwargs["direct"]) != "list" or "nullable" in kwargs or "omitted" in kwargs:
        fail("initializer argument filtering changed")
    return {"copts": kwargs["copts"], "linkopts": kwargs["linkopts"], "direct": kwargs["direct"], "marker": "called"}

probe = rule(implementation = _impl, initializer = _init, attrs = {
    "copts": attr.string_list(),
    "linkopts": attr.string_list(),
    "direct": attr.string_list(),
    "nullable": attr.string_list(),
    "omitted": attr.string_list(),
    "marker": attr.string(),
})
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        concat!(
            "load(':defs.bzl', 'COPTS', 'LINKOPTS', 'probe')\n",
            "config_setting(name = 'msvc', values = {'cpu': 'x64_windows'})\n",
            "config_setting(name = 'android', values = {'cpu': 'armeabi-v7a'})\n",
            "config_setting(name = 'darwin', values = {'cpu': 'darwin'})\n",
            "probe(name = 'subject', copts = COPTS, linkopts = LINKOPTS, direct = ['direct'], nullable = None)\n",
        ),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let marker = starlark_rule_attr(&loaded, "subject", "marker");
    assert_eq!(marker.provenance, AttributeProvenance::Explicit);
    assert_eq!(
        marker.value.as_ref(),
        &CoercedAttributeValue::String("called".into())
    );
    let copts = starlark_rule_attr(&loaded, "subject", "copts");
    assert_eq!(copts.provenance, AttributeProvenance::Explicit);
    let CoercedAttributeValue::Selector { branches, default } = copts.value.as_ref() else {
        panic!("initializer lost COPTS selector")
    };
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].0.to_string(), "@@//pkg:msvc");
    assert_eq!(
        branches[0].1.as_ref(),
        &CoercedAttributeValue::StringList(Arc::from(["/wd4065".into(), "/wd4146".into()]))
    );
    assert_eq!(
        default.as_deref(),
        Some(&CoercedAttributeValue::StringList(Arc::from([
            "-Wno-sign-compare".into()
        ])))
    );
    let linkopts = starlark_rule_attr(&loaded, "subject", "linkopts");
    assert_eq!(linkopts.provenance, AttributeProvenance::Explicit);
    let CoercedAttributeValue::Selector { branches, default } = linkopts.value.as_ref() else {
        panic!("initializer lost LINKOPTS selector")
    };
    assert_eq!(
        branches
            .iter()
            .map(|(condition, _)| condition.to_string())
            .collect::<Vec<_>>(),
        ["@@//pkg:android", "@@//pkg:darwin"]
    );
    assert_eq!(
        branches[0].1.as_ref(),
        &CoercedAttributeValue::StringList(Arc::from([]))
    );
    assert_eq!(
        branches[1].1.as_ref(),
        &CoercedAttributeValue::StringList(Arc::from([
            "-framework".into(),
            "CoreFoundation".into()
        ]))
    );
    assert_eq!(
        default.as_deref(),
        Some(&CoercedAttributeValue::StringList(Arc::from([
            "-lpthread".into(),
            "-lm".into()
        ])))
    );
    assert_eq!(
        starlark_rule_attr(&loaded, "subject", "$config_dependencies")
            .value
            .as_ref(),
        &CoercedAttributeValue::LabelList(Arc::from([
            CanonicalLabel::parse("@@//pkg:msvc").unwrap(),
            CanonicalLabel::parse("@@//pkg:android").unwrap(),
            CanonicalLabel::parse("@@//pkg:darwin").unwrap(),
        ]))
    );
    let direct = starlark_rule_attr(&loaded, "subject", "direct");
    assert_eq!(direct.provenance, AttributeProvenance::Explicit);
    assert_eq!(
        direct.value.as_ref(),
        &CoercedAttributeValue::StringList(Arc::from(["direct".into()]))
    );
    for name in ["nullable", "omitted"] {
        let value = starlark_rule_attr(&loaded, "subject", name);
        assert_eq!(value.provenance, AttributeProvenance::Default);
        assert_eq!(
            value.value.as_ref(),
            &CoercedAttributeValue::StringList(Arc::from([]))
        );
    }
}

#[test]
fn initializer_selector_passthrough_rejects_changed_and_invalid_shapes() {
    let workspace = scratch("initializer-selector-rejections");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let defs = package.join("defs.bzl");
    let build = package.join(BUILD_FILE_PRIMARY);
    fs::write(
        &build,
        "load(':defs.bzl', 'COPTS', 'LINKOPTS', 'probe')\nfilegroup(name='earlier')\nprobe(name='bad', copts=COPTS, linkopts=LINKOPTS)\n",
    )
    .unwrap();
    for (copts, copts_decl, result, expected) in [
        (
            "select({':msvc': ['-DMSVC'], '//conditions:default': ['-DDEFAULT']})",
            "attr.string_list()",
            "return {'copts': select({'//conditions:default': ['-DNEW']})}",
            "initializer result",
        ),
        (
            "select({':msvc': ['-DMSVC'], '//conditions:default': ['-DDEFAULT']})",
            "attr.string_list()",
            "return {'copts': kwargs['linkopts']}",
            "initializer result",
        ),
        (
            "select({':msvc': ['-DMSVC'], '//conditions:default': ['-DDEFAULT']})",
            "attr.string_list()",
            "return {'copts': kwargs['copts'] + ['-DTAIL']}",
            "initializer result",
        ),
        (
            "select({':msvc': ['-DMSVC'], '//conditions:default': ['-DDEFAULT']})",
            // This declaration is rejected before the initializer is called;
            // it does not exercise the selector pass-through guard.
            "attr.string_list(configurable = False)",
            "return kwargs",
            "has the 'configurable' argument set, which is not allowed in rule definitions",
        ),
        (
            "select({':msvc': [1], '//conditions:default': ['-DDEFAULT']})",
            "attr.string_list()",
            "return kwargs",
            "must be a string",
        ),
    ] {
        fs::write(
            &defs,
            format!(
                "COPTS = {copts}\nLINKOPTS = select({{'//conditions:default': ['-lpthread']}})\ndef _impl(ctx): return []\ndef _init(name, **kwargs): {result}\nprobe = rule(implementation=_impl, initializer=_init, attrs={{'copts': {copts_decl}, 'linkopts': attr.string_list()}})\n"
            ),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{result}: {error}");
    }
    fs::write(
        &defs,
        "def _impl(ctx): return []\ndef _init(name, **kwargs): return kwargs\nprobe = rule(implementation=_impl, initializer=_init, attrs={'dep': attr.label()})\n",
    )
    .unwrap();
    fs::write(
        &build,
        "load(':defs.bzl', 'probe')\nprobe(name='bad', dep=select({'//conditions:default': ':dep'}))\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("initializer input 'dep' cannot use select()"),
        "{error}"
    );
}

#[test]
fn initializer_imported_selector_source_restores_on_same_dice() {
    let workspace = scratch("initializer-selector-source-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl', 'COPTS', 'probe')\nconfig_setting(name='msvc', values={'cpu':'x64_windows'})\nconfig_setting(name='darwin', values={'cpu':'darwin'})\nprobe(name='subject', copts=COPTS)\n",
    )
    .unwrap();
    let defs = package.join("defs.bzl");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |condition: &str, branch: &str| {
        fs::write(
            &defs,
            format!(
                "COPTS = select({{':{condition}': ['{branch}'], '//conditions:default': ['-DDEFAULT']}})\ndef _impl(ctx): return []\ndef _init(name, **kwargs): return kwargs\nprobe = rule(implementation=_impl, initializer=_init, attrs={{'copts': attr.string_list()}})\n"
            ),
        )
        .unwrap();
        runtime
            .block_on(load_package_with_retained_dice(&dice, &workspace, &package))
            .unwrap()
    };
    let a = load("msvc", "-DMSVC");
    let b = load("darwin", "-DDARWIN");
    let restored = load("msvc", "-DMSVC");
    let value = |loaded: &slug_loading_v2::LegacyLoadedPackage, name: &str| {
        starlark_rule_attr(loaded, "subject", name).clone()
    };
    assert_ne!(value(&a, "copts"), value(&b, "copts"));
    assert_eq!(value(&a, "copts"), value(&restored, "copts"));
    assert_ne!(
        value(&a, "$config_dependencies"),
        value(&b, "$config_dependencies")
    );
    assert_eq!(
        value(&a, "$config_dependencies"),
        value(&restored, "$config_dependencies")
    );
    assert_ne!(a, b);
    assert_eq!(a, restored);
}

#[test]
fn protobuf_php_filegroup_macro_accepts_integer_testonly() {
    let workspace = scratch("protobuf-php-filegroup-testonly");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("protobuf.bzl"),
        "def internal_php_proto_library(name, testonly = None):\n    native.filegroup(name = name, srcs = [], testonly = testonly)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':protobuf.bzl', 'internal_php_proto_library')\ninternal_php_proto_library(name = 'test_messages_proto3_php_proto', testonly = 1)\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let testonly = loaded
        .native_attributes("test_messages_proto3_php_proto")
        .unwrap()
        .get("testonly")
        .unwrap()
        .1;
    assert_eq!(testonly.provenance, AttributeProvenance::Explicit);
    assert_eq!(testonly.value, CoercedAttributeValue::Boolean(true));
}

#[test]
fn native_filegroup_and_alias_testonly_kwargs_keep_values_and_default_provenance() {
    let workspace = scratch("native-testonly-kwargs");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def emit_filegroup(name, **kwargs):\n    native.filegroup(name = name, **kwargs)\n\ndef emit_alias(name, **kwargs):\n    native.alias(name = name, actual = ':leaf', **kwargs)\n",
    )
    .unwrap();
    let build_file = package.join(BUILD_FILE_PRIMARY);
    let cases = [
        ("zero", Some("0"), Some(false)),
        ("one", Some("1"), Some(true)),
        ("false", Some("False"), Some(false)),
        ("true", Some("True"), Some(true)),
        ("none", Some("None"), None),
        ("omitted", None, None),
    ];
    let routes = [
        ("filegroup", "direct_filegroup", false),
        ("alias", "direct_alias", true),
        ("emit_filegroup", "macro_filegroup", false),
        ("emit_alias", "macro_alias", false),
    ];
    for default in [false, true] {
        let mut source = String::from(
            "load(':defs.bzl', 'emit_filegroup', 'emit_alias')\nfilegroup(name = 'leaf')\n",
        );
        for (callable, prefix, needs_actual) in routes {
            for (case, argument, _) in cases {
                let actual = if needs_actual {
                    ", actual = ':leaf'"
                } else {
                    ""
                };
                let testonly =
                    argument.map_or(String::new(), |value| format!(", testonly = {value}"));
                source.push_str(&format!(
                    "{callable}(name = '{prefix}_{case}'{actual}{testonly})\n"
                ));
            }
        }
        source.push_str(&format!(
            "package(default_testonly = {})\n",
            if default { "True" } else { "False" }
        ));
        fs::write(&build_file, source).unwrap();
        let loaded = load_package(&workspace, &package);
        for (_, prefix, _) in routes {
            for (case, _, supplied) in cases {
                let name = format!("{prefix}_{case}");
                let attribute = loaded
                    .native_attributes(&name)
                    .unwrap()
                    .get("testonly")
                    .unwrap()
                    .1;
                assert_eq!(
                    attribute.value,
                    CoercedAttributeValue::Boolean(supplied.unwrap_or(default)),
                    "{name} with package default {default}"
                );
                assert_eq!(
                    attribute.provenance,
                    if supplied.is_some() {
                        AttributeProvenance::Explicit
                    } else {
                        AttributeProvenance::Default
                    },
                    "{name} with package default {default}"
                );
            }
        }
    }
}

#[test]
fn native_filegroup_none_kwargs_inherit_and_empty_kwargs_remain_explicit() {
    let workspace = scratch("native-filegroup-none-kwargs");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "filegroup(name = 'none', tags = None, data = None)\nfilegroup(name = 'empty', tags = [], data = [])\n",
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    for (name, provenance) in [
        ("none", AttributeProvenance::Default),
        ("empty", AttributeProvenance::Explicit),
    ] {
        let attributes = loaded.native_attributes(name).unwrap();
        let tags = attributes.get("tags").unwrap().1;
        let data = attributes.get("data").unwrap().1;
        assert_eq!(tags.provenance, provenance, "{name} tags");
        assert_eq!(data.provenance, provenance, "{name} data");
        assert!(
            matches!(&tags.value, CoercedAttributeValue::StringList(values) if values.is_empty())
        );
        assert!(
            matches!(&data.value, CoercedAttributeValue::LabelList(values) if values.is_empty())
        );
    }
}

#[test]
fn native_testonly_kwargs_reject_invalid_values_and_unknown_none() {
    let workspace = scratch("native-testonly-invalid-kwargs");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let build_file = package.join(BUILD_FILE_PRIMARY);
    for (source, diagnostic) in [
        ("filegroup(name = 'bad', testonly = 2)", "must be a bool"),
        ("filegroup(name = 'bad', testonly = -1)", "must be a bool"),
        ("filegroup(name = 'bad', testonly = '1')", "must be a bool"),
        (
            "alias(name = 'bad', actual = ':leaf', testonly = 2)",
            "must be a bool",
        ),
        (
            "filegroup(name = 'bad', testonly = select({'//conditions:default': True}))",
            "must be a bool",
        ),
        ("filegroup(name = 'bad', unknown = None)", "not declared"),
    ] {
        fs::write(&build_file, source).unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(diagnostic), "{source}: {error}");
    }
}

#[test]
fn native_filegroup_testonly_integer_source_restores_on_same_dice() {
    let workspace = scratch("native-filegroup-testonly-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def emit(testonly):\n    native.filegroup(name = 'subject', testonly = testonly)\n",
    )
    .unwrap();
    let build_file = package.join(BUILD_FILE_PRIMARY);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |testonly| {
        fs::write(
            &build_file,
            format!("load(':defs.bzl', 'emit')\nemit(testonly = {testonly})\n"),
        )
        .unwrap();
        runtime
            .block_on(load_package_with_retained_dice(&dice, &workspace, &package))
            .unwrap()
    };
    let first = load(1);
    let changed = load(0);
    let restored = load(1);
    let value = |loaded: &slug_loading_v2::LegacyLoadedPackage| {
        loaded
            .native_attributes("subject")
            .unwrap()
            .get("testonly")
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(value(&first).value, CoercedAttributeValue::Boolean(true));
    assert_eq!(value(&changed).value, CoercedAttributeValue::Boolean(false));
    assert_eq!(value(&first).provenance, AttributeProvenance::Explicit);
    assert_ne!(first, changed);
    assert_eq!(first, restored);
}

#[test]
fn build_file_genrule_retains_protobuf_declaration_and_effective_testonly() {
    let workspace = scratch("build-file-genrule");
    let package = workspace.join("protobuf");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        concat!(
            "genrule(name = 'generated_protos', testonly = 1, srcs = ['//src/google/protobuf:test_proto_srcs'], outs = ['unittest_gen_import.proto'], cmd = 'cat src/google/protobuf/unittest_import.proto > $@')\n",
            "genrule(name = 'explicit_false', testonly = 0, outs = ['false.out'], cmd = 'true')\n",
            "genrule(name = 'omitted', outs = ['omitted.out'], cmd = 'true')\n",
            "genrule(name = 'explicit_none', testonly = None, outs = ['none.out'], cmd = 'true')\n",
            "package(default_testonly = True)\n",
        ),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let rule = |name: &str| {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::Genrule(rule) = &target.kind else {
            panic!("{name} was not recorded as a genrule")
        };
        rule
    };
    let generated = rule("generated_protos");
    assert_eq!(generated.testonly, Some(true));
    assert_eq!(rule("explicit_false").testonly, Some(false));
    assert_eq!(rule("omitted").testonly, Some(true));
    assert_eq!(rule("explicit_none").testonly, Some(true));
    assert_eq!(
        generated.cmd,
        "cat src/google/protobuf/unittest_import.proto > $@"
    );
    assert_eq!(
        generated.srcs.as_deref().unwrap(),
        [CanonicalLabel::parse("@@//src/google/protobuf:test_proto_srcs").unwrap()]
    );
    assert_eq!(
        generated.outs.as_ref(),
        [CanonicalLabel::parse("@@//protobuf:unittest_gen_import.proto").unwrap()]
    );
    let output = loaded
        .targets
        .iter()
        .find(|target| target.name == "unittest_gen_import.proto")
        .unwrap();
    assert!(matches!(
        &output.kind,
        PackageTargetKind::GeneratedFile { generating_rule, .. } if generating_rule == "generated_protos"
    ));
}

#[test]
fn build_file_genrule_testonly_source_and_default_restore_on_same_dice() {
    let workspace = scratch("build-file-genrule-testonly-aba");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let build = |explicit: i32, default: bool| {
        format!(
            "genrule(name = 'explicit', testonly = {explicit}, outs = ['explicit.out'], cmd = 'true')\n\
             genrule(name = 'inherited', testonly = None, outs = ['inherited.out'], cmd = 'true')\n\
             package(default_testonly = {})\n",
            if default { "True" } else { "False" },
        )
    };
    let build_file = package.join(BUILD_FILE_PRIMARY);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let load = |explicit, default| {
        fs::write(&build_file, build(explicit, default)).unwrap();
        runtime
            .block_on(load_package_with_retained_dice(&dice, &workspace, &package))
            .unwrap()
    };
    let a = load(1, false);
    let changed_explicit = load(0, false);
    assert_ne!(a, changed_explicit);
    let restored_explicit = load(1, false);
    assert_eq!(a, restored_explicit);
    let changed_default = load(1, true);
    assert_ne!(a, changed_default);
    let restored_default = load(1, false);
    assert_eq!(a, restored_default);
    let effective = |loaded: &slug_loading_v2::LegacyLoadedPackage, name: &str| {
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        let PackageTargetKind::Genrule(rule) = &target.kind else {
            panic!("{name} was not recorded as a genrule")
        };
        rule.testonly
    };
    assert_eq!(effective(&a, "explicit"), Some(true));
    assert_eq!(effective(&changed_explicit, "explicit"), Some(false));
    assert_eq!(effective(&a, "inherited"), Some(false));
    assert_eq!(effective(&changed_default, "inherited"), Some(true));
    assert_eq!(effective(&restored_default, "inherited"), Some(false));
}

#[test]
fn imported_native_genrule_rejects_unadmitted_and_conflicting_shapes_atomically() {
    let workspace = scratch("native-genrule-rejections");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    let cases = [
        "native.genrule('x', cmd='x', outs=['x.out'])",
        "native.genrule(cmd='x', outs=['x.out'])",
        "native.genrule(name=1, cmd='x', outs=['x.out'])",
        "native.genrule(name='x', cmd=1, outs=['x.out'])",
        "native.genrule(name='x', cmd='x')",
        "native.genrule(name='x', cmd='x', outs=1)",
        "native.genrule(name='x',cmd='x',outs=['o'],srcs=1)",
        "native.genrule(name='x',cmd='x',outs=['o'],toolchains=1)",
        "native.genrule(name='x',cmd='x',outs=['o'],tags=[1])",
        "native.genrule(name='x', cmd='x', outs=[])",
        "native.genrule(name='x', cmd='x', outs=['x.out'], srcs=select({'//conditions:default': []}))",
        "native.genrule(name='x', cmd='x', outs=['x.out'], executable=True)",
        "native.genrule(name='x', cmd='x', outs=['same','same'])",
        "native.filegroup(name='taken')\n    native.genrule(name='x',cmd='x',outs=['taken'])",
        "native.genrule(name='x',cmd='x',outs=['dir','dir/out'])",
        "native.genrule(name='prior',cmd='x',outs=['dir/out'])\n    native.genrule(name='x',cmd='x',outs=['dir'])",
        "native.genrule(name='prior',cmd='x',outs=['dir'])\n    native.genrule(name='x',cmd='x',outs=['dir/out'])",
        "native.genrule(name='x',cmd='x',srcs=['same'],outs=['same'])",
        "native.genrule(name='x',cmd='x',toolchains=['same'],outs=['same'])",
    ];
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(':defs.bzl','bad')\nbad()\n",
    )
    .unwrap();
    for body in cases {
        fs::write(
            package.join("defs.bzl"),
            format!("def bad():\n    {}\n", body.replace('\n', "\n    ")),
        )
        .unwrap();
        assert!(
            try_load_package(&workspace, &package).is_err(),
            "accepted: {body}"
        );
    }
}

#[test]
fn build_file_genrule_rejects_unselected_shapes_and_bzl_global() {
    let workspace = scratch("build-file-genrule-rejections");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let build_file = package.join(BUILD_FILE_PRIMARY);
    for source in [
        "genrule(name='x',testonly=2,outs=['x.out'],cmd='true')",
        "genrule(name='x',testonly='true',outs=['x.out'],cmd='true')",
        "genrule(name='x',srcs=1,outs=['x.out'],cmd='true')",
        "genrule(name='x',outs=['x.out'],cmd=1)",
        "genrule(name='x',outs=['x.out'],cmd='true',tools=[':tool'])",
        "genrule(name='x',outs=['x.out'],cmd='true',srcs=select({'//conditions:default': []}))",
        "genrule(name='x',outs=['same','same'],cmd='true')",
        "genrule(name='x',srcs=[':same'],outs=['same'],cmd='true')",
    ] {
        fs::write(&build_file, source).unwrap();
        assert!(
            try_load_package(&workspace, &package).is_err(),
            "accepted: {source}"
        );
    }
    fs::write(
        package.join("defs.bzl"),
        "def emit(): genrule(name='x',outs=['x.out'],cmd='true')\n",
    )
    .unwrap();
    fs::write(&build_file, "load(':defs.bzl', 'emit')\nemit()\n").unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("genrule"), "{error}");
}

#[test]
fn native_alias_retains_configurable_actual_and_config_dependencies() {
    let workspace = scratch("native-alias-configurable");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"config_setting(name = "a", values = {"compilation_mode": "dbg"})
config_setting(name = "b", values = {"compilation_mode": "opt"})
alias(name = "literal", actual = ":left")
alias(name = "selected", actual = select({":a": ":left", ":b": ":right", "//conditions:default": ":fallback"}))
"#,
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    let target = |name: &str| {
        loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap()
    };
    assert!(matches!(
        &target("literal").kind,
        PackageTargetKind::Alias { actual: AliasActual::Visible(actual) }
            if actual == &CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//pkg:left").unwrap()
            )
    ));
    let PackageTargetKind::Alias {
        actual: AliasActual::Visible(actual),
    } = &target("selected").kind
    else {
        panic!("selected target must remain an alias")
    };
    let CoercedAttributeValue::Selector { branches, default } = actual else {
        panic!("selected actual must retain its selector")
    };
    assert_eq!(
        branches
            .iter()
            .map(|(condition, value)| (condition.to_string(), value.as_ref().clone()))
            .collect::<Vec<_>>(),
        [
            (
                "@@//pkg:a".to_owned(),
                CoercedAttributeValue::Label(CanonicalLabel::parse("@@//pkg:left").unwrap())
            ),
            (
                "@@//pkg:b".to_owned(),
                CoercedAttributeValue::Label(CanonicalLabel::parse("@@//pkg:right").unwrap())
            ),
        ]
    );
    assert_eq!(
        default.as_deref(),
        Some(&CoercedAttributeValue::Label(
            CanonicalLabel::parse("@@//pkg:fallback").unwrap()
        ))
    );
    let attributes = loaded.native_attributes("selected").unwrap();
    assert!(matches!(
        &attributes.get("actual").unwrap().1.value,
        CoercedAttributeValue::Selector { .. }
    ));
    assert!(matches!(
        &attributes.get("$config_dependencies").unwrap().1.value,
        CoercedAttributeValue::LabelList(labels)
            if labels.as_ref() == [
                CanonicalLabel::parse("@@//pkg:a").unwrap(),
                CanonicalLabel::parse("@@//pkg:b").unwrap(),
            ]
    ));
}

#[test]
fn native_alias_none_visibility_inherits_package_default_in_build_and_macro() {
    let workspace = scratch("native-alias-none-visibility");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"def emit_aliases():
    native.alias(name = "macro_omitted", actual = ":leaf")
    native.alias(name = "macro_none", actual = ":leaf", visibility = None)
    native.alias(name = "macro_empty", actual = ":leaf", visibility = [])
    native.alias(name = "macro_list", actual = ":leaf", visibility = ["//visibility:public"])
    native.alias(name = "macro_tuple", actual = ":leaf", visibility = ("//visibility:public",))

def bad_visibility(value):
    native.alias(name = "bad", actual = ":leaf", visibility = value)

def bad_unknown():
    native.alias(name = "bad", actual = ":leaf", bogus = None)
"#,
    )
    .unwrap();
    let build = package.join(BUILD_FILE_PRIMARY);
    let build_prefix = r#"load(":defs.bzl", "emit_aliases", "bad_visibility", "bad_unknown")
package(default_visibility = ["//visibility:public"])
filegroup(name = "leaf")
"#;
    fs::write(
        &build,
        format!(
            "{build_prefix}{}",
            r#"alias(name = "direct_omitted", actual = ":leaf")
alias(name = "direct_none", actual = ":leaf", visibility = None)
alias(name = "direct_empty", actual = ":leaf", visibility = [])
alias(name = "direct_list", actual = ":leaf", visibility = ["//visibility:public"])
alias(name = "direct_tuple", actual = ":leaf", visibility = ("//visibility:public",))
emit_aliases()
"#
        ),
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    assert_eq!(loaded.default_visibility, RuleVisibility::Public);
    for prefix in ["direct", "macro"] {
        for suffix in ["omitted", "none"] {
            let name = format!("{prefix}_{suffix}");
            let target = loaded
                .targets
                .iter()
                .find(|target| target.name == name)
                .unwrap();
            assert_eq!(
                target.visibility,
                VisibilitySource::PackageDefault,
                "{name}"
            );
            assert_eq!(
                loaded.effective_visibility(target),
                Some(RuleVisibility::Public)
            );
        }
        let name = format!("{prefix}_empty");
        let target = loaded
            .targets
            .iter()
            .find(|target| target.name == name)
            .unwrap();
        assert_eq!(
            target.visibility,
            VisibilitySource::Declared(RuleVisibility::Private)
        );
        assert_eq!(
            loaded.effective_visibility(target),
            Some(RuleVisibility::Private)
        );
        for suffix in ["list", "tuple"] {
            let name = format!("{prefix}_{suffix}");
            let target = loaded
                .targets
                .iter()
                .find(|target| target.name == name)
                .unwrap();
            assert_eq!(
                target.visibility,
                VisibilitySource::Declared(RuleVisibility::Public),
                "{name}"
            );
            assert_eq!(
                loaded.effective_visibility(target),
                Some(RuleVisibility::Public)
            );
        }
    }

    for invalid in [
        "alias(name = 'bad', actual = ':leaf', visibility = '//visibility:public')",
        "alias(name = 'bad', actual = ':leaf', visibility = [1])",
        "alias(name = 'bad', actual = ':leaf', bogus = None)",
        "alias(name = 'bad', actual = [':leaf'], visibility = None)",
        "bad_visibility('//visibility:public')",
        "bad_visibility([1])",
        "bad_unknown()",
    ] {
        fs::write(&build, format!("{build_prefix}{invalid}\n")).unwrap();
        assert!(try_load_package(&workspace, &package).is_err(), "{invalid}");
    }
}

#[test]
fn native_alias_rejects_invalid_loading_shapes_atomically() {
    let workspace = scratch("native-alias-invalid");
    fs::write(workspace.join(MODULE_FILE), "module(name = 'root')\n").unwrap();
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    for invalid in [
        "alias(name = 'bad')",
        "alias(name = 'bad', actual = None)",
        "alias(name = 'bad', actual = [':x'])",
        "alias(name = 'bad', actual = select({'//conditions:default': 1}))",
        "alias(name = 'bad', actual = select({':a': ':x'}) + select({'//conditions:default': ':y'}))",
    ] {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("alias(name = 'prior', actual = ':x')\n{invalid}\n"),
        )
        .unwrap();
        assert!(try_load_package(&workspace, &package).is_err(), "{invalid}");
    }
}
