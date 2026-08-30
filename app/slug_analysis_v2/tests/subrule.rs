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
use slug_analysis_v2::AnalysisPreparationOutcome;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredNodeAnalysisKey;
use slug_analysis_v2::ConfiguredNodeAnalysisObservationKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::analysis_cycle_detector;
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
use slug_bzlmod_v2::RootPackagePolicyInputs;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_bzlmod_v2::inject_root_package_policy_inputs;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
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
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshot;
use slug_workspace_v2::WorkspaceSnapshotKey;
use starlark_map::small_map::SmallMap;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}

#[derive(Default)]
struct Epoch {
    entries: SmallMap<PathObservationDemand, PathObservationResult>,
}

impl Epoch {
    fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    }

    fn node(&mut self, path: &str, kind: PathNodeKind) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 1, 1, 1, 0o755,
            ))),
        );
    }

    fn directory(&mut self, path: &str) {
        self.node(path, PathNodeKind::Directory);
    }

    fn missing(&mut self, path: &str) {
        self.entries.insert(
            Self::demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        );
    }

    fn file(&mut self, path: &str, source: &str) {
        self.node(path, PathNodeKind::RegularFile);
        self.entries.insert(
            Self::demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                source.as_bytes(),
            ))),
        );
    }

    fn package(&mut self, name: &str, build: &str) {
        let directory = format!("/workspace/{name}");
        self.directory(&directory);
        self.file(&format!("{directory}/BUILD.bazel"), build);
    }

    fn fixture() -> Self {
        let mut epoch = Self::default();
        epoch.directory("/");
        epoch.directory("/workspace");
        epoch.file("/workspace/MODULE.bazel", "module(name = 'root')\n");
        for path in ["MODULE.bazel.lock", "REPO.bazel", ".bazelignore"] {
            epoch.missing(&format!("/workspace/{path}"));
        }
        epoch.directory("/workspace/.slug_builtin");
        epoch.directory("/workspace/.slug_builtin/bazel_tools");
        epoch.file(
            "/workspace/.slug_builtin/bazel_tools/MODULE.bazel",
            "module(name = 'bazel_tools')\n",
        );
        epoch.package(".slug_test_host", "platform(name = 'host')\n");
        epoch
    }

    fn build(self) -> PathObservationEpoch {
        PathObservationEpoch::new(self.entries).unwrap()
    }
}

fn snapshots(epoch: &PathObservationEpoch) -> (Arc<WorkspaceSnapshot>, Arc<WorkspaceRawSnapshot>) {
    let mut text = Vec::new();
    let mut raw = Vec::new();
    for (demand, result) in epoch.observations() {
        let PathObservationResult::FileBytes(result) = result.as_ref() else {
            continue;
        };
        let path = demand.path().as_path().to_path_buf();
        if let PathOperationResult::Present(bytes) = result {
            raw.push((path.clone(), WorkspaceRawFileValue::Present(bytes.clone())));
            text.push((
                path,
                WorkspaceFileValue::Present(Arc::new(String::from_utf8(bytes.to_vec()).unwrap())),
            ));
        }
    }
    (
        Arc::new(WorkspaceSnapshot {
            files: Arc::new(text.into_iter().collect()),
        }),
        Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(raw.into_iter().collect()),
        }),
    )
}

#[derive(Default)]
struct Tracker(Mutex<Vec<(String, ConfigurationKey, ActivationKind)>>);

impl ActivationTracker for Tracker {
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
        let Some(key) = key.downcast_ref::<ConfiguredNodeAnalysisKey>() else {
            return;
        };
        let Some(target) = key.configured_target() else {
            return;
        };
        self.0.lock().unwrap().push((
            target.label().to_string(),
            target.configuration().clone(),
            activation.kind(),
        ));
    }
}

fn configuration(profile: Option<&str>) -> ConfigurationKey {
    let base = SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap(),
    )
    .unwrap()
    .with_host_platform_label(&CanonicalLabel::parse("@@//.slug_test_host:host").unwrap());
    let Some(profile) = profile else {
        return ConfigurationKey::from_slug(base);
    };
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::FdoProfile,
        Some(profile),
        false,
    )]
    .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

fn grte_configuration(value: &str) -> ConfigurationKey {
    let base = configuration(None).slug_configuration().unwrap().clone();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::GrteTop,
        Some(value),
        false,
    )]
    .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

async fn analyze(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<Tracker>,
    route: AnalysisRoute,
) -> String {
    let data = UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        activation_tracker: Some(tracker),
        ..Default::default()
    };
    let mut updater = dice.updater_with_data(data);
    let (text, raw) = snapshots(&epoch);
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: "/workspace".into(),
            },
            text,
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: "/workspace".into(),
            },
            raw,
        )])
        .unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    let mut attributes = SmallMap::new();
    attributes.insert(
        "path".into(),
        OverrideAttributeValue::String("/workspace/.slug_builtin/bazel_tools".into()),
    );
    let request = Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new("bazel_tools+").unwrap(),
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
            logical_root: NormalizedAbsolutePath::new("/workspace/.slug_builtin/bazel_tools")
                .unwrap(),
        },
    });
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            RepositoryMaterializationResultEpoch::new(
                workspace(),
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
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace(),
            [workspace()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace().as_path(),
        BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            workspace().as_path(),
            ["bazel_tools=/workspace/.slug_builtin/bazel_tools"],
        )
        .unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let target = ConfiguredTargetKey::new(CanonicalLabel::parse(target).unwrap(), configuration);
    let mut transaction = updater.commit().await;
    match route {
        AnalysisRoute::Legacy => {
            let key = ConfiguredNodeAnalysisKey::new(workspace(), target).unwrap();
            let outcome = transaction.compute(&key).await.unwrap();
            let AnalysisPreparationOutcome::Complete(result) = outcome else {
                panic!("legacy analysis returned Need: {outcome:?}")
            };
            result.as_ref().as_ref().unwrap_err().to_string()
        }
        AnalysisRoute::Observed => {
            let key = ConfiguredNodeAnalysisObservationKey::new(workspace(), target).unwrap();
            let outcome = transaction.compute(&key).await.unwrap();
            let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
                panic!("observed analysis did not complete: {outcome:?}")
            };
            result.as_ref().as_ref().unwrap_err().to_string()
        }
    }
}

#[derive(Clone, Copy)]
enum AnalysisRoute {
    Legacy,
    Observed,
}

fn semantic_epoch() -> Epoch {
    let defs = r#"
ProfileInfo = provider()
def _profile(ctx): return [DefaultInfo(), ProfileInfo()]
def _plain(ctx): return [DefaultInfo()]
def _multi(ctx):
    first = ctx.actions.declare_file(ctx.label.name + ".one")
    second = ctx.actions.declare_file(ctx.label.name + ".two")
    ctx.actions.write(first, "one")
    ctx.actions.write(second, "two")
    return [DefaultInfo(files = depset([first, second]))]
def _tool(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "tool", is_executable = True)
    return [DefaultInfo(executable = out), ProfileInfo()]
profile = rule(implementation = _profile)
plain = rule(implementation = _plain)
multi = rule(implementation = _multi)
tool = rule(implementation = _tool, executable = True)
def _subrule(ctx, **kwargs): fail("subrule implementation ran")
configured = subrule(
    implementation = _subrule,
    attrs = {
        "_literal": attr.label(default = "//deps:literal", providers = [ProfileInfo]),
        "_list": attr.label_list(default = ["//deps:literal", "//deps:other"], providers = [ProfileInfo]),
        "_source": attr.label(default = "//files:inferred.txt", allow_single_file = [".txt"]),
        "_source_provider": attr.label(default = "//files:inferred.txt", allow_files = True, providers = [[ProfileInfo], [DefaultInfo]]),
        "_exported": attr.label(default = "//files:exported.sh", allow_files = True, cfg = "exec", executable = True),
        "_exec": attr.label(default = "//deps:tool", cfg = "exec", executable = True, providers = [ProfileInfo]),
        "_late": attr.label(default = configuration_field(fragment = "cpp", name = "fdo_profile"), providers = [ProfileInfo]),
        "_or": attr.label(default = "//deps:plain", providers = [[ProfileInfo], [DefaultInfo]]),
        "_unrestricted": attr.label(default = "//deps:plain"),
        "_generated": attr.label(default = "//deps:generated.bin", allow_single_file = [".bin"]),
        "_alias": attr.label(default = "//deps:alias_profile", providers = [ProfileInfo]),
    },
)
def _subject(ctx): fail("rule implementation ran")
subject = rule(implementation = _subject, subrules = [configured])
bad_provider = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label_list(default = ["//deps:literal", "//deps:plain"], providers = [ProfileInfo]),
})
bad_provider_subject = rule(implementation = _subject, subrules = [bad_provider])
zero_single = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//deps:plain", allow_single_file = True),
})
zero_single_subject = rule(implementation = _subject, subrules = [zero_single])
multi_single = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//deps:multi", allow_single_file = True),
})
multi_single_subject = rule(implementation = _subject, subrules = [multi_single])
bad_extension = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//files:wrong.bin", allow_single_file = [".txt"]),
})
bad_extension_subject = rule(implementation = _subject, subrules = [bad_extension])
bad_executable = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//deps:plain", cfg = "exec", executable = True),
})
bad_executable_subject = rule(implementation = _subject, subrules = [bad_executable])
bad_file = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//files:exported.sh"),
})
bad_file_subject = rule(implementation = _subject, subrules = [bad_file])
bad_file_provider = subrule(implementation = _subrule, attrs = {
    "_bad": attr.label(default = "//files:inferred.txt", allow_files = True, providers = [ProfileInfo]),
})
bad_file_provider_subject = rule(implementation = _subject, subrules = [bad_file_provider])
exec_first = subrule(implementation = _subrule, attrs = {
    "_exec": attr.label(default = "//deps:plain", cfg = "exec", executable = True),
    "_target": attr.label(default = "//deps:plain", providers = [ProfileInfo]),
})
exec_first_subject = rule(implementation = _subject, subrules = [exec_first])
target_first = subrule(implementation = _subrule, attrs = {
    "_target": attr.label(default = "//deps:plain", providers = [ProfileInfo]),
    "_exec": attr.label(default = "//deps:plain", cfg = "exec", executable = True),
})
target_first_subject = rule(implementation = _subject, subrules = [target_first])
ordinary = rule(implementation = _subject, attrs = {
    "_libc_top": attr.label(default = configuration_field(fragment = "cpp", name = "libc_top")),
    "_zipper": attr.label(default = configuration_field(fragment = "cpp", name = "zipper"), cfg = "exec", executable = True),
})
def _declares(ctx): return [DefaultInfo()]
declares = rule(implementation = _declares, attrs = {"src": attr.label(allow_files = True)})
def _generate(ctx):
    ctx.actions.write(ctx.outputs.out, "generated")
    return [DefaultInfo(files = depset([ctx.outputs.out]))]
generate = rule(implementation = _generate, attrs = {"out": attr.output()})
"#;
    let mut epoch = Epoch::fixture();
    epoch.package("rules", "");
    epoch.file("/workspace/rules/defs.bzl", defs);
    epoch.package(
        "deps",
        "load('//rules:defs.bzl', 'generate', 'multi', 'plain', 'profile', 'tool')\nprofile(name='literal')\nprofile(name='other')\nprofile(name='profile')\nprofile(name='everything')\nplain(name='plain')\nmulti(name='multi')\ntool(name='tool')\ngenerate(name='generator', out='generated.bin')\nalias(name='alias_profile', actual=':literal')\n",
    );
    epoch.package(
        "files",
        "load('//rules:defs.bzl', 'declares')\ndeclares(name='owner', src='inferred.txt')\ndeclares(name='wrong_owner', src='wrong.bin')\nexports_files(['exported.sh'])\n",
    );
    epoch.file("/workspace/files/inferred.txt", "source");
    epoch.file("/workspace/files/wrong.bin", "wrong");
    epoch.file("/workspace/files/exported.sh", "#!/bin/sh\n");
    epoch.package(
        "subject",
        "load('//rules:defs.bzl', 'bad_executable_subject', 'bad_extension_subject', 'bad_file_provider_subject', 'bad_file_subject', 'bad_provider_subject', 'exec_first_subject', 'multi_single_subject', 'ordinary', 'subject', 'target_first_subject', 'zero_single_subject')\nsubject(name='subject')\nbad_provider_subject(name='bad_provider')\nzero_single_subject(name='zero_single')\nmulti_single_subject(name='multi_single')\nbad_extension_subject(name='bad_extension')\nbad_executable_subject(name='bad_executable')\nbad_file_subject(name='bad_file')\nbad_file_provider_subject(name='bad_file_provider')\nexec_first_subject(name='exec_first')\ntarget_first_subject(name='target_first')\nordinary(name='ordinary')\n",
    );
    epoch
}

fn cycle_epoch(repaired: bool) -> Epoch {
    let direct = if repaired {
        "//subject:leaf"
    } else {
        "//subject:direct"
    };
    let cross = if repaired {
        "//subject:leaf"
    } else {
        "//subject:cross_a"
    };
    let defs = format!(
        r#"def _leaf(ctx): return [DefaultInfo()]
leaf = rule(implementation = _leaf)
def _toolchain(ctx): return [platform_common.ToolchainInfo()]
toolchain_impl = rule(implementation = _toolchain)
def _subrule(ctx, **kwargs): fail("subrule implementation ran")
direct_dep = subrule(implementation = _subrule, attrs = {{"_dep": attr.label(default = "{direct}")}})
cross_a_dep = subrule(implementation = _subrule, attrs = {{"_dep": attr.label(default = "//subject:cross_b")}})
cross_b_dep = subrule(implementation = _subrule, attrs = {{"_dep": attr.label(default = "{cross}")}})
def _subject(ctx): fail("rule implementation ran")
direct_rule = rule(implementation = _subject, subrules = [direct_dep], toolchains = ["//subject:type"])
cross_a_rule = rule(implementation = _subject, subrules = [cross_a_dep], toolchains = ["//subject:type"])
cross_b_rule = rule(implementation = _subject, subrules = [cross_b_dep], toolchains = ["//subject:type"])
"#,
    );
    let mut epoch = Epoch::fixture();
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name = 'root')\nregister_execution_platforms('//subject:platform')\nregister_toolchains('//subject:tc')\n",
    );
    epoch.package("rules", "");
    epoch.file("/workspace/rules/cycle.bzl", &defs);
    epoch.package(
        "subject",
        "load('//rules:cycle.bzl', 'cross_a_rule', 'cross_b_rule', 'direct_rule', 'leaf', 'toolchain_impl')\nplatform(name='platform')\ntoolchain_type(name='type')\ntoolchain_impl(name='impl')\ntoolchain(name='tc', toolchain_type=':type', toolchain=':impl')\nleaf(name='leaf')\ndirect_rule(name='direct')\ncross_a_rule(name='cross_a')\ncross_b_rule(name='cross_b')\n",
    );
    epoch
}

#[tokio::test]
async fn configured_subrule_dependencies_validate_before_the_invocation_boundary() {
    let epoch = semantic_epoch();
    let tracker = Arc::new(Tracker::default());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let error = analyze(
        &dice,
        epoch.build(),
        "@@//subject:subject",
        configuration(Some("//deps:profile")),
        tracker.clone(),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(
        error.contains("reached the deferred invocation boundary"),
        "{error}"
    );
    assert!(!error.contains("implementation ran"), "{error}");
    let activations = tracker.0.lock().unwrap();
    assert!(
        activations
            .iter()
            .any(|(label, _, _)| label == "@@//deps:alias_profile")
    );
    let exec = activations
        .iter()
        .find(|(label, _, _)| label == "@@//deps:tool")
        .expect("Exec dependency was analyzed");
    assert_eq!(exec.1.kind(), slug_analysis_v2::ConfigurationKind::Exec);
    assert_eq!(
        exec.1
            .slug_configuration()
            .unwrap()
            .target_platform_label()
            .unwrap()
            .to_string(),
        "@@//.slug_test_host:host"
    );
}

#[tokio::test]
async fn configured_subrule_validation_rejects_each_child_before_invocation() {
    for (target, expected) in [
        (
            "bad_provider",
            "does not provide any admitted provider alternative",
        ),
        ("zero_single", "must provide exactly one file, got 0"),
        ("multi_single", "must provide exactly one file, got 2"),
        ("bad_extension", "does not match an admitted extension"),
        ("bad_executable", "is not executable"),
        ("bad_file", "does not admit file target"),
        (
            "bad_file_provider",
            "does not provide any admitted provider alternative",
        ),
    ] {
        let tracker = Arc::new(Tracker::default());
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let error = analyze(
            &dice,
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            configuration(Some("//deps:profile")),
            tracker,
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(error.contains(expected), "{target}: {error}");
        assert!(
            !error.contains("reached the deferred invocation boundary"),
            "{target}: {error}"
        );
        assert!(!error.contains("implementation ran"), "{target}: {error}");
    }
}

#[tokio::test]
async fn ordinary_configuration_fields_use_the_same_dependency_owner() {
    let tracker = Arc::new(Tracker::default());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let error = analyze(
        &dice,
        semantic_epoch().build(),
        "@@//subject:ordinary",
        grte_configuration("//deps:profile"),
        tracker.clone(),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(
        error.contains(
            "rule attribute '_libc_top' reached the deferred late-bound value materialization boundary"
        ),
        "{error}"
    );
    assert!(!error.contains("implementation ran"), "{error}");
    assert!(
        tracker
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|(label, configuration, _)| {
                label == "@@//deps:everything"
                    && configuration.kind() == slug_analysis_v2::ConfigurationKind::Target
            })
    );
}

#[tokio::test]
async fn interleaved_target_and_exec_rows_preserve_failure_order() {
    for (target, expected, later) in [
        ("exec_first", "is not executable", "provider alternative"),
        ("target_first", "provider alternative", "is not executable"),
    ] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let error = analyze(
            &dice,
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            configuration(Some("//deps:profile")),
            Arc::new(Tracker::default()),
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(error.contains(expected), "{target}: {error}");
        assert!(!error.contains(later), "{target}: {error}");
    }
}

#[tokio::test]
async fn hidden_dependency_cycles_terminate_and_repair_in_both_key_families() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        for target in ["direct", "cross_a"] {
            let dice = Dice::builder().build(DetectCycles::Enabled);
            let cyclic = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                analyze(
                    &dice,
                    cycle_epoch(false).build(),
                    &format!("@@//subject:{target}"),
                    configuration(None),
                    Arc::new(Tracker::default()),
                    route,
                ),
            )
            .await
            .expect("configured hidden-dependency cycle must terminate");
            assert!(cyclic.contains("configured alias cycle"), "{cyclic}");

            let repaired = analyze(
                &dice,
                cycle_epoch(true).build(),
                &format!("@@//subject:{target}"),
                configuration(None),
                Arc::new(Tracker::default()),
                route,
            )
            .await;
            assert!(
                repaired.contains("reached the deferred invocation boundary"),
                "{repaired}"
            );
            assert!(!repaired.contains("configured alias cycle"), "{repaired}");
        }
    }
}
