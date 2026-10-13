//! Configured consumers of the Bazel 9.2 ctx.var collection contract.
use super::*;

fn workspace(defs: &str, build: &str) -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), defs).unwrap();
    fs::write(root.join("BUILD.bazel"), build).unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{name}"),
        configuration,
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

fn answer(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Answers").unwrap())
}

fn configuration(rows: &[(NativeCommandOption, &str)]) -> ConfigurationKey {
    let base = typed_action_test_configuration();
    let base = base.slug_configuration().unwrap();
    let overlay: CommandConfigurationOverlay = rows
        .iter()
        .map(|(option, value)| CommandConfigurationOccurrence::native(*option, Some(*value), false))
        .collect::<Vec<_>>()
        .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

const GLOBAL_DEFS: &str = r#"
Answers = provider(fields = ['value'])
Snapshot = provider(fields = ['variables'])
def _probe(ctx):
    variables = ctx.var
    if type(variables) != 'dict' or variables != ctx.var:
        fail('ctx.var must be a stable plain dictionary')
    if ctx.attr.mode == 'globals':
        if variables['BINDIR'] != ctx.bin_dir.path or variables['GENDIR'] != ctx.genfiles_dir.path:
            fail('make roots must use the active artifact roots')
        if variables['BINDIR'] != variables['GENDIR']:
            fail('default genfiles root must be merged')
        return [Answers(value='|'.join([variables['TARGET_CPU'], variables['COMPILATION_MODE'], ','.join(variables.keys())]))]
    if ctx.attr.mode == 'assign': variables['TARGET_CPU'] = 'changed'
    if ctx.attr.mode == 'clear': variables.clear()
    if ctx.attr.mode == 'update': variables.update({'ADDED': 'changed'})
    if ctx.attr.mode == 'pop': variables.pop('COMPILATION_MODE')
    if ctx.attr.mode == 'setdefault': variables.setdefault('ADDED', 'changed')
    copied = dict(variables)
    copied['TARGET_CPU'] = 'copy-only'
    if variables['TARGET_CPU'] == 'copy-only': fail('dict copy leaked into cached variables')
    return [Answers(value='|'.join([','.join(variables.keys()), ','.join(variables.values())])), Snapshot(variables=variables)]
probe = rule(implementation=_probe, attrs={'mode': attr.string(default='globals')})
def _read(ctx):
    variables = ctx.attr.dep[Snapshot].variables
    return [Answers(value='|'.join([variables['COMPILATION_MODE'], variables['TARGET_CPU'], ','.join(variables.keys())]))]
read = rule(implementation=_read, attrs={'dep': attr.label(providers=[Snapshot])})
"#;

#[tokio::test]
async fn make_variable_globals_defines_preserve_reserved_key_positions_and_last_wins() {
    let root = workspace(
        GLOBAL_DEFS,
        "load(':defs.bzl','probe')\nprobe(name='request')\nprobe(name='defined',mode='defined')\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "request", configuration(&[]))
        .await
        .unwrap();
    assert_eq!(
        answer(&first),
        "|fastbuild|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR"
    );
    let rows = [
        (NativeCommandOption::CompilationMode, "dbg"),
        (NativeCommandOption::Define, "z=first"),
        (NativeCommandOption::Define, "a="),
        (NativeCommandOption::Define, "z=last=with=equals"),
        (NativeCommandOption::Define, "TARGET_CPU=defined-cpu"),
        (NativeCommandOption::Define, "COMPILATION_MODE=defined-mode"),
        (NativeCommandOption::Define, "BINDIR=defined-bin"),
        (NativeCommandOption::Define, "GENDIR=defined-gen"),
    ];
    let defined = request(&dice, &root, "defined", configuration(&rows))
        .await
        .unwrap();
    assert_eq!(
        answer(&defined),
        "TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR,a,z|defined-cpu,defined-mode,defined-bin,defined-gen,,last=with=equals"
    );
    assert_eq!(
        first,
        request(&dice, &root, "request", configuration(&[]))
            .await
            .unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn make_variable_dictionary_refuses_mutation_and_survives_provider_publication() {
    let root = workspace(
        GLOBAL_DEFS,
        "load(':defs.bzl','probe','read')\nprobe(name='producer',mode='publish')\nread(name='request',dep=':producer')\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = configuration(&[(NativeCommandOption::CompilationMode, "dbg")]);
    let first = request(&dice, &root, "request", config.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&first),
        "dbg||TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR"
    );
    for mode in ["assign", "clear", "update", "pop", "setdefault"] {
        fs::write(
            root.join("BUILD.bazel"),
            format!("load(':defs.bzl','probe','read')\nprobe(name='producer',mode='{mode}')\nread(name='request',dep=':producer')\n"),
        ).unwrap();
        let error = request(&dice, &root, "request", config.clone())
            .await
            .unwrap_err();
        assert!(error.contains("Immutable"), "{mode}: {error}");
    }
    fs::write(root.join("BUILD.bazel"), "load(':defs.bzl','probe','read')\nprobe(name='producer',mode='publish')\nread(name='request',dep=':producer')\n").unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "request", config).await.unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

const ATTRIBUTE_DEFS: &str = r#"
Answers = provider(fields = ['value'])
def _supplier(ctx):
    return [platform_common.TemplateVariableInfo(ctx.attr.variables)]
supplier = rule(implementation=_supplier, attrs={'variables': attr.string_dict()})
def _probe(ctx):
    variables = ctx.var
    if 'POISON' in variables: fail('ordinary dependencies supplied make variables')
    return [Answers(value='|'.join([variables['SHARED'], variables['TARGET_CPU'], ','.join(variables.keys())]))]
probe = rule(implementation=_probe, attrs={
    'deps': attr.label_list(),
    '_cc_toolchain': attr.label(default=':implicit'),
})
"#;

const ATTRIBUTE_BUILD: &str = r#"
load(':defs.bzl','supplier','probe')
supplier(name='first',variables={'SHARED':'first','FIRST':'one','TARGET_CPU':'provider-cpu'})
supplier(name='second',variables={'SECOND':'two','SHARED':'second'})
supplier(name='implicit',variables={'IMPLICIT':'three'})
supplier(name='unrelated',variables={'POISON':'excluded'})
probe(name='request',toolchains=[':first',':second'],deps=[':unrelated'])
"#;

#[tokio::test]
async fn make_variable_default_attributes_collect_ordered_suppliers_and_exclude_arbitrary_deps() {
    let root = workspace(ATTRIBUTE_DEFS, ATTRIBUTE_BUILD);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = configuration(&[]);
    let first = request(&dice, &root, "request", config.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&first),
        "second|provider-cpu|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR,FIRST,IMPLICIT,SECOND,SHARED"
    );
    // Provider encounter order controls the winner; the combined provider map
    // is sorted before dictionary insertion (MakeVariableSupplier.java64-69).
    fs::write(
        root.join("BUILD.bazel"),
        ATTRIBUTE_BUILD.replace("[':first',':second']", "[':second',':first']"),
    )
    .unwrap();
    let reversed = request(&dice, &root, "request", config.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&reversed),
        "first|provider-cpu|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR,FIRST,IMPLICIT,SECOND,SHARED"
    );
    assert_ne!(first, reversed);
    let defined = request(
        &dice,
        &root,
        "request",
        configuration(&[
            (NativeCommandOption::Define, "SHARED=define"),
            (NativeCommandOption::Define, "TARGET_CPU=define-cpu"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        answer(&defined),
        "define|define-cpu|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR,FIRST,IMPLICIT,SECOND,SHARED"
    );
    fs::write(
        root.join("BUILD.bazel"),
        ATTRIBUTE_BUILD.replace(
            "{'IMPLICIT':'three'}",
            "{'IMPLICIT':'three','SHARED':'implicit'}",
        ),
    )
    .unwrap();
    assert_eq!(
        answer(
            &request(&dice, &root, "request", config.clone())
                .await
                .unwrap()
        ),
        "implicit|provider-cpu|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR,FIRST,IMPLICIT,SECOND,SHARED"
    );
    fs::write(root.join("BUILD.bazel"), ATTRIBUTE_BUILD).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "request", config).await.unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn make_variable_configuration_source_and_provider_a_b_a_share_one_dice_engine() {
    let root = workspace(ATTRIBUTE_DEFS, ATTRIBUTE_BUILD);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration(&[]);
    let first = request(&dice, &root, "request", base.clone())
        .await
        .unwrap();
    let changed = request(
        &dice,
        &root,
        "request",
        configuration(&[(NativeCommandOption::Define, "SHARED=config-B")]),
    )
    .await
    .unwrap();
    assert!(answer(&changed).starts_with("config-B|"));
    assert_ne!(first, changed);
    assert_eq!(
        first,
        request(&dice, &root, "request", base.clone())
            .await
            .unwrap()
    );
    for (file, original, changed, expected) in [
        (
            "BUILD.bazel",
            ATTRIBUTE_BUILD,
            ATTRIBUTE_BUILD.replace("'SHARED':'second'", "'SHARED':'provider-B'"),
            "provider-B|",
        ),
        (
            "defs.bzl",
            ATTRIBUTE_DEFS,
            ATTRIBUTE_DEFS.replace("variables['SHARED']", "variables['FIRST']"),
            "one|",
        ),
    ] {
        fs::write(root.join(file), changed).unwrap();
        let next = request(&dice, &root, "request", base.clone())
            .await
            .unwrap();
        assert!(answer(&next).starts_with(expected), "{}", answer(&next));
        assert_ne!(first, next);
        fs::write(root.join(file), original).unwrap();
        assert_eq!(
            first,
            request(&dice, &root, "request", base.clone())
                .await
                .unwrap()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

const RESOLVED_DEFS: &str = r#"
Answers = provider(fields = ['value'])
def _tc(ctx):
    return [platform_common.ToolchainInfo(marker=ctx.attr.marker), platform_common.TemplateVariableInfo(ctx.attr.variables)]
tc = rule(implementation=_tc, attrs={'marker':attr.string(), 'variables':attr.string_dict()})
def _probe(ctx):
    if ctx.toolchains['//:compile_type'].marker != 'compile': fail('default selection')
    if ctx.exec_groups['named'].toolchains['//:link_type'].marker != 'link': fail('named selection')
    if ctx.var['COMPILE'] != 'compile' or ctx.var['LINK'] != 'link': fail('resolved variables missing')
    return [Answers(value=ctx.var['SHARED'])]
probe = rule(implementation=_probe, toolchains=['//:compile_type'], exec_groups={'named':exec_group(toolchains=['//:link_type'])})
"#;

const RESOLVED_BUILD: &str = r#"
load(':defs.bzl','tc','probe')
platform(name='platform')
toolchain_type(name='compile_type')
toolchain_type(name='link_type')
tc(name='compile_impl',marker='compile',variables={'COMPILE':'compile','SHARED':'equal'})
tc(name='link_impl',marker='link',variables={'LINK':'link','SHARED':'equal'})
toolchain(name='compile_tc',toolchain_type=':compile_type',toolchain=':compile_impl')
toolchain(name='link_tc',toolchain_type=':link_type',toolchain=':link_impl')
probe(name='request')
"#;

#[tokio::test]
async fn make_variable_selected_named_groups_allow_equal_overlap_refuse_conflicts_and_repair() {
    let root = workspace(RESOLVED_DEFS, RESOLVED_BUILD);
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nregister_execution_platforms('//:platform')\nregister_toolchains('//:compile_tc','//:link_tc')\n").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = execution_group_configuration(false);
    let first = request(&dice, &root, "request", config.clone())
        .await
        .unwrap();
    assert_eq!(answer(&first), "equal");
    let changed = RESOLVED_BUILD.replace(
        "'LINK':'link','SHARED':'equal'",
        "'LINK':'link','SHARED':'conflict'",
    );
    fs::write(root.join("BUILD.bazel"), changed).unwrap();
    let error = request(&dice, &root, "request", config.clone())
        .await
        .unwrap_err();
    assert!(
        error.contains("SHARED") && error.contains("unsupported"),
        "{error}"
    );
    // A define would mask either value but cannot prove unresolved supplier order.
    let error = request(
        &dice,
        &root,
        "request",
        configuration(&[(NativeCommandOption::Define, "SHARED=mask")]),
    )
    .await
    .unwrap_err();
    assert!(
        error.contains("SHARED") && error.contains("unsupported"),
        "{error}"
    );
    fs::write(root.join("BUILD.bazel"), RESOLVED_BUILD).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "request", config.clone())
            .await
            .unwrap()
    );
    fs::write(
        root.join("BUILD.bazel"),
        RESOLVED_BUILD.replace("'LINK':'link','SHARED':'equal'", "'LINK':'link'"),
    )
    .unwrap();
    assert_eq!(
        answer(&request(&dice, &root, "request", config).await.unwrap()),
        "equal"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn make_variable_target_exec_compilation_modes_follow_actual_configurations() {
    let root = workspace(
        GLOBAL_DEFS,
        "load(':defs.bzl','probe')\nprobe(name='request')\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for exec_mode in ["fastbuild", "dbg", "opt"] {
        let target = configuration(&[
            (NativeCommandOption::CompilationMode, "dbg"),
            (NativeCommandOption::HostCompilationMode, exec_mode),
        ]);
        let first = request(&dice, &root, "request", target.clone())
            .await
            .unwrap();
        assert_eq!(
            answer(&first),
            "|dbg|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR"
        );
        let exec =
            ConfigurationKey::from_slug(target.slug_configuration().unwrap().to_exec().unwrap());
        let configured = request(&dice, &root, "request", exec).await.unwrap();
        assert_eq!(
            answer(&configured),
            format!("|{exec_mode}|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR")
        );
        assert_eq!(
            first,
            request(&dice, &root, "request", target).await.unwrap()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn make_variable_target_cpu_tracks_actual_standard_constraint_and_restores() {
    let build = "load(':defs.bzl','probe')\nplatform(name='selected',constraint_values=['@@platforms//cpu:aarch64'])\nprobe(name='request')\n";
    let root = workspace(GLOBAL_DEFS, build);
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='platforms',version='1.0.0')\nlocal_path_override(module_name='platforms',path='platforms')\n").unwrap();
    fs::create_dir_all(root.join("platforms/cpu")).unwrap();
    fs::write(
        root.join("platforms/MODULE.bazel"),
        "module(name='platforms',version='1.0.0')\n",
    )
    .unwrap();
    fs::write(root.join("platforms/REPO.bazel"), "").unwrap();
    fs::write(root.join("platforms/.bazelignore"), "").unwrap();
    fs::write(root.join("platforms/cpu/BUILD.bazel"), "constraint_setting(name='cpu')\nconstraint_value(name='aarch64',constraint_setting=':cpu')\nconstraint_value(name='x86_64',constraint_setting=':cpu')\n").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let selected = CanonicalLabel::parse("@@//:selected").unwrap();
    let target = configuration(&[])
        .slug_configuration()
        .unwrap()
        .with_host_platform_label(&selected);
    for config in [
        ConfigurationKey::from_slug(target.clone()),
        ConfigurationKey::from_slug(target.to_exec_for_platform(&selected).unwrap()),
    ] {
        let request = || {
            root_target_request_with_inputs(
                &dice,
                &root,
                "@@//:request",
                config.clone(),
                Arc::new(RootActivationTracker::default()),
                root_epoch(&root),
                &[("platforms", "platforms"), ("platforms+", "platforms")],
            )
        };
        let first = request().await.unwrap();
        let mode = if config.kind() == slug_analysis_v2::ConfigurationKind::Exec {
            "opt"
        } else {
            "fastbuild"
        };
        assert_eq!(
            answer(&first),
            format!("aarch64|{mode}|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR")
        );
        fs::write(
            root.join("BUILD.bazel"),
            build.replace("cpu:aarch64", "cpu:x86_64"),
        )
        .unwrap();
        let changed = request().await.unwrap();
        assert_eq!(
            answer(&changed),
            format!("x86_64|{mode}|TARGET_CPU,COMPILATION_MODE,BINDIR,GENDIR")
        );
        assert_ne!(first, changed);
        fs::write(root.join("BUILD.bazel"), build).unwrap();
        assert_eq!(first, request().await.unwrap());
    }
    fs::remove_dir_all(root).unwrap();
}
