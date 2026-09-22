//! Unchanged helper + genuine FDO subrule, not the full cc_toolchain rule.
use super::*;

const CALLER: &str = "rules_cc/cc/private/rules_impl/provider_proof.bzl";

fn workspace() -> PathBuf {
    let root = super::workspace();
    fs::write(root.join(CALLER), include_str!("provider.bzl")).unwrap();
    fs::create_dir_all(root.join("include")).unwrap();
    fs::write(root.join("include/stdc-predef.h"), "// inert builtin\n").unwrap();
    for (file, main) in [("BUILD.bazel", true), ("rules_rust/BUILD.bazel", false)] {
        let path = root.join(file);
        let old = fs::read_to_string(&path).unwrap();
        let retained = old
            .lines()
            .filter(|line| !line.starts_with("subject(name='producer',"))
            .collect::<Vec<_>>()
            .join("\n");
        let retained = if main {
            retained.replace("@@rules_rust+//:producer", ":producer")
        } else {
            retained
        };
        fs::write(path, format!("{retained}\nload('@rules_cc//cc/private/rules_impl:provider_proof.bzl','helper_subject')\nhelper_subject(name='producer',metadata='A',visibility=['//visibility:public'])\n{}",
            if main { "exports_files(['include/stdc-predef.h'])\n" } else { "" })).unwrap();
    }
    root
}

fn label(main: bool, target: &str) -> String {
    if main {
        format!("@@//:{target}")
    } else {
        target.to_owned()
    }
}

fn default_exec(configuration: &ConfigurationKey) -> ConfigurationKey {
    let configuration = configuration.slug_configuration().unwrap();
    let exec = configuration
        .to_exec_for_platform(&configuration.host_platform_label().unwrap())
        .unwrap();
    assert_eq!(
        slug_configuration_v2::CppFragmentProjection::new(exec.clone())
            .unwrap()
            .compilation_mode()
            .unwrap(),
        "opt"
    );
    ConfigurationKey::from_slug(exec)
}

fn check_provider(
    result: &ConfiguredNodeResult,
    main: bool,
    exec: bool,
    opt: bool,
    minimum: Option<&str>,
    stamp: bool,
    metadata: &str,
) {
    assert!(
        result.actions().is_empty(),
        "no-profile helper must register no actions"
    );
    let tc = field(result, "toolchain");
    assert_eq!(member(tc, "_build_variables"), field(result, "variables"));
    assert_eq!(member(tc, "_cpp_configuration"), field(result, "cpp"));
    assert_eq!(member(tc, "_fdo_context"), field(result, "fdo"));
    assert_eq!(member(tc, "all_files"), field(result, "files"));
    assert!(
        matches!(member(tc, "_is_tool_configuration").kind(), AnalysisValueKind::Boolean(v) if v == exec)
    );
    assert!(
        matches!(member(tc, "_stamp_binaries").kind(), AnalysisValueKind::Boolean(v) if v == stamp)
    );
    assert!(matches!(
        member(tc, "_is_sibling_repository_layout").kind(),
        AnalysisValueKind::Boolean(false)
    ));
    let prefix = if main { "" } else { "external/rules_rust+/" };
    for (field, tool) in [
        ("compiler_executable", "gcc"),
        ("preprocessor_executable", "cpp"),
        ("ar_executable", "ar"),
        ("ld_executable", "ld"),
        ("nm_executable", "nm"),
        ("objdump_executable", "objdump"),
        ("strip_executable", "strip"),
    ] {
        assert_eq!(
            member(tc, field).as_str().unwrap(),
            format!("{prefix}bin/{tool}")
        );
    }
    assert_eq!(
        strings(member(tc, "built_in_include_directories")),
        [
            "/proof-sysroot/include".to_owned(),
            "toolchain/include".into(),
            format!("{prefix}local/include")
        ]
    );
    assert_eq!(
        paths(member(tc, "_builtin_include_files")),
        ["include/stdc-predef.h"]
    );
    assert_eq!(member(tc, "_solib_dir").as_str(), Some("_solib_k8"));
    assert!(matches!(
        member(tc, "_static_runtime_lib_depset").kind(),
        AnalysisValueKind::None
    ));
    assert!(matches!(
        member(tc, "_dynamic_runtime_lib_depset").kind(),
        AnalysisValueKind::None
    ));
    let AnalysisValueKind::Struct(fdo) = field(result, "fdo").kind() else {
        panic!("authentic FDO struct")
    };
    let snapshot = field(result, "snapshot");
    assert_eq!(
        member(snapshot, "mode").as_str(),
        Some(if opt { "opt" } else { "fastbuild" }),
        "main={main} exec={exec} expected_opt={opt}"
    );
    assert_eq!(
        fdo.len(),
        if opt { 5 } else { 0 },
        "main={main} exec={exec} expected_opt={opt} actual_mode={:?}",
        member(snapshot, "mode").as_str()
    );
    if opt {
        for name in [
            "branch_fdo_profile",
            "prefetch_hints_artifact",
            "propeller_optimize_info",
            "memprof_profile_artifact",
            "proto_profile_artifact",
        ] {
            assert!(matches!(
                fdo.get(name).unwrap().kind(),
                AnalysisValueKind::None
            ));
        }
    }
    assert_eq!(member(snapshot, "minimum_os").as_str(), minimum);
    assert_eq!(member(snapshot, "metadata").as_str(), Some(metadata));
    assert_eq!(member(snapshot, "stack").as_str(), Some(""));
    assert_eq!(member(snapshot, "legacy").as_str(), Some("--helper-legacy"));
    assert_eq!(
        member(snapshot, "variable_sysroot").as_str(),
        Some("/proof-sysroot")
    );
}

fn check_compile(result: &ConfiguredNodeResult, main: bool, exec: bool, opt: bool, target: &str) {
    // cc_helper_internal333–354 and compile.bzl1431/1468: opt supports_pic
    // emits PIC for dynamic libraries first, then non-PIC for binaries.
    let pic_sequence: &[bool] = match (target == "typed_pic", opt) {
        (true, true) => &[true, false],
        (true, false) => &[true],
        (false, _) => &[false],
    };
    assert_eq!(result.actions().len(), pic_sequence.len());
    for (action, pic) in result.actions().iter().zip(pic_sequence) {
        assert_eq!(action.cpp_compile_spec().unwrap().data().use_pic, *pic);
        let spec = action.cpp_compile_spec().unwrap();
        let before = spec.clone();
        let mut expected = command_expansion::expected(
            spec,
            main,
            exec,
            if target == "typed_compile" {
                &["-DRECIPE_A"]
            } else {
                &[]
            },
        );
        expected[0] = if main {
            "bin/gcc".into()
        } else {
            "external/rules_rust+/bin/gcc".into()
        };
        for arg in &mut expected {
            if arg == "--sysroot=/sysroot" {
                *arg = "--sysroot=/proof-sysroot".into();
            }
        }
        for _ in 0..2 {
            assert_eq!(
                spec.expand_command_line()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            let prepared = spec.prepare_inputs_posix().unwrap();
            assert_eq!(prepared.inputs(), &spec.input_sets().unwrap());
            assert!(prepared.warnings().is_empty());
            assert_eq!(spec, &before);
        }
        assert!(action.render_argv().is_err());
        assert!(slug_build_api_v2::ReapiCommandProjection::from_action(action).is_err());
    }
}

#[tokio::test]
async fn unchanged_toolchain_helper_publishes_no_profile_providers_into_public_compile() {
    let root = workspace();
    let dice = registry::dice(&root);
    let base = config(&[]);
    let exec = default_exec(&base);
    for main in [false, true] {
        for (configuration, is_exec, opt) in [
            (base.clone(), false, false),
            (
                config(&[(NativeCommandOption::CompilationMode, "opt")]),
                false,
                true,
            ),
            (exec.clone(), true, true),
        ] {
            let producer = request(
                &dice,
                &root,
                &label(main, "producer"),
                configuration.clone(),
            )
            .await
            .unwrap();
            check_provider(&producer, main, is_exec, opt, None, false, "A");
            for name in ["typed_compile", "typed_c", "typed_pic"] {
                let result = request(&dice, &root, &label(main, name), configuration.clone())
                    .await
                    .unwrap();
                check_compile(&result, main, is_exec, opt, name);
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_toolchain_helper_restores_files_metadata_and_configuration_in_same_dice() {
    let root = workspace();
    let dice = registry::dice(&root);
    let baseline = request(&dice, &root, "@@//:producer", config(&[]))
        .await
        .unwrap();
    let compiled = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    fs::write(
        &build,
        original.replace("metadata='A'", "metadata='B',inputs=['@@//:other.a']"),
    )
    .unwrap();
    let changed = request(&dice, &root, "@@//:producer", config(&[]))
        .await
        .unwrap();
    check_provider(&changed, true, false, false, None, false, "B");
    assert_ne!(baseline, changed);
    assert_eq!(paths(field(&changed, "files")), ["other.a"]);
    let changed_compile = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    check_compile(&changed_compile, true, false, false, "typed_compile");
    assert_ne!(compiled, changed_compile);
    fs::write(&build, &original).unwrap();
    let target = config(&[
        (NativeCommandOption::MinimumOsVersion, "arbitrary-min"),
        (NativeCommandOption::Stamp, "true"),
    ]);
    let configured = request(&dice, &root, "@@//:producer", target.clone())
        .await
        .unwrap();
    check_provider(
        &configured,
        true,
        false,
        false,
        Some("arbitrary-min"),
        true,
        "A",
    );
    assert_ne!(baseline, configured);
    let configured_compile = request(&dice, &root, "@@//:typed_compile", target.clone())
        .await
        .unwrap();
    check_compile(&configured_compile, true, false, false, "typed_compile");
    assert_ne!(compiled, configured_compile);
    let exec = default_exec(&target);
    let exec_provider = request(&dice, &root, "@@//:producer", exec).await.unwrap();
    check_provider(&exec_provider, true, true, true, None, false, "A");
    let unused = request(
        &dice,
        &root,
        "@@//:producer",
        config(&[(NativeCommandOption::HostCopt, "-DUNUSED")]),
    )
    .await
    .unwrap();
    assert_ne!(baseline, unused);
    check_provider(&unused, true, false, false, None, false, "A");
    let restored = request(&dice, &root, "@@//:producer", config(&[]))
        .await
        .unwrap();
    assert_eq!(baseline, restored);
    check_provider(&restored, true, false, false, None, false, "A");
    let restored_compile = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(compiled, restored_compile);
    check_compile(&restored_compile, true, false, false, "typed_compile");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_toolchain_helper_rejects_source_tools_and_missing_inputs_then_restores() {
    let root = workspace();
    let dice = registry::dice(&root);
    let baseline = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    for (change, expected) in [
        ("failure='missing_tool'", "Tool path for 'strip' is missing"),
        ("failure='unnormalized'", "is not normalized"),
        ("inputs=['@@//:missing-tool-input']", "missing-tool-input"),
    ] {
        fs::write(
            &build,
            original.replace("metadata='A'", &format!("metadata='A',{change}")),
        )
        .unwrap();
        let error = request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .err()
            .expect("failed dependency cannot publish partial recipe");
        assert!(error.contains(expected), "{error}");
        fs::write(&build, &original).unwrap();
        let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .unwrap();
        assert_eq!(baseline, restored);
        check_compile(&restored, true, false, false, "typed_compile");
    }
    fs::remove_dir_all(root).unwrap();
}
