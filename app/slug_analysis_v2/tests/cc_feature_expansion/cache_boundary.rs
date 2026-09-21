//! Authored callers of native variables/query APIs, exercising the real rule
//! evaluation/publication boundary. Loading separately owns live freeze/GC tests.
use super::*;

const SUBJECT: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'action_config', 'tool', 'flag_set', 'flag_group', 'env_set', 'env_entry')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')

CacheInfo = provider(fields = ['variables', 'configuration', 'argv'])
ACTION = 'compile'

def _argv(variables, configuration):
    return cc_common.get_memory_inefficient_command_line(feature_configuration = configuration, action_name = ACTION, variables = variables)

def _impl(ctx):
    if ctx.attr.dep:
        data = ctx.attr.dep[CacheInfo]
        return [CacheInfo(variables = data.variables, configuration = data.configuration, argv = _argv(data.variables, data.configuration))]
    if ctx.attr.mode == 'cycle':
        values = []
        variables = cc_internal.cc_toolchain_variables(vars = {'cycle': values})
        values.append(variables)
        return [CacheInfo(variables = variables)]
    if ctx.attr.mode == 'range':
        return [CacheInfo(variables = cc_internal.cc_toolchain_variables(vars = {'unused': range(0)}))]

    source = ctx.attr.src[DefaultInfo].files.to_list()[0]
    probe = feature(name = 'probe', flag_sets = [flag_set(actions = [ACTION], flag_groups = [
        flag_group(iterate_over = 'obj.items', flags = ['--cached=%{obj.items}']),
        flag_group(iterate_over = 'direct', flags = ['--direct=%{direct}']),
    ])], env_sets = [env_set(actions = [ACTION], env_entries = [env_entry(key = 'PROOF', value = 'value')])])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = 'cache-proof', compiler = 'gcc',
        features = [probe], action_configs = [action_config(action_name = ACTION, tools = [tool(path = 'cc', execution_requirements = ['local'])])])
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = '.')
    configuration = model.configure_features(['compile', 'probe'])
    items = ['old']
    variables = cc_internal.cc_toolchain_variables(vars = {
        'obj': struct(items = items), 'direct': items,
        'unused_file': source, 'unused_depset': depset([source]),
    })
    seeded = _argv(variables, configuration)
    items.append('new')
    mode = ctx.attr.mode
    if mode == 'cache':
        return [CacheInfo(variables = variables, configuration = configuration, argv = seeded)]
    if mode == 'positional':
        cc_common.get_tool_for_action(configuration, ACTION)
    elif mode == 'configuration_type':
        cc_common.get_tool_for_action(feature_configuration = None, action_name = ACTION)
    elif mode == 'variables_type':
        cc_common.get_environment_variables(feature_configuration = configuration, action_name = ACTION, variables = None)
    elif mode == 'action_type':
        cc_common.action_is_enabled(feature_configuration = configuration, action_name = 1)
    elif mode == 'empty_arity':
        cc_common.empty_variables(1)
    elif mode == 'argv_mutation':
        _argv(variables, configuration).append('bad')
    elif mode == 'environment_mutation':
        environment = cc_common.get_environment_variables(feature_configuration = configuration, action_name = ACTION, variables = variables)
        environment['bad'] = 'bad'
    elif mode == 'requirements_mutation':
        cc_common.get_execution_requirements(feature_configuration = configuration, action_name = ACTION).append('bad')
    else:
        fail('unknown boundary case')
    fail('boundary operation unexpectedly succeeded')

subject = rule(implementation = _impl, attrs = {
    'mode': attr.string(default = 'cache'), 'dep': attr.label(),
    'src': attr.label(default = ':input.cc', allow_single_file = True),
})
"#;

fn workspace(targets: &str) -> PathBuf {
    let root = workspace_with(targets, SUBJECT, &[]);
    fs::write(
        root.join("rules_cc/cc/private/input.cc"),
        "int cache_proof;\n",
    )
    .unwrap();
    root
}

#[tokio::test]
async fn live_dotted_cache_and_shared_mutable_locals_survive_real_publication() {
    let root = workspace(
        "exports_files(['input.cc'])\nsubject(name='producer')\nsubject(name='consumer',dep=':producer')",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let producer = request(&dice, &root, "producer", config(&[]))
        .await
        .unwrap();
    let consumer = request(&dice, &root, "consumer", config(&[]))
        .await
        .unwrap();
    assert_eq!(list(&producer, "argv"), ["--cached=old", "--direct=old"]);
    assert_eq!(
        list(&consumer, "argv"),
        ["--cached=old", "--direct=old", "--direct=new"]
    );
    assert!(field(&producer, "variables").publication_eq(field(&consumer, "variables")));
    let AnalysisValueKind::CcToolchainVariables(variables) = field(&producer, "variables").kind()
    else {
        panic!("retained Variables")
    };
    assert!(
        variables
            .model()
            .cache
            .iter()
            .any(|(name, _)| name == "obj.items")
    );
    assert!(
        variables
            .model()
            .locals
            .iter()
            .any(|(name, value)| name == "unused_file"
                && matches!(value.kind(), AnalysisValueKind::Artifact(_)))
    );
    assert!(
        variables
            .model()
            .locals
            .iter()
            .any(|(name, value)| name == "unused_depset"
                && matches!(value.kind(), AnalysisValueKind::Depset(_)))
    );
    assert!(producer.actions().is_empty());
    assert!(consumer.actions().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn public_feature_queries_reject_wrong_binding_types_and_output_mutation() {
    let cases = [
        ("positional", "Missing named-only parameter"),
        (
            "configuration_type",
            "feature_configuration must be FeatureConfiguration",
        ),
        ("variables_type", "variables must be CcToolchainVariables"),
        (
            "action_type",
            "Type of parameter `action_name` doesn't match",
        ),
        (
            "empty_arity",
            "Wrong number of positional arguments, expected 0, got 1",
        ),
        ("argv_mutation", "Immutable"),
        ("environment_mutation", "Immutable"),
        ("requirements_mutation", "Immutable"),
    ];
    let targets = cases
        .iter()
        .map(|(mode, _)| format!("subject(name='{mode}',mode='{mode}')\n"))
        .collect::<String>();
    let root = workspace(&format!("exports_files(['input.cc'])\n{targets}"));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (mode, expected) in cases {
        let error = request(&dice, &root, mode, config(&[])).await.unwrap_err();
        assert!(error.contains(expected), "{mode}: {error}");
        assert!(!error.contains("unexpectedly succeeded"), "{mode}: {error}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn variable_publication_rejects_cycles_and_unadmitted_raw_range() {
    let root = workspace("subject(name='cycle',mode='cycle')\nsubject(name='range',mode='range')");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let cycle = request(&dice, &root, "cycle", config(&[]))
        .await
        .unwrap_err();
    assert!(cycle.contains("cyclic analysis value"), "{cycle}");
    let range = request(&dice, &root, "range", config(&[]))
        .await
        .unwrap_err();
    assert!(
        range.contains("unsupported analysis value of type `range`"),
        "{range}"
    );
    fs::remove_dir_all(root).unwrap();
}
