//! Pure preparation of genuinely published actions, not execution admission.
use slug_build_api_v2::CppCompileInputSets;
use slug_build_api_v2::CppCompileSpec;

use super::*;

fn only_recipe(result: &ConfiguredNodeResult) -> &CppCompileSpec {
    assert_eq!(result.actions().len(), 1);
    result.actions()[0].cpp_compile_spec().unwrap()
}

fn check(result: &ConfiguredNodeResult) -> CppCompileInputSets {
    let recipe = only_recipe(result);
    let before = recipe.clone();
    let inputs = recipe.input_sets().unwrap();
    for _ in 0..2 {
        let prepared = recipe.prepare_inputs_posix().unwrap();
        assert_eq!(prepared.inputs(), &inputs);
        assert!(prepared.warnings().is_empty(), "{:?}", prepared.warnings());
        assert_eq!(recipe, &before);
    }
    assert!(result.actions()[0].render_argv().is_err());
    assert!(slug_build_api_v2::ReapiCommandProjection::from_action(&result.actions()[0]).is_err());
    inputs
}

#[tokio::test]
async fn published_c_cpp_pic_preparation_preserves_main_external_target_exec_input_sets() {
    let root = workspace();
    let dice = registry::dice(&root);
    // Match the target's fastbuild mode so supports_pic has one compilation
    // recipe. The opt PIC/non-PIC pair is covered in provider.rs's matrix.
    let target = config(&[(NativeCommandOption::HostCompilationMode, "fastbuild")]);
    let exec = ConfigurationKey::from_slug(target.slug_configuration().unwrap().to_exec().unwrap());
    for main in [false, true] {
        for configuration in [target.clone(), exec.clone()] {
            for name in ["typed_compile", "typed_c", "typed_pic"] {
                let label = if main {
                    format!("@@//:{name}")
                } else {
                    name.to_owned()
                };
                let result = request(&dice, &root, &label, configuration.clone())
                    .await
                    .unwrap();
                check(&result);
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_preparation_rejects_build_and_configuration_flags_then_restores_in_same_dice() {
    let root = workspace();
    let dice = registry::dice(&root);
    let first = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let original_inputs = check(&first);
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    for (flags, invalid) in [
        (vec!["-Id/../../escape"], "../escape"),
        (vec!["-I/outside"], "/outside"),
        (vec!["-iquote", "../quotes"], "../quotes"),
        (vec!["-isystem/system"], "/system"),
    ] {
        let changed = original.replace(
            "compile_flags=['-DRECIPE_A']",
            &format!("compile_flags={flags:?}"),
        );
        assert_ne!(
            changed, original,
            "fixture mutation must select the real public caller"
        );
        fs::write(&build, changed).unwrap();
        let result = request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .unwrap();
        let recipe = only_recipe(&result);
        let before = recipe.clone();
        for _ in 0..2 {
            let error = recipe
                .prepare_inputs_posix()
                .err()
                .expect("escaping include rejected after successful analysis");
            assert!(error.contains(invalid), "{flags:?}: {error}");
            assert_eq!(recipe, &before);
        }
        fs::write(&build, &original).unwrap();
        let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .unwrap();
        assert_eq!(first, restored);
        assert_eq!(original_inputs, check(&restored));
    }
    // A native dangling raw quote flag warns/skips rather than rejecting.
    fs::write(
        &build,
        original.replace("compile_flags=['-DRECIPE_A']", "compile_flags=['-iquote']"),
    )
    .unwrap();
    let dangling = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let recipe = only_recipe(&dangling);
    let before = recipe.clone();
    let prepared = recipe.prepare_inputs_posix().unwrap();
    assert_eq!(prepared.inputs(), &recipe.input_sets().unwrap());
    assert_eq!(prepared.warnings().len(), 1);
    assert!(prepared.warnings()[0].contains("-iquote"));
    assert_eq!(recipe, &before);
    fs::write(&build, &original).unwrap();
    let configured = request(
        &dice,
        &root,
        "@@//:typed_compile",
        config(&[(NativeCommandOption::Copt, "-isystem/configuration-outside")]),
    )
    .await
    .unwrap();
    let error = only_recipe(&configured)
        .prepare_inputs_posix()
        .err()
        .expect("configuration flag is consumed");
    assert!(error.contains("/configuration-outside"), "{error}");
    let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(original_inputs, check(&restored));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_preparation_uses_original_cached_options_not_fresh_command_argv() {
    let root = workspace();
    // Adapt only our authored caller before creating selected-registry/DICE
    // observations. All 54 upstream bodies remain unchanged. The first native
    // query seeds a safe dotted sequence; registration precedes list growth.
    let path = root.join("rules_cc/cc/private/compile_registration_proof.bzl");
    let original = fs::read_to_string(&path).unwrap();
    let mut subject = original.clone();
    for (from, to) in [
        ("--cached=%{obj.items}", "-isystem%{obj.items}"),
        (
            "flag_group(iterate_over = \"live\", flags = [\"--direct=%{live}\"]),",
            "",
        ),
        ("live.append(\"after\")", "live.append(\"/outside-root\")"),
    ] {
        assert!(subject.contains(from));
        subject = subject.replace(from, to);
    }
    fs::write(path, subject).unwrap();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "@@//:native_command_cache", config(&[]))
        .await
        .unwrap();
    assert_eq!(strings(field(&result, "seeded")), ["-isystembefore"]);
    assert_eq!(strings(field(&result, "original_argv")), ["-isystembefore"]);
    let recipes = result
        .actions()
        .iter()
        .filter_map(|action| action.cpp_compile_spec())
        .collect::<Vec<_>>();
    assert_eq!(recipes.len(), 3);
    for recipe in recipes {
        let before = recipe.clone();
        for _ in 0..2 {
            assert_eq!(
                recipe.expand_command_line().unwrap(),
                [
                    "toolchain/bin/gcc",
                    "-isystembefore",
                    "-isystem/outside-root"
                ]
            );
            let prepared = recipe.prepare_inputs_posix().unwrap();
            assert!(prepared.warnings().is_empty());
            assert_eq!(prepared.inputs(), &recipe.input_sets().unwrap());
            assert_eq!(recipe, &before);
        }
    }
    fs::remove_dir_all(root).unwrap();
}
