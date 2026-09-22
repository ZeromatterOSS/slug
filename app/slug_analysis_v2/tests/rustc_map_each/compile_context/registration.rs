//! Pinned CppCompile constructor/publication evidence; inert tools never execute.
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ArtifactInputs;

use super::*;

pub(super) const NATIVE_MODES: &[&str] = &[
    "malformed_without",
    "malformed_fallback",
    "fake_ctx",
    "fake_features",
    "fake_toolchain",
    "foreign_toolchain_key",
    "schemaless_toolchain",
    "fake_context",
    "fake_header",
    "malformed_headers",
    "source_output",
    "foreign_output",
    "tree_output",
    "tree_source",
    "bad_source",
    "malformed_builtin",
    "scan_true",
    "scan_false",
    "scan_requested_default",
    "suspended",
];

#[tokio::test]
async fn native_compile_genuine_bridge_rejects_malformed_inputs_and_scanning() {
    let root = workspace();
    let dice = registry::dice(&root);
    for (mode, needle) in [
        ("malformed_without", "_compiler_files_without_includes"),
        ("malformed_fallback", "_compiler_files"),
        ("fake_ctx", "genuine rule ctx"),
        ("fake_features", "native FeatureConfiguration"),
        ("fake_toolchain", "schema-backed CcToolchainInfo"),
        (
            "foreign_toolchain_key",
            "native CcToolchainInfo provider key",
        ),
        ("schemaless_toolchain", "schema-backed CcToolchainInfo"),
        ("fake_context", "CcCompilationContextInfo provider"),
        ("fake_header", "native HeaderInfo"),
        ("malformed_headers", "depset of File"),
        ("source_output", "requires a declared file"),
        ("foreign_output", "requires a declared file"),
        ("tree_output", "distinct regular object and dotd"),
        ("tree_source", "source must be a regular File"),
        ("bad_source", "regular C/C++ source Files"),
        ("malformed_builtin", "builtin include files requires a File"),
        ("scan_true", "include scanning is unsupported"),
        ("suspended", "outside of its own implementation function"),
    ] {
        let error = request(&dice, &root, &format!("@@//:native_{mode}"), config(&[]))
            .await
            .err()
            .expect("bounded constructor rejection");
        assert!(error.contains(needle), "{mode}: {error}");
        assert!(!error.contains("unexpectedly succeeded"), "{mode}: {error}");
    }
    // Explicit false and requested-only scanning both remain dotd recipes.
    // Structural nondefault scanning flags are separately tested at their owner.
    for mode in ["scan_false", "scan_requested_default"] {
        let result = request(&dice, &root, &format!("@@//:native_{mode}"), config(&[]))
            .await
            .unwrap();
        assert_eq!(
            result
                .actions()
                .iter()
                .filter(|action| action.cpp_compile_spec().is_some())
                .count(),
            3
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unchanged_public_compile_registers_regular_c_source() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "@@//:typed_c", config(&[]))
        .await
        .unwrap();
    assert_eq!(result.actions().len(), 1);
    let spec = result.actions()[0].cpp_compile_spec().unwrap();
    assert_eq!(spec.data().action_name, "c-compile");
    assert_eq!(spec.source().path().as_ref(), "allocator_library.c");
    assert_eq!(
        spec.object_output().root_relative_path(),
        "_objs/typed_c/allocator_library.o"
    );
    assert_eq!(
        spec.dotd_output().root_relative_path(),
        "_objs/typed_c/allocator_library.d"
    );
    assert_eq!(spec.data().selected_tool, "toolchain/bin/gcc");
    assert_eq!(
        input_paths(spec.mandatory_spawn_inputs()),
        ["header.h", "static.a", "dynamic.so", "allocator_library.c"]
    );
    fs::remove_dir_all(root).unwrap();
}

fn input_paths(inputs: &ArtifactInputs) -> Vec<String> {
    let mut paths = Vec::new();
    inputs
        .visit(|file| {
            let path = file.path().into_owned();
            if !paths.contains(&path) {
                paths.push(path);
            }
        })
        .unwrap();
    paths
}

fn check_compile(result: &ConfiguredNodeResult, main: bool, pic: bool) {
    assert_eq!(result.actions().len(), 1);
    let action = &result.actions()[0];
    assert_eq!(action.kind(), &ActionKind::CppCompile);
    assert_eq!(action.mnemonic(), "CppCompile");
    assert!(action.spawn_spec().is_none());
    let spec = action
        .cpp_compile_spec()
        .expect("distinct typed CppCompile");
    assert_eq!(spec.source().path().as_ref(), "allocator_library.cc");
    assert_eq!(spec.outputs().len(), 2);
    assert_eq!(spec.data().action_name, "c++-compile");
    assert_eq!(spec.data().selected_tool, "toolchain/bin/gcc");
    assert_eq!(spec.data().use_pic, pic);
    assert!(spec.data().needs_include_validation);
    assert_eq!(
        input_paths(spec.mandatory_spawn_inputs()),
        ["header.h", "static.a", "dynamic.so", "allocator_library.cc"]
    );
    assert!(input_paths(spec.cache_key_inputs()).is_empty());
    let headers = input_paths(spec.declared_headers());
    for path in [
        "public.h",
        "private.h",
        "textual.h",
        "ordinary.h",
        "implementation.h",
        "header.h",
    ] {
        assert!(
            headers.iter().any(|header| header == path),
            "{path}: {headers:?}"
        );
    }
    assert!(
        !headers
            .iter()
            .any(|header| header == "allocator_library.cc")
    );
    assert_eq!(input_paths(&spec.data().builtin_inputs), ["header.h"]);
    assert_eq!(
        spec.data().builtin_include_directories.as_ref(),
        ["toolchain/include"]
    );
    let name = if pic { "typed_pic" } else { "typed_compile" };
    let source = if pic {
        "allocator_library.pic"
    } else {
        "allocator_library"
    };
    let repo = if main { "" } else { "external/rules_rust+/" };
    assert_eq!(
        spec.object_output().root_relative_path(),
        format!("{repo}_objs/{name}/{source}.o")
    );
    assert_eq!(
        spec.dotd_output().root_relative_path(),
        format!("{repo}_objs/{name}/{source}.d")
    );
    assert_eq!(spec.object_output().root(), spec.dotd_output().root());
    assert_eq!(
        spec.object_output().root().configuration(),
        result
            .configured_target_key()
            .unwrap()
            .configuration()
            .slug_configuration()
            .unwrap()
    );
    assert_eq!(
        action.context().owner(),
        result.configured_target_key().unwrap()
    );
    assert_eq!(
        action.context().exec_group(),
        &slug_analysis_v2::ConfiguredExecGroup::Default
    );
    assert!(action.render_argv().is_err(), "no empty command projection");
    assert!(slug_build_api_v2::ReapiCommandProjection::from_action(action).is_err());
    assert!(
        result.configured_file_write_actions().is_err(),
        "aquery family remains deferred"
    );
}

#[tokio::test]
async fn unchanged_public_compile_registers_main_external_target_exec_dotd_recipes() {
    let root = workspace();
    let dice = registry::dice(&root);
    let target = config(&[]);
    let exec = ConfigurationKey::from_slug(target.slug_configuration().unwrap().to_exec().unwrap());
    for (label, main) in [("typed_compile", false), ("@@//:typed_compile", true)] {
        for configuration in [target.clone(), exec.clone()] {
            let result = request(&dice, &root, label, configuration).await.unwrap();
            check_compile(&result, main, false);
        }
    }
    let pic = request(&dice, &root, "typed_pic", target).await.unwrap();
    check_compile(&pic, false, true);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn compile_recipe_metadata_and_unused_configuration_restore_in_same_dice() {
    let root = workspace();
    let dice = registry::dice(&root);
    let first = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let build = root.join("BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    fs::write(&build, original.replace("-DRECIPE_A", "-DRECIPE_B")).unwrap();
    let changed = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_ne!(first.actions(), changed.actions());
    assert_eq!(first.actions()[0].outputs(), changed.actions()[0].outputs());
    check_compile(&changed, true, false);
    fs::write(&build, &original).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .unwrap()
    );
    let configured = request(
        &dice,
        &root,
        "@@//:typed_compile",
        config(&[(
            NativeCommandOption::HostCopt,
            "unused-compile-configuration",
        )]),
    )
    .await
    .unwrap();
    assert_ne!(first.actions(), configured.actions());
    assert_eq!(
        first,
        request(&dice, &root, "@@//:typed_compile", config(&[]))
            .await
            .unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_compile_snapshots_tools_but_publishes_final_live_recipes_in_registration_order() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "@@//:registration_lifecycle", config(&[]))
        .await
        .unwrap();
    assert_eq!(
        result
            .actions()
            .iter()
            .map(|action| action.mnemonic())
            .collect::<Vec<_>>(),
        [
            "FileWrite",
            "CppCompile",
            "FileWrite",
            "CppCompile",
            "CppCompile",
            "FileWrite"
        ]
    );
    let AnalysisValueKind::CcToolchainVariables(published) = field(&result, "variables").kind()
    else {
        panic!("published Variables");
    };
    let final_context = field(&result, "context");
    assert_eq!(
        paths(member(final_context, "direct_headers")),
        ["header.h", "private.h"]
    );
    for (index, ordinal) in [1, 3, 4].into_iter().enumerate() {
        let action = &result.actions()[ordinal];
        let recipe = action.cpp_compile_spec().unwrap();
        assert_eq!(
            recipe
                .data()
                .execution_requirements
                .iter()
                .collect::<Vec<_>>(),
            [("custom-compile-tool", ""), ("no-sandbox", "")]
        );
        assert_eq!(
            recipe.data().variables.occurrence(),
            published.occurrence(),
            "actions/providers must lower through one publication memo"
        );
        let live = recipe
            .data()
            .variables
            .model()
            .locals
            .iter()
            .find(|(name, _)| name == "live")
            .unwrap();
        assert_eq!(strings(&live.1), ["before", "after"]);
        assert!(
            recipe
                .data()
                .compilation_context
                .publication_eq(final_context)
        );
        let AnalysisValueKind::Depset(headers) = member(final_context, "headers").kind() else {
            panic!("context headers depset");
        };
        let mut relinked = recipe.data().clone();
        relinked.construction.declared_headers =
            ArtifactInputs::new(vec![slug_build_api_v2::ArtifactInputSource::Depset(
                slug_build_api_v2::RetainedArtifactInputs::new(headers.clone()).unwrap(),
            )]);
        // Equality's shared alias bijection would reject this relinking if the
        // native bridge had eagerly copied the context's depset occurrence.
        assert!(
            recipe == &slug_build_api_v2::CppCompileSpec::new(relinked).unwrap(),
            "declared-header role must share the published context's depset"
        );
        assert_eq!(
            recipe.object_output().root_relative_path(),
            format!("registration_lifecycle.{index}.o")
        );
        assert_eq!(
            recipe.dotd_output().root_relative_path(),
            format!("registration_lifecycle.{index}.d")
        );
        let builtin = input_paths(&recipe.data().builtin_inputs);
        assert_eq!(
            builtin,
            if index == 0 {
                vec!["header.h"]
            } else {
                vec!["header.h", "private.h"]
            }
        );
        let dirs = recipe
            .data()
            .builtin_include_directories
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            dirs,
            if index == 0 {
                vec!["toolchain/include"]
            } else {
                vec!["toolchain/include", "late/headers"]
            }
        );
        let mandatory = input_paths(recipe.mandatory_spawn_inputs());
        assert_eq!(
            mandatory,
            [
                "private.h",
                "header.h",
                "static.a",
                "dynamic.so",
                "allocator_library.cc"
            ]
        );
        assert_eq!(input_paths(recipe.cache_key_inputs()), ["changed.h"]);
        assert!(!mandatory.iter().any(|path| path == "changed.h"));
        assert_eq!(input_paths(recipe.declared_headers()), ["header.h"]);
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_compile_cross_family_and_pending_conflicts_fail_entire_publication() {
    let root = workspace();
    let dice = registry::dice(&root);
    for suffix in ["before", "after", "dotd", "pending"] {
        let error = request(
            &dice,
            &root,
            &format!("@@//:registration_conflict_{suffix}"),
            config(&[]),
        )
        .await
        .err()
        .expect("conflicting pending action must not publish");
        let extension = if matches!(suffix, "dotd" | "pending") {
            "d"
        } else {
            "o"
        };
        assert!(
            error.contains("action output declared more than once:"),
            "{suffix}: {error}"
        );
        assert!(
            error.contains(&format!("registration_conflict_{suffix}.0.{extension}")),
            "{error}"
        );
    }
    // Failed evaluations cannot leave reservations in another configured rule.
    let good = request(&dice, &root, "@@//:registration_lifecycle", config(&[]))
        .await
        .unwrap();
    assert_eq!(good.actions().len(), 6);
    fs::remove_dir_all(root).unwrap();
}
