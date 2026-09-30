//! Verbatim compile-variable builder after the authentic creator/legacy/public
//! feature wrapper. The toolchain/FDO envelope and feature declarations below
//! are authored boundary inputs, not production cc_toolchain construction.
use slug_build_api_v2::AnalysisArtifact;

use super::cc_configure_features::config;
use super::cc_configure_features::field;
use super::cc_configure_features::request;
use super::cc_configure_features::workspace_with;
use super::*;

mod cache_boundary;
mod link_args;
mod link_variables;

macro_rules! source {
    ($path:literal) => {
        (
            $path,
            include_str!(concat!("../fixtures/rustc-map-each/", $path)),
        )
    };
}
const SOURCES: &[(&str, &str)] = &[
    source!("rules_cc/cc/private/compile/compile_build_variables.bzl"),
    source!("rules_cc/cc/common/cc_helper_internal.bzl"),
    source!("rules_cc/cc/private/paths.bzl"),
    source!("rules_cc/cc/private/rules_impl/native_cc_common.bzl"),
];
const SUBJECT: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'tool_path', 'action_config', 'tool', 'with_feature_set', 'flag_set', 'flag_group', 'env_set', 'env_entry')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')
load('//cc/private/toolchain_config:configure_features.bzl', 'configure_features')
load('//cc/private/compile:compile_build_variables.bzl', 'create_compile_variables')

ExpansionInfo = provider(fields = ['variables', 'configuration', 'source', 'argv', 'env', 'tool', 'requirements', 'enabled'])
ACTION = 'c++-compile'

def _query(variables, configuration, source):
    return ExpansionInfo(
        variables = variables, configuration = configuration, source = source,
        argv = cc_common.get_memory_inefficient_command_line(feature_configuration = configuration, action_name = ACTION, variables = variables),
        env = cc_common.get_environment_variables(feature_configuration = configuration, action_name = ACTION, variables = variables),
        tool = cc_common.get_tool_for_action(feature_configuration = configuration, action_name = ACTION),
        requirements = cc_common.get_execution_requirements(feature_configuration = configuration, action_name = ACTION),
        enabled = cc_common.action_is_enabled(feature_configuration = configuration, action_name = ACTION),
    )

def _impl(ctx):
    if ctx.attr.dep:
        data = ctx.attr.dep[ExpansionInfo]
        return [_query(data.variables, data.configuration, data.source)]
    source = ctx.attr.src[DefaultInfo].files.to_list()[0]
    defs = [
        feature(name = 'fast', flag_sets = [flag_set(actions = [ACTION], flag_groups = [flag_group(flags = ['--fast'])])]),
        feature(name = 'probe', enabled = True,
            flag_sets = [flag_set(actions = [ACTION], flag_groups = [flag_group(flags = ['--probe=%{probe}'])])],
            env_sets = [env_set(actions = [ACTION], env_entries = [env_entry(key = 'PROBE', value = '%{probe}'), env_entry(key = 'INPUT', value = '%{source_file}')])]),
    ]
    compile = action_config(action_name = ACTION, tools = [
        tool(path = 'bin/fast-cc', with_features = [with_feature_set(features = ['fast'])], execution_requirements = ['local', 'no-sandbox', 'local']),
        tool(path = 'bin/cc', execution_requirements = ['local', 'no-sandbox', 'local']),
    ], flag_sets = [flag_set(flag_groups = [flag_group(flags = ['--action-first'])])])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = 'expansion-proof',
        compiler = 'gcc', target_libc = 'local', features = defs, action_configs = [compile],
        tool_paths = [tool_path(name = n, path = 'bin/' + n) for n in ['gcc', 'ar', 'strip']])
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = 'toolchain')
    fdo = struct(branch_fdo_profile = None, propeller_optimize_info = None)
    parent = cc_internal.combine_cc_toolchain_variables(cc_common.empty_variables(),
        cc_internal.cc_toolchain_variables(vars = {'legacy_compile_flags': ['--legacy'], 'sysroot': '/sysroot', 'unused_file': source}))
    envelope = struct(_supports_header_parsing = False,
        _cc_info = struct(compilation_context = struct(_module_map = None)),
        _toolchain_features = model, _is_tool_configuration = False,
        _toolchain_label = ctx.label, _fdo_context = fdo,
        _cpp_configuration = ctx.fragments.cpp, _build_variables = parent)
    configuration = configure_features(ctx = ctx, cc_toolchain = envelope,
        requested_features = ctx.features, unsupported_features = ctx.disabled_features)
    variables = create_compile_variables(cc_toolchain = envelope, feature_configuration = configuration,
        source_file = source, output_file = 'out/compile.o', use_pic = True,
        user_compile_flags = ctx.attr.flags, includes = ['forced.h'],
        include_directories = depset(['include']), quote_include_directories = depset(['quote']),
        system_include_directories = depset(['system']), framework_include_directories = depset(['framework']),
        preprocessor_defines = depset(['ANSWER=42']),
        variables_extension = {'probe': 'P', 'unused': ctx.attr.unused})
    return [_query(variables, configuration, source)]

subject = rule(implementation = _impl, fragments = ['cpp'], attrs = {
    'compiler': attr.string(default = 'gcc'),
    'src': attr.label(default = ':input.cc', allow_single_file = True),
    'flags': attr.string_list(default = ['--user']), 'unused': attr.string(default = 'A'),
    'dep': attr.label(),
})
"#;

fn build(root: &std::path::Path, flags: &str, unused: &str) {
    fs::write(root.join("rules_cc/cc/private/BUILD.bazel"), format!(
        "load(':proof.bzl','subject')\nexports_files(['input.cc'])\nsubject(name='producer',flags=['{flags}'],unused='{unused}')\nsubject(name='consumer',dep=':producer')\n"
    )).unwrap();
}
fn workspace() -> PathBuf {
    let root = workspace_with("", SUBJECT, SOURCES);
    fs::write(root.join("rules_cc/cc/private/input.cc"), "int proof;\n").unwrap();
    build(&root, "--user", "A");
    root
}
fn list(result: &ConfiguredNodeResult, name: &str) -> Vec<String> {
    let AnalysisValueKind::List(values) = field(result, name).kind() else {
        panic!("{name} must be a list")
    };
    values
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}
fn text<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a str {
    field(result, name).as_str().unwrap()
}
fn expected_argv(source: &str, user: &str, fast: bool) -> Vec<String> {
    // CcToolchainFeatures action flags precede features. Legacy generator order:
    // legacy flags, random seed, PIC, defines, includes/paths, authored features,
    // then get_features_to_appear_last: user flags, sysroot, compiler input/output.
    let mut result = [
        "--action-first",
        "--legacy",
        "-frandom-seed=out/compile.o",
        "-fPIC",
        "-DANSWER=42",
        "-include",
        "forced.h",
        "-iquote",
        "quote",
        "-Iinclude",
        "-isystem",
        "system",
        "-Fframework",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if fast {
        result.push("--fast".to_owned());
    }
    result.extend(
        [
            "--probe=P",
            user,
            "--sysroot=/sysroot",
            "-c",
            source,
            "-o",
            "out/compile.o",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    result
}

#[tokio::test]
async fn authentic_compile_variables_expand_and_survive_configured_dependency_publication() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let producer = request(&dice, &root, "producer", config(&[]))
        .await
        .unwrap();
    let consumer = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    let AnalysisValueKind::Artifact(source) = field(&producer, "source").kind() else {
        panic!("compile input must retain the genuine configured source File")
    };
    assert!(
        matches!(source, AnalysisArtifact::Source(label) if label.to_string() == "@@rules_cc+//cc/private:input.cc")
    );
    let source_path = source.path();
    assert_eq!(
        list(&producer, "argv"),
        expected_argv(&source_path, "--user", false)
    );
    assert_eq!(text(&producer, "tool"), "toolchain/bin/cc");
    assert_eq!(list(&producer, "requirements"), ["local", "no-sandbox"]);
    assert!(matches!(
        field(&producer, "enabled").kind(),
        AnalysisValueKind::Boolean(true)
    ));
    let AnalysisValueKind::Dictionary(env) = field(&producer, "env").kind() else {
        panic!("environment must be a dictionary")
    };
    let env: Vec<_> = env
        .iter()
        .map(|(k, v)| (k.as_str().unwrap(), v.as_str().unwrap()))
        .collect();
    assert_eq!(env, [("PROBE", "P"), ("INPUT", source_path.as_ref())]);
    for name in [
        "variables",
        "configuration",
        "source",
        "argv",
        "env",
        "tool",
        "requirements",
        "enabled",
    ] {
        assert!(
            field(&producer, name).publication_eq(field(&consumer, name)),
            "dependency round trip changed {name}"
        );
    }
    assert!(producer.actions().is_empty());
    assert!(consumer.actions().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn authentic_compile_expansion_observes_flags_unused_payload_and_selection_restoration() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    build(&root, "--changed-user", "A");
    let flags = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    let AnalysisValueKind::Artifact(source) = field(&flags, "source").kind() else {
        panic!("source File")
    };
    assert_eq!(
        list(&flags, "argv"),
        expected_argv(&source.path(), "--changed-user", false)
    );
    assert!(!field(&first, "variables").publication_eq(field(&flags, "variables")));
    build(&root, "--user", "B");
    let unused = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    assert_eq!(list(&first, "argv"), list(&unused, "argv"));
    assert!(
        !field(&first, "variables").publication_eq(field(&unused, "variables")),
        "unused local variables must participate in publication"
    );
    build(&root, "--user", "A");
    let fast = request(
        &dice,
        &root,
        "consumer",
        config(&[(NativeCommandOption::Features, "fast")]),
    )
    .await
    .unwrap();
    assert_eq!(text(&fast, "tool"), "toolchain/bin/fast-cc");
    assert_eq!(
        list(&fast, "argv"),
        expected_argv(&source.path(), "--user", true)
    );
    let restored = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    fs::remove_dir_all(root).unwrap();
}
