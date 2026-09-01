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
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ProviderId;
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
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
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
        .unwrap()
        .with_action_environment_host(ActionEnvironmentHost::without_environment(
            ActionEnvironmentHostOs::Linux,
        )),
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

fn coverage_configuration(enabled: bool, generator: Option<&str>) -> ConfigurationKey {
    let base = configuration(None).slug_configuration().unwrap().clone();
    let mut occurrences = Vec::new();
    if let Some(generator) = generator {
        occurrences.push(CommandConfigurationOccurrence::native(
            NativeCommandOption::CoverageOutputGenerator,
            Some(generator),
            false,
        ));
    }
    occurrences.push(CommandConfigurationOccurrence::native(
        NativeCommandOption::CollectCodeCoverage,
        None::<&str>,
        !enabled,
    ));
    let overlay: CommandConfigurationOverlay = occurrences.into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

fn compilation_configuration(mode: &str, host_mode: &str) -> ConfigurationKey {
    let base = configuration(None).slug_configuration().unwrap().clone();
    let overlay: CommandConfigurationOverlay = vec![
        CommandConfigurationOccurrence::native(
            NativeCommandOption::CompilationMode,
            Some(mode),
            false,
        ),
        CommandConfigurationOccurrence::native(
            NativeCommandOption::HostCompilationMode,
            Some(host_mode),
            false,
        ),
    ]
    .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

async fn analyze_result(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<Tracker>,
    route: AnalysisRoute,
) -> Result<Arc<slug_analysis_v2::ConfiguredNodeResult>, String> {
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
            result
                .as_ref()
                .as_ref()
                .map(Arc::clone)
                .map_err(ToString::to_string)
        }
        AnalysisRoute::Observed => {
            let key = ConfiguredNodeAnalysisObservationKey::new(workspace(), target).unwrap();
            let outcome = transaction.compute(&key).await.unwrap();
            let AnalysisPreparationOutcome::Complete(Ok(result)) = outcome else {
                panic!("observed analysis did not complete: {outcome:?}")
            };
            result
                .as_ref()
                .as_ref()
                .map(Arc::clone)
                .map_err(ToString::to_string)
        }
    }
}

async fn analyze(
    dice: &Arc<Dice>,
    epoch: PathObservationEpoch,
    target: &str,
    configuration: ConfigurationKey,
    tracker: Arc<Tracker>,
    route: AnalysisRoute,
) -> String {
    analyze_result(dice, epoch, target, configuration, tracker, route)
        .await
        .unwrap_err()
}

#[derive(Clone, Copy, Debug)]
enum AnalysisRoute {
    Legacy,
    Observed,
}

fn semantic_epoch() -> Epoch {
    let defs = r#"
load("//tools/build_defs/cc:helper.bzl", "allowed_mode")
ProfileInfo = provider()
ReturnedInfo = provider()
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
def _subrule(ctx, prefix, suffix = "", **kwargs):
    if prefix != "positional" or suffix != "named": fail("user arguments were not bound")
    if ctx.label.name != "subject": fail("subrule context label was not materialized")
    expected = ["_literal", "_list", "_source", "_source_provider", "_exported", "_exec", "_late", "_or", "_unrestricted", "_none", "_empty", "_generated", "_generated_provider", "_alias"]
    if kwargs.keys() != expected: fail("hidden kwargs lost descriptor order: " + repr(kwargs.keys()))
    if len(kwargs["_list"]) != 2: fail("label_list was not materialized")
    if kwargs["_none"] != None: fail("absent label was not materialized as None")
    if kwargs["_empty"] != []: fail("absent label_list was not materialized as empty")
    if kwargs["_source"].path != "files/inferred.txt": fail("single file was not materialized")
    if kwargs["_generated"].path != "deps/generated.bin": fail("generated file was not materialized")
    if kwargs["_exec"].executable.path != "deps/tool": fail("executable provider was not materialized")
    if kwargs["_exported"].executable.path != "files/exported.sh": fail("source executable provider was not materialized")
    if ProfileInfo not in kwargs["_literal"]: fail("Target provider membership was not materialized")
    if kwargs["_alias"].label.name != "literal" or ProfileInfo not in kwargs["_alias"]: fail("alias actual Target was not materialized")
    if DefaultInfo not in kwargs["_source_provider"]: fail("source Target has no inherent DefaultInfo")
    source_files = kwargs["_source_provider"][DefaultInfo].files.to_list()
    if len(source_files) != 1 or source_files[0].path != "files/inferred.txt": fail("source Target DefaultInfo was not materialized as File")
    if DefaultInfo not in kwargs["_generated_provider"]: fail("generated Target has no inherent DefaultInfo")
    generated_files = kwargs["_generated_provider"][DefaultInfo].files.to_list()
    if len(generated_files) != 1 or generated_files[0].path != "deps/generated.bin": fail("generated Target DefaultInfo was not materialized as File")
    out = ctx.actions.declare_file("subrule.txt")
    ctx.actions.write(out, "subrule")
    return ReturnedInfo(
        source_target = kwargs["_source_provider"],
        generated_target = kwargs["_generated_provider"],
        executable = kwargs["_exec"],
        output = out,
    )
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
        "_none": attr.label(default = None, allow_files = True),
        "_empty": attr.label_list(default = [], allow_files = True),
        "_generated": attr.label(default = "//deps:generated.bin", allow_single_file = [".bin"]),
        "_generated_provider": attr.label(default = "//deps:generated.bin", allow_files = True, providers = [DefaultInfo]),
        "_alias": attr.label(default = "//deps:alias_profile", providers = [ProfileInfo]),
    },
)
def _subject(ctx):
    result = configured("positional", suffix = "named")
    return [DefaultInfo(), result]
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
def _leaf_call(ctx, value): return value
leaf_call = subrule(implementation = _leaf_call)
def _parent_call(ctx):
    value = leaf_call("nested")
    if ctx.label.name != "nested": fail("parent context was not restored after child")
    return value
parent_call = subrule(implementation = _parent_call, subrules = [leaf_call])
def _nested_subject(ctx):
    if parent_call() != "nested": fail("nested subrule result was not returned")
    if parent_call() != "nested": fail("repeated nested subrule result was not returned")
    return [DefaultInfo()]
nested_subject = rule(implementation = _nested_subject, subrules = [parent_call])
def _rogue(ctx): return None
rogue = subrule(implementation = _rogue)
def _undeclared_subject(ctx):
    rogue()
    return [DefaultInfo()]
undeclared_subject = rule(implementation = _undeclared_subject)
def _bad_parent(ctx): return rogue()
bad_parent = subrule(implementation = _bad_parent)
def _bad_nested_subject(ctx):
    bad_parent()
    return [DefaultInfo()]
bad_nested_subject = rule(implementation = _bad_nested_subject, subrules = [bad_parent])
def _override(ctx, **kwargs): return None
override_call = subrule(implementation = _override, attrs = {
    "_dep": attr.label(default = "//deps:plain"),
})
def _override_subject(ctx):
    override_call(_dep = None)
    return [DefaultInfo()]
override_subject = rule(implementation = _override_subject, subrules = [override_call])
def _use_outer_actions(ctx, outer): return outer.declare_file("forbidden.txt")
use_outer_actions = subrule(implementation = _use_outer_actions)
def _outer_lock_subject(ctx):
    use_outer_actions(ctx.actions)
    return [DefaultInfo()]
outer_lock_subject = rule(implementation = _outer_lock_subject, subrules = [use_outer_actions])
def _return_ctx(ctx): return ctx
return_ctx = subrule(implementation = _return_ctx)
def _escaped_ctx_subject(ctx):
    escaped = return_ctx()
    value = escaped.label
    return [DefaultInfo()]
escaped_ctx_subject = rule(implementation = _escaped_ctx_subject, subrules = [return_ctx])
def _return_actions(ctx): return ctx.actions
return_actions = subrule(implementation = _return_actions)
def _escaped_actions_subject(ctx):
    escaped = return_actions()
    escaped.declare_file("forbidden.txt")
    return [DefaultInfo()]
escaped_actions_subject = rule(implementation = _escaped_actions_subject, subrules = [return_actions])
def _inspect_parent(ctx, parent): return parent.label
inspect_parent = subrule(implementation = _inspect_parent)
def _parent_lock(ctx): return inspect_parent(ctx)
parent_lock = subrule(implementation = _parent_lock, subrules = [inspect_parent])
def _parent_lock_subject(ctx):
    parent_lock()
    return [DefaultInfo()]
parent_lock_subject = rule(implementation = _parent_lock_subject, subrules = [parent_lock])
def _repeat_context(ctx, old = None):
    if old != None: return old.label
    return ctx
repeat_context = subrule(implementation = _repeat_context)
def _repeat_context_subject(ctx):
    old = repeat_context()
    repeat_context(old)
    return [DefaultInfo()]
repeat_context_subject = rule(implementation = _repeat_context_subject, subrules = [repeat_context])
def _fragment_call(ctx): return ctx.fragments.cpp
fragment_call = subrule(implementation = _fragment_call, fragments = ["cpp"])
def _fragment_subject(ctx):
    fragment_call()
    return [DefaultInfo()]
fragment_subject = rule(implementation = _fragment_subject, subrules = [fragment_call])
def _allowed_helper_subject(ctx):
    if allowed_mode(ctx.fragments.cpp) != "fastbuild": fail("allowed helper saw wrong mode")
    return [DefaultInfo()]
allowed_helper_subject = rule(implementation = _allowed_helper_subject, fragments = ["cpp"])
def _toolchain_call(ctx): return ctx.toolchains["//subject:type"]
toolchain_call = subrule(implementation = _toolchain_call)
def _toolchain_subject(ctx):
    toolchain_call()
    return [DefaultInfo()]
toolchain_subject = rule(implementation = _toolchain_subject, subrules = [toolchain_call])
def _missing_action_call(ctx): return ctx.actions.expand_template()
missing_action_call = subrule(implementation = _missing_action_call)
def _missing_action_subject(ctx):
    missing_action_call()
    return [DefaultInfo()]
missing_action_subject = rule(implementation = _missing_action_subject, subrules = [missing_action_call])
def _ordinary(ctx):
    if ctx.attr._libc_top.label.name != "everything": fail("ordinary target late-bound value was not materialized")
    return [DefaultInfo()]
ordinary = rule(implementation = _ordinary, attrs = {
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
    epoch.file(
        "/workspace/rules/coverage.bzl",
        r#"load("//rules:defs.bzl", "ProfileInfo")
OUTPUT_GENERATOR = configuration_field(fragment = "coverage", name = "output_generator")

def _coverage_sub(ctx, **kwargs):
    projected = ctx.fragments.coverage.output_generator
    dependency = kwargs["_generator"]
    if (dependency == None) != (projected == None): fail("subrule coverage option/dependency presence diverged")
    if dependency != None and not dependency.executable.path.endswith(projected.name): fail("subrule coverage executable diverged")
    return projected

coverage_sub = subrule(
    implementation = _coverage_sub,
    attrs = {"_generator": attr.label(
        default = OUTPUT_GENERATOR,
        allow_files = True,
        cfg = "exec",
        executable = True,
        providers = [[DefaultInfo], [ProfileInfo]],
    )},
    fragments = ["coverage"],
)

def _coverage(ctx):
    projected = ctx.fragments.coverage.output_generator
    dependency = ctx.attr._generator
    if (dependency == None) != (projected == None): fail("root coverage option/dependency presence diverged")
    if dependency != None and dependency.label != projected: fail("root coverage label diverged")
    if coverage_sub() != projected: fail("root and subrule coverage fragments diverged")
    return [DefaultInfo()]

coverage_rule = rule(
    implementation = _coverage,
    attrs = {"_generator": attr.label(
        default = OUTPUT_GENERATOR,
        allow_files = True,
    )},
    fragments = ["coverage"],
    subrules = [coverage_sub],
)

def _bad_file(ctx): return [DefaultInfo()]
bad_file = rule(implementation = _bad_file, attrs = {
    "_generator": attr.label(default = OUTPUT_GENERATOR, allow_files = [".prof"]),
})

def _bad_provider_sub(ctx, **kwargs): return None
bad_provider_sub = subrule(implementation = _bad_provider_sub, attrs = {
    "_generator": attr.label(default = OUTPUT_GENERATOR, allow_files = True, providers = [ProfileInfo]),
})
def _bad_provider(ctx):
    bad_provider_sub()
    return [DefaultInfo()]
bad_provider = rule(implementation = _bad_provider, subrules = [bad_provider_sub])

def _undeclared(ctx):
    value = ctx.fragments.coverage.output_generator
    return [DefaultInfo()]
undeclared = rule(implementation = _undeclared)

def _sub_undeclared(ctx): return ctx.fragments.coverage.output_generator
sub_undeclared = subrule(implementation = _sub_undeclared)
def _sub_undeclared_rule(ctx):
    sub_undeclared()
    return [DefaultInfo()]
sub_undeclared_rule = rule(implementation = _sub_undeclared_rule, subrules = [sub_undeclared])

def _escape(ctx): return ctx.fragments
escape = subrule(implementation = _escape, fragments = ["coverage"])
def _escaped(ctx):
    stale = escape()
    value = stale.coverage
    return [DefaultInfo()]
escaped = rule(implementation = _escaped, subrules = [escape])

def _unknown(ctx):
    value = ctx.fragments.coverage.unknown
    return [DefaultInfo()]
unknown = rule(implementation = _unknown, fragments = ["coverage"])
"#,
    );
    epoch.file(
        "/workspace/rules/denied_helper.bzl",
        "def denied_mode(cpp): return cpp.compilation_mode()\n",
    );
    epoch.package(
        "deps",
        "load('//rules:defs.bzl', 'generate', 'multi', 'plain', 'profile', 'tool')\nprofile(name='literal')\nprofile(name='other')\nprofile(name='profile')\nprofile(name='everything')\nplain(name='plain')\nmulti(name='multi')\ntool(name='tool')\ntool(name='lcov_merger')\ngenerate(name='generator', out='generated.bin')\nalias(name='alias_profile', actual=':literal')\n",
    );
    epoch.package(
        "files",
        "load('//rules:defs.bzl', 'declares')\ndeclares(name='owner', src='inferred.txt')\ndeclares(name='wrong_owner', src='wrong.bin')\nexports_files(['exported.sh'])\n",
    );
    epoch.file("/workspace/files/inferred.txt", "source");
    epoch.file("/workspace/files/wrong.bin", "wrong");
    epoch.file("/workspace/files/exported.sh", "#!/bin/sh\n");
    epoch.directory("/workspace/tools");
    epoch.directory("/workspace/tools/build_defs");
    epoch.package("tools/build_defs/cc", "");
    epoch.file(
        "/workspace/tools/build_defs/cc/helper.bzl",
        "def allowed_mode(cpp): return cpp.compilation_mode()\n",
    );
    epoch.file(
        "/workspace/tools/build_defs/cc/fragments.bzl",
        r#"load("//rules:denied_helper.bzl", "denied_mode")
def _sub(ctx):
    if dir(ctx.fragments) != ["cpp", "py"]: fail("subrule fragment dir mismatch: %s" % dir(ctx.fragments))
    return ctx.fragments.cpp
sub = subrule(implementation = _sub, fragments = ["cpp", "py"])
def _impl(ctx):
    expected = ["android", "apple", "bazel_android", "bazel_py", "coverage", "cpp", "j2objc", "java", "objc", "platform", "proto", "py"]
    if dir(ctx.fragments) != expected: fail("rule fragment dir mismatch: %s" % dir(ctx.fragments))
    cpp = ctx.fragments.cpp
    if cpp != sub(): fail("rule and subrule did not share the cpp fragment")
    if cpp.compilation_mode() != "fastbuild": fail("unexpected target compilation mode")
    if cpp.propeller_optimize_absolute_cc_profile() != None: fail("unexpected propeller cc path")
    if cpp.propeller_optimize_absolute_ld_profile() != None: fail("unexpected propeller ld path")
    if cpp.fdo_path() != None: fail("unexpected fdo path")
    if cpp.cs_fdo_path() != None: fail("unexpected cs fdo path")
    if cpp.proto_profile() != True: fail("unexpected proto profile")
    return [DefaultInfo()]
fragment_methods = rule(implementation = _impl, fragments = ["cpp"], subrules = [sub])
def _undeclared(ctx):
    value = ctx.fragments.cpp
    return [DefaultInfo()]
undeclared_fragment = rule(implementation = _undeclared)
def _sub_undeclared(ctx): return ctx.fragments.cpp
sub_undeclared = subrule(implementation = _sub_undeclared)
def _sub_undeclared_rule(ctx):
    sub_undeclared()
    return [DefaultInfo()]
undeclared_sub_fragment = rule(implementation = _sub_undeclared_rule, subrules = [sub_undeclared])
def _arity(ctx):
    ctx.fragments.cpp.compilation_mode("extra")
    return [DefaultInfo()]
fragment_arity = rule(implementation = _arity, fragments = ["cpp"])
def _denied_helper(ctx):
    denied_mode(ctx.fragments.cpp)
    return [DefaultInfo()]
denied_helper = rule(implementation = _denied_helper, fragments = ["cpp"])
def _create_fdo_context(ctx):
    cpp_config = ctx.fragments.cpp
    if cpp_config.compilation_mode() != "opt": return None
    cpp_config.propeller_optimize_absolute_cc_profile()
    cpp_config.propeller_optimize_absolute_ld_profile()
    cpp_config.fdo_path()
    cpp_config.cs_fdo_path()
    cpp_config.proto_profile()
    ctx.actions.args()
create_fdo_context = subrule(implementation = _create_fdo_context, fragments = ["cpp"])
def _opt(ctx):
    create_fdo_context()
    return [DefaultInfo()]
fragment_opt_terminal = rule(implementation = _opt, subrules = [create_fdo_context])
def _exec_probe(ctx, **kwargs): return None
exec_probe = subrule(implementation = _exec_probe, attrs = {
    "_probe": attr.label(default = "//subject:fragment_opt_terminal", cfg = "exec"),
})
def _exec_parent(ctx):
    exec_probe()
    return [DefaultInfo()]
fragment_exec_parent = rule(implementation = _exec_parent, subrules = [exec_probe])
"#,
    );
    epoch.package(
        "subject",
        "load('//rules:defs.bzl', 'allowed_helper_subject', 'bad_executable_subject', 'bad_extension_subject', 'bad_file_provider_subject', 'bad_file_subject', 'bad_nested_subject', 'bad_provider_subject', 'escaped_actions_subject', 'escaped_ctx_subject', 'exec_first_subject', 'fragment_subject', 'missing_action_subject', 'multi_single_subject', 'nested_subject', 'ordinary', 'outer_lock_subject', 'override_subject', 'parent_lock_subject', 'repeat_context_subject', 'subject', 'target_first_subject', 'toolchain_subject', 'undeclared_subject', 'zero_single_subject')\nload('//rules:coverage.bzl', 'bad_file', 'bad_provider', 'coverage_rule', 'escaped', 'sub_undeclared_rule', 'undeclared', 'unknown')\nload('//tools/build_defs/cc:fragments.bzl', 'denied_helper', 'fragment_arity', 'fragment_exec_parent', 'fragment_methods', 'fragment_opt_terminal', 'undeclared_fragment', 'undeclared_sub_fragment')\nsubject(name='subject')\nbad_provider_subject(name='bad_provider')\nzero_single_subject(name='zero_single')\nmulti_single_subject(name='multi_single')\nbad_extension_subject(name='bad_extension')\nbad_executable_subject(name='bad_executable')\nbad_file_subject(name='bad_file')\nbad_file_provider_subject(name='bad_file_provider')\nexec_first_subject(name='exec_first')\ntarget_first_subject(name='target_first')\nnested_subject(name='nested')\nundeclared_subject(name='undeclared')\nbad_nested_subject(name='bad_nested')\noverride_subject(name='override')\nouter_lock_subject(name='outer_lock')\nescaped_ctx_subject(name='escaped_ctx')\nescaped_actions_subject(name='escaped_actions')\nparent_lock_subject(name='parent_lock')\nrepeat_context_subject(name='repeat_context')\nfragment_subject(name='fragment')\nallowed_helper_subject(name='allowed_helper')\nfragment_methods(name='fragment_methods')\ndenied_helper(name='denied_helper')\nundeclared_fragment(name='undeclared_fragment')\nundeclared_sub_fragment(name='undeclared_sub_fragment')\nfragment_arity(name='fragment_arity')\nfragment_opt_terminal(name='fragment_opt_terminal')\nfragment_exec_parent(name='fragment_exec_parent')\ntoolchain_subject(name='toolchain_deferred')\nmissing_action_subject(name='missing_action')\nordinary(name='ordinary')\ncoverage_rule(name='coverage')\nbad_file(name='coverage_bad_file')\nbad_provider(name='coverage_bad_provider')\nundeclared(name='coverage_undeclared')\nsub_undeclared_rule(name='coverage_sub_undeclared')\nescaped(name='coverage_escaped')\nunknown(name='coverage_unknown')\n",
    );
    epoch
}

fn high_count_invocation_epoch(count: usize) -> Epoch {
    let mut defs = String::from("def _call(ctx, value): return value\n");
    for index in 0..count {
        defs.push_str(&format!("call_{index} = subrule(implementation = _call)\n"));
    }
    defs.push_str("def _subject(ctx):\n");
    for index in 0..count {
        defs.push_str(&format!(
            "    if call_{index}({index}) != {index}: fail(\"high-count call {index} failed\")\n"
        ));
    }
    defs.push_str("    return [DefaultInfo()]\n");
    defs.push_str("subject = rule(implementation = _subject, subrules = [");
    for index in 0..count {
        if index != 0 {
            defs.push_str(", ");
        }
        defs.push_str(&format!("call_{index}"));
    }
    defs.push_str("])\n");

    let mut epoch = Epoch::fixture();
    epoch.package("rules", "");
    epoch.file("/workspace/rules/high_count.bzl", &defs);
    epoch.package(
        "subject",
        "load('//rules:high_count.bzl', 'subject')\nsubject(name='many')\n",
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
def _subject(ctx): return [DefaultInfo()]
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

fn invocation_revision_epoch(fail: bool) -> Epoch {
    let terminal = if fail {
        "fail(\"revision B\")"
    } else {
        "return out"
    };
    let defs = format!(
        r#"def _call(ctx):
    out = ctx.actions.declare_file("revision.txt")
    ctx.actions.write(out, "revision")
    {terminal}
call = subrule(implementation = _call)
def _subject(ctx):
    call()
    return [DefaultInfo()]
subject = rule(implementation = _subject, subrules = [call])
"#,
    );
    let mut epoch = Epoch::fixture();
    epoch.package("rules", "");
    epoch.file("/workspace/rules/revision.bzl", &defs);
    epoch.package(
        "subject",
        "load('//rules:revision.bzl', 'subject')\nsubject(name='revision')\n",
    );
    epoch
}

#[tokio::test]
async fn configured_subrule_dependencies_materialize_and_invoke() {
    let epoch = semantic_epoch();
    let tracker = Arc::new(Tracker::default());
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let result = analyze_result(
        &dice,
        epoch.build(),
        "@@//subject:subject",
        configuration(Some("//deps:profile")),
        tracker.clone(),
        AnalysisRoute::Legacy,
    )
    .await
    .unwrap();
    assert_eq!(result.actions().len(), 1);
    assert_eq!(
        result.actions()[0].context().owner().label().to_string(),
        "@@//subject:subject"
    );
    assert_eq!(result.providers().len(), 2);
    let returned = result
        .providers()
        .user(&ProviderId::new("//rules:defs.bzl", "ReturnedInfo").unwrap())
        .expect("subrule result provider was lowered");
    for (field, expected_path, null_identity) in [
        ("source_target", "files/inferred.txt", true),
        ("generated_target", "deps/generated.bin", false),
    ] {
        let AnalysisValueKind::ConfiguredTarget(target) =
            returned.field(field).expect("returned Target field").kind()
        else {
            panic!("{field} retained its Target shape")
        };
        assert_eq!(
            matches!(
                target.identity(),
                slug_build_api_v2::AnalysisTargetIdentity::Null(_)
            ),
            null_identity,
            "{field} identity"
        );
        let files = target
            .providers()
            .default_info()
            .expect("file Target retained materialized DefaultInfo")
            .file_artifacts();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path().as_ref(), expected_path);
    }
    let implicit = result
        .edges()
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind(),
                slug_analysis_v2::ConfiguredEdgeKind::ImplicitAttribute { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(implicit.len(), 13);
    assert_eq!(
        implicit
            .iter()
            .map(|edge| edge.target().label().to_string())
            .collect::<Vec<_>>(),
        [
            "@@//deps:literal",
            "@@//deps:literal",
            "@@//deps:other",
            "@@//files:inferred.txt",
            "@@//files:inferred.txt",
            "@@//files:exported.sh",
            "@@//deps:tool",
            "@@//deps:profile",
            "@@//deps:plain",
            "@@//deps:plain",
            "@@//deps:generated.bin",
            "@@//deps:generated.bin",
            "@@//deps:alias_profile",
        ]
    );
    assert_eq!(implicit.iter().filter(|edge| edge.tool()).count(), 2);
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
async fn nested_authorization_overrides_and_context_lifetimes_are_enforced() {
    let nested = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:nested",
        configuration(None),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(nested.is_ok(), "{nested:?}");

    let fragment = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment",
        configuration(None),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(fragment.is_ok(), "{fragment:?}");

    for (target, expected) in [
        ("undeclared", "rule must declare 'rogue' in 'subrules'"),
        (
            "bad_nested",
            "subrule bad_parent must declare rogue in 'subrules'",
        ),
        ("override", "implicit dependency and cannot be overridden"),
        (
            "outer_lock",
            "cannot access field or method 'declare_file' of rule context",
        ),
        (
            "escaped_ctx",
            "cannot access field or method 'label' of subrule context",
        ),
        (
            "escaped_actions",
            "cannot access field or method 'declare_file' of subrule context",
        ),
        (
            "parent_lock",
            "cannot access field or method 'label' of subrule context",
        ),
        (
            "repeat_context",
            "cannot access field or method 'label' of subrule context",
        ),
        (
            "toolchain_deferred",
            "configured subrule toolchains are deferred",
        ),
        ("missing_action", "has no attribute `expand_template`"),
    ] {
        let error = analyze(
            &Dice::builder().build(DetectCycles::Enabled),
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            configuration(None),
            Arc::new(Tracker::default()),
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(error.contains(expected), "{target}: {error}");
    }
}

#[tokio::test]
async fn configured_fragment_facades_project_methods_and_fail_closed() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        for target in ["fragment_methods", "allowed_helper"] {
            let result = analyze_result(
                &Dice::builder().build(DetectCycles::Enabled),
                semantic_epoch().build(),
                &format!("@@//subject:{target}"),
                configuration(None),
                Arc::new(Tracker::default()),
                route,
            )
            .await;
            assert!(result.is_ok(), "{route:?}/{target}: {result:?}");
        }
    }

    for (target, expected) in [
        (
            "undeclared_fragment",
            "has to declare 'cpp' as a required fragment",
        ),
        ("undeclared_sub_fragment", "has no attribute `cpp`"),
        ("fragment_arity", "expected 0, got 1"),
        ("denied_helper", "cannot use private API"),
    ] {
        let error = analyze(
            &Dice::builder().build(DetectCycles::Enabled),
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            configuration(None),
            Arc::new(Tracker::default()),
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(error.contains(expected), "{target}: {error}");
    }

    let target_opt = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment_opt_terminal",
        compilation_configuration("opt", "fastbuild"),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(target_opt.is_ok(), "{target_opt:?}");

    let target_dbg = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment_opt_terminal",
        compilation_configuration("dbg", "opt"),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(target_dbg.is_ok(), "{target_dbg:?}");

    let exec_default = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment_exec_parent",
        compilation_configuration("fastbuild", "opt"),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(exec_default.is_ok(), "{exec_default:?}");

    let target_dbg_exec_opt = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment_exec_parent",
        compilation_configuration("dbg", "opt"),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(target_dbg_exec_opt.is_ok(), "{target_dbg_exec_opt:?}");

    let exec_fastbuild = analyze_result(
        &Dice::builder().build(DetectCycles::Enabled),
        semantic_epoch().build(),
        "@@//subject:fragment_exec_parent",
        compilation_configuration("opt", "fastbuild"),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(exec_fastbuild.is_ok(), "{exec_fastbuild:?}");
}

#[tokio::test]
async fn coverage_field_and_public_facades_restore_false_true_false_in_one_dice() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let disabled = analyze_result(
            &dice,
            semantic_epoch().build(),
            "@@//subject:coverage",
            coverage_configuration(false, None),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        assert!(
            disabled
                .edges()
                .iter()
                .all(|edge| edge.target().label().target().as_str() != "lcov_merger"),
            "{route:?}: disabled coverage edge"
        );

        let enabled = analyze_result(
            &dice,
            semantic_epoch().build(),
            "@@//subject:coverage",
            coverage_configuration(true, Some("//deps:lcov_merger")),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        let coverage_edges = enabled
            .edges()
            .iter()
            .filter(|edge| edge.target().label().target().as_str() == "lcov_merger")
            .collect::<Vec<_>>();
        assert_eq!(coverage_edges.len(), 2, "{route:?}");
        assert_eq!(coverage_edges.iter().filter(|edge| edge.tool()).count(), 1);

        let restored = analyze_result(
            &dice,
            semantic_epoch().build(),
            "@@//subject:coverage",
            coverage_configuration(false, None),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        assert!(Arc::ptr_eq(&disabled, &restored), "{route:?}");

        let explicit = analyze_result(
            &dice,
            semantic_epoch().build(),
            "@@//subject:coverage",
            coverage_configuration(true, Some("//deps:tool")),
            tracker,
            route,
        )
        .await
        .unwrap();
        let explicit_edges = explicit
            .edges()
            .iter()
            .filter(|edge| edge.target().label().to_string() == "@@//deps:tool")
            .collect::<Vec<_>>();
        assert_eq!(explicit_edges.len(), 2, "{route:?}");
        assert_eq!(explicit_edges.iter().filter(|edge| edge.tool()).count(), 1);
    }
}

#[tokio::test]
async fn coverage_field_omission_and_shared_dependency_validation_fail_closed() {
    let restoration_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let first = analyze_result(
        &restoration_dice,
        semantic_epoch().build(),
        "@@//subject:coverage_bad_file",
        coverage_configuration(false, None),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await
    .unwrap();
    let error = analyze(
        &restoration_dice,
        semantic_epoch().build(),
        "@@//subject:coverage_bad_file",
        coverage_configuration(true, Some("//deps:lcov_merger")),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(
        error.contains("does not produce any file matching its admitted file types"),
        "{error}"
    );
    let restored = analyze_result(
        &restoration_dice,
        semantic_epoch().build(),
        "@@//subject:coverage_bad_file",
        coverage_configuration(false, None),
        Arc::new(Tracker::default()),
        AnalysisRoute::Legacy,
    )
    .await
    .unwrap();
    assert!(Arc::ptr_eq(&first, &restored));

    for target in ["coverage_bad_provider"] {
        let disabled = analyze_result(
            &Dice::builder().build(DetectCycles::Enabled),
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            coverage_configuration(false, None),
            Arc::new(Tracker::default()),
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(disabled.is_ok(), "{target}: {disabled:?}");
    }

    for (target, configuration, expected) in [
        (
            "coverage_bad_file",
            coverage_configuration(true, Some("//deps:lcov_merger")),
            "does not produce any file matching its admitted file types",
        ),
        (
            "coverage_bad_provider",
            coverage_configuration(true, Some("//deps:plain")),
            "does not provide any admitted provider alternative",
        ),
        (
            "coverage",
            coverage_configuration(true, Some("//deps:plain")),
            "is not executable",
        ),
        (
            "coverage_undeclared",
            coverage_configuration(false, None),
            "has to declare 'coverage' as a required fragment",
        ),
        (
            "coverage_sub_undeclared",
            coverage_configuration(false, None),
            "has no attribute `coverage`",
        ),
        (
            "coverage_escaped",
            coverage_configuration(false, None),
            "has no attribute `coverage`",
        ),
        (
            "coverage_unknown",
            coverage_configuration(false, None),
            "has no attribute `unknown`",
        ),
    ] {
        let error = analyze(
            &Dice::builder().build(DetectCycles::Enabled),
            semantic_epoch().build(),
            &format!("@@//subject:{target}"),
            configuration,
            Arc::new(Tracker::default()),
            AnalysisRoute::Legacy,
        )
        .await;
        assert!(error.contains(expected), "{target}: {error}");
    }
}

#[tokio::test]
async fn high_count_nonrecursive_subrule_calls_share_one_dispatch_payload() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let result = analyze_result(
            &Dice::builder().build(DetectCycles::Enabled),
            high_count_invocation_epoch(256).build(),
            "@@//subject:many",
            configuration(None),
            Arc::new(Tracker::default()),
            route,
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
    }
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
        ("bad_extension", "does not match its admitted file types"),
        ("bad_executable", "is not executable"),
        ("bad_file", "does not match its admitted file types"),
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
    let result = analyze_result(
        &dice,
        semantic_epoch().build(),
        "@@//subject:ordinary",
        grte_configuration("//deps:profile"),
        tracker.clone(),
        AnalysisRoute::Legacy,
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
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

            let repaired = analyze_result(
                &dice,
                cycle_epoch(true).build(),
                &format!("@@//subject:{target}"),
                configuration(None),
                Arc::new(Tracker::default()),
                route,
            )
            .await;
            assert!(repaired.is_ok(), "{repaired:?}");

            let cyclic_again = analyze(
                &dice,
                cycle_epoch(false).build(),
                &format!("@@//subject:{target}"),
                configuration(None),
                Arc::new(Tracker::default()),
                route,
            )
            .await;
            assert!(
                cyclic_again.contains("configured alias cycle"),
                "{cyclic_again}"
            );
        }
    }
}

#[tokio::test]
async fn invocation_state_and_actions_restore_across_same_dice_a_b_a() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let first = analyze_result(
            &dice,
            invocation_revision_epoch(false).build(),
            "@@//subject:revision",
            configuration(None),
            Arc::new(Tracker::default()),
            route,
        )
        .await
        .unwrap();
        assert_eq!(first.actions().len(), 1);

        let error = analyze(
            &dice,
            invocation_revision_epoch(true).build(),
            "@@//subject:revision",
            configuration(None),
            Arc::new(Tracker::default()),
            route,
        )
        .await;
        assert!(error.contains("revision B"), "{error}");

        let restored = analyze_result(
            &dice,
            invocation_revision_epoch(false).build(),
            "@@//subject:revision",
            configuration(None),
            Arc::new(Tracker::default()),
            route,
        )
        .await
        .unwrap();
        assert_eq!(restored.actions().len(), 1);
    }
}
