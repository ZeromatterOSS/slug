//! Complete published commands from unchanged public compile; tools remain inert.
use slug_build_api_v2::CppCompileSpec;
use slug_configuration_v2::CanonicalStringMap;

use super::*;

fn expected(spec: &CppCompileSpec, main: bool, exec: bool, user: &[&str]) -> Vec<String> {
    // legacy_features: dependency, random seed, PIC, defines, include paths;
    // get_features_to_appear_last: user flags, sysroot, compiler input/output.
    // get_copts adds host -g0 once for C and twice for C++ in exec configs.
    let output = spec.object_output().path();
    let mut argv = vec![
        "toolchain/bin/gcc".to_owned(),
        "-MD".into(),
        "-MF".into(),
        spec.dotd_output().path().to_owned(),
        format!("-frandom-seed={output}"),
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
    let bin = spec.object_output().root().path();
    let repo = if main {
        ".".to_owned()
    } else {
        "external/rules_rust+".to_owned()
    };
    let generated = if main {
        bin.to_owned()
    } else {
        format!("{bin}/external/rules_rust+")
    };
    for quote in [
        repo,
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
    if exec {
        argv.push("-g0".into());
        if spec.data().action_name == "c++-compile" {
            argv.push("-g0".into());
        }
    }
    argv.extend(user.iter().map(|text| (*text).to_owned()));
    argv.extend([
        "--sysroot=/sysroot".into(),
        "-c".into(),
        spec.source().path().into_owned(),
        "-o".into(),
        output.to_owned(),
    ]);
    argv
}

fn check(result: &ConfiguredNodeResult, main: bool, exec: bool, flags: &[&str]) -> Vec<String> {
    assert_eq!(result.actions().len(), 1);
    let action = &result.actions()[0];
    let spec = action.cpp_compile_spec().unwrap();
    let before = spec.clone();
    let expected = expected(spec, main, exec, flags);
    for _ in 0..2 {
        assert_eq!(
            spec.expand_command_line()
                .unwrap()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        let env = spec
            .effective_environment(&CanonicalStringMap::default())
            .unwrap();
        assert_eq!(
            env,
            CanonicalStringMap::from_pairs(
                spec.data()
                    .environment
                    .fixed()
                    .iter()
                    .chain([("PWD", "/proc/self/cwd")])
            )
        );
        assert_eq!(spec, &before);
    }
    // Typed rendering is not generic command/aquery/REAPI admission.
    assert!(action.render_argv().is_err());
    assert!(slug_build_api_v2::ReapiCommandProjection::from_action(action).is_err());
    expected
}

#[tokio::test]
async fn published_public_compile_expands_complete_c_cpp_pic_main_external_target_exec_commands() {
    let root = workspace();
    let dice = registry::dice(&root);
    let target = config(&[]);
    let exec = ConfigurationKey::from_slug(target.slug_configuration().unwrap().to_exec().unwrap());
    for main in [false, true] {
        for (configuration, is_exec) in [(target.clone(), false), (exec.clone(), true)] {
            for name in ["typed_compile", "typed_c", "typed_pic"] {
                let label = if main {
                    format!("@@//:{name}")
                } else {
                    name.to_owned()
                };
                let result = request(&dice, &root, &label, configuration.clone())
                    .await
                    .unwrap();
                check(
                    &result,
                    main,
                    is_exec,
                    if name == "typed_compile" {
                        &["-DRECIPE_A"]
                    } else {
                        &[]
                    },
                );
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_compile_commands_restore_flags_and_configuration_without_recipe_mutation() {
    let root = workspace();
    let dice = registry::dice(&root);
    let first = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let original = check(&first, true, false, &["-DRECIPE_A"]);
    let build = root.join("BUILD.bazel");
    let source = fs::read_to_string(&build).unwrap();
    fs::write(
        &build,
        source.replace("'-DRECIPE_A'", "'-DRECIPE_B','','-DRECIPE_B'"),
    )
    .unwrap();
    let changed = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_ne!(
        original,
        check(&changed, true, false, &["-DRECIPE_B", "", "-DRECIPE_B"])
    );
    fs::write(&build, &source).unwrap();
    let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(original, check(&restored, true, false, &["-DRECIPE_A"]));
    let altered = request(
        &dice,
        &root,
        "@@//:typed_compile",
        config(&[(NativeCommandOption::Copt, "-DCONFIG")]),
    )
    .await
    .unwrap();
    assert_ne!(
        original,
        check(&altered, true, false, &["-DCONFIG", "-DRECIPE_A"])
    );
    let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(original, check(&restored, true, false, &["-DRECIPE_A"]));
    check(&first, true, false, &["-DRECIPE_A"]);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_compile_argv_uses_fresh_overlay_while_original_dotted_cache_stays_stale() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "@@//:native_command_cache", config(&[]))
        .await
        .unwrap();
    assert_eq!(
        strings(field(&result, "seeded")),
        ["--cached=before", "--direct=before"]
    );
    assert_eq!(
        strings(field(&result, "original_argv")),
        ["--cached=before", "--direct=before", "--direct=after"]
    );
    let published = field(&result, "variables").clone();
    // The lifecycle caller registers exactly three compiler actions, with
    // ordinary writes between them. Neither filtered loop may pass vacuously.
    assert_eq!(
        result
            .actions()
            .iter()
            .filter(|action| action.cpp_compile_spec().is_some())
            .count(),
        3
    );
    for action in result
        .actions()
        .iter()
        .filter(|action| action.cpp_compile_spec().is_some())
    {
        let spec = action.cpp_compile_spec().unwrap();
        let before = spec.clone();
        let snapshot =
            slug_build_api_v2::AnalysisValue::cc_toolchain_variables(spec.data().variables.clone());
        for _ in 0..2 {
            assert_eq!(
                spec.expand_command_line().unwrap(),
                [
                    "toolchain/bin/gcc",
                    "--cached=before",
                    "--cached=after",
                    "--direct=before",
                    "--direct=after"
                ]
            );
            let expected =
                CanonicalStringMap::from_pairs(spec.data().environment.fixed().iter().chain([
                    ("PWD", "/proc/self/cwd"),
                    ("SOURCE", "allocator_library.cc"),
                ]));
            assert_eq!(
                spec.effective_environment(&CanonicalStringMap::default())
                    .unwrap(),
                expected
            );
            assert_eq!(spec, &before);
            assert!(snapshot.publication_eq(&published));
        }
    }
    // Successful analysis precedes both failures: unused integer metadata is
    // retained lazily and becomes an error only when a selected expansion reads it.
    let bad = request(&dice, &root, "@@//:native_command_error", config(&[]))
        .await
        .unwrap();
    assert_eq!(
        bad.actions()
            .iter()
            .filter(|action| action.cpp_compile_spec().is_some())
            .count(),
        3
    );
    for action in bad
        .actions()
        .iter()
        .filter(|action| action.cpp_compile_spec().is_some())
    {
        let spec = action.cpp_compile_spec().unwrap();
        let before = spec.clone();
        for _ in 0..2 {
            assert!(spec.expand_command_line().is_err());
            assert!(
                spec.effective_environment(&CanonicalStringMap::default())
                    .is_err()
            );
            assert_eq!(spec, &before);
        }
    }
    fs::remove_dir_all(root).unwrap();
}
