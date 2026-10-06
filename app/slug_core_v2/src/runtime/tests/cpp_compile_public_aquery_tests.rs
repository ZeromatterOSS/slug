// Normal public cc_common.compile callers over the shared observed fixture.
// Original compile/provider bodies are borrowed; tools and runtime files are inert.
use super::*;

const PUBLIC_COMPILE: &str =
    include_str!("../../../../slug_analysis_v2/tests/rustc_map_each/compile_context.bzl");
const PUBLIC_BRIDGE: &str = r#"load('//cc/private:toolchain_proof.bzl','make_compile_toolchain')
load('//cc:cc_toolchain_config_lib.bzl','feature','env_set','env_entry')
def _provider(ctx):
    env=feature(name='query_environment',enabled=True,env_sets=[env_set(actions=['c-compile','c++-compile'],env_entries=[env_entry(key='FEATURE_ONLY',value='%{source_file}')])])
    return [make_compile_toolchain(ctx,compile_requirements=['requires-network'],compile_features=[env])]
tool_provider=rule(implementation=_provider,fragments=['cpp'],attrs={
    'header':attr.label(default='@@//:header.h',allow_single_file=True),
    'static_file':attr.label(default='@@//:static.a',allow_single_file=True),
    'dynamic_file':attr.label(default='@@//:dynamic.so',allow_single_file=True),
    'runtime':attr.string(default='files'),'unused':attr.string(default='unused'),
    'object_prefix':attr.string(default=''),'object_extension':attr.string(default='.o')})
def _bridge(ctx):
    out=ctx.actions.declare_file('bridge')
    ctx.actions.write(out,'bridge')
    return [DefaultInfo(files=depset([out]))]
bridge=rule(implementation=_bridge,attrs={'deps':attr.label_list()})
exec_bridge=rule(implementation=_bridge,attrs={'deps':attr.label_list(cfg='exec')})
"#;

fn public_declarations(marker: &str) -> String {
    format!(
        "subject(name='ordinary',leaf='dep',public=['@@//:ordinary.h'],visibility=['//visibility:public'])\n\
         subject(name='implementation',leaf='impl',public=['@@//:implementation.h'],visibility=['//visibility:public'])\n\
         subject(name='typed_compile',sources=['@@//:allocator_library.cc'],compile_flags=['-DRECIPE_{marker}'],{common})\n\
         subject(name='typed_c',sources=['@@//:allocator_library.c'],{common})\n\
         subject(name='typed_pic',sources=['@@//:allocator_library.cc'],requested=['supports_pic'],{common})\n",
        common = "toolchain='@@//:producer',public=['@@//:public.h'],private=['@@//:private.h'],textual=['@@//:textual.h'],deps=[':ordinary'],implementation_deps=[':implementation'],public_compile=True,configured_compile=True,visibility=['//visibility:public']"
    )
}

fn public_epoch(revision: i64, marker: &str) -> PathObservationEpoch {
    let external = format!(
        "load('//cc/private:public_compile.bzl','subject')\n{}",
        public_declarations(marker)
    );
    let root = format!(
        "load('@rules_cc//cc/private:public_compile.bzl','subject')\n\
         load('@rules_cc//cc/private:public_bridge.bzl','tool_provider','bridge','exec_bridge')\n\
         platform(name='platform',exec_properties={{'owner-only':'a b','requires-network':'owner'}})\n\
         exports_files(['allocator_library.cc','allocator_library.c','public.h','private.h','textual.h','ordinary.h','implementation.h','header.h','static.a','dynamic.so'])\n\
         tool_provider(name='producer',visibility=['//visibility:public'])\n\
         {declarations}\n\
         bridge(name='main',deps=[':typed_compile',':typed_c',':typed_pic'])\n\
         exec_bridge(name='main_exec',deps=[':typed_compile',':typed_c',':typed_pic'])\n\
         bridge(name='external',deps=['@rules_cc//:typed_compile','@rules_cc//:typed_c','@rules_cc//:typed_pic'])\n\
         exec_bridge(name='external_exec',deps=['@rules_cc//:typed_compile','@rules_cc//:typed_c','@rules_cc//:typed_pic'])\n",
        declarations = public_declarations(marker)
    );
    cpp_epoch_with(
        revision,
        marker,
        &[
            ("rules_cc/cc/private/public_compile.bzl", PUBLIC_COMPILE),
            ("rules_cc/cc/private/public_bridge.bzl", PUBLIC_BRIDGE),
            ("allocator_library.cc", "inert C++"),
            ("allocator_library.c", "inert C"),
            ("public.h", "inert"),
            ("private.h", "inert"),
            ("textual.h", "inert"),
            ("ordinary.h", "inert"),
            ("implementation.h", "inert"),
            // Last in this package: empty source-registration BUILD observations
            // must not overwrite its actual external authored declarations.
            ("rules_cc/BUILD.bazel", &external),
        ],
        Some(&root),
    )
}

fn public_owners(evaluation: &BuildCommandEvaluation) -> Vec<&Arc<ConfiguredNodeResult>> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .filter(|o| o.actions().iter().any(|a| a.cpp_compile_spec().is_some()))
        .collect()
}

// Independent ordered formula from the existing public command proof. Only the
// authored external repository spelling differs from that Analysis fixture.
fn public_argv(spec: &CppCompileSpec, external: bool, tool: bool, flags: &[&str]) -> Vec<String> {
    let mut argv = vec![
        "toolchain/bin/gcc".into(),
        "-MD".into(),
        "-MF".into(),
        spec.dotd_output().path().into(),
        format!("-frandom-seed={}", spec.object_output().path()),
    ];
    if spec.data().use_pic {
        argv.push("-fPIC".into());
    }
    argv.extend(
        [
            "-Ddep-define",
            "-Dimpl-define",
            "-Down-define",
            "-Down-local",
        ]
        .map(str::to_owned),
    );
    let repo = if external { "external/rules_cc+" } else { "." };
    let generated = if external {
        format!("{}/external/rules_cc+", spec.object_output().root().path())
    } else {
        spec.object_output().root().path().to_owned()
    };
    for quote in [
        repo.to_owned(),
        generated,
        "own-quote".into(),
        "dep-quote".into(),
        "impl-quote".into(),
    ] {
        argv.extend(["-iquote".into(), quote]);
    }
    argv.extend(["-Iown-include", "-Idep-include", "-Iimpl-include"].map(str::to_owned));
    for system in ["own-system", "dep-system", "impl-system"] {
        argv.extend(["-isystem".into(), system.into()]);
    }
    argv.push("-Fown-framework".into());
    if tool {
        argv.push("-g0".into());
        if spec.data().action_name == "c++-compile" {
            argv.push("-g0".into());
        }
    }
    argv.extend(flags.iter().map(|s| (*s).to_owned()));
    argv.extend([
        "--sysroot=/sysroot".into(),
        "-c".into(),
        spec.source().path().into_owned(),
        "-o".into(),
        spec.object_output().path().into(),
    ]);
    argv
}

fn public_block(
    view: ConfiguredCppCompileActionView<'_>,
    external: bool,
    tool: bool,
    pic: bool,
    flags: &[&str],
) -> String {
    let spec = view.spec();
    let name = view.owner().label().target().as_str();
    let source = if name == "typed_c" {
        "allocator_library.c"
    } else {
        "allocator_library.cc"
    };
    let object = if pic {
        "allocator_library.pic"
    } else {
        "allocator_library"
    };
    let prefix = if external { "external/rules_cc+/" } else { "" };
    assert_eq!(spec.source().path().as_ref(), source);
    assert_eq!(spec.data().use_pic, pic);
    assert_eq!(
        spec.data().action_name,
        if name == "typed_c" {
            "c-compile"
        } else {
            "c++-compile"
        }
    );
    assert_eq!(
        spec.object_output().root_relative_path(),
        format!("{prefix}_objs/{name}/{object}.o")
    );
    assert_eq!(
        spec.dotd_output().root_relative_path(),
        format!("{prefix}_objs/{name}/{object}.d")
    );
    let args = public_argv(spec, external, tool, flags);
    assert_eq!(
        spec.expand_command_line()
            .unwrap()
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        args.iter().map(String::as_str).collect::<Vec<_>>()
    );
    // The finite ASCII corpus requires quoting only empty/equals arguments.
    // This expected block never calls a production text/shell encoder.
    let command = args
        .iter()
        .map(|a| {
            if a.is_empty() || a.contains('=') {
                format!("'{a}'")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" \\\n    ");
    let mut paths = vec![
        "dynamic.so",
        "header.h",
        "implementation.h",
        "ordinary.h",
        "private.h",
        "public.h",
        source,
        "static.a",
        "textual.h",
    ];
    paths.sort();
    let mut outputs = [spec.dotd_output().path(), spec.object_output().path()];
    outputs.sort();
    let cfg = view
        .owner()
        .configuration()
        .slug_configuration()
        .unwrap()
        .projection()
        .display_token();
    let target = if external {
        format!("@@rules_cc+//:{name}")
    } else {
        format!("//:{name}")
    };
    assert_eq!(
        view.owner().label().to_string(),
        if external {
            target.clone()
        } else {
            format!("@@//:{name}")
        }
    );
    // Public registration retains the configured strict Unix action environment.
    assert_eq!(
        view.data().environment.fixed().get("PATH"),
        Some("/bin:/usr/bin:/usr/local/bin")
    );
    assert_eq!(
        view.data().environment.fixed().iter().count(),
        if tool { 1 } else { 2 }
    );
    let env = if tool {
        "  Environment: [PATH=/bin:/usr/bin:/usr/local/bin]\n"
    } else {
        "  Environment: [FIXED=a b, PATH=/bin:/usr/bin:/usr/local/bin]\n"
    };
    assert_eq!(
        view.data().environment.fixed().get("FIXED"),
        if tool { None } else { Some("a b") }
    );
    assert_eq!(
        view.data()
            .environment
            .inherited()
            .contains("INHERITED_ONLY"),
        !tool
    );
    let info = if name == "typed_c" { "owner" } else { "''" };
    assert_eq!(
        view.data().execution_requirements.get("requires-network"),
        if name == "typed_c" { None } else { Some("") }
    );
    format!(
        "action 'Compiling {source}{}'\n  Mnemonic: CppCompile\n  Target: {target}\n  Configuration: {cfg}\n  Execution platform: //:platform\n  Inputs: [{}]\n  Outputs: [{}]\n{env}  Command Line: (exec {command})\n# Configuration: {cfg}\n# Execution platform: //:platform\n  ExecutionInfo: {{owner-only: 'a b', requires-network: {info}}}\n",
        if tool { " [for tool]" } else { "" },
        paths.join(", "),
        outputs.join(", ")
    )
}

fn public_borrows(action: &slug_analysis_v2::ConfiguredAction) {
    let view = ConfiguredCppCompileActionView::from_configured_action(action).unwrap();
    assert!(std::ptr::eq(view.action(), action));
    assert!(std::ptr::eq(
        view.spec(),
        action.cpp_compile_spec().unwrap()
    ));
    assert!(std::ptr::eq(view.data(), view.spec().data()));
    assert!(std::ptr::eq(view.context(), action.context().as_ref()));
    assert!(std::ptr::eq(view.owner(), action.context().owner()));
    assert!(std::ptr::eq(
        view.execution_platform(),
        action.context().execution_platform().unwrap()
    ));
    assert!(std::ptr::eq(view.source(), &view.data().source));
    assert!(std::ptr::eq(
        view.object_output(),
        view.spec().object_output()
    ));
    assert!(std::ptr::eq(view.dotd_output(), view.spec().dotd_output()));
    assert!(std::ptr::eq(
        view.data().selected_tool.as_ptr(),
        view.spec().data().selected_tool.as_ptr()
    ));
    assert!(std::ptr::eq(
        view.mandatory_spawn_inputs(),
        view.spec().mandatory_spawn_inputs()
    ));
    assert!(std::ptr::eq(
        view.cache_key_inputs(),
        view.spec().cache_key_inputs()
    ));
    assert!(std::ptr::eq(
        view.declared_headers(),
        view.spec().declared_headers()
    ));
    assert_eq!(view.context().exec_group(), &ConfiguredExecGroup::Default);
    assert_eq!(
        view.context().aspect(),
        ConfiguredActionAspectProvenance::Absent
    );
    assert!(action.exec_group().is_none());
    assert!(action.exec_properties().is_empty());
    assert_eq!(
        view.context().raw_platform_fact(),
        view.context().platform_fact()
    );
    assert_eq!(
        view.execution_platform().label().to_string(),
        "@@//:platform"
    );
    let cfg = view.owner().configuration().slug_configuration().unwrap();
    assert!(!cfg.cpp_compile_include_scanning().unwrap());
    assert!(cfg.cpp_compile_execution_modifiers_empty().unwrap());
    assert!(
        !view
            .data()
            .feature_configuration
            .is_requested("cc_include_scanning")
    );
    for feature in [
        "header_modules",
        "cpp_modules",
        "use_header_modules",
        "parse_showincludes",
        "compiler_param_file",
    ] {
        assert!(!view.data().feature_configuration.is_enabled(feature));
    }
    assert_eq!(
        cfg.configured_action_path_flavor().unwrap(),
        slug_configuration_v2::HostPathFlavor::Unix
    );
    assert_eq!(view.object_output().root().configuration(), cfg);
    assert_eq!(view.object_output().root(), view.dotd_output().root());
    let [ArtifactInputSource::Depset(mandatory)] = view.mandatory_spawn_inputs().sources() else {
        panic!("public producer must retain one mandatory depset")
    };
    assert_eq!(mandatory.order(), DepsetOrder::Postorder);
    let mut original = Vec::new();
    mandatory.visit(|a| original.push(a.clone())).unwrap();
    assert!(original.contains(view.source()));
    assert_eq!(
        original
            .iter()
            .map(|a| a.path().into_owned())
            .collect::<Vec<_>>(),
        [
            "header.h",
            "static.a",
            "dynamic.so",
            view.source().path().as_ref()
        ]
    );
    assert_ne!(
        &original[0],
        view.source(),
        "original mandatory input is not inferred from source"
    );
    let mut union = original;
    for role in [view.cache_key_inputs(), view.declared_headers()] {
        role.visit(|a| {
            if !union.contains(a) {
                union.push(a.clone());
            }
        })
        .unwrap();
    }
    assert_eq!(union.len(), 9);
    let mut cache_count = 0;
    view.cache_key_inputs().visit(|_| cache_count += 1).unwrap();
    assert_eq!(cache_count, 0);
    assert!(action.render_argv().is_err());
    assert!(slug_build_api_v2::ReapiCommandProjection::from_action(action).is_err());
}

#[tokio::test]
async fn cpp_compile_public_aquery_fields_scopes_and_tool_owners() {
    let dice = cpp_dice();
    let mut tx = cpp_transaction_for_epoch(&dice, public_epoch(1, "A")).await;
    let mut cases = 0;
    for (target, external, tool) in [
        ("main", false, false),
        ("main_exec", false, true),
        ("external", true, false),
        ("external_exec", true, true),
    ] {
        let outcome = tx.compute(&cpp_key(&format!("//:{target}"))).await.unwrap();
        let evaluation = cpp_evaluation(&outcome);
        let owners = public_owners(evaluation);
        assert_eq!(owners.len(), 3);
        let deps = cpp_text(evaluation, AqueryScope::Deps);
        let literal = cpp_text(evaluation, AqueryScope::Literal);
        assert_eq!(cpp_records(&literal).len(), 1);
        assert!(literal.contains(&format!("  Target: //:{target}\n")));
        assert_eq!(
            cpp_records(&deps).len(),
            if tool { 5 } else { 4 },
            "{target}: {deps}"
        );
        let expected_order = evaluation
            .action_closure
            .owners()
            .iter()
            .flat_map(|o| o.actions().iter().map(|a| a.mnemonic()))
            .collect::<Vec<_>>();
        let blocks = cpp_records(&deps);
        assert_eq!(blocks[0], cpp_records(&literal)[0]);
        let mut expected_public = Vec::new();
        assert_eq!(
            blocks
                .iter()
                .map(|b| b
                    .lines()
                    .find_map(|l| l.strip_prefix("  Mnemonic: "))
                    .unwrap())
                .collect::<Vec<_>>(),
            expected_order
        );
        for owner in owners {
            let name = owner
                .configured_target_key()
                .unwrap()
                .label()
                .target()
                .as_str();
            let expected_bits: &[bool] = match (name, tool) {
                ("typed_pic", true) => &[true, false],
                ("typed_pic", false) => &[true],
                ("typed_compile" | "typed_c", _) => &[false],
                _ => panic!("unexpected public compile owner"),
            };
            assert_eq!(owner.actions().len(), expected_bits.len());
            for (action, &pic) in owner.actions().iter().zip(expected_bits) {
                assert_eq!(
                    action.context().owner(),
                    owner.configured_target_key().unwrap()
                );
                public_borrows(action);
                let view = ConfiguredCppCompileActionView::from_configured_action(action).unwrap();
                assert_eq!(
                    view.owner().configuration().kind()
                        == slug_analysis_v2::ConfigurationKind::Exec,
                    tool
                );
                let flags: &[&str] = if view.owner().label().target().as_str() == "typed_compile" {
                    &["-DRECIPE_A"]
                } else {
                    &[]
                };
                let before = view.spec().clone();
                let variables = slug_build_api_v2::AnalysisValue::cc_toolchain_variables(
                    view.data().variables.clone(),
                );
                let cfg = view.owner().configuration().slug_configuration().unwrap();
                assert_eq!(
                    slug_configuration_v2::CppFragmentProjection::new(cfg.clone())
                        .unwrap()
                        .compilation_mode()
                        .unwrap(),
                    if tool { "opt" } else { "fastbuild" }
                );
                let expected = public_block(view, external, tool, pic, flags);
                let effective = view
                    .spec()
                    .effective_environment(&Default::default())
                    .unwrap();
                assert_eq!(effective.get("PWD"), Some("/proc/self/cwd"));
                assert_eq!(
                    effective.get("FEATURE_ONLY"),
                    Some(view.source().path().as_ref())
                );
                assert!(view.data().environment.fixed().get("PWD").is_none());
                assert!(
                    view.data()
                        .environment
                        .fixed()
                        .get("FEATURE_ONLY")
                        .is_none()
                );
                assert_eq!(
                    blocks.iter().filter(|b| **b == expected).count(),
                    1,
                    "{deps}"
                );
                assert_eq!(cpp_text(evaluation, AqueryScope::Deps), deps);
                assert_eq!(view.spec(), &before);
                assert!(variables.publication_eq(
                    &slug_build_api_v2::AnalysisValue::cc_toolchain_variables(
                        view.data().variables.clone()
                    )
                ));
                for absent in [
                    "  ActionKey:",
                    "  SlugActionToken:",
                    "PWD=",
                    "INHERITED_ONLY",
                    "FEATURE_ONLY",
                    "(TreeArtifact)",
                ] {
                    assert!(!expected.contains(absent));
                }
                expected_public.push(expected);
                cases += 1;
            }
        }
        assert_eq!(
            blocks
                .iter()
                .filter(|b| b.contains("  Mnemonic: CppCompile\n"))
                .copied()
                .collect::<Vec<_>>(),
            expected_public
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        let mut expected_deps = literal;
        for block in expected_public {
            expected_deps.push_str(&block);
            expected_deps.push('\n');
        }
        assert_eq!(deps, expected_deps);
    }
    assert_eq!(cases, 14);
}

#[tokio::test]
async fn cpp_compile_public_aquery_restores_source_configuration_and_owned_text() {
    let dice = cpp_dice();
    let key = cpp_key("//:main");
    let mut tx = cpp_transaction_for_epoch(&dice, public_epoch(10, "A")).await;
    let first = tx.compute(&key).await.unwrap();
    let owners = public_owners(cpp_evaluation(&first))
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let specs = owners
        .iter()
        .map(|o| o.actions()[0].cpp_compile_spec().unwrap().clone())
        .collect::<Vec<_>>();
    let variables = specs
        .iter()
        .map(|s| {
            slug_build_api_v2::AnalysisValue::cc_toolchain_variables(s.data().variables.clone())
        })
        .collect::<Vec<_>>();
    let text = cpp_text(cpp_evaluation(&first), AqueryScope::Deps);
    drop(tx);
    let changed_epoch = public_epoch(11, "B','','-DRECIPE_B");
    let mut tx = cpp_transaction_for_epoch(&dice, changed_epoch).await;
    let changed = tx.compute(&key).await.unwrap();
    let changed_text = cpp_text(cpp_evaluation(&changed), AqueryScope::Deps);
    assert_ne!(changed_text, text);
    assert!(changed_text.contains("-DRECIPE_B \\\n    '' \\\n    -DRECIPE_B"));
    let changed_owners = public_owners(cpp_evaluation(&changed));
    assert_ne!(
        changed_owners
            .iter()
            .map(|o| o.actions())
            .collect::<Vec<_>>(),
        owners.iter().map(|o| o.actions()).collect::<Vec<_>>()
    );
    assert_eq!(cpp_text(cpp_evaluation(&first), AqueryScope::Deps), text);
    drop(changed);
    drop(tx);
    let mut tx = cpp_transaction_for_epoch(&dice, public_epoch(12, "A")).await;
    let restored = tx.compute(&key).await.unwrap();
    assert_eq!(
        public_owners(cpp_evaluation(&restored)),
        owners.iter().collect::<Vec<_>>()
    );
    assert_eq!(cpp_text(cpp_evaluation(&restored), AqueryScope::Deps), text);
    let mut config_key = key.clone();
    config_key.configuration_overlay = vec![
        slug_configuration_v2::CommandConfigurationOccurrence::native(
            slug_configuration_v2::NativeCommandOption::Copt,
            Some("-DCONFIG"),
            false,
        ),
    ]
    .into();
    let configured = tx.compute(&config_key).await.unwrap();
    let configured_text = cpp_text(cpp_evaluation(&configured), AqueryScope::Deps);
    assert_ne!(configured_text, text);
    assert!(configured_text.contains("-DCONFIG \\\n    -DRECIPE_A"));
    assert_ne!(
        public_owners(cpp_evaluation(&configured))
            .iter()
            .map(|o| o.actions())
            .collect::<Vec<_>>(),
        owners.iter().map(|o| o.actions()).collect::<Vec<_>>()
    );
    assert_ne!(
        public_owners(cpp_evaluation(&configured))[0]
            .configured_target_key()
            .unwrap()
            .configuration(),
        owners[0].configured_target_key().unwrap().configuration()
    );
    let restored_configuration = tx.compute(&key).await.unwrap();
    assert_eq!(
        public_owners(cpp_evaluation(&restored_configuration)),
        owners.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        cpp_text(cpp_evaluation(&restored_configuration), AqueryScope::Deps),
        text
    );
    for ((owner, spec), variables) in owners.iter().zip(&specs).zip(&variables) {
        assert_eq!(owner.actions()[0].cpp_compile_spec().unwrap(), spec);
        assert!(variables.publication_eq(
            &slug_build_api_v2::AnalysisValue::cc_toolchain_variables(
                spec.data().variables.clone()
            )
        ));
    }
    drop(restored_configuration);
    drop(configured);
    drop(restored);
    drop(first);
    drop(variables);
    drop(specs);
    drop(owners);
    drop(tx);
    drop(dice);
    assert_eq!(cpp_records(&text).len(), 4);
    assert!(text.contains("-DRECIPE_A"));
    assert!(!text.contains("-DRECIPE_B"));
}
