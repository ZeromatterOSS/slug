//! Unchanged link-variable producer over real Cpp options and native expansion.
use super::*;

const LINK_SOURCE: &[(&str, &str)] = &[(
    "rules_cc/cc/private/link/link_build_variables.bzl",
    include_str!("../fixtures/rustc-map-each/rules_cc/cc/private/link/link_build_variables.bzl"),
)];
const SUBJECT: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'flag_set', 'flag_group')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')
load('//cc/private/link:link_build_variables.bzl', 'create_link_variables')

Proof = provider(fields = ['cases'])
ACTION = 'c++-link-executable'

def _impl(ctx):
    groups = [flag_group(flags = ['--' + name], expand_if_available = name) for name in ['force_pic', 'strip_debug_symbols', 'is_using_fission']]
    groups.append(flag_group(flags = ['--test'], expand_if_true = 'is_cc_test'))
    for name in ['runtime_library_search_directories', 'library_search_directories', 'user_link_flags']:
        groups.append(flag_group(iterate_over = name, flags = [name + '=%{' + name + '}']))
    for name in ['linker_param_file', 'output_execpath', 'base']:
        groups.append(flag_group(flags = [name + '=%{' + name + '}'], expand_if_available = name))
    probe = feature(name = 'probe', flag_sets = [flag_set(actions = [ACTION], flag_groups = groups)])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = 'link-variables', compiler = 'gcc',
        features = [feature(name = 'no_legacy_features'), feature(name = 'per_object_debug_info'), feature(name = 'fdo_instrument'), probe])
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = '.')
    configuration = model.configure_features(['probe', 'per_object_debug_info'])
    envelope = struct(_cpp_configuration = ctx.fragments.cpp, _build_variables_dict = {'base': 'kept'})
    if ctx.attr.mode == 'fdo':
        create_link_variables(cc_toolchain = envelope, feature_configuration = model.configure_features(['fdo_instrument']))
        fail('FDO unexpectedly succeeded')
    if ctx.attr.mode == 'output_type':
        create_link_variables(cc_toolchain = envelope, feature_configuration = configuration, output_file = 7)
        fail('output type unexpectedly succeeded')
    cases = {}
    for name in ['normal', 'dynamic', 'archiver', 'keep_debug', 'no_debug_feature', 'empty']:
        arguments = dict(cc_toolchain = envelope, feature_configuration = configuration,
            output_file = 'out/program', runtime_library_search_directories = depset(['rpath']),
            library_search_directories = depset(['libpath']),
            user_link_flags = ['-pie', '-Wl,-pie', '--keep', '-pie=retained'], param_file = 'params',
            must_keep_debug = False, use_test_only_flags = True)
        if name == 'dynamic':
            arguments['is_linking_dynamic_library'] = True
        elif name == 'archiver':
            arguments['is_using_linker'] = False
        elif name == 'keep_debug':
            arguments['must_keep_debug'] = True
        elif name == 'no_debug_feature':
            arguments['feature_configuration'] = model.configure_features(['probe'])
        elif name == 'empty':
            arguments.update(output_file = None, runtime_library_search_directories = None,
                library_search_directories = None, user_link_flags = None, param_file = None,
                use_test_only_flags = None)
        variables = create_link_variables(**arguments)
        cases[name] = cc_common.get_memory_inefficient_command_line(feature_configuration = arguments['feature_configuration'], action_name = ACTION, variables = variables)
    return [Proof(cases = cases)]

subject = rule(implementation = _impl, fragments = ['cpp'], attrs = {'mode': attr.string(default = 'normal')})
"#;

fn workspace(targets: &str) -> PathBuf {
    let sources = SOURCES
        .iter()
        .chain(LINK_SOURCE)
        .copied()
        .collect::<Vec<_>>();
    workspace_with(targets, SUBJECT, &sources)
}

fn cases(result: &ConfiguredNodeResult) -> Vec<(String, Vec<String>)> {
    let AnalysisValueKind::Dictionary(cases) = field(result, "cases").kind() else {
        panic!("cases dictionary")
    };
    cases
        .iter()
        .map(|(name, value)| {
            let AnalysisValueKind::List(items) = value.kind() else {
                panic!("argv list")
            };
            (
                name.as_str().unwrap().to_owned(),
                items
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn unchanged_link_variables_project_real_cpp_modes_and_builder_guards() {
    let root = workspace("subject(name='subject')");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for mode in ["fastbuild", "dbg"] {
        let result = request(
            &dice,
            &root,
            "subject",
            config(&[
                (NativeCommandOption::CompilationMode, mode),
                (NativeCommandOption::Fission, "yes"),
                (NativeCommandOption::ForcePic, "true"),
            ]),
        )
        .await
        .unwrap();
        for (name, actual) in cases(&result) {
            let mut expected = vec!["--force_pic".to_owned()];
            if mode == "fastbuild" && name != "keep_debug" {
                expected.push("--strip_debug_symbols".to_owned());
            }
            if name != "archiver" && name != "no_debug_feature" {
                expected.push("--is_using_fission".to_owned());
            }
            if name != "empty" {
                expected.extend(
                    [
                        "--test",
                        "runtime_library_search_directories=rpath",
                        "library_search_directories=libpath",
                    ]
                    .map(str::to_owned),
                );
                if name != "archiver" {
                    if name != "dynamic" {
                        expected.extend(
                            ["user_link_flags=-pie", "user_link_flags=-Wl,-pie"].map(str::to_owned),
                        );
                    }
                    expected.extend(
                        ["user_link_flags=--keep", "user_link_flags=-pie=retained"]
                            .map(str::to_owned),
                    );
                }
                expected.extend(
                    ["linker_param_file=params", "output_execpath=out/program"].map(str::to_owned),
                );
            }
            expected.push("base=kept".to_owned());
            assert_eq!(actual, expected, "{mode}/{name}");
        }
        assert!(result.actions().is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_link_variables_reject_fdo_and_wrong_output_type() {
    let root =
        workspace("subject(name='fdo',mode='fdo')\nsubject(name='output_type',mode='output_type')");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (target, expected) in [
        ("fdo", "FDO instrumentation not supported"),
        (
            "output_type",
            "Parameter 'output_file' expected String, got 'int'",
        ),
    ] {
        let error = request(&dice, &root, target, config(&[]))
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{target}: {error}");
        assert!(!error.contains("unexpectedly succeeded"), "{error}");
    }
    fs::remove_dir_all(root).unwrap();
}
