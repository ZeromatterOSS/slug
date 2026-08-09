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
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AllowSingleFile;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::PackageTarget;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::TestSuiteMembership;
use slug_loading_v2::VisibilitySource;
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
use slug_loading_v2::package::NativeToolchainTarget;
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

fn load_package(workspace: &Path, package: &Path) -> slug_loading_v2::LoadedPackage {
    try_load_package(workspace, package).unwrap()
}

fn try_load_package(
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    try_load_package_with_extra_bzl(workspace, package, &[])
}

fn try_load_package_with_extra_bzl(
    workspace: &Path,
    package: &Path,
    extra_bzl: &[PathBuf],
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    try_load_package_with_event_capture(workspace, package, extra_bzl, None, false)
}

fn try_load_package_with_event_capture(
    workspace: &Path,
    package: &Path,
    extra_bzl: &[PathBuf],
    tracker: Option<Arc<dyn ActivationTracker>>,
    capture_events: bool,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
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
            .output(),
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
fn config_setting_retains_values_and_rejects_unmodeled_arguments() {
    let workspace = scratch("config-setting");
    let package = workspace.join("pkg");
    fs::create_dir_all(&package).unwrap();
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\", \"compilation_mode\": \"opt\"})\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(
        loaded.targets,
        vec![PackageTarget {
            name: "linux".to_owned(),
            kind: PackageTargetKind::ConfigSetting {
                values: vec![
                    ("compilation_mode".into(), "opt".into()),
                    ("cpu".into(), "k8".into()),
                ]
                .into(),
            },
            visibility: VisibilitySource::AlwaysPublic,
        }]
    );

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "config_setting(name = \"unsupported\", values = {}, define_values = {\"mode\": \"fast\"})\n",
    )
    .unwrap();
    let loaded = load_package(&workspace, &package);
    assert!(
        loaded
            .native_attributes("unsupported")
            .unwrap()
            .get("define_values")
            .is_some()
    );
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
    toolchain = ":implementation",
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
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintSetting)
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
        }) if toolchain_type == &CanonicalLabel::parse("@@//pkg:demo_type").unwrap()
            && implementation == &CanonicalLabel::parse("@@//pkg:implementation").unwrap()
            && exec_compatible_with.as_ref() == [
                CanonicalLabel::parse("@@//pkg:first").unwrap(),
                CanonicalLabel::parse("@@//pkg:second").unwrap(),
            ]
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
            "platform(name = 'bad', constraint_values = (':one',))",
            "constraint_values",
        ),
        (
            "platform(name = 'bad', constraint_values = ['//pkg/...'])",
            "direct target labels",
        ),
        (
            "toolchain(name = 'bad', exec_compatible_with = ['//pkg:*'], toolchain = ':impl', toolchain_type = ':type')",
            "direct target labels",
        ),
        (
            "toolchain(name = 'bad', exec_compatible_with = [], toolchain = '@repo//:impl', toolchain_type = ':type')",
            "external repository dependency labels are not supported",
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
                    actual: CanonicalLabel::parse("@@//pkg:fg").unwrap(),
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
        PackageTargetKind::Alias { actual }
            if actual == &CanonicalLabel::parse("@@//pkg:group").unwrap()
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
        "load(\":defs.bzl\", \"with_deps\")\nwith_deps(name = \"ordered\", deps = (\"//leaf:second\", \"bare\", \"dir/name\", \":local\", \"//leaf:first\"), visibility = [\"//visibility:public\"])\nwith_deps(name = \"omitted\")\n",
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
            "invalid label 'pkg:target': absolute label must begin with '@' or '//'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"...\"])\n",
            "invalid label '...': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"foo/...\"])\n",
            "invalid label 'foo/...': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"...:all\"])\n",
            "invalid label '...:all': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"foo/...:all\"])\n",
            "invalid label 'foo/...:all': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"//foo/...\"])\n",
            "invalid label '//foo/...': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"//foo/...:all\"])\n",
            "invalid label '//foo/...:all': package name cannot contain '...'",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
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
    assert!(matches!(
        schema("optional").allow_single_file(),
        Some(AllowSingleFile::True)
    ));
    assert!(matches!(
        schema("no_files").allow_single_file(),
        Some(AllowSingleFile::False)
    ));
    assert!(matches!(
        schema("extensions").allow_single_file(),
        Some(AllowSingleFile::Extensions(extensions))
            if extensions.as_ref() == [".txt", ".md"]
    ));
    assert_eq!(schema("omitted_files").allow_single_file(), None);
    assert_eq!(schema("explicit_none_files").allow_single_file(), None);
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
            .output(),
        "//pkg:setting...variant"
    );

    fs::write(package.join("defs.bzl"), definitions("//pkg/...:setting")).unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("one main-repository target label"),
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
    for invalid in ["//other:out", "../out", "bad:name"] {
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
    let requirements = |loaded: &slug_loading_v2::LoadedPackage, name: &str| {
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
    let dependencies = |loaded: &slug_loading_v2::LoadedPackage, name: &str| {
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
fn rule_toolchains_and_toolchain_info_fail_closed_outside_the_fixture_subset() {
    let cases = [
        ("toolchains = \":demo_type\"", "list"),
        ("toolchains = (\":demo_type\",)", "list"),
        ("toolchains = [1]", "expected `list[str]`"),
        ("toolchains = [\"@external//:type\"]", "external repository"),
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
