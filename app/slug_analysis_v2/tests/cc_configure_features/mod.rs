//! Real DICE ctx + verbatim creator/legacy generator/public wrapper. The small
//! authored capability/FDO envelope is not production toolchain construction.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::cc_features::CcFeatureConfiguration;

mod context_boundary;

use super::*;

macro_rules! source {
    ($path:literal) => {
        (
            $path,
            include_str!(concat!("../fixtures/rustc-map-each/", $path)),
        )
    };
}
const SOURCES: &[(&str, &str)] = &[
    source!("rules_cc/cc/action_names.bzl"),
    source!("rules_cc/cc/cc_toolchain_config_lib.bzl"),
    source!("rules_cc/cc/common/semantics.bzl"),
    source!("rules_cc/cc/private/cc_internal.bzl"),
    source!("rules_cc/cc/private/toolchain_config/cc_toolchain_config_info.bzl"),
    source!("rules_cc/cc/private/toolchain_config/legacy_features.bzl"),
    source!("rules_cc/cc/private/toolchain_config/configure_features.bzl"),
    source!("bazel_skylib/lib/paths.bzl"),
];
const REPOS: &[(&str, &str)] = &[
    ("rules_cc+", "rules_cc"),
    ("bazel_skylib+", "bazel_skylib"),
    ("platforms", "platforms"),
];
const SUBJECT: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'tool_path')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')
load('//cc/private/toolchain_config:configure_features.bzl', 'configure_features')
Proof = provider(fields = ['config', 'model', 'os'])
NAMES = [
    'fastbuild', 'dbg', 'opt', 'supports_pic', 'generate_dsym_file', 'no_generate_debug_symbols',
    'host', 'nonhost', 'coverage', 'llvm_coverage_map_format', 'gcc_coverage_map_format',
    'fdo_instrument', 'cs_fdo_instrument', 'fdo_optimize', 'cs_fdo_optimize',
    'enable_fdo_memprof_optimize', 'enable_fdo_thinlto', 'enable_fdo_split_functions',
    'autofdo', 'enable_autofdo_memprof_optimize', 'enable_afdo_thinlto', 'enable_fsafdo',
    'xbinaryfdo', 'enable_xbinaryfdo_thinlto', 'fdo_prefetch_hints', 'propeller_optimize',
    'repo_on', 'package_on', 'rule_on', 'global_on', 'blocked', 'selected_opt',
]
def _impl(ctx):
    enabled = ctx.features
    disabled = ctx.disabled_features
    enabled.append('mutated')
    disabled.append('mutated')
    if 'mutated' in ctx.features or 'mutated' in ctx.disabled_features:
        fail('feature access did not return fresh mutable copies')
    defs = [feature(name = n) for n in NAMES if not (ctx.attr.omit_pic and n == 'supports_pic')]
    if ctx.attr.omit_pic:
        defs.append(feature(name = 'pic'))  # Suppress the legacy enabled pic default.
    defs += [feature(name = 'probe_default', enabled = True),
             feature(name = 'parse_headers', enabled = True),
             feature(name = 'module_maps', enabled = True),
             feature(name = 'force_bad', implies = ['blocked'])]
    info = create_cc_toolchain_config_info(
        ctx = ctx, toolchain_identifier = 'proof', compiler = 'gcc', target_libc = 'local',
        features = defs, tool_paths = [tool_path(name = n, path = 'bin/' + n) for n in ['gcc', 'ar', 'strip']],
    )
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = '.')
    profile = struct(branch_fdo_mode = ctx.attr.profile) if ctx.attr.profile else None
    propeller = struct(cc_profile = 'authored-profile-input', ld_profile = None) if ctx.attr.propeller else None
    envelope = struct(
        _supports_header_parsing = ctx.attr.headers,
        _cc_info = struct(compilation_context = struct(_module_map = 'authored-module-map' if ctx.attr.module_map else None)),
        _toolchain_features = model, _is_tool_configuration = ctx.attr.tool,
        _toolchain_label = ctx.label,
        _fdo_context = struct(branch_fdo_profile = profile, propeller_optimize_info = propeller),
    )
    configured = configure_features(ctx = ctx, cc_toolchain = envelope,
        requested_features = ctx.features + ctx.attr.requested,
        unsupported_features = ctx.disabled_features + ctx.attr.unsupported)
    return [Proof(config = configured, model = model, os = cc_internal.exec_os(ctx))]
subject = rule(implementation = _impl, fragments = ['cpp'], attrs = {
    'compiler': attr.string(default = 'gcc'), 'profile': attr.string(),
    'propeller': attr.bool(), 'tool': attr.bool(), 'headers': attr.bool(),
    'module_map': attr.bool(), 'omit_pic': attr.bool(),
    'requested': attr.string_list(), 'unsupported': attr.string_list(),
})
"#;

fn workspace(targets: &str) -> PathBuf {
    workspace_with(targets, SUBJECT, &[])
}

pub(super) fn workspace_with(
    targets: &str,
    subject: &str,
    extra_sources: &[(&str, &str)],
) -> PathBuf {
    let root = scratch();
    let mut module = "module(name='root')\n".to_owned();
    for (_, name) in REPOS {
        module.push_str(&format!("bazel_dep(name='{name}',version='1.0')\nlocal_path_override(module_name='{name}',path='{name}')\n"));
        fs::create_dir_all(root.join(name)).unwrap();
        fs::write(root.join(name).join("REPO.bazel"), "").unwrap();
        fs::write(root.join(name).join(".bazelignore"), "").unwrap();
        let dependencies = if *name == "rules_cc" {
            "bazel_dep(name='bazel_skylib',version='1.0')\nbazel_dep(name='platforms',version='1.0')\n"
        } else {
            ""
        };
        fs::write(
            root.join(name).join("MODULE.bazel"),
            format!("module(name='{name}',version='1.0')\n{dependencies}"),
        )
        .unwrap();
    }
    fs::write(root.join("MODULE.bazel"), module).unwrap();
    for (path, body) in SOURCES.iter().chain(extra_sources.iter()) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path.parent().unwrap().join("BUILD.bazel"), "").unwrap();
        fs::write(path, body).unwrap();
    }
    fs::write(root.join("rules_cc/cc/private/proof.bzl"), subject).unwrap();
    fs::create_dir_all(root.join("platforms/os")).unwrap();
    fs::write(root.join("platforms/os/BUILD.bazel"), "constraint_setting(name='os')\nconstraint_value(name='linux',constraint_setting=':os')\nconstraint_value(name='macos',constraint_setting=':os')\nconstraint_value(name='mystery',constraint_setting=':os')\n").unwrap();
    build(&root, targets);
    root
}

fn build(root: &std::path::Path, targets: &str) {
    // Explicit canonical labels test ConstraintConstants identity, not the
    // older root apparent-repository mapping's platforms+ spelling.
    fs::write(root.join("BUILD.bazel"), "platform(name='target_linux',constraint_values=['@@platforms//os:linux'])\nplatform(name='remote',constraint_values=['@@platforms//os:macos'])\nplatform(name='unknown',constraint_values=['@@platforms//os:mystery'])\n").unwrap();
    fs::write(
        root.join("rules_cc/cc/private/BUILD.bazel"),
        format!("load(':proof.bzl','subject')\n{targets}\n"),
    )
    .unwrap();
}

pub(super) fn config(options: &[(NativeCommandOption, &str)]) -> ConfigurationKey {
    let base = typed_action_test_configuration()
        .slug_configuration()
        .unwrap()
        .clone();
    let overlay: CommandConfigurationOverlay = options
        .iter()
        .map(|(option, value)| CommandConfigurationOccurrence::native(*option, Some(*value), false))
        .collect::<Vec<_>>()
        .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

pub(super) async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    name: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let epoch = root_epoch(root);
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(analysis_cycle_detector()),
        ..Default::default()
    });
    // Bootstrap's generic route still demands platforms+; selected canonical
    // inventory demands platforms. Supply both actual request identities for
    // the declared local override without rewriting configured constraint labels.
    let mut repositories = REPOS.to_vec();
    repositories.push(("platforms+", "platforms"));
    inject_root_target_inputs(&mut updater, root, epoch, &repositories);
    let mut tx = updater.commit().await;
    let key = match slug_analysis_v2::prepare_configured_node_analysis_observed(
        &mut tx,
        NormalizedAbsolutePath::new(root.to_path_buf()).unwrap(),
        CanonicalLabel::parse(&format!("@@rules_cc+//cc/private:{name}")).unwrap(),
        configuration,
    )
    .await
    {
        AnalysisPreparationOutcome::Complete(Ok(Ok(key))) => key,
        other => {
            return Err(format!(
                "observed feature configuration preparation: {other:?}"
            ));
        }
    };
    match tx.compute(&key).await.map_err(|error| error.to_string())? {
        AnalysisPreparationOutcome::Complete(Ok(result)) => result
            .as_ref()
            .as_ref()
            .cloned()
            .map_err(ToString::to_string),
        other => Err(format!("observed feature configuration: {other:?}")),
    }
}

pub(super) fn field<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a AnalysisValue {
    result
        .providers()
        .occurrences()
        .find_map(|provider| provider.field(name))
        .unwrap()
}

fn selection(result: &ConfiguredNodeResult) -> &CcFeatureConfiguration {
    let AnalysisValueKind::CcFeatureConfiguration(value) = field(result, "config").kind() else {
        panic!("missing retained native selection")
    };
    value
}

fn enabled(result: &ConfiguredNodeResult, yes: &[&str], no: &[&str]) {
    for name in yes {
        assert!(
            selection(result).is_enabled(name),
            "expected enabled {name}"
        );
    }
    for name in no {
        assert!(
            !selection(result).is_enabled(name),
            "expected disabled {name}"
        );
    }
}

#[tokio::test]
async fn authentic_creator_legacy_and_public_wrapper_use_real_ctx() {
    let root = workspace("subject(name='subject',requested=['unknown_request'])");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, "subject", config(&[])).await.unwrap();
    enabled(
        &result,
        &["fastbuild", "probe_default", "no_generate_debug_symbols"],
        &[
            "parse_headers",
            "module_maps",
            "coverage",
            "unknown_request",
        ],
    );
    assert!(selection(&result).is_requested("unknown_request"));
    let AnalysisValueKind::CcToolchainFeatures(model) = field(&result, "model").kind() else {
        panic!("missing native model")
    };
    assert!(!model.model().action_configs.is_empty());
    assert!(
        model
            .model()
            .features
            .iter()
            .any(|feature| feature.name == "legacy_compile_flags")
    );
    assert!(
        matches!(field(&result, "os").kind(), AnalysisValueKind::String(value) if value == "LINUX")
    );
    assert!(result.actions().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_wrapper_configuration_changes_and_restores_in_one_dice() {
    let root = workspace(
        "subject(name='subject',headers=True,module_map=True)\nsubject(name='tool',tool=True)",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = request(&dice, &root, "subject", config(&[])).await.unwrap();
    let changed = request(
        &dice,
        &root,
        "subject",
        config(&[
            (NativeCommandOption::CompilationMode, "opt"),
            (NativeCommandOption::CollectCodeCoverage, "true"),
            (NativeCommandOption::ExperimentalUseLlvmCovmap, "true"),
            (NativeCommandOption::ForcePic, "true"),
            (NativeCommandOption::AppleGenerateDsym, "true"),
            (
                NativeCommandOption::IncompatibleDontEnableHostNonhostCrosstoolFeatures,
                "false",
            ),
            (NativeCommandOption::FdoInstrument, "instrumentation"),
            (NativeCommandOption::FdoPrefetchHints, "//:hints"),
        ]),
    )
    .await
    .unwrap();
    enabled(
        &changed,
        &[
            "opt",
            "coverage",
            "llvm_coverage_map_format",
            "supports_pic",
            "generate_dsym_file",
            "nonhost",
            "fdo_instrument",
            "fdo_prefetch_hints",
            "parse_headers",
            "module_maps",
        ],
        &[
            "fastbuild",
            "gcc_coverage_map_format",
            "no_generate_debug_symbols",
            "cs_fdo_instrument",
        ],
    );
    let restored = request(&dice, &root, "subject", config(&[])).await.unwrap();
    assert_eq!(base, restored);
    assert_ne!(base, changed);
    let gcc = request(
        &dice,
        &root,
        "tool",
        config(&[
            (NativeCommandOption::CollectCodeCoverage, "true"),
            (NativeCommandOption::CsFdoInstrument, "instrumentation"),
            (
                NativeCommandOption::IncompatibleDontEnableHostNonhostCrosstoolFeatures,
                "false",
            ),
        ]),
    )
    .await
    .unwrap();
    enabled(
        &gcc,
        &[
            "host",
            "coverage",
            "gcc_coverage_map_format",
            "cs_fdo_instrument",
        ],
        &["nonhost", "llvm_coverage_map_format", "fdo_instrument"],
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_wrapper_profile_gates_are_source_derived() {
    let root = workspace(
        "subject(name='llvm',profile='llvm_fdo')\nsubject(name='cs',profile='llvm_cs_fdo')\nsubject(name='auto',profile='auto_fdo')\nsubject(name='xbinary',profile='xbinary_fdo')\nsubject(name='propeller',profile='llvm_fdo',propeller=True)\nsubject(name='blocked',profile='auto_fdo',unsupported=['fsafdo','thin_lto','memprof_optimize'])",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let opt = config(&[(NativeCommandOption::CompilationMode, "opt")]);
    for (name, yes, no) in [
        (
            "llvm",
            vec![
                "fdo_optimize",
                "enable_fdo_memprof_optimize",
                "enable_fdo_thinlto",
                "enable_fdo_split_functions",
            ],
            vec!["cs_fdo_optimize"],
        ),
        (
            "cs",
            vec!["fdo_optimize", "cs_fdo_optimize"],
            vec!["autofdo"],
        ),
        (
            "auto",
            vec![
                "autofdo",
                "enable_autofdo_memprof_optimize",
                "enable_afdo_thinlto",
                "enable_fsafdo",
                "enable_fdo_split_functions",
            ],
            vec!["fdo_optimize"],
        ),
        (
            "xbinary",
            vec!["xbinaryfdo", "enable_xbinaryfdo_thinlto"],
            vec!["fdo_optimize"],
        ),
        (
            "propeller",
            vec!["fdo_optimize", "propeller_optimize"],
            vec!["enable_fdo_split_functions"],
        ),
        (
            "blocked",
            vec!["autofdo"],
            vec![
                "enable_fsafdo",
                "enable_fdo_split_functions",
                "enable_afdo_thinlto",
                "enable_autofdo_memprof_optimize",
            ],
        ),
    ] {
        let result = request(&dice, &root, name, opt.clone()).await.unwrap();
        enabled(&result, &yes, &no);
    }
    let nonopt = request(&dice, &root, "llvm", config(&[])).await.unwrap();
    enabled(&nonopt, &["fastbuild"], &["fdo_optimize"]);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_wrapper_rejects_unsupported_implied_and_pic_features() {
    let root = workspace(
        "subject(name='implied',requested=['force_bad'],unsupported=['blocked'])\nsubject(name='early',unsupported=['supports_pic'])\nsubject(name='late',omit_pic=True)\nsubject(name='instrument',unsupported=['fdo_instrument'])",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    assert!(
        request(&dice, &root, "implied", config(&[]))
            .await
            .unwrap_err()
            .contains("unconditionally implies feature 'blocked'")
    );
    for name in ["early", "late"] {
        assert!(
            request(
                &dice,
                &root,
                name,
                config(&[(NativeCommandOption::ForcePic, "true")])
            )
            .await
            .unwrap_err()
            .contains("PIC compilation is requested")
        );
    }
    let result = request(
        &dice,
        &root,
        "instrument",
        config(&[(NativeCommandOption::CsFdoInstrument, "instrumentation")]),
    )
    .await
    .unwrap();
    enabled(&result, &[], &["fdo_instrument", "cs_fdo_instrument"]);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_wrapper_repo_package_rule_global_and_select_features_restore() {
    let targets = "package(features=['-repo_on','package_on','-blocked'])\nconfig_setting(name='opt',values={'compilation_mode':'opt'})\nsubject(name='subject',features=['repo_on','rule_on','blocked']+select({':opt':['selected_opt'],'//conditions:default':[]}))";
    let root = workspace(targets);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = config(&[
        (NativeCommandOption::Features, "global_on"),
        (NativeCommandOption::Features, "-blocked"),
        (NativeCommandOption::CompilationMode, "opt"),
    ]);
    fs::write(
        root.join("rules_cc/REPO.bazel"),
        "repo(features=['repo_on','blocked'])\n",
    )
    .unwrap();
    let first = request(&dice, &root, "subject", configuration.clone())
        .await
        .unwrap();
    enabled(
        &first,
        &[
            "repo_on",
            "package_on",
            "rule_on",
            "global_on",
            "selected_opt",
        ],
        &["blocked"],
    );
    // Only REPO changes; unchanged BUILD still must observe a new semantic default.
    fs::write(
        root.join("rules_cc/REPO.bazel"),
        "repo(features=['repo_on','blocked','probe_repo_change'])\n",
    )
    .unwrap();
    let changed = request(&dice, &root, "subject", configuration.clone())
        .await
        .unwrap();
    assert!(selection(&changed).is_requested("probe_repo_change"));
    assert!(!selection(&first).is_requested("probe_repo_change"));
    fs::write(
        root.join("rules_cc/REPO.bazel"),
        "repo(features=['repo_on','blocked'])\n",
    )
    .unwrap();
    let restored = request(&dice, &root, "subject", configuration)
        .await
        .unwrap();
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_wrapper_exec_os_uses_selected_platform_not_host_or_target() {
    let root = workspace("subject(name='subject')");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let platform_config = |label: &str| {
        ConfigurationKey::from_slug(
            config(&[])
                .slug_configuration()
                .unwrap()
                .with_host_platform_label(&CanonicalLabel::parse(label).unwrap())
                .with_transition_target_platform(Some(
                    &CanonicalLabel::parse("@@//:target_linux").unwrap(),
                ))
                .unwrap(),
        )
    };
    let darwin = request(&dice, &root, "subject", platform_config("@@//:remote"))
        .await
        .unwrap();
    assert!(
        matches!(field(&darwin, "os").kind(), AnalysisValueKind::String(value) if value == "DARWIN")
    );
    let unknown = request(&dice, &root, "subject", platform_config("@@//:unknown"))
        .await
        .unwrap_err();
    assert!(unknown.contains("UNKNOWN"), "{unknown}"); // WP758 deliberately rejects this constructor OS.
    let restored = request(&dice, &root, "subject", config(&[])).await.unwrap();
    assert!(
        matches!(field(&restored, "os").kind(), AnalysisValueKind::String(value) if value == "LINUX")
    );
    fs::remove_dir_all(root).unwrap();
}
