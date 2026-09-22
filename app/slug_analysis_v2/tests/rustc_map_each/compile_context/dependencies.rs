//! Supplied synthetic dotd over genuine published recipes. No compiler executes
//! and this explicit artifact map is not Core source-resolution authority.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::CppCompileDependencies;
use slug_build_api_v2::CppCompileSpec;
use slug_configuration_v2::CppFragmentProjection;

use super::*;

const EXEC_ROOT: &str = "/proof/execroot/main";

fn artifact_paths(values: &[AnalysisArtifact]) -> Vec<String> {
    values.iter().map(|file| file.path().into_owned()).collect()
}

fn reconcile(
    spec: &CppCompileSpec,
    bytes: &[u8],
    extra: &[AnalysisArtifact],
) -> Result<CppCompileDependencies, String> {
    let sets = spec.input_sets().unwrap();
    // All candidates are actual artifacts retained by analyzed dependencies;
    // never turn an arbitrary compiler-reported path into a source Label.
    let sources = sets
        .allowed_derived()
        .iter()
        .chain(extra)
        .filter(|file| matches!(file, AnalysisArtifact::Source(_)));
    let sources = sources.cloned().collect::<Vec<_>>();
    spec.reconcile_dotd_posix(EXEC_ROOT, bytes, |path| {
        Ok(sources.iter().find(|file| file.path() == path).cloned())
    })
}

fn only_recipe(result: &ConfiguredNodeResult) -> &CppCompileSpec {
    assert_eq!(result.actions().len(), 1);
    result.actions()[0].cpp_compile_spec().unwrap()
}

#[tokio::test]
async fn published_public_compile_dotd_prunes_unused_headers_across_main_external_target_exec() {
    let root = workspace();
    let dice = registry::dice(&root);
    let target = config(&[]);
    let exec = ConfigurationKey::from_slug(target.slug_configuration().unwrap().to_exec().unwrap());
    for label in ["typed_compile", "@@//:typed_compile"] {
        for configuration in [target.clone(), exec.clone()] {
            let result = request(&dice, &root, label, configuration).await.unwrap();
            let recipe = only_recipe(&result);
            let before = recipe.clone();
            let sets = recipe.input_sets().unwrap();
            assert_eq!(
                artifact_paths(sets.mandatory()),
                ["header.h", "static.a", "dynamic.so", "allocator_library.cc"]
            );
            assert!(artifact_paths(sets.spawn()).contains(&"textual.h".to_owned()));
            let bytes = b"not-authenticated-output.o: allocator_library.cc ordinary.h ordinary.h implementation.h /proof/execroot/main/public.h\n";
            for _ in 0..2 {
                let resolved = reconcile(recipe, bytes, &[]).unwrap();
                assert_eq!(
                    artifact_paths(resolved.discovered()),
                    ["ordinary.h", "implementation.h", "public.h"]
                );
                assert_eq!(
                    artifact_paths(resolved.post_discovery()),
                    [
                        "header.h",
                        "static.a",
                        "dynamic.so",
                        "allocator_library.cc",
                        "ordinary.h",
                        "implementation.h",
                        "public.h"
                    ]
                );
                assert_eq!(recipe, &before);
            }
            assert!(result.actions()[0].render_argv().is_err());
            assert!(
                slug_build_api_v2::ReapiCommandProjection::from_action(&result.actions()[0])
                    .is_err()
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_native_compile_keeps_cache_only_inputs_out_of_spawn_and_after_pruning() {
    let root = workspace();
    let dice = registry::dice(&root);
    let result = request(&dice, &root, "@@//:registration_lifecycle", config(&[]))
        .await
        .unwrap();
    let recipes = result
        .actions()
        .iter()
        .filter_map(|action| action.cpp_compile_spec())
        .collect::<Vec<_>>();
    assert_eq!(recipes.len(), 3);
    for recipe in recipes {
        let before = recipe.clone();
        let sets = recipe.input_sets().unwrap();
        assert!(artifact_paths(sets.mandatory()).contains(&"changed.h".to_owned()));
        assert!(!artifact_paths(sets.spawn()).contains(&"changed.h".to_owned()));
        let resolved = reconcile(recipe, b"out: allocator_library.cc\n", &[]).unwrap();
        assert!(
            resolved.discovered().is_empty(),
            "primary source is excluded"
        );
        assert_eq!(resolved.post_discovery(), sets.mandatory());
        assert_eq!(recipe, &before);
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn published_dotd_reconciliation_restores_header_metadata_and_strict_owner_policy() {
    let root = workspace();
    // A separate genuine provider supplies the source resolver witness without
    // adding it to the compiler's declared/mandatory headers.
    fs::create_dir_all(root.join("own-system")).unwrap();
    fs::write(
        root.join("own-system/unlisted.h"),
        "// explicitly unlisted header\n",
    )
    .unwrap();
    let build = root.join("BUILD.bazel");
    let original = format!(
        "{}\nexports_files(['own-system/unlisted.h'])\ncomposition(name='system_witness',leaf='witness',public=[':own-system/unlisted.h'])\n",
        fs::read_to_string(&build).unwrap()
    );
    fs::write(&build, &original).unwrap();
    let dice = registry::dice(&root);
    let witness = request(&dice, &root, "@@//:system_witness", config(&[]))
        .await
        .unwrap();
    let files = values(member(field(&witness, "context"), "headers"));
    assert_eq!(files.len(), 1);
    let AnalysisValueKind::Artifact(unlisted) = files[0].kind() else {
        panic!("genuine File witness");
    };
    let unlisted = [unlisted.clone()];
    let first = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let baseline = reconcile(only_recipe(&first), b"out: ordinary.h\n", &[]).unwrap();
    let permissive = reconcile(
        only_recipe(&first),
        b"out: own-system/unlisted.h\n",
        &unlisted,
    )
    .unwrap();
    assert_eq!(permissive.discovered(), unlisted);
    fs::write(
        &build,
        original.replace("public=['@@//:ordinary.h']", "public=['@@//:changed.h']"),
    )
    .unwrap();
    let changed = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    let changed_inputs = reconcile(only_recipe(&changed), b"out: changed.h\n", &[]).unwrap();
    assert_ne!(baseline, changed_inputs);
    assert!(reconcile(only_recipe(&changed), b"out: ordinary.h\n", &[]).is_err());
    fs::write(&build, &original).unwrap();
    let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(
        baseline,
        reconcile(only_recipe(&restored), b"out: ordinary.h\n", &[]).unwrap()
    );

    let strict = request(
        &dice,
        &root,
        "@@//:typed_compile",
        config(&[(NativeCommandOption::StrictSystemIncludes, "true")]),
    )
    .await
    .unwrap();
    let recipe = only_recipe(&strict);
    assert!(
        CppFragmentProjection::new(recipe.object_output().root().configuration().clone())
            .unwrap()
            .strict_system_includes()
            .unwrap()
    );
    assert!(
        !CppFragmentProjection::new(
            only_recipe(&first)
                .object_output()
                .root()
                .configuration()
                .clone()
        )
        .unwrap()
        .strict_system_includes()
        .unwrap()
    );
    let error = reconcile(recipe, b"out: own-system/unlisted.h\n", &unlisted).unwrap_err();
    assert!(
        error.contains("undeclared inclusions") && error.contains("own-system/unlisted.h"),
        "{error}"
    );
    let restored = request(&dice, &root, "@@//:typed_compile", config(&[]))
        .await
        .unwrap();
    assert_eq!(first, restored);
    assert_eq!(
        permissive,
        reconcile(
            only_recipe(&restored),
            b"out: own-system/unlisted.h\n",
            &unlisted
        )
        .unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}
