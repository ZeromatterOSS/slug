//! Immutable native link Args normalize into the existing generic action owners.
use slug_configuration_v2::native::CppFragmentProjection;

use super::super::cc_configure_features::request_with_route;
use super::*;

const SUBJECT: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'flag_set', 'flag_group')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')
Proof = provider(fields = ['marker', 'mode'])
ACTION = 'c++-link-executable'

def _args(configuration, variables, parameter_file_type = None):
    return cc_internal.get_link_args(feature_configuration = configuration, action_name = ACTION,
        build_variables = variables, parameter_file_type = parameter_file_type)

def _consume(ctx, configuration, scope, parameter_file_type):
    wrapped = cc_internal.wrap_link_actions(ctx.actions)
    values = ['dup', 'with space', 'dup']
    variables = cc_internal.cc_toolchain_variables(vars = {
        'stamp': scope + ':MARKER:' + ctx.fragments.cpp.compilation_mode(),
        'values': values, 'linker_param_file': 'LINKER_PARAM_FILE_PLACEHOLDER',
    })
    lazy = _args(configuration, variables)
    flagged = _args(configuration, variables, parameter_file_type)
    write = wrapped.declare_file(scope + '.params')
    run = wrapped.declare_file(scope + '.run')
    param = wrapped.declare_file(scope + '.flagged')
    values.append('before-write')
    wrapped.write(write, lazy)
    values.append('before-run')
    wrapped.run(outputs = [run], executable = 'runner', arguments = [lazy])
    wrapped.run(outputs = [param], executable = 'runner', arguments = [flagged])
    return [write, run, param]

child = subrule(implementation = _consume, fragments = ['cpp'])
def _locked(ctx, outer, output, args):
    outer.write(output, args)
locked = subrule(implementation = _locked)
def _escaped(ctx):
    return cc_internal.wrap_link_actions(ctx.actions)
escaped = subrule(implementation = _escaped)
def _scope(ctx, configuration):
    wrapped = cc_internal.wrap_link_actions(ctx.actions)
    output = wrapped.declare_file('bad.run')
    bad = _args(configuration, cc_internal.cc_toolchain_variables(vars = {'values': [7]}))
    wrapped.run(outputs = [output], executable = 'runner', arguments = [bad], toolchain = True)
scope_guard = subrule(implementation = _scope)

def _impl(ctx):
    groups = [flag_group(flags = ['--stamp=%{stamp}'], expand_if_available = 'stamp'),
        flag_group(iterate_over = 'values', flags = ['%{values}']),
        flag_group(flags = ['--first=100%%/%%s:%{linker_param_file}', '--second=%{linker_param_file}',
            'beforeLINKER_PARAM_FILE_PLACEHOLDERafter'], expand_if_available = 'linker_param_file'),
        flag_group(flags = ['--tail'])]
    probe = feature(name = 'probe', flag_sets = [flag_set(actions = [ACTION], flag_groups = groups)])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = 'link-args', compiler = 'gcc',
        features = [feature(name = 'no_legacy_features'), probe])
    configuration = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = '.').configure_features(['probe'])
    if ctx.attr.guard:
        bad = _args(configuration, cc_internal.cc_toolchain_variables(vars = {'values': [7]}))
        output = ctx.actions.declare_file('bad.params')
        if ctx.attr.guard == 'locked':
            locked(cc_internal.wrap_link_actions(ctx.actions), output, bad)
        elif ctx.attr.guard == 'expired':
            escaped().write(output, bad)
        else:
            scope_guard(configuration)
        fail('action guard unexpectedly succeeded')
    outputs = _consume(ctx, configuration, 'root', ctx.attr.parameter_file_type) + child(configuration, 'sub', ctx.attr.parameter_file_type)
    return [DefaultInfo(files = depset(outputs)), Proof(marker = 'MARKER', mode = ctx.fragments.cpp.compilation_mode())]
subject = rule(implementation = _impl, fragments = ['cpp'], attrs = {'guard': attr.string(), 'parameter_file_type': attr.string(default = 'UNQUOTED')}, subrules = [child, locked, escaped, scope_guard])
"#;

fn source(root: &std::path::Path, marker: &str) {
    fs::write(
        root.join("rules_cc/cc/private/proof.bzl"),
        SUBJECT.replace("MARKER", marker),
    )
    .unwrap();
}

fn assert_actions(
    result: &ConfiguredNodeResult,
    marker: &str,
    mode: &str,
    parameter_file_type: &str,
) {
    assert_eq!(field(result, "marker").as_str(), Some(marker));
    assert_eq!(field(result, "mode").as_str(), Some(mode));
    assert_eq!(result.actions().len(), 6);
    for (actions, scope) in result.actions().chunks_exact(3).zip(["root", "sub"]) {
        let stamp = format!("--stamp={scope}:{marker}:{mode}");
        let mut argv = vec![
            stamp.clone(),
            "dup".into(),
            "with space".into(),
            "dup".into(),
            "before-write".into(),
        ];
        let write = actions[0]
            .args_write_spec()
            .expect("existing ArgsWrite owner");
        assert_eq!(
            write.render_content(),
            format!("'{stamp}'\ndup\n'with space'\ndup\nbefore-write\n--tail\n")
        );
        argv.extend(["before-run".into(), "--tail".into()]);
        for (action, flagged) in actions[1..].iter().zip([false, true]) {
            let spawn = action.spawn_spec().expect("existing generic Spawn owner");
            let mut expected = vec!["runner".to_owned()];
            expected.extend(argv.clone());
            assert_eq!(spawn.render_argv(), expected);
            let [RetainedCommandLineSegment::ArgsSnapshot(args)] = spawn.command_line().segments()
            else {
                panic!("one ordinary retained Args snapshot")
            };
            assert_eq!(args.recipe().render(), argv);
            let expanded = spawn.expand_forced_param_files().unwrap();
            if flagged {
                let policy = args.param_file().expect("forced parameter policy");
                assert!(policy.use_always());
                assert_eq!(policy.flag_format(), "--first=100%%/%%s:%s");
                assert_eq!(expanded.param_files().len(), 1);
                let param = &expanded.param_files()[0];
                assert_eq!(
                    param.path(),
                    format!("{}-0.params", action.outputs()[0].path())
                );
                let bytes = if parameter_file_type == "GCC_QUOTED" {
                    format!("{stamp}\ndup\nwith\\ space\ndup\nbefore-write\nbefore-run\n--tail\n")
                } else {
                    format!("{}\n", argv.join("\n"))
                };
                assert_eq!(param.bytes(), bytes.as_bytes());
                assert_eq!(
                    expanded.argv(),
                    [
                        "runner".to_owned(),
                        format!("--first=100%/%s:{}", param.path())
                    ]
                );
            } else {
                assert!(args.param_file().is_none());
                assert_eq!(expanded.argv(), expected);
                assert!(expanded.param_files().is_empty());
            }
        }
        for action in actions {
            assert_eq!(
                action.outputs()[0].root().configuration(),
                result
                    .configured_target_key()
                    .unwrap()
                    .configuration()
                    .slug_configuration()
                    .unwrap()
            );
        }
    }
}

#[tokio::test]
async fn immutable_link_args_use_generic_root_subrule_snapshots_and_restore_inputs() {
    for (observed, parameter_file_type) in [false, true]
        .into_iter()
        .flat_map(|observed| ["UNQUOTED", "GCC_QUOTED"].map(|format| (observed, format)))
    {
        let root = workspace_with(
            &format!("subject(name='subject',parameter_file_type='{parameter_file_type}')"),
            &SUBJECT.replace("MARKER", "A"),
            &[],
        );
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let fast = config(&[]);
        let first = request_with_route(&dice, &root, "subject", fast.clone(), observed)
            .await
            .unwrap();
        assert_actions(&first, "A", "fastbuild", parameter_file_type);
        let dbg = config(&[(NativeCommandOption::CompilationMode, "dbg")]);
        let changed = request_with_route(&dice, &root, "subject", dbg.clone(), observed)
            .await
            .unwrap();
        assert_actions(&changed, "A", "dbg", parameter_file_type);
        assert_ne!(changed, first);
        assert_eq!(
            request_with_route(&dice, &root, "subject", fast.clone(), observed)
                .await
                .unwrap(),
            first
        );
        let target = dbg.slug_configuration().unwrap();
        let platform = target.host_platform_label().unwrap();
        for execution in [
            target.to_exec().unwrap(),
            target.to_exec_for_platform(&platform).unwrap(),
        ] {
            let projection = CppFragmentProjection::new(execution.clone()).unwrap();
            let mode = projection.compilation_mode().unwrap();
            let result = request_with_route(
                &dice,
                &root,
                "subject",
                ConfigurationKey::from_slug(execution.clone()),
                observed,
            )
            .await
            .unwrap();
            assert_actions(&result, "A", mode, parameter_file_type);
        }
        assert_eq!(
            request_with_route(&dice, &root, "subject", dbg, observed)
                .await
                .unwrap(),
            changed
        );
        source(&root, "B");
        let changed_source = request_with_route(&dice, &root, "subject", fast.clone(), observed)
            .await
            .unwrap();
        assert_actions(&changed_source, "B", "fastbuild", parameter_file_type);
        assert_ne!(changed_source, first);
        source(&root, "A");
        assert_eq!(
            request_with_route(&dice, &root, "subject", fast, observed)
                .await
                .unwrap(),
            first
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn immutable_link_args_expansion_follows_action_lifecycle_and_binding_guards() {
    for observed in [false, true] {
        let root = workspace_with(
            "subject(name='locked',guard='locked')\nsubject(name='expired',guard='expired')\nsubject(name='scope',guard='scope')",
            &SUBJECT.replace("MARKER", "A"),
            &[],
        );
        let dice = Dice::builder().build(DetectCycles::Enabled);
        for (name, expected) in [
            (
                "locked",
                "cannot access field or method 'write' of rule context",
            ),
            (
                "expired",
                "cannot access field or method 'write' of subrule context",
            ),
            ("scope", "toolchain must be a Label, string, or None"),
        ] {
            let error = request_with_route(&dice, &root, name, config(&[]), observed)
                .await
                .unwrap_err();
            assert!(error.contains(expected), "{name}: {error}");
            assert!(!error.contains("unexpectedly succeeded"), "{error}");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
